use crate::animations::AnimationPosTracker;
use crate::assets::Graphics;
use crate::world_generation::{
    ChunkManager, ChunkObjectData, GameData, TileMapPositionData, WorldObjectEntityData, CHUNK_SIZE,
};
use crate::{AnimationTimer, GameState, Player};
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::Collider;

use serde::{Deserialize, Serialize};

#[derive(Component)]
pub struct Breakable(pub Option<WorldObject>);

#[derive(Component)]
pub struct Equipment;
#[derive(Component)]
pub struct DropItem;
#[derive(Component)]
pub struct Size(pub Vec2);
/// The core enum of the game, lists everything that can be held or placed in the game
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component)]
pub enum WorldObject {
    None,
    Grass,
    StoneHalf,
    StoneFull,
    StoneTop,
    Water,
    Sand,
    Tree,
    Sword,
    Log,
    Flint,
}

pub const PLAYER_EQUIPMENT_POSITIONS: [(f32, f32); 1] = [(-3., -5.); 1];

#[derive(Debug, Resource)]
pub struct WorldObjectResource {
    pub data: HashMap<WorldObject, WorldObjectData>,
}

#[derive(Debug, Default)]
pub struct WorldObjectData {
    pub size: Vec2,
    pub anchor: Option<Vec2>,
    pub collider: bool,
    pub breakable: bool,
    pub breaks_into: Option<WorldObject>,
    pub breaks_with: Option<WorldObject>,
    pub equip_slot: Option<usize>,
}
impl WorldObjectResource {
    fn new() -> Self {
        let m = HashMap::new();
        Self { data: m }
    }
}
impl WorldObject {
    pub fn spawn(
        self,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
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
        let obj_data = world_obj_res.data.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x,
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y,
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
        if obj_data.breakable {
            commands
                .entity(item)
                .insert(Breakable(obj_data.breaks_into));
        }

        if obj_data.collider {
            commands.entity(item).insert(Collider::cuboid(
                obj_data.size.x / 3.5,
                obj_data.size.y / 4.5,
            ));
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

        item
    }
    pub fn spawn_and_save(
        self,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        game_data: &mut GameData,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
        let item = self.spawn(
            commands,
            world_obj_res,
            graphics,
            chunk_manager,
            tile_pos,
            chunk_pos,
        );

        let old_points = game_data.data.get(&(chunk_pos.x, chunk_pos.y));

        if let Some(old_points) = old_points {
            println!("SAVING NEW OBJ {:?} {:?}", self, tile_pos);
            let mut new_points = old_points.0.clone();
            new_points.push((tile_pos.x as f32, tile_pos.y as f32, self));

            game_data
                .data
                .insert((chunk_pos.x, chunk_pos.y), ChunkObjectData(new_points));
        }

        return item;
    }
    pub fn spawn_with_collider(
        self,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        tile_pos: IVec2,
        chunk_pos: IVec2,
        size: Vec2,
    ) -> Entity {
        let item = self.spawn(
            commands,
            world_obj_res,
            graphics,
            chunk_manager,
            tile_pos,
            chunk_pos,
        );

        commands.entity(item).insert(Size(size));
        return item;
    }
    pub fn spawn_equipment_on_player(
        self,
        mut player_query: Query<(Entity, &mut Player)>,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
    ) -> Entity {
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
        let player_data = &mut player_query.single_mut();
        let obj_data = world_obj_res.data.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position;
        if let Some(slot) = obj_data.equip_slot {
            position = Vec3::new(
                PLAYER_EQUIPMENT_POSITIONS[slot].0 + anchor.x * obj_data.size.x,
                PLAYER_EQUIPMENT_POSITIONS[slot].1 + anchor.y * obj_data.size.y,
                0.1,
            );
            player_data.1.main_hand_slot = Some(self)
        } else {
            panic!("No slot found for equipment");
        }
        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: position,
                    scale: Vec3::new(0.8, 0.8, 0.8),
                    // rotation: Quat::from_rotation_x(0.1),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Equipment)
            .insert(Name::new("EquipItem"))
            .insert(self)
            .id();

        if obj_data.collider {
            commands.entity(item).insert(Collider::cuboid(
                obj_data.size.x / 3.5,
                obj_data.size.y / 4.5,
            ));
        }

        commands.entity(item).set_parent(player_data.0);

        item
    }
    pub fn spawn_item_drop(
        self,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
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
        let obj_data = world_obj_res.data.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x,
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y,
            0.1,
        );
        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: position,
                    scale: Vec3::new(0.5, 0.5, 0.5),
                    // rotation: Quat::from_rotation_x(0.1),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("DropItem"))
            .insert(DropItem)
            .insert(AnimationTimer(Timer::from_seconds(
                0.1,
                TimerMode::Repeating,
            )))
            .insert(AnimationPosTracker(0., 0., 0.3))
            .insert(self)
            .id();

        if obj_data.collider {
            commands.entity(item).insert(Collider::cuboid(
                obj_data.size.x / 3.5,
                obj_data.size.y / 4.5,
            ));
        }

        item
    }
    pub fn attempt_to_break_item(
        self,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        game_data: &mut GameData,
        tile_pos: IVec2,
        chunk_pos: IVec2,
        with_item: Option<WorldObject>,
    ) {
        if let Some(data) = world_obj_res.data.get(&self) {
            if with_item == data.breaks_with {
                Self::break_item(
                    self,
                    commands,
                    world_obj_res,
                    graphics,
                    chunk_manager,
                    game_data,
                    tile_pos,
                    chunk_pos,
                )
            }
        }
    }
    pub fn break_item(
        self,
        commands: &mut Commands,
        world_obj_res: &WorldObjectResource,
        graphics: &Graphics,
        chunk_manager: &mut ChunkManager,
        game_data: &mut GameData,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) {
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

        if let Some(breaks_into_option) = world_obj_res.data.get(&self) {
            commands.entity(obj_data.entity).despawn();
            if let Some(breaks_into) = breaks_into_option.breaks_into {
                let old_points = game_data.data.get(&(chunk_pos.x, chunk_pos.y)).unwrap();
                let updated_old_points = old_points
                    .0
                    .clone()
                    .iter()
                    .filter(|p| **p != (tile_pos.x as f32, tile_pos.y as f32, self))
                    .map(|p| *p)
                    .collect::<Vec<(f32, f32, Self)>>();
                info!(
                    "DELETING BLOCK {:?} {:?} {:?}",
                    (tile_pos.x as f32, tile_pos.y as f32, self),
                    updated_old_points.len(),
                    old_points.0.len()
                );
                game_data.data.insert(
                    (chunk_pos.x, chunk_pos.y),
                    ChunkObjectData(updated_old_points.to_vec()),
                );
                breaks_into.spawn_item_drop(
                    commands,
                    &world_obj_res,
                    &graphics,
                    tile_pos,
                    chunk_pos,
                );
                //TODO: store appropriate block data.
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
                let old_points = game_data.data.get(&(chunk_pos.x, chunk_pos.y)).unwrap();
                let updated_old_points = old_points
                    .0
                    .clone()
                    .iter()
                    .filter(|p| **p != (tile_pos.x as f32, tile_pos.y as f32, self))
                    .map(|p| *p)
                    .collect::<Vec<(f32, f32, Self)>>();
                info!(
                    "DELETING BLOCK {:?} {:?} {:?}",
                    (tile_pos.x as f32, tile_pos.y as f32, self),
                    updated_old_points.len(),
                    old_points.0.len()
                );
                game_data.data.insert(
                    (chunk_pos.x, chunk_pos.y),
                    ChunkObjectData(updated_old_points.to_vec()),
                );
            }
        }
    }
}

impl Default for WorldObject {
    fn default() -> Self {
        WorldObject::None
    }
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectResource::new())
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
