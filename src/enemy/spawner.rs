use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::Rng;

use crate::{
    chaos::ChaosTracker,
    client::is_not_paused,
    custom_commands::CommandsExt,
    night::{InfiniteMode, InfiniteModeMob, NightTracker},
    player::Player,
    proto::proto_param::ProtoParam,
    run_once_per_run,
    world::{
        dimension::{ActiveDimension, DimensionSpawnEvent},
        dungeon::Dungeon,
        TILE_SIZE,
    },
    GameParam, GameState, DEBUG,
};

use super::{spawn_helpers::can_spawn_mob_here, CombatAlignment, EliteMob, Mob};

pub const BASE_MAX_MOBS_TOTAL: i32 = 60;
pub const ELITE_SPAWN_RATE: f32 = 0.06;
pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>()
            .add_systems(
                (
                    handle_spawn_mobs,
                    tick_spawner_timers.run_if(is_not_paused),
                    // handle_add_fairy_spawners,
                    test_mob_count,
                    spawn_stone_golem_timer.run_if(is_not_paused),
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
) {
    let mut spawners = vec![];
    if maybe_dungeon.get_single().is_err() {
        spawners.push(Spawner {
            enemy: Mob::SpikeSlime,
            weight: 100.,
            spawn_timer: Timer::from_seconds(16., TimerMode::Once),
            min_days_to_spawn: 3,
            num_to_spawn: Some(2),
        });
        spawners.push(Spawner {
            enemy: Mob::FurDevil,
            weight: 100.,
            spawn_timer: Timer::from_seconds(10.5, TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(4),
        });
        spawners.push(Spawner {
            enemy: Mob::RedMushling,
            weight: 200.,
            spawn_timer: Timer::from_seconds(25., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: None,
        });
        // spawners.push(Spawner {
        //     enemy: Mob::Hog,
        //     weight: 20.,
        //     spawn_timer: Timer::from_seconds(120., TimerMode::Once),
        //     min_days_to_spawn: 0,
        //     num_to_spawn: None,
        // });
        spawners.push(Spawner {
            enemy: Mob::StingFly,
            weight: 100.,
            spawn_timer: Timer::from_seconds(18., TimerMode::Once),
            min_days_to_spawn: 2,
            num_to_spawn: Some(2),
        });
        spawners.push(Spawner {
            enemy: Mob::Bushling,
            weight: 100.,
            spawn_timer: Timer::from_seconds(16., TimerMode::Once),
            min_days_to_spawn: 1,
            num_to_spawn: Some(3),
        });
    }
    commands.insert_resource(GlobalSpawners {
        spawners,
        initial_spawn_delay: Timer::from_seconds(5., TimerMode::Once),
    });
}

fn handle_spawn_mobs(
    game: GameParam,
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    prototypes: Prototypes,
    mut spawner_trigger_event: EventReader<MobSpawnEvent>,
    proto_param: ProtoParam,
    player_t: Query<&GlobalTransform, With<Player>>,
    mut spawners: ResMut<GlobalSpawners>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    infinite_mode: Res<InfiniteMode>,
) {
    if maybe_dungeon.get_single().is_ok() {
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
            let max_range: f32 = TILE_SIZE.x * 16.;

            while pos.distance(player_pos) <= TILE_SIZE.x * 8. || !can_spawn_mob_here_check {
                let spawn_pos_delta: Vec2 = Vec2::new(
                    rng.gen_range(-max_range / 2. ..max_range / 2.),
                    rng.gen_range(-max_range / 2. ..max_range / 2.),
                );
                pos = player_pos + spawn_pos_delta;
                can_spawn_mob_here_check = can_spawn_mob_here(pos, &game, &proto_param, false);
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
                if rng.gen::<f32>() < ELITE_SPAWN_RATE
                    && !(proto_param
                        .get_component::<CombatAlignment, _>(mob)
                        .expect("mob has no alignment")
                        == &CombatAlignment::Passive)
                {
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
            timer: Timer::from_seconds(240.0, TimerMode::Repeating), // 4 minutes
        }
    }
}

/// Initialize Stone Golem spawn timer as a fresh resource
/// Called when entering Main Menu to ensure it resets between runs
fn initialize_stone_golem_timer(mut commands: Commands) {
    commands.insert_resource(StoneGolemSpawnTimer::default());
    info!("Stone Golem spawn timer initialized");
}

/// System to spawn Stone Golem every 4 minutes in main eras (not dungeons)
fn spawn_stone_golem_timer(
    time: Res<Time>,
    golem_timer: Option<ResMut<StoneGolemSpawnTimer>>,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    proto_param: ProtoParam,
    player_query: Query<&GlobalTransform, With<Player>>,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    game: GameParam,
    existing_golems: Query<&Mob>,
) {
    // Check if in dungeon - don't spawn in dungeons
    if maybe_dungeon.get_single().is_ok() {
        return;
    }

    let Some(mut golem_timer) = golem_timer else {
        return;
    };

    // Tick the timer
    golem_timer.timer.tick(time.delta());

    // Spawn golem when timer finishes
    if golem_timer.timer.just_finished() {
        // Don't spawn if a Stone Golem already exists
        let golem_exists = existing_golems.iter().any(|m| m == &Mob::StoneGolem);
        if golem_exists {
            info!("Stone Golem already exists, skipping spawn");
            return;
        }

        // Find a spawn position near the player
        if let Ok(player_txfm) = player_query.get_single() {
            let player_pos = player_txfm.translation().truncate();
            let mut rng = rand::thread_rng();
            let mut pos = player_pos;
            let mut attempts = 20;
            let spawn_distance = TILE_SIZE.x * 12.0; // Spawn 12 tiles away

            while attempts > 0 {
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let offset = Vec2::new(angle.cos(), angle.sin()) * spawn_distance;
                pos = player_pos + offset;

                if can_spawn_mob_here(pos, &game, &proto_param, false) {
                    break;
                }
                attempts -= 1;
            }

            if attempts > 0 {
                proto_commands.spawn_from_proto(Mob::StoneGolem, &prototypes, pos);
                info!("Spawned Stone Golem at {:?}", pos);
            } else {
                info!("Failed to find valid spawn location for Stone Golem");
            }
        }
    }
}

/// Reset Stone Golem spawn timer when changing eras/dimensions
fn reset_stone_golem_timer_on_era_change(
    mut golem_timer: Option<ResMut<StoneGolemSpawnTimer>>,
    dimension_spawn_events: EventReader<DimensionSpawnEvent>,
) {
    if !dimension_spawn_events.is_empty() {
        if let Some(ref mut timer) = golem_timer {
            timer.timer.reset();
            info!("Stone Golem spawn timer reset due to era/dimension change");
        }
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
) {
    if !spawners.initial_spawn_delay.finished() {
        spawners.initial_spawn_delay.tick(time.delta());
        return;
    }
    // for each spawned chunk, check if mob count is < max
    // and if so, send event to spawn more
    let mob_count = mobs
        .iter()
        .filter(|m| m != &&Mob::RedMushling && m != &&Mob::Hog && m != &&Mob::Fairy)
        .count() as i32;

    // In infinite mode, allow more mobs to spawn
    let max_mobs = if infinite_mode.active {
        BASE_MAX_MOBS_TOTAL * 2 + night_tracker.days as i32 * 10
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
    let endless_mode_spawn_count_increase = if infinite_mode.active {
        match infinite_mode.difficulty_level {
            0..=3 => 1,
            4..=7 => 2,
            8..=10 => 3,
            _ => 0,
        }
    } else {
        0
    };
    for spawner in spawners.spawners.iter_mut() {
        debug!("spawner check: {:?} {:?}", spawner.min_days_to_spawn, day);
        if day < spawner.min_days_to_spawn {
            continue;
        }

        spawner.spawn_timer.tick(time.delta());

        // Speed up spawns during night OR infinite mode
        if night_tracker.is_night() || infinite_mode.active {
            // 3x spawn rate at night / infinite mode
            spawner.spawn_timer.tick(time.delta());
            spawner.spawn_timer.tick(time.delta());
            spawner.spawn_timer.tick(time.delta());
            // tick extra time in infinite mode
            if infinite_mode.active {
                spawner.spawn_timer.tick(time.delta());
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
