use crate::animations::AttackAnimationTimer;
use crate::assets::{SpriteSize, WorldObjectData};
use crate::attributes::ItemAttributes;
use crate::colors::{
    BLACK, BLUE, BROWN, DARK_GREEN, LIGHT_GREEN, LIGHT_GREY, UI_GRASS_GREEN, YELLOW,
};
use crate::combat::ObjBreakEvent;

use crate::inventory::ItemStack;
use crate::proto::proto_param::ProtoParam;
use crate::schematic::handle_new_scene_entities_parent_chunk;
use crate::schematic::loot_chests::LootChestType;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{ChestInventory, InventorySlotType};
use crate::world::generation::WallBreakEvent;
use crate::world::world_helpers::{
    can_object_be_placed_here, tile_pos_to_world_pos, world_pos_to_tile_pos,
};
use crate::world::TileMapPosition;
use crate::{custom_commands::CommandsExt, player::Limb, CustomFlush, GameParam, GameState, YSort};
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_proto::prelude::{ProtoCommands, Prototypes, ReflectSchematic, Schematic};
use rand::Rng;

mod crafting;
pub mod item_actions;
mod loot_table;
pub mod melee;
pub mod object_actions;
pub mod projectile;
pub use crafting::*;
pub use loot_table::*;

use bevy_rapier2d::prelude::{Collider, Sensor};
use lazy_static::lazy_static;

use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, IntoStaticStr};

use self::crafting::CraftingPlugin;
use self::item_actions::handle_item_action_success;
use self::projectile::RangedAttackPlugin;

#[derive(Component, Reflect, FromReflect, Schematic)]
#[reflect(Schematic)]
pub struct Breakable(pub Option<WorldObject>);
#[derive(Component, Reflect, FromReflect, Schematic)]
#[reflect(Schematic)]
pub struct BreaksWith(pub WorldObject);
#[derive(Component, Reflect, FromReflect, Schematic)]
#[reflect(Schematic)]
pub struct PlacesInto(pub WorldObject);
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct Block;
#[derive(Component)]
pub struct Equipment(pub Limb);
#[derive(Component, Reflect, Debug, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum EquipmentType {
    #[default]
    None,
    Head,
    Chest,
    Legs,
    Feet,
    Ring,
    Pendant,
    Trinket,
    Weapon,
}
impl EquipmentType {
    pub fn get_valid_slots(&self) -> Vec<usize> {
        match self {
            EquipmentType::Head => vec![3],
            EquipmentType::Chest => vec![2],
            EquipmentType::Legs => vec![1],
            EquipmentType::Feet => vec![0],
            EquipmentType::Ring => vec![3, 2],
            EquipmentType::Pendant => vec![1],
            EquipmentType::Trinket => vec![0],
            _ => vec![],
        }
    }
    pub fn get_valid_slot_type(&self) -> InventorySlotType {
        match self {
            EquipmentType::Head => InventorySlotType::Equipment,
            EquipmentType::Chest => InventorySlotType::Equipment,
            EquipmentType::Legs => InventorySlotType::Equipment,
            EquipmentType::Feet => InventorySlotType::Equipment,
            EquipmentType::Ring => InventorySlotType::Accessory,
            EquipmentType::Pendant => InventorySlotType::Accessory,
            EquipmentType::Trinket => InventorySlotType::Accessory,
            _ => InventorySlotType::Normal,
        }
    }
    pub fn is_equipment(&self) -> bool {
        match self {
            EquipmentType::Head => true,
            EquipmentType::Chest => true,
            EquipmentType::Legs => true,
            EquipmentType::Feet => true,
            EquipmentType::Ring => false,
            EquipmentType::Pendant => false,
            EquipmentType::Trinket => false,
            _ => false,
        }
    }
    pub fn is_accessory(&self) -> bool {
        match self {
            EquipmentType::Ring => true,
            EquipmentType::Pendant => true,
            EquipmentType::Trinket => true,
            _ => false,
        }
    }
}
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
    Ord,
    PartialOrd,
)]
#[reflect(Component, Schematic)]
pub enum WorldObject {
    #[default]
    None,
    GrassTile,
    StoneWall,
    StoneWallBlock,
    DungeonStone,
    WaterTile,
    SandTile,
    StoneShard,
    Tree,
    Log,
    Sword,
    BasicStaff,
    FireStaff,
    Chestplate,
    Pants,
    DualStaff,
    Dagger,
    Fireball,
    Ring,
    Pendant,
    SmallPotion,
    LargePotion,
    Chest,
    ChestBlock,
    DungeonEntrance,
    DungeonEntranceBlock,
    Grass,
    GrassBlock,
    Boulder,
    SlimeGoo,
    Stick,
    PlantFibre,
    String,
    Bandage,
    DeadSapling,
    Apple,
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
    EnumIter,
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

lazy_static! {
    pub static ref PLAYER_EQUIPMENT_POSITIONS: HashMap<Limb, Vec2> = HashMap::from([
        (Limb::Head, Vec2::new(0., 9.)),
        (Limb::Torso, Vec2::new(0., 0.)),
        (Limb::Hands, Vec2::new(-9., -5.)),
        (Limb::Legs, Vec2::new(0., -9.))
    ]);
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
    pub fn is_block(&self) -> bool {
        match self {
            WorldObject::StoneWall => true,
            WorldObject::DungeonStone => true,
            _ => false,
        }
    }
    pub fn is_medium_size(&self, proto_param: &ProtoParam) -> bool {
        proto_param
            .get_component::<SpriteSize, _>(*self)
            .unwrap()
            .is_medium()
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

        position = Vec3::new(
            PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].x + anchor.x * obj_data.size.x,
            PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].y + anchor.y * obj_data.size.y,
            500. - (PLAYER_EQUIPMENT_POSITIONS[&Limb::Hands].y + anchor.y * obj_data.size.y) * 0.1,
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
            .insert(Equipment(Limb::Hands))
            .insert(YSort)
            .insert(Name::new("EquipItem"))
            .insert(self)
            .id();

        let mut item_entity = commands.entity(item);

        item_entity
            .insert(Collider::cuboid(
                obj_data.size.x / 3.5,
                obj_data.size.y / 4.5,
            ))
            .insert(Sensor);

        item_entity.insert(AttackAnimationTimer(
            Timer::from_seconds(0.125, TimerMode::Once),
            0.,
        ));
        item_entity.set_parent(player_e);

        item
    }

    pub fn get_minimap_color(&self) -> Color {
        match self {
            WorldObject::None => BLACK,
            WorldObject::Grass => UI_GRASS_GREEN,
            WorldObject::GrassTile => LIGHT_GREEN,
            WorldObject::DeadSapling => BROWN,
            WorldObject::StoneWall => LIGHT_GREY,
            WorldObject::Boulder => LIGHT_GREY,
            WorldObject::DungeonStone => BLACK,
            WorldObject::WaterTile => BLUE,
            WorldObject::SandTile => YELLOW,
            WorldObject::Tree => DARK_GREEN,
            _ => BLACK,
        }
    }
}

pub struct PlaceItemEvent {
    pub obj: WorldObject,
    pub pos: Vec2,
    pub loot_chest_type: Option<LootChestType>,
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectResource::new())
            .add_event::<PlaceItemEvent>()
            .add_plugin(CraftingPlugin)
            .add_plugin(RangedAttackPlugin)
            .add_plugin(LootTablePlugin)
            .add_system(
                handle_break_object
                    .before(CustomFlush)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_placing_world_object
                    .after(handle_new_scene_entities_parent_chunk)
                    .after(CustomFlush)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(handle_item_action_success.in_set(OnUpdate(GameState::Main)))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

pub fn handle_placing_world_object(
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    mut proto_param: ProtoParam,
    mut game: GameParam,
    mut commands: Commands,
    mut events: EventReader<PlaceItemEvent>,
) {
    for place_event in events.iter() {
        let pos = place_event.pos;
        let is_medium = place_event.obj.is_medium_size(&proto_param);
        let tile_pos = world_pos_to_tile_pos(pos);
        if !can_object_be_placed_here(tile_pos, &mut game, is_medium, &proto_param) {
            continue;
        }

        if let Some(chunk) = game.get_chunk_entity(tile_pos.chunk_pos) {
            let item = proto_commands.spawn_object_from_proto(
                place_event.obj,
                pos,
                &prototypes,
                &mut proto_param,
                true,
            );
            if let Some(item) = item {
                //TODO: do what old game data did, add obj to registry
                commands.entity(item).set_parent(*chunk);
                if let Some(loot_chest_type) = &place_event.loot_chest_type {
                    commands.entity(item).insert(loot_chest_type.clone());
                }
                if is_medium {
                    for q in 0..4 {
                        minimap_event.send(UpdateMiniMapEvent {
                            pos: Some(tile_pos.set_quadrant(q)),
                            new_tile: Some(place_event.obj),
                        });
                    }
                } else {
                    minimap_event.send(UpdateMiniMapEvent {
                        pos: Some(tile_pos),
                        new_tile: Some(place_event.obj),
                    });
                }
            }

            continue;
        } else {
            game.add_object_to_chunk_cache(tile_pos, place_event.obj);
        }
    }
}
pub fn handle_break_object(
    mut commands: Commands,
    proto_param: ProtoParam,
    mut game_param: GameParam,
    mut proto_commands: ProtoCommands,
    mut obj_break_events: EventReader<ObjBreakEvent>,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    mut wall_break_event: EventWriter<WallBreakEvent>,
    loot_tables: Query<&LootTable>,
    chest_containers: Query<&ChestInventory>,
) {
    for broken in obj_break_events.iter() {
        let mut rng = rand::thread_rng();
        if broken.obj == WorldObject::Chest {
            if let Ok(chest) = chest_containers.get(broken.entity) {
                for item_option in chest.items.items.iter() {
                    if let Some(item) = item_option {
                        let pos = tile_pos_to_world_pos(broken.pos, false);
                        item.item_stack
                            .spawn_as_drop(&mut commands, &mut game_param, pos);
                    }
                }
            }
        }
        commands.entity(broken.entity).despawn();
        if let Ok(loot_table) = loot_tables.get(broken.entity) {
            for drop in LootTablePlugin::get_drops(loot_table, 0) {
                let pos = if broken.obj.is_medium_size(&proto_param) {
                    tile_pos_to_world_pos(
                        TileMapPosition::new(broken.pos.chunk_pos, broken.pos.tile_pos, 0),
                        true,
                    )
                } else {
                    tile_pos_to_world_pos(broken.pos, false)
                };
                let drop_spread = 10.;

                let pos = Vec3::new(
                    pos.x + rng.gen_range(-drop_spread..drop_spread),
                    pos.y + rng.gen_range(-drop_spread..drop_spread),
                    0.,
                );
                proto_commands.spawn_item_from_proto(
                    drop.obj_type,
                    &proto_param,
                    pos.truncate(),
                    drop.count,
                );
            }
        }

        if let Some(_wall) = proto_param.get_component::<Wall, _>(broken.obj) {
            wall_break_event.send(WallBreakEvent { pos: broken.pos })
        }
        if broken.obj.is_medium_size(&proto_param) {
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos.set_quadrant(0)),
                new_tile: None,
            });
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos.set_quadrant(1)),
                new_tile: None,
            });
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos.set_quadrant(2)),
                new_tile: None,
            });
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos.set_quadrant(3)),
                new_tile: None,
            });
        } else {
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos),
                new_tile: None,
            });
        }
    }
}
