use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use bevy::input::gamepad::{GamepadButton, GamepadButtonType};
use leafwing_input_manager::prelude::ActionState;

use crate::gamepad_bindings::{
    format_binding_label, gamepad_connected, BindingLabel, GamepadBindingButton, GamepadMappings,
};
use crate::gamepad_input::{UiGamepadAction, UiGamepadInputMarker};
use crate::{
    aim::AimSensitivity,
    assets::Graphics,
    audio::{AudioSoundEffect, AudioVolume, SoundSpawner},
    client::GameData,
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
            FocusNavBottomRow, FocusNavHorizontalSkip, Focusable, UiFocus, UiNavDir,
        },
        interactions::Interaction,
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
    /// When true, damage/healing/regen floating text and item pickup labels use compact `4x5` at 5.0
    pub small_damage_text: bool,
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
}

impl Default for CheatSettings {
    fn default() -> Self {
        Self {
            bypass_class_unlocks: false,
            color_blind_mode: false,
            dev_mode: false,
            show_enemy_damage_numbers: true,
            small_damage_text: false,
            show_player_damage_numbers: true,
            show_tile_hover: false,
            bypass_time_crystal_pool: false,
            hide_attack_anims: false,
            hide_skill_anims: false,
            hide_heirloom_anims: false,
        }
    }
}

impl CheatSettings {
    /// Builds settings from defaults, then overrides `bypass_class_unlocks` from `game_data.json` when present.
    pub fn load_from_game_data() -> Self {
        let mut s = Self::default();
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            if let Ok(game_data) = GameData::try_from_json_reader(reader) {
                if let Some(v) = game_data.bypass_class_unlocks {
                    s.bypass_class_unlocks = v;
                }
            }
        }
        s
    }

    pub fn persist_bypass_class_unlocks(bypass_class_unlocks: bool) {
        let path = datafiles::game_data();
        let mut game_data = if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            GameData::default()
        };
        game_data.bypass_class_unlocks = Some(bypass_class_unlocks);
        if let Ok(file) = File::create(&path) {
            let writer = BufWriter::new(file);
            let _ = serde_json::to_writer_pretty(writer, &game_data);
        }
    }
}

/// Identifies which option an options-screen checkbox controls
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionsCheckboxType {
    UnlockAllClasses,
    ColorBlindMode,
    DevMode,
    ShowEnemyDamageNumbers,
    SmallDamageText,
    ShowPlayerDamageNumbers,
    ShowTileHover,
    BypassTimeCrystalPool,
    HideAttackAnims,
    HideSkillAnims,
    HideHeirloomAnims,
    DoubleCursorSize,
    AutoAttack,
    MouselessMode,
    SwapMovementAimKeys,
}

#[derive(Component)]
pub struct OptionsCheckbox(pub OptionsCheckboxType);

/// Color for boss damage warning indicators. When color blind mode is on, uses dark purple (visible on green/blue backgrounds).
pub fn boss_warning_indicator_color(settings: &CheatSettings) -> Color {
    if settings.color_blind_mode {
        Color::rgba(0.35, 0.0, 0.5, 0.3)
    } else {
        Color::rgba(1.0, 0.0, 0.0, 0.3)
    }
}

#[derive(Component)]
pub struct OptionsUI;

/// Incremented when the options menu must rebuild at a new UI scale while staying open.
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct OptionsUiLayoutRevision(pub u32);

/// Shifts tab body content up; title and bottom button row stay put.
const OPTIONS_BODY_Y_OFFSET: f32 = 20.;

const OPTIONS_TAB_GREY: Color = Color::rgba(0.25, 0.25, 0.25, 1.0);
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
}

#[derive(Component, Clone, Copy)]
pub struct OptionsFocusRow(pub OptionsRowKind);

/// Label text for a row; highlights when the row is focused or hovered.
#[derive(Component, Clone, Copy)]
pub struct OptionsRowLabel {
    pub row: Entity,
}

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
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.),
                    custom_size: Some(Vec2::new(150., 16.)),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(
                    row_center_x,
                    label_pos.y,
                    label_pos.z - 0.01,
                )),
                visibility: tab_visibility(tab, active),
                ..default()
            },
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

#[derive(Component)]
pub struct SensitivityButton {
    pub direction: VolumeDirection,
}

#[derive(Component)]
pub struct SensitivityValueText;

pub fn handle_options_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    focus_input: FocusInput,
    gamepads: Res<Gamepads>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &KeyBindButton), Without<WaitingForKeyInput>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
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
            commands
                .entity(entity)
                .insert(UIElement::XLKey)
                .insert(graphics.get_ui_element_texture(UIElement::XLKey));
        }
    }
}

pub fn handle_key_rebind_input(
    mut commands: Commands,
    mut key_input: ResMut<Input<KeyCode>>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    mut keybinds: ResMut<InputMappings>,
    mut gamepad_mappings: ResMut<GamepadMappings>,
    gamepad_buttons: Res<Input<GamepadButton>>,
    mut waiting: Query<(Entity, &mut WaitingForKeyInput)>,
    graphics: Res<Graphics>,
) {
    if waiting.is_empty() {
        return;
    }

    let just_pressed_key: Vec<KeyCode> = key_input.get_just_pressed().copied().collect();
    let just_pressed_mouse: Vec<MouseButton> = mouse_input.get_just_pressed().copied().collect();
    let just_pressed_gamepad: Vec<GamepadButton> =
        gamepad_buttons.get_just_pressed().copied().collect();

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
                commands
                    .entity(entity)
                    .insert(UIElement::BackButton)
                    .insert(graphics.get_ui_element_texture(UIElement::BackButton));
            }
            key_input.clear();
            return;
        }
    }

    if capture_gamepad {
        for button in just_pressed_gamepad {
            if button.button_type == GamepadButtonType::East {
                for (entity, _) in waiting.iter() {
                    commands.entity(entity).remove::<WaitingForKeyInput>();
                    commands
                        .entity(entity)
                        .insert(UIElement::BackButton)
                        .insert(graphics.get_ui_element_texture(UIElement::BackButton));
                }
                return;
            }

            let Some(binding) = GamepadBindingButton::from_button_type(button.button_type) else {
                continue;
            };

            for (entity, waiting_for) in waiting.iter() {
                apply_gamepad_rebind(&mut gamepad_mappings, waiting_for.bind_type, binding);
                gamepad_mappings.save();
                commands.entity(entity).remove::<WaitingForKeyInput>();
                commands
                    .entity(entity)
                    .insert(UIElement::BackButton)
                    .insert(graphics.get_ui_element_texture(UIElement::BackButton));
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
            commands
                .entity(entity)
                .insert(UIElement::BackButton)
                .insert(graphics.get_ui_element_texture(UIElement::BackButton));
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
            commands
                .entity(entity)
                .insert(UIElement::BackButton)
                .insert(graphics.get_ui_element_texture(UIElement::BackButton));
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
    gamepads: &Gamepads,
) -> String {
    format_binding_label(bind_type.into(), keybinds, gamepad_mappings, gamepads)
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
    gamepads: Res<Gamepads>,
    waiting: Query<&WaitingForKeyInput>,
    mut texts: Query<(&KeyBindText, &mut Text)>,
    mut was_waiting: Local<bool>,
    mut last_gamepad_connected: Local<Option<bool>>,
) {
    let waiting_binds: Vec<KeyBindType> = waiting.iter().map(|w| w.bind_type).collect();
    let is_waiting = !waiting_binds.is_empty();
    let use_gamepad = gamepad_connected(&gamepads);
    let device_changed = last_gamepad_connected.map_or(true, |prev| prev != use_gamepad);
    *last_gamepad_connected = Some(use_gamepad);

    if !keybinds.is_changed()
        && !gamepad_mappings.is_changed()
        && !is_waiting
        && !*was_waiting
        && !device_changed
    {
        return;
    }

    *was_waiting = is_waiting;

    for (key_text, mut text) in texts.iter_mut() {
        if waiting_binds.contains(&key_text.bind_type) {
            let waiting_gamepad = waiting
                .iter()
                .find(|w| w.bind_type == key_text.bind_type)
                .map(|w| w.capture_gamepad)
                .unwrap_or(use_gamepad);
            text.sections[0].value = if waiting_gamepad {
                "Press any button...".to_string()
            } else {
                "Press any key...".to_string()
            };
            text.sections[0].style.color = WHITE;
        } else {
            text.sections[0].value = options_display_binding(
                key_text.bind_type,
                &keybinds,
                &gamepad_mappings,
                &gamepads,
            );
            text.sections[0].style.color = WHITE;
        }
    }
}

pub fn update_options_controls_section_titles(
    gamepads: Res<Gamepads>,
    mut titles: Query<(&OptionsControlsSectionTitle, &mut Text)>,
    mut last_gamepad_connected: Local<Option<bool>>,
) {
    let use_gamepad = gamepad_connected(&gamepads);
    let device_changed = last_gamepad_connected.map_or(true, |prev| prev != use_gamepad);
    if !device_changed {
        return;
    }
    *last_gamepad_connected = Some(use_gamepad);

    let suffix = if use_gamepad { " (Controller)" } else { "" };
    for (title, mut text) in titles.iter_mut() {
        text.sections[0].value = format!("{}{}", title.0, suffix);
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
        commands.entity(entity).despawn_recursive();
    }
    for entity in popup.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

pub fn setup_options_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    keybinds: Res<InputMappings>,
    game_state: Res<State<crate::GameState>>,
    cheat_settings: Res<CheatSettings>,
    audio_volume: Res<AudioVolume>,
    display_scale: Res<DisplayScaleSettings>,
    cursor_color: Res<CursorColorSettings>,
    auto_attack: Res<AutoAttackState>,
    mouseless_mode: Res<MouselessModeState>,
    keyboard_aim_sensitivity: Res<AimSensitivity>,
    swap_movement_aim_keys: Res<SwapMovementAimKeysState>,
    active_tab: Res<ActiveOptionsTab>,
    existing_ui: Query<Entity, Or<(With<OptionsUI>, With<WipeDataPopup>)>>,
) {
    for entity in existing_ui.iter() {
        commands.entity(entity).despawn_recursive();
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
        Text2dBundle {
            text: Text::from_section(
                "Options",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                0.,
                resolution.game_height / 2. - 40.,
                z,
            )),
            ..Default::default()
        },
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

    if game_state.0 == crate::GameState::Main {
        let tutorial_btn = spawn_main_menu_wide_button(
            Vec3::new(-65., -156., z),
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
            Vec3::new(65., -156., z),
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
                SpriteBundle {
                    sprite: Sprite {
                        color: if is_active {
                            DARK_GREEN
                        } else {
                            OPTIONS_TAB_GREY
                        },
                        custom_size: Some(OPTIONS_TAB_BUTTON_SIZE),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(tab_x, y, z)),
                    ..default()
                },
                Interactable::default(),
                RenderLayers::from_layers(&[3]),
                OptionsUI,
                OptionsTabButton(*tab),
                options_focus(tab.focus_index()),
                Name::new(format!("Options Tab: {}", tab.label())),
            ))
            .id();

        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        tab.label(),
                        TextStyle {
                            font: asset_server.load("fonts/alagard.ttf"),
                            font_size: 15.0,
                            color: WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                OptionsUI,
            ))
            .set_parent(button_e);
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
        Text2dBundle {
            text: Text::from_section(
                title,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
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
        Text2dBundle {
            text: Text::from_section(
                title,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
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

    spawn_options_checkbox(
        commands,
        graphics,
        asset_server,
        "Small damage text:",
        Vec3::new(main_x, y, z),
        Vec3::new(main_x + checkbox_x_offset, y, z),
        OptionsCheckboxType::SmallDamageText,
        cheat_settings.small_damage_text,
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
            Vec3::new(skills_x + 148., skills_y - 3.5, z),
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
            Vec3::new(skills_x + 148., skills_y - 3.5, z),
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
            Vec3::new(other_x + 148., other_y - 3.5, z),
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
    mut tab_content: Query<(&OptionsTabContent, &mut Visibility)>,
) {
    if !active_tab.is_changed() {
        return;
    }
    let active = active_tab.0;
    for (content, mut visibility) in tab_content.iter_mut() {
        *visibility = tab_visibility(content.0, active);
    }
}

pub fn handle_options_tab_buttons(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
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
    let focused_entity = ui_focus.focused;
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
            ("Attack Auto Target:", keybinds.get_attack_auto_target_key())
        }
    };

    let button_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(40., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(button_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    "Rebind ",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(2., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            Name::new(format!("Keybind Button Label {:?}", bind_type)),
        ))
        .set_parent(button_entity);

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: button_entity },
        Name::new(format!("Keybind Row Label {:?}", bind_type)),
    ));

    let current_key_pos = Vec3::new(label_pos.x + 90., label_pos.y, label_pos.z);
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                crate::keybinds::get_key_display_name(current_key),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(current_key_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
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
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_checkbox).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(16., 16.)),
                ..Default::default()
            },
            transform: Transform::from_translation(checkbox_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
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
    mouse_input: Res<Input<MouseButton>>,
    focus_input: FocusInput,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut checkboxes: Query<(Entity, &OptionsCheckbox, &mut Interactable), With<OptionsCheckbox>>,
    mut cheat_settings: ResMut<CheatSettings>,
    mut auto_attack: ResMut<AutoAttackState>,
    mut mouseless_mode: ResMut<MouselessModeState>,
    mut swap_movement_aim_keys: ResMut<SwapMovementAimKeysState>,
    mut cursor_color: ResMut<CursorColorSettings>,
    mut commands: Commands,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
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

        let setting = match options_checkbox.0 {
            OptionsCheckboxType::UnlockAllClasses => {
                cheat_settings.bypass_class_unlocks = !cheat_settings.bypass_class_unlocks;
                CheatSettings::persist_bypass_class_unlocks(cheat_settings.bypass_class_unlocks);
                cheat_settings.bypass_class_unlocks
            }
            OptionsCheckboxType::ColorBlindMode => {
                cheat_settings.color_blind_mode = !cheat_settings.color_blind_mode;
                cheat_settings.color_blind_mode
            }
            OptionsCheckboxType::DevMode => {
                cheat_settings.dev_mode = !cheat_settings.dev_mode;
                cheat_settings.dev_mode
            }
            OptionsCheckboxType::ShowEnemyDamageNumbers => {
                cheat_settings.show_enemy_damage_numbers =
                    !cheat_settings.show_enemy_damage_numbers;
                cheat_settings.show_enemy_damage_numbers
            }
            OptionsCheckboxType::SmallDamageText => {
                cheat_settings.small_damage_text = !cheat_settings.small_damage_text;
                cheat_settings.small_damage_text
            }
            OptionsCheckboxType::ShowPlayerDamageNumbers => {
                cheat_settings.show_player_damage_numbers =
                    !cheat_settings.show_player_damage_numbers;
                cheat_settings.show_player_damage_numbers
            }
            OptionsCheckboxType::ShowTileHover => {
                cheat_settings.show_tile_hover = !cheat_settings.show_tile_hover;
                cheat_settings.show_tile_hover
            }
            OptionsCheckboxType::BypassTimeCrystalPool => {
                cheat_settings.bypass_time_crystal_pool = !cheat_settings.bypass_time_crystal_pool;
                cheat_settings.bypass_time_crystal_pool
            }
            OptionsCheckboxType::HideAttackAnims => {
                cheat_settings.hide_attack_anims = !cheat_settings.hide_attack_anims;
                cheat_settings.hide_attack_anims
            }
            OptionsCheckboxType::HideSkillAnims => {
                cheat_settings.hide_skill_anims = !cheat_settings.hide_skill_anims;
                cheat_settings.hide_skill_anims
            }
            OptionsCheckboxType::HideHeirloomAnims => {
                cheat_settings.hide_heirloom_anims = !cheat_settings.hide_heirloom_anims;
                cheat_settings.hide_heirloom_anims
            }
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
    mut checkboxes: Query<(&OptionsCheckbox, &mut Handle<Image>)>,
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

    for (options_checkbox, mut texture) in checkboxes.iter_mut() {
        let checkbox_ui = match options_checkbox.0 {
            OptionsCheckboxType::UnlockAllClasses => {
                if cheat_settings.bypass_class_unlocks {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::ColorBlindMode => {
                if cheat_settings.color_blind_mode {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::DevMode => {
                if cheat_settings.dev_mode {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::ShowEnemyDamageNumbers => {
                if cheat_settings.show_enemy_damage_numbers {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::SmallDamageText => {
                if cheat_settings.small_damage_text {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::ShowPlayerDamageNumbers => {
                if cheat_settings.show_player_damage_numbers {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::ShowTileHover => {
                if cheat_settings.show_tile_hover {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::BypassTimeCrystalPool => {
                if cheat_settings.bypass_time_crystal_pool {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::HideAttackAnims => {
                if cheat_settings.hide_attack_anims {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::HideSkillAnims => {
                if cheat_settings.hide_skill_anims {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::HideHeirloomAnims => {
                if cheat_settings.hide_heirloom_anims {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::DoubleCursorSize => {
                if cursor_color.double_size {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::AutoAttack => {
                if auto_attack.0 {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::MouselessMode => {
                if mouseless_mode.0 {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
            OptionsCheckboxType::SwapMovementAimKeys => {
                if swap_movement_aim_keys.0 {
                    UIElement::CheckBoxSelected
                } else {
                    UIElement::CheckBox
                }
            }
        };
        *texture = graphics.get_ui_element_texture(checkbox_ui).clone();
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
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new(format!("Volume Label {:?}", channel)),
    ));

    let controls_x = label_pos.x + 50.;

    let minus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    "<",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(minus_entity);

    // Value text
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("{}", current_value),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                controls_x + 18.,
                label_pos.y - 3.,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        VolumeValueText { channel },
        Name::new(format!("Volume Value {:?}", channel)),
    ));

    // "+" button
    let plus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    ">",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(plus_entity);
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
) {
    let row_entity = spawn_stepper_row_focus(
        commands,
        label_pos,
        tab,
        active,
        focus_index,
        OptionsRowKind::Sensitivity,
        "Sensitivity Row Focus",
    );

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new("Sensitivity Label"),
    ));

    let controls_x = label_pos.x + 70.;

    let minus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(SensitivityButton {
            direction: VolumeDirection::Down,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Sensitivity Down"))
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "<",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(minus_entity);

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("{}", current_value),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                controls_x + 18.,
                label_pos.y - 3.,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        SensitivityValueText,
        Name::new("Sensitivity Value"),
    ));

    let plus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(OptionsTabContent(tab))
        .insert(SensitivityButton {
            direction: VolumeDirection::Up,
        })
        .insert(OptionsRowMember { row: row_entity })
        .insert(Interactable::default())
        .insert(Name::new("Sensitivity Up"))
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    ">",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(plus_entity);
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
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new(format!("Scale Label {:?}", channel)),
    ));

    let controls_x = label_pos.x + 50.;

    let minus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    "<",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(minus_entity);

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                current_value.to_string(),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                controls_x + 18.,
                label_pos.y - 3.,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        ScaleValueText { channel },
        Name::new(format!("Scale Value {:?}", channel)),
    ));

    let plus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    ">",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(plus_entity);
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
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        OptionsTabContent(tab),
        OptionsRowLabel { row: row_entity },
        Name::new("Cursor Color Label"),
    ));

    let controls_x = label_pos.x + 50.;

    let minus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    "<",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(minus_entity);

    // Sprite preview of the currently selected cursor color.
    let mut preview_sprite = graphics
        .get_cursor_color_sprite(current_index)
        .unwrap_or_default();
    preview_sprite.custom_size = Some(Vec2::new(16., 16.));
    if let Some(atlas) = graphics.texture_atlas.as_ref() {
        commands.spawn((
            SpriteSheetBundle {
                texture_atlas: atlas.clone(),
                sprite: preview_sprite,
                transform: Transform::from_translation(Vec3::new(
                    controls_x + 14.,
                    label_pos.y,
                    label_pos.z,
                )),
                visibility: tab_visibility(tab, active),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            OptionsTabContent(tab),
            CursorColorPreview,
            Name::new("Cursor Color Preview"),
        ));
    }

    let plus_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(14., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(
                controls_x + 36.,
                label_pos.y - 3.5,
                label_pos.z,
            )),
            visibility: tab_visibility(tab, active),
            ..Default::default()
        })
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
            Text2dBundle {
                text: Text::from_section(
                    ">",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
        ))
        .set_parent(plus_entity);
}

/// Arrow-click handler for the cursor color row. Mirrors `handle_scale_button_click`.
pub fn handle_cursor_color_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    focus_input: FocusInput,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &CursorColorButton)>,
    mut cursor_color: ResMut<CursorColorSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, color_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
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
            commands
                .entity(entity)
                .insert(UIElement::XLKey)
                .insert(graphics.get_ui_element_texture(UIElement::XLKey));
        }
    }
}

/// Keeps the cursor color preview sprite in sync with the selected color.
pub fn update_cursor_color_preview(
    cursor_color: Res<CursorColorSettings>,
    graphics: Res<Graphics>,
    mut previews: Query<&mut TextureAtlasSprite, With<CursorColorPreview>>,
) {
    if !cursor_color.is_changed() {
        return;
    }
    let Some(new_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        return;
    };
    for mut sprite in previews.iter_mut() {
        sprite.index = new_sprite.index;
    }
}

pub fn handle_volume_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    focus_input: FocusInput,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &VolumeButton)>,
    mut audio_volume: ResMut<AudioVolume>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, vol_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
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
            commands
                .entity(entity)
                .insert(UIElement::XLKey)
                .insert(graphics.get_ui_element_texture(UIElement::XLKey));
        }
    }
}

pub fn handle_scale_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    focus_input: FocusInput,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &ScaleButton), Without<VolumeButton>>,
    mut display_scale: ResMut<DisplayScaleSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, scale_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
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
            commands
                .entity(entity)
                .insert(UIElement::XLKey)
                .insert(graphics.get_ui_element_texture(UIElement::XLKey));
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

/// Highlights the label of whichever options row is focused or mouse-hovered.
pub fn update_options_row_label_colors(
    ui_focus: Res<UiFocus>,
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut row_labels: Query<(&OptionsRowLabel, &mut Text), With<OptionsRowLabel>>,
    focus_rows: Query<Entity, With<OptionsFocusRow>>,
    row_members: Query<&OptionsRowMember>,
    visibility: Query<&Visibility>,
) {
    let mut highlighted: HashSet<Entity> = HashSet::new();

    if let Some(focused) = ui_focus.focused {
        if focus_entity_visible(focused, &visibility) {
            if let Some(row) = options_row_entity(focused, &focus_rows, &row_members) {
                highlighted.insert(row);
            }
        }
    }

    if let Some((hit_entity, _, _)) = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None) {
        if let Some(row) = options_row_entity(hit_entity, &focus_rows, &row_members) {
            highlighted.insert(row);
        }
    }

    for (label, mut text) in row_labels.iter_mut() {
        if text.sections.is_empty() {
            continue;
        }
        text.sections[0].style.color = if highlighted.contains(&label.row) {
            YELLOW_2
        } else {
            WHITE
        };
    }
}

fn nudge_options_stepper(
    row_kind: OptionsRowKind,
    up: bool,
    audio_volume: &mut AudioVolume,
    display_scale: &mut DisplayScaleSettings,
    cursor_color: &mut CursorColorSettings,
    sensitivity: &mut AimSensitivity,
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
                sensitivity.0 = (sensitivity.0 + 1).min(AimSensitivity::MAX);
            } else {
                sensitivity.0 = sensitivity.0.saturating_sub(1).max(AimSensitivity::MIN);
            }
            sensitivity.save();
        }
        OptionsRowKind::Checkbox(_) | OptionsRowKind::Keybind(_) => {}
    }
}

/// Left/right on a focused stepper row adjusts its value instead of moving focus.
pub fn handle_options_focus_row_input(
    ui_focus: Res<UiFocus>,
    key_input: Res<Input<KeyCode>>,
    ui_gamepad_q: Query<&ActionState<UiGamepadAction>, With<UiGamepadInputMarker>>,
    focus_rows: Query<&OptionsFocusRow>,
    mut focus_nav_blocked: ResMut<FocusNavBlocked>,
    mut audio_volume: ResMut<AudioVolume>,
    mut display_scale: ResMut<DisplayScaleSettings>,
    mut cursor_color: ResMut<CursorColorSettings>,
    mut sensitivity: ResMut<AimSensitivity>,
    mut commands: Commands,
    mut last_stick_dir: Local<Option<UiNavDir>>,
) {
    let Some(focused) = ui_focus.focused else {
        return;
    };
    let Ok(row) = focus_rows.get(focused) else {
        *last_stick_dir = None;
        return;
    };

    let stepper_kind = match row.0 {
        OptionsRowKind::Volume(_)
        | OptionsRowKind::Scale(_)
        | OptionsRowKind::CursorColor
        | OptionsRowKind::Sensitivity => row.0,
        OptionsRowKind::Checkbox(_) | OptionsRowKind::Keybind(_) => {
            *last_stick_dir = None;
            return;
        }
    };

    let Some(dir) = ui_nav_dir_just_pressed(
        &key_input,
        ui_gamepad_q.get_single().ok(),
        &mut last_stick_dir,
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
        &mut sensitivity,
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
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0.0, 0.0, 0.0, 0.7),
                    custom_size: Some(Vec2::new(10000., 10000.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., backdrop_z)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Backdrop"),
        ))
        .id();

    // Panel
    let panel = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0.05, 0.05, 0.05, 0.95),
                    custom_size: Some(Vec2::new(260., 110.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Panel"),
        ))
        .id();
    commands.entity(panel).set_parent(backdrop);

    // Title
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Wipe Game Data?",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: crate::colors::YELLOW_2,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 36., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Title"),
        ))
        .set_parent(panel);

    // Warning text
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "This will reset all game progress\n\nto a fresh account.\n\nThis cannot be undone.",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 4., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Popup Warning"),
        ))
        .set_parent(panel);

    // Delete button (left)
    let delete_btn = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(80., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(-50., -30., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::BackButton,
            MenuButton::WipeDataConfirm,
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Delete Button"),
        ))
        .id();
    commands.entity(delete_btn).set_parent(panel);
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Delete",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(delete_btn);

    // Back button (right)
    let back_btn = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::BackButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(80., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(50., -30., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::BackButton,
            MenuButton::WipeDataCancel,
            UIState::Options,
            WipeDataPopup,
            Name::new("Wipe Data Back Button"),
        ))
        .id();
    commands.entity(back_btn).set_parent(panel);
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Back",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(back_btn);
}

pub fn update_volume_text(
    audio_volume: Res<AudioVolume>,
    mut texts: Query<(&VolumeValueText, &mut Text)>,
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
        if text.sections[0].value != new_value {
            text.sections[0].value = new_value;
        }
    }
}

pub fn update_scale_text(
    display_scale: Res<DisplayScaleSettings>,
    mut texts: Query<(&ScaleValueText, &mut Text)>,
) {
    if !display_scale.is_changed() {
        return;
    }
    for (scale_text, mut text) in texts.iter_mut() {
        let new_value = match scale_text.channel {
            ScaleChannel::Game => display_scale.format_game_zoom_display(),
            ScaleChannel::Ui => display_scale.format_ui_zoom_display(),
        };
        if text.sections[0].value != new_value {
            text.sections[0].value = new_value;
        }
    }
}

pub fn handle_sensitivity_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    focus_input: FocusInput,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &SensitivityButton)>,
    mut sensitivity: ResMut<AimSensitivity>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, sens_button) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        match sens_button.direction {
                            VolumeDirection::Down => {
                                sensitivity.0 =
                                    sensitivity.0.saturating_sub(1).max(AimSensitivity::MIN);
                            }
                            VolumeDirection::Up => {
                                sensitivity.0 = (sensitivity.0 + 1).min(AimSensitivity::MAX);
                            }
                        }
                        sensitivity.save();
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
            commands
                .entity(entity)
                .insert(UIElement::XLKey)
                .insert(graphics.get_ui_element_texture(UIElement::XLKey));
        }
    }
}

pub fn update_sensitivity_text(
    sensitivity: Res<AimSensitivity>,
    mut texts: Query<&mut Text, With<SensitivityValueText>>,
) {
    if !sensitivity.is_changed() {
        return;
    }
    let new_value = format!("{}", sensitivity.0);
    for mut text in texts.iter_mut() {
        if text.sections[0].value != new_value {
            text.sections[0].value = new_value.clone();
        }
    }
}
