pub mod chunk;
pub mod dimension;
pub mod dungeon;
mod dungeon_generation;
pub mod generation;
mod noise_helpers;
mod tile;
pub mod wall_auto_tile;
pub mod world_helpers;
pub mod y_sort;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};

use bevy::{prelude::*, utils::HashMap};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};

use crate::item::WorldObject;

use self::{
    chunk::{ChunkPlugin, ReflectedPos},
    dimension::DimensionPlugin,
    dungeon::DungeonPlugin,
    dungeon_generation::{Bias, GridSize, NumSteps},
    generation::GenerationPlugin,
    tile::TilePlugin,
    y_sort::YSortPlugin,
};

pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 32., y: 32. };
pub const CHUNK_SIZE: u32 = 16;
pub const MAX_VISIBILITY: u32 = (CHUNK_SIZE / 3) * TILE_SIZE.x as u32;
pub const NUM_CHUNKS_AROUND_CAMERA: i32 = 2;

#[derive(Debug, Component, Resource, Reflect, Default, Clone)]
#[reflect(Resource)]
// for dimensions, chunks are child of D, and when swapping,
// save only obj data, tiles/chunks will regenerate from seed
// obj data is  saved in cm of dimension
// when dim is changed, despawn all children, but parent dim
// will keep its CM data with the obj data inside
// when we swap back, use the obj data to spawn teh objs back
// may not need to add obj data as comp to tile??
pub struct ChunkManager {
    pub chunks: HashMap<ReflectedPos, Entity>,
    // turn into comp for each tile
    pub world_generation_params: WorldGeneration,
}

impl ChunkManager {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            world_generation_params: WorldGeneration::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChunkObjectData(pub Vec<(f32, f32, WorldObject)>);

#[derive(Eq, Hash, PartialEq, Debug, Component, Copy, Clone, Default, Reflect)]
#[reflect(Component)]
pub struct TileMapPositionData {
    //TODO: Add ::new() impl
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}

#[derive(
    Eq, Hash, Component, PartialEq, Debug, Clone, Default, Reflect, FromReflect, Schematic,
)]
#[reflect(Component, Schematic)]

pub struct WorldObjectEntityData {
    pub object: WorldObject,
    pub obj_bit_index: u8,
    pub texture_offset: u8,
}

#[derive(Default, Debug, Reflect, Clone)]
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
            .add_plugin(DungeonPlugin)
            .add_plugin(TilePlugin)
            .add_plugin(YSortPlugin)
            // .add_plugin(ResourceInspectorPlugin::<NumSteps>::default())
            // .add_plugin(ResourceInspectorPlugin::<GridSize>::default())
            // .add_plugin(ResourceInspectorPlugin::<Bias>::default())
            .init_resource::<NumSteps>()
            .init_resource::<GridSize>()
            .init_resource::<Bias>();
        // .register_type::<NumSteps>()
        // .register_type::<GridSize>()
        // .register_type::<Bias>();
    }
}
