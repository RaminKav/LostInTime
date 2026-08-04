use bevy::prelude::*;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};
use interpolation::lerp;

use crate::{item::WorldObject, world::ISLAND_SIZE, GameParam};

use super::{
    chunk::TileSpriteData,
    noise_helpers::{self, CachedNoiseGenerators},
    world_helpers::get_neighbour_tile,
    TileMapPosition, WorldGeneration, CHUNK_SIZE,
};

pub struct TilePlugin;
impl Plugin for TilePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, Self::update_tile_sprite);
    }
}

impl TilePlugin {
    fn update_tile_sprite(
        tiles: Query<(Entity, &TileSpriteData), Changed<TileSpriteData>>,
        mut commands: Commands,
    ) {
        for (tile_e, tile_data) in tiles.iter() {
            commands.entity(tile_e).insert(TileTextureIndex(
                (tile_data.tile_bit_index + tile_data.texture_offset).into(),
            ));
        }
    }
    /// Get tile data from perlin noise using cached generators (optimized version).
    pub fn get_tile_from_perlin_noise_cached(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        tile_pos: TilePos,
        generators: &CachedNoiseGenerators,
    ) -> ([u8; 4], u8, [WorldObject; 4]) {
        let x = tile_pos.x as f64;
        let y = tile_pos.y as f64;
        let nx = (x as i32 + chunk_pos.x * CHUNK_SIZE as i32) as f64;
        let ny = (y as i32 + chunk_pos.y * CHUNK_SIZE as i32) as f64;

        let mut bits = [0, 0, 0, 0];
        let mut blocks = [
            WorldObject::GrassTile,
            WorldObject::GrassTile,
            WorldObject::GrassTile,
            WorldObject::GrassTile,
        ];

        // Inline sample function to use cached generators
        let sample = |x: f64, y: f64| -> (u8, WorldObject) {
            if world_generation_params.stone_frequency > 0. {
                return (0, WorldObject::StoneTile);
            }
            let e = noise_helpers::get_perlin_noise_for_tile_cached(x, y, generators);
            let e = Self::apply_distance_function_to_tile(x as f32, y as f32, e);
            let block = if e <= world_generation_params.water_frequency {
                WorldObject::WaterTile
            } else {
                WorldObject::GrassTile
            };
            let block_bits: u8 = if block == WorldObject::GrassTile {
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

        if blocks.contains(&WorldObject::StoneTile) {
            index_shift = 16;
        }
        (bits, index_shift, blocks)
    }

    /// Legacy function that creates generators per-call (slower, for backwards compatibility)
    pub fn get_tile_from_perlin_noise(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        tile_pos: TilePos,
        seed: u64,
    ) -> ([u8; 4], u8, [WorldObject; 4]) {
        let generators = CachedNoiseGenerators::new(seed);
        Self::get_tile_from_perlin_noise_cached(
            world_generation_params,
            chunk_pos,
            tile_pos,
            &generators,
        )
    }
    pub fn apply_distance_function_to_tile(x: f32, y: f32, e: f64) -> f64 {
        // Add angular noise to distance for irregular (non-circular) coastline
        let angle = (y as f64).atan2(x as f64);
        let coastline_noise = (angle * 5.0).sin() * 0.08 + (angle * 11.0).cos() * 0.04;

        let base_d = (Vec2::new(x, y).distance(Vec2::ZERO)) / ISLAND_SIZE;
        let d = (base_d as f64 + coastline_noise).max(0.0);

        // Spawn protection: reduce water near spawn (0,0), increase further out
        // Distance from spawn in tiles (not normalized)
        let spawn_distance = Vec2::new(x, y).length();
        // Protection zone: 0-40 tiles from spawn (about 2.5 chunks)
        let spawn_protection_radius = 20.0;
        // Protection factor: 1.0 at spawn, 0.0 at protection_radius and beyond
        let spawn_protection =
            (1.0 - (spawn_distance / spawn_protection_radius).min(1.0)).max(0.0) as f64;
        // Add land bias near spawn (reduces water probability)
        let spawn_bias = spawn_protection * 0.25 as f64;

        // land_factor: 1.0 at center, 0.0 at boundary, negative outside (forces water)
        let land_factor = 1.0 - d;

        // Variable noise contribution: high at center (allows ponds), low at edges (clean coastline)
        // d=0 (center): 0.7, d=1 (edge): 0.4
        let noise_weight = 0.7 - d.min(1.0) * 0.3;

        // Variable land influence: low at center (allows ponds), high at edges (clean coastline)
        // d=0 (center): 0.1, d=1 (edge): 0.9
        let land_influence = 0.1 + d.min(1.0) * 0.8;

        // Variable bias: low at center (allows ponds), higher at edges (less water)
        // d=0 (center): 0.0, d=1 (edge): 0.15
        // But reduce overall bias to allow more water, and add spawn protection
        let bias = d.min(1.0) * 0.1 + spawn_bias;

        // Blend: noise creates ponds, land_factor shapes the island
        // Reduce overall result slightly to allow more water formation
        let result = e * noise_weight + land_factor * land_influence + bias - 0.05;
        result.clamp(0.0, 1.0)
    }
    pub fn _update_neighbour_tiles(
        new_tile_pos: TilePos,
        commands: &mut Commands,
        game: &mut GameParam,
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
                    let neighbour_tile_blocks;
                    let neighbour_raw_tile_blocks;

                    if game.get_chunk_entity(adjusted_chunk_pos).is_none() {
                        continue;
                    }
                    let neighbour_tile_entity_data = game.get_tile_data(TileMapPosition::new(
                        adjusted_chunk_pos,
                        neighbour_tile_pos,
                    ));
                    let new_tile_entity_data = game
                        .get_tile_data(TileMapPosition::new(chunk_pos, new_tile_pos))
                        .unwrap();

                    if let Some(neighbour_tile_entity_data) = neighbour_tile_entity_data {
                        neighbour_tile_blocks = neighbour_tile_entity_data.block_type;
                        neighbour_raw_tile_blocks = neighbour_tile_entity_data.raw_block_type;
                    } else {
                        continue;
                    }
                    let mut updated_bit_index;
                    let updated_blocks = Self::compute_tile_blocks(
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
                    commands
                        .entity(
                            game.get_tile_entity(TileMapPosition::new(
                                adjusted_chunk_pos,
                                neighbour_tile_pos,
                            ))
                            .unwrap(),
                        )
                        .insert(TileSpriteData {
                            tile_bit_index: updated_bit_index,
                            block_type: updated_block_type,
                            texture_offset: neighbour_tile_offset,
                            raw_block_type: neighbour_raw_tile_blocks,
                        });
                }
            }
        }
    }
    pub fn update_this_tile(
        commands: &mut Commands,
        tile_pos: TilePos,
        tile_index_offset: u8,
        game: &mut GameParam,
        chunk_pos: IVec2,
    ) {
        let target_block_entity_data = game
            .get_tile_data(TileMapPosition::new(chunk_pos, tile_pos))
            .unwrap();
        let mut updated_bits = target_block_entity_data.tile_bit_index;
        let updated_index = tile_index_offset;
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                // only use neighbours that have at least one water bitt

                let TileMapPosition {
                    chunk_pos: adjusted_chunk_pos,
                    tile_pos: neighbour_tile_pos,
                    ..
                } = get_neighbour_tile(TileMapPosition::new(chunk_pos, tile_pos), (dx, dy));

                let neighbour_pos = TileMapPosition::new(adjusted_chunk_pos, neighbour_tile_pos);
                if !(dx == 0 && dy == 0) {
                    if game.get_chunk_entity(adjusted_chunk_pos).is_none() {
                        continue;
                    }

                    let neighbour_block_entity_data = game.get_tile_data(neighbour_pos).unwrap();
                    let neighbour_bits: u8 = neighbour_block_entity_data
                        .block_type
                        .iter()
                        .enumerate()
                        .map(|(i, b)| match b {
                            WorldObject::WaterTile => u8::pow(2, i as u32),
                            _ => 0_u8,
                        })
                        .sum();

                    // only continue for tiles with water
                    updated_bits = if target_block_entity_data
                        .raw_block_type
                        .contains(&WorldObject::WaterTile)
                    {
                        let my_blocks = Self::compute_tile_blocks(
                            Self::get_block_type_from_bits(updated_bits, 0),
                            neighbour_block_entity_data.block_type,
                            (dx, dy),
                        );
                        Self::get_bits_from_block_type(my_blocks).0 & updated_bits
                    } else if target_block_entity_data
                        .raw_block_type
                        .contains(&WorldObject::GrassTile)
                        && updated_index != 0
                    {
                        Self::compute_tile_bits(updated_bits, neighbour_bits, (dx, dy))
                    } else {
                        continue;
                    };
                }
            }
        }
        let block_type = Self::get_block_type_from_bits(updated_bits, updated_index);
        commands
            .entity(
                game.get_tile_entity(TileMapPosition::new(chunk_pos, tile_pos))
                    .unwrap(),
            )
            .insert(TileSpriteData {
                tile_bit_index: updated_bits,
                block_type,
                texture_offset: updated_index,
                raw_block_type: target_block_entity_data.raw_block_type,
            });
    }
    fn get_block_type_from_bits(bits: u8, _offset: u8) -> [WorldObject; 4] {
        let used_blocks = (WorldObject::GrassTile, WorldObject::WaterTile);

        let mut block_type: [WorldObject; 4] = [WorldObject::GrassTile; 4];
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
        let offset = 0;
        let mut bits = 0b0000;

        bits |= if block_type[0] == WorldObject::WaterTile {
            0b0001
        } else {
            0b0000
        };
        bits |= if block_type[1] == WorldObject::WaterTile {
            0b0010
        } else {
            0b0000
        };
        bits |= if block_type[2] == WorldObject::WaterTile {
            0b0100
        } else {
            0b0000
        };
        bits |= if block_type[3] == WorldObject::WaterTile {
            0b1000
        } else {
            0b0000
        };

        (bits, offset)
    }
    pub fn compute_tile_bits(new_tile_bits: u8, neighbour_tile_bits: u8, edge: (i8, i8)) -> u8 {
        let mut bits = 0;
        // new tile will be 0b1111 i think
        if edge == (0, 1) {
            // Top edge needs b0 b1
            bits = new_tile_bits
                | match neighbour_tile_bits & 0b1100 {
                    0b1100 => 0b0011,
                    0b0100 => 0b0001,
                    0b1000 => 0b0010,
                    _ => 0b0000,
                };
        } else if edge == (1, 0) {
            // Right edge
            bits = new_tile_bits
                | match neighbour_tile_bits & 0b0101 {
                    0b0101 => 0b1010,
                    0b0100 => 0b1000,
                    0b0001 => 0b0010,
                    _ => 0b0000,
                };
        } else if edge == (0, -1) {
            // Bottom edge
            bits = new_tile_bits
                | match neighbour_tile_bits & 0b0011 {
                    0b0011 => 0b1100,
                    0b0001 => 0b0100,
                    0b0010 => 0b1000,
                    _ => 0b0000,
                };
        } else if edge == (-1, 0) {
            // Left edge
            bits = new_tile_bits
                | match neighbour_tile_bits & 0b1010 {
                    0b1010 => 0b0101,
                    0b1000 => 0b0100,
                    0b0010 => 0b0001,
                    _ => 0b0000,
                };
        } else if edge == (-1, 1) {
            // Top-left corner
            bits |= neighbour_tile_bits & 0b1000;
            bits = new_tile_bits | if bits == 0b1000 { 0b0001 } else { 0b0000 };
        } else if edge == (1, 1) {
            // Top-right corner
            bits |= neighbour_tile_bits & 0b0100;
            bits = new_tile_bits | if bits == 0b0100 { 0b0010 } else { 0b0000 };
        } else if edge == (-1, -1) {
            // Bottom-left corner
            bits |= neighbour_tile_bits & 0b0010;
            bits = new_tile_bits | if bits == 0b0010 { 0b0100 } else { 0b0000 };
        } else if edge == (1, -1) {
            // Bottom-right corner
            bits |= neighbour_tile_bits & 0b0001;
            bits = new_tile_bits | if bits == 0b0001 { 0b1000 } else { 0b0000 };
        }
        bits
    }
    fn compute_tile_blocks(
        new_tile_blocks: [WorldObject; 4],
        neighbour_blocks: [WorldObject; 4],
        edge: (i8, i8),
    ) -> [WorldObject; 4] {
        // 1 -1
        let mut updated_blocks = new_tile_blocks;
        if edge == (0, 1) {
            // Top edge needs b0 b1
            updated_blocks[0] = neighbour_blocks[2];
            updated_blocks[1] = neighbour_blocks[3];
        } else if edge == (1, 0) {
            // Right edge
            updated_blocks[1] = neighbour_blocks[0];
            updated_blocks[3] = neighbour_blocks[2];
        } else if edge == (0, -1) {
            // Bottom edge
            updated_blocks[2] = neighbour_blocks[0];
            updated_blocks[3] = neighbour_blocks[1];
        } else if edge == (-1, 0) {
            // Left edge
            updated_blocks[0] = neighbour_blocks[1];
            updated_blocks[2] = neighbour_blocks[3];
        } else if edge == (-1, 1) {
            // Top-left corner
            updated_blocks[0] = neighbour_blocks[3];
        } else if edge == (1, 1) {
            // Top-right corner
            updated_blocks[1] = neighbour_blocks[2];
        } else if edge == (-1, -1) {
            // Bottom-left corner
            updated_blocks[2] = neighbour_blocks[1];
        } else if edge == (1, -1) {
            // Bottom-right corner
            updated_blocks[3] = neighbour_blocks[0];
        }
        updated_blocks
    }
    pub fn _change_tile_and_update_neighbours(
        tile_pos: TilePos,
        chunk_pos: IVec2,
        bits: u8,
        offset: u8,
        game: &mut GameParam,
        commands: &mut Commands,
    ) {
        let block_type = Self::get_block_type_from_bits(bits, offset);

        let tile_entity_data = game.get_tile_data_mut(TileMapPosition::new(chunk_pos, tile_pos));

        if let Some(mut tile_entity_data) = tile_entity_data {
            tile_entity_data.block_type = block_type;
            Self::_update_neighbour_tiles(tile_pos, commands, game, chunk_pos, true);
        }
    }
}
