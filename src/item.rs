use crate::animations::AnimationPosTracker;
use crate::assets::Graphics;
use crate::attributes::{Attack, BlockAttributeBundle, EquipmentAttributeBundle, Health};
use crate::world_generation::{
    ChunkObjectData, TileMapPositionData, WorldObjectEntityData, CHUNK_SIZE,
};
use crate::{AnimationTimer, GameParam, GameState, YSort};
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::{Collider, Sensor};

use rand::Rng;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Component)]
pub struct Breakable(pub Option<WorldObject>);

#[derive(Component)]
pub struct Block;
#[derive(Component)]
pub struct Equipment;
#[derive(Component, Debug, PartialEq, Copy, Clone)]
pub struct ItemStack(pub WorldObject, pub u8);

pub struct EquipmentMetaData {
    entity: Entity,
    obj: WorldObject,
    health: Health,
    attack: Attack,
}
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
    Foliage(Foliage),
    Sword,
    Log,
    Flint,
}
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component, Display)]
pub enum Foliage {
    Tree,
}

pub const PLAYER_EQUIPMENT_POSITIONS: [(f32, f32); 1] = [(-9., -5.); 1];

#[derive(Debug, Resource)]
pub struct WorldObjectResource {
    pub properties: HashMap<WorldObject, WorldObjectData>,
    pub drop_entities: HashMap<Entity, (ItemStack, Transform)>,
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
        Self {
            properties: HashMap::new(),
            drop_entities: HashMap::new(),
        }
    }
}
impl WorldObject {
    pub fn spawn(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        let sprite = game
            .graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&self)
            .unwrap_or_else(|| panic!("No graphic for object {self:?}"))
            .0
            .clone();
        //TODO: WIP FADING OUT ITEMS SHADER
        // let item = commands.spawn(MaterialMesh2dBundle {mesh: Mesh2dHandle(meshes.add(Mesh::from(shape::Quad { size: Vec2::new(32.,32.), flip: false }))),
        //  material:,
        //  transform:,
        //  ..Default::Default()});
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x,
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y,
            0.,
        );
        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: game.graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: position,
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("GroundItem"))
            .insert(BlockAttributeBundle {
                health: Health(100),
            })
            .insert(Block)
            .insert(YSort)
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
        game.chunk_manager.chunk_generation_data.insert(
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
    pub fn spawn_foliage(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x,
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y,
            0.,
        );
        let foliage_material = game
            .graphics
            .image_handle_map
            .as_ref()
            .unwrap()
            .get(&self)
            .unwrap()
            .0
            .clone();

        let item = commands
            .spawn(MaterialMesh2dBundle {
                mesh: game.meshes.add(Mesh::from(shape::Quad::default())).into(),
                transform: Transform {
                    translation: position,
                    scale: obj_data.size.extend(1.),
                    ..Default::default()
                },
                material: foliage_material,
                ..default()
            })
            .insert(Name::new("Foliage"))
            .insert(BlockAttributeBundle {
                health: Health(100),
            })
            .insert(Block)
            .insert(YSort)
            .insert(self)
            .id();
        if obj_data.breakable {
            commands
                .entity(item)
                .insert(Breakable(obj_data.breaks_into));
        }

        if obj_data.collider {
            commands
                .entity(item)
                .insert(Collider::cuboid(1. / 3.5, 1. / 4.5));
        }
        game.chunk_manager.chunk_generation_data.insert(
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
    pub fn spawn_and_save_block(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
        let item = self.spawn(commands, game, tile_pos, chunk_pos);

        let old_points = game.game_data.data.get(&(chunk_pos.x, chunk_pos.y));

        if let Some(old_points) = old_points {
            println!("SAVING NEW OBJ {self:?} {tile_pos:?}");
            let mut new_points = old_points.0.clone();
            new_points.push((tile_pos.x as f32, tile_pos.y as f32, self));

            game.game_data
                .data
                .insert((chunk_pos.x, chunk_pos.y), ChunkObjectData(new_points));
        }

        item
    }
    pub fn spawn_equipment_on_player(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
    ) -> Entity {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        let sprite = game
            .graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&self)
            .unwrap_or_else(|| panic!("No graphic for object {self:?}"))
            .0
            .clone();
        let player_data = &mut game.player_query.single_mut();
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position;
        let health = Health(100);
        let attack = Attack(20);
        if let Some(slot) = obj_data.equip_slot {
            position = Vec3::new(
                PLAYER_EQUIPMENT_POSITIONS[slot].0 + anchor.x * obj_data.size.x,
                PLAYER_EQUIPMENT_POSITIONS[slot].1 + anchor.y * obj_data.size.y,
                500. - (PLAYER_EQUIPMENT_POSITIONS[slot].1 + anchor.y * obj_data.size.y) * 0.1,
            );
            let item = commands
                .spawn(SpriteSheetBundle {
                    sprite,
                    texture_atlas: game.graphics.texture_atlas.as_ref().unwrap().clone(),
                    transform: Transform {
                        translation: position,
                        scale: Vec3::new(1., 1., 1.),
                        // rotation: Quat::from_rotation_z(0.8),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(EquipmentAttributeBundle { health, attack })
                .insert(Equipment)
                .insert(Name::new("EquipItem"))
                .insert(YSort)
                .insert(self)
                .id();

            player_data.1.main_hand_slot = Some(EquipmentMetaData {
                obj: self,
                entity: item,
                health,
                attack,
            });

            if obj_data.collider {
                println!("Add collider");
                commands
                    .entity(item)
                    .insert(Collider::cuboid(
                        obj_data.size.x / 3.5,
                        obj_data.size.y / 4.5,
                    ))
                    .insert(Sensor);
            }

            commands.entity(item).set_parent(player_data.0);

            item
        } else {
            panic!("No slot found for equipment");
        }
    }
    pub fn spawn_item_drop(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Entity {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        let sprite = game
            .graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&self)
            .unwrap_or_else(|| panic!("No graphic for object {self:?}"))
            .0
            .clone();
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let mut rng = rand::thread_rng();

        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x
                + rng.gen_range(-10. ..10.),
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y
                + rng.gen_range(-10. ..10.),
            500. - ((tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y
                + rng.gen_range(-10. ..10.))
                * 0.1,
        );
        let stack = ItemStack(self, rng.gen_range(1..4));
        let transform = Transform {
            translation: position,
            scale: Vec3::new(1., 1., 1.), // rotation: Quat::from_rotation_x(0.1),
            ..Default::default()
        };

        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: game.graphics.texture_atlas.as_ref().unwrap().clone(),
                transform,
                ..Default::default()
            })
            .insert(Name::new("DropItem"))
            .insert(stack)
            .insert(Collider::cuboid(8., 8.))
            .insert(Sensor)
            .insert(AnimationTimer(Timer::from_seconds(
                0.1,
                TimerMode::Repeating,
            )))
            .insert(AnimationPosTracker(0., 0., 0.3))
            .insert(YSort)
            .insert(self)
            .id();

        if obj_data.collider {
            commands.entity(item).insert(Collider::cuboid(
                obj_data.size.x / 3.5,
                obj_data.size.y / 4.5,
            ));
        }
        game.world_obj_data
            .drop_entities
            .insert(item, (stack, transform));
        item
    }
    pub fn attempt_to_break_item(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) {
        let obj_data = game
            .chunk_manager
            .chunk_generation_data
            .get(&TileMapPositionData {
                chunk_pos,
                tile_pos: TilePos {
                    x: tile_pos.x as u32,
                    y: tile_pos.y as u32,
                },
            })
            .unwrap();

        let main_hand_tool = &game.player_query.single().1.main_hand_slot;
        let b_data = game.block_query.get_mut(obj_data.entity).unwrap();

        if let Some(data) = game.world_obj_data.properties.get(&self) {
            if let Some(breaks_with) = data.breaks_with {
                if let Some(main_hand_tool) = main_hand_tool {
                    if main_hand_tool.obj == breaks_with {
                        let mut h = b_data.1;
                        h.0 -= main_hand_tool.attack.0;
                        if h.0 <= 0 {
                            Self::break_item(self, commands, game, tile_pos, chunk_pos)
                        }
                    }
                }
            } else {
                Self::break_item(self, commands, game, tile_pos, chunk_pos)
            }
        }
    }
    pub fn break_item(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) {
        let obj_data = game
            .chunk_manager
            .chunk_generation_data
            .get(&TileMapPositionData {
                chunk_pos,
                tile_pos: TilePos {
                    x: tile_pos.x as u32,
                    y: tile_pos.y as u32,
                },
            })
            .unwrap();
        println!(
            "{:?} {:?}",
            obj_data,
            (tile_pos.x as f32, tile_pos.y as f32, self),
        );

        if let Some(breaks_into_option) = game.world_obj_data.properties.get(&self) {
            commands.entity(obj_data.entity).despawn();
            if let Some(breaks_into) = breaks_into_option.breaks_into {
                breaks_into.spawn_item_drop(commands, game, tile_pos, chunk_pos);
            }
            game.chunk_manager
                .chunk_generation_data
                .remove(&TileMapPositionData {
                    chunk_pos,
                    tile_pos: TilePos {
                        x: tile_pos.x as u32,
                        y: tile_pos.y as u32,
                    },
                });
            let old_points = game
                .game_data
                .data
                .get(&(chunk_pos.x, chunk_pos.y))
                .unwrap();
            let updated_old_points = old_points
                .0
                .clone()
                .iter()
                .filter(|p| **p != (tile_pos.x as f32, tile_pos.y as f32, self))
                .copied()
                .collect::<Vec<(f32, f32, Self)>>();
            info!(
                "DELETING BLOCK {:?} {:?} {:?}",
                (tile_pos.x as f32, tile_pos.y as f32, self),
                updated_old_points.len(),
                old_points.0.len()
            );
            game.game_data.data.insert(
                (chunk_pos.x, chunk_pos.y),
                ChunkObjectData(updated_old_points.to_vec()),
            );
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
        let item_map = &&graphics.spritesheet_map;
        if let Some(item_map) = item_map {
            for (mut sprite, world_object) in to_update_query.iter_mut() {
                sprite.clone_from(
                    &item_map
                        .get(world_object)
                        .unwrap_or_else(|| panic!("No graphic for object {world_object:?}"))
                        .0,
                );
            }
        }
    }
}
