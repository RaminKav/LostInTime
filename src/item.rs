use crate::assets::Graphics;
use crate::world_generation::{
    ChunkManager, TileMapPositionData, WorldObjectEntityData, CHUNK_SIZE,
};
use crate::{Game, GameState, WORLD_SIZE};
use bevy::prelude::*;
use bevy::sprite::{MaterialMesh2dBundle, Mesh2dHandle};
use bevy::utils::HashMap;
use bevy_ecs_tilemap::tiles::TilePos;
use noise::{NoiseFn, Seedable, Simplex};

use serde::Deserialize;

#[derive(Component)]
pub struct Breakable(pub Option<WorldObject>);

#[derive(Component)]
pub struct Collider;
#[derive(Component)]
pub struct Size(pub Vec2);
/// The core enum of the game, lists everything that can be held or placed in the game
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Component)]
pub enum WorldObject {
    None,
    Grass,
    StoneHalf,
    StoneFull,
    StoneTop,
    Water,
    Sand,
    Tree,
}
#[derive(Debug, Resource)]
pub struct WorldObjectBreakData(HashMap<WorldObject, Option<WorldObject>>);
impl WorldObjectBreakData {
    fn new() -> Self {
        let mut m = HashMap::new();
        m.insert(WorldObject::StoneFull, Some(WorldObject::StoneHalf));
        m.insert(WorldObject::Tree, None);
        m.insert(WorldObject::StoneHalf, None);
        Self(m)
    }
}
impl WorldObject {
    pub fn spawn(
        self,
        commands: &mut Commands,
        break_data: &WorldObjectBreakData,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
        // println!("I SPAWNED A TREE AT {:?}", position);
        let item_map = &graphics.item_map;
        if let None = item_map {
            panic!("graphics not loaded");
        }
        let sprite = graphics
            .item_map
            .as_ref()
            .unwrap()
            .get(&self)
            .expect(&format!("No graphic for object {:?}", self))
            .0
            .clone();
        //TODO: WIP FADING OUT ITEMS SHADER
        // let item = commands.spawn(MaterialMesh2dBundle {mesh: Mesh2dHandle(meshes.add(Mesh::from(shape::Quad { size: Vec2::new(32.,32.), flip: false }))),
        //  material:,
        //  transform:,
        //  ..Default::Default()});
        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
            0.1,
        );
        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: position,
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("GroundItem"))
            .insert(self)
            .id();
        if let Some(b) = break_data.0.get(&self) {
            commands.entity(item).insert(Breakable(*b));
        }
        chunk_manager.chunk_generation_data.insert(
            TileMapPositionData {
                tile_pos: TilePos {
                    x: tile_pos.x as u32,
                    y: tile_pos.y as u32,
                },
                chunk_pos,
            },
            WorldObjectEntityData {
                object: self,
                entity: item,
            },
        );
        return item;

        // if let Some(breakable) = self.as_breakable() {
        //     commands.entity(item).insert(breakable);
        // }

        // if let Some(pickup) = self.as_pickup() {
        //     commands.entity(item).insert(pickup);
        // }

        // if self.grows_into().is_some() {
        //     commands.entity(item).insert(GrowthTimer {
        //         timer: Timer::from_seconds(3.0, false),
        //     });
        // }
    }
    pub fn spawn_with_collider(
        self,
        commands: &mut Commands,
        break_data: &WorldObjectBreakData,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        tile_pos: IVec2,
        chunk_pos: IVec2,
        size: Vec2,
    ) -> Entity {
        // println!("I SPAWNED A TREE AT {:?}", position);
        let item = self.spawn(
            commands,
            break_data,
            graphics,
            chunk_manager,
            tile_pos,
            chunk_pos,
        );
        commands.entity(item).insert(Collider);
        commands.entity(item).insert(Size(size));
        return item;
    }
    pub fn break_item(
        self,
        commands: &mut Commands,
        break_data: &WorldObjectBreakData,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) {
        // println!("I SPAWNED A TREE AT {:?}", position);
        let obj_data = chunk_manager
            .chunk_generation_data
            .get(&TileMapPositionData {
                chunk_pos,
                tile_pos: TilePos {
                    x: tile_pos.x as u32,
                    y: tile_pos.y as u32,
                },
            })
            .unwrap();

        if let Some(breaks_into_option) = break_data.0.get(&self) {
            commands.entity(obj_data.entity).despawn();
            if let Some(breaks_into) = breaks_into_option {
                let item = breaks_into.spawn_with_collider(
                    commands,
                    &break_data,
                    &graphics,
                    chunk_manager,
                    tile_pos,
                    chunk_pos,
                    Vec2::new(32., 48.), //TODO: add size to gen data
                );
                if let Some(b) = break_data.0.get(breaks_into) {
                    commands.entity(item).insert(Breakable(*b));
                }
                chunk_manager.chunk_generation_data.insert(
                    TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    },
                    WorldObjectEntityData {
                        object: *breaks_into,
                        entity: item,
                    },
                );
            } else {
                chunk_manager
                    .chunk_generation_data
                    .remove(&TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    });
            }
        }
    }
    // pub fn as_breakable(&self) -> Option<Breakable> {
    //     match self {
    //         WorldObject::Grass => Some(Breakable {
    //             object: WorldObject::Grass,
    //             turnsInto: Some(WorldObject::Dirt),
    //         }),
    //         WorldObject::Stone => Some(Breakable {
    //             object: WorldObject::Stone,
    //             turnsInto: Some(WorldObject::Coal),
    //         }),
    //         _ => None,
    //     }
    // }
}

impl Default for WorldObject {
    fn default() -> Self {
        WorldObject::None
    }
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectBreakData::new())
            .add_system_set(
                SystemSet::on_update(GameState::Main).with_system(Self::update_graphics),
                // .with_system(Self::world_object_growth),
            );
    }
}

impl ItemsPlugin {
    /// Keeps the graphics up to date for things that are harvested or grown
    fn update_graphics(
        mut to_update_query: Query<(&mut TextureAtlasSprite, &WorldObject), Changed<WorldObject>>,
        graphics: Res<Graphics>,
    ) {
        let item_map = &&graphics.item_map;
        if let Some(item_map) = item_map {
            for (mut sprite, world_object) in to_update_query.iter_mut() {
                sprite.clone_from(
                    &item_map
                        .get(world_object)
                        .expect(&format!("No graphic for object {:?}", world_object))
                        .0,
                );
            }
        }
    }
}
