use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::Rng;

use crate::{
    chaos::{ChaosTracker, EraTransitionState},
    client::is_not_paused,
    custom_commands::CommandsExt,
    night::{InfiniteMode, InfiniteModeMob, NightTracker},
    player::Player,
    proto::proto_param::ProtoParam,
    run_once_per_run,
    world::{
        dimension::{ActiveDimension, DimensionSpawnEvent, Era, EraManager},
        dungeon::Dungeon,
        TILE_SIZE,
    },
    GameParam, GameState, DEBUG, NO_SPAWN,
};

use super::{spawn_helpers::can_spawn_mob_here, CombatAlignment, EliteMob, Mob};

pub const BASE_MAX_MOBS_TOTAL: i32 = 60;
pub const INFINITE_MAX_MOBS_BONUS: i32 = 100;
pub const ELITE_SPAWN_RATE: f32 = 0.06;

/// Tracks which era the current [`GlobalSpawners::spawners`] list was built for (overworld only).
#[derive(Resource, Default, Debug)]
pub struct SpawnerListEraTracker {
    pub era_at_last_build: Option<Era>,
    /// Whether the currently-built spawner list is the endless-mode list.
    pub endless_at_last_build: bool,
}

/// Overworld mob spawner rows for the given era (weights, timers, min day, batch size).  
/// Dungeon and other dimensions do not use this list — keep [`Era::DungeonMain`] mapping empty.
pub fn overworld_spawners_for_era(era: Era) -> Vec<Spawner> {
    match era {
        Era::Main => spawners_era_main(),
        Era::Second => spawners_era_second(),
        Era::Third => spawners_era_third(),
        Era::DungeonMain => vec![],
    }
}

fn spawners_era_main() -> Vec<Spawner> {
    vec![
        Spawner {
            enemy: Mob::SpikeSlime,
            weight: 100.,
            spawn_timer: Timer::from_seconds(20., TimerMode::Once),
            min_days_to_spawn: 2,
            num_to_spawn: Some(1),
        },
        Spawner {
            enemy: Mob::FurDevil,
            weight: 100.,
            spawn_timer: Timer::from_seconds(2., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(1),
        },
        Spawner {
            enemy: Mob::RedMushling,
            weight: 200.,
            spawn_timer: Timer::from_seconds(25., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: None,
        },
        Spawner {
            enemy: Mob::StingFly,
            weight: 100.,
            spawn_timer: Timer::from_seconds(14., TimerMode::Once),
            min_days_to_spawn: 1,
            num_to_spawn: Some(2),
        },
        Spawner {
            enemy: Mob::Bushling,
            weight: 100.,
            spawn_timer: Timer::from_seconds(7., TimerMode::Once),
            min_days_to_spawn: 1,
            num_to_spawn: Some(1),
        },
    ]
}

fn spawners_era_second() -> Vec<Spawner> {
    vec![
        Spawner {
            enemy: Mob::Lizard,
            weight: 60.,
            spawn_timer: Timer::from_seconds(2., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(1),
        },
        Spawner {
            enemy: Mob::SmallCactus,
            weight: 100.,
            spawn_timer: Timer::from_seconds(3., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(1),
        },
        Spawner {
            enemy: Mob::BigCactus,
            weight: 90.,
            spawn_timer: Timer::from_seconds(8., TimerMode::Once),
            min_days_to_spawn: 1,
            num_to_spawn: Some(1),
        },
        Spawner {
            enemy: Mob::Bull,
            weight: 110.,
            spawn_timer: Timer::from_seconds(17., TimerMode::Once),
            min_days_to_spawn: 2,
            num_to_spawn: Some(1),
        },
    ]
}

/// Era 3: tune separately when ready; currently matches era 2 pool cadence.
fn spawners_era_third() -> Vec<Spawner> {
    spawners_era_main()
}

/// Seconds of [`InfiniteMode::elapsed_seconds`] before Void Worms can spawn in endless.
const VOID_WORM_ENDLESS_GATE_SECS: f32 = 240.0;

/// Endless mode mob pool. Replaces normal era enemies while [`InfiniteMode::active`].
fn spawners_endless() -> Vec<Spawner> {
    vec![
        Spawner {
            enemy: Mob::VoidCrawler,
            weight: 100.,
            spawn_timer: Timer::from_seconds(1.2, TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(1),
        },
        Spawner {
            enemy: Mob::VoidWorm,
            weight: 100.,
            spawn_timer: Timer::from_seconds(15., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(3),
        },
    ]
}

/// Resource to track if mob spawning should be paused (e.g., after boss defeat with time remaining)
#[derive(Resource, Default, Debug)]
pub struct MobSpawningPaused {
    pub paused: bool,
}

/// Global timer for despawning distant mobs when at cap. When mob count >= max, every 7s we despawn the 5 furthest mobs.
#[derive(Resource)]
pub struct EnemyDespawnTimer {
    pub timer: Timer,
}

impl Default for EnemyDespawnTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(3.0, TimerMode::Repeating),
        }
    }
}

pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>()
            .init_resource::<MobSpawningPaused>()
            .init_resource::<EnemyDespawnTimer>()
            .init_resource::<SpawnerListEraTracker>()
            .add_systems(
                (
                    sync_overworld_spawners_with_era,
                    handle_spawn_mobs,
                    tick_spawner_timers.run_if(is_not_paused),
                    tick_enemy_despawn_timer.run_if(is_not_paused),
                    // handle_add_fairy_spawners,
                    test_mob_count,
                    spawn_stone_golem_timer.run_if(is_not_paused),
                    spawn_endless_stone_golem_timer.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(reset_stone_golem_timer_on_era_change)
            .add_system(
                add_spawners_to_new_chunks
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            // Initialize Stone Golem timer as a fresh resource when entering Main Menu
            // This ensures it resets between runs
            .add_system(initialize_stone_golem_timer.in_schedule(OnEnter(GameState::MainMenu)));
    }
}

#[derive(Clone, Debug, Default)]

pub struct Spawner {
    // pub radius: u32,
    pub weight: f32,
    pub spawn_timer: Timer,
    pub min_days_to_spawn: u8,
    pub enemy: Mob,
    pub num_to_spawn: Option<u32>,
}
impl PartialEq for Spawner {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
            && self.min_days_to_spawn == other.min_days_to_spawn
            && self.enemy == other.enemy
    }
}
#[derive(Resource, Debug)]
pub struct GlobalSpawners {
    pub spawners: Vec<Spawner>,
    pub initial_spawn_delay: Timer,
}

#[derive(Debug)]
pub struct MobSpawnEvent {
    mob: Mob,
    bypass_timers: bool,
}

fn test_mob_count(q: Query<&Mob>, key_input: Res<Input<KeyCode>>) {
    if *DEBUG && key_input.just_pressed(KeyCode::G) {
        info!(
            "Fur Devils: {:?}",
            q.iter().filter(|m| m == &&Mob::FurDevil).count()
        );
        info!(
            "Bushlings: {:?}",
            q.iter().filter(|m| m == &&Mob::Bushling).count()
        );
        info!(
            "Stingflys: {:?}",
            q.iter().filter(|m| m == &&Mob::StingFly).count()
        );
        info!(
            "SpikeSlimes: {:?}",
            q.iter().filter(|m| m == &&Mob::SpikeSlime).count()
        );
        info!(
            "Mushlings: {:?}",
            q.iter().filter(|m| m == &&Mob::RedMushling).count()
        );
    }
}

fn add_spawners_to_new_chunks(
    mut commands: Commands,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    game: GameParam,
) {
    let era = game.era.current_era.clone();
    let spawners = if maybe_dungeon.get_single().is_err() && !era.is_dungeon() {
        overworld_spawners_for_era(era.clone())
    } else {
        vec![]
    };
    let era_at_last_build = if spawners.is_empty() { None } else { Some(era) };
    commands.insert_resource(GlobalSpawners {
        spawners,
        initial_spawn_delay: Timer::from_seconds(5., TimerMode::Once),
    });
    commands.insert_resource(SpawnerListEraTracker {
        era_at_last_build,
        endless_at_last_build: false,
    });
}

/// Rebuild overworld spawner table when [`EraManager::current_era`] changes (not in dungeon).
fn sync_overworld_spawners_with_era(
    game: GameParam,
    mut spawners: ResMut<GlobalSpawners>,
    mut tracker: ResMut<SpawnerListEraTracker>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    infinite_mode: Res<InfiniteMode>,
) {
    if maybe_dungeon.get_single().is_ok() {
        return;
    }
    let era = game.era.current_era.clone();
    if era.is_dungeon() {
        return;
    }

    // Endless mode rework: swap the entire spawner list for the Void Crawler-only
    // endless pool while infinite mode is active, and restore the era pool when it ends.
    if infinite_mode.active {
        if tracker.endless_at_last_build {
            return;
        }
        tracker.endless_at_last_build = true;
        tracker.era_at_last_build = None;
        spawners.spawners = spawners_endless();
        return;
    }

    if !tracker.endless_at_last_build && tracker.era_at_last_build.as_ref() == Some(&era) {
        return;
    }
    tracker.endless_at_last_build = false;
    tracker.era_at_last_build = Some(era.clone());
    spawners.spawners = overworld_spawners_for_era(era);
}

fn handle_spawn_mobs(
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    prototypes: Prototypes,
    mut spawner_trigger_event: EventReader<MobSpawnEvent>,
    proto_param: ProtoParam,
    player_t: Query<&GlobalTransform, With<Player>>,
    mut spawners: ResMut<GlobalSpawners>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    infinite_mode: Res<InfiniteMode>,
    mob_spawning_paused: Res<MobSpawningPaused>,
) {
    if *NO_SPAWN {
        return;
    }
    if maybe_dungeon.get_single().is_ok() {
        return;
    }
    if mob_spawning_paused.paused {
        return;
    }
    'outer: for e in spawner_trigger_event.iter() {
        let mut rng = rand::thread_rng();
        let maybe_spawner = spawners.spawners.iter_mut().find(|s| s.enemy == e.mob);
        let mut picked_mob_to_spawn = None;
        if let Some(mut chunk_spawner) = maybe_spawner {
            let player_pos = player_t.single().translation().truncate();
            let mut pos = player_pos;
            let mut can_spawn_mob_here_check = false;
            let mut fallback_attempts = 10;
            let max_range: f32 = TILE_SIZE.x * 28.;

            while pos.distance(player_pos) <= TILE_SIZE.x * 10. || !can_spawn_mob_here_check {
                let spawn_pos_delta: Vec2 = Vec2::new(
                    rng.gen_range(-max_range / 2. ..max_range / 2.),
                    rng.gen_range(-max_range / 2. ..max_range / 2.),
                );
                pos = player_pos + spawn_pos_delta;
                can_spawn_mob_here_check = true; //can_spawn_mob_here(pos, &game, &proto_param, false);
                fallback_attempts -= 1;
                if fallback_attempts <= 0 {
                    info!("skip spawn: cant find a valid spawn location {:?}", pos);

                    continue 'outer;
                }
            }
            picked_mob_to_spawn = Some((e.mob.clone(), pos));
        }
        if let Some((mob, pos)) = picked_mob_to_spawn {
            if let Some(spawned_mob) =
                proto_commands.spawn_from_proto(mob.clone(), &prototypes, pos)
            {
                debug!("SPAWNED A MOB!!! {spawned_mob:?}");
                let can_be_elite = proto_param
                    .get_component::<CombatAlignment, _>(mob.clone())
                    .map(|a| a != &CombatAlignment::Passive)
                    .unwrap_or(false);
                if rng.gen::<f32>() < ELITE_SPAWN_RATE && can_be_elite {
                    commands.entity(spawned_mob).insert(EliteMob);
                }

                // Mark mobs spawned during infinite mode for red tint and speed boost
                if infinite_mode.active {
                    commands.entity(spawned_mob).insert(InfiniteModeMob);
                }
            }
        }
    }
}

/// Resource to track Stone Golem spawn timer
#[derive(Resource)]
pub struct StoneGolemSpawnTimer {
    pub timer: Timer,
}

impl Default for StoneGolemSpawnTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(240.0, TimerMode::Once), // 4 minutes (base mode)
        }
    }
}

/// Resource to track Stone Golem spawn timer specifically for endless mode.
#[derive(Resource)]
pub struct EndlessStoneGolemSpawnTimer {
    pub timer: Timer,
}

impl Default for EndlessStoneGolemSpawnTimer {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(480.0, TimerMode::Repeating), // 8 minutes
        }
    }
}

/// Initialize Stone Golem spawn timer as a fresh resource
/// Called when entering Main Menu to ensure it resets between runs
fn initialize_stone_golem_timer(mut commands: Commands) {
    commands.insert_resource(StoneGolemSpawnTimer::default());
    commands.insert_resource(EndlessStoneGolemSpawnTimer::default());
    info!("Stone Golem spawn timer initialized");
}

#[derive(Clone, Copy)]
enum StoneGolemSpawnMode {
    Base,
    Endless,
}

impl StoneGolemSpawnMode {
    fn tag_infinite_mode(self) -> bool {
        matches!(self, Self::Endless)
    }
}

fn stone_golem_spawn_blocked_by_existing(existing_golems: &Query<&Mob>) -> bool {
    existing_golems.iter().any(|m| m == &Mob::StoneGolem)
}

fn pick_valid_stone_golem_spawn_near_player(
    player_query: &Query<&GlobalTransform, With<Player>>,
    game: &GameParam,
    proto_param: &ProtoParam,
) -> Option<Vec2> {
    let player_txfm = player_query.get_single().ok()?;
    let player_pos = player_txfm.translation().truncate();
    let mut rng = rand::thread_rng();
    let mut pos = player_pos;
    let mut attempts = 20usize;
    let spawn_distance = TILE_SIZE.x * 12.0;

    while attempts > 0 {
        let angle = rng.gen_range(0.0..std::f32::consts::TAU);
        let offset = Vec2::new(angle.cos(), angle.sin()) * spawn_distance;
        pos = player_pos + offset;

        if can_spawn_mob_here(pos, game, proto_param, false) {
            return Some(pos);
        }
        attempts -= 1;
    }
    None
}

fn try_spawn_stone_golem(
    mode: StoneGolemSpawnMode,
    proto_commands: &mut ProtoCommands,
    prototypes: &Prototypes,
    commands: &mut Commands,
    player_query: &Query<&GlobalTransform, With<Player>>,
    game: &GameParam,
    proto_param: &ProtoParam,
    existing_golems: &Query<&Mob>,
) {
    if stone_golem_spawn_blocked_by_existing(existing_golems) {
        return;
    }
    let Some(pos) = pick_valid_stone_golem_spawn_near_player(player_query, game, proto_param)
    else {
        return;
    };
    if let Some(spawned) = proto_commands.spawn_from_proto(Mob::StoneGolem, prototypes, pos) {
        if mode.tag_infinite_mode() {
            commands.entity(spawned).insert(InfiniteModeMob);
        }
    }
}

/// System to spawn Stone Golem in base mode cadence (4 minutes, one-time per era)
fn spawn_stone_golem_timer(
    time: Res<Time>,
    golem_timer: Option<ResMut<StoneGolemSpawnTimer>>,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    proto_param: ProtoParam,
    player_query: Query<&GlobalTransform, With<Player>>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    game: GameParam,
    infinite_mode: Res<InfiniteMode>,
    existing_golems: Query<&Mob>,
    mut commands: Commands,
) {
    if *NO_SPAWN {
        return;
    }
    // Check if in dungeon - don't spawn in dungeons
    if maybe_dungeon.get_single().is_ok() {
        return;
    }

    let Some(mut golem_timer) = golem_timer else {
        return;
    };

    // Base-mode golem logic should not run during endless mode.
    if infinite_mode.active {
        return;
    }

    // Tick the timer
    golem_timer.timer.tick(time.delta());

    // Spawn golem when timer finishes
    if golem_timer.timer.just_finished() {
        try_spawn_stone_golem(
            StoneGolemSpawnMode::Base,
            &mut proto_commands,
            &prototypes,
            &mut commands,
            &player_query,
            &game,
            &proto_param,
            &existing_golems,
        );
    }
}

/// System to spawn Stone Golem every 8 minutes during endless mode only.
fn spawn_endless_stone_golem_timer(
    time: Res<Time>,
    endless_golem_timer: Option<ResMut<EndlessStoneGolemSpawnTimer>>,
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    proto_param: ProtoParam,
    player_query: Query<&GlobalTransform, With<Player>>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    game: GameParam,
    infinite_mode: Res<InfiniteMode>,
    existing_golems: Query<&Mob>,
) {
    if *NO_SPAWN {
        return;
    }
    if maybe_dungeon.get_single().is_ok() {
        return;
    }

    let Some(mut endless_golem_timer) = endless_golem_timer else {
        return;
    };

    // Only tick while in endless mode; reset while inactive so endless gets a fresh 8-minute timer.
    if !infinite_mode.active {
        if endless_golem_timer.timer.elapsed_secs() > 0.0 {
            endless_golem_timer.timer.reset();
        }
        return;
    }

    endless_golem_timer.timer.tick(time.delta());
    if !endless_golem_timer.timer.just_finished() {
        return;
    }

    try_spawn_stone_golem(
        StoneGolemSpawnMode::Endless,
        &mut proto_commands,
        &prototypes,
        &mut commands,
        &player_query,
        &game,
        &proto_param,
        &existing_golems,
    );
}

/// Reset Stone Golem spawn timer when advancing to a new overworld era.
/// Dungeon entry/exit within the same era must not reset the timer.
fn reset_stone_golem_timer_on_era_change(
    mut golem_timer: Option<ResMut<StoneGolemSpawnTimer>>,
    mut endless_golem_timer: Option<ResMut<EndlessStoneGolemSpawnTimer>>,
    mut dimension_spawn_events: EventReader<DimensionSpawnEvent>,
    era: Option<Res<EraManager>>,
) {
    let Some(era) = era else {
        return;
    };
    for event in dimension_spawn_events.iter() {
        let Some(new_era) = &event.new_era else {
            continue;
        };
        if new_era.is_dungeon() || era.current_era.is_dungeon() {
            continue;
        }
        if era.current_era == *new_era {
            continue;
        }
        if let Some(ref mut timer) = golem_timer {
            timer.timer.reset();
        }
        if let Some(ref mut timer) = endless_golem_timer {
            timer.timer.reset();
        }
        info!(
            "Stone Golem spawn timers reset due to era change {:?} -> {:?}",
            era.current_era, new_era
        );
    }
}

fn tick_spawner_timers(
    time: Res<Time>,
    mut spawners: ResMut<GlobalSpawners>,
    night_tracker: Res<NightTracker>,
    infinite_mode: Res<InfiniteMode>,
    mut spawn_event: EventWriter<MobSpawnEvent>,
    mobs: Query<&Mob>,
    chaos_tracker: Res<ChaosTracker>,
    mob_spawning_paused: Res<MobSpawningPaused>,
    transition_state: Res<EraTransitionState>,
) {
    if *NO_SPAWN {
        return;
    }
    if !spawners.initial_spawn_delay.finished() {
        spawners.initial_spawn_delay.tick(time.delta());
        return;
    }
    if mob_spawning_paused.paused {
        return;
    }
    // for each spawned chunk, check if mob count is < max
    // and if so, send event to spawn more
    let mob_count = mobs
        .iter()
        .filter(|m| m != &&Mob::RedMushling && m != &&Mob::Hog && m != &&Mob::Fairy)
        .count() as i32;

    // In endless mode use base cap (gauge damage output, not mobbing); otherwise scale with days
    let max_mobs = if infinite_mode.active {
        INFINITE_MAX_MOBS_BONUS + BASE_MAX_MOBS_TOTAL + night_tracker.days as i32 * 10
    } else {
        BASE_MAX_MOBS_TOTAL + night_tracker.days as i32 * 10
    };

    if mob_count >= max_mobs {
        info!(
            "MAX MOBS {:?} {:?} {:?}",
            mob_count,
            max_mobs,
            mobs.iter().count()
        );
        return;
    }
    let chaos = chaos_tracker.get_chaos();
    let bonus_spawn_count_chaos = if infinite_mode.active {
        0
    } else if night_tracker.is_night() {
        (chaos / 3.).floor() as u32
    } else {
        (chaos / 5.).floor() as u32
    };
    let day = night_tracker.days;
    // Endless: no extra spawns per tick (base spawn rates only)
    let endless_mode_spawn_count_increase = 2u32;
    for spawner in spawners.spawners.iter_mut() {
        debug!("spawner check: {:?} {:?}", spawner.min_days_to_spawn, day);
        if day < spawner.min_days_to_spawn {
            continue;
        }

        // Check if this mob is unlocked in the current era transition
        if !transition_state.is_mob_unlocked(&spawner.enemy) {
            continue;
        }

        // Void Worms share the normal spawn pipeline but gate on endless elapsed
        // time and use their configured timer as-is (no endless spawn-rate boost).
        if spawner.enemy == Mob::VoidWorm {
            if !infinite_mode.active || infinite_mode.elapsed_seconds < VOID_WORM_ENDLESS_GATE_SECS
            {
                continue;
            }
            spawner.spawn_timer.tick(time.delta());
            if spawner.spawn_timer.finished() {
                spawner.spawn_timer.reset();
                spawn_event.send(MobSpawnEvent {
                    mob: Mob::VoidWorm,
                    bypass_timers: false,
                });
            }
            continue;
        }

        spawner.spawn_timer.tick(time.delta());

        // Speed up spawns during night or endless mode
        if night_tracker.is_night() || infinite_mode.active {
            // 3x spawn rate at night
            spawner.spawn_timer.tick(time.delta());
            spawner.spawn_timer.tick(time.delta());
            spawner.spawn_timer.tick(time.delta());
            if infinite_mode.active {
                for _ in 0..5 {
                    spawner.spawn_timer.tick(time.delta());
                }
            }
        }
        if spawner.spawn_timer.finished() {
            spawner.spawn_timer.reset();
            let num_to_spawn = if spawner.enemy == Mob::RedMushling {
                1
            } else {
                spawner.num_to_spawn.unwrap_or(1)
                    + bonus_spawn_count_chaos
                    + endless_mode_spawn_count_increase as u32
            };
            for _ in 0..num_to_spawn {
                spawn_event.send(MobSpawnEvent {
                    mob: spawner.enemy.clone(),
                    bypass_timers: false,
                });
            }
        }
    }
}

/// When at or over the mob cap, every 7s despawn the 5 mobs furthest from the player.
fn tick_enemy_despawn_timer(
    time: Res<Time>,
    mut despawn_timer: ResMut<EnemyDespawnTimer>,
    mut commands: Commands,
    mobs: Query<(Entity, &GlobalTransform, &Mob)>,
    player_t: Query<&GlobalTransform, With<Player>>,
    night_tracker: Res<NightTracker>,
    infinite_mode: Res<InfiniteMode>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
) {
    let NUM_TO_DESPAWN: usize = if infinite_mode.active { 20 } else { 10 };
    let NUM_TO_SKIP: usize = 20;
    if maybe_dungeon.get_single().is_ok() {
        return;
    }
    despawn_timer.timer.tick(time.delta());
    if !despawn_timer.timer.just_finished() {
        return;
    }
    let max_mobs = if infinite_mode.active {
        INFINITE_MAX_MOBS_BONUS + BASE_MAX_MOBS_TOTAL + night_tracker.days as i32 * 10
    } else {
        BASE_MAX_MOBS_TOTAL + night_tracker.days as i32 * 10
    };
    let player_pos = match player_t.get_single() {
        Ok(t) => t.translation().truncate(),
        Err(_) => return,
    };
    let mut eligible: Vec<(Entity, f32)> = mobs
        .iter()
        .filter(|(_, _, m)| {
            m != &&Mob::RedMushling && m != &&Mob::Hog && m != &&Mob::Fairy && !m.is_boss()
        })
        .map(|(e, t, _)| {
            let dist = t.translation().truncate().distance(player_pos);
            (e, dist)
        })
        .collect();
    let count = eligible.len() as i32;
    if count < max_mobs {
        return;
    }
    eligible.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    for (entity, _) in eligible.into_iter().skip(NUM_TO_SKIP).take(NUM_TO_DESPAWN) {
        commands.entity(entity).despawn_recursive();
    }
}
