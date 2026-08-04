use bevy::text::Justify;
use crate::aseprite_assets::OptionsCursor;
use crate::aseprite_helpers::aseprite_bundle;
use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use bevy::input::gamepad::GamepadButton;
use leafwing_input_manager::prelude::ActionState;

use crate::gamepad_bindings::{
    format_binding_label, gamepad_connected, BindingLabel, ConnectedGamepads, GamepadBindingButton,
    GamepadMappings,
};
use crate::gamepad_input::{UiGamepadAction, UiGamepadInputMarker};
use crate::{
    aim::AimSensitivity,
    assets::Graphics,
    audio::{AudioSoundEffect, AudioVolume, SoundSpawner},
    client::{GameData, PersistedOptionsSettings},
    colors::{DARK_GREEN, WHITE, YELLOW_2},
    cursor::CursorColorSettings,
    cursor::CursorPos,
    datafiles,
    inputs::{AutoAttackState, MouselessModeState, SwapMovementAimKeysState},
    keybinds::InputMappings,
    player::skills::VISIBLE_CLASS_SKILL_COUNT,
    ui::{
        focus::{
            focus_entity_visible, ui_nav_dir_just_pressed, FocusInput, FocusNavBlocked,
            FocusNavBottomRow, FocusNavHorizontalSkip, FocusNavTabColumn, Focusable,
            ModalFocusable, UiFocus, UiNavDir, UiNavStickStability, UiStickNavLatch,
        },
        game_fonts as gf,
        interactions::{set_sprite_image, Interaction},
        main_menu::{spawn_exit_icon_button, spawn_main_menu_wide_button, MenuButton},
        ui_helpers, Interactable, UIElement, UIState,
    },
    DisplayScaleSettings, InputBinding, ScreenResolution,
};

/// Resource to track cheat settings and accessibility options
#[derive(Resource, Debug, Clone)]
pub struct CheatSettings {
    /// When true, all classes and pets are selectable regardless of unlock status
    pub bypass_class_unlocks: bool,
    /// When true, boss damage warning indicators use a color-blind friendly color (dark purple) instead of red
    pub color_blind_mode: bool,
    /// When true, dev tools (XP, spawn chest/tome/orb, era teleport, endless) are shown in the inventory
    pub dev_mode: bool,
    /// When true, damage numbers are shown when enemies take damage.
    /// Player HP loss is always shown regardless of this setting.
    pub show_enemy_damage_numbers: bool,
    /// Size of damage/healing/regen floating text and item pickup labels.
    pub damage_text_size: gf::DamageTextSize,
    /// When true, player HP healing/regen and MP gain floating numbers are shown.
    /// Player HP loss is always shown regardless of this setting.
    pub show_player_damage_numbers: bool,
    /// When true, the tile under the cursor is highlighted during gameplay
    pub show_tile_hover: bool,
    /// When true, heirloom level-up / chest pools ignore time crystal progress and use
    /// the full pool (same as completing every crystal).
    pub bypass_time_crystal_pool: bool,
    pub hide_attack_anims: bool,
    pub hide_skill_anims: bool,
    pub hide_heirloom_anims: bool,
    /// When true, inventory material drop filters are kept when a run ends.
    pub persist_item_filters: bool,
}

impl Default for CheatSettings {
    fn default() -> Self {
        Self {
            bypass_class_unlocks: false,
            color_blind_mode: false,
            dev_mode: false,
            show_enemy_damage_numbers: true,
            damage_text_size: gf::DamageTextSize::default(),
            show_player_damage_numbers: true,
            show_tile_hover: false,
            bypass_time_crystal_pool: false,
            hide_attack_anims: false,
            hide_skill_anims: false,
            hide_heirloom_anims: false,
            persist_item_filters: false,
        }
    }
}

impl PersistedOptionsSettings {
    pub fn from_cheat_settings(settings: &CheatSettings) -> Self {
        Self {
            color_blind_mode: settings.color_blind_mode,
            show_enemy_damage_numbers: settings.show_enemy_damage_numbers,
            show_player_damage_numbers: settings.show_player_damage_numbers,
            show_tile_hover: settings.show_tile_hover,
            damage_text_size: settings.damage_text_size,
            hide_attack_anims: settings.hide_attack_anims,
            hide_skill_anims: settings.hide_skill_anims,
            hide_heirloom_anims: settings.hide_heirloom_anims,
            persist_item_filters: settings.persist_item_filters,
            ..Default::default()
        }
    }

    pub fn apply_to(&self, settings: &mut CheatSettings) {
        settings.color_blind_mode = self.color_blind_mode;
        settings.show_enemy_damage_numbers = self.show_enemy_damage_numbers;
        settings.show_player_damage_numbers = self.show_player_damage_numbers;
        settings.show_tile_hover = self.show_tile_hover;
        settings.damage_text_size = self.damage_text_size;
        settings.hide_attack_anims = self.hide_attack_anims;
        settings.hide_skill_anims = self.hide_skill_anims;
        settings.hide_heirloom_anims = self.hide_heirloom_anims;
        settings.persist_item_filters = self.persist_item_filters;
    }

    fn from_game_data(game_data: &GameData) -> Self {
        let mut settings = game_data
            .options_settings
            .clone()
            .unwrap_or_else(|| Self::from_legacy_game_data(game_data));
        settings.normalize_legacy_fields();
        settings
    }
}

impl CheatSettings {
    fn load_game_data() -> GameData {
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            GameData::default()
        }
    }

    fn write_game_data(game_data: &GameData) {
        let path = datafiles::game_data();
        if let Ok(file) = File::create(&path) {
            let writer = BufWriter::new(file);
            let _ = serde_json::to_writer_pretty(writer, game_data);
        }
    }

    /// Builds settings from defaults, then overrides persisted fields from `game_data.json`.
    pub fn load_from_game_data() -> Self {
        let mut settings = Self::default();
        let game_data = Self::load_game_data();
        if let Some(v) = game_data.bypass_class_unlocks {
            settings.bypass_class_unlocks = v;
        }
        PersistedOptionsSettings::from_game_data(&game_data).apply_to(&mut settings);
        settings
    }

    /// Persists non-cheat accessibility/display toggles to `game_data.json`.
    pub fn persist_persisted_options(&self) {
        let mut game_data = Self::load_game_data();
        game_data.options_settings = Some(PersistedOptionsSettings::from_cheat_settings(self));
        Self::write_game_data(&game_data);
    }

    pub fn checkbox_value(&self, kind: OptionsCheckboxType) -> bool {
        match kind {
            OptionsCheckboxType::UnlockAllClasses => self.bypass_class_unlocks,
            OptionsCheckboxType::ColorBlindMode => self.color_blind_mode,
            OptionsCheckboxType::DevMode => self.dev_mode,
            OptionsCheckboxType::ShowEnemyDamageNumbers => self.show_enemy_damage_numbers,
            OptionsCheckboxType::ShowPlayerDamageNumbers => self.show_player_damage_numbers,
            OptionsCheckboxType::ShowTileHover => self.show_tile_hover,
            OptionsCheckboxType::PersistItemFilters => self.persist_item_filters,
            OptionsCheckboxType::BypassTimeCrystalPool => self.bypass_time_crystal_pool,
            OptionsCheckboxType::HideAttackAnims => self.hide_attack_anims,
            OptionsCheckboxType::HideSkillAnims => self.hide_skill_anims,
            OptionsCheckboxType::HideHeirloomAnims => self.hide_heirloom_anims,
            _ => false,
        }
    }

    pub fn toggle_checkbox(&mut self, kind: OptionsCheckboxType) -> bool {
        let value = match kind {
            OptionsCheckboxType::UnlockAllClasses => {
                self.bypass_class_unlocks = !self.bypass_class_unlocks;
                Self::persist_bypass_class_unlocks(self.bypass_class_unlocks);
                self.bypass_class_unlocks
            }
            OptionsCheckboxType::ColorBlindMode => {
                self.color_blind_mode = !self.color_blind_mode;
                self.color_blind_mode
            }
            OptionsCheckboxType::DevMode => {
                self.dev_mode = !self.dev_mode;
                self.dev_mode
            }
            OptionsCheckboxType::ShowEnemyDamageNumbers => {
                self.show_enemy_damage_numbers = !self.show_enemy_damage_numbers;
                self.show_enemy_damage_numbers
            }
            OptionsCheckboxType::ShowPlayerDamageNumbers => {
                self.show_player_damage_numbers = !self.show_player_damage_numbers;
                self.show_player_damage_numbers
            }
            OptionsCheckboxType::ShowTileHover => {
                self.show_tile_hover = !self.show_tile_hover;
                self.show_tile_hover
            }
            OptionsCheckboxType::PersistItemFilters => {
                self.persist_item_filters = !self.persist_item_filters;
                self.persist_item_filters
            }
            OptionsCheckboxType::BypassTimeCrystalPool => {
                self.bypass_time_crystal_pool = !self.bypass_time_crystal_pool;
                self.bypass_time_crystal_pool
            }
            OptionsCheckboxType::HideAttackAnims => {
                self.hide_attack_anims = !self.hide_attack_anims;
                self.hide_attack_anims
            }
            OptionsCheckboxType::HideSkillAnims => {
                self.hide_skill_anims = !self.hide_skill_anims;
                self.hide_skill_anims
            }
            OptionsCheckboxType::HideHeirloomAnims => {
                self.hide_heirloom_anims = !self.hide_heirloom_anims;
                self.hide_heirloom_anims
            }
            _ => unreachable!("checkbox is not stored on CheatSettings: {kind:?}"),
        };

        if kind.persists_to_game_data() {
            self.persist_persisted_options();
        }

        value
    }

    pub fn persist_bypass_class_unlocks(bypass_class_unlocks: bool) {
        let mut game_data = Self::load_game_data();
        game_data.bypass_class_unlocks = Some(bypass_class_unlocks);
        Self::write_game_data(&game_data);
    }
}

/// Identifies which option an options-screen checkbox controls
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionsCheckboxType {
    UnlockAllClasses,
    ColorBlindMode,
    DevMode,
    ShowEnemyDamageNumbers,
    ShowPlayerDamageNumbers,
    ShowTileHover,
    PersistItemFilters,
    BypassTimeCrystalPool,
    HideAttackAnims,
    HideSkillAnims,
    HideHeirloomAnims,
    DoubleCursorSize,
    AutoAttack,
    MouselessMode,
    SwapMovementAimKeys,
}

impl OptionsCheckboxType {
    pub const fn uses_cheat_settings(self) -> bool {
        matches!(
            self,
            Self::UnlockAllClasses
                | Self::ColorBlindMode
                | Self::DevMode
                | Self::ShowEnemyDamageNumbers
                | Self::ShowPlayerDamageNumbers
                | Self::ShowTileHover
                | Self::PersistItemFilters
                | Self::BypassTimeCrystalPool
                | Self::HideAttackAnims
                | Self::HideSkillAnims
                | Self::HideHeirloomAnims
        )
    }

    pub const fn persists_to_game_data(self) -> bool {
        matches!(
            self,
            Self::ColorBlindMode
                | Self::ShowEnemyDamageNumbers
                | Self::ShowPlayerDamageNumbers
                | Self::ShowTileHover
                | Self::PersistItemFilters
                | Self::HideAttackAnims
                | Self::HideSkillAnims
                | Self::HideHeirloomAnims
        )
    }
}

fn options_checkbox_ui(checked: bool) -> UIElement {
    if checked {
        UIElement::CheckBoxSelected
    } else {
        UIElement::CheckBox
    }
}

#[derive(Component)]
pub struct OptionsCheckbox(pub OptionsCheckboxType);

/// Color for boss damage warning indicators. When color blind mode is on, uses dark purple (visible on green/blue backgrounds).
pub fn boss_warning_indicator_color(settings: &CheatSettings) -> Color {
    if settings.color_blind_mode {
        Color::srgba(0.35, 0.0, 0.5, 0.3)
    } else {
        Color::srgba(1.0, 0.0, 0.0, 0.3)
    }
}

#[derive(Component)]
pub struct OptionsUI;

/// Incremented when the options menu must rebuild at a new UI scale while staying open.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct OptionsUiLayoutRevision(pub u32);

/// Set when the options menu is (re)built so control labels refresh for the active input device.
#[derive(Resource, Debug, Clone, Copy)]
pub struct OptionsControlsNeedsLabelSync;

/// Shifts tab body content up; title and bottom button row stay put.
const OPTIONS_BODY_Y_OFFSET: f32 = 20.;

const OPTIONS_TAB_GREY: Color = Color::srgba(0.25, 0.25, 0.25, 1.0);
const OPTIONS_TAB_BUTTON_SIZE: Vec2 = Vec2::new(72., 18.);
const OPTIONS_TAB_SPACING: f32 = 24.;
const OPTIONS_CONTENT_ROW_SPACING: f32 = -18.;
const OPTIONS_CONTROLS_ROW_SPACING: f32 = -16.;
const OPTIONS_COLUMN_WIDTH: f32 = 168.;
const OPTIONS_COLUMN_GAP: f32 = 14.;

fn options_content_pane_bounds(resolution: &ScreenResolution) -> (f32, f32) {
    let left = options_content_x(resolution);
    let right = resolution.game_width / 2. - 70.;
    (left, right)
}

fn options_content_column_x(resolution: &ScreenResolution, column_index: u32) -> f32 {
    options_content_x(resolution)
        + column_index as f32 * (OPTIONS_COLUMN_WIDTH + OPTIONS_COLUMN_GAP)
}

/// Horizontally centers a single content column within the pane to the right of the tab strip.
fn options_single_column_x(resolution: &ScreenResolution) -> f32 {
    let (left, right) = options_content_pane_bounds(resolution);
    left + (right - left - OPTIONS_COLUMN_WIDTH) * 0.5
}

/// Main options menu categories shown in the left tab column.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum OptionsTab {
    #[default]
    Gameplay,
    Controls,
    Audio,
    Video,
}

impl OptionsTab {
    pub const ALL: [OptionsTab; 4] = [
        OptionsTab::Gameplay,
        OptionsTab::Controls,
        OptionsTab::Audio,
        OptionsTab::Video,
    ];

    pub fn label(self) -> &'static str {
        match self {
            OptionsTab::Gameplay => "Gameplay",
            OptionsTab::Controls => "Controls",
            OptionsTab::Audio => "Audio",
            OptionsTab::Video => "Video",
        }
    }

    pub fn focus_index(self) -> u32 {
        match self {
            OptionsTab::Gameplay => 0,
            OptionsTab::Controls => 1,
            OptionsTab::Audio => 2,
            OptionsTab::Video => 3,
        }
    }
}

/// Which options tab is currently displayed in the content pane.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ActiveOptionsTab(pub OptionsTab);

#[derive(Component, Clone, Copy)]
pub struct OptionsTabButton(pub OptionsTab);

#[derive(Component, Clone, Copy)]
pub struct OptionsTabContent(pub OptionsTab);

/// Identifies an options setting row for keyboard/controller focus.
#[derive(Clone, Copy, Debug)]
pub enum OptionsRowKind {
    Checkbox(OptionsCheckboxType),
    Keybind(KeyBindType),
    Volume(VolumeChannel),
    Scale(ScaleChannel),
    CursorColor,
    Sensitivity,
    NavStickStability,
    DamageTextSize,
}

#[derive(Component, Clone, Copy)]
pub struct OptionsFocusRow(pub OptionsRowKind);

/// Label text for a row; highlights when the row is focused or hovered.
#[derive(Component, Clone, Copy)]
pub struct OptionsRowLabel {
    pub row: Entity,
}

/// 8×8 cursor shown left of a focused/hovered options row label.
#[derive(Component)]
pub(crate) struct OptionsRowCursor {
    label: Entity,
}

const OPTIONS_CURSOR_SIZE: f32 = 8.;
const OPTIONS_CURSOR_GAP: f32 = 4.;
/// Offset from the label's left edge (CenterLeft) to the cursor center.
const OPTIONS_CURSOR_OFFSET_X: f32 = -(OPTIONS_CURSOR_SIZE * 0.5 + OPTIONS_CURSOR_GAP);

/// Child control (e.g. stepper arrow) belonging to a row focus target.
#[derive(Component, Clone, Copy)]
pub struct OptionsRowMember {
    pub row: Entity,
}

fn tab_visibility(tab: OptionsTab, active: OptionsTab) -> Visibility {
    if tab == active {
        Visibility::Visible
    } else {
        Visibility::Hidden
    }
}

fn options_pointcast<'a>(
    cursor_pos: &Res<CursorPos>,
    ui_sprites: &'a Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    computed_visibility: &Query<&ViewVisibility>,
) -> Option<(Entity, &'a Sprite, &'a GlobalTransform)> {
    ui_helpers::pointcast_2d(cursor_pos, ui_sprites, None, Some(computed_visibility))
}

fn options_focus(index: u32) -> Focusable {
    Focusable {
        group: UIState::Options,
        index,
    }
}

fn spawn_stepper_row_focus(
    commands: &mut Commands,
    label_pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
    row_kind: OptionsRowKind,
    name: &str,
) -> Entity {
    let row_center_x = label_pos.x + 65.;
    commands
        .spawn((
            (
                Sprite {
                    color: Color::srgba(0., 0., 0., 0.),
                    custom_size: Some(Vec2::new(150., 16.)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(
                    row_center_x,
                    label_pos.y,
                    label_pos.z - 0.01,
                )),
                tab_visibility(tab, active),
            ),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            OptionsUI,
            OptionsTabContent(tab),
            OptionsFocusRow(row_kind),
            FocusNavHorizontalSkip,
            Interactable::default(),
            options_focus(focus_index),
            Name::new(name.to_string()),
        ))
        .id()
}

fn options_tab_x(resolution: &ScreenResolution) -> f32 {
    -resolution.game_width / 2. + 38.
}

fn options_content_x(resolution: &ScreenResolution) -> f32 {
    -resolution.game_width / 2. + 98.
}

/// Marker for the "Wipe Game Data" confirmation popup (overlay + panel + buttons).
#[derive(Component)]
pub struct WipeDataPopup;

#[derive(Component)]
pub struct KeyBindButton {
    bind_type: KeyBindType,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum KeyBindType {
    ActiveSkill(usize),
    /// Consume/use the item sitting in hotbar slot `usize` (0..=3).
    /// Note this is not an "active slot" binding — there is no selected slot anymore.
    Hotbar(usize),
    Inventory,
    Minimap,
    Interact,
    AttackAutoTarget,
}

#[derive(Component)]
pub struct KeyBindText {
    bind_type: KeyBindType,
}

#[derive(Component)]
pub struct WaitingForKeyInput {
    pub bind_type: KeyBindType,
    /// Ignore mouse presses briefly so the click that opened rebind is not captured.
    pub ignore_mouse_frames: u8,
    /// When true, capture a gamepad button instead of keyboard/mouse.
    pub capture_gamepad: bool,
}

/// Section headers on the Controls tab whose suffix reflects keyboard vs controller editing.
#[derive(Component, Clone, Copy)]
pub struct OptionsControlsSectionTitle(pub &'static str);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VolumeChannel {
    Global,
    Music,
    Sfx,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VolumeDirection {
    Down,
    Up,
}

#[derive(Component)]
pub struct VolumeButton {
    pub channel: VolumeChannel,
    pub direction: VolumeDirection,
}

#[derive(Component)]
pub struct VolumeValueText {
    pub channel: VolumeChannel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ScaleChannel {
    Game,
    Ui,
}

#[derive(Component)]
pub struct ScaleButton {
    pub channel: ScaleChannel,
    pub direction: VolumeDirection,
}

#[derive(Component)]
pub struct ScaleValueText {
    pub channel: ScaleChannel,
}

#[derive(Component)]
pub struct CursorColorButton {
    pub direction: VolumeDirection,
}

/// Marker for the sprite that previews the currently selected cursor color.
#[derive(Component)]
pub struct CursorColorPreview;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SensitivitySetting {
    Aim,
    UiNavStick,
}

#[derive(Component)]
pub struct SensitivityButton {
    pub direction: VolumeDirection,
    pub setting: SensitivitySetting,
}

#[derive(Component)]
pub struct SensitivityValueText(pub SensitivitySetting);

#[derive(Component)]
pub struct DamageTextSizeButton {
    pub direction: VolumeDirection,
}

#[derive(Component)]
pub struct DamageTextSizeValueText;

pub fn handle_options_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    gamepads: ConnectedGamepads,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &KeyBindButton), Without<WaitingForKeyInput>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::XLKeyHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::XLKeyHover),
                    );
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        commands.entity(entity).insert(WaitingForKeyInput {
                            bind_type: button.bind_type,
                            ignore_mouse_frames: 2,
                            capture_gamepad: gamepad_connected(&gamepads),
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::XLKey);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::XLKey),
            );
        }
    }
}

pub fn handle_key_rebind_input(
    mut commands: Commands,
    mut key_input: ResMut<ButtonInput<KeyCode>>,
    mut mouse_input: ResMut<ButtonInput<MouseButton>>,
    mut keybinds: ResMut<InputMappings>,
    mut gamepad_mappings: ResMut<GamepadMappings>,
    // Bevy 0.19: gamepad digital state lives on each `Gamepad` component, not a global resource.
    gamepads: Query<&Gamepad>,
    mut waiting: Query<(Entity, &mut WaitingForKeyInput)>,
    graphics: Res<Graphics>,
) {
    if waiting.is_empty() {
        return;
    }

    let just_pressed_key: Vec<KeyCode> = key_input.get_just_pressed().copied().collect();
    let just_pressed_mouse: Vec<MouseButton> = mouse_input.get_just_pressed().copied().collect();
    let just_pressed_gamepad: Vec<GamepadButton> = gamepads
        .iter()
        .flat_map(|gamepad| gamepad.get_just_pressed().copied())
        .collect();

    for (_, mut waiting_for) in waiting.iter_mut() {
        if waiting_for.ignore_mouse_frames > 0 {
            waiting_for.ignore_mouse_frames -= 1;
        }
    }

    let capture_gamepad = waiting.iter().any(|(_, w)| w.capture_gamepad);

    for key in &just_pressed_key {
        if *key == KeyCode::Escape {
            for (entity, _) in waiting.iter() {
                commands.entity(entity).remove::<WaitingForKeyInput>();
                commands.entity(entity).insert(UIElement::BackButton);
                set_sprite_image(
                    &mut commands,
                    entity,
                    graphics.get_ui_element_texture(UIElement::BackButton),
                );
            }
            key_input.clear();
            return;
        }
    }

    if capture_gamepad {
        for button in just_pressed_gamepad {
            if button == GamepadButton::East {
                for (entity, _) in waiting.iter() {
                    commands.entity(entity).remove::<WaitingForKeyInput>();
                    commands.entity(entity).insert(UIElement::BackButton);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::BackButton),
                    );
                }
                return;
            }

            let Some(binding) = GamepadBindingButton::from_button_type(button) else {
                continue;
            };

            for (entity, waiting_for) in waiting.iter() {
                apply_gamepad_rebind(&mut gamepad_mappings, waiting_for.bind_type, binding);
                gamepad_mappings.save();
                commands.entity(entity).remove::<WaitingForKeyInput>();
                commands.entity(entity).insert(UIElement::BackButton);
                set_sprite_image(
                    &mut commands,
                    entity,
                    graphics.get_ui_element_texture(UIElement::BackButton),
                );
                commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillSelection, 0.15));
            }
            return;
        }
        return;
    }

    for key in just_pressed_key {
        for (entity, waiting_for) in waiting.iter() {
            match waiting_for.bind_type {
                KeyBindType::ActiveSkill(slot) => {
                    let binding = InputBinding::KeyBinding(key);
                    keybinds.clear_active_skill_binding_from_other_slots(binding, slot);
                    keybinds.set_active_skill_key(slot, binding);
                }
                KeyBindType::Hotbar(slot) => {
                    keybinds.set_hotbar_key(slot, InputBinding::KeyBinding(key))
                }
                KeyBindType::Inventory => keybinds.set_inventory_key(InputBinding::KeyBinding(key)),
                KeyBindType::Minimap => keybinds.set_minimap_key(InputBinding::KeyBinding(key)),
                KeyBindType::Interact => keybinds.set_interact_key(InputBinding::KeyBinding(key)),
                KeyBindType::AttackAutoTarget => {
                    keybinds.set_attack_auto_target_key(InputBinding::KeyBinding(key))
                }
            }
            keybinds.save();
            commands.entity(entity).remove::<WaitingForKeyInput>();
            commands.entity(entity).insert(UIElement::BackButton);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::BackButton),
            );
            commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillSelection, 0.15));
        }
        key_input.clear();
        break;
    }

    for mouse_button in just_pressed_mouse {
        for (entity, waiting_for) in waiting.iter() {
            if waiting_for.ignore_mouse_frames > 0 {
                continue;
            }
            match waiting_for.bind_type {
                KeyBindType::ActiveSkill(slot) => {
                    let binding = InputBinding::MouseBinding(mouse_button);
                    keybinds.clear_active_skill_binding_from_other_slots(binding, slot);
                    keybinds.set_active_skill_key(slot, binding);
                }
                KeyBindType::Hotbar(slot) => {
                    keybinds.set_hotbar_key(slot, InputBinding::MouseBinding(mouse_button))
                }
                KeyBindType::Inventory => {
                    keybinds.set_inventory_key(InputBinding::MouseBinding(mouse_button))
                }
                KeyBindType::Minimap => {
                    keybinds.set_minimap_key(InputBinding::MouseBinding(mouse_button))
                }
                KeyBindType::Interact => {
                    keybinds.set_interact_key(InputBinding::MouseBinding(mouse_button))
                }
                KeyBindType::AttackAutoTarget => {
                    keybinds.set_attack_auto_target_key(InputBinding::MouseBinding(mouse_button))
                }
            }
            keybinds.save();
            commands.entity(entity).remove::<WaitingForKeyInput>();
            commands.entity(entity).insert(UIElement::BackButton);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::BackButton),
            );
            commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillSelection, 0.15));
        }
        mouse_input.clear();
        break;
    }
}

fn apply_gamepad_rebind(
    mappings: &mut GamepadMappings,
    bind_type: KeyBindType,
    button: GamepadBindingButton,
) {
    match bind_type {
        KeyBindType::ActiveSkill(slot) => {
            mappings.clear_active_skill_button_from_other_slots(button, slot);
            mappings.set_active_skill_button(slot, button);
        }
        KeyBindType::Hotbar(slot) => mappings.set_hotbar_button(slot, button),
        KeyBindType::Inventory => mappings.set_inventory_button(button),
        KeyBindType::Minimap => mappings.set_minimap_button(button),
        KeyBindType::Interact => mappings.set_interact_button(button),
        KeyBindType::AttackAutoTarget => mappings.set_attack_auto_target_button(button),
    }
}

fn options_display_binding(
    bind_type: KeyBindType,
    keybinds: &InputMappings,
    gamepad_mappings: &GamepadMappings,
    gamepad_connected: bool,
) -> String {
    format_binding_label(
        bind_type.into(),
        keybinds,
        gamepad_mappings,
        gamepad_connected,
    )
}

impl From<KeyBindType> for BindingLabel {
    fn from(bind_type: KeyBindType) -> Self {
        match bind_type {
            KeyBindType::ActiveSkill(slot) => BindingLabel::ActiveSkill(slot),
            KeyBindType::Hotbar(slot) => BindingLabel::Hotbar(slot),
            KeyBindType::Inventory => BindingLabel::Inventory,
            KeyBindType::Minimap => BindingLabel::Minimap,
            KeyBindType::Interact => BindingLabel::Interact,
            KeyBindType::AttackAutoTarget => BindingLabel::AttackAutoTarget,
        }
    }
}

pub fn update_keybind_text(
    keybinds: Res<InputMappings>,
    gamepad_mappings: Res<GamepadMappings>,
    gamepads: ConnectedGamepads,
    needs_sync: Option<Res<OptionsControlsNeedsLabelSync>>,
    waiting: Query<&WaitingForKeyInput>,
    mut texts: Query<(&KeyBindText, &mut Text2d, &mut TextColor)>,
    mut was_waiting: Local<bool>,
    mut last_gamepad_connected: Local<Option<bool>>,
) {
    let waiting_binds: Vec<KeyBindType> = waiting.iter().map(|w| w.bind_type).collect();
    let is_waiting = !waiting_binds.is_empty();
    let use_gamepad = gamepad_connected(&gamepads);
    let device_changed = last_gamepad_connected.map_or(true, |prev| prev != use_gamepad);
    let force_sync = needs_sync.is_some();
    *last_gamepad_connected = Some(use_gamepad);

    if !keybinds.is_changed()
        && !gamepad_mappings.is_changed()
        && !is_waiting
        && !*was_waiting
        && !device_changed
        && !force_sync
    {
        return;
    }

    *was_waiting = is_waiting;

    for (key_text, mut text, mut text_color) in texts.iter_mut() {
        if waiting_binds.contains(&key_text.bind_type) {
            let waiting_gamepad = waiting
                .iter()
                .find(|w| w.bind_type == key_text.bind_type)
                .map(|w| w.capture_gamepad)
                .unwrap_or(use_gamepad);
            text.0 = if waiting_gamepad {
                "Press any button...".to_string()
            } else {
                "Press any key...".to_string()
            };
            text_color.0 = WHITE;
        } else {
            text.0 = options_display_binding(
                key_text.bind_type,
                &keybinds,
                &gamepad_mappings,
                gamepad_connected(&gamepads),
            );
            text_color.0 = WHITE;
        }
    }
}

pub fn update_options_controls_section_titles(
    gamepads: ConnectedGamepads,
    needs_sync: Option<Res<OptionsControlsNeedsLabelSync>>,
    mut commands: Commands,
    mut titles: Query<(&OptionsControlsSectionTitle, &mut Text2d)>,
    mut last_gamepad_connected: Local<Option<bool>>,
) {
    let use_gamepad = gamepad_connected(&gamepads);
    let device_changed = last_gamepad_connected.map_or(true, |prev| prev != use_gamepad);
    let force_sync = needs_sync.is_some();
    if !device_changed && !force_sync {
        return;
    }
    *last_gamepad_connected = Some(use_gamepad);
    if force_sync {
        commands.remove_resource::<OptionsControlsNeedsLabelSync>();
    }

    let suffix = if use_gamepad { " (Controller)" } else { "" };
    for (title, mut text) in titles.iter_mut() {
        text.0 = format!("{}{}", title.0, suffix);
    }
}

pub fn cleanup_options_ui(
    mut commands: Commands,
    query: Query<Entity, With<OptionsUI>>,
    popup: Query<Entity, With<WipeDataPopup>>,
) {
    bevy::log::info!(
        "[overlay] cleanup_options_ui despawning {} OptionsUI entities",
        query.iter().count()
    );
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in popup.iter() {
        commands.entity(entity).despawn();
    }
}

#[derive(SystemParam)]
pub(crate) struct SetupOptionsResources<'w, 's> {
    graphics: Res<'w, Graphics>,
    asset_server: Res<'w, AssetServer>,
    resolution: Res<'w, ScreenResolution>,
    keybinds: Res<'w, InputMappings>,
    game_state: Res<'w, State<crate::GameState>>,
    cheat_settings: Res<'w, CheatSettings>,
    audio_volume: Res<'w, AudioVolume>,
    display_scale: Res<'w, DisplayScaleSettings>,
    cursor_color: Res<'w, CursorColorSettings>,
    auto_attack: Res<'w, AutoAttackState>,
    mouseless_mode: Res<'w, MouselessModeState>,
    swap_movement_aim_keys: Res<'w, SwapMovementAimKeysState>,
    keyboard_aim_sensitivity: Res<'w, AimSensitivity>,
    ui_nav_stick_stability: Res<'w, UiNavStickStability>,
    active_tab: Res<'w, ActiveOptionsTab>,
    existing_ui: Query<'w, 's, Entity, Or<(With<OptionsUI>, With<WipeDataPopup>)>>,
}

pub fn setup_options_ui(mut commands: Commands, deps: SetupOptionsResources) {
    let SetupOptionsResources {
        graphics,
        asset_server,
        resolution,
        keybinds,
        game_state,
        cheat_settings,
        audio_volume,
        display_scale,
        cursor_color,
        auto_attack,
        mouseless_mode,
        swap_movement_aim_keys,
        keyboard_aim_sensitivity,
        ui_nav_stick_stability,
        active_tab,
        existing_ui,
    } = deps;
    for entity in existing_ui.iter() {
        commands.entity(entity).despawn();
    }

    let active = active_tab.0;
    let z = ui_helpers::Z_DEPTH_OPTIONS_CONTENT;

    let overlay = ui_helpers::spawn_full_screen_ui_overlay_tuned(
        &mut commands,
        &resolution,
        0.88,
        0.98,
        ui_helpers::Z_DEPTH_OPTIONS_OVERLAY,
    );
    commands
        .entity(overlay)
        .insert(OptionsUI)
        .insert(UIState::Options);

    commands.spawn((
        gf::MENU_TITLE_LARGE
            .text(&asset_server, "Options", WHITE)
            .justify(Justify::Center)
            .anchor(bevy::sprite::Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., resolution.game_height / 2. - 40., z),
                scale: gf::MENU_TITLE_LARGE.transform_scale(),
                ..default()
            }),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Options Title"),
    ));

    spawn_options_tab_column(&mut commands, &asset_server, &resolution, active, z);

    spawn_gameplay_tab_content(
        &mut commands,
        &graphics,
        &asset_server,
        &resolution,
        active,
        z,
        &cheat_settings,
        &cursor_color,
    );
    spawn_controls_tab_content(
        &mut commands,
        &graphics,
        &asset_server,
        &resolution,
        active,
        z,
        &keybinds,
        auto_attack.0,
        mouseless_mode.0,
        swap_movement_aim_keys.0,
        keyboard_aim_sensitivity.0,
        ui_nav_stick_stability.0,
    );
    spawn_audio_tab_content(
        &mut commands,
        &graphics,
        &asset_server,
        &resolution,
        active,
        z,
        &audio_volume,
    );
    spawn_video_tab_content(
        &mut commands,
        &graphics,
        &asset_server,
        &resolution,
        active,
        z,
        &display_scale,
        &cheat_settings,
    );

    if *game_state == crate::GameState::Main {
        let tutorial_btn = spawn_main_menu_wide_button(
            Vec3::new(-75., -156., z),
            "Show Tutorial",
            MenuButton::ShowTutorial,
            UIElement::MainMenuStartButton,
            &mut commands,
            &graphics,
            &asset_server,
        );
        commands
            .entity(tutorial_btn)
            .insert((OptionsUI, options_focus(500), FocusNavBottomRow));

        let exit_button = spawn_main_menu_wide_button(
            Vec3::new(75., -156., z),
            "Exit to Menu",
            MenuButton::OptionsExit,
            UIElement::MainMenuStartButton,
            &mut commands,
            &graphics,
            &asset_server,
        );
        commands
            .entity(exit_button)
            .insert((OptionsUI, options_focus(501), FocusNavBottomRow));
    } else {
        let wipe_btn = spawn_main_menu_wide_button(
            Vec3::new(0., -156., z),
            "Wipe Data",
            MenuButton::WipeGameData,
            UIElement::MainMenuStartButton,
            &mut commands,
            &graphics,
            &asset_server,
        );
        commands
            .entity(wipe_btn)
            .insert((OptionsUI, options_focus(500), FocusNavBottomRow));
    }

    let back_button = spawn_exit_icon_button(Vec3::new(255., -156., z), &mut commands, &graphics);
    commands
        .entity(back_button)
        .insert((OptionsUI, options_focus(502), FocusNavBottomRow));

    commands.insert_resource(OptionsControlsNeedsLabelSync);
}

fn spawn_options_tab_column(
    commands: &mut Commands,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    active: OptionsTab,
    z: f32,
) {
    let tab_x = options_tab_x(resolution);
    let first_tab_y = 80. + OPTIONS_BODY_Y_OFFSET;

    for (i, tab) in OptionsTab::ALL.iter().enumerate() {
        let y = first_tab_y - i as f32 * OPTIONS_TAB_SPACING;
        let is_active = *tab == active;
        let button_e = commands
            .spawn((
                (
                    Sprite {
                        color: if is_active {
                            DARK_GREEN
                        } else {
                            OPTIONS_TAB_GREY
                        },
                        custom_size: Some(OPTIONS_TAB_BUTTON_SIZE),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(tab_x, y, z)),
                ),
                Interactable::default(),
                RenderLayers::from_layers(&[3]),
                OptionsUI,
                OptionsTabButton(*tab),
                FocusNavTabColumn,
                options_focus(tab.focus_index()),
                Name::new(format!("Options Tab: {}", tab.label())),
            ))
            .id();

        commands
            .spawn((
                gf::MENU_TITLE
                    .text(&asset_server, tab.label(), WHITE)
                    .justify(Justify::Center)
                    .anchor(Anchor::CENTER)
                    .with_transform(Transform {
                        translation: Vec3::new(0., -1., 1.),
                        scale: gf::MENU_TITLE.transform_scale(),
                        ..default()
                    }),
                RenderLayers::from_layers(&[3]),
                OptionsUI,
            ))
            .insert(ChildOf(button_e));
    }
}

fn spawn_controls_section_title(
    commands: &mut Commands,
    asset_server: &AssetServer,
    title: &'static str,
    pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
) {
    commands.spawn((
        (
            gf::MENU_TITLE
                .text(&asset_server, title, YELLOW_2)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: pos,
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsControlsSectionTitle(title),
        Name::new(format!("Options Controls Section: {title}")),
    ));
}

fn spawn_options_section_title(
    commands: &mut Commands,
    asset_server: &AssetServer,
    title: &str,
    pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
) {
    commands.spawn((
        (
            gf::MENU_TITLE
                .text(&asset_server, title, YELLOW_2)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: pos,
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        Name::new(format!("Options Section: {title}")),
    ));
}

fn spawn_gameplay_tab_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    active: OptionsTab,
    z: f32,
    cheat_settings: &CheatSettings,
    cursor_color: &CursorColorSettings,
) {
    let tab = OptionsTab::Gameplay;
    let main_x = options_content_column_x(resolution, 0);
    let cheats_x = options_content_column_x(resolution, 1);
    let checkbox_x_offset = 108.;
    let mut focus = 100u32;
    let top_y = 90. + OPTIONS_BODY_Y_OFFSET;

    let mut y = top_y;
    spawn_options_section_title(
        commands,
        asset_server,
        "General",
        Vec3::new(main_x, y, z),
        tab,
        active,
    );
    y += OPTIONS_CONTENT_ROW_SPACING * 1.5;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Damage Numbers:",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::ShowEnemyDamageNumbers,
        cheat_settings.show_enemy_damage_numbers,
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Player Regen Numbers",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::ShowPlayerDamageNumbers,
        cheat_settings.show_player_damage_numbers,
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Tile Hover:",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::ShowTileHover,
        cheat_settings.show_tile_hover,
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Persist Item Filters:",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::PersistItemFilters,
        cheat_settings.persist_item_filters,
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING * 1.5;

    spawn_options_section_title(
        commands,
        asset_server,
        "Accessibility",
        Vec3::new(main_x, y, z),
        tab,
        active,
    );
    y += OPTIONS_CONTENT_ROW_SPACING * 1.5;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Color Blind Mode:",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::ColorBlindMode,
        cheat_settings.color_blind_mode,
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Double Cursor Size:",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::DoubleCursorSize,
        cursor_color.double_size,
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_damage_text_size_row(
        commands,
        graphics,
        asset_server,
        "Damage text size:",
        cheat_settings.damage_text_size,
        Vec3::new(main_x, y, z),
        tab,
        active,
        focus,
    );
    focus += 1;
    y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_cursor_color_row(
        commands,
        graphics,
        asset_server,
        "Cursor:",
        cursor_color.index,
        Vec3::new(main_x, y, z),
        tab,
        active,
        focus,
    );
    focus += 1;

    let mut cheats_y = top_y;
    spawn_options_section_title(
        commands,
        asset_server,
        "Cheats",
        Vec3::new(cheats_x, cheats_y, z),
        tab,
        active,
    );
    cheats_y += OPTIONS_CONTENT_ROW_SPACING * 1.5;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Unlock All Skills:",
        Vec3::new(cheats_x, cheats_y, z),
        Vec3::new(cheats_x + checkbox_x_offset, cheats_y, z),
        OptionsCheckboxType::UnlockAllClasses,
        cheat_settings.bypass_class_unlocks,
        tab,
        active,
        focus,
    );
    focus += 1;
    cheats_y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Dev Mode:",
        Vec3::new(cheats_x, cheats_y, z),
        Vec3::new(cheats_x + checkbox_x_offset, cheats_y, z),
        OptionsCheckboxType::DevMode,
        cheat_settings.dev_mode,
        tab,
        active,
        focus,
    );
    focus += 1;
    cheats_y += OPTIONS_CONTENT_ROW_SPACING;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Bypass Heirloom Pool:",
        Vec3::new(cheats_x, cheats_y, z),
        Vec3::new(cheats_x + checkbox_x_offset, cheats_y, z),
        OptionsCheckboxType::BypassTimeCrystalPool,
        cheat_settings.bypass_time_crystal_pool,
        tab,
        active,
        focus,
    );
}

fn spawn_controls_tab_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    active: OptionsTab,
    z: f32,
    keybinds: &InputMappings,
    auto_attack: bool,
    mouseless_mode: bool,
    swap_movement_aim_keys: bool,
    aim_sensitivity: u8,
    ui_nav_stick_stability: u8,
) {
    let tab = OptionsTab::Controls;
    let skills_x = options_content_column_x(resolution, 0);
    let other_x = options_content_column_x(resolution, 1);
    let toggles_x = options_content_column_x(resolution, 2);
    let row_spacing = OPTIONS_CONTROLS_ROW_SPACING;
    let checkbox_x_offset = 108.;
    let mut focus = 200u32;
    let top_y = 90. + OPTIONS_BODY_Y_OFFSET;

    let mut skills_y = top_y;
    spawn_controls_section_title(
        commands,
        asset_server,
        "Active Skill",
        Vec3::new(skills_x, skills_y, z),
        tab,
        active,
    );
    skills_y += row_spacing * 1.5;

    for slot in 0..VISIBLE_CLASS_SKILL_COUNT {
        spawn_keybind_row(
            commands,
            graphics,
            asset_server,
            KeyBindType::ActiveSkill(slot),
            Vec3::new(skills_x, skills_y, z),
            Vec3::new(skills_x + 130., skills_y - 3.5, z),
            keybinds,
            tab,
            active,
            focus,
        );
        focus += 1;
        skills_y += row_spacing;
    }

    skills_y += row_spacing * 0.5;
    spawn_controls_section_title(
        commands,
        asset_server,
        "Hotbar",
        Vec3::new(skills_x, skills_y, z),
        tab,
        active,
    );
    skills_y += row_spacing * 1.5;

    for slot in 0..4 {
        spawn_keybind_row(
            commands,
            graphics,
            asset_server,
            KeyBindType::Hotbar(slot),
            Vec3::new(skills_x, skills_y, z),
            Vec3::new(skills_x + 130., skills_y - 3.5, z),
            keybinds,
            tab,
            active,
            focus,
        );
        focus += 1;
        skills_y += row_spacing;
    }

    let mut other_y = top_y;
    spawn_controls_section_title(
        commands,
        asset_server,
        "Other",
        Vec3::new(other_x, other_y, z),
        tab,
        active,
    );
    other_y += row_spacing * 1.5;

    for bind_type in [
        KeyBindType::Inventory,
        KeyBindType::Minimap,
        KeyBindType::Interact,
        KeyBindType::AttackAutoTarget,
    ] {
        spawn_keybind_row(
            commands,
            graphics,
            asset_server,
            bind_type,
            Vec3::new(other_x, other_y, z),
            Vec3::new(other_x + 130., other_y - 3.5, z),
            keybinds,
            tab,
            active,
            focus,
        );
        focus += 1;
        other_y += row_spacing;
    }

    let mut toggles_y = top_y;
    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Toggle Auto Attack:",
        Vec3::new(toggles_x, toggles_y, z),
        Vec3::new(toggles_x + checkbox_x_offset, toggles_y, z),
        OptionsCheckboxType::AutoAttack,
        auto_attack,
        tab,
        active,
        focus,
    );
    focus += 1;
    toggles_y += row_spacing;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Mouseless Mode:",
        Vec3::new(toggles_x, toggles_y, z),
        Vec3::new(toggles_x + checkbox_x_offset, toggles_y, z),
        OptionsCheckboxType::MouselessMode,
        mouseless_mode,
        tab,
        active,
        focus,
    );
    focus += 1;
    toggles_y += row_spacing;

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Swap Move/Aim Keys:",
        Vec3::new(toggles_x, toggles_y, z),
        Vec3::new(toggles_x + checkbox_x_offset, toggles_y, z),
        OptionsCheckboxType::SwapMovementAimKeys,
        swap_movement_aim_keys,
        tab,
        active,
        focus,
    );
    focus += 1;
    toggles_y += row_spacing * 1.5;

    spawn_sensitivity_row(
        commands,
        graphics,
        asset_server,
        "Aim Sensitivity:",
        aim_sensitivity,
        Vec3::new(toggles_x, toggles_y, z),
        tab,
        active,
        focus,
        OptionsRowKind::Sensitivity,
        SensitivitySetting::Aim,
    );
    focus += 1;
    toggles_y += row_spacing;

    spawn_sensitivity_row(
        commands,
        graphics,
        asset_server,
        "UI Nav Stability:",
        ui_nav_stick_stability,
        Vec3::new(toggles_x, toggles_y, z),
        tab,
        active,
        focus,
        OptionsRowKind::NavStickStability,
        SensitivitySetting::UiNavStick,
    );
}

fn spawn_audio_tab_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    active: OptionsTab,
    z: f32,
    audio_volume: &AudioVolume,
) {
    let tab = OptionsTab::Audio;
    let content_x = options_single_column_x(resolution);
    let mut focus = 300u32;

    let mut y = 90. + OPTIONS_BODY_Y_OFFSET;
    spawn_options_section_title(
        commands,
        asset_server,
        "Volume",
        Vec3::new(content_x, y, z),
        tab,
        active,
    );
    y -= 26.;

    spawn_volume_row(
        commands,
        graphics,
        asset_server,
        "Global:",
        VolumeChannel::Global,
        audio_volume.global,
        Vec3::new(content_x, y, z),
        tab,
        active,
        focus,
    );
    focus += 1;
    y -= 20.;

    spawn_volume_row(
        commands,
        graphics,
        asset_server,
        "Music:",
        VolumeChannel::Music,
        audio_volume.music,
        Vec3::new(content_x, y, z),
        tab,
        active,
        focus,
    );
    focus += 1;
    y -= 20.;

    spawn_volume_row(
        commands,
        graphics,
        asset_server,
        "SFX:",
        VolumeChannel::Sfx,
        audio_volume.sfx,
        Vec3::new(content_x, y, z),
        tab,
        active,
        focus,
    );
}

fn spawn_video_tab_content(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    active: OptionsTab,
    z: f32,
    display_scale: &DisplayScaleSettings,
    cheat_settings: &CheatSettings,
) {
    let tab = OptionsTab::Video;
    let content_x = options_single_column_x(resolution);
    let checkbox_x_offset = 108.;
    let mut focus = 400u32;

    let mut y = 90. + OPTIONS_BODY_Y_OFFSET;
    spawn_options_section_title(
        commands,
        asset_server,
        "Scale",
        Vec3::new(content_x, y, z),
        tab,
        active,
    );
    y -= 26.;

    spawn_scale_row(
        commands,
        graphics,
        asset_server,
        "Game:",
        ScaleChannel::Game,
        &display_scale.format_game_zoom_display(),
        Vec3::new(content_x, y, z),
        tab,
        active,
        focus,
    );
    focus += 1;
    y -= 20.;

    spawn_scale_row(
        commands,
        graphics,
        asset_server,
        "UI:",
        ScaleChannel::Ui,
        &display_scale.format_ui_zoom_display(),
        Vec3::new(content_x, y, z),
        tab,
        active,
        focus,
    );
    focus += 1;
    y -= 28.;

    spawn_options_section_title(
        commands,
        asset_server,
        "Animations",
        Vec3::new(content_x, y, z),
        tab,
        active,
    );
    y += OPTIONS_CONTENT_ROW_SPACING * 1.5;

    for (label, option_type, checked) in [
        (
            "Hide Attack Anims:",
            OptionsCheckboxType::HideAttackAnims,
            cheat_settings.hide_attack_anims,
        ),
        (
            "Hide Skill Anims:",
            OptionsCheckboxType::HideSkillAnims,
            cheat_settings.hide_skill_anims,
        ),
        (
            "Hide Heirloom Anims:",
            OptionsCheckboxType::HideHeirloomAnims,
            cheat_settings.hide_heirloom_anims,
        ),
    ] {
        spawn_options_checkbox(
            commands,
            graphics,
            asset_server,
            label,
            Vec3::new(content_x, y, z),
            Vec3::new(content_x + checkbox_x_offset, y, z),
            option_type,
            checked,
            tab,
            active,
            focus,
        );
        focus += 1;
        y += OPTIONS_CONTENT_ROW_SPACING;
    }
}

pub fn sync_options_tab_visibility(
    active_tab: Res<ActiveOptionsTab>,
    mut tab_content: Query<(Entity, &OptionsTabContent, &mut Visibility)>,
    mut hidden_interactables: Query<&mut Interactable, With<OptionsTabContent>>,
) {
    if !active_tab.is_changed() {
        return;
    }
    let active = active_tab.0;
    for (entity, content, mut visibility) in tab_content.iter_mut() {
        let now_visible = content.0 == active;
        *visibility = if now_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if !now_visible {
            if let Ok(mut interactable) = hidden_interactables.get_mut(entity) {
                interactable.change(Interaction::None);
            }
        }
    }
}

pub fn handle_options_tab_buttons(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mouseless: Res<crate::inputs::MouselessModeState>,
    mut active_tab: ResMut<ActiveOptionsTab>,
    mut ui_focus: ResMut<UiFocus>,
    mut button_queries: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<OptionsTabButton>>,
        Query<(Entity, &mut Interactable, &OptionsTabButton, &mut Sprite), With<OptionsTabButton>>,
    )>,
) {
    if button_queries.p1().is_empty() {
        return;
    }
    let hit_entity = {
        let q = button_queries.p0();
        options_tab_hit_under_cursor(&cursor_pos, &q)
    };
    let just_clicked = mouse_input.just_released(MouseButton::Left);
    let confirm_pressed = ui_focus.confirm_just_pressed;
    // Keyboard/gamepad focus only counts as a highlight source while it's actually driving the
    // UI (mouseless mode, or a gamepad genuinely in use) — otherwise the tab that merely holds
    // default focus would show its own green highlight alongside both the real mouse hover *and*
    // the currently-selected tab, giving up to 3 simultaneous highlights instead of at most 2
    // (one hover-or-focus highlight, plus the selected tab if it isn't already the same one).
    let focus_driving = mouseless.0 || cursor_pos.suppress_ui_hover;
    let focused_entity = focus_driving.then_some(ui_focus.focused).flatten();
    let mut requested: Option<(OptionsTab, Entity)> = None;

    for (entity, mut interactable, tab_button, mut sprite) in button_queries.p1().iter_mut() {
        let is_active = tab_button.0 == active_tab.0;
        let is_hovered = hit_entity == Some(entity);
        let is_focused = focused_entity == Some(entity);
        // One highlight source: pointer or keyboard focus. Selected tab is separate and
        // only shown when this button is not the current highlight target.
        let is_highlighted = is_hovered || is_focused;

        if is_highlighted {
            sprite.color = DARK_GREEN;
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
            }
            if (is_hovered && just_clicked) || (is_focused && confirm_pressed) {
                requested = Some((tab_button.0, entity));
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
            }
        } else if is_active {
            sprite.color = DARK_GREEN;
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        } else {
            sprite.color = OPTIONS_TAB_GREY;
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
    }

    let Some((tab, entity)) = requested else {
        return;
    };
    if active_tab.0 == tab {
        ui_focus.focused = Some(entity);
        return;
    }
    active_tab.0 = tab;
    ui_focus.focused = Some(entity);
}

fn options_tab_hit_under_cursor(
    cursor_pos: &Res<CursorPos>,
    query: &Query<(Entity, &Sprite, &GlobalTransform), With<OptionsTabButton>>,
) -> Option<Entity> {
    if !cursor_pos.ui_hover_hit_allowed() {
        return None;
    }

    let mut ret = None;
    for (ent, sprite, xform) in query.iter() {
        let Some(size) = sprite.custom_size else {
            continue;
        };
        let initial_x = xform.translation().x - (0.5 * size.x);
        let initial_y = xform.translation().y - (0.5 * size.y);
        let terminal_x = initial_x + size.x;
        let terminal_y = initial_y + size.y;
        if (initial_x..=terminal_x).contains(&cursor_pos.ui_coords.x)
            && (initial_y..=terminal_y).contains(&cursor_pos.ui_coords.y)
        {
            ret = Some(ent);
        }
    }
    ret
}

fn spawn_keybind_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    bind_type: KeyBindType,
    label_pos: Vec3,
    button_pos: Vec3,
    keybinds: &InputMappings,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
) {
    let (label, current_key) = match bind_type {
        KeyBindType::ActiveSkill(slot) => {
            let label = match slot {
                0 => "Skill Slot 1",
                1 => "Skill Slot 2:",
                2 => "Skill Slot 3:",
                3 => "Skill Slot 4:",
                _ => "Unknown Slot",
            };
            (label, keybinds.get_active_skill_key(slot))
        }
        KeyBindType::Hotbar(slot) => {
            let label = match slot {
                0 => "Hotbar 1:",
                1 => "Hotbar 2:",
                2 => "Hotbar 3:",
                3 => "Hotbar 4:",
                _ => "Unknown Hotbar",
            };
            (label, keybinds.get_hotbar_key(slot))
        }
        KeyBindType::Inventory => ("Inventory:", keybinds.get_inventory_key()),
        KeyBindType::Minimap => ("Map:", keybinds.get_minimap_key()),
        KeyBindType::Interact => ("Interact:", keybinds.get_interact_key()),
        KeyBindType::AttackAutoTarget => {
            ("Attack Auto Aim:", keybinds.get_attack_auto_target_key())
        }
    };

    let button_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(40., 12.)),
                ..default()
            },
            Transform::from_translation(button_pos),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::BackButton)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(KeyBindButton { bind_type })
        .insert(OptionsFocusRow(OptionsRowKind::Keybind(bind_type)))
        .insert(Interactable::default())
        .insert(options_focus(focus_index))
        .insert(Name::new(format!("Keybind Button {:?}", bind_type)))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "Rebind ", WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(2., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            Name::new(format!("Keybind Button Label {:?}", bind_type)),
        ))
        .insert(ChildOf(button_entity));

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: button_entity },
        Name::new(format!("Keybind Row Label {:?}", bind_type)),
    ));

    let current_key_pos = Vec3::new(label_pos.x + 72., label_pos.y, label_pos.z);
    commands.spawn((
        (
            gf::BODY
                .text(
                    &asset_server,
                    crate::keybinds::get_key_display_name(current_key),
                    WHITE,
                )
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: current_key_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        KeyBindText { bind_type },
        Name::new(format!("Keybind Current Key {:?}", bind_type)),
    ));
}

fn spawn_options_checkbox(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    label_pos: Vec3,
    checkbox_pos: Vec3,
    option_type: OptionsCheckboxType,
    is_checked: bool,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
) {
    let ui_checkbox = if is_checked {
        UIElement::CheckBoxSelected
    } else {
        UIElement::CheckBox
    };
    let checkbox_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(ui_checkbox).clone(),
                custom_size: Some(Vec2::new(16., 16.)),
                ..default()
            },
            Transform::from_translation(checkbox_pos),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(OptionsCheckbox(option_type))
        .insert(OptionsFocusRow(OptionsRowKind::Checkbox(option_type)))
        .insert(Interactable::default())
        .insert(options_focus(focus_index))
        .insert(Name::new("Options Checkbox"))
        .id();

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel {
            row: checkbox_entity,
        },
        Name::new("Options Checkbox Label"),
    ));
}

pub fn handle_cheat_checkbox_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut checkboxes: Query<(Entity, &OptionsCheckbox, &mut Interactable), With<OptionsCheckbox>>,
    mut cheat_settings: ResMut<CheatSettings>,
    mut auto_attack: ResMut<AutoAttackState>,
    mut mouseless_mode: ResMut<MouselessModeState>,
    mut swap_movement_aim_keys: ResMut<SwapMovementAimKeysState>,
    mut cursor_color: ResMut<CursorColorSettings>,
    mut commands: Commands,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, options_checkbox, mut interactable) in checkboxes.iter_mut() {
        let is_hit = matches!(hit_test, Some((hit, _, _)) if hit == entity);
        let is_focused = focus_input.is_focused(entity);

        if is_hit || is_focused {
            if matches!(interactable.current(), Interaction::None) {
                interactable.change(Interaction::Hovering);
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            continue;
        }

        let should_toggle =
            (is_hit && left_mouse_released) || (is_focused && focus_input.confirm_just_pressed());
        if !should_toggle {
            continue;
        }

        let setting = if options_checkbox.0.uses_cheat_settings() {
            cheat_settings.toggle_checkbox(options_checkbox.0)
        } else {
            match options_checkbox.0 {
                OptionsCheckboxType::DoubleCursorSize => {
                    cursor_color.double_size = !cursor_color.double_size;
                    cursor_color.save();
                    cursor_color.double_size
                }
                OptionsCheckboxType::AutoAttack => {
                    auto_attack.0 = !auto_attack.0;
                    auto_attack.save();
                    auto_attack.0
                }
                OptionsCheckboxType::MouselessMode => {
                    mouseless_mode.0 = !mouseless_mode.0;
                    mouseless_mode.save();
                    mouseless_mode.0
                }
                OptionsCheckboxType::SwapMovementAimKeys => {
                    swap_movement_aim_keys.0 = !swap_movement_aim_keys.0;
                    swap_movement_aim_keys.save();
                    swap_movement_aim_keys.0
                }
                _ => unreachable!(
                    "unexpected CheatSettings checkbox: {:?}",
                    options_checkbox.0
                ),
            }
        };
        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
        info!("Options: {:?} = {}", options_checkbox.0, setting);
    }
}

pub fn update_cheat_checkbox_visual(
    cheat_settings: Res<CheatSettings>,
    auto_attack: Res<AutoAttackState>,
    mouseless_mode: Res<MouselessModeState>,
    swap_movement_aim_keys: Res<SwapMovementAimKeysState>,
    cursor_color: Res<CursorColorSettings>,
    mut checkboxes: Query<(&OptionsCheckbox, &mut Sprite)>,
    graphics: Res<Graphics>,
) {
    if !cheat_settings.is_changed()
        && !auto_attack.is_changed()
        && !mouseless_mode.is_changed()
        && !swap_movement_aim_keys.is_changed()
        && !cursor_color.is_changed()
    {
        return;
    }

    for (options_checkbox, mut sprite) in checkboxes.iter_mut() {
        let checked = if options_checkbox.0.uses_cheat_settings() {
            cheat_settings.checkbox_value(options_checkbox.0)
        } else {
            match options_checkbox.0 {
                OptionsCheckboxType::DoubleCursorSize => cursor_color.double_size,
                OptionsCheckboxType::AutoAttack => auto_attack.0,
                OptionsCheckboxType::MouselessMode => mouseless_mode.0,
                OptionsCheckboxType::SwapMovementAimKeys => swap_movement_aim_keys.0,
                _ => false,
            }
        };
        sprite.image = graphics.get_ui_element_texture(options_checkbox_ui(checked));
    }
}

fn spawn_volume_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    channel: VolumeChannel,
    current_value: u8,
    label_pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
) {
    let row_entity = spawn_stepper_row_focus(
        commands,
        label_pos,
        tab,
        active,
        focus_index,
        OptionsRowKind::Volume(channel),
        &format!("Volume Row Focus {:?}", channel),
    );

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new(format!("Volume Label {:?}", channel)),
    ));

    let controls_x = label_pos.x + 50.;

    let minus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(controls_x, label_pos.y - 3.5, label_pos.z)),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(VolumeButton {
            channel,
            direction: VolumeDirection::Down,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new(format!("Volume Down {:?}", channel)))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "<", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(minus_entity));

    // Value text
    commands.spawn((
        (
            gf::BODY
                .text(
                    &asset_server,
                    format!("{}", current_value),
                    crate::colors::WHITE,
                )
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(controls_x + 18., label_pos.y - 3., label_pos.z),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        VolumeValueText { channel },
        Name::new(format!("Volume Value {:?}", channel)),
    ));

    // "+" button
    let plus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(VolumeButton {
            channel,
            direction: VolumeDirection::Up,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new(format!("Volume Up {:?}", channel)))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, ">", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(plus_entity));
}

fn spawn_damage_text_size_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    current: gf::DamageTextSize,
    label_pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
) {
    let row_entity = spawn_stepper_row_focus(
        commands,
        label_pos,
        tab,
        active,
        focus_index,
        OptionsRowKind::DamageTextSize,
        "Damage Text2d Size Row Focus",
    );

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new("Damage Text2d Size Label"),
    ));

    let controls_x = label_pos.x + 90.;

    let minus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(controls_x, label_pos.y - 3.5, label_pos.z)),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(DamageTextSizeButton {
            direction: VolumeDirection::Down,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Damage Text2d Size Down"))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "<", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(minus_entity));

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, current.label(), crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(controls_x + 18., label_pos.y - 3., label_pos.z),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        DamageTextSizeValueText,
        Name::new("Damage Text2d Size Value"),
    ));

    let plus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(DamageTextSizeButton {
            direction: VolumeDirection::Up,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Damage Text2d Size Up"))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, ">", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(plus_entity));
}

/// "Aim Sensitivity" stepper row (same `-`/value/`+` layout as `spawn_volume_row`), for the
/// single `AimSensitivity` value (1-10) rather than a per-channel value.
fn spawn_sensitivity_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    current_value: u8,
    label_pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
    row_kind: OptionsRowKind,
    setting: SensitivitySetting,
) {
    let row_entity = spawn_stepper_row_focus(
        commands,
        label_pos,
        tab,
        active,
        focus_index,
        row_kind,
        "Sensitivity Row Focus",
    );

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new("Sensitivity Label"),
    ));

    let controls_x = label_pos.x + 70.;

    let minus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(controls_x, label_pos.y - 3.5, label_pos.z)),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(SensitivityButton {
            direction: VolumeDirection::Down,
            setting,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Sensitivity Down"))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "<", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(minus_entity));

    commands.spawn((
        (
            gf::BODY
                .text(
                    &asset_server,
                    format!("{}", current_value),
                    crate::colors::WHITE,
                )
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(controls_x + 18., label_pos.y - 3., label_pos.z),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        SensitivityValueText(setting),
        Name::new("Sensitivity Value"),
    ));

    let plus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(SensitivityButton {
            direction: VolumeDirection::Up,
            setting,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Sensitivity Up"))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, ">", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(plus_entity));
}

fn spawn_scale_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    channel: ScaleChannel,
    current_value: &str,
    label_pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
) {
    let row_entity = spawn_stepper_row_focus(
        commands,
        label_pos,
        tab,
        active,
        focus_index,
        OptionsRowKind::Scale(channel),
        &format!("Scale Row Focus {:?}", channel),
    );

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new(format!("Scale Label {:?}", channel)),
    ));

    let controls_x = label_pos.x + 50.;

    let minus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(controls_x, label_pos.y - 3.5, label_pos.z)),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(ScaleButton {
            channel,
            direction: VolumeDirection::Down,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new(format!("Scale Down {:?}", channel)))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "<", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(minus_entity));

    commands.spawn((
        (
            gf::BODY
                .text(
                    &asset_server,
                    current_value.to_string(),
                    crate::colors::WHITE,
                )
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(controls_x + 18., label_pos.y - 3., label_pos.z),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        ScaleValueText { channel },
        Name::new(format!("Scale Value {:?}", channel)),
    ));

    let plus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(ScaleButton {
            channel,
            direction: VolumeDirection::Up,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new(format!("Scale Up {:?}", channel)))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, ">", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(plus_entity));
}

/// Cursor color row: `[label]  [<]  [sprite preview]  [>]`, mirroring `spawn_scale_row`
/// but showing the actual cursor sprite instead of a text value.
fn spawn_cursor_color_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    current_index: u8,
    label_pos: Vec3,
    tab: OptionsTab,
    active: OptionsTab,
    focus_index: u32,
) {
    let row_entity = spawn_stepper_row_focus(
        commands,
        label_pos,
        tab,
        active,
        focus_index,
        OptionsRowKind::CursorColor,
        "Cursor Color Row Focus",
    );

    commands.spawn((
        (
            gf::BODY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Left)
                .anchor(bevy::sprite::Anchor::CENTER_LEFT)
                .with_transform(Transform {
                    translation: label_pos,
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            tab_visibility(tab, active),
        ),
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new("Cursor Color Label"),
    ));

    let controls_x = label_pos.x + 50.;

    let minus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(controls_x, label_pos.y - 3.5, label_pos.z)),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(CursorColorButton {
            direction: VolumeDirection::Down,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Cursor Color Down"))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, "<", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(minus_entity));

    // Sprite preview of the currently selected cursor color.
    let mut preview_sprite = graphics
        .get_cursor_color_sprite(current_index)
        .unwrap_or_default();
    preview_sprite.custom_size = Some(Vec2::new(16., 16.));
    if graphics.texture_atlas_layout.is_some() && graphics.texture_atlas_image.is_some() {
        commands.spawn((
            preview_sprite.clone(),
            Transform::from_translation(Vec3::new(controls_x + 14., label_pos.y, label_pos.z)),
            tab_visibility(tab, active),
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            OptionsTabContent(tab),
            CursorColorPreview,
            Name::new("Cursor Color Preview"),
        ));
    }

    let plus_entity = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
                custom_size: Some(Vec2::new(14., 12.)),
                ..default()
            },
            Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            tab_visibility(tab, active),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(CursorColorButton {
            direction: VolumeDirection::Up,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Cursor Color Up"))
        .id();

    commands
        .spawn((
            gf::BODY
                .text(&asset_server, ">", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(bevy::sprite::Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .insert(ChildOf(plus_entity));
}

/// Arrow-click handler for the cursor color row. Mirrors `handle_scale_button_click`.
pub fn handle_cursor_color_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &CursorColorButton)>,
    mut cursor_color: ResMut<CursorColorSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, color_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::XLKeyHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::XLKeyHover),
                    );
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        cursor_color.nudge(color_button.direction == VolumeDirection::Up);
                        cursor_color.save();
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::XLKey);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::XLKey),
            );
        }
    }
}

/// Keeps the cursor color preview sprite in sync with the selected color.
pub fn update_cursor_color_preview(
    cursor_color: Res<CursorColorSettings>,
    graphics: Res<Graphics>,
    mut previews: Query<&mut Sprite, With<CursorColorPreview>>,
) {
    if !cursor_color.is_changed() {
        return;
    }
    let Some(new_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        return;
    };
    for mut sprite in previews.iter_mut() {
        *sprite = new_sprite.clone();
    }
}

pub fn handle_volume_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &VolumeButton)>,
    mut audio_volume: ResMut<AudioVolume>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, vol_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::XLKeyHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::XLKeyHover),
                    );
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        let val = match vol_button.channel {
                            VolumeChannel::Global => &mut audio_volume.global,
                            VolumeChannel::Music => &mut audio_volume.music,
                            VolumeChannel::Sfx => &mut audio_volume.sfx,
                        };
                        match vol_button.direction {
                            VolumeDirection::Down => {
                                *val = val.saturating_sub(1);
                            }
                            VolumeDirection::Up => {
                                *val = (*val + 1).min(10);
                            }
                        }
                        audio_volume.save();
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::XLKey);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::XLKey),
            );
        }
    }
}

pub fn handle_scale_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &ScaleButton), Without<VolumeButton>>,
    mut display_scale: ResMut<DisplayScaleSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, scale_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::XLKeyHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::XLKeyHover),
                    );
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        let before = (
                            display_scale.clamped_game_steps(),
                            display_scale.clamped_ui_steps(),
                        );
                        let zoom_in = scale_button.direction == VolumeDirection::Up;
                        match scale_button.channel {
                            ScaleChannel::Game => display_scale.nudge_game(zoom_in),
                            ScaleChannel::Ui => display_scale.nudge_ui(zoom_in),
                        }
                        let after = (
                            display_scale.clamped_game_steps(),
                            display_scale.clamped_ui_steps(),
                        );
                        if before != after {
                            display_scale.save();
                            commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                        }
                    }
                }
                _ => {}
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::XLKey);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::XLKey),
            );
        }
    }
}

fn options_row_entity(
    entity: Entity,
    focus_rows: &Query<Entity, With<OptionsFocusRow>>,
    row_members: &Query<&OptionsRowMember>,
) -> Option<Entity> {
    if focus_rows.get(entity).is_ok() {
        Some(entity)
    } else {
        row_members.get(entity).ok().map(|member| member.row)
    }
}

/// Hover hit for a content row label. Matches [`spawn_stepper_row_focus`] bounds so checkbox /
/// keybind rows highlight when the pointer is over the text, not only the tiny control sprite.
fn options_row_under_label_cursor(
    cursor_pos: &CursorPos,
    labels: &Query<(Entity, &OptionsRowLabel, &GlobalTransform)>,
    computed_visibility: &Query<&ViewVisibility>,
) -> Option<Entity> {
    if !cursor_pos.ui_hover_hit_allowed() {
        return None;
    }

    const ROW_SIZE: Vec2 = Vec2::new(150., 16.);
    const ROW_CENTER_OFFSET_X: f32 = 65.;

    let mut hit = None;
    for (label_entity, label, gt) in labels.iter() {
        if computed_visibility
            .get(label_entity)
            .ok()
            .is_some_and(|visibility| !visibility.get())
        {
            continue;
        }

        let pos = gt.translation();
        let center_x = pos.x + ROW_CENTER_OFFSET_X;
        let center_y = pos.y;
        let left = center_x - ROW_SIZE.x * 0.5;
        let right = center_x + ROW_SIZE.x * 0.5;
        let bottom = center_y - ROW_SIZE.y * 0.5;
        let top = center_y + ROW_SIZE.y * 0.5;
        if (left..=right).contains(&cursor_pos.ui_coords.x)
            && (bottom..=top).contains(&cursor_pos.ui_coords.y)
        {
            hit = Some(label.row);
        }
    }
    hit
}

fn highlighted_options_rows(
    ui_focus: &UiFocus,
    cursor_pos: &Res<CursorPos>,
    computed_visibility: &Query<&ViewVisibility>,
    ui_sprites: &Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    focus_rows: &Query<Entity, With<OptionsFocusRow>>,
    row_members: &Query<&OptionsRowMember>,
    visibility: &Query<&Visibility>,
    row_labels: &Query<(Entity, &OptionsRowLabel, &GlobalTransform)>,
) -> HashSet<Entity> {
    let mut highlighted: HashSet<Entity> = HashSet::new();

    if let Some(focused) = ui_focus.focused {
        if focus_entity_visible(focused, visibility) {
            if let Some(row) = options_row_entity(focused, focus_rows, row_members) {
                highlighted.insert(row);
            }
        }
    }

    if let Some((hit_entity, _, _)) = options_pointcast(cursor_pos, ui_sprites, computed_visibility)
    {
        if let Some(row) = options_row_entity(hit_entity, focus_rows, row_members) {
            highlighted.insert(row);
        }
    }

    if let Some(row) = options_row_under_label_cursor(cursor_pos, row_labels, computed_visibility) {
        highlighted.insert(row);
    }

    highlighted
}

/// Highlights the label of whichever options row is focused or mouse-hovered.
pub fn update_options_row_label_colors(
    ui_focus: Res<UiFocus>,
    cursor_pos: Res<CursorPos>,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut row_labels: Query<(&OptionsRowLabel, &mut TextColor), With<OptionsRowLabel>>,
    label_transforms: Query<(Entity, &OptionsRowLabel, &GlobalTransform)>,
    focus_rows: Query<Entity, With<OptionsFocusRow>>,
    row_members: Query<&OptionsRowMember>,
    visibility: Query<&Visibility>,
) {
    let highlighted = highlighted_options_rows(
        &ui_focus,
        &cursor_pos,
        &computed_visibility,
        &ui_sprites,
        &focus_rows,
        &row_members,
        &visibility,
        &label_transforms,
    );

    for (label, mut text_color) in row_labels.iter_mut() {
        text_color.0 = if highlighted.contains(&label.row) {
            YELLOW_2
        } else {
            WHITE
        };
    }
}

/// Spawns/moves `OptionsCursor` to the left of focused or hovered option row labels.
pub fn sync_options_row_cursor(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    ui_focus: Res<UiFocus>,
    cursor_pos: Res<CursorPos>,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    focus_rows: Query<Entity, With<OptionsFocusRow>>,
    row_members: Query<&OptionsRowMember>,
    visibility: Query<&Visibility>,
    row_labels: Query<(Entity, &OptionsRowLabel, &GlobalTransform)>,
    mut existing: Query<(Entity, &OptionsRowCursor, &mut Transform)>,
) {
    let highlighted = highlighted_options_rows(
        &ui_focus,
        &cursor_pos,
        &computed_visibility,
        &ui_sprites,
        &focus_rows,
        &row_members,
        &visibility,
        &row_labels,
    );

    let mut desired_labels: HashSet<Entity> = HashSet::new();
    let mut label_pos: Vec<(Entity, Vec3)> = Vec::new();
    for (label_entity, label, gt) in row_labels.iter() {
        if !highlighted.contains(&label.row) {
            continue;
        }
        desired_labels.insert(label_entity);
        label_pos.push((label_entity, gt.translation()));
    }

    let mut kept: HashSet<Entity> = HashSet::new();
    for (cursor_entity, cursor, mut transform) in existing.iter_mut() {
        if !desired_labels.contains(&cursor.label) {
            commands.entity(cursor_entity).despawn();
            continue;
        }
        kept.insert(cursor.label);
        if let Some((_, pos)) = label_pos.iter().find(|(e, _)| *e == cursor.label) {
            *transform = Transform::from_translation(Vec3::new(
                pos.x + OPTIONS_CURSOR_OFFSET_X,
                pos.y,
                pos.z + 1.,
            ));
        }
    }

    for (label_entity, pos) in label_pos {
        if kept.contains(&label_entity) {
            continue;
        }
        commands.spawn((
            aseprite_bundle(
                asset_server.load(OptionsCursor::PATH),
                OptionsCursor::tags::SELECT,
                Transform::from_translation(Vec3::new(
                    pos.x + OPTIONS_CURSOR_OFFSET_X,
                    pos.y,
                    pos.z + 1.,
                )),
                Visibility::Inherited,
                false,
            ),
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            UIState::Options,
            OptionsRowCursor {
                label: label_entity,
            },
            Name::new("Options Row Cursor"),
        ));
    }
}

fn nudge_options_stepper(
    row_kind: OptionsRowKind,
    up: bool,
    audio_volume: &mut AudioVolume,
    display_scale: &mut DisplayScaleSettings,
    cursor_color: &mut CursorColorSettings,
    aim_sensitivity: &mut AimSensitivity,
    ui_nav_stick_stability: &mut UiNavStickStability,
    cheat_settings: &mut CheatSettings,
) {
    match row_kind {
        OptionsRowKind::Volume(channel) => {
            let val = match channel {
                VolumeChannel::Global => &mut audio_volume.global,
                VolumeChannel::Music => &mut audio_volume.music,
                VolumeChannel::Sfx => &mut audio_volume.sfx,
            };
            if up {
                *val = (*val + 1).min(10);
            } else {
                *val = val.saturating_sub(1);
            }
            audio_volume.save();
        }
        OptionsRowKind::Scale(channel) => match channel {
            ScaleChannel::Game => display_scale.nudge_game(up),
            ScaleChannel::Ui => display_scale.nudge_ui(up),
        },
        OptionsRowKind::CursorColor => {
            cursor_color.nudge(up);
            cursor_color.save();
        }
        OptionsRowKind::Sensitivity => {
            if up {
                aim_sensitivity.0 = (aim_sensitivity.0 + 1).min(AimSensitivity::MAX);
            } else {
                aim_sensitivity.0 = aim_sensitivity.0.saturating_sub(1).max(AimSensitivity::MIN);
            }
            aim_sensitivity.save();
        }
        OptionsRowKind::NavStickStability => {
            if up {
                ui_nav_stick_stability.0 =
                    (ui_nav_stick_stability.0 + 1).min(UiNavStickStability::MAX);
            } else {
                ui_nav_stick_stability.0 = ui_nav_stick_stability
                    .0
                    .saturating_sub(1)
                    .max(UiNavStickStability::MIN);
            }
            ui_nav_stick_stability.save();
        }
        OptionsRowKind::DamageTextSize => {
            cheat_settings.damage_text_size = cheat_settings.damage_text_size.nudge(up);
            cheat_settings.persist_persisted_options();
        }
        OptionsRowKind::Checkbox(_) | OptionsRowKind::Keybind(_) => {}
    }
}

/// Left/right on a focused stepper row adjusts its value instead of moving focus.
pub fn handle_options_focus_row_input(
    ui_focus: Res<UiFocus>,
    key_input: Res<ButtonInput<KeyCode>>,
    ui_gamepad_q: Query<&ActionState<UiGamepadAction>, With<UiGamepadInputMarker>>,
    focus_rows: Query<&OptionsFocusRow>,
    mut focus_nav_blocked: ResMut<FocusNavBlocked>,
    mut audio_volume: ResMut<AudioVolume>,
    mut display_scale: ResMut<DisplayScaleSettings>,
    mut cursor_color: ResMut<CursorColorSettings>,
    mut aim_sensitivity: ResMut<AimSensitivity>,
    mut ui_nav_stick_stability: ResMut<UiNavStickStability>,
    mut cheat_settings: ResMut<CheatSettings>,
    mut commands: Commands,
    mut stick_latch: Local<UiStickNavLatch>,
) {
    let Some(focused) = ui_focus.focused else {
        return;
    };
    let Ok(row) = focus_rows.get(focused) else {
        *stick_latch = UiStickNavLatch::default();
        return;
    };

    let stepper_kind = match row.0 {
        OptionsRowKind::Volume(_)
        | OptionsRowKind::Scale(_)
        | OptionsRowKind::CursorColor
        | OptionsRowKind::Sensitivity
        | OptionsRowKind::NavStickStability
        | OptionsRowKind::DamageTextSize => row.0,
        OptionsRowKind::Checkbox(_) | OptionsRowKind::Keybind(_) => {
            *stick_latch = UiStickNavLatch::default();
            return;
        }
    };

    let Some(dir) = ui_nav_dir_just_pressed(
        &key_input,
        true,
        ui_gamepad_q.single().ok(),
        &mut stick_latch,
        *ui_nav_stick_stability,
    ) else {
        return;
    };

    let up = match dir {
        UiNavDir::Left => false,
        UiNavDir::Right => true,
        UiNavDir::Up | UiNavDir::Down => return,
    };

    nudge_options_stepper(
        stepper_kind,
        up,
        &mut audio_volume,
        &mut display_scale,
        &mut cursor_color,
        &mut aim_sensitivity,
        &mut ui_nav_stick_stability,
        &mut cheat_settings,
    );
    focus_nav_blocked.0 = true;
    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
}

/// Spawns the "Wipe Game Data" confirmation popup: a semi-transparent black full-screen
/// backdrop with a centered panel containing a warning message and Delete / Back buttons.
pub fn spawn_wipe_data_popup(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
) {
    use crate::ui::main_menu::MenuButton;

    // Backdrop: semi-transparent black full-screen overlay, above the options content.
    let backdrop_z = ui_helpers::Z_DEPTH_OPTIONS_CONTENT + 5.0;
    let backdrop = commands
        .spawn((
            (
                Sprite {
                    color: Color::srgba(0.0, 0.0, 0.0, 0.7),
                    custom_size: Some(Vec2::new(10000., 10000.)),
                    ..Default::default()
                },
                Transform::from_translation(Vec3::new(0., 0., backdrop_z)),
            ),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Backdrop"),
        ))
        .id();

    // Panel
    let panel = commands
        .spawn((
            (
                Sprite {
                    color: Color::srgba(0.05, 0.05, 0.05, 0.95),
                    custom_size: Some(Vec2::new(260., 110.)),
                    ..Default::default()
                },
                Transform::from_translation(Vec3::new(0., 0., 1.)),
            ),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Panel"),
        ))
        .id();
    commands.entity(panel).insert(ChildOf(backdrop));

    // Title
    commands
        .spawn((
            gf::MENU_TITLE_LARGE
                .text(&asset_server, "Wipe Game Data?", crate::colors::YELLOW_2)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 36., 1.),
                    scale: gf::MENU_TITLE_LARGE.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Title"),
        ))
        .insert(ChildOf(panel));

    // Warning text
    commands
        .spawn((
            gf::BODY.text(&asset_server, "This will reset all game progress\n\nto a fresh account.\n\nThis cannot be undone.", crate::colors::WHITE).justify(Justify::Center).anchor(Anchor::CENTER).with_transform(Transform {
                    translation: Vec3::new(0., 4., 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Warning"),
        ))
        .insert(ChildOf(panel));

    // Delete button (left)
    let delete_btn = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::BackButton),
                    custom_size: Some(Vec2::new(80., 18.)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-50., -30., 1.)),
            ),
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::BackButton,
            MenuButton::WipeDataConfirm,
            ModalFocusable { index: 1 },
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Delete Button"),
        ))
        .id();
    commands.entity(delete_btn).insert(ChildOf(panel));
    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, "Delete", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(delete_btn));

    // Back button (right)
    let back_btn = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::BackButton),
                    custom_size: Some(Vec2::new(80., 18.)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(50., -30., 1.)),
            ),
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::BackButton,
            MenuButton::WipeDataCancel,
            ModalFocusable { index: 0 },
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Back Button"),
        ))
        .id();
    commands.entity(back_btn).insert(ChildOf(panel));
    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, "Back", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(back_btn));
}

pub fn update_volume_text(
    audio_volume: Res<AudioVolume>,
    mut texts: Query<(&VolumeValueText, &mut Text2d)>,
) {
    if !audio_volume.is_changed() {
        return;
    }
    for (vol_text, mut text) in texts.iter_mut() {
        let val = match vol_text.channel {
            VolumeChannel::Global => audio_volume.global,
            VolumeChannel::Music => audio_volume.music,
            VolumeChannel::Sfx => audio_volume.sfx,
        };
        let new_value = format!("{}", val);
        if text.0 != new_value {
            text.0 = new_value;
        }
    }
}

pub fn update_scale_text(
    display_scale: Res<DisplayScaleSettings>,
    mut texts: Query<(&ScaleValueText, &mut Text2d)>,
) {
    if !display_scale.is_changed() {
        return;
    }
    for (scale_text, mut text) in texts.iter_mut() {
        let new_value = match scale_text.channel {
            ScaleChannel::Game => display_scale.format_game_zoom_display(),
            ScaleChannel::Ui => display_scale.format_ui_zoom_display(),
        };
        if text.0 != new_value {
            text.0 = new_value;
        }
    }
}

pub fn handle_sensitivity_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &SensitivityButton)>,
    mut aim_sensitivity: ResMut<AimSensitivity>,
    mut ui_nav_stick_stability: ResMut<UiNavStickStability>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, sens_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::XLKeyHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::XLKeyHover),
                    );
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        match sens_button.setting {
                            SensitivitySetting::Aim => match sens_button.direction {
                                VolumeDirection::Down => {
                                    aim_sensitivity.0 = aim_sensitivity
                                        .0
                                        .saturating_sub(1)
                                        .max(AimSensitivity::MIN);
                                }
                                VolumeDirection::Up => {
                                    aim_sensitivity.0 =
                                        (aim_sensitivity.0 + 1).min(AimSensitivity::MAX);
                                }
                            },
                            SensitivitySetting::UiNavStick => match sens_button.direction {
                                VolumeDirection::Down => {
                                    ui_nav_stick_stability.0 = ui_nav_stick_stability
                                        .0
                                        .saturating_sub(1)
                                        .max(UiNavStickStability::MIN);
                                }
                                VolumeDirection::Up => {
                                    ui_nav_stick_stability.0 = (ui_nav_stick_stability.0 + 1)
                                        .min(UiNavStickStability::MAX);
                                }
                            },
                        }
                        match sens_button.setting {
                            SensitivitySetting::Aim => aim_sensitivity.save(),
                            SensitivitySetting::UiNavStick => ui_nav_stick_stability.save(),
                        }
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::XLKey);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::XLKey),
            );
        }
    }
}

pub fn handle_damage_text_size_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    focus_input: FocusInput,
    computed_visibility: Query<&ViewVisibility>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &DamageTextSizeButton)>,
    mut cheat_settings: ResMut<CheatSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = options_pointcast(&cursor_pos, &ui_sprites, &computed_visibility);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, size_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands.entity(entity).insert(UIElement::XLKeyHover);
                    set_sprite_image(
                        &mut commands,
                        entity,
                        graphics.get_ui_element_texture(UIElement::XLKeyHover),
                    );
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        let up = size_button.direction == VolumeDirection::Up;
                        cheat_settings.damage_text_size = cheat_settings.damage_text_size.nudge(up);
                        cheat_settings.persist_persisted_options();
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            let Interaction::Hovering = interactable.current() else {
                continue;
            };
            interactable.change(Interaction::None);
            commands.entity(entity).insert(UIElement::XLKey);
            set_sprite_image(
                &mut commands,
                entity,
                graphics.get_ui_element_texture(UIElement::XLKey),
            );
        }
    }
}

pub fn update_damage_text_size_text(
    cheat_settings: Res<CheatSettings>,
    mut texts: Query<&mut Text2d, With<DamageTextSizeValueText>>,
) {
    if !cheat_settings.is_changed() {
        return;
    }
    let label = cheat_settings.damage_text_size.label();
    for mut text in texts.iter_mut() {
        if text.0 != label {
            text.0 = label.to_string();
        }
    }
}

pub fn update_sensitivity_text(
    aim_sensitivity: Res<AimSensitivity>,
    ui_nav_stick_stability: Res<UiNavStickStability>,
    mut texts: Query<(&mut Text2d, &SensitivityValueText)>,
) {
    for (mut text, value_for) in texts.iter_mut() {
        let new_value = match value_for.0 {
            SensitivitySetting::Aim => {
                if !aim_sensitivity.is_changed() {
                    continue;
                }
                format!("{}", aim_sensitivity.0)
            }
            SensitivitySetting::UiNavStick => {
                if !ui_nav_stick_stability.is_changed() {
                    continue;
                }
                format!("{}", ui_nav_stick_stability.0)
            }
        };
        if text.0 != new_value {
            text.0 = new_value;
        }
    }
}
