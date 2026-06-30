use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::{
    client::leaderboard::LeaderboardCache,
    colors::*,
    ui::{inventory_ui::UIState, main_menu::MainMenuLeaderboardVisible},
    ScreenResolution,
};

fn format_score(score: i32) -> String {
    super::ui_helpers::format_number(score as i64)
}

/// Must match the leaderboard panel sprite and `setup_leaderboard_ui` placement math.
const LEADERBOARD_PANEL_WIDTH: f32 = 160.0;
const LEADERBOARD_PANEL_HEIGHT: f32 = 135.0;

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
    visible: Res<MainMenuLeaderboardVisible>,
    existing_query: Query<(), With<LeaderboardUI>>,
) {
    if !visible.0 || !existing_query.is_empty() {
        return;
    }

    spawn_leaderboard_panel(&mut commands, &asset_server, &resolution, &cache);
}

fn spawn_leaderboard_panel(
    commands: &mut Commands,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    cache: &LeaderboardCache,
) {
    info!("=== SETUP_LEADERBOARD_UI CALLED ===");

    info!(
        "Cache state - is_loading: {}, entries: {}, has_error: {}",
        cache.is_loading,
        cache.entries.len(),
        cache.last_error.is_some()
    );

    // Position in top left corner
    let panel_x = -resolution.game_width / 2. + LEADERBOARD_PANEL_WIDTH / 2. + 5.;
    let panel_y = resolution.game_height / 2. - LEADERBOARD_PANEL_HEIGHT / 2. - 15.;
    info!("LEADER BOARD {:?}", panel_x);

    // Background panel
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.15, 0.12, 0.10, 0.95),
                custom_size: Some(Vec2::new(LEADERBOARD_PANEL_WIDTH, LEADERBOARD_PANEL_HEIGHT)),
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
                panel_y + LEADERBOARD_PANEL_HEIGHT / 2. - 8.5,
                12.,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        LeaderboardUI,
        UIState::Closed,
        Name::new("Leaderboard Title"),
    ));

    spawn_leaderboard_entries(
        commands,
        asset_server,
        cache,
        panel_x,
        panel_y,
        LEADERBOARD_PANEL_WIDTH,
        LEADERBOARD_PANEL_HEIGHT,
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
    let text_x = panel_x - panel_width / 2. + 4.;
    let start_y = panel_y + panel_height / 2. - 18.5;
    let row_spacing = -12.0;

    info!(
        "spawn_leaderboard_entries - is_loading: {}, entries: {}, error: {} {}",
        cache.is_loading,
        cache.entries.len(),
        cache.last_error.is_some(),
        text_x
    );
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
        // Display entries (only top )
        info!(
            "Spawning {} leaderboard entries",
            cache.entries.len().min(10)
        );
        for (i, entry) in cache.entries.iter().enumerate().take(10) {
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
                    text_anchor: bevy::sprite::Anchor::CenterRight,
                    transform: Transform::from_translation(Vec3::new(text_x + 9., y, 15.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                LeaderboardUI,
                LeaderboardEntryText,
                UIState::Closed,
            ));

            // Truncate name if too long
            let name = entry.player_name.clone();

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
                    transform: Transform::from_translation(Vec3::new(text_x + 12., y, 15.)),
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
                        format_score(entry.score),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: YELLOW_2,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(text_x + 94., y, 15.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                LeaderboardUI,
                LeaderboardEntryText,
                UIState::Closed,
            ));

            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        &entry.class,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: LIGHT_BLUE,
                        },
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(text_x + 122., y, 15.)),
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
        let panel_x = panel_transform.translation.x;
        let panel_y = panel_transform.translation.y;

        // Respawn entries with new data
        spawn_leaderboard_entries(
            &mut commands,
            &asset_server,
            &cache,
            panel_x,
            panel_y,
            LEADERBOARD_PANEL_WIDTH,
            LEADERBOARD_PANEL_HEIGHT,
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

        let panel_x = panel_transform.translation.x;
        let panel_y = panel_transform.translation.y;

        spawn_leaderboard_entries(
            &mut commands,
            &asset_server,
            &cache,
            panel_x,
            panel_y,
            LEADERBOARD_PANEL_WIDTH,
            LEADERBOARD_PANEL_HEIGHT,
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

/// Show or hide the main-menu leaderboard panel according to [`MainMenuLeaderboardVisible`].
pub fn sync_main_menu_leaderboard_ui(
    mut commands: Commands,
    visible: Res<MainMenuLeaderboardVisible>,
    ui_state: Res<State<UIState>>,
    existing: Query<Entity, With<LeaderboardUI>>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    cache: Res<LeaderboardCache>,
) {
    if ui_state.0 != UIState::Closed {
        return;
    }

    if visible.0 {
        if existing.is_empty() {
            spawn_leaderboard_panel(&mut commands, &asset_server, &resolution, &cache);
        }
    } else {
        for entity in existing.iter() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
