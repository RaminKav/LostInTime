use bevy::{prelude::*, render::view::RenderLayers};
use serde::{Deserialize, Serialize};

use crate::{
    audio::{BGMPicker, UpdateBGMTrackEvent},
    chaos::ChaosTracker,
    client::is_not_paused,
    colors::{overwrite_alpha, NIGHT},
    enemy::spawner::MobSpawningPaused,
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
    /// Elapsed time in endless mode (seconds)
    pub elapsed_seconds: f32,
}

/// Maximum difficulty level in infinite mode (Tier 1: 1-5, Tier 2: 6-10)
pub const MAX_DIFFICULTY_LEVEL: u8 = 10;
/// Seconds between difficulty increases
pub const DIFFICULTY_INCREASE_INTERVAL: f32 = 90.0; // 1.5 minutes
/// Seconds between chaos threshold increases in endless mode (noticeable jumps every 60-90s)
pub const CHAOS_THRESHOLD_INTERVAL: f32 = 75.0; // 75 seconds for noticeable difficulty ramps
const CHAOS_TIMER_SECONDS: f32 = CHAOS_THRESHOLD_INTERVAL;

/// Era timer - 10 minutes per era. Timer pauses in dungeons.
pub const ERA_TIMER_SECONDS: f32 = 11.0 * 60.0; // 10 minutes

impl InfiniteMode {
    pub fn new() -> Self {
        Self {
            active: true,
            chaos_timer: Timer::from_seconds(CHAOS_TIMER_SECONDS, TimerMode::Repeating),
            // Start at difficulty level 1 so mobs immediately get some enhancement
            difficulty_level: 1,
            difficulty_timer: Timer::from_seconds(
                DIFFICULTY_INCREASE_INTERVAL,
                TimerMode::Repeating,
            ),
            chaos_bonus: 0.0,
            elapsed_seconds: 0.0,
        }
    }

    /// Get the elapsed time formatted as MM:SS
    pub fn get_elapsed_display_string(&self) -> String {
        let minutes = (self.elapsed_seconds / 60.0) as u32;
        let seconds = (self.elapsed_seconds % 60.0) as u32;
        format!("{:02}:{:02}", minutes, seconds)
    }

    /// Get the chaos bonus for mob scaling (only during infinite mode)
    /// Tier 1: 1x multiplier, Tier 2: 3x multiplier (triple chaos)
    pub fn get_chaos_bonus(&self) -> f32 {
        if self.active {
            self.chaos_bonus * self.get_chaos_multiplier()
        } else {
            0.0
        }
    }

    /// Get which tier the current difficulty level is in
    /// Tier 1: levels 1-5, Tier 2: levels 6-10
    pub fn get_tier(&self) -> u8 {
        if self.difficulty_level <= 5 {
            1
        } else {
            2
        }
    }

    /// Get the speed multiplier for the current difficulty level
    /// Tier 1 (1-5): 50% speed per level (1.5x to 3.5x)
    /// Tier 2 (6-10): 70% speed per level (4.2x to 7.7x)
    pub fn get_speed_multiplier(&self) -> f32 {
        if self.difficulty_level <= 5 {
            // Tier 1: 50% per level
            1.0 + (self.difficulty_level as f32 * 0.5)
        } else {
            // Tier 2: 70% per level, starting from tier 1 max (3.5x at level 5)
            let tier1_max = 1.0 + (5.0 * 0.5); // 3.5x
            let tier2_levels = self.difficulty_level - 5;
            tier1_max + (tier2_levels as f32 * 0.7)
        }
    }

    /// Get the tint alpha for the current difficulty level
    /// Tier 1 (1-5): Purple tint from 0.2 to 1.0 alpha
    /// Tier 2 (6-10): Red tint always at 1.0 alpha
    pub fn get_tint_alpha(&self) -> f32 {
        if self.difficulty_level <= 5 {
            // Tier 1: Purple tint from 0.2 (level 1) to 1.0 (level 5)
            // Range is 0.8 over 4 intervals (level 1->2, 2->3, 3->4, 4->5)
            let base_alpha = 0.2;
            let alpha_range = 1.0 - base_alpha; // 0.8
            let intervals = 4.0; // 4 intervals between levels 1-5
            if self.difficulty_level == 0 {
                base_alpha
            } else {
                base_alpha + (alpha_range / intervals * (self.difficulty_level - 1) as f32)
            }
        } else {
            // Tier 2: Always 1.0 alpha for red tint
            1.0
        }
    }

    /// Get the chaos bonus multiplier for the current tier
    /// Tier 1: 1x (default), Tier 2: 3x (triple)
    pub fn get_chaos_multiplier(&self) -> f32 {
        if self.difficulty_level <= 5 {
            1.0
        } else {
            2.0
        }
    }
}

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
    mut chaos_tracker: ResMut<ChaosTracker>,
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
                chaos_tracker.add_chaos(1.);
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
    world_obj_cache: Res<crate::world::generation::WorldObjectCache>,
    pathfinding_cache: Res<crate::ai::pathfinding::PathfindingCache>,
    chunk_query: Query<&crate::world::chunk::Chunk>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            *infinite_mode = InfiniteMode::new();
            let tier = infinite_mode.get_tier();
            let tier_name = if tier == 1 { "Purple" } else { "Red" };
            info!(
                "INFINITE MODE ACTIVATED! Starting at difficulty {} (Tier {} - {}), speed {:.1}x, tint {:.1}. Chaos will increase every 30 seconds.",
                infinite_mode.difficulty_level,
                tier,
                tier_name,
                infinite_mode.get_speed_multiplier(),
                infinite_mode.get_tint_alpha()
            );

            // Debug: Log cache sizes to identify performance issues
            info!(
                "DEBUG: WorldObjectCache sizes - objects: {}, unique_objs: {}, dungeon_objects: {}, generated_chunks: {}, tile_data_cache: {}",
                world_obj_cache.objects.len(),
                world_obj_cache.unique_objs.len(),
                world_obj_cache.dungeon_objects.len(),
                world_obj_cache.generated_chunks.len(),
                world_obj_cache.tile_data_cache.len()
            );
            info!(
                "DEBUG: PathfindingCache size - tile_valid_cache: {}",
                pathfinding_cache.tile_valid_cache.len()
            );
            info!("DEBUG: Active chunks count: {}", chunk_query.iter().count());
        }
    }
}

/// Tick the chaos timer in infinite mode and increase internal chaos bonus at thresholds
/// This chaos is stored in InfiniteMode and only applies during infinite mode (not carried to next era)
/// Uses exponential growth per threshold to create noticeable difficulty jumps every 75s
pub fn tick_infinite_mode_chaos(time: Res<Time>, mut infinite_mode: ResMut<InfiniteMode>) {
    if !infinite_mode.active {
        return;
    }

    // Tick elapsed time
    infinite_mode.elapsed_seconds += time.delta_seconds();

    infinite_mode.chaos_timer.tick(time.delta());
    if infinite_mode.chaos_timer.just_finished() {
        // Calculate which threshold we're at (0-indexed)
        let threshold_number =
            (infinite_mode.elapsed_seconds / CHAOS_THRESHOLD_INTERVAL).floor() as u32;

        let base_chaos_per_threshold = 3.0;
        let growth_rate = 1.15_f32;
        let chaos_to_add = base_chaos_per_threshold * growth_rate.powf(threshold_number as f32);

        infinite_mode.chaos_bonus += chaos_to_add;
        info!(
            "Infinite mode: Chaos bonus increased by {:.1} to {:.1}! (Threshold {}, {:.0}s elapsed)",
            chaos_to_add, infinite_mode.chaos_bonus, threshold_number, infinite_mode.elapsed_seconds
        );
    }
}

/// Tick the difficulty timer in infinite mode - increases difficulty every 1.5 minutes (up to level 10)
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
        let tier = infinite_mode.get_tier();
        let tier_name = if tier == 1 { "Purple" } else { "Red" };
        info!(
            "Infinite mode: Difficulty increased to level {} (Tier {} - {})! Speed: {:.1}x, Tint: {:.1}",
            infinite_mode.difficulty_level,
            tier,
            tier_name,
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
    mut mob_spawning_paused: ResMut<crate::enemy::spawner::MobSpawningPaused>,
) {
    for _ in events.iter() {
        if !infinite_mode.active {
            info!("ERA TIMER EXPIRED! Entering infinite mode!");
            infinite_mode_event.send_default();
        }
        // Resume mob spawning when timer expires
        if mob_spawning_paused.paused {
            info!("Era timer expired - resuming mob spawning");
            mob_spawning_paused.paused = false;
        }
    }
}

/// Reset era timer and infinite mode when entering a new era (called from dimension.rs)
pub fn reset_era_timer_and_infinite_mode(
    era_timer: &mut EraTimer,
    infinite_mode: &mut InfiniteMode,
    mob_spawning_paused: &mut MobSpawningPaused,
) {
    era_timer.reset();

    infinite_mode.active = false;
    infinite_mode.difficulty_level = 0;
    infinite_mode.chaos_timer = Timer::from_seconds(CHAOS_TIMER_SECONDS, TimerMode::Repeating);
    infinite_mode.difficulty_timer =
        Timer::from_seconds(DIFFICULTY_INCREASE_INTERVAL, TimerMode::Repeating);
    infinite_mode.chaos_bonus = 0.0;
    infinite_mode.elapsed_seconds = 0.0;

    if mob_spawning_paused.paused {
        info!("Era changed - resuming mob spawning");
        mob_spawning_paused.paused = false;
    }

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
    mut mob_spawning_paused: ResMut<MobSpawningPaused>,
) {
    // Reset era timer to full 12 minutes
    era_timer.reset();

    // Reset infinite mode completely
    infinite_mode.active = false;
    infinite_mode.chaos_timer = Timer::from_seconds(CHAOS_TIMER_SECONDS, TimerMode::Repeating);
    infinite_mode.difficulty_level = 0;
    infinite_mode.difficulty_timer =
        Timer::from_seconds(DIFFICULTY_INCREASE_INTERVAL, TimerMode::Repeating);
    infinite_mode.chaos_bonus = 0.0;
    infinite_mode.elapsed_seconds = 0.0;

    // Reset mob spawning paused state
    mob_spawning_paused.paused = false;

    info!("Era timer and infinite mode reset for new run");
}
