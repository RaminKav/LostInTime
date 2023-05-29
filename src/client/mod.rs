use bevy::{ecs::system::SystemState, prelude::*, utils::HashMap};
use bevy_save::{Build, CloneReflect, DespawnMode, MappingMode, Snapshot};

use crate::world::{
    chunk::{DespawnChunkEvent, SpawnChunkEvent},
    dimension::{ActiveDimension, ChunkCache},
    ChunkManager,
};

pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems((Self::save_chunk, Self::load_chunk));
    }
}

impl ClientPlugin {
    fn load_chunk(world: &mut World) {
        let mut state: SystemState<(
            Query<&ChunkCache, With<ActiveDimension>>,
            EventReader<SpawnChunkEvent>,
        )> = SystemState::new(world);
        let (dim_query, mut spawn_events) = state.get_mut(world);
        let mut snapshots = vec![];
        for event in spawn_events.iter() {
            if let Some(snapshot) = dim_query.single().snapshots.get(&event.chunk_pos) {
                snapshots.push(snapshot.clone_value());
            }
        }
        for snapshot in snapshots.iter() {
            snapshot
                .applier(world)
                .despawn(DespawnMode::None)
                .mapping(MappingMode::Strict)
                .apply()
                .expect("Failed to Load snapshot.");
            // let mut cache_events = state.get_mut(world).0;
            println!("Loading chunk from cache.");

            // cache_events.send(GenerateEvent);
        }
    }
    //TODO: make this work with Spawn events too ,change event name
    fn save_chunk(world: &mut World) {
        let mut state: SystemState<(Res<ChunkManager>, EventReader<DespawnChunkEvent>)> =
            SystemState::new(world);

        let (chunk_manager, mut save_events) = state.get_mut(world);

        let mut saved_chunks = HashMap::default();
        for saves in save_events.iter() {
            println!("SAVING...");

            saved_chunks.insert(
                saves.chunk_pos,
                chunk_manager.chunks.get(&saves.chunk_pos).unwrap().clone(),
            );
        }

        let mut snapshots: HashMap<IVec2, Snapshot> = HashMap::default();
        for (chunk_pos, entity) in saved_chunks.iter() {
            let snapshot = Snapshot::builder(world)
                .extract_entities(vec![*entity].into_iter())
                .build();
            snapshots.insert(*chunk_pos, snapshot);
        }

        let mut state: SystemState<(
            Commands,
            Res<ChunkManager>,
            Query<&mut ChunkCache, With<ActiveDimension>>,
        )> = SystemState::new(world);

        let (mut commands, chunk_manager, mut dim_query) = state.get_mut(world);

        for (chunk_pos, snapshot) in snapshots.iter() {
            println!("Inserting new snapshot for {chunk_pos:?} and despawning it");
            dim_query
                .single_mut()
                .snapshots
                .insert(*chunk_pos, snapshot.clone_value());
            commands
                .entity(*chunk_manager.chunks.get(chunk_pos).unwrap())
                .despawn_recursive();
        }
    }
}
