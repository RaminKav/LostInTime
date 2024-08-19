pub mod chunk;
pub mod dimension;
pub mod dungeon;
mod dungeon_generation;
pub mod generation;
mod noise_helpers;
pub mod portal;
pub mod tile;
pub mod wall_auto_tile;
pub mod world_helpers;
pub mod y_sort;
use std::fmt::Display;
use std::fmt::Formatter;

use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};

use bevy::{prelude::*, utils::HashMap};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use portal::handle_player_near_portal;
use serde::{Deserialize, Serialize};
use world_helpers::tile_pos_to_world_pos;

use crate::{item::WorldObject, schematic::SchematicType};

use self::{
    chunk::ChunkPlugin,
    dimension::DimensionPlugin,
    dungeon::DungeonPlugin,
    dungeon_generation::{Bias, GridSize, NumSteps},
    generation::GenerationPlugin,
    tile::TilePlugin,
    world_helpers::get_neighbour_tile,
    y_sort::YSortPlugin,
};

pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 16., y: 16. };
pub const CHUNK_SIZE: u32 = 16;
pub const ISLAND_SIZE: f32 = CHUNK_SIZE as f32 * 6.;
pub const MAX_VISIBILITY: u32 = (2.3 * (CHUNK_SIZE as f32 / 2.) * TILE_SIZE.x) as u32;
pub const NUM_CHUNKS_AROUND_CAMERA: i32 = 1;

#[derive(Serialize, Deserialize, Debug)]
pub struct ChunkObjectData(pub Vec<(f32, f32, WorldObject)>);

/// A component that represents a position in the tilemap. The `quadrant` is a number from 0 to 3
/// where 0 is top left, 1 is top right, 2 is bottom left, 3 is bottom right
#[derive(
    Eq,
    Hash,
    PartialEq,
    Debug,
    Component,
    Copy,
    Clone,
    Default,
    Reflect,
    FromReflect,
    Serialize,
    Deserialize,
)]
#[reflect(Component)]
pub struct TileMapPosition {
    pub chunk_pos: IVec2,

    #[serde(with = "TilePosSerde")]
    pub tile_pos: TilePos,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "TilePos")]
struct TilePosSerde {
    pub x: u32,
    pub y: u32,
}

impl TileMapPosition {
    pub fn new(chunk_pos: IVec2, tile_pos: TilePos) -> Self {
        Self {
            chunk_pos,
            tile_pos,
        }
    }

    pub fn matches_tile(&self, other: &TileMapPosition) -> bool {
        self.chunk_pos == other.chunk_pos && self.tile_pos == other.tile_pos
    }
    pub fn get_neighbour_tiles_for_medium_objects(&self) -> Vec<Self> {
        vec![
            get_neighbour_tile(*self, (1, 1)),
            get_neighbour_tile(*self, (0, 1)),
            get_neighbour_tile(*self, (1, 0)),
        ]
    }
    pub fn distance(&self, other: &TileMapPosition) -> f32 {
        let p = tile_pos_to_world_pos(*self, true);
        let other_p = tile_pos_to_world_pos(*other, true);
        p.distance(other_p)
    }
}
impl Display for TileMapPosition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chunk: {:?}, Tile: {:?}", self.chunk_pos, self.tile_pos)
    }
}

#[derive(
    Eq, Hash, Component, PartialEq, Debug, Clone, Default, Reflect, FromReflect, Schematic,
)]
#[reflect(Component, Schematic)]

pub struct WallTextureData {
    pub obj_bit_index: u8,
    pub texture_offset: u8,
}

#[derive(Resource, Schematic, Reflect, FromReflect, Default, Debug, Clone)]
#[reflect(Schematic)]
#[schematic(kind = "resource")]
pub struct WorldGeneration {
    pub water_frequency: f64,
    pub stone_frequency: f64,
    pub sand_frequency: f64,
    pub dirt_frequency: f64,
    pub forest_params: ForestGenerationParams,
    pub stone_wall_frequency: f64,
    pub schematic_frequencies: HashMap<SchematicType, f64>,
    pub object_generation_frequencies: HashMap<WorldObject, f64>,
    pub obj_allowed_tiles_map: HashMap<WorldObject, Vec<WorldObject>>,
}

#[derive(Component, Schematic, Reflect, FromReflect, Default, Debug, Clone)]
#[reflect(Schematic)]
pub struct ForestGenerationParams {
    pub tree_spacing_radius: f32,
    pub tree_density: f32,
    pub forest_radius: f32,
    pub max_trees_per_forest: usize,
    pub tree_weights: HashMap<WorldObject, f32>,
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
            .init_resource::<Bias>()
            .add_system(handle_player_near_portal);
        // .register_type::<NumSteps>()
        // .register_type::<GridSize>()
        // .register_type::<Bias>();
    }
}
