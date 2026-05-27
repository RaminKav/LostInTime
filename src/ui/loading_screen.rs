use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::{
    colors::DARK_WOOD_BROWN, enemy::spawner::GlobalSpawners, player::Player,
    ui::ui_helpers,
    world::chunk::DoneCreateChunkEvent, GameState, RenderLayers, ScreenResolution,
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
                custom_size: Some(ui_helpers::full_screen_overlay_size(&res)),
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

/// Time to wait in Initializing after the main timer finishes before forcing transition if no chunks exist (fallback for stuck state).
const STUCK_FALLBACK_SECS: f32 = 10.0;

pub struct InitializationTimer {
    timer: Timer,
    done_chunk_event_received: bool,
    /// When we're waiting for chunks after timer finished; after STUCK_FALLBACK_SECS we force transition.
    stuck_fallback_timer: Option<Timer>,
    /// Avoid `info!` every frame while the init timer runs (with trace_tracy, each line formats span context).
    last_logged_timer_pct: Option<u32>,
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
    spawners: Option<ResMut<GlobalSpawners>>,
) {
    // Initialize timer on first run
    if init_timer.is_none() {
        *init_timer = Some(InitializationTimer {
            timer: Timer::from_seconds(2.0, TimerMode::Once), // Wait 2 seconds after chunks start generating to allow objects to spawn
            done_chunk_event_received: false,
            stuck_fallback_timer: None,
            last_logged_timer_pct: None,
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

    // Start stuck fallback when we have player + done event + main timer finished but still 0 chunks
    let waiting_for_chunks_stuck = player_exists
        && timer.done_chunk_event_received
        && timer.timer.finished()
        && chunks_created == 0;
    if waiting_for_chunks_stuck {
        if timer.stuck_fallback_timer.is_none() {
            timer.stuck_fallback_timer =
                Some(Timer::from_seconds(STUCK_FALLBACK_SECS, TimerMode::Once));
        }
        if let Some(ref mut t) = timer.stuck_fallback_timer {
            t.tick(time.delta());
        }
    } else {
        timer.stuck_fallback_timer = None;
    }

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
        let pct =
            (timer.timer.elapsed_secs() / timer.timer.duration().as_secs_f32() * 100.0) as u32;
        if timer.last_logged_timer_pct != Some(pct) {
            timer.last_logged_timer_pct = Some(pct);
            info!("Waiting for timer to finish ({}%)...", pct);
        }
    }

    // Force transition if we've been stuck with 0 chunks for too long (safety fallback)
    let force_transition_stuck = waiting_for_chunks_stuck
        && timer
            .stuck_fallback_timer
            .as_ref()
            .map(|t| t.finished())
            .unwrap_or(false);
    if force_transition_stuck {
        warn!(
            "Initialization stuck with 0 chunks after {:.0}s fallback; forcing transition to Main",
            STUCK_FALLBACK_SECS
        );
        for entity in loading_screens.iter() {
            commands.entity(entity).despawn_recursive();
        }
        next_state.set(GameState::Main);
        *init_timer = None;
        if let Some(mut spawners) = spawners {
            spawners.initial_spawn_delay.reset();
        }
        return;
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
        *init_timer = None;
        if let Some(mut spawners) = spawners {
            spawners.initial_spawn_delay.reset();
        }
    }
}
