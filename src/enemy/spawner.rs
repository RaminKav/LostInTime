use std::time::Duration;

use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::{seq::SliceRandom, Rng};

use crate::{
    combat::EnemyDeathEvent,
    custom_commands::CommandsExt,
    proto::proto_param::ProtoParam,
    world::{
        chunk::Chunk,
        world_helpers::{camera_pos_to_chunk_pos, tile_pos_to_world_pos, world_pos_to_tile_pos},
        TileMapPosition, CHUNK_SIZE,
    },
    GameParam, GameState,
};

use super::Mob;

pub const MAX_MOB_PER_CHUNK: i32 = 6;
pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>().add_systems(
            (
                handle_spawn_mobs,
                tick_spawner_timers,
                add_spawners_to_new_chunks,
                check_mob_count,
                reduce_chunk_mob_count_on_mob_death,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}

#[derive(Clone, Debug, Default)]

pub struct Spawner {
    pub chunk_pos: IVec2,
    // pub radius: u32,
    pub weight: f32,
    pub spawn_timer: Timer,
    pub max_summons: u32,
    pub enemy: Mob,
}
impl PartialEq for Spawner {
    fn eq(&self, other: &Self) -> bool {
        self.chunk_pos == other.chunk_pos
            && self.weight == other.weight
            && self.max_summons == other.max_summons
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
}

fn add_spawners_to_new_chunks(
    mut commands: Commands,
    new_chunk_query: Query<(Entity, &Chunk), Added<Chunk>>,
) {
    for new_chunk in new_chunk_query.iter() {
        let mut spawners = vec![];
        spawners.push(Spawner {
            chunk_pos: new_chunk.1.chunk_pos,
            weight: 1.,
            spawn_timer: Timer::from_seconds(5., TimerMode::Once),
            max_summons: 5,
            enemy: Mob::SpikeSlime,
        });
        spawners.push(Spawner {
            chunk_pos: new_chunk.1.chunk_pos,
            weight: 1.,
            spawn_timer: Timer::from_seconds(5., TimerMode::Once),
            max_summons: 5,
            enemy: Mob::FurDevil,
        });
        commands.entity(new_chunk.0).insert(ChunkSpawners {
            spawners,
            spawned_mobs: 0,
        });
    }
}
fn handle_spawn_mobs(
    mut game: GameParam,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    mut spawner_trigger_event: EventReader<MobSpawnEvent>,
    proto_param: ProtoParam,
) {
    for e in spawner_trigger_event.iter() {
        if game.get_chunk_entity(e.chunk_pos).is_none() {
            continue;
        }
        let mut rng = rand::thread_rng();
        let maybe_spawner = game
            .chunk_query
            .get_mut(*game.get_chunk_entity(e.chunk_pos).unwrap());
        let mut picked_mob_to_spawn = None;
        if let Ok(mut chunk_spawner) = maybe_spawner {
            if let Ok(picked_spawner) = chunk_spawner
                .2
                .spawners
                .choose_weighted_mut(&mut rng, |spawner| spawner.weight)
            {
                if picked_spawner.spawn_timer.percent() == 0. {
                    let tile_pos = TilePos {
                        x: rng.gen_range(0..CHUNK_SIZE),
                        y: rng.gen_range(0..CHUNK_SIZE),
                    };
                    let pos = tile_pos_to_world_pos(
                        TileMapPosition::new(picked_spawner.chunk_pos, tile_pos, 0),
                        true,
                    );
                    picked_spawner.spawn_timer.tick(Duration::from_nanos(1));
                    picked_mob_to_spawn = Some((picked_spawner.enemy.clone(), pos));
                }
            }
        }
        if let Some((mob, pos)) = picked_mob_to_spawn {
            if let Some(_existing_object) =
                game.get_obj_entity_at_tile(world_pos_to_tile_pos(pos), &proto_param)
            {
                return;
            }
            // prototypes.is_ready("Slime");
            game.chunk_query
                .get_mut(*game.get_chunk_entity(e.chunk_pos).unwrap())
                .unwrap()
                .2
                .spawned_mobs += 1;
            proto_commands.spawn_from_proto(mob, &prototypes, pos);
        }
    }
}
fn reduce_chunk_mob_count_on_mob_death(
    mut death_events: EventReader<EnemyDeathEvent>,
    mut game: GameParam,
) {
    for death in death_events.iter() {
        let chunk = camera_pos_to_chunk_pos(&death.enemy_pos);
        if let Some(chunk_entity) = game.get_chunk_entity(chunk) {
            if let Ok(mut chunk_spawner) = game.chunk_query.get_mut(*chunk_entity) {
                chunk_spawner.2.spawned_mobs -= 1;
            }
        }
    }
}
fn check_mob_count(
    chunk_query: Query<(Entity, &Transform, &mut ChunkSpawners), With<Chunk>>,
    mut spawn_event: EventWriter<MobSpawnEvent>,
) {
    // for each spawned chunk, check if mob count is < max
    // and if so, send event to spawn more
    for (_e, t, spawners) in chunk_query.iter() {
        let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
        if spawners.spawned_mobs >= MAX_MOB_PER_CHUNK {
            continue;
        }
        spawn_event.send(MobSpawnEvent { chunk_pos });
    }
}
fn tick_spawner_timers(mut game: GameParam, time: Res<Time>) {
    for (_e, _t, mut spawners) in game.chunk_query.iter_mut() {
        for spawner in spawners.spawners.iter_mut() {
            if spawner.spawn_timer.percent() > 0. {
                spawner.spawn_timer.tick(time.delta());
                if spawner.spawn_timer.just_finished() {
                    spawner.spawn_timer.reset();
                }
            }
        }
    }
}
