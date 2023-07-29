use std::time::Duration;

use bevy::{prelude::*, utils::HashMap};
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::{seq::SliceRandom, Rng};

use crate::{
    custom_commands::CommandsExt,
    proto::proto_param::ProtoParam,
    world::{
        chunk::Chunk,
        world_helpers::{camera_pos_to_chunk_pos, tile_pos_to_world_pos, world_pos_to_tile_pos},
        TileMapPosition, CHUNK_SIZE,
    },
    GameParam, GameState,
};

use super::{Mob, NeutralMob};

pub const MAX_MOB_PER_CHUNK: u32 = 6;
pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>().add_systems(
            (
                Self::handle_spawn_mobs,
                Self::tick_spawner_timers,
                Self::add_spawners_to_new_chunks,
                Self::check_mob_count,
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
}

#[derive(Debug)]
pub struct MobSpawnEvent {
    chunk_pos: IVec2,
}

impl SpawnerPlugin {
    fn add_spawners_to_new_chunks(
        mut commands: Commands,
        new_chunk_query: Query<(Entity, &Chunk), Added<Chunk>>,
    ) {
        for new_chunk in new_chunk_query.iter() {
            let mut spawners = vec![];
            spawners.push(Spawner {
                chunk_pos: new_chunk.1.chunk_pos,
                weight: 1.,
                spawn_timer: Timer::from_seconds(30., TimerMode::Once),
                max_summons: 5,
                enemy: Mob::Neutral(NeutralMob::Slime),
            });
            commands
                .entity(new_chunk.0)
                .insert(ChunkSpawners { spawners });
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
                        if let Some(_existing_object) =
                            game.get_obj_entity_at_tile(world_pos_to_tile_pos(pos), &proto_param)
                        {
                            return;
                        }
                        prototypes.is_ready("Slime");
                        proto_commands.spawn_from_proto(NeutralMob::Slime, &prototypes, pos);
                    }
                }
            }
        }
    }
    fn check_mob_count(
        chunk_query: Query<(Entity, &Transform, &mut ChunkSpawners), With<Chunk>>,
        mobs: Query<&Transform, With<Mob>>,
        mut spawn_event: EventWriter<MobSpawnEvent>,
    ) {
        //
        let mut mob_counts = HashMap::new();
        // count # mobs in each chunk
        for t in mobs.iter() {
            let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
            mob_counts.insert(chunk_pos, *mob_counts.get(&chunk_pos).unwrap_or(&0) + 1);
        }
        // for each spawned chunk, check if mob count is < max
        // and if so, send event to spawn more
        for (_e, t, _spawners) in chunk_query.iter() {
            let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
            if *mob_counts.get(&chunk_pos).unwrap_or(&0) >= MAX_MOB_PER_CHUNK {
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
}
