use std::time::Duration;

use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::{seq::SliceRandom, Rng};

use crate::{
    combat::EnemyDeathEvent,
    custom_commands::CommandsExt,
    item::WorldObject,
    night::{NewDayEvent, NightTracker},
    player::Player,
    proto::proto_param::ProtoParam,
    world::{
        chunk::Chunk,
        dimension::ActiveDimension,
        dungeon::Dungeon,
        world_helpers::{camera_pos_to_chunk_pos, tile_pos_to_world_pos, world_pos_to_tile_pos},
        TileMapPosition, CHUNK_SIZE, TILE_SIZE,
    },
    GameParam, GameState,
};

use super::{CombatAlignment, EliteMob, Mob};

pub const MAX_MOB_PER_CHUNK: i32 = 6;
pub const ELITE_SPAWN_RATE: f32 = 0.07;
pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>()
            .add_systems(
                (
                    handle_spawn_mobs,
                    tick_spawner_timers,
                    add_spawners_to_new_chunks,
                    handle_add_fairy_spawners,
                    spawn_one_time_enemies_at_day,
                    reduce_chunk_mob_count_on_mob_death,
                    despawn_out_of_range_mobs,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(check_mob_count.in_base_set(CoreSet::PreUpdate));
    }
}

#[derive(Clone, Debug, Default)]

pub struct Spawner {
    pub chunk_pos: IVec2,
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
        self.chunk_pos == other.chunk_pos
            && self.weight == other.weight
            && self.min_days_to_spawn == other.min_days_to_spawn
            && self.enemy == other.enemy
    }
}
#[derive(Component, Debug)]
pub struct ChunkSpawners {
    pub spawners: Vec<Spawner>,
    pub spawned_mobs: i32,
}

#[derive(Debug)]
pub struct MobSpawnEvent {
    chunk_pos: IVec2,
    bypass_timers: bool,
}

fn add_spawners_to_new_chunks(
    mut commands: Commands,
    maybe_dungeon: Query<&Dungeon, With<ActiveDimension>>,
    new_chunk_query: Query<(Entity, &Chunk), Added<Chunk>>,
) {
    for new_chunk in new_chunk_query.iter() {
        let mut spawners = vec![];
        if maybe_dungeon.get_single().is_err() {
            spawners.push(Spawner {
                enemy: Mob::SpikeSlime,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(50., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::FurDevil,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(50., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::RedMushling,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 50.,
                spawn_timer: Timer::from_seconds(50., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::Hog,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(50., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::StingFly,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(50., TimerMode::Once),
                min_days_to_spawn: 2,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::Bushling,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(50., TimerMode::Once),
                min_days_to_spawn: 1,
                num_to_spawn: None,
                num_spawned: 0,
            });
        } else {
            spawners.push(Spawner {
                enemy: Mob::SpikeSlime,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(12., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::FurDevil,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(12., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
            spawners.push(Spawner {
                enemy: Mob::Bushling,
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 100.,
                spawn_timer: Timer::from_seconds(12., TimerMode::Once),
                min_days_to_spawn: 0,
                num_to_spawn: None,
                num_spawned: 0,
            });
        }
        commands.entity(new_chunk.0).insert(ChunkSpawners {
            spawners,
            spawned_mobs: 0,
        });
    }
}

fn handle_add_fairy_spawners(
    mut chunk_query: Query<(&Chunk, &mut ChunkSpawners)>,
    new_day_event: EventReader<NewDayEvent>,
    player_pos: Query<&GlobalTransform, With<Player>>,
) {
    if !new_day_event.is_empty() {
        let player_chunk = camera_pos_to_chunk_pos(&player_pos.single().translation().truncate());
        for (chunk, mut spawners) in chunk_query.iter_mut() {
            if chunk.chunk_pos == player_chunk {
                spawners.spawners.push(Spawner {
                    enemy: Mob::Fairy,
                    chunk_pos: player_chunk,
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
    night_tracker: Res<NightTracker>,
    player_t: Query<&GlobalTransform, With<Player>>,
    mut spawners: Query<&mut ChunkSpawners>,
) {
    let c = spawner_trigger_event.len();
    for e in spawner_trigger_event.iter() {
        if game.get_chunk_entity(e.chunk_pos).is_none() {
            continue;
        }
        let chunk_e = game.get_chunk_entity(e.chunk_pos).unwrap();

        let mut rng = rand::thread_rng();
        let maybe_spawner = spawners.get_mut(chunk_e);
        let mut picked_mob_to_spawn = None;
        if let Ok(mut chunk_spawner) = maybe_spawner {
            let is_currently_spawning = chunk_spawner
                .spawners
                .iter()
                .any(|spawner| spawner.spawn_timer.percent() > 0.);
            if is_currently_spawning && !e.bypass_timers {
                continue;
            }

            if let Ok(picked_spawner) = chunk_spawner
                .spawners
                .choose_weighted_mut(&mut rng, |spawner| spawner.weight)
            {
                let no_more_spawns_left = picked_spawner.num_to_spawn.is_some()
                    && picked_spawner.num_spawned >= picked_spawner.num_to_spawn.unwrap();
                if (picked_spawner.spawn_timer.percent() == 0. || e.bypass_timers)
                    && picked_spawner.min_days_to_spawn <= night_tracker.days
                    && !no_more_spawns_left
                {
                    let player_pos = player_t.single().translation().truncate();
                    let mut pos = player_pos.clone();
                    while pos.distance(player_pos) <= TILE_SIZE.x * 10. {
                        let tile_pos = TilePos {
                            x: rng.gen_range(0..CHUNK_SIZE),
                            y: rng.gen_range(0..CHUNK_SIZE),
                        };
                        pos = tile_pos_to_world_pos(
                            TileMapPosition::new(picked_spawner.chunk_pos, tile_pos),
                            true,
                        );
                    }
                    picked_spawner.spawn_timer.tick(Duration::from_nanos(1));
                    picked_mob_to_spawn = Some((picked_spawner.enemy.clone(), pos));

                    picked_spawner.num_spawned += 1;
                }
            }
        }
        if let Some((mob, pos)) = picked_mob_to_spawn {
            let tile_pos = world_pos_to_tile_pos(pos);
            if let Some(_existing_object) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
                continue;
            }
            if game
                .get_tile_data(tile_pos)
                .expect("spawned mob but tile does not exist?")
                .block_type
                .contains(&WorldObject::WaterTile)
            {
                continue;
            }

            spawners
                .get_mut(game.get_chunk_entity(e.chunk_pos).unwrap())
                .unwrap()
                .spawned_mobs += 1;

            if let Some(spawned_mob) =
                proto_commands.spawn_from_proto(mob.clone(), &prototypes, pos)
            {
                if mob.clone() == Mob::Fairy {
                    println!("SPAWNED A FAIRY!!!");
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
    mut spawners: Query<&mut ChunkSpawners>,
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
fn check_mob_count(
    chunk_query: Query<(Entity, &Chunk, &mut ChunkSpawners), With<Chunk>>,
    mut spawn_event: EventWriter<MobSpawnEvent>,
) {
    // for each spawned chunk, check if mob count is < max
    // and if so, send event to spawn more
    for (_e, chunk, spawners) in chunk_query.iter() {
        let chunk_pos = chunk.chunk_pos;
        if spawners.spawned_mobs >= MAX_MOB_PER_CHUNK {
            continue;
        }
        spawn_event.send(MobSpawnEvent {
            chunk_pos,
            bypass_timers: false,
        });
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
    if night_tracker.days >= 5 && *day_tracker < 3 {
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
        proto_commands.spawn_from_proto(Mob::Slime, &prototypes, pos);
        *day_tracker += 1;
    }
}
fn tick_spawner_timers(
    time: Res<Time>,
    mut spawners: Query<&mut ChunkSpawners>,
    night_tracker: Res<NightTracker>,
) {
    for mut spawners in spawners.iter_mut() {
        for spawner in spawners.spawners.iter_mut() {
            if spawner.spawn_timer.percent() > 0. {
                spawner.spawn_timer.tick(time.delta());
                if night_tracker.is_night() {
                    // double spawn rate at night
                    spawner.spawn_timer.tick(time.delta());
                }
                if spawner.spawn_timer.just_finished() {
                    spawner.spawn_timer.reset();
                }
            }
        }
    }
}
