use std::collections::HashMap;

use super::SchematicSpawnEvent;
use crate::{
    item::{PlaceItemEvent, WorldObject},
    world::{
        chunk::Chunk, world_helpers::tile_pos_to_world_pos, TileMapPosition, CHUNK_SIZE,
        ISLAND_SIZE,
    },
    GameParam,
};
use bevy_ecs_tilemap::tiles::TilePos;
use rand::Rng;

use bevy::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct SchematicSpawner {
    pub object: WorldObject,
}

/// Tracks how many of each schematic type were spawned during world generation.
#[derive(Resource, Default)]
pub struct SchematicSpawnTracker {
    pub counts: HashMap<WorldObject, u32>,
}

/// Clears the schematic spawn tracker when entering Initializing state.
pub fn clear_schematic_tracker(mut tracker: ResMut<SchematicSpawnTracker>) {
    tracker.counts.clear();
}

/// Logs the schematic spawn stats when leaving Initializing state.
pub fn log_schematic_spawn_stats(tracker: Res<SchematicSpawnTracker>) {
    if tracker.counts.is_empty() {
        return;
    }

    let total: u32 = tracker.counts.values().sum();
    info!("=== Schematic Spawn Stats ===");
    for (object, count) in tracker.counts.iter() {
        info!("  {:?}: {}", object, count);
    }
    info!("  Total: {}", total);
    info!("=============================");
}

/// Directly spawns schematic objects without loading scene files.
/// This is much faster than the file-based system (~80% performance improvement).
pub fn attempt_to_spawn_schematic_in_chunk(
    mut commands: Commands,
    chunks: Query<(Entity, &Chunk, &SchematicSpawner)>,
    mut place_item_event: EventWriter<PlaceItemEvent>,
    mut tracker: ResMut<SchematicSpawnTracker>,
) {
    for (e, chunk, spawner) in chunks.iter() {
        let mut rng = rand::thread_rng();
        let rng_x = rng.gen_range(4..13);
        let rng_y = rng.gen_range(4..13);
        let target_pos = tile_pos_to_world_pos(
            TileMapPosition::new(chunk.chunk_pos, TilePos::new(rng_x, rng_y)),
            true,
        );
        place_item_event.send(PlaceItemEvent {
            obj: spawner.object,
            pos: target_pos,
            placed_by_player: false,
            override_existing_obj: false,
        });

        *tracker.counts.entry(spawner.object).or_insert(0) += 1;

        commands.entity(e).remove::<SchematicSpawner>();
    }
}

pub fn give_chunks_schematic_spawners(
    mut commands: Commands,
    game: GameParam,
    mut spawn_event: EventReader<SchematicSpawnEvent>,
) {
    for chunk in spawn_event.iter() {
        // Skip center chunks to keep spawn area clear
        if chunk.0 == IVec2::ZERO
            || chunk.0 == IVec2::new(-1, 0)
            || chunk.0 == IVec2::new(0, -1)
            || chunk.0 == IVec2::new(-1, -1)
            || chunk.0.x.abs() > (ISLAND_SIZE / CHUNK_SIZE as f32) as i32 - 1
            || chunk.0.y.abs() > (ISLAND_SIZE / CHUNK_SIZE as f32) as i32 - 1
        {
            continue;
        }

        if let Some(e) = game.get_chunk_entity(chunk.0) {
            let mut rng = rand::thread_rng();
            for (world_object, frequency) in
                game.world_generation_params.schematic_frequencies.iter()
            {
                if rng.gen::<f64>() < *frequency {
                    commands.entity(e).insert(SchematicSpawner {
                        object: *world_object,
                    });
                    break;
                }
            }
        }
    }
}
