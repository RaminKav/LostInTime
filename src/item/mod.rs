use crate::animations::{AnimationPosTracker, AttackAnimationTimer};
use crate::assets::{Graphics, WorldObjectData};
use crate::attributes::{AttributeChangeEvent, ItemAttributes};
use crate::combat::ObjBreakEvent;
use crate::inputs::FacingDirection;
use crate::inventory::{Inventory, InventoryItemStack, ItemStack};
use crate::proto::proto_param::ProtoParam;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::InventoryState;
use crate::world::generation::WallBreakEvent;
use crate::world::world_helpers::{camera_pos_to_chunk_pos, world_pos_to_tile_pos};
use crate::world::CHUNK_SIZE;
use crate::{
    custom_commands::CommandsExt, player::Limb, AnimationTimer, CustomFlush, GameParam, GameState,
    YSort,
};
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::{ProtoCommands, Prototypes, ReflectSchematic, Schematic};

mod crafting;
mod loot_table;
pub mod melee;
pub mod projectile;
pub use crafting::*;
pub use loot_table::*;

use bevy_rapier2d::prelude::{Collider, Sensor};
use lazy_static::lazy_static;

use rand::Rng;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, IntoStaticStr};

use self::crafting::CraftingPlugin;
use self::projectile::RangedAttackPlugin;

#[derive(Component, Reflect, FromReflect, Schematic)]
#[reflect(Schematic)]
pub struct Breakable(pub Option<WorldObject>);
#[derive(Component, Reflect, FromReflect, Schematic)]
#[reflect(Schematic)]
pub struct BreaksWith(pub WorldObject);

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]

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

#[derive(Component, Reflect, FromReflect, Schematic, Debug, Default, PartialEq, Clone)]
#[reflect(Schematic)]
pub struct ItemDisplayMetaData {
    pub name: String,
    pub desc: String,
}
#[derive(Component)]
pub struct Size(pub Vec2);
/// The core enum of the game, lists everything that can be held or placed in the game
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
    Schematic,
    IntoStaticStr,
    Display,
    Default,
)]
#[reflect(Component, Schematic)]
pub enum WorldObject {
    #[default]
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
    BasicStaff,
    FireStaff,
    DualStaff,
    Dagger,
    Fireball,
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
    Schematic,
    IntoStaticStr,
    Display,
)]
#[reflect(Component, Schematic)]
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
    Schematic,
    Display,
    IntoStaticStr,
    EnumIter,
)]
#[reflect(Component, Schematic)]
pub enum Wall {
    StoneWall,
}
impl Default for Wall {
    fn default() -> Self {
        Self::StoneWall
    }
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
    IntoStaticStr,
    Display,
    Schematic,
)]
#[reflect(Component, Schematic)]
pub enum Placeable {
    Log,
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

impl WorldObject {
    pub fn spawn_and_save_block(
        self,
        proto_commands: &mut ProtoCommands,
        prototypes: &Prototypes,
        pos: Vec2,
        minimap_event: &mut EventWriter<UpdateMiniMapEvent>,
        proto_param: &mut ProtoParam,
        game: &GameParam,
        commands: &mut Commands,
    ) -> Option<Entity> {
        if let Some(_existing_object) = game.get_obj_entity_at_tile(world_pos_to_tile_pos(pos)) {
            warn!("obj exists here {pos}");
            return None;
        }
        let item = match self {
            WorldObject::Foliage(obj) => {
                proto_commands.spawn_object_from_proto(obj, pos, prototypes, proto_param)
            }
            WorldObject::Wall(obj) => {
                proto_commands.spawn_object_from_proto(obj, pos, prototypes, proto_param)
            }
            _ => None,
        };
        if let Some(item) = item {
            //TODO: do what old game data did, add obj to registry
            let chunk_pos = camera_pos_to_chunk_pos(&pos);
            commands
                .entity(item)
                .set_parent(*game.get_chunk_entity(chunk_pos).unwrap());
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
        proto: ProtoParam,
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
        let is_facing_left = player_state.direction == FacingDirection::Left;

        let limb = Limb::Hands;
        let position = Vec3::new(
            PLAYER_EQUIPMENT_POSITIONS[&limb].x
                + anchor.x * obj_data.size.x
                + if is_facing_left { 0. } else { 11. },
            PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y,
            0.01, //500. - (PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y) * 0.1,
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
                visibility: Visibility::Visible,
                ..Default::default()
            })
            .insert(Equipment(limb))
            .insert(MainHand)
            .insert(Name::new("HeldItem"))
            .insert(Sensor)
            .insert(inv_item_stack.item_stack.attributes.clone())
            .insert(Collider::cuboid(obj_data.size.x / 2., obj_data.size.y / 2.))
            .insert(self)
            .insert(AttackAnimationTimer(
                Timer::from_seconds(0.125, TimerMode::Once),
                0.,
            ))
            .set_parent(player_e)
            .id();

        player_state.main_hand_slot = Some(EquipmentData {
            obj: self,
            entity: item,
        });
        let mut item_entity = commands.entity(item);
        if let Some(melee) = proto.is_item_melee_weapon(self) {
            item_entity.insert(melee.clone());
        }
        if let Some(ranged) = proto.is_item_ranged_weapon(self) {
            item_entity.insert(ranged.clone());
        }

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

        let stack = ItemStack {
            obj_type: self,
            attributes,
            metadata: ItemDisplayMetaData {
                name: self.to_string(),
                desc: "A cool item drop!".to_string(),
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
            //TODO: double colliders??
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
            _ => (255, 70, 255),
        }
    }
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectResource::new())
            .add_plugin(CraftingPlugin)
            .add_plugin(RangedAttackPlugin)
            .add_plugin(LootTablePlugin)
            .add_systems(
                (
                    Self::update_graphics,
                    Self::break_item.before(CustomFlush),
                    Self::update_held_hotbar_item,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl ItemsPlugin {
    pub fn break_item(
        mut commands: Commands,
        mut game: GameParam,
        mut obj_break_events: EventReader<ObjBreakEvent>,
        mut minimap_event: EventWriter<UpdateMiniMapEvent>,
        mut wall_break_event: EventWriter<WallBreakEvent>,
    ) {
        for broken in obj_break_events.iter() {
            if let Some(breaks_into_option) = game.world_obj_data.properties.get(&broken.obj) {
                commands.entity(broken.entity).despawn();
                println!("break");
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

            if let WorldObject::Wall(_) = broken.obj {
                wall_break_event.send(WallBreakEvent {
                    chunk_pos: broken.chunk_pos,
                    tile_pos: broken.tile_pos,
                })
            }
            minimap_event.send(UpdateMiniMapEvent);
        }
    }
    /// Keeps the graphics up to date for things that are spawned from proto, or change Obj type
    fn update_graphics(
        mut to_update_query: Query<
            (Entity, &mut TextureAtlasSprite, &WorldObject),
            (Changed<WorldObject>, Without<Wall>),
        >,
        game: GameParam,
        mut commands: Commands,
        graphics: Res<Graphics>,
    ) {
        let item_map = &&graphics.spritesheet_map;
        if let Some(item_map) = item_map {
            for (e, mut sprite, world_object) in to_update_query.iter_mut() {
                let has_icon = graphics.icons.as_ref().unwrap().get(&world_object);
                let new_sprite = if let Some(icon) = has_icon {
                    icon
                } else {
                    &item_map
                        .get(world_object)
                        .unwrap_or_else(|| panic!("No graphic for object {world_object:?}"))
                };
                commands
                    .entity(e)
                    .insert(game.graphics.texture_atlas.as_ref().unwrap().clone());
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
        proto: ProtoParam,
    ) {
        let active_hotbar_slot = inv_state.single().active_hotbar_slot;
        let active_hotbar_item = inv.single_mut().items[active_hotbar_slot].clone();
        let player_data = &mut game_param.game.player_state;
        let prev_held_item_data = &player_data.main_hand_slot;
        if let Some(new_item) = active_hotbar_item {
            let new_item_obj = new_item.item_stack.obj_type;
            if let Some(current_item) = prev_held_item_data {
                let curr_attributes = item_stack_query.get(current_item.entity).unwrap();
                let new_attributes = &new_item.item_stack.attributes;
                if new_item_obj != current_item.obj {
                    new_item_obj.spawn_item_on_hand(
                        &mut commands,
                        &mut game_param,
                        &new_item,
                        proto,
                    );
                    att_event.send(AttributeChangeEvent);
                } else if curr_attributes != new_attributes {
                    commands
                        .entity(current_item.entity)
                        .insert(new_attributes.clone());
                    att_event.send(AttributeChangeEvent);
                }
            } else {
                new_item_obj.spawn_item_on_hand(&mut commands, &mut game_param, &new_item, proto);
                att_event.send(AttributeChangeEvent);
            }
        } else if let Some(current_item) = prev_held_item_data {
            commands.entity(current_item.entity).despawn();
            player_data.main_hand_slot = None;
            att_event.send(AttributeChangeEvent);
        }
    }
}
