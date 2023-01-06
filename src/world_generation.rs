use std::ops::Index;

use crate::assets::Graphics;
use crate::item::{WorldObject, WorldObjectResource};
use crate::{Game, GameState, ImageAssets, Player, WORLD_SIZE};
use bevy::prelude::*;
use bevy::ui::update;
use bevy::utils::HashMap;
use bevy::{math::Vec3Swizzles, utils::HashSet};
use bevy_ecs_tilemap::helpers::square_grid::neighbors::Neighbors;
use bevy_ecs_tilemap::prelude::*;
use bevy_inspector_egui::Inspectable;
use bevy_pkv::PkvStore;
use noise::{NoiseFn, Perlin, Seedable, Simplex};
use rand::rngs::ThreadRng;
use rand::Rng;
use serde::{Deserialize, Serialize};

pub struct WorldGenerationPlugin;
pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 32., y: 32. };
pub const CHUNK_SIZE: u32 = 64;
const CHUNK_CACHE_AMOUNT: i32 = 3;
const NUM_CHUNKS_AROUND_CAMERA: i32 = 2;

impl Plugin for WorldGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkManager::new())
            .add_system_set(
                SystemSet::on_enter(GameState::Main).with_system(Self::spawn_and_cache_init_chunks),
            )
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    // .with_system(Self::spawn_chunk)
                    .with_system(Self::spawn_chunks_around_camera)
                    .with_system(Self::despawn_outofrange_chunks),
            )
            .add_system_set(
                SystemSet::on_enter(GameState::Main).with_system(Self::spawn_test_objects),
            );
    }
}

#[derive(Debug, Resource)]
pub struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
    pub cached_chunks: HashSet<IVec2>,
    pub raw_chunk_data: HashMap<IVec2, RawChunkData>,
    pub chunk_tile_entity_data: HashMap<TileMapPositionData, TileEntityData>,
    pub chunk_generation_data: HashMap<TileMapPositionData, WorldObjectEntityData>,
    pub state: ChunkLoadingState,
}
#[derive(Serialize, Deserialize)]
pub struct WorldObjectData {
    data: String,
    name: String,
}

#[derive(Debug)]
pub enum ChunkLoadingState {
    Spawning,
    Caching,
    Despawning,
    None,
}

#[derive(Eq, Hash, PartialEq, Debug)]
pub struct RawChunkData {
    pub raw_chunk_bits: [[[u8; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
    pub raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
}
#[derive(Eq, Hash, PartialEq, Debug)]
pub struct TileMapPositionData {
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}
#[derive(Eq, Hash, PartialEq, Debug)]
pub struct TileEntityData {
    pub entity: Option<Entity>,
    pub tile_bit_index: u8,
    pub block_type: [WorldObject; 4],
    pub block_offset: u8,
}
#[derive(Eq, Hash, PartialEq, Debug)]

pub struct WorldObjectEntityData {
    pub object: WorldObject,
    pub entity: Entity,
}

impl ChunkManager {
    fn new() -> Self {
        Self {
            spawned_chunks: HashSet::default(),
            cached_chunks: HashSet::default(),
            chunk_tile_entity_data: HashMap::new(),
            raw_chunk_data: HashMap::new(),
            state: ChunkLoadingState::Spawning,
            chunk_generation_data: HashMap::new(),
        }
    }
}

impl WorldGenerationPlugin {
    fn cache_chunk(
        commands: &mut Commands,
        game: &Res<Game>,
        chunk_pos: IVec2,
        chunk_manager: &mut ResMut<ChunkManager>,
    ) {
        if chunk_manager.cached_chunks.contains(&chunk_pos) {
            return;
        }
        chunk_manager.cached_chunks.insert(chunk_pos);

        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let tile_pos = TilePos { x, y };
                chunk_manager.chunk_tile_entity_data.insert(
                    TileMapPositionData {
                        chunk_pos,
                        tile_pos,
                    },
                    TileEntityData {
                        entity: None,
                        tile_bit_index: 0b0000,
                        block_type: [WorldObject::Sand; 4],
                        block_offset: 0,
                    },
                );
            }
        }

        let mut raw_chunk_bits: [[[u8; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[[0; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        let mut raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
            [[[WorldObject::Sand; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                let tile_pos = TilePos { x, y };

                let (bits, index_shift, blocks) =
                    Self::get_tile_from_perlin_noise(game, chunk_pos, tile_pos);

                raw_chunk_bits[x as usize][y as usize] = bits;
                raw_chunk_blocks[x as usize][y as usize] = blocks;
                let block_bits = bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8;

                chunk_manager.chunk_tile_entity_data.insert(
                    TileMapPositionData {
                        chunk_pos,
                        tile_pos,
                    },
                    TileEntityData {
                        entity: None,
                        tile_bit_index: block_bits,
                        block_type: blocks,
                        block_offset: index_shift,
                    },
                );
            }
        }
        chunk_manager.raw_chunk_data.insert(
            chunk_pos,
            RawChunkData {
                raw_chunk_bits,
                raw_chunk_blocks,
            },
        );
        for y in 0..CHUNK_SIZE {
            for x in 0..CHUNK_SIZE {
                if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Water)
                    && raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Sand)
                {
                    Self::update_neighbour_tiles(
                        TilePos { x, y },
                        commands,
                        chunk_manager,
                        chunk_pos,
                        false,
                    );
                } else if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Grass) {
                    Self::update_this_tile(
                        TilePos { x, y },
                        16,
                        commands,
                        chunk_manager,
                        chunk_pos,
                    );
                }
            }
        }
    }
    fn spawn_chunk(
        commands: &mut Commands,
        sprite_sheet: &Res<ImageAssets>,
        game: &Res<Game>,
        chunk_pos: IVec2,
        chunk_manager: &mut ResMut<ChunkManager>,
    ) {
        let tilemap_size = TilemapSize {
            x: CHUNK_SIZE as u32,
            y: CHUNK_SIZE as u32,
        };
        let tile_size = TilemapTileSize {
            x: TILE_SIZE.x,
            y: TILE_SIZE.y,
        };
        let grid_size = tile_size.into();
        let map_type = TilemapType::default();

        let tilemap_entity = commands.spawn_empty().id();
        let mut tile_storage = TileStorage::empty(tilemap_size);
        if chunk_manager.cached_chunks.contains(&chunk_pos) {
            println!("Loading chunk {:?} from CACHE!", chunk_pos);

            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };
                    let tile_entity_data = chunk_manager
                        .chunk_tile_entity_data
                        .get(&TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        })
                        .unwrap();
                    if tile_pos.x == 0 && tile_pos.y == 0 && chunk_pos.x == 2 && chunk_pos.y == -2 {
                        println!(
                            "YETTIDY: {:?} {:?}",
                            tile_entity_data.tile_bit_index, tile_entity_data.block_offset
                        );
                    }
                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(tilemap_entity),
                            texture_index: TileTextureIndex(
                                (tile_entity_data.tile_bit_index + tile_entity_data.block_offset)
                                    .into(),
                            ),
                            ..Default::default()
                        })
                        .id();

                    chunk_manager
                        .chunk_tile_entity_data
                        .get_mut(&TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        })
                        .unwrap()
                        .entity = Some(tile_entity);
                    commands.entity(tilemap_entity).add_child(tile_entity);
                    tile_storage.set(&tile_pos, tile_entity);
                }
            }

            let transform = Transform::from_translation(Vec3::new(
                chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
                chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y,
                0.0,
            ));

            commands.entity(tilemap_entity).insert(TilemapBundle {
                grid_size,
                map_type,
                size: tilemap_size,
                storage: tile_storage,
                texture: TilemapTexture::Single(sprite_sheet.tiles_sheet.clone()),
                tile_size,
                transform,
                ..Default::default()
            });
            return;
        }
        println!("WARNING: chunk {:?} not in CACHE!", chunk_pos);
    }
    fn get_tile_from_perlin_noise(
        game: &Res<Game>,
        chunk_pos: IVec2,
        tile_pos: TilePos,
    ) -> ([u8; 4], u8, [WorldObject; 4]) {
        let noise_e = Perlin::new(1);
        let noise_e2 = Perlin::new(2);
        let noise_e3 = Perlin::new(3);

        let noise_m = Simplex::new(4);
        let noise_m2 = Simplex::new(5);
        let noise_m3 = Simplex::new(6);

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
            let base_oct = 1. / 10. / 8.;

            let e1 = (noise_e.get([x * base_oct, y * base_oct]));
            let e2 = (noise_e2.get([x * base_oct * 8., y * base_oct * 8.]));
            let e3 = (noise_e3.get([x * base_oct * 16., y * base_oct * 16.]));

            let e = f64::min(e1, f64::min(e2, e3) + 0.4) + 0.5;
            // let m = (noise_m.get([x * base_oct, ny * base_oct]) + 0.5)
            //     + 0.5 * (noise_m2.get([x * base_oct * 2., ny * base_oct * 2.]) + 0.5)
            //     + 0.25 * (noise_m3.get([x * base_oct * 3., ny * base_oct * 3.]) + 0.5);

            // let e = f64::powf(e / (1. + 0.5 + 0.25), 1.);
            // let m = f64::powf(m / (1. + 0.5 + 0.25), 1.);
            // print!("{:?}", e);
            // let m = f64::powf(m, 1.);
            let block = if e <= game.world_generation_params.water_frequency {
                WorldObject::Water
            } else if e <= game.world_generation_params.sand_frequency {
                WorldObject::Sand
            } else if e <= game.world_generation_params.dirt_frequency {
                WorldObject::Grass
            }
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
            if nx == 0. && ny == 1. {
                println!("yoooo: bits: {:?} blocks: {:?}", bits, blocks,);
            }
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

        if nx == 0. && ny == 1. {
            println!(
                "bits: {:?} blocks: {:?} shift: {:?} | f: {:?}",
                bits,
                blocks,
                index_shift,
                bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8
            );
        }
        (bits, index_shift, blocks)
    }

    // Use chunk manager as source of truth for index
    fn update_neighbour_tiles(
        new_tile_pos: TilePos,
        commands: &mut Commands,
        chunk_manager: &mut ResMut<ChunkManager>,
        chunk_pos: IVec2,
        update_entity: bool,
    ) {
        // return;
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
                            chunk_pos: chunk_pos,
                            tile_pos: new_tile_pos,
                        })
                        .unwrap();

                    if let Some(neighbour_tile_entity_data) = neighbour_tile_entity_data {
                        neighbour_entity = neighbour_tile_entity_data.entity;
                        neighbour_tile_offset = neighbour_tile_entity_data.block_offset;
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
                            block_offset: neighbour_tile_offset,
                        },
                    );
                }
            }
        }
    }
    fn update_this_tile(
        tile_pos: TilePos,
        mut tile_index_offset: u8,
        commands: &mut Commands,
        chunk_manager: &mut ResMut<ChunkManager>,
        chunk_pos: IVec2,
    ) {
        // return;
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
                        + target_block_entity_data.block_offset
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
                        Self::compute_tile_index(0b1111, bits, (dx * -1, dy * -1))
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
                            chunk_pos: chunk_pos,
                            tile_pos: tile_pos,
                        },
                        TileEntityData {
                            entity: None,
                            tile_bit_index: updated_bit_index,
                            block_type,
                            block_offset: tile_index_offset,
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

    fn compute_tile_index(new_tile_bits: u8, neighbour_bits: u8, edge: (i8, i8)) -> u8 {
        let mut index = 0;
        // new tile will be 0b1111 i think
        if edge == (0, 1) {
            // Top edge needs b0 b1
            index |= (new_tile_bits & 0b1100);
            index |= (neighbour_bits & 0b0011);
        } else if edge == (1, 0) {
            // Right edge
            index |= (new_tile_bits & 0b0101);
            index |= (neighbour_bits & 0b1010);
        } else if edge == (0, -1) {
            // Bottom edge
            index |= (new_tile_bits & 0b0011);
            index |= (neighbour_bits & 0b1100);
        } else if edge == (-1, 0) {
            // Left edge
            index |= (new_tile_bits & 0b1010);
            index |= (neighbour_bits & 0b0101);
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

    //TODO: update this to use new constants at top of file
    fn smooth_terrain(
        k: i8,
        tile_storage: &mut TileStorage,
        tile_index_grid: [[u32; 16]; 16],
        commands: &mut Commands,
    ) {
        // Create a new grid to hold the smoothed terrain
        let mut smooth_grid = [[10000; 16 as usize]; 16 as usize];

        // Loop over each tile in the grid
        for y in 0..16 {
            for x in 0..16 {
                let current_tile = tile_index_grid[x as usize][y as usize];
                // Count the number of adjacent tiles that are the same type as the current tile
                let mut adjacent_count = 0;
                let mut previous_tile: u32 = 10000;
                let mut smooth_tile: u32 = 10000;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if x + dx >= 0 && x + dx < 16 && y + dy >= 0 && y + dy < 16 {
                            let adj_tile = tile_index_grid[i32::abs(x + dx) as usize]
                                [i32::abs(y + dy) as usize];
                            if adj_tile == current_tile {
                                continue;
                            }
                            if adj_tile == previous_tile {
                                adjacent_count += 1;
                                if adjacent_count >= k {
                                    smooth_tile = adj_tile;
                                }
                            } else {
                                previous_tile = adj_tile;
                            }
                        }
                    }
                }
                // If at least 5 adjacent tiles are the same type, set the smooth_grid value to 1
                // (indicating that this tile should be the same type as the current tile)
                if adjacent_count >= k {
                    smooth_grid[y as usize][x as usize] = smooth_tile;
                }
            }
        }

        // Use the smooth_grid to set the tile types in the tile_storage
        for y in 0..16 {
            for x in 0..16 {
                let tile_pos = TilePos {
                    x: x.try_into().unwrap(),
                    y: y.try_into().unwrap(),
                };
                if smooth_grid[y][x] < 1000 {
                    // tile_storage.get(&tile_pos, smoothed_tile);
                    commands
                        .entity(tile_storage.get(&tile_pos).unwrap())
                        .insert(TileTextureIndex(smooth_grid[y][x]));
                }
            }
        }
    }

    pub fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
        // do this bc we want bottom left of the block to be 0,0 instead of centre
        let camera_pos = Vec2::new(
            camera_pos.x + (TILE_SIZE.x / 2.) as f32,
            camera_pos.y + (TILE_SIZE.y / 2.) as f32,
        );
        IVec2::new(
            (camera_pos.x / (CHUNK_SIZE as f32 * TILE_SIZE.x) as f32).floor() as i32,
            (camera_pos.y / (CHUNK_SIZE as f32 * TILE_SIZE.y) as f32).floor() as i32,
        )
    }
    pub fn camera_pos_to_block_pos(camera_pos: &Vec2) -> IVec2 {
        let camera_pos = Vec2::new(
            camera_pos.x + (TILE_SIZE.x / 2.) as f32,
            camera_pos.y + (TILE_SIZE.y / 2.) as f32,
        );
        let mut block_pos = IVec2::new(
            ((camera_pos.x % (CHUNK_SIZE as f32 * TILE_SIZE.x) as f32) / TILE_SIZE.x as f32).floor()
                as i32,
            ((camera_pos.y % (CHUNK_SIZE as f32 * TILE_SIZE.y) as f32) / TILE_SIZE.y as f32).floor()
                as i32,
        );
        // do this bc bottom left is 0,0
        if block_pos.x < 0 {
            block_pos.x += CHUNK_SIZE as i32
        }
        if block_pos.y < 0 {
            block_pos.y += CHUNK_SIZE as i32;
        }

        block_pos
    }
    fn spawn_and_cache_init_chunks(
        mut commands: Commands,
        camera_query: Query<&Transform, With<Camera>>,
        mut chunk_manager: ResMut<ChunkManager>,
        game: Res<Game>,
    ) {
        for transform in camera_query.iter() {
            let camera_chunk_pos = Self::camera_pos_to_chunk_pos(&transform.translation.xy());
            for y in
                (camera_chunk_pos.y - CHUNK_CACHE_AMOUNT)..(camera_chunk_pos.y + CHUNK_CACHE_AMOUNT)
            {
                for x in (camera_chunk_pos.x - CHUNK_CACHE_AMOUNT)
                    ..(camera_chunk_pos.x + CHUNK_CACHE_AMOUNT)
                {
                    if !chunk_manager.cached_chunks.contains(&IVec2::new(x, y)) {
                        println!("Caching chunk at {:?} {:?}", x, y);
                        chunk_manager.state = ChunkLoadingState::Spawning;
                        // chunk_manager.cached_chunks.insert(IVec2::new(x, y));
                        Self::cache_chunk(
                            &mut commands,
                            &game,
                            IVec2::new(x, y),
                            &mut chunk_manager,
                        );
                    }
                }
            }
        }
        chunk_manager.state = ChunkLoadingState::None;
    }

    fn spawn_chunks_around_camera(
        mut commands: Commands,
        sprite_sheet: Res<ImageAssets>,
        camera_query: Query<&Transform, With<Camera>>,
        mut chunk_manager: ResMut<ChunkManager>,
        game: Res<Game>,
    ) {
        for transform in camera_query.iter() {
            let camera_chunk_pos = Self::camera_pos_to_chunk_pos(&transform.translation.xy());
            for y in
                (camera_chunk_pos.y - CHUNK_CACHE_AMOUNT)..(camera_chunk_pos.y + CHUNK_CACHE_AMOUNT)
            {
                for x in (camera_chunk_pos.x - CHUNK_CACHE_AMOUNT)
                    ..(camera_chunk_pos.x + CHUNK_CACHE_AMOUNT)
                {
                    if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                        chunk_manager.state = ChunkLoadingState::Caching;
                        Self::cache_chunk(
                            &mut commands,
                            &game,
                            IVec2::new(x, y),
                            &mut chunk_manager,
                        );
                    }
                }
            }
            for y in (camera_chunk_pos.y - NUM_CHUNKS_AROUND_CAMERA)
                ..(camera_chunk_pos.y + NUM_CHUNKS_AROUND_CAMERA)
            {
                for x in (camera_chunk_pos.x - NUM_CHUNKS_AROUND_CAMERA)
                    ..(camera_chunk_pos.x + NUM_CHUNKS_AROUND_CAMERA)
                {
                    if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                        println!("spawning chunk at {:?} {:?}", x, y);
                        chunk_manager.state = ChunkLoadingState::Spawning;
                        chunk_manager.spawned_chunks.insert(IVec2::new(x, y));
                        Self::spawn_chunk(
                            &mut commands,
                            &sprite_sheet,
                            &game,
                            IVec2::new(x, y),
                            &mut chunk_manager,
                        );
                    }
                }
            }
        }
        chunk_manager.state = ChunkLoadingState::None;
    }

    fn despawn_outofrange_chunks(
        mut commands: Commands,
        camera_query: Query<&Transform, With<Camera>>,
        chunks_query: Query<(Entity, &Transform)>,
        mut chunk_manager: ResMut<ChunkManager>,
    ) {
        for camera_transform in camera_query.iter() {
            let max_distance = f32::hypot(
                CHUNK_SIZE as f32 * TILE_SIZE.x,
                CHUNK_SIZE as f32 * TILE_SIZE.y,
            );
            for (entity, chunk_transform) in chunks_query.iter() {
                let chunk_pos = chunk_transform.translation.xy();
                let distance = camera_transform.translation.xy().distance(chunk_pos);
                //TODO: calculate maximum possible distance for 2x2 chunksa
                let x = (chunk_pos.x as f32 / (CHUNK_SIZE as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y as f32 / (CHUNK_SIZE as f32 * TILE_SIZE.y)).floor() as i32;
                if distance > max_distance * 2. * NUM_CHUNKS_AROUND_CAMERA as f32
                    && chunk_manager.spawned_chunks.contains(&IVec2::new(x, y))
                {
                    println!("despawning chunk at {:?} {:?} d === {:?}", x, y, distance);
                    chunk_manager.state = ChunkLoadingState::Despawning;
                    chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        chunk_manager.state = ChunkLoadingState::None;
    }
    pub fn change_tile_and_update_neighbours(
        tile_pos: TilePos,
        chunk_pos: IVec2,
        bits: u8,
        offset: u8,
        chunk_manager: &mut ResMut<ChunkManager>,
        commands: &mut Commands,
    ) {
        let block_type = Self::get_block_type_from_bits(bits, offset);

        let mut tile_entity_data =
            chunk_manager
                .chunk_tile_entity_data
                .get_mut(&TileMapPositionData {
                    chunk_pos: chunk_pos,
                    tile_pos: tile_pos,
                });
        if let Some(tile_entity_data) = tile_entity_data {
            if let Some(mut e_commands) = commands.get_entity(tile_entity_data.entity.unwrap()) {
                e_commands.insert(TileTextureIndex((bits + offset).into()));
                tile_entity_data.block_type = block_type;
                Self::update_neighbour_tiles(tile_pos, commands, chunk_manager, chunk_pos, true);
            }
        }
    }

    fn spawn_test_objects(
        mut commands: Commands,
        graphics: Res<Graphics>,
        world_obj_data: Res<WorldObjectResource>,
        mut chunk_manager: ResMut<ChunkManager>,
        mut pkv: ResMut<PkvStore>,
    ) {
        let tree_points;
        let mut tree_children = Vec::new();
        if let Ok(data) = pkv.get::<WorldObjectData>("data") {
            println!("LOADING OLD DATA@@@@@@@@@@@@@@");
            info!("Welcome back {}", data.name);
            tree_points = serde_json::from_str(&data.data).unwrap();
        } else {
            println!("STORING NEW DATA@@@@@@@@@@@@@@");
            tree_points = poisson_disk_sampling(1. * TILE_SIZE.x as f64, 30, rand::thread_rng());
            let data = WorldObjectData {
                data: serde_json::to_string(&tree_points).unwrap(),
                name: "RAMIN".to_string(),
            };
            pkv.set("data", &data).expect("failed to store data");
        }
        for tp in tree_points {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&tp);
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&tp);
            // Vec3::new(
            //     (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            //     (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
            //     0.1,
            // )
            let tree = WorldObject::Tree.spawn_with_collider(
                &mut commands,
                &world_obj_data,
                &graphics,
                &mut chunk_manager,
                tile_pos,
                chunk_pos,
                Vec2::new(32., 32.),
            );
            tree_children.push(tree);
        }
        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            // .push_children(&children)
            .push_children(&tree_children);
    }
}

fn poisson_disk_sampling(r: f64, k: i8, mut rng: ThreadRng) -> Vec<Vec2> {
    // TODO: fix this to work w 4 quadrants -/+
    let n = 2.;
    // the final set of points to return
    let mut points: Vec<Vec2> = vec![];
    // the currently "Active" set of points
    let mut active: Vec<Vec2> = vec![];
    let p0 = Vec2::new(
        rng.gen_range(0..WORLD_SIZE) as f32,
        rng.gen_range(0..WORLD_SIZE) as f32,
    );

    let cell_size = f64::floor(r / f64::sqrt(n));
    let num_cell: usize = (f64::ceil(WORLD_SIZE as f64 / cell_size) + 1.) as usize;
    let mut grid: Vec<Vec<Option<Vec2>>> = vec![vec![None; num_cell]; num_cell];

    let insert_point = |g: &mut Vec<Vec<Option<Vec2>>>, p: Vec2| {
        let xi: usize = f64::floor(p.x as f64 / cell_size) as usize;
        let yi: usize = f64::floor(p.y as f64 / cell_size) as usize;
        g[xi][yi] = Some(p);
    };

    let is_valid_point = move |g: &Vec<Vec<Option<Vec2>>>, p: Vec2| -> bool {
        // make sure p is on screen
        if p.x < 0. || p.x > WORLD_SIZE as f32 || p.y < 0. || p.y > WORLD_SIZE as f32 {
            return false;
        }

        // check neighboring eight cells
        let xi: f64 = f64::floor(p.x as f64 / cell_size);
        let yi: f64 = f64::floor(p.y as f64 / cell_size);
        let i0 = usize::max((xi - 1.) as usize, 0);
        let i1 = usize::min((xi + 1.) as usize, num_cell - 1. as usize);
        let j0 = usize::max((yi - 1.) as usize, 0);
        let j1 = usize::min((yi + 1.) as usize, num_cell - 1. as usize);

        for i in i0..=i1 {
            for j in j0..=j1 {
                if let Some(sample_point) = g[i][j] {
                    if sample_point.distance(p) < r as f32 {
                        return false;
                    }
                }
            }
        }
        true
    };

    insert_point(&mut grid, p0);
    points.push(p0);
    active.push(p0);
    while active.len() > 0 {
        let i = rng.gen_range(0..=(active.len() - 1));
        let p = active.get(i).unwrap();
        let mut found = false;

        for _ in 0..k {
            // get a random angle
            let theta: f64 = rng.gen_range(0. ..360.);
            let new_r = rng.gen_range(r..(2. * r));

            // create new point from randodm angle r distance away from p
            let new_px = p.x as f64 + new_r * theta.to_radians().cos();
            let new_py = p.y as f64 + new_r * theta.to_radians().sin();
            let new_p = Vec2::new(new_px as f32, new_py as f32);

            if !is_valid_point(&grid, new_p) {
                continue;
            }

            //add the new point to our lists and break
            points.push(new_p);
            insert_point(&mut grid, new_p);
            active.push(new_p);
            found = true;
            break;
        }

        if !found {
            active.remove(i);
        }
    }

    points
}

//TODO: figure out why spawning chunks causes it to lag/glitch
