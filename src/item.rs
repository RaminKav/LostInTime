use std::fmt;

use crate::animations::{AnimationPosTracker, AttackAnimationTimer};
use crate::assets::Graphics;
use crate::attributes::{AttributeChangeEvent, BlockAttributeBundle, Health, ItemAttributes};
use crate::inventory::{Inventory, InventoryItemStack, ItemStack};
use crate::ui::InventoryState;
use crate::world_generation::{
    ChunkObjectData, TileMapPositionData, WorldObjectEntityData, CHUNK_SIZE,
};
use crate::{AnimationTimer, GameParam, GameState, Limb, YSort};
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_inspector_egui::Inspectable;
use bevy_rapier2d::prelude::{Collider, Sensor};
use lazy_static::lazy_static;

use rand::Rng;
use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Component)]
pub struct Breakable(pub Option<WorldObject>);

#[derive(Component)]
pub struct Block;
#[derive(Component)]
pub struct Equipment(Limb);
#[derive(Component)]
pub struct MainHand;

//TODO: Convert attributes to a vec of attributes?
#[derive(Debug, Clone)]
pub struct EquipmentData {
    pub entity: Entity,
    pub obj: WorldObject,
}

#[derive(Component, Inspectable, Debug, PartialEq, Clone)]
pub struct ItemDisplayMetaData {
    pub name: String,
    pub desc: String,
    pub attributes: Vec<String>,
    pub durability: String,
}
#[derive(Component)]
pub struct Size(pub Vec2);
/// The core enum of the game, lists everything that can be held or placed in the game
#[derive(
    Debug, Inspectable, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component,
)]
pub enum WorldObject {
    None,
    Grass,
    StoneHalf,
    StoneFull,
    StoneTop,
    Water,
    Sand,
    Foliage(Foliage),
    Placeable(Placeable),
    Sword,
}
#[derive(
    Debug, Inspectable, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component, Display,
)]
pub enum Foliage {
    Tree,
}
impl Default for Foliage {
    fn default() -> Self {
        Self::Tree
    }
}
#[derive(
    Debug, Inspectable, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component,
)]
pub enum Placeable {
    Log,
    Flint,
}
impl fmt::Display for Placeable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Placeable::Log => write!(f, "Log"),
            Placeable::Flint => write!(f, "Flint"),
        }
    }
}
impl Default for Placeable {
    fn default() -> Self {
        Self::Flint
    }
}
lazy_static! {
    pub static ref PLAYER_EQUIPMENT_POSITIONS: HashMap<Limb, Vec2> =
        HashMap::from([(Limb::Hands, Vec2::new(-9., -5.))]);
}

#[derive(Debug, Resource)]
pub struct WorldObjectResource {
    pub properties: HashMap<WorldObject, WorldObjectData>,
    pub drop_entities: HashMap<Entity, (ItemStack, Transform)>,
}

//TODO: delete this and unify with WorldItemMetadata...
#[derive(Debug, Default)]
pub struct WorldObjectData {
    pub size: Vec2,
    pub anchor: Option<Vec2>,
    pub collider: bool,
    pub breakable: bool,
    pub breaks_into: Option<WorldObject>,
    pub breaks_with: Option<WorldObject>,
    /// 0 = main hand, 1 = head, 2 = chest, 3 = legs
    pub equip_slot: Option<Limb>,
    pub places_into: Option<WorldObject>,
}
impl WorldObjectResource {
    fn new() -> Self {
        Self {
            properties: HashMap::new(),
            drop_entities: HashMap::new(),
        }
    }
}
impl fmt::Display for WorldObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WorldObject::None => write!(f, ""),
            WorldObject::Grass => write!(f, "Grass"),
            WorldObject::StoneHalf => write!(f, "Stone Block"),
            WorldObject::StoneFull => write!(f, "Stone Block"),
            WorldObject::StoneTop => write!(f, "Stone Block"),
            WorldObject::Water => write!(f, "Water"),
            WorldObject::Sand => write!(f, "Sand"),
            WorldObject::Foliage(_) => write!(f, "Tree"),
            WorldObject::Placeable(p) => write!(f, "{}", p.to_string()),
            WorldObject::Sword => write!(f, "Basic Sword"),
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
    ) -> Option<Entity> {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        if game
            .chunk_manager
            .chunk_generation_data
            .get(&TileMapPositionData {
                tile_pos: TilePos {
                    x: tile_pos.x as u32,
                    y: tile_pos.y as u32,
                },
                chunk_pos,
            })
            .is_some()
        {
            warn!("Block {self:?} already exists on tile {tile_pos:?}, skipping...");
            return None;
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

        Some(item)
    }
    pub fn spawn_foliage(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Option<Entity> {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }

        if game
            .chunk_manager
            .chunk_generation_data
            .get(&TileMapPositionData {
                tile_pos: TilePos {
                    x: tile_pos.x as u32,
                    y: tile_pos.y as u32,
                },
                chunk_pos,
            })
            .is_some()
        {
            warn!("Block {self:?} already exists on tile {tile_pos:?}, skipping...");
            return None;
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
            .foliage_material_map
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

        Some(item)
    }
    pub fn spawn_and_save_block(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
    ) -> Option<Entity> {
        let item = match self {
            WorldObject::Foliage(_) => self.spawn_foliage(commands, game, tile_pos, chunk_pos),
            _ => self.spawn(commands, game, tile_pos, chunk_pos),
        };
        if let Some(item) = item {
            let old_points = game.game_data.data.get(&(chunk_pos.x, chunk_pos.y));

            if let Some(old_points) = old_points {
                let mut new_points = old_points.0.clone();
                new_points.push((tile_pos.x as f32, tile_pos.y as f32, self));

                game.game_data
                    .data
                    .insert((chunk_pos.x, chunk_pos.y), ChunkObjectData(new_points));
            }

            return Some(item);
        }
        None
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
        let player_e = game.player_query.single().0;
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position;
        let attributes = ItemAttributes {
            durability: 100,
            max_durability: 100,
            attack: 20,
            attack_cooldown: 0.4,
            ..Default::default()
        };
        let tooltips = attributes.get_tooltips();
        let durability_tooltip = attributes.get_durability_tooltip();

        if let Some(limb) = obj_data.equip_slot {
            position = Vec3::new(
                PLAYER_EQUIPMENT_POSITIONS[&limb].x + anchor.x * obj_data.size.x,
                PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y,
                500. - (PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y) * 0.1,
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
                .insert(attributes)
                .insert(ItemDisplayMetaData {
                    name: self.to_string(),
                    desc: "A cool piece of Equipment".to_string(),
                    attributes: tooltips,
                    durability: durability_tooltip,
                })
                .insert(Equipment(limb))
                .insert(Name::new("EquipItem"))
                .insert(self)
                .id();

            let mut item_entity = commands.entity(item);
            if obj_data.collider {
                item_entity
                    .insert(Collider::cuboid(
                        obj_data.size.x / 3.5,
                        obj_data.size.y / 4.5,
                    ))
                    .insert(Sensor);
            }
            if limb == Limb::Hands {
                item_entity.insert(AttackAnimationTimer(
                    Timer::from_seconds(0.125, TimerMode::Once),
                    0.,
                ));
            }
            item_entity.set_parent(player_e);

            item
        } else {
            panic!("No slot found for equipment");
        }
    }
    pub fn spawn_item_on_hand(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        inv_item_stack: &InventoryItemStack,
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
        let player_state = &mut game.game.player_state;
        let player_e = game.player_query.single().0;
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position;

        let limb = Limb::Hands;
        position = Vec3::new(
            PLAYER_EQUIPMENT_POSITIONS[&limb].x + anchor.x * obj_data.size.x,
            PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y,
            0.000000000001, //500. - (PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y) * 0.1,
        );
        //despawn old item if one exists
        if let Some(main_hand_data) = &player_state.main_hand_slot {
            commands.entity(main_hand_data.entity).despawn();
        }
        //spawn new item entity
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
            .insert(Equipment(limb))
            .insert(MainHand)
            .insert(Name::new("HeldItem"))
            .insert(inv_item_stack.item_stack.attributes.clone())
            .insert(self)
            .set_parent(player_e)
            .id();

        player_state.main_hand_slot = Some(EquipmentData {
            obj: self,
            entity: item,
        });
        let mut item_entity = commands.entity(item);

        if obj_data.collider {
            item_entity
                .insert(Collider::cuboid(
                    obj_data.size.x / 3.5,
                    obj_data.size.y / 2.,
                ))
                .insert(Sensor);
        }
        item_entity.insert(AttackAnimationTimer(
            Timer::from_seconds(0.125, TimerMode::Once),
            0.,
        ));

        item
    }
    pub fn spawn_item_drop(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: IVec2,
        chunk_pos: IVec2,
        count: usize,
        attributes: Option<ItemAttributes>,
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
        let drop_spread = 10.;

        let position = Vec3::new(
            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x
                + rng.gen_range(-drop_spread..drop_spread),
            (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y
                + rng.gen_range(-drop_spread..drop_spread),
            500. - ((tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y
                + rng.gen_range(-drop_spread..drop_spread))
                * 0.1,
        );
        //TODO: temp attr
        let attributes = if let Some(att) = attributes {
            att
        } else {
            ItemAttributes::default()
        };
        let tooltips = attributes.get_tooltips();
        let durability_tooltip = attributes.get_durability_tooltip();

        let stack = ItemStack {
            obj_type: self,
            attributes,
            metadata: ItemDisplayMetaData {
                name: self.to_string(),
                desc: "A cool item drop!".to_string(),
                attributes: tooltips,
                durability: durability_tooltip,
            },
            count,
        };
        let transform = Transform {
            translation: position,
            scale: Vec3::new(1., 1., 1.),
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
            .insert(stack.clone())
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
            .insert(item, (stack.clone(), transform));
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

        let main_hand_tool = &game.game.player_state.main_hand_slot;
        let b_data = game.block_query.get_mut(obj_data.entity).unwrap();

        if let Some(data) = game.world_obj_data.properties.get(&self) {
            //TODO: maybe move this logic to combat.rs? or move the hp loss from combat to here?
            if let Some(breaks_with) = data.breaks_with {
                if let Some(main_hand_tool) = main_hand_tool {
                    if main_hand_tool.obj == breaks_with {
                        let h = b_data.1;
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

        if let Some(breaks_into_option) = game.world_obj_data.properties.get(&self) {
            commands.entity(obj_data.entity).despawn();
            if let Some(breaks_into) = breaks_into_option.breaks_into {
                let mut rng = rand::thread_rng();
                breaks_into.spawn_item_drop(
                    commands,
                    game,
                    tile_pos,
                    chunk_pos,
                    rng.gen_range(1..4),
                    None,
                );
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
                SystemSet::on_update(GameState::Main)
                    .with_system(Self::update_graphics)
                    .with_system(Self::update_help_hotbar_item),
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
    fn update_help_hotbar_item(
        mut commands: Commands,
        mut game_param: GameParam,
        inv_state: Query<&mut InventoryState>,
        mut inv: Query<&mut Inventory>,
        item_stack_query: Query<&ItemAttributes>,
        mut att_event: EventWriter<AttributeChangeEvent>,
    ) {
        let active_hotbar_slot = inv_state.single().active_hotbar_slot;
        let active_hotbar_item = inv.single_mut().items[active_hotbar_slot].clone();
        let player_data = &mut game_param.game.player_state;
        let current_held_item_data = &player_data.main_hand_slot;
        if let Some(new_item) = active_hotbar_item {
            let new_item_obj = new_item.item_stack.obj_type;
            if let Some(current_item) = current_held_item_data {
                let curr_attributes = item_stack_query.get(current_item.entity).unwrap();
                let new_attributes = &new_item.item_stack.attributes;
                if new_item_obj != current_item.obj {
                    new_item_obj.spawn_item_on_hand(&mut commands, &mut game_param, &new_item);
                } else if curr_attributes != new_attributes {
                    commands
                        .entity(current_item.entity)
                        .insert(new_attributes.clone());
                }
            } else {
                new_item_obj.spawn_item_on_hand(&mut commands, &mut game_param, &new_item);
            }
            att_event.send(AttributeChangeEvent);
        } else if let Some(current_item) = current_held_item_data {
            commands.entity(current_item.entity).despawn();
            player_data.main_hand_slot = None;
            att_event.send(AttributeChangeEvent);
        }
    }
}
