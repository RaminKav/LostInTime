use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    inputs::CursorPos,
    keybinds::KeyBindings,
    ui::{
        interactions::Interaction, spawn_back_button, ui_helpers, Interactable, UIElement, UIState,
    },
    ScreenResolution,
};

#[derive(Component)]
pub struct OptionsUI;

#[derive(Component)]
pub struct KeyBindButton {
    slot: usize,
}

#[derive(Component)]
pub struct KeyBindText {
    slot: usize,
}

#[derive(Component)]
pub struct WaitingForKeyInput {
    pub slot: usize,
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
                        commands
                            .entity(entity)
                            .insert(WaitingForKeyInput { slot: button.slot });
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
    mut keybinds: ResMut<KeyBindings>,
    waiting: Query<(Entity, &WaitingForKeyInput)>,
    graphics: Res<Graphics>,
) {
    if waiting.is_empty() {
        return;
    }

    // Collect keys to avoid borrow checker issues
    let just_pressed: Vec<KeyCode> = key_input.get_just_pressed().copied().collect();

    // Check for any key press
    for key in just_pressed {
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
            keybinds.set_active_skill_key(waiting_for.slot, key);
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
}

pub fn update_keybind_text(
    keybinds: Res<KeyBindings>,
    waiting: Query<&WaitingForKeyInput>,
    mut texts: Query<(&KeyBindText, &mut Text)>,
    mut was_waiting: Local<bool>,
) {
    // Collect waiting slots to avoid borrow issues
    let waiting_slots: Vec<usize> = waiting.iter().map(|w| w.slot).collect();
    let is_waiting = !waiting_slots.is_empty();

    // Update if:
    // - Keybinds changed, OR
    // - There's waiting input, OR
    // - We just stopped waiting (to show the new key)
    if !keybinds.is_changed() && !is_waiting && !*was_waiting {
        return;
    }

    *was_waiting = is_waiting;

    for (key_text, mut text) in texts.iter_mut() {
        if waiting_slots.contains(&key_text.slot) {
            text.sections[0].value = "Press any key...".to_string();
            text.sections[0].style.color = crate::colors::YELLOW_2;
        } else {
            let key = keybinds.get_active_skill_key(key_text.slot);
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
    keybinds: Res<KeyBindings>,
    game_state: Res<State<crate::GameState>>,
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
    let text_x = -resolution.game_width / 2. + 22.;
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
            transform: Transform::from_translation(Vec3::new(text_x, 70., 11.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Keybind Section Title"),
    ));

    let start_y = 46.5;
    let row_spacing = -16.0;

    // Skill slot keybinds
    for slot in 0..3 {
        let y = start_y + row_spacing * slot as f32;
        spawn_keybind_row(
            &mut commands,
            &graphics,
            &asset_server,
            slot,
            Vec3::new(text_x + 2., y, 11.),
            Vec3::new(text_x + 160., y - 3.5, 11.),
            &keybinds,
        );
    }
    //TODO: FIX THESE BUTTONS
    // // Only show Restart and Exit buttons during an active game (not in main menu)
    // if game_state.0 == crate::GameState::Main {
    //     // Restart button
    //     let restart_button = crate::ui::main_menu::spawn_menu_button(
    //         Vec3::new(0., -55., 11.),
    //         Vec3::new(-30., 0., 1.),
    //         "Restart Run",
    //         crate::ui::main_menu::MenuButton::OptionsRestart,
    //         Vec2::new(84., 18.),
    //         &mut commands,
    //         &graphics,
    //         &asset_server,
    //         crate::ui::UIElement::UnlocksButton,
    //     );
    //     commands.entity(restart_button).insert(OptionsUI);

    //     // Exit to Menu button
    //     let exit_button = crate::ui::main_menu::spawn_menu_button(
    //         Vec3::new(0., -80., 11.),
    //         Vec3::new(-30., 0., 1.),
    //         "Exit to Menu",
    //         crate::ui::main_menu::MenuButton::OptionsExit,
    //         Vec2::new(84., 18.),
    //         &mut commands,
    //         &graphics,
    //         &asset_server,
    //         crate::ui::UIElement::UnlocksButton,
    //     );
    //     commands.entity(exit_button).insert(OptionsUI);
    // }

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
    slot: usize,
    label_pos: Vec3,
    button_pos: Vec3,
    keybinds: &KeyBindings,
) {
    // Skill slot label
    let slot_name = match slot {
        0 => "Skill Slot 1:",
        1 => "Skill Slot 2:",
        2 => "Skill Slot 3:",
        _ => "Unknown Slot",
    };
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                slot_name,
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
        Name::new(format!("Keybind Row Label {}", slot)),
    ));

    // Current key text
    let key = keybinds.get_active_skill_key(slot);
    let current_key_pos = Vec3::new(label_pos.x + 60., label_pos.y, label_pos.z);
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                crate::keybinds::get_key_display_name(key),
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
        KeyBindText { slot },
        Name::new(format!("Keybind Current Key {}", slot)),
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
        .insert(KeyBindButton { slot })
        .insert(Interactable::default())
        .insert(Name::new(format!("Keybind Button {}", slot)))
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
            Name::new(format!("Keybind Button Label {}", slot)),
        ))
        .set_parent(button_entity);
}
