pub mod chunk;
pub mod dimension;
pub mod dungeon;
mod dungeon_generation;
pub mod generation;
mod noise_helpers;
mod tile;
pub mod world_helpers;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};

use bevy::{
    prelude::*,
    utils::{HashMap, HashSet},
};
use serde::{Deserialize, Serialize};

use crate::{enemy::spawner::ChunkSpawner, item::WorldObject};

use self::{
    chunk::ChunkPlugin,
    dimension::DimensionPlugin,
    dungeon_generation::{Bias, GridSize, NumSteps},
    generation::GenerationPlugin,
    tile::TilePlugin,
};

pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 32., y: 32. };
pub const CHUNK_SIZE: u32 = 16;
pub const MAX_VISIBILITY: u32 = (CHUNK_SIZE / 3) * TILE_SIZE.x as u32;
const CHUNK_CACHE_AMOUNT: i32 = 4;
pub const NUM_CHUNKS_AROUND_CAMERA: i32 = 2;

#[derive(Debug, Component, Resource, Clone)]
// for dimensions, chunks are child of D, and when swapping,
// save only obj data, tiles/chunks will regenerate from seed
// obj data is  saved in cm of dimension
// when dim is changed, despawn all children, but parent dim
// will keep its CM data with the obj data inside
// when we swap back, use the obj data to spawn teh objs back
// may not need to add obj data as comp to tile??
pub struct ChunkManager {
    // give Spawned comonent to chunk entity
    pub spawned_chunks: HashSet<IVec2>,
    // any chunk entity that exists is cached
    pub cached_chunks: HashSet<IVec2>,
    // give as comonent to each tile entity
    pub raw_chunk_data: HashMap<IVec2, RawChunkData>,
    // turn TileEntityData into comp for each tile
    pub chunk_tile_entity_data: HashMap<TileMapPositionData, TileEntityData>,
    // turn into comp for each tile
    pub chunk_generation_data: HashMap<TileMapPositionData, WorldObjectEntityData>,
    pub state: ChunkLoadingState,
    pub world_generation_params: WorldGeneration,
    pub spawner_data: HashMap<IVec2, Vec<ChunkSpawner>>,
}

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            spawned_chunks: HashSet::default(),
            cached_chunks: HashSet::default(),
            chunk_tile_entity_data: HashMap::new(),
            spawner_data: HashMap::new(),
            raw_chunk_data: HashMap::new(),
            state: ChunkLoadingState::Spawning,
            chunk_generation_data: HashMap::new(),
            world_generation_params: WorldGeneration::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChunkObjectData(pub Vec<(f32, f32, WorldObject)>);

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkLoadingState {
    Spawning,
    Caching,
    Despawning,
    None,
}

#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct RawChunkData {
    pub raw_chunk_bits: [[[u8; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
    pub raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize],
}
#[derive(Eq, Hash, PartialEq, Debug, Component, Clone)]
pub struct TileMapPositionData {
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}
#[derive(Eq, Hash, PartialEq, Debug, Clone)]
pub struct TileEntityData {
    pub entity: Option<Entity>,
    pub block_type: [WorldObject; 4],
    pub tile_bit_index: u8,
    pub texture_offset: u8,
}

#[derive(Eq, Hash, PartialEq, Debug, Clone)]

pub struct WorldObjectEntityData {
    pub object: WorldObject,
    pub entity: Entity,
    pub obj_bit_index: u8,
    pub texture_offset: u8,
}

#[derive(Default, Debug, Clone)]
pub struct WorldGeneration {
    pub water_frequency: f64,
    pub dungeon_stone_frequency: f64,
    pub sand_frequency: f64,
    pub dirt_frequency: f64,
    pub stone_frequency: f64,
    pub tree_frequency: f64,
}
pub struct WorldPlugin;
impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(GenerationPlugin)
            .add_plugin(ChunkPlugin)
            .add_plugin(DimensionPlugin)
            .add_plugin(TilePlugin)
            // .add_plugin(ResourceInspectorPlugin::<NumSteps>::default())
            // .add_plugin(ResourceInspectorPlugin::<GridSize>::default())
            // .add_plugin(ResourceInspectorPlugin::<Bias>::default())
            .init_resource::<NumSteps>()
            .init_resource::<GridSize>()
            .init_resource::<Bias>()
            .register_type::<NumSteps>()
            .register_type::<GridSize>()
            .register_type::<Bias>();
    }
}
