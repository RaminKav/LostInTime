use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    client::GameData,
    colors::*,
    datafiles,
    inputs::CursorPos,
    ui::{
        interactions::{Interactable, Interaction, UIElement},
        inventory_ui::UIState,
        ui_helpers,
    },
    ScreenResolution,
};
use std::fs::File;
use std::io::{BufReader, BufWriter};

#[derive(Component)]
pub struct NameEntryUI;

#[derive(Component)]
pub struct NameEntryInput;

#[derive(Component)]
pub struct NameEntryOKButton;

#[derive(Component)]
pub struct CursorBlink;

#[derive(Resource, Default)]
pub struct CurrentNameInput {
    pub text: String,
}

#[derive(Resource)]
pub struct CursorBlinkTimer {
    pub timer: Timer,
}

impl Default for CursorBlinkTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(0.5, TimerMode::Repeating),
        }
    }
}

/// Setup the name entry popup UI
pub fn setup_name_entry_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
) {
    let panel_width = 180.0;
    let panel_height = 100.0;

    // // Background overlay (darken screen)
    // commands.spawn((
    //     SpriteBundle {
    //         sprite: Sprite {
    //             color: Color::rgba(0., 0., 0., 0.9),
    //             custom_size: Some(Vec2::new(
    //                 resolution.game_width + 10.,
    //                 resolution.game_height + 20.,
    //             )),
    //             ..Default::default()
    //         },
    //         transform: Transform::from_translation(Vec3::new(0., 0., 100.)),
    //         ..Default::default()
    //     },
    //     RenderLayers::from_layers(&[3]),
    //     NameEntryUI,
    //     UIState::EnterName,
    //     Name::new("Name Entry Overlay"),
    // ));
    let overlay = ui_helpers::spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width, resolution.game_height),
        0.99,
        100.,
    );
    commands
        .entity(overlay)
        .insert(NameEntryUI)
        .insert(UIState::EnterName);
    // Panel background
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.15, 0.12, 0.10, 0.98),
                custom_size: Some(Vec2::new(panel_width, panel_height)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 101.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Name Entry Panel"),
    ));

    // Title text
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Enter Your Name",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 35., 102.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Name Entry Title"),
    ));

    // Input field background
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: DARK_WOOD_BROWN,
                custom_size: Some(Vec2::new(140., 18.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 5., 102.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Input Field BG"),
    ));

    // Input field inner
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.95, 0.93, 0.88),
                custom_size: Some(Vec2::new(136., 14.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 5., 103.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Input Field Inner"),
    ));

    // Input text with cursor (using two sections so cursor follows text)
    let font_handle = asset_server.load("fonts/4x5.ttf");
    commands.spawn((
        Text2dBundle {
            text: Text {
                sections: vec![
                    TextSection {
                        value: "".to_string(),
                        style: TextStyle {
                            font: font_handle.clone(),
                            font_size: 5.0,
                            color: DARK_WOOD_BROWN,
                        },
                    },
                    TextSection {
                        value: "|".to_string(),
                        style: TextStyle {
                            font: font_handle,
                            font_size: 5.0,
                            color: DARK_WOOD_BROWN,
                        },
                    },
                ],
                alignment: TextAlignment::Left,
                ..Default::default()
            },
            text_anchor: Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(-65., 5.5, 104.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        NameEntryInput,
        CursorBlink,
        UIState::EnterName,
        Name::new("Input Text"),
    ));

    // OK Button
    let button_entity = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::MenuButton)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(50., 18.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., -30., 102.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIElement::MenuButton)
        .insert(Interactable::default())
        .insert(NameEntryOKButton)
        .insert(NameEntryUI)
        .insert(UIState::EnterName)
        .insert(Name::new("OK Button"))
        .id();

    // OK Button text
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "OK",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::EnterName)
        .insert(Name::new("OK Button Text"))
        .set_parent(button_entity);
}

/// Handle text input for name entry
pub fn handle_name_entry_input(
    mut char_events: EventReader<ReceivedCharacter>,
    key_input: Res<Input<KeyCode>>,
    mut current_input: ResMut<CurrentNameInput>,
) {
    // Handle backspace
    if key_input.just_pressed(KeyCode::Back) {
        current_input.text.pop();
    }

    // Handle character input
    for event in char_events.iter() {
        let c = event.char;

        // Ignore control characters and limit length to 15 characters
        if c.is_alphanumeric() || c == ' ' || c == '_' || c == '-' {
            if current_input.text.len() < 15 {
                current_input.text.push(c);
            }
        }
    }
}

/// Update the displayed text in the input field
/// We rebuild the entire Text to ensure Bevy's change detection picks it up
pub fn update_name_entry_text(
    current_input: Res<CurrentNameInput>,
    mut text_query: Query<&mut Text, With<NameEntryInput>>,
    asset_server: Res<AssetServer>,
) {
    // Defensive: Check if query is empty to avoid race condition
    if text_query.is_empty() {
        return;
    }

    for mut text in text_query.iter_mut() {
        let new_value = current_input.text.clone();
        // Only update if different to avoid unnecessary work
        if text.sections[0].value != new_value {
            // Rebuild the entire Text to force change detection
            let font_handle = asset_server.load("fonts/4x5.ttf");
            let cursor_value = text
                .sections
                .get(1)
                .map(|s| s.value.clone())
                .unwrap_or_else(|| "|".to_string());

            *text = Text {
                sections: vec![
                    TextSection {
                        value: new_value,
                        style: TextStyle {
                            font: font_handle.clone(),
                            font_size: 5.0,
                            color: DARK_WOOD_BROWN,
                        },
                    },
                    TextSection {
                        value: cursor_value,
                        style: TextStyle {
                            font: font_handle,
                            font_size: 5.0,
                            color: DARK_WOOD_BROWN,
                        },
                    },
                ],
                alignment: TextAlignment::Left,
                ..Default::default()
            };
        }
    }
}

/// Handle OK button click
pub fn handle_name_entry_ok_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<NameEntryOKButton>>,
    current_input: Res<CurrentNameInput>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable) in buttons.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        // Save the name and close the UI
                        let name = current_input.text.trim().to_string();
                        if !name.is_empty() {
                            save_player_name(&name, &mut game_data);
                            next_ui_state.set(UIState::Closed);
                            commands
                                .spawn(SoundSpawner::new(AudioSoundEffect::UISkillSelection, 0.1));
                        }
                    }
                }
                _ => {}
            },
            _ => {
                interactable.change(Interaction::None);
            }
        }
    }
}

/// Save player name to game_data.json and update the resource
fn save_player_name(name: &str, game_data_resource: &mut GameData) {
    let game_data_file_path = datafiles::game_data();

    // Load existing game data or create new
    let mut game_data: GameData = if let Ok(file) = File::open(&game_data_file_path) {
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap_or_default()
    } else {
        GameData::default()
    };

    // Update player name
    game_data.player_name = Some(name.to_string());

    // ALSO update the in-memory resource so it's available immediately
    game_data_resource.player_name = Some(name.to_string());
    info!("Updated GameData resource with player name: {}", name);

    // Save back to file
    if let Ok(file) = File::create(&game_data_file_path) {
        let writer = BufWriter::new(file);
        if let Err(e) = serde_json::to_writer_pretty(writer, &game_data) {
            error!("Failed to save player name: {:?}", e);
        } else {
            info!("Player name saved to file: {}", name);
        }
    }
}

/// System to check if we need to show the name entry popup
pub fn check_show_name_entry_popup(
    mut next_ui_state: ResMut<NextState<UIState>>,
    current_ui_state: Res<State<UIState>>,
) {
    // Only check when in Closed state
    if current_ui_state.0 != UIState::Closed {
        return;
    }

    let game_data_file_path = datafiles::game_data();

    // Check if player name exists
    let needs_name = if let Ok(file) = File::open(&game_data_file_path) {
        let reader = BufReader::new(file);
        match serde_json::from_reader::<_, GameData>(reader) {
            Ok(data) => data.player_name.is_none() || data.player_name.as_ref().unwrap().is_empty(),
            Err(_) => true, // File doesn't exist or is corrupted, need name
        }
    } else {
        true // File doesn't exist, need name
    };

    if needs_name {
        next_ui_state.set(UIState::EnterName);
    }
}

/// Update cursor blink animation
/// We rebuild the entire Text to ensure Bevy's change detection picks it up
pub fn update_cursor_blink(
    time: Res<Time>,
    mut blink_timer: ResMut<CursorBlinkTimer>,
    mut text_query: Query<&mut Text, With<CursorBlink>>,
    asset_server: Res<AssetServer>,
) {
    // Defensive: Check if query is empty to avoid race condition
    if text_query.is_empty() {
        return;
    }

    blink_timer.timer.tick(time.delta());

    if blink_timer.timer.just_finished() {
        for mut text in text_query.iter_mut() {
            if text.sections.len() > 1 {
                let user_text = text.sections[0].value.clone();
                let current_cursor = &text.sections[1].value;
                let new_cursor = if current_cursor == "|" {
                    "".to_string()
                } else {
                    "|".to_string()
                };

                // Rebuild the entire Text to force change detection
                let font_handle = asset_server.load("fonts/4x5.ttf");
                *text = Text {
                    sections: vec![
                        TextSection {
                            value: user_text,
                            style: TextStyle {
                                font: font_handle.clone(),
                                font_size: 5.0,
                                color: DARK_WOOD_BROWN,
                            },
                        },
                        TextSection {
                            value: new_cursor,
                            style: TextStyle {
                                font: font_handle,
                                font_size: 5.0,
                                color: DARK_WOOD_BROWN,
                            },
                        },
                    ],
                    alignment: TextAlignment::Left,
                    ..Default::default()
                };
            }
        }
    }
}

/// Cleanup name entry UI
pub fn cleanup_name_entry_ui(
    mut commands: Commands,
    query: Query<Entity, With<NameEntryUI>>,
    mut current_input: ResMut<CurrentNameInput>,
    mut blink_timer: ResMut<CursorBlinkTimer>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    current_input.text.clear();
    // Reset blink timer for next time
    blink_timer.timer.reset();
}
