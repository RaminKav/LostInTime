use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::{
    assets::Graphics,
    client::leaderboard::{FetchLeaderboardEvent, LeaderboardCache},
    colors::*,
    inputs::CursorPos,
    ui::{
        interactions::{Interactable, Interaction, UIElement},
        inventory_ui::UIState,
        ui_helpers,
    },
    ScreenResolution,
};

#[derive(Component)]
pub struct LeaderboardUI;

#[derive(Component)]
pub struct LeaderboardEntryText;

/// Setup the leaderboard UI panel (compact version for main menu)
pub fn setup_leaderboard_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    cache: Res<LeaderboardCache>,
    existing_query: Query<(), With<LeaderboardUI>>,
) {
    info!("=== SETUP_LEADERBOARD_UI CALLED ===");

    // Don't setup if UI already exists
    if !existing_query.is_empty() {
        info!("Leaderboard UI already exists, skipping setup");
        return;
    }

    info!(
        "Cache state - is_loading: {}, entries: {}, has_error: {}",
        cache.is_loading,
        cache.entries.len(),
        cache.last_error.is_some()
    );

    let panel_width = 102.0;
    let panel_height = 75.0;

    // Position in top left corner
    let panel_x = -resolution.game_width / 2. + panel_width / 2. + 5.;
    let panel_y = resolution.game_height / 2. - panel_height / 2. - 5.;

    // Background panel
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.15, 0.12, 0.10, 0.95),
                custom_size: Some(Vec2::new(panel_width, panel_height)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(panel_x, panel_y, 9.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        LeaderboardUI,
        UIState::Closed,
        Name::new("Leaderboard Panel"),
    ));

    // Title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Leaderboard",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                panel_x + 0.5,
                panel_y + panel_height / 2. - 8.5,
                12.,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        LeaderboardUI,
        UIState::Closed,
        Name::new("Leaderboard Title"),
    ));

    // Spawn entry text entities (will be updated by update system)
    spawn_leaderboard_entries(
        &mut commands,
        &asset_server,
        &cache,
        panel_x,
        panel_y,
        panel_width,
        panel_height,
    );
}

/// Spawn or update leaderboard entry texts
fn spawn_leaderboard_entries(
    commands: &mut Commands,
    asset_server: &AssetServer,
    cache: &LeaderboardCache,
    panel_x: f32,
    panel_y: f32,
    panel_width: f32,
    panel_height: f32,
) {
    info!(
        "spawn_leaderboard_entries - is_loading: {}, entries: {}, error: {}",
        cache.is_loading,
        cache.entries.len(),
        cache.last_error.is_some()
    );

    let text_x = panel_x - panel_width / 2.;
    let start_y = panel_y + panel_height / 2. - 18.5;
    let row_spacing = -12.0;

    // Loading or entries
    if cache.is_loading {
        info!("Spawning LOADING text");

        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Loading...",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: YELLOW_2,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: bevy::sprite::Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(text_x, start_y, 11.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            LeaderboardUI,
            LeaderboardEntryText,
            UIState::Closed,
            Name::new("Loading Text"),
        ));
    } else if let Some(_error) = &cache.last_error {
        info!("Spawning ERROR text");
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Server offline",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: RED,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: bevy::sprite::Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(text_x, start_y, 11.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            LeaderboardUI,
            LeaderboardEntryText,
            UIState::Closed,
            Name::new("Error Text"),
        ));
    } else if cache.entries.is_empty() {
        info!("Spawning NO SCORES text");
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "No scores!",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: DARK_WOOD_BROWN,
                    },
                )
                .with_alignment(TextAlignment::Left),
                text_anchor: bevy::sprite::Anchor::CenterLeft,
                transform: Transform::from_translation(Vec3::new(text_x, start_y, 11.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            LeaderboardUI,
            LeaderboardEntryText,
            UIState::Closed,
            Name::new("Empty Text"),
        ));
    } else {
        // Display entries (only top 5)
        info!(
            "Spawning {} leaderboard entries",
            cache.entries.len().min(5)
        );
        for (i, entry) in cache.entries.iter().enumerate().take(5) {
            let y = start_y + row_spacing * i as f32;

            // Rank
            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format!("{}.", entry.rank.unwrap_or(0)),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: YELLOW,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(text_x, y, 15.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                LeaderboardUI,
                LeaderboardEntryText,
                UIState::Closed,
            ));

            // Truncate name if too long
            let name = if entry.player_name.len() > 9 {
                format!("{}...", &entry.player_name[..9])
            } else {
                entry.player_name.clone()
            };

            // Player name
            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        &name,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(text_x + 8., y, 15.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                LeaderboardUI,
                LeaderboardEntryText,
                UIState::Closed,
            ));

            // Score
            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format!("{}", entry.score),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: YELLOW_2,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(text_x + 48., y, 15.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                LeaderboardUI,
                LeaderboardEntryText,
                UIState::Closed,
            ));

            // Class (truncated to 4 chars)
            let class_short = if entry.class.len() > 4 {
                &entry.class[..4]
            } else {
                &entry.class
            };

            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        class_short,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: LIGHT_BLUE,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(text_x + 74., y, 15.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                LeaderboardUI,
                LeaderboardEntryText,
                UIState::Closed,
            ));
        }
    }
}

/// Update leaderboard display when cache changes
pub fn update_leaderboard_display(
    mut commands: Commands,
    cache: Res<LeaderboardCache>,
    asset_server: Res<AssetServer>,
    entry_query: Query<Entity, With<LeaderboardEntryText>>,
    panel_query: Query<
        &Transform,
        (
            With<LeaderboardUI>,
            Without<LeaderboardEntryText>,
            Without<Text>,
        ),
    >,
) {
    if !cache.is_changed() {
        return;
    }

    // Despawn old entry texts
    for entity in entry_query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Get panel position
    if let Ok(panel_transform) = panel_query.get_single() {
        let panel_width = 90.0;
        let panel_height = 75.0;
        let panel_x = panel_transform.translation.x;
        let panel_y = panel_transform.translation.y;

        // Respawn entries with new data
        spawn_leaderboard_entries(
            &mut commands,
            &asset_server,
            &cache,
            panel_x,
            panel_y,
            panel_width,
            panel_height,
        );
    }
}

/// Ensure leaderboard entries exist when UI is present but entries are missing
pub fn ensure_leaderboard_entries(
    mut commands: Commands,
    cache: Res<LeaderboardCache>,
    asset_server: Res<AssetServer>,
    panel_query: Query<&Transform, (With<LeaderboardUI>, Without<Text>)>,
    entry_query: Query<(), With<LeaderboardEntryText>>,
) {
    // Only run if we have the panel but no entries
    if panel_query.is_empty() {
        return;
    }

    if !entry_query.is_empty() {
        return;
    }

    // We have a panel but no entries - spawn them
    if let Ok(panel_transform) = panel_query.get_single() {
        info!(
            "Ensuring leaderboard entries exist (is_loading: {}, entries: {})",
            cache.is_loading,
            cache.entries.len()
        );

        let panel_width = 90.0;
        let panel_height = 75.0;
        let panel_x = panel_transform.translation.x;
        let panel_y = panel_transform.translation.y;

        spawn_leaderboard_entries(
            &mut commands,
            &asset_server,
            &cache,
            panel_x,
            panel_y,
            panel_width,
            panel_height,
        );
    }
}

/// Cleanup leaderboard UI when exiting the main menu
pub fn cleanup_leaderboard_ui(mut commands: Commands, query: Query<Entity, With<LeaderboardUI>>) {
    let count = query.iter().count();
    if count > 0 {
        info!(
            "=== CLEANUP_LEADERBOARD_UI despawning {} entities ===",
            count
        );
    }
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
