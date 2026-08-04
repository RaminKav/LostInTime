use bevy::text::Justify;
use bevy::camera::visibility::RenderLayers;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::window::Ime;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    client::GameData,
    colors::*,
    cursor::CursorPos,
    datafiles,
    ui::{
        game_fonts as gf,
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

#[derive(Component)]
pub struct NameEntryValue;

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
    //             color: Color::srgba(0., 0., 0., 0.9),
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
    let overlay = ui_helpers::spawn_full_screen_ui_overlay(&mut commands, &resolution, 0.99, 100.);
    commands
        .entity(overlay)
        .insert(NameEntryUI)
        .insert(UIState::EnterName);
    // Panel background
    commands.spawn((
        (
            Sprite::from_color(
                Color::srgba(0.15, 0.12, 0.10, 0.98),
                Vec2::new(panel_width, panel_height),
            ),
            Transform::from_translation(Vec3::new(0., 0., 101.)),
        ),
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Name Entry Panel"),
    ));

    // Title text
    commands.spawn((
        gf::MENU_TITLE
            .text(&asset_server, "Enter Your Name", WHITE)
            .justify(Justify::Center)
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., 35., 102.),
                scale: gf::MENU_TITLE.transform_scale(),
                ..default()
            }),
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Name Entry Title"),
    ));

    // Input field background
    commands.spawn((
        (
            Sprite::from_color(DARK_WOOD_BROWN, Vec2::new(140., 18.)),
            Transform::from_translation(Vec3::new(0., 5., 102.)),
        ),
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Input Field BG"),
    ));

    // Input field inner
    commands.spawn((
        (
            Sprite::from_color(Color::srgb(0.95, 0.93, 0.88), Vec2::new(136., 14.)),
            Transform::from_translation(Vec3::new(0., 5., 103.)),
        ),
        RenderLayers::from_layers(&[3]),
        NameEntryUI,
        UIState::EnterName,
        Name::new("Input Field Inner"),
    ));

    // Input text with cursor as independently mutable native spans.
    let input = commands
        .spawn((
            gf::BODY
                .text(&asset_server, "", DARK_WOOD_BROWN)
                .justify(Justify::Left)
                .anchor(Anchor::CENTER_LEFT)
                .with_transform(Transform {
                translation: Vec3::new(-65., 5.5, 104.),
                scale: gf::BODY.transform_scale(),
                ..default()
                }),
            RenderLayers::from_layers(&[3]),
            NameEntryUI,
            NameEntryInput,
            UIState::EnterName,
            Name::new("Input Text"),
        ))
        .id();
    commands.spawn((
        TextSpan::new(""),
        gf::BODY.text_font(&asset_server),
        TextColor(DARK_WOOD_BROWN),
        NameEntryValue,
        ChildOf(input),
    ));
    commands.spawn((
        TextSpan::new("|"),
        gf::BODY.text_font(&asset_server),
        TextColor(DARK_WOOD_BROWN),
        CursorBlink,
        ChildOf(input),
    ));

    // OK Button
    let button_entity = commands
        .spawn((
            Sprite {
                image: graphics
                    .get_ui_element_texture(UIElement::MenuButton)
                    .clone(),
                custom_size: Some(Vec2::new(50., 18.)),
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(0., -30., 102.)),
        ))
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
        .spawn(
            gf::BODY
                .text(&asset_server, "OK", DARK_WOOD_BROWN)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::EnterName)
        .insert(Name::new("OK Button Text"))
        .insert(ChildOf(button_entity));
}

fn push_name_chars(current_input: &mut CurrentNameInput, value: &str) {
    for c in value.chars() {
        // Ignore control characters and limit length to 15 characters
        if (c.is_alphanumeric() || c == ' ' || c == '_' || c == '-')
            && current_input.text.len() < 15
        {
            current_input.text.push(c);
        }
    }
}

/// Handle text input for name entry
pub fn handle_name_entry_input(
    mut ime_events: MessageReader<Ime>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    key_input: Res<ButtonInput<KeyCode>>,
    mut current_input: ResMut<CurrentNameInput>,
) {
    // Handle backspace
    if key_input.just_pressed(KeyCode::Backspace) {
        current_input.text.pop();
    }

    // IME commit (when enabled) and KeyboardInput::text (normal Latin typing on 0.19).
    for event in ime_events.read() {
        if let Ime::Commit { value, .. } = event {
            push_name_chars(&mut current_input, value);
        }
    }
    for event in keyboard_events.read() {
        if event.state != bevy::input::ButtonState::Pressed {
            continue;
        }
        if let Some(text) = event.text.as_deref() {
            push_name_chars(&mut current_input, text);
        }
    }
}

/// Update the displayed text span in the input field.
pub fn update_name_entry_text(
    current_input: Res<CurrentNameInput>,
    mut text_query: Query<&mut TextSpan, With<NameEntryValue>>,
) {
    // Defensive: Check if query is empty to avoid race condition
    if text_query.is_empty() {
        return;
    }

    for mut text in text_query.iter_mut() {
        let new_value = current_input.text.clone();
        if text.0 != new_value {
            text.0 = new_value;
        }
    }
}

/// Handle OK button click
pub fn handle_name_entry_ok_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<NameEntryOKButton>>,
    current_input: Res<CurrentNameInput>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut game_data: ResMut<GameData>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
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
        GameData::try_from_json_reader(reader).unwrap_or_default()
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
    game_data: Option<Res<GameData>>,
) {
    // Only check when in Closed state
    if *current_ui_state.get() != UIState::Closed {
        return;
    }

    // Prefer the already-loaded resource (from `load_game_data_for_ui`). Falling back to a file
    // re-read used to treat any deserialize error as "needs name", which trapped the main menu
    // in EnterName after the Bevy 0.19 KeyCode rename broke legacy keybind JSON.
    let needs_name = match game_data.as_ref() {
        Some(data) => data
            .player_name
            .as_ref()
            .map(|name| name.trim().is_empty())
            .unwrap_or(true),
        None => {
            let game_data_file_path = datafiles::game_data();
            if let Ok(file) = File::open(&game_data_file_path) {
                let reader = BufReader::new(file);
                match GameData::try_from_json_reader(reader) {
                    Ok(data) => data
                        .player_name
                        .as_ref()
                        .map(|name| name.trim().is_empty())
                        .unwrap_or(true),
                    Err(_) => true,
                }
            } else {
                true
            }
        }
    };

    if needs_name {
        next_ui_state.set(UIState::EnterName);
    }
}

/// Update the cursor span's blink animation.
pub fn update_cursor_blink(
    time: Res<Time>,
    mut blink_timer: ResMut<CursorBlinkTimer>,
    mut text_query: Query<&mut TextSpan, With<CursorBlink>>,
) {
    // Defensive: Check if query is empty to avoid race condition
    if text_query.is_empty() {
        return;
    }

    blink_timer.timer.tick(time.delta());

    if blink_timer.timer.just_finished() {
        for mut text in text_query.iter_mut() {
            text.0 = if text.0 == "|" { "" } else { "|" }.to_string();
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
        commands.entity(entity).despawn();
    }
    current_input.text.clear();
    // Reset blink timer for next time
    blink_timer.timer.reset();
}
