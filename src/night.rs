use bevy::{prelude::*, render::view::RenderLayers};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{BGMPicker, UpdateBGMTrackEvent},
    client::is_not_paused,
    colors::{overwrite_alpha, NIGHT},
    run_once_per_run,
    world::dimension::EraManager,
    GameState, ScreenResolution, GAME_HEIGHT,
};

#[derive(Component)]
pub struct Night(Timer);

/// Resource to track infinite mode state (activated after defeating Era 3 boss or era timer expiration)
#[derive(Default, Resource, Clone, Debug)]
pub struct InfiniteMode {
    pub active: bool,
    pub chaos_timer: Timer,
    /// Difficulty level (0-5), increases every 2 minutes
    pub difficulty_level: u8,
    /// Timer for difficulty scaling (1.5 minutes per level, max 5 levels)
    pub difficulty_timer: Timer,
    /// Chaos bonus accumulated during infinite mode (increases every 30s)
    /// This is separate from the global chaos tracker and only applies during infinite mode
    pub chaos_bonus: f32,
}

/// Maximum difficulty level in infinite mode
pub const MAX_DIFFICULTY_LEVEL: u8 = 5;
/// Seconds between difficulty increases
pub const DIFFICULTY_INCREASE_INTERVAL: f32 = 90.0; // 1.5 minutes

impl InfiniteMode {
    pub fn new() -> Self {
        Self {
            active: true,
            chaos_timer: Timer::from_seconds(30.0, TimerMode::Repeating),
            // Start at difficulty level 1 so mobs immediately get some enhancement
            difficulty_level: 1,
            difficulty_timer: Timer::from_seconds(
                DIFFICULTY_INCREASE_INTERVAL,
                TimerMode::Repeating,
            ),
            chaos_bonus: 0.0,
        }
    }

    /// Get the chaos bonus for mob scaling (only during infinite mode)
    pub fn get_chaos_bonus(&self) -> f32 {
        if self.active {
            self.chaos_bonus
        } else {
            0.0
        }
    }

    /// Get the speed multiplier for the current difficulty level
    /// Scales from 1.0 (level 0) to 2.5 (level 5) in 0.3 increments
    pub fn get_speed_multiplier(&self) -> f32 {
        1.0 + (self.difficulty_level as f32 * 0.3)
    }

    /// Get the red tint alpha for the current difficulty level
    /// Scales from 0.0 (level 0) to 1.0 (level 5) in 0.2 increments
    pub fn get_tint_alpha(&self) -> f32 {
        self.difficulty_level as f32 * 0.2
    }
}

/// Era timer - 12 minutes per era. Timer pauses in dungeons.
pub const ERA_TIMER_SECONDS: f32 = 12.0 * 60.0; // 12 minutes

/// Resource to track era timer
#[derive(Resource, Clone, Debug)]
pub struct EraTimer {
    pub remaining_seconds: f32,
    pub paused: bool,
}

impl Default for EraTimer {
    fn default() -> Self {
        Self {
            remaining_seconds: ERA_TIMER_SECONDS,
            paused: false,
        }
    }
}

impl EraTimer {
    pub fn reset(&mut self) {
        self.remaining_seconds = ERA_TIMER_SECONDS;
        self.paused = false;
    }

    pub fn get_display_string(&self) -> String {
        let minutes = (self.remaining_seconds / 60.0).floor() as u32;
        let seconds = (self.remaining_seconds % 60.0).floor() as u32;
        format!("{:02}:{:02}", minutes, seconds)
    }

    pub fn is_expired(&self) -> bool {
        self.remaining_seconds <= 0.0
    }
}

/// Marker component for mobs spawned during infinite mode
/// These mobs get red tint and 50% speed boost
#[derive(Component, Debug, Clone)]
pub struct InfiniteModeMob;

/// Event sent when infinite mode starts
#[derive(Default)]
pub struct InfiniteModeStartedEvent;

#[derive(Default, Reflect, Resource, Clone, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct NightTracker {
    pub days: u8,
    pub time: f32,
}
impl NightTracker {
    pub fn get_alpha(&self) -> f32 {
        if self.time < 6. {
            0.
        } else if self.time >= 6. && self.time <= 14. {
            return (self.time - 6.) * 0.09833333;
        } else {
            return 0.8 - (self.time - 14.) * 0.11666666;
        }
    }
    /// Returns the alpha for infinite mode (always dark)
    pub fn get_infinite_mode_alpha(&self) -> f32 {
        0.8 // Always full night darkness
    }
    pub fn is_night(&self) -> bool {
        // 12am to 6am ->
        self.time >= 15. && self.time <= 18.
    }
    pub fn is_start_of_new_day(&self) -> bool {
        self.time == 0.
    }
    pub fn get_hour(&self) -> u8 {
        self.time as u8
    }
}

pub struct NightPlugin;

#[derive(Default)]
pub struct NewDayEvent;

impl Plugin for NightPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<NightTracker>()
            .add_event::<NewDayEvent>()
            .add_event::<InfiniteModeStartedEvent>()
            .add_event::<EraTimerExpiredEvent>()
            .init_resource::<InfiniteMode>()
            .init_resource::<EraTimer>()
            // .add_plugin(ResourceInspectorPlugin::<NightTracker>::default().run_if(dim_spawned))
            .add_system(
                spawn_night
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            // Reset era timer and infinite mode when starting a new run
            .add_system(reset_era_timer_on_new_run.in_schedule(OnEnter(GameState::MainMenu)))
            .add_systems(
                (
                    tick_night_color.run_if(is_not_paused),
                    handle_infinite_mode_started,
                    tick_infinite_mode_chaos.run_if(is_not_paused),
                    tick_infinite_mode_difficulty.run_if(is_not_paused),
                    tick_era_timer.run_if(is_not_paused),
                    handle_era_timer_expired,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

/// Event sent when era timer expires
#[derive(Default)]
pub struct EraTimerExpiredEvent;

pub fn spawn_night(
    mut commands: Commands,
    night_tracker: Res<NightTracker>,
    res: Res<ScreenResolution>,
) {
    info!("Spawning night overlay");
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: overwrite_alpha(NIGHT, night_tracker.get_alpha()),
                custom_size: Some(Vec2::new(res.game_width, GAME_HEIGHT)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Night(Timer::from_seconds(9.5, TimerMode::Repeating)))
        .insert(Name::new("night"));
}

pub fn tick_night_color(
    time: Res<Time>,
    mut query: Query<(&mut Night, &mut Sprite)>,
    mut night_tracker: ResMut<NightTracker>,
    mut bgm_track_event: EventWriter<UpdateBGMTrackEvent>,
    bgm_tracker: Res<BGMPicker>,
    mut new_day_event: EventWriter<NewDayEvent>,
    infinite_mode: Res<InfiniteMode>,
) {
    // In infinite mode, keep it always night
    if infinite_mode.active {
        for (_night_state, mut sprite) in query.iter_mut() {
            sprite.color = overwrite_alpha(sprite.color, night_tracker.get_infinite_mode_alpha());
        }
        // Always play night music in infinite mode
        if bgm_tracker.current_track != *"sounds/bgm_night.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_night.ogg".to_owned(),
            });
        }
        return;
    }

    let mut music_changed = false;
    for (mut night_state, mut sprite) in query.iter_mut() {
        night_state.0.tick(time.delta());
        if night_state.0.finished() {
            night_tracker.time += 1.;
            sprite.color = overwrite_alpha(sprite.color, night_tracker.get_alpha());
            if night_tracker.time == 24. {
                night_tracker.days += 1;
                night_tracker.time = 0.;
            }
            if night_tracker.is_start_of_new_day() && night_tracker.days > 0 {
                new_day_event.send_default();
            }
            music_changed = true;
        }
    }

    if music_changed || night_tracker.is_added() {
        // change music
        if night_tracker.is_night() && bgm_tracker.current_track != *"sounds/bgm_night.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_night.ogg".to_owned(),
            });
        } else if !night_tracker.is_night() && bgm_tracker.current_track != *"sounds/bgm_day.ogg" {
            bgm_track_event.send(UpdateBGMTrackEvent {
                asset_path: "sounds/bgm_day.ogg".to_owned(),
            });
        }
    }
}

/// Handle infinite mode started event - set up infinite mode state
pub fn handle_infinite_mode_started(
    mut events: EventReader<InfiniteModeStartedEvent>,
    mut infinite_mode: ResMut<InfiniteMode>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            *infinite_mode = InfiniteMode::new();
            info!(
                "INFINITE MODE ACTIVATED! Starting at difficulty {}, speed {:.1}x, tint {:.1}. Chaos will increase every 30 seconds.",
                infinite_mode.difficulty_level,
                infinite_mode.get_speed_multiplier(),
                infinite_mode.get_tint_alpha()
            );
        }
    }
}

/// Tick the chaos timer in infinite mode and increase internal chaos bonus every 30 seconds
/// This chaos is stored in InfiniteMode and only applies during infinite mode (not carried to next era)
pub fn tick_infinite_mode_chaos(time: Res<Time>, mut infinite_mode: ResMut<InfiniteMode>) {
    if !infinite_mode.active {
        return;
    }

    infinite_mode.chaos_timer.tick(time.delta());
    if infinite_mode.chaos_timer.just_finished() {
        infinite_mode.chaos_bonus += 1.0;
        info!(
            "Infinite mode: Chaos bonus increased to {}!",
            infinite_mode.chaos_bonus
        );
    }
}

/// Tick the difficulty timer in infinite mode - increases difficulty every 2 minutes (up to 5 times)
pub fn tick_infinite_mode_difficulty(time: Res<Time>, mut infinite_mode: ResMut<InfiniteMode>) {
    if !infinite_mode.active {
        return;
    }

    // Already at max difficulty
    if infinite_mode.difficulty_level >= MAX_DIFFICULTY_LEVEL {
        return;
    }

    infinite_mode.difficulty_timer.tick(time.delta());
    if infinite_mode.difficulty_timer.just_finished() {
        infinite_mode.difficulty_level += 1;
        info!(
            "Infinite mode: Difficulty increased to level {}! Speed: {:.1}x, Tint: {:.1}",
            infinite_mode.difficulty_level,
            infinite_mode.get_speed_multiplier(),
            infinite_mode.get_tint_alpha()
        );
    }
}

/// Tick the era timer - pauses in dungeons, triggers infinite mode when expired
pub fn tick_era_timer(
    time: Res<Time>,
    mut era_timer: ResMut<EraTimer>,
    era_manager: Res<EraManager>,
    infinite_mode: Res<InfiniteMode>,
    mut expired_event: EventWriter<EraTimerExpiredEvent>,
) {
    // Don't tick if infinite mode is already active
    if infinite_mode.active {
        return;
    }

    // Pause timer in dungeons
    if era_manager.current_era.is_dungeon() {
        return;
    }

    // Tick the timer
    era_timer.remaining_seconds -= time.delta_seconds();

    // Check if expired
    if era_timer.is_expired() {
        era_timer.remaining_seconds = 0.0; // Clamp to 0
        expired_event.send_default();
    }
}

/// Handle era timer expiration - trigger infinite mode
pub fn handle_era_timer_expired(
    mut events: EventReader<EraTimerExpiredEvent>,
    mut infinite_mode_event: EventWriter<InfiniteModeStartedEvent>,
    infinite_mode: Res<InfiniteMode>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            info!("ERA TIMER EXPIRED! Entering infinite mode!");
            infinite_mode_event.send_default();
        }
    }
}

/// Reset era timer and infinite mode when entering a new era (called from dimension.rs)
pub fn reset_era_timer_and_infinite_mode(
    era_timer: &mut EraTimer,
    infinite_mode: &mut InfiniteMode,
) {
    era_timer.reset();

    // Also reset infinite mode when entering a new era
    infinite_mode.active = false;
    infinite_mode.difficulty_level = 0;
    infinite_mode.chaos_timer = Timer::from_seconds(30.0, TimerMode::Repeating);
    infinite_mode.difficulty_timer =
        Timer::from_seconds(DIFFICULTY_INCREASE_INTERVAL, TimerMode::Repeating);
    infinite_mode.chaos_bonus = 0.0;

    info!(
        "Era timer reset to {:02}:{:02}, infinite mode deactivated",
        (ERA_TIMER_SECONDS / 60.0) as u32,
        (ERA_TIMER_SECONDS % 60.0) as u32
    );
}

/// Reset era timer and infinite mode when starting a new run
fn reset_era_timer_on_new_run(
    mut era_timer: ResMut<EraTimer>,
    mut infinite_mode: ResMut<InfiniteMode>,
) {
    // Reset era timer to full 12 minutes
    era_timer.reset();

    // Reset infinite mode completely
    infinite_mode.active = false;
    infinite_mode.chaos_timer = Timer::from_seconds(30.0, TimerMode::Repeating);
    infinite_mode.difficulty_level = 0;
    infinite_mode.difficulty_timer =
        Timer::from_seconds(DIFFICULTY_INCREASE_INTERVAL, TimerMode::Repeating);
    infinite_mode.chaos_bonus = 0.0;

    info!("Era timer and infinite mode reset for new run");
}
