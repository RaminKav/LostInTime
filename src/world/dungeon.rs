use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;

use crate::{
    item::WorldObject,
    world::{
        dimension::{Dimension, GenerationSeed, SpawnDimension},
        RawChunkData,
    },
};

use super::{
    chunk::ZERO_ZERO,
    dungeon_generation::{gen_new_dungeon, Bias},
    ChunkManager, TileEntityData, TileMapPositionData, CHUNK_SIZE,
};

#[derive(Component)]
pub struct Dungeon;
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, _app: &mut App) {}
}

impl DungeonPlugin {
    pub fn gen_and_spawn_new_dungeon_dimension(commands: &mut Commands) {
        let grid = gen_new_dungeon(
            1500,
            CHUNK_SIZE as usize,
            Bias {
                bias: super::dungeon_generation::Direction::Left,
                strength: 0,
            },
        );
        let chunk_pos = ZERO_ZERO;

        let mut cm = ChunkManager::new();
        let mut raw_chunk_bits: [[[u8; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[[0; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        let mut raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[[WorldObject::Sand; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let tile_pos = TilePos { x, y };
                let tile_bits = if grid[x as usize][y as usize] == 1 {
                    0000
                } else {
                    0001
                };
                let tile_quad_blocks = if tile_bits == 0000 {
                    [WorldObject::DungeonStone; 4]
                } else {
                    [WorldObject::None; 4]
                };

                raw_chunk_bits[x as usize][y as usize] = [0; 4];
                raw_chunk_blocks[x as usize][y as usize] = tile_quad_blocks;

                cm.cached_chunks.insert(chunk_pos);

                cm.chunk_tile_entity_data.insert(
                    TileMapPositionData {
                        chunk_pos,
                        tile_pos,
                    },
                    TileEntityData {
                        entity: None,
                        tile_bit_index: tile_bits,
                        block_type: tile_quad_blocks,
                        block_offset: 32,
                    },
                );

                cm.raw_chunk_data.insert(
                    chunk_pos,
                    RawChunkData {
                        raw_chunk_bits,
                        raw_chunk_blocks,
                    },
                );
            }
        }
        println!("SENDING DIM SPAWN EVENT");
        let dim_e = commands
            .spawn((Dimension, Dungeon, GenerationSeed { seed: 123 }, cm))
            .id();
        commands.entity(dim_e).insert(SpawnDimension);
    }
}
