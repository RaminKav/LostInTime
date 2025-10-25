use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::Rng;

use crate::{
    client::is_not_paused,
    combat::EnemyDeathEvent,
    custom_commands::CommandsExt,
    item::WorldObject,
    night::{NewDayEvent, NightTracker},
    player::Player,
    proto::proto_param::ProtoParam,
    ui::damage_numbers::spawn_screen_locked_icon,
    world::{
        chunk::Chunk,
        dimension::ActiveDimension,
        dungeon::Dungeon,
        world_helpers::{camera_pos_to_chunk_pos, tile_pos_to_world_pos, world_pos_to_tile_pos},
        TileMapPosition, CHUNK_SIZE, TILE_SIZE,
    },
    GameParam, GameState, DEBUG,
};

use super::{spawn_helpers::can_spawn_mob_here, CombatAlignment, EliteMob, Mob};

pub const BASE_MAX_MOBS_TOTAL: i32 = 120;
pub const ELITE_SPAWN_RATE: f32 = 0.07;
pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>()
            .add_system(add_global_spawn_timer.in_schedule(OnEnter(GameState::Main)))
            .add_systems(
                (
                    handle_spawn_mobs,
                    tick_spawner_timers.run_if(is_not_paused),
                    // handle_add_fairy_spawners,
                    test_mob_count,
                    spawn_one_time_enemies_at_day,
                    reduce_chunk_mob_count_on_mob_death,
                    despawn_out_of_range_mobs,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(add_spawners_to_new_chunks.in_schedule(OnEnter(GameState::Main)));
    }
}

#[derive(Resource, Debug)]
pub struct GlobalSpawnTimer {
    pub timer: Timer,
}

#[derive(Clone, Debug, Default)]

pub struct Spawner {
    // pub radius: u32,
    pub weight: f32,
    pub spawn_timer: Timer,
    pub min_days_to_spawn: u8,
    pub enemy: Mob,
    pub num_to_spawn: Option<u32>,
    pub num_spawned: u32,
}
impl PartialEq for Spawner {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
            && self.min_days_to_spawn == other.min_days_to_spawn
            && self.enemy == other.enemy
    }
}
#[derive(Component, Debug)]
pub struct GlobalSpawners {
    pub spawners: Vec<Spawner>,
    pub initial_spawn_delay: Timer,
    pub spawned_mobs: i32,
}

#[derive(Debug)]
pub struct MobSpawnEvent {
    spawner: Entity,
    mob: Mob,
    bypass_timers: bool,
}

fn add_global_spawn_timer(mut commands: Commands) {
    commands.insert_resource(GlobalSpawnTimer {
        timer: Timer::from_seconds(1., TimerMode::Once),
    });
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
            spawn_timer: Timer::from_seconds(25.5, TimerMode::Once),
            min_days_to_spawn: 2,
            num_to_spawn: Some(4),
            num_spawned: 0,
        });
        spawners.push(Spawner {
            enemy: Mob::FurDevil,
            weight: 100.,
            spawn_timer: Timer::from_seconds(10.5, TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: Some(5),
            num_spawned: 0,
        });
        spawners.push(Spawner {
            enemy: Mob::RedMushling,
            weight: 200.,
            spawn_timer: Timer::from_seconds(25., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: None,
            num_spawned: 0,
        });
        // spawners.push(Spawner {
        //     enemy: Mob::Hog,
        //     weight: 20.,
        //     spawn_timer: Timer::from_seconds(120., TimerMode::Once),
        //     min_days_to_spawn: 0,
        //     num_to_spawn: None,
        //     num_spawned: 0,
        // });
        spawners.push(Spawner {
            enemy: Mob::StingFly,
            weight: 100.,
            spawn_timer: Timer::from_seconds(25.5, TimerMode::Once),
            min_days_to_spawn: 2,
            num_to_spawn: Some(5),
            num_spawned: 0,
        });
        spawners.push(Spawner {
            enemy: Mob::Bushling,
            weight: 100.,
            spawn_timer: Timer::from_seconds(10.5, TimerMode::Once),
            min_days_to_spawn: 1,
            num_to_spawn: Some(5),
            num_spawned: 0,
        });
    } else {
        spawners.push(Spawner {
            enemy: Mob::SpikeSlime,
            weight: 100.,
            spawn_timer: Timer::from_seconds(20., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: None,
            num_spawned: 0,
        });
        spawners.push(Spawner {
            enemy: Mob::FurDevil,
            weight: 100.,
            spawn_timer: Timer::from_seconds(20., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: None,
            num_spawned: 0,
        });
        spawners.push(Spawner {
            enemy: Mob::Bushling,
            weight: 100.,
            spawn_timer: Timer::from_seconds(20., TimerMode::Once),
            min_days_to_spawn: 0,
            num_to_spawn: None,
            num_spawned: 0,
        });
    }
    commands.spawn(GlobalSpawners {
        spawners,
        spawned_mobs: 0,
        initial_spawn_delay: Timer::from_seconds(5., TimerMode::Once),
    });
}

fn _handle_add_fairy_spawners(
    mut chunk_query: Query<(&Chunk, &mut GlobalSpawners)>,
    new_day_event: EventReader<NewDayEvent>,
    player_pos: Query<&GlobalTransform, With<Player>>,
) {
    if !new_day_event.is_empty() {
        let player_chunk = camera_pos_to_chunk_pos(&player_pos.single().translation().truncate());
        for (chunk, mut spawners) in chunk_query.iter_mut() {
            if chunk.chunk_pos == player_chunk {
                debug!("ADDED FAIRY SPAWNER TO {player_chunk:?}");
                spawners.spawners.push(Spawner {
                    enemy: Mob::Fairy,
                    weight: 9999.,
                    spawn_timer: Timer::from_seconds(60., TimerMode::Once),
                    min_days_to_spawn: 0,
                    num_to_spawn: Some(1),
                    num_spawned: 0,
                });
            }
        }
    }
}
fn handle_spawn_mobs(
    game: GameParam,
    mut proto_commands: ProtoCommands,
    mut commands: Commands,
    prototypes: Prototypes,
    mut spawner_trigger_event: EventReader<MobSpawnEvent>,
    proto_param: ProtoParam,
    player_t: Query<&GlobalTransform, With<Player>>,
    mut spawners: Query<&mut GlobalSpawners>,
    asset_server: Res<AssetServer>,
) {
    'outer: for e in spawner_trigger_event.iter() {
        let mut rng = rand::thread_rng();
        let maybe_spawner = spawners.get_mut(e.spawner);
        let mut picked_mob_to_spawn = None;
        if let Ok(mut chunk_spawner) = maybe_spawner {
            // let is_currently_spawning = chunk_spawner
            //     .spawners
            //     .iter()
            //     .any(|spawner| spawner.spawn_timer.percent() > 0.);
            // if is_currently_spawning && !e.bypass_timers {
            //     continue;
            // }

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

            chunk_spawner
                .spawners
                .iter_mut()
                .find(|s| s.enemy == e.mob)
                .expect("Mob spawner should exist {:mob}")
                .num_spawned += 1;
        }
        if let Some((mob, pos)) = picked_mob_to_spawn {
            spawners.get_mut(e.spawner).unwrap().spawned_mobs += 1;

            if let Some(spawned_mob) =
                proto_commands.spawn_from_proto(mob.clone(), &prototypes, pos)
            {
                debug!("SPAWNED A MOB!!! {spawned_mob:?}");
                if mob.clone() == Mob::Fairy {
                    debug!("SPAWNED A FAIRY!!! {spawned_mob:?}");
                    spawn_screen_locked_icon(
                        spawned_mob,
                        &mut commands,
                        &game.graphics,
                        &asset_server,
                        WorldObject::TimeFragment,
                    );
                }
                if rng.gen::<f32>() < ELITE_SPAWN_RATE
                    && !(proto_param
                        .get_component::<CombatAlignment, _>(mob)
                        .expect("mob has no alignment")
                        == &CombatAlignment::Passive)
                {
                    commands.entity(spawned_mob).insert(EliteMob);
                }
            }
        }
    }
}
fn reduce_chunk_mob_count_on_mob_death(
    mut death_events: EventReader<EnemyDeathEvent>,
    game: GameParam,
    mut spawners: Query<&mut GlobalSpawners>,
) {
    for death in death_events.iter() {
        let chunk = camera_pos_to_chunk_pos(&death.enemy_pos);
        if let Some(chunk_entity) = game.get_chunk_entity(chunk) {
            if let Ok(mut chunk_spawner) = spawners.get_mut(chunk_entity) {
                chunk_spawner.spawned_mobs -= 1;
            }
        }
    }
}

fn despawn_out_of_range_mobs(
    game: GameParam,
    mut commands: Commands,
    mut query: Query<(Entity, &Transform), With<Mob>>,
) {
    for (e, t) in query.iter_mut() {
        let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
        if game.get_chunk_entity(chunk_pos).is_none() {
            commands.entity(e).despawn_recursive();
        }
    }
}
fn spawn_one_time_enemies_at_day(
    game: GameParam,
    night_tracker: ResMut<NightTracker>,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    proto_param: ProtoParam,
    mut day_tracker: Local<u8>,
    maybe_dungeon: Query<Option<&Dungeon>, With<ActiveDimension>>,
) {
    if maybe_dungeon.get_single().is_ok() {
        return;
    }
    if night_tracker.days == 4 && *day_tracker == 3 {
        let mut rng = rand::thread_rng();
        let mut pos = Vec2::new(0., 0.);
        for _ in 0..10 {
            let tile_pos = TilePos {
                x: rng.gen_range(0..CHUNK_SIZE),
                y: rng.gen_range(0..CHUNK_SIZE),
            };
            pos = tile_pos_to_world_pos(TileMapPosition::new(IVec2::new(0, 0), tile_pos), true);
            if let Some(_existing_object) =
                game.get_obj_entity_at_tile(world_pos_to_tile_pos(pos), &proto_param)
            {
                continue;
            }
            break;
        }
        proto_commands.spawn_from_proto(Mob::RedMushking, &prototypes, pos);
        *day_tracker += 1;
    }
}
fn tick_spawner_timers(
    time: Res<Time>,
    mut spawners: Query<(Entity, &mut GlobalSpawners)>,
    night_tracker: Res<NightTracker>,
    mut spawn_event: EventWriter<MobSpawnEvent>,
    mobs: Query<&Mob>,
) {
    for (spawner_e, mut spawners) in spawners.iter_mut() {
        if !spawners.initial_spawn_delay.finished() {
            spawners.initial_spawn_delay.tick(time.delta());
            continue;
        }
        // for each spawned chunk, check if mob count is < max
        // and if so, send event to spawn more
        let mob_count = mobs
            .iter()
            .filter(|m| m != &&Mob::RedMushling && m != &&Mob::Hog && m != &&Mob::Fairy)
            .count() as i32;
        let max_mobs = BASE_MAX_MOBS_TOTAL + night_tracker.days as i32 * 10;
        if mob_count >= max_mobs {
            info!(
                "MAX MOBS {:?} {:?} {:?}",
                mob_count,
                max_mobs,
                mobs.iter().count()
            );
            return;
        }
        let day = night_tracker.days;
        for spawner in spawners.spawners.iter_mut() {
            debug!("spawner check: {:?} {:?}", spawner.min_days_to_spawn, day);
            if day < spawner.min_days_to_spawn {
                continue;
            }

            spawner.spawn_timer.tick(time.delta());
            if night_tracker.is_night() {
                // double spawn rate at night
                spawner.spawn_timer.tick(time.delta());
                spawner.spawn_timer.tick(time.delta());
                spawner.spawn_timer.tick(time.delta());
                spawner.spawn_timer.tick(time.delta());
                spawner.spawn_timer.tick(time.delta());
            }
            if spawner.spawn_timer.finished() {
                spawner.spawn_timer.reset();
                for _ in 0..spawner.num_to_spawn.unwrap_or(1) {
                    // info!("send spawn event! {:?}", spawner.enemy);
                    spawn_event.send(MobSpawnEvent {
                        spawner: spawner_e,
                        mob: spawner.enemy.clone(),
                        bypass_timers: false,
                    });
                }
            }
        }
    }
}
