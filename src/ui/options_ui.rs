use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, AudioVolume, SoundSpawner},
    cursor::CursorPos,
    keybinds::InputMappings,
    ui::{
        interactions::Interaction, spawn_back_button, ui_helpers, Interactable, UIElement, UIState,
    },
    InputBinding, ScreenResolution,
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
    /// When true, damage numbers are shown when enemies take damage (player damage numbers always show)
    pub show_enemy_damage_numbers: bool,
    /// When true, the tile under the cursor is highlighted during gameplay
    pub show_tile_hover: bool,
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
            show_tile_hover: true,
            hide_attack_anims: false,
            hide_skill_anims: false,
            hide_heirloom_anims: false,
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
    ShowTileHover,
    HideAttackAnims,
    HideSkillAnims,
    HideHeirloomAnims,
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
    AutoAttackToggle,
}

#[derive(Component)]
pub struct KeyBindText {
    bind_type: KeyBindType,
}

#[derive(Component)]
pub struct WaitingForKeyInput {
    pub bind_type: KeyBindType,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VolumeChannel {
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

pub fn handle_options_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &KeyBindButton), Without<WaitingForKeyInput>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, button) in buttons.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        // Start waiting for key input
                        commands.entity(entity).insert(WaitingForKeyInput {
                            bind_type: button.bind_type,
                        });
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            },
            _ => {
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
}

pub fn handle_key_rebind_input(
    mut commands: Commands,
    mut key_input: ResMut<Input<KeyCode>>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    mut keybinds: ResMut<InputMappings>,
    waiting: Query<(Entity, &WaitingForKeyInput)>,
    graphics: Res<Graphics>,
) {
    if waiting.is_empty() {
        return;
    }

    // Collect keys to avoid borrow checker issues
    let just_pressed_key: Vec<KeyCode> = key_input.get_just_pressed().copied().collect();
    let just_pressed_mouse: Vec<MouseButton> = mouse_input.get_just_pressed().copied().collect();

    // Check for any key press
    for key in just_pressed_key {
        // Ignore Escape (used to cancel)
        if key == KeyCode::Escape {
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

        // Set the new key binding
        for (entity, waiting_for) in waiting.iter() {
            match waiting_for.bind_type {
                KeyBindType::ActiveSkill(slot) => {
                    keybinds.set_active_skill_key(slot, InputBinding::KeyBinding(key))
                }
                KeyBindType::Hotbar(slot) => {
                    keybinds.set_hotbar_key(slot, InputBinding::KeyBinding(key))
                }
                KeyBindType::Inventory => keybinds.set_inventory_key(InputBinding::KeyBinding(key)),
                KeyBindType::Minimap => keybinds.set_minimap_key(InputBinding::KeyBinding(key)),
                KeyBindType::AutoAttackToggle => {
                    keybinds.set_auto_attack_toggle_key(InputBinding::KeyBinding(key))
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

    // Check for any mouse press
    for mouse_button in just_pressed_mouse {
        for (entity, waiting_for) in waiting.iter() {
            match waiting_for.bind_type {
                KeyBindType::ActiveSkill(slot) => {
                    keybinds.set_active_skill_key(slot, InputBinding::MouseBinding(mouse_button))
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
                KeyBindType::AutoAttackToggle => {
                    keybinds.set_auto_attack_toggle_key(InputBinding::MouseBinding(mouse_button))
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

pub fn update_keybind_text(
    keybinds: Res<InputMappings>,
    waiting: Query<&WaitingForKeyInput>,
    mut texts: Query<(&KeyBindText, &mut Text)>,
    mut was_waiting: Local<bool>,
) {
    // Collect waiting bind types to avoid borrow issues
    let waiting_binds: Vec<KeyBindType> = waiting.iter().map(|w| w.bind_type).collect();
    let is_waiting = !waiting_binds.is_empty();

    // Update if:
    // - Keybinds changed, OR
    // - There's waiting input, OR
    // - We just stopped waiting (to show the new key)
    if !keybinds.is_changed() && !is_waiting && !*was_waiting {
        return;
    }

    *was_waiting = is_waiting;

    for (key_text, mut text) in texts.iter_mut() {
        if waiting_binds.contains(&key_text.bind_type) {
            text.sections[0].value = "Press any key...".to_string();
            text.sections[0].style.color = crate::colors::YELLOW_2;
        } else {
            let key = match key_text.bind_type {
                KeyBindType::ActiveSkill(slot) => keybinds.get_active_skill_key(slot),
                KeyBindType::Hotbar(slot) => keybinds.get_hotbar_key(slot),
                KeyBindType::Inventory => keybinds.get_inventory_key(),
                KeyBindType::Minimap => keybinds.get_minimap_key(),
                KeyBindType::AutoAttackToggle => keybinds.get_auto_attack_toggle_key(),
            };
            text.sections[0].value = crate::keybinds::get_key_display_name(key);
            text.sections[0].style.color = crate::colors::WHITE;
        }
    }
}

pub fn cleanup_options_ui(mut commands: Commands, query: Query<Entity, With<OptionsUI>>) {
    for entity in query.iter() {
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
) {
    let overlay = ui_helpers::spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width, resolution.game_height),
        1.,
        ui_helpers::Z_DEPTH_OPTIONS_OVERLAY,
    );
    commands
        .entity(overlay)
        .insert(OptionsUI)
        .insert(UIState::Options);

    // Title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Options",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: crate::colors::YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                0.,
                resolution.game_height / 2. - 40.,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Options Title"),
    ));
    let left_side_x = -resolution.game_width / 2. + 22.;
    let right_side_x = resolution.game_width / 2. - 142.;
    // Section title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Active Skill Keybinds",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                left_side_x,
                90.,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Keybind Section Title"),
    ));

    let start_y = 66.5;
    let row_spacing = -16.0;

    // Skill slot keybinds (slots 0-3)
    for slot in 0..4 {
        let y = start_y + row_spacing * slot as f32;
        spawn_keybind_row(
            &mut commands,
            &graphics,
            &asset_server,
            KeyBindType::ActiveSkill(slot),
            Vec3::new(left_side_x + 2., y, ui_helpers::Z_DEPTH_OPTIONS_CONTENT),
            Vec3::new(
                left_side_x + 160.,
                y - 3.5,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            ),
            &keybinds,
        );
    }

    // Hotbar keybinds section (Hotbar consume/use keys for slots 0-3)
    let hotbar_section_y = start_y + row_spacing * 4.5;
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Hotbar Keybinds",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                left_side_x,
                hotbar_section_y,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Hotbar Keybind Section Title"),
    ));

    let hotbar_start_y = hotbar_section_y + row_spacing * 1.;
    for slot in 0..4 {
        let y = hotbar_start_y + row_spacing * slot as f32;
        spawn_keybind_row(
            &mut commands,
            &graphics,
            &asset_server,
            KeyBindType::Hotbar(slot),
            Vec3::new(left_side_x + 2., y, ui_helpers::Z_DEPTH_OPTIONS_CONTENT),
            Vec3::new(
                left_side_x + 160.,
                y - 3.5,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            ),
            &keybinds,
        );
    }

    // UI keybinds section
    let ui_section_y = hotbar_start_y + row_spacing * 4.5;
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Other Keybinds",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                left_side_x,
                ui_section_y,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Other Keybind Section Title"),
    ));

    // Inventory keybind
    let inventory_y = ui_section_y + row_spacing * 1.5;
    spawn_keybind_row(
        &mut commands,
        &graphics,
        &asset_server,
        KeyBindType::Inventory,
        Vec3::new(
            left_side_x + 2.,
            inventory_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            left_side_x + 160.,
            inventory_y - 3.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        &keybinds,
    );

    // Minimap keybind
    let minimap_y = inventory_y + row_spacing;
    spawn_keybind_row(
        &mut commands,
        &graphics,
        &asset_server,
        KeyBindType::Minimap,
        Vec3::new(
            left_side_x + 2.,
            minimap_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            left_side_x + 160.,
            minimap_y - 3.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        &keybinds,
    );

    // Auto attack toggle keybind
    let auto_attack_y = minimap_y + row_spacing;
    spawn_keybind_row(
        &mut commands,
        &graphics,
        &asset_server,
        KeyBindType::AutoAttackToggle,
        Vec3::new(
            left_side_x + 2.,
            auto_attack_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            left_side_x + 160.,
            auto_attack_y - 3.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        &keybinds,
    );

    // Cheats section
    let cheats_section_y = 90.;
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Cheats",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                right_side_x,
                cheats_section_y,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Cheats Section Title"),
    ));

    // Unlock all classes checkbox
    let checkbox_y = 70.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Unlock All Classes:",
        Vec3::new(
            right_side_x,
            checkbox_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            checkbox_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::UnlockAllClasses,
        cheat_settings.bypass_class_unlocks,
    );

    // Color blind mode checkbox (boss damage indicators use dark purple instead of red)
    let color_blind_checkbox_y = checkbox_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Color Blind Mode:",
        Vec3::new(
            right_side_x,
            color_blind_checkbox_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            color_blind_checkbox_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::ColorBlindMode,
        cheat_settings.color_blind_mode,
    );

    // Dev mode checkbox (shows dev tools in inventory)
    let dev_mode_checkbox_y = color_blind_checkbox_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Dev Mode:",
        Vec3::new(
            right_side_x,
            dev_mode_checkbox_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            dev_mode_checkbox_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::DevMode,
        cheat_settings.dev_mode,
    );

    // Enemy damage numbers checkbox (when off, only player damage numbers show)
    let enemy_damage_checkbox_y = dev_mode_checkbox_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Damage Numbers:",
        Vec3::new(
            right_side_x,
            enemy_damage_checkbox_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            enemy_damage_checkbox_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::ShowEnemyDamageNumbers,
        cheat_settings.show_enemy_damage_numbers,
    );

    let tile_hover_checkbox_y = enemy_damage_checkbox_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Tile Hover:",
        Vec3::new(
            right_side_x,
            tile_hover_checkbox_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            tile_hover_checkbox_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::ShowTileHover,
        cheat_settings.show_tile_hover,
    );

    let hide_attack_y = tile_hover_checkbox_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Hide Attack Anims:",
        Vec3::new(
            right_side_x,
            hide_attack_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            hide_attack_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::HideAttackAnims,
        cheat_settings.hide_attack_anims,
    );

    let hide_skill_y = hide_attack_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Hide Skill Anims:",
        Vec3::new(
            right_side_x,
            hide_skill_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            hide_skill_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::HideSkillAnims,
        cheat_settings.hide_skill_anims,
    );

    let hide_heirloom_y = hide_skill_y - 16.;
    spawn_options_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Hide Heirloom Anims:",
        Vec3::new(
            right_side_x,
            hide_heirloom_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        Vec3::new(
            right_side_x + 100.5,
            hide_heirloom_y + 0.5,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
        OptionsCheckboxType::HideHeirloomAnims,
        cheat_settings.hide_heirloom_anims,
    );

    // Volume section
    let volume_section_y = hide_heirloom_y - 28.;
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Volume",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                right_side_x,
                volume_section_y,
                ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Volume Section Title"),
    ));

    let music_vol_y = volume_section_y - 26.;
    spawn_volume_row(
        &mut commands,
        &graphics,
        &asset_server,
        "Music:",
        VolumeChannel::Music,
        audio_volume.music,
        Vec3::new(
            right_side_x,
            music_vol_y,
            ui_helpers::Z_DEPTH_OPTIONS_CONTENT,
        ),
    );

    let sfx_vol_y = music_vol_y - 18.;
    spawn_volume_row(
        &mut commands,
        &graphics,
        &asset_server,
        "SFX:",
        VolumeChannel::Sfx,
        audio_volume.sfx,
        Vec3::new(right_side_x, sfx_vol_y, ui_helpers::Z_DEPTH_OPTIONS_CONTENT),
    );

    //TODO: fix restart button
    if game_state.0 == crate::GameState::Main {
        // // Restart button
        // let restart_button = crate::ui::main_menu::spawn_menu_button(
        //     Vec3::new(0., -75., 11.),
        //     Vec3::new(-30., 0., 1.),
        //     "Restart",
        //     crate::ui::main_menu::MenuButton::OptionsRestart,
        //     Vec2::new(84., 18.),
        //     &mut commands,
        //     &graphics,
        //     &asset_server,
        //     crate::ui::UIElement::UnlocksButton,
        // );
        // commands.entity(restart_button).insert(OptionsUI);

        // Exit to Menu button
        let exit_button = crate::ui::main_menu::spawn_menu_button(
            Vec3::new(140., -148., ui_helpers::Z_DEPTH_OPTIONS_CONTENT),
            Vec3::new(-50., -1., 1.),
            "Exit to Menu",
            crate::ui::main_menu::MenuButton::OptionsExit,
            Vec2::new(118., 18.),
            &mut commands,
            &graphics,
            &asset_server,
            crate::ui::UIElement::AchievementsButton,
        );
        commands.entity(exit_button).insert(OptionsUI);
    }

    // Back Button
    let back_button = spawn_back_button(
        Vec3::new(240., -148., ui_helpers::Z_DEPTH_OPTIONS_CONTENT),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands.entity(back_button).insert(OptionsUI);
}

fn spawn_keybind_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    bind_type: KeyBindType,
    label_pos: Vec3,
    button_pos: Vec3,
    keybinds: &InputMappings,
) {
    // Get label and current key based on bind type
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
        KeyBindType::AutoAttackToggle => ("Auto Attack:", keybinds.get_auto_attack_toggle_key()),
    };

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        UIState::Options,
        Name::new(format!("Keybind Row Label {:?}", bind_type)),
    ));

    // Current key text
    let current_key_pos = Vec3::new(label_pos.x + 90., label_pos.y, label_pos.z);
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                crate::keybinds::get_key_display_name(current_key),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(current_key_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        UIState::Options,
        KeyBindText { bind_type },
        Name::new(format!("Keybind Current Key {:?}", bind_type)),
    ));

    // Rebind button
    let button_entity = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::XLKey).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(40., 12.)),
                ..Default::default()
            },
            transform: Transform::from_translation(button_pos),
            visibility: Visibility::Visible,
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::BackButton)
        .insert(OptionsUI)
        .insert(KeyBindButton { bind_type })
        .insert(Interactable::default())
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
                        color: crate::colors::WHITE,
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
) {
    // Label
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        UIState::Options,
        Name::new("Options Checkbox Label"),
    ));

    // Checkbox - uses same assets as achievements UI (CheckBox / CheckBoxSelected)
    let ui_checkbox = if is_checked {
        UIElement::CheckBoxSelected
    } else {
        UIElement::CheckBox
    };
    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_checkbox).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(9., 9.)),
                ..Default::default()
            },
            transform: Transform::from_translation(checkbox_pos),
            visibility: Visibility::Visible,
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(OptionsUI)
        .insert(OptionsCheckbox(option_type))
        .insert(Interactable::default())
        .insert(Name::new("Options Checkbox"));
}

pub fn handle_cheat_checkbox_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut checkboxes: Query<
        (
            Entity,
            &OptionsCheckbox,
            &mut Interactable,
            &mut Handle<Image>,
        ),
        With<OptionsCheckbox>,
    >,
    mut cheat_settings: ResMut<CheatSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, options_checkbox, mut interactable, mut texture) in checkboxes.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        let (setting, checkbox_ui) = match options_checkbox.0 {
                            OptionsCheckboxType::UnlockAllClasses => {
                                cheat_settings.bypass_class_unlocks =
                                    !cheat_settings.bypass_class_unlocks;
                                (
                                    cheat_settings.bypass_class_unlocks,
                                    if cheat_settings.bypass_class_unlocks {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::ColorBlindMode => {
                                cheat_settings.color_blind_mode = !cheat_settings.color_blind_mode;
                                (
                                    cheat_settings.color_blind_mode,
                                    if cheat_settings.color_blind_mode {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::DevMode => {
                                cheat_settings.dev_mode = !cheat_settings.dev_mode;
                                (
                                    cheat_settings.dev_mode,
                                    if cheat_settings.dev_mode {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::ShowEnemyDamageNumbers => {
                                cheat_settings.show_enemy_damage_numbers =
                                    !cheat_settings.show_enemy_damage_numbers;
                                (
                                    cheat_settings.show_enemy_damage_numbers,
                                    if cheat_settings.show_enemy_damage_numbers {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::ShowTileHover => {
                                cheat_settings.show_tile_hover = !cheat_settings.show_tile_hover;
                                (
                                    cheat_settings.show_tile_hover,
                                    if cheat_settings.show_tile_hover {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::HideAttackAnims => {
                                cheat_settings.hide_attack_anims =
                                    !cheat_settings.hide_attack_anims;
                                (
                                    cheat_settings.hide_attack_anims,
                                    if cheat_settings.hide_attack_anims {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::HideSkillAnims => {
                                cheat_settings.hide_skill_anims = !cheat_settings.hide_skill_anims;
                                (
                                    cheat_settings.hide_skill_anims,
                                    if cheat_settings.hide_skill_anims {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                            OptionsCheckboxType::HideHeirloomAnims => {
                                cheat_settings.hide_heirloom_anims =
                                    !cheat_settings.hide_heirloom_anims;
                                (
                                    cheat_settings.hide_heirloom_anims,
                                    if cheat_settings.hide_heirloom_anims {
                                        UIElement::CheckBoxSelected
                                    } else {
                                        UIElement::CheckBox
                                    },
                                )
                            }
                        };
                        *texture = graphics.get_ui_element_texture(checkbox_ui).clone();
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                        info!("Options: {:?} = {}", options_checkbox.0, setting);
                    }
                }
                _ => {}
            },
            _ => {
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                interactable.change(Interaction::None);
            }
        }
    }
}

pub fn update_cheat_checkbox_visual(
    cheat_settings: Res<CheatSettings>,
    mut checkboxes: Query<(&OptionsCheckbox, &mut Handle<Image>)>,
    graphics: Res<Graphics>,
) {
    if !cheat_settings.is_changed() {
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
            OptionsCheckboxType::ShowTileHover => {
                if cheat_settings.show_tile_hover {
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
) {
    // Label
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                label,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(label_pos),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        UIState::Options,
        Name::new(format!("Volume Label {:?}", channel)),
    ));

    let controls_x = label_pos.x + 50.;

    // "-" button
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
            visibility: Visibility::Visible,
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(VolumeButton {
            channel,
            direction: VolumeDirection::Down,
        })
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
                    color: crate::colors::YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                controls_x + 18.,
                label_pos.y - 3.,
                label_pos.z,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        UIState::Options,
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
            visibility: Visibility::Visible,
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Options)
        .insert(UIElement::XLKey)
        .insert(OptionsUI)
        .insert(VolumeButton {
            channel,
            direction: VolumeDirection::Up,
        })
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

pub fn handle_volume_button_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable, &VolumeButton)>,
    mut audio_volume: ResMut<AudioVolume>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, vol_button) in buttons.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                    commands
                        .entity(entity)
                        .insert(UIElement::XLKeyHover)
                        .insert(graphics.get_ui_element_texture(UIElement::XLKeyHover));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        let val = match vol_button.channel {
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
            },
            _ => {
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
            VolumeChannel::Music => audio_volume.music,
            VolumeChannel::Sfx => audio_volume.sfx,
        };
        text.sections[0].value = format!("{}", val);
    }
}
