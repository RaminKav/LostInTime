use bevy::prelude::*;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};

use crate::item::WorldObject;

use super::{
    noise_helpers, ChunkManager, TileEntityData, TileMapPositionData, WorldObjectEntityData,
    CHUNK_SIZE,
};

pub struct TilePlugin;
impl Plugin for TilePlugin {
    fn build(&self, app: &mut App) {
        app;
    }
}

impl TilePlugin {
    pub fn get_tile_from_perlin_noise(
        chunk_manager: &ChunkManager,
        chunk_pos: IVec2,
        tile_pos: TilePos,
        seed: u32,
    ) -> ([u8; 4], u8, [WorldObject; 4]) {
        let x = tile_pos.x as f64;
        let y = tile_pos.y as f64;
        //TODO: figure out what this 16. is for
        let nx = (x as i32 + chunk_pos.x * CHUNK_SIZE as i32) as f64; // as f64 / 16. as f64 - 0.5;
        let ny = (y as i32 + chunk_pos.y * CHUNK_SIZE as i32) as f64; // as f64 / 16. as f64 - 0.5;
                                                                      // let e = noise_e.get([nx, ny]) + 0.5;
        let mut bits = [0, 0, 0, 0];
        let mut blocks = [
            WorldObject::Sand,
            WorldObject::Sand,
            WorldObject::Sand,
            WorldObject::Sand,
        ];
        let sample = |x: f64, y: f64| -> (u8, WorldObject) {
            let e = noise_helpers::get_perlin_noise_for_tile(x, y, seed);
            // let m = (noise_m.get([x * base_oct, ny * base_oct]) + 0.5)
            //     + 0.5 * (noise_m2.get([x * base_oct * 2., ny * base_oct * 2.]) + 0.5)
            //     + 0.25 * (noise_m3.get([x * base_oct * 3., ny * base_oct * 3.]) + 0.5);

            // let e = f64::powf(e / (1. + 0.5 + 0.25), 1.);
            // let m = f64::powf(m / (1. + 0.5 + 0.25), 1.);
            // print!("{:?}", e);
            // let m = f64::powf(m, 1.);
            let block = if e <= chunk_manager.world_generation_params.water_frequency {
                WorldObject::Water
            } else if e <= chunk_manager.world_generation_params.sand_frequency {
                WorldObject::Sand
            }
            // else if e <= game.world_generation_params.dirt_frequency {
            //     WorldObject::Grass
            // }
            // } else if e <= game.world_generation_params.stone_frequency {
            //     WorldObject::Stone
            // }
            else {
                // if m > 0.75 {
                //     WorldObject::DryGrass
                // } else if m > 0.45 {
                //     WorldObject::Grass
                // } else {
                WorldObject::Grass
                // }
            };
            // if chunk_pos.x == 0 && chunk_pos.y == 0 {
            //     if y <= 8 {
            //         block = WorldObject::Grass
            //     } else {
            //         block = WorldObject::Dirt
            //     }
            // }
            let block_bits: u8 = if block == WorldObject::Sand || block == WorldObject::Grass {
                0
            } else {
                1
            };
            (block_bits, block)
        };
        let mut index_shift = 0;

        let tl = sample(nx - 0.5, ny + 0.5); // top left
        let tr = sample(nx + 0.5, ny + 0.5); // top right
        let bl = sample(nx - 0.5, ny - 0.5); // bot left
        let br = sample(nx + 0.5, ny - 0.5); // bot right
        bits[0] = tl.0;
        bits[1] = tr.0;
        bits[2] = bl.0;
        bits[3] = br.0;
        blocks[0] = tl.1;
        blocks[1] = tr.1;
        blocks[2] = bl.1;
        blocks[3] = br.1;

        // if there is grass and water in the same tile, turn the grass to sand
        if blocks.contains(&WorldObject::Grass) && blocks.contains(&WorldObject::Water) {
            for b in 0..4 {
                if blocks[b] == WorldObject::Grass {
                    blocks[b] = WorldObject::Sand;
                }
            }
        }
        // for grass/sand blocks, turn sand bits to 1, since grass bits are 0
        if blocks.contains(&WorldObject::Grass) {
            index_shift = 16;

            for b in 0..4 {
                if blocks[b] == WorldObject::Sand {
                    bits[b] = 1;
                }
            }
        }
        (bits, index_shift, blocks)
    }
    pub fn update_neighbour_tiles(
        new_tile_pos: TilePos,
        commands: &mut Commands,
        chunk_manager: &mut ResMut<ChunkManager>,
        chunk_pos: IVec2,
        update_entity: bool,
    ) {
        let x = new_tile_pos.x as i8;
        let y = new_tile_pos.y as i8;
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                // only use neighbours that have at least one water bitt
                let mut neighbour_tile_pos = TilePos {
                    x: (dx + x) as u32,
                    y: (dy + y) as u32,
                };
                let mut adjusted_chunk_pos = chunk_pos;

                if x + dx < 0 {
                    adjusted_chunk_pos.x = chunk_pos.x - 1;
                    neighbour_tile_pos.x = CHUNK_SIZE - 1;
                } else if x + dx >= CHUNK_SIZE.try_into().unwrap() {
                    adjusted_chunk_pos.x = chunk_pos.x + 1;
                    neighbour_tile_pos.x = 0;
                }
                if y + dy < 0 {
                    adjusted_chunk_pos.y = chunk_pos.y - 1;
                    neighbour_tile_pos.y = CHUNK_SIZE - 1;
                } else if y + dy >= CHUNK_SIZE.try_into().unwrap() {
                    adjusted_chunk_pos.y = chunk_pos.y + 1;
                    neighbour_tile_pos.y = 0;
                }
                if !(dx == 0 && dy == 0) {
                    let mut neighbour_tile_offset;
                    let neighbour_entity;
                    let neighbour_tile_blocks;

                    if !chunk_manager.cached_chunks.contains(&adjusted_chunk_pos) {
                        continue;
                    }
                    let neighbour_tile_entity_data =
                        chunk_manager
                            .chunk_tile_entity_data
                            .get(&TileMapPositionData {
                                chunk_pos: adjusted_chunk_pos,
                                tile_pos: neighbour_tile_pos,
                            });
                    let new_tile_entity_data = chunk_manager
                        .chunk_tile_entity_data
                        .get(&TileMapPositionData {
                            chunk_pos,
                            tile_pos: new_tile_pos,
                        })
                        .unwrap();

                    if let Some(neighbour_tile_entity_data) = neighbour_tile_entity_data {
                        neighbour_entity = neighbour_tile_entity_data.entity;
                        neighbour_tile_offset = neighbour_tile_entity_data.texture_offset;
                        neighbour_tile_blocks = neighbour_tile_entity_data.block_type;
                    } else {
                        continue;
                    }
                    let mut updated_blocks = [WorldObject::Sand; 4];
                    let mut updated_bit_index;
                    updated_blocks = Self::compute_tile_blocks(
                        new_tile_entity_data.block_type,
                        neighbour_tile_blocks,
                        (dx, dy),
                    );

                    (updated_bit_index, neighbour_tile_offset) =
                        Self::get_bits_from_block_type(updated_blocks);

                    // only continue for tiles with grass
                    if neighbour_tile_offset == 0 && !update_entity {
                        continue;
                    };
                    // set to correct sand values if we are now fully sand
                    if updated_bit_index == 0b1111 && neighbour_tile_offset == 16 {
                        updated_bit_index = 0b0000;
                        neighbour_tile_offset = 0;
                    }
                    let updated_block_type =
                        Self::get_block_type_from_bits(updated_bit_index, neighbour_tile_offset);
                    if update_entity {
                        if let Some(mut e_commands) = commands.get_entity(neighbour_entity.unwrap())
                        {
                            e_commands.insert(TileTextureIndex(
                                (updated_bit_index + neighbour_tile_offset).into(),
                            ));
                        }
                    };
                    chunk_manager.chunk_tile_entity_data.insert(
                        TileMapPositionData {
                            chunk_pos: adjusted_chunk_pos,
                            tile_pos: neighbour_tile_pos,
                        },
                        TileEntityData {
                            entity: if update_entity {
                                neighbour_entity
                            } else {
                                None
                            },
                            tile_bit_index: updated_bit_index,
                            block_type: updated_block_type,
                            texture_offset: neighbour_tile_offset,
                        },
                    );
                }
            }
        }
    }
    pub fn update_this_tile(
        tile_pos: TilePos,
        mut tile_index_offset: u8,
        chunk_manager: &mut ResMut<ChunkManager>,
        chunk_pos: IVec2,
    ) {
        let x = tile_pos.x as i8;
        let y = tile_pos.y as i8;
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                // only use neighbours that have at least one water bitt
                let mut neighbour_tile_pos = TilePos {
                    x: (dx + x) as u32,
                    y: (dy + y) as u32,
                };
                let mut adjusted_chunk_pos = chunk_pos;

                if x + dx < 0 {
                    adjusted_chunk_pos.x = chunk_pos.x - 1;
                    neighbour_tile_pos.x = CHUNK_SIZE - 1;
                } else if x + dx >= CHUNK_SIZE.try_into().unwrap() {
                    adjusted_chunk_pos.x = chunk_pos.x + 1;
                    neighbour_tile_pos.x = 0;
                }
                if y + dy < 0 {
                    adjusted_chunk_pos.y = chunk_pos.y - 1;
                    neighbour_tile_pos.y = CHUNK_SIZE - 1;
                } else if y + dy >= CHUNK_SIZE.try_into().unwrap() {
                    adjusted_chunk_pos.y = chunk_pos.y + 1;
                    neighbour_tile_pos.y = 0;
                }
                if !(dx == 0 && dy == 0) {
                    if !chunk_manager.cached_chunks.contains(&adjusted_chunk_pos) {
                        continue;
                    }
                    let neighbour_raw_block_data = chunk_manager
                        .raw_chunk_data
                        .get(&adjusted_chunk_pos)
                        .unwrap();

                    let target_block_entity_data = chunk_manager
                        .chunk_tile_entity_data
                        .get(&TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        })
                        .unwrap();

                    if target_block_entity_data.tile_bit_index
                        + target_block_entity_data.texture_offset
                        == 0
                    {
                        continue;
                    }

                    // only continue for tiles with water
                    let mut updated_bit_index = if neighbour_raw_block_data.raw_chunk_blocks
                        [neighbour_tile_pos.x as usize][neighbour_tile_pos.y as usize]
                        .contains(&WorldObject::Water)
                    {
                        let bits = target_block_entity_data.tile_bit_index;
                        Self::compute_tile_index(0b1111, bits, (-dx, -dy))
                    } else {
                        continue;
                    };

                    // turn to sand
                    if updated_bit_index == 0b1111 && tile_index_offset == 16 {
                        updated_bit_index = 0b0000;
                        tile_index_offset = 0;
                    }
                    let block_type =
                        Self::get_block_type_from_bits(updated_bit_index, tile_index_offset);

                    chunk_manager.chunk_tile_entity_data.insert(
                        TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        },
                        TileEntityData {
                            entity: None,
                            tile_bit_index: updated_bit_index,
                            block_type,
                            texture_offset: tile_index_offset,
                        },
                    );
                }
            }
        }
    }
    fn get_block_type_from_bits(bits: u8, offset: u8) -> [WorldObject; 4] {
        let used_blocks = if offset == 0 {
            (WorldObject::Sand, WorldObject::Water)
        } else {
            (WorldObject::Grass, WorldObject::Sand)
        };

        let mut block_type: [WorldObject; 4] = [WorldObject::Sand; 4];
        block_type[0] = if bits & 0b0001 != 0b0001 {
            used_blocks.0
        } else {
            used_blocks.1
        };
        block_type[1] = if bits & 0b0010 != 0b0010 {
            used_blocks.0
        } else {
            used_blocks.1
        };
        block_type[2] = if bits & 0b0100 != 0b0100 {
            used_blocks.0
        } else {
            used_blocks.1
        };
        block_type[3] = if bits & 0b1000 != 0b1000 {
            used_blocks.0
        } else {
            used_blocks.1
        };
        block_type
    }
    fn get_bits_from_block_type(block_type: [WorldObject; 4]) -> (u8, u8) {
        let offset = if block_type.contains(&WorldObject::Grass) {
            16
        } else {
            0
        };
        let mut bits = 0b0000;

        bits |= if block_type[0] == WorldObject::Water
            || (offset == 16 && block_type[0] == WorldObject::Sand)
        {
            0b0001
        } else {
            0b0000
        };
        bits |= if block_type[1] == WorldObject::Water
            || (offset == 16 && block_type[1] == WorldObject::Sand)
        {
            0b0010
        } else {
            0b0000
        };
        bits |= if block_type[2] == WorldObject::Water
            || (offset == 16 && block_type[2] == WorldObject::Sand)
        {
            0b0100
        } else {
            0b0000
        };
        bits |= if block_type[3] == WorldObject::Water
            || (offset == 16 && block_type[03] == WorldObject::Sand)
        {
            0b1000
        } else {
            0b0000
        };

        (bits, offset)
    }
    pub fn compute_tile_index(new_tile_bits: u8, neighbour_bits: u8, edge: (i8, i8)) -> u8 {
        let mut index = 0;
        // new tile will be 0b1111 i think
        if edge == (0, 1) {
            // Top edge needs b0 b1
            index |= new_tile_bits & 0b1100;
            index |= neighbour_bits & 0b0011;
        } else if edge == (1, 0) {
            // Right edge
            index |= new_tile_bits & 0b0101;
            index |= neighbour_bits & 0b1010;
        } else if edge == (0, -1) {
            // Bottom edge
            index |= new_tile_bits & 0b0011;
            index |= neighbour_bits & 0b1100;
        } else if edge == (-1, 0) {
            // Left edge
            index |= new_tile_bits & 0b1010;
            index |= neighbour_bits & 0b0101;
        } else if edge == (-1, 1) {
            // Top-left corner
            index |= new_tile_bits & 0b1000;
            index |= neighbour_bits & 0b0111;
        } else if edge == (1, 1) {
            // Top-right corner
            index |= new_tile_bits & 0b0100;
            index |= neighbour_bits & 0b1011;
        } else if edge == (-1, -1) {
            // Bottom-left corner
            index |= new_tile_bits & 0b0010;
            index |= neighbour_bits & 0b1101;
        } else if edge == (1, -1) {
            // Bottom-right corner
            index |= new_tile_bits & 0b0001;
            index |= neighbour_bits & 0b1110;
        }
        index
    }
    fn compute_tile_blocks(
        new_tile_blocks: [WorldObject; 4],
        neighbour_blocks: [WorldObject; 4],
        edge: (i8, i8),
    ) -> [WorldObject; 4] {
        // let mut index = Self::get_bits_from_block_type(block_type);
        // new tile will be 0b1111 i think
        let mut updated_blocks = neighbour_blocks;
        if edge == (0, 1) {
            // Top edge needs b0 b1
            updated_blocks[2] = new_tile_blocks[2];
            updated_blocks[3] = new_tile_blocks[3];
        } else if edge == (1, 0) {
            // Right edge
            updated_blocks[0] = new_tile_blocks[0];
            updated_blocks[2] = new_tile_blocks[2];
        } else if edge == (0, -1) {
            // Bottom edge
            updated_blocks[0] = new_tile_blocks[0];
            updated_blocks[1] = new_tile_blocks[1];
        } else if edge == (-1, 0) {
            // Left edge
            updated_blocks[1] = new_tile_blocks[1];
            updated_blocks[3] = new_tile_blocks[3];
        } else if edge == (-1, 1) {
            // Top-left corner
            updated_blocks[3] = new_tile_blocks[3];
        } else if edge == (1, 1) {
            // Top-right corner
            updated_blocks[2] = new_tile_blocks[2];
        } else if edge == (-1, -1) {
            // Bottom-left corner
            updated_blocks[1] = new_tile_blocks[1];
        } else if edge == (1, -1) {
            // Bottom-right corner
            updated_blocks[0] = new_tile_blocks[0];
        }
        updated_blocks
    }
    pub fn _change_tile_and_update_neighbours(
        tile_pos: TilePos,
        chunk_pos: IVec2,
        bits: u8,
        offset: u8,
        chunk_manager: &mut ResMut<ChunkManager>,
        commands: &mut Commands,
    ) {
        let block_type = Self::get_block_type_from_bits(bits, offset);

        let tile_entity_data = chunk_manager
            .chunk_tile_entity_data
            .get_mut(&TileMapPositionData {
                chunk_pos,
                tile_pos,
            });
        if let Some(tile_entity_data) = tile_entity_data {
            if let Some(mut e_commands) = commands.get_entity(tile_entity_data.entity.unwrap()) {
                e_commands.insert(TileTextureIndex((bits + offset).into()));
                tile_entity_data.block_type = block_type;
                Self::update_neighbour_tiles(tile_pos, commands, chunk_manager, chunk_pos, true);
            }
        }
    }
}
