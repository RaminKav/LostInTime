use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    inputs::CursorPos,
    keybinds::InputMappings,
    ui::{
        interactions::Interaction, spawn_back_button, ui_helpers, Interactable, UIElement, UIState,
    },
    InputBinding, ScreenResolution,
};

/// Resource to track cheat settings
#[derive(Resource, Default, Debug, Clone)]
pub struct CheatSettings {
    /// When true, all classes and pets are selectable regardless of unlock status
    pub bypass_class_unlocks: bool,
}

#[derive(Component)]
pub struct CheatCheckbox;

#[derive(Component)]
pub struct OptionsUI;

#[derive(Component)]
pub struct KeyBindButton {
    bind_type: KeyBindType,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum KeyBindType {
    ActiveSkill(usize),
    Inventory,
    Minimap,
}

#[derive(Component)]
pub struct KeyBindText {
    bind_type: KeyBindType,
}

#[derive(Component)]
pub struct WaitingForKeyInput {
    pub bind_type: KeyBindType,
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
                KeyBindType::Inventory => keybinds.set_inventory_key(InputBinding::KeyBinding(key)),
                KeyBindType::Minimap => keybinds.set_minimap_key(InputBinding::KeyBinding(key)),
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
                KeyBindType::Inventory => {
                    keybinds.set_inventory_key(InputBinding::MouseBinding(mouse_button))
                }
                KeyBindType::Minimap => {
                    keybinds.set_minimap_key(InputBinding::MouseBinding(mouse_button))
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
                KeyBindType::Inventory => keybinds.get_inventory_key(),
                KeyBindType::Minimap => keybinds.get_minimap_key(),
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
) {
    let overlay = ui_helpers::spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width, resolution.game_height),
        1.,
        10.,
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
                resolution.game_height / 2. - 20.,
                11.,
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
            transform: Transform::from_translation(Vec3::new(left_side_x, 70., 11.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Keybind Section Title"),
    ));

    let start_y = 46.5;
    let row_spacing = -16.0;

    // Skill slot keybinds (slots 0-3)
    for slot in 0..4 {
        let y = start_y + row_spacing * slot as f32;
        spawn_keybind_row(
            &mut commands,
            &graphics,
            &asset_server,
            KeyBindType::ActiveSkill(slot),
            Vec3::new(left_side_x + 2., y, 11.),
            Vec3::new(left_side_x + 160., y - 3.5, 11.),
            &keybinds,
        );
    }

    // UI keybinds section
    let ui_section_y = start_y + row_spacing * 4.5;
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
            transform: Transform::from_translation(Vec3::new(left_side_x, ui_section_y, 11.)),
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
        Vec3::new(left_side_x + 2., inventory_y, 11.),
        Vec3::new(left_side_x + 160., inventory_y - 3.5, 11.),
        &keybinds,
    );

    // Minimap keybind
    let minimap_y = inventory_y + row_spacing;
    spawn_keybind_row(
        &mut commands,
        &graphics,
        &asset_server,
        KeyBindType::Minimap,
        Vec3::new(left_side_x + 2., minimap_y, 11.),
        Vec3::new(left_side_x + 160., minimap_y - 3.5, 11.),
        &keybinds,
    );

    // Cheats section
    let cheats_section_y = 70.;
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
            transform: Transform::from_translation(Vec3::new(right_side_x, cheats_section_y, 11.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Cheats Section Title"),
    ));

    // Unlock all classes checkbox
    let checkbox_y = 50.;
    spawn_cheat_checkbox(
        &mut commands,
        &graphics,
        &asset_server,
        "Unlock All Classes:",
        Vec3::new(right_side_x, checkbox_y, 11.),
        Vec3::new(right_side_x + 100.5, checkbox_y + 0.5, 11.),
        cheat_settings.bypass_class_unlocks,
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
            Vec3::new(-110., -108., 11.),
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
        Vec3::new(0., -108., 11.),
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
                0 => "Roll:",
                1 => "Skill Slot 1:",
                2 => "Skill Slot 2:",
                3 => "Blessing Skill Slot:",
                _ => "Unknown Slot",
            };
            (label, keybinds.get_active_skill_key(slot))
        }
        KeyBindType::Inventory => ("Inventory:", keybinds.get_inventory_key()),
        KeyBindType::Minimap => ("Map:", keybinds.get_minimap_key()),
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

fn spawn_cheat_checkbox(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    label: &str,
    label_pos: Vec3,
    checkbox_pos: Vec3,
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
        Name::new("Cheat Checkbox Label"),
    ));

    // Checkbox - uses same assets as achievements UI (CheckBox / CheckBoxSelected)
    // Use the current state to determine initial texture
    let checkbox_type = if is_checked {
        UIElement::CheckBoxSelected
    } else {
        UIElement::CheckBox
    };
    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(checkbox_type).clone(),
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
        .insert(CheatCheckbox)
        .insert(Interactable::default())
        .insert(Name::new("Cheat Checkbox"));
}

pub fn handle_cheat_checkbox_click(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut checkboxes: Query<(Entity, &mut Interactable, &mut Handle<Image>), With<CheatCheckbox>>,
    mut cheat_settings: ResMut<CheatSettings>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable, mut texture) in checkboxes.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        // Toggle the cheat setting
                        cheat_settings.bypass_class_unlocks = !cheat_settings.bypass_class_unlocks;

                        // Update checkbox texture
                        let checkbox_type = if cheat_settings.bypass_class_unlocks {
                            UIElement::CheckBoxSelected
                        } else {
                            UIElement::CheckBox
                        };
                        *texture = graphics.get_ui_element_texture(checkbox_type).clone();

                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                        info!(
                            "Cheat: bypass_class_unlocks = {}",
                            cheat_settings.bypass_class_unlocks
                        );
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
    mut checkboxes: Query<&mut Handle<Image>, With<CheatCheckbox>>,
    graphics: Res<Graphics>,
) {
    if !cheat_settings.is_changed() {
        return;
    }

    let checkbox_type = if cheat_settings.bypass_class_unlocks {
        UIElement::CheckBoxSelected
    } else {
        UIElement::CheckBox
    };

    for mut texture in checkboxes.iter_mut() {
        *texture = graphics
            .get_ui_element_texture(checkbox_type.clone())
            .clone();
    }
}
