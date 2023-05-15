use crate::animations::{AnimationPosTracker, AttackAnimationTimer};
use crate::assets::{Graphics, WorldObjectData};
use crate::attributes::{AttributeChangeEvent, BlockAttributeBundle, Health, ItemAttributes};
use crate::combat::ObjBreakEvent;
use crate::inventory::{Inventory, InventoryItemStack, ItemStack};
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::InventoryState;
use crate::world::{ChunkObjectData, TileMapPositionData, WorldObjectEntityData, CHUNK_SIZE};
use crate::{AnimationTimer, GameParam, GameState, Limb, YSort};
use bevy::prelude::*;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::tiles::TilePos;
use std::fmt;

mod crafting;
mod loot_table;
pub use crafting::*;
pub use loot_table::*;

use bevy_rapier2d::prelude::{Collider, Sensor};
use lazy_static::lazy_static;

use rand::Rng;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

use self::crafting::CraftingPlugin;

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

#[derive(Component, Reflect, FromReflect, Debug, PartialEq, Clone)]
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
    Debug, FromReflect, Reflect, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component,
)]
pub enum WorldObject {
    None,
    Grass,
    Wall(Wall),
    DungeonStone,
    Water,
    Sand,
    Flint,
    Foliage(Foliage),
    Placeable(Placeable),
    Sword,
}

#[derive(
    Debug,
    FromReflect,
    Reflect,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Serialize,
    Deserialize,
    Component,
    Display,
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
    Debug,
    Reflect,
    FromReflect,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Serialize,
    Deserialize,
    Component,
    Display,
    EnumIter,
)]
pub enum Wall {
    Stone,
}
impl Default for Wall {
    fn default() -> Self {
        Self::Stone
    }
}

#[derive(
    Debug, FromReflect, Reflect, PartialEq, Eq, Clone, Copy, Hash, Serialize, Deserialize, Component,
)]
pub enum Placeable {
    Log,
}
impl fmt::Display for Placeable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Placeable::Log => write!(f, "Log"),
        }
    }
}
impl Default for Placeable {
    fn default() -> Self {
        Self::Log
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
            WorldObject::Wall(_) => write!(f, "Stone Wall"),
            WorldObject::DungeonStone => write!(f, "Dungeon Stone Block"),
            WorldObject::Water => write!(f, "Water"),
            WorldObject::Sand => write!(f, "Sand"),
            WorldObject::Foliage(_) => write!(f, "Tree"),
            WorldObject::Placeable(p) => write!(f, "{}", p.to_string()),
            WorldObject::Sword => write!(f, "Basic Sword"),
            WorldObject::Flint => write!(f, "Flint"),
        }
    }
}
impl WorldObject {
    //TODO: turn this into event
    pub fn spawn(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: TilePos,
        chunk_pos: IVec2,
    ) -> Option<Entity> {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        if game
            .get_obj_entity_at_tile(TileMapPositionData {
                tile_pos,
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
            .clone();
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let position = Vec3::new(
            (tile_pos.x as i32 * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x,
            (tile_pos.y as i32 * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
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
            .insert(WorldObjectEntityData {
                object: self,
                obj_bit_index: 0,
                texture_offset: 0,
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

        Some(item)
    }
    pub fn spawn_wall(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: TilePos,
        chunk_pos: IVec2,
    ) -> Option<Entity> {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        let pos = TileMapPositionData {
            tile_pos,
            chunk_pos,
        };
        if game.get_obj_entity_at_tile(pos.clone()).is_some() {
            warn!("Block {self:?} already exists on tile {tile_pos:?}, skipping...");
            return None;
        }
        match self {
            WorldObject::Wall(_) => {
                let obj_data = game.world_obj_data.properties.get(&self).unwrap();
                let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
                let position = Vec3::new(
                    // (tile_pos.x as i32 * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                    //     + anchor.x * obj_data.size.x,
                    // (tile_pos.y as i32 * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                    //     + anchor.y * obj_data.size.y,
                    0., 0., 1.,
                );
                let item = commands
                    .spawn(SpriteSheetBundle {
                        texture_atlas: game.graphics.wall_texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: position,
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(Name::new("Wall"))
                    .insert(BlockAttributeBundle {
                        health: Health(100),
                    })
                    .insert(Wall::Stone)
                    .insert(WorldObjectEntityData {
                        object: self,
                        obj_bit_index: 0,
                        texture_offset: 0,
                    })
                    // .insert(YSort)
                    .insert(pos)
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
                Some(item)
            }
            _ => {
                error!("Tried to spawn non-wall WorldObject as a Wall!");
                None
            }
        }
    }
    pub fn spawn_foliage(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: TilePos,
        chunk_pos: IVec2,
    ) -> Option<Entity> {
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }

        if game
            .get_obj_entity_at_tile(TileMapPositionData {
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
            (tile_pos.x as i32 * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x,
            (tile_pos.y as i32 * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
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
            .insert(WorldObjectEntityData {
                object: self,
                obj_bit_index: 0,
                texture_offset: 0,
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

        Some(item)
    }
    pub fn spawn_and_save_block(
        self,
        commands: &mut Commands,
        game: &mut GameParam,
        tile_pos: TilePos,
        chunk_pos: IVec2,
        mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    ) -> Option<Entity> {
        let item = match self {
            WorldObject::Foliage(_) => self.spawn_foliage(commands, game, tile_pos, chunk_pos),
            WorldObject::Wall(_) => self.spawn_wall(commands, game, tile_pos, chunk_pos),
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
            minimap_event.send(UpdateMiniMapEvent);
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
                .insert(YSort)
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
        //TODO: extract this out to helper fn vvvv
        let has_icon = game.graphics.icons.as_ref().unwrap().get(&self);
        let sprite = if let Some(icon) = has_icon {
            icon.clone()
        } else {
            game.graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&self)
                .unwrap_or_else(|| panic!("No graphic for object {self:?}"))
                .clone()
        };

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
            .insert(YSort)
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
        tile_pos: TilePos,
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
            .clone();
        let obj_data = game.world_obj_data.properties.get(&self).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let mut rng = rand::thread_rng();
        let drop_spread = 10.;

        let position = Vec3::new(
            (tile_pos.x as i32 * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32
                + anchor.x * obj_data.size.x
                + rng.gen_range(-drop_spread..drop_spread),
            (tile_pos.y as i32 * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
                + anchor.y * obj_data.size.y
                + rng.gen_range(-drop_spread..drop_spread),
            500. - ((tile_pos.y as i32 * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
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
    pub fn get_minimap_color(&self) -> (u8, u8, u8) {
        match self {
            WorldObject::None => (255, 70, 255),
            WorldObject::Grass => (113, 133, 51),
            WorldObject::Wall(_) => (171, 155, 142),
            WorldObject::DungeonStone => (53, 53, 57),
            WorldObject::Water => (87, 72, 82),
            WorldObject::Sand => (210, 201, 165),
            WorldObject::Foliage(_) => (119, 116, 59),
            WorldObject::Placeable(_) => (255, 70, 255),
            WorldObject::Sword => (255, 70, 255),
            WorldObject::Flint => (255, 70, 255),
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
            .add_plugin(CraftingPlugin)
            .add_plugin(LootTablePlugin)
            .add_systems(
                (
                    Self::update_graphics,
                    Self::break_item,
                    Self::update_held_hotbar_item,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

impl ItemsPlugin {
    pub fn break_item(
        mut commands: Commands,
        mut game: GameParam,
        mut obj_break_events: EventReader<ObjBreakEvent>,
        mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    ) {
        for broken in obj_break_events.iter() {
            if let Some(breaks_into_option) = game.world_obj_data.properties.get(&broken.obj) {
                commands.entity(broken.entity).despawn();
                //TODO: remove SpawnedObject comp from parent tile
                if let Some(breaks_into) = breaks_into_option.breaks_into {
                    let mut rng = rand::thread_rng();
                    breaks_into.spawn_item_drop(
                        &mut commands,
                        &mut game,
                        broken.tile_pos,
                        broken.chunk_pos,
                        rng.gen_range(1..4),
                        None,
                    );
                }
            }
            let old_points = game
                .game_data
                .data
                .get(&(broken.chunk_pos.x, broken.chunk_pos.y))
                .unwrap();
            let updated_old_points = old_points
                .0
                .clone()
                .iter()
                .filter(|p| {
                    **p != (
                        broken.tile_pos.x as f32,
                        broken.tile_pos.y as f32,
                        broken.obj,
                    )
                })
                .copied()
                .collect::<Vec<(f32, f32, WorldObject)>>();

            game.game_data.data.insert(
                (broken.chunk_pos.x, broken.chunk_pos.y),
                ChunkObjectData(updated_old_points.to_vec()),
            );
            minimap_event.send(UpdateMiniMapEvent);
        }
    }
    /// Keeps the graphics up to date for things that are harvested or grown
    fn update_graphics(
        mut to_update_query: Query<
            (&mut TextureAtlasSprite, &WorldObject),
            (Changed<WorldObject>, Without<Wall>),
        >,
        graphics: Res<Graphics>,
    ) {
        let item_map = &&graphics.spritesheet_map;
        if let Some(item_map) = item_map {
            for (mut sprite, world_object) in to_update_query.iter_mut() {
                let has_icon = graphics.icons.as_ref().unwrap().get(&world_object);
                let new_sprite = if let Some(icon) = has_icon {
                    icon
                } else {
                    &item_map
                        .get(world_object)
                        .unwrap_or_else(|| panic!("No graphic for object {world_object:?}"))
                };
                sprite.clone_from(new_sprite);
            }
        }
    }
    fn update_held_hotbar_item(
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
