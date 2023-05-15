use std::time::Duration;

use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use rand::{seq::SliceRandom, Rng};

use crate::{
    world::{
        chunk::{Chunk, SpawnChunkEvent},
        world_helpers::camera_pos_to_chunk_pos,
        ChunkManager, CHUNK_SIZE, NUM_CHUNKS_AROUND_CAMERA,
    },
    GameParam, GameState, TIME_STEP,
};

use super::{Enemy, EnemyMaterial};

pub const MAX_MOB_PER_CHUNK: u32 = 16;
pub struct SpawnerPlugin;
impl Plugin for SpawnerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MobSpawnEvent>().add_systems(
            (
                Self::handle_spawn_mobs,
                Self::handle_add_spawners_on_chunk_spawn,
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
    pub enemy: Enemy,
}
#[derive(Component, Debug)]
pub struct ChunkSpawners {
    pub spawners: Vec<Spawner>,
}

#[derive(Debug)]
pub struct MobSpawnEvent {
    chunk_pos: IVec2,
    spawners: Vec<Spawner>,
}

impl Spawner {
    fn spawn_mob(
        &mut self,
        game: &mut GameParam,
        commands: &mut Commands,
        asset_server: AssetServer,
        materials: &mut Assets<EnemyMaterial>,
    ) {
        let mut rng = rand::thread_rng();

        let tile_pos = TilePos {
            x: rng.gen_range(0..CHUNK_SIZE),
            y: rng.gen_range(0..CHUNK_SIZE),
        };
        let pos = Vec2::new(
            (tile_pos.x as i32 * 32 + self.chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            (tile_pos.y as i32 * 32 + self.chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
        );
        println!("spawning! {:?} at {:?}", self.enemy, pos);
        self.spawn_timer.tick(Duration::from_nanos(1));
        self.enemy
            .clone()
            .summon(commands, game, &asset_server, materials, pos);
    }
}

impl SpawnerPlugin {
    fn handle_spawn_mobs(mut game: GameParam, mut spawn_event: EventReader<MobSpawnEvent>) {
        for e in spawn_event.iter() {
            println!("GOT SPAWN EVENT FOR {:?}", e.chunk_pos);
            let mut rng = rand::thread_rng();
            if let Ok(picked_spawner) = game
                .chunk_query
                .get_mut(*game.get_chunk_entity(e.chunk_pos).unwrap())
                .unwrap()
                .2
                .spawners
                .choose_weighted_mut(&mut rng, |spawner| spawner.weight)
            {
                if picked_spawner.spawn_timer.percent() == 0. {
                    //TODO: Turn into event vvv

                    // picked_spawner.spawn_mob(
                    //     &mut game,
                    //     &mut commands,
                    //     asset_server.clone(),
                    //     &mut materials,
                    // );
                }
            }
        }
    }
    fn check_mob_count(
        chunk_query: Query<(Entity, &Transform, &mut ChunkSpawners), With<Chunk>>,
        mobs: Query<&Transform, With<Enemy>>,
        mut spawn_event: EventWriter<MobSpawnEvent>,
    ) {
        //
        const S: usize = (NUM_CHUNKS_AROUND_CAMERA + 1) as usize;
        let mut mob_counts: [[u32; S]; S] = [[0; S]; S];
        // count # mobs in each chunk
        for t in mobs.iter() {
            let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
            mob_counts[chunk_pos.x as usize][chunk_pos.y as usize] += 1;
        }
        // for each spawned chunk, check if mob count is < max
        // and if so, send event to spawn more
        for (e, t, spawners) in chunk_query.iter() {
            let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
            if mob_counts[chunk_pos.x as usize][chunk_pos.y as usize] >= MAX_MOB_PER_CHUNK {
                continue;
            }
            spawn_event.send(MobSpawnEvent {
                chunk_pos,
                spawners: spawners.spawners.to_vec(),
            });
        }
    }
    fn tick_spawner_timers(mut game: GameParam, time: Res<Time>) {
        for (e, t, mut spawners) in game.chunk_query.iter_mut() {
            let chunk_pos = camera_pos_to_chunk_pos(&t.translation.truncate());
            for spawner in spawners.spawners.iter_mut() {
                if spawner.spawn_timer.percent() > 0. {
                    spawner.spawn_timer.tick(time.delta());
                    println!("tick! {:?}", spawner.spawn_timer.percent());
                    if spawner.spawn_timer.just_finished() {
                        spawner.spawn_timer.reset();
                    }
                }
            }
        }
    }
    fn handle_add_spawners_on_chunk_spawn(
        mut spawn_events: EventReader<SpawnChunkEvent>,
        mut game: GameParam,
        mut commands: Commands,
    ) {
        for e in spawn_events.iter() {
            if e.chunk_pos != (IVec2 { x: 0, y: 0 }) {
                continue;
            }
            let spawner = Spawner {
                chunk_pos: IVec2 { x: 0, y: 0 },
                weight: 1.,
                spawn_timer: Timer::new(Duration::from_secs(5), TimerMode::Once),
                max_summons: 5,
                enemy: Enemy::Slime,
            };
            println!("Adding spawner for {:?}", e.chunk_pos);
            commands
                .entity(*game.get_chunk_entity(e.chunk_pos).unwrap())
                .insert(ChunkSpawners {
                    spawners: vec![spawner],
                });
        }
    }
}
