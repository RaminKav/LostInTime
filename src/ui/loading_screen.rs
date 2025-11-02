use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::{
    colors::DARK_WOOD_BROWN, player::Player, world::chunk::DoneCreateChunkEvent, GameState,
    RenderLayers, ScreenResolution, GAME_HEIGHT,
};

#[derive(Component)]
pub struct LoadingScreen;

pub fn setup_loading_screen(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
) {
    // Background overlay
    let _background = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 1.0),
                custom_size: Some(Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 100.),
                ..default()
            },
            ..default()
        })
        .insert(LoadingScreen)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("LOADING SCREEN BG"))
        .id();

    // Loading text
    let _loading_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Generating World...",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: DARK_WOOD_BROWN,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 0., 101.),
                ..default()
            },
            ..default()
        })
        .insert(LoadingScreen)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("LOADING TEXT"))
        .id();
}

pub struct InitializationTimer {
    timer: Timer,
    done_chunk_event_received: bool,
}

pub fn check_initialization_complete(
    mut next_state: ResMut<NextState<GameState>>,
    player_query: Query<Entity, With<Player>>,
    mut done_chunk_event: EventReader<DoneCreateChunkEvent>,
    mut commands: Commands,
    loading_screens: Query<Entity, With<LoadingScreen>>,
    chunk_query: Query<&crate::world::chunk::Chunk>,
    mut init_timer: Local<Option<InitializationTimer>>,
    time: Res<Time>,
) {
    // Initialize timer on first run
    if init_timer.is_none() {
        *init_timer = Some(InitializationTimer {
            timer: Timer::from_seconds(2.0, TimerMode::Once), // Wait 2 seconds after chunks start generating to allow objects to spawn
            done_chunk_event_received: false,
        });
        info!("Initialization timer started");
    }

    let timer = init_timer.as_mut().unwrap();

    // Check if DoneCreateChunkEvent has been received (signals chunk generation has started)
    if done_chunk_event.iter().next().is_some() {
        if !timer.done_chunk_event_received {
            info!("DoneCreateChunkEvent received, starting timer");
        }
        timer.done_chunk_event_received = true;
    }

    // Wait a bit after chunks start generating to allow them to finish and objects to spawn
    if timer.done_chunk_event_received {
        timer.timer.tick(time.delta());
    }

    // Check if player exists
    let player_exists = player_query.get_single().is_ok();

    // Count how many chunks have been created (at least a few should exist)
    let chunks_created = chunk_query.iter().count();

    // Debug logging
    if !player_exists {
        info!("Waiting for player to spawn...");
    }
    if !timer.done_chunk_event_received {
        info!("Waiting for DoneCreateChunkEvent...");
    }
    if chunks_created == 0 {
        info!(
            "Waiting for chunks to be created (currently: {})...",
            chunks_created
        );
    }
    if timer.done_chunk_event_received && !timer.timer.finished() {
        info!(
            "Waiting for timer to finish ({}%)...",
            (timer.timer.elapsed_secs() / timer.timer.duration().as_secs_f32() * 100.0) as u32
        );
    }

    // Transition when player exists, chunk generation has started, we've waited for objects to spawn,
    // and at least some chunks have been created
    if player_exists
        && timer.done_chunk_event_received
        && timer.timer.finished()
        && chunks_created > 0
    {
        info!(
            "Initialization complete! Player: {}, Chunks: {}, Transitioning to Main",
            player_exists, chunks_created
        );
        // Remove loading screen
        for entity in loading_screens.iter() {
            commands.entity(entity).despawn_recursive();
        }

        // Transition to Main state
        next_state.set(GameState::Main);
    }
}
