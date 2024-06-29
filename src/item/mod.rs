use crate::animations::AttackAnimationTimer;
use crate::assets::{SpriteSize, WorldObjectData};
use crate::attributes::item_abilities::ItemAbility;
use crate::attributes::ItemAttributes;
use crate::colors::{
    BLACK, BLUE, DARK_BROWN, DARK_GREEN, LIGHT_BROWN, LIGHT_GREEN, LIGHT_GREY, PINK, RED,
    UI_GRASS_GREEN, YELLOW,
};
use crate::combat::{handle_hits, ObjBreakEvent};

use crate::enemy::Mob;

use crate::inventory::ItemStack;
use crate::juice::{spawn_obj_death_particles, spawn_xp_particles, Particles};
use crate::player::levels::{ExperienceReward, PlayerLevel};
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;

use crate::schematic::loot_chests::get_random_loot_chest_type;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{ChestContainer, InventorySlotType};
use crate::world::dimension::ActiveDimension;
use crate::world::dungeon::Dungeon;
use crate::world::generation::WallBreakEvent;
use crate::world::world_helpers::{
    can_object_be_placed_here, tile_pos_to_world_pos, world_pos_to_tile_pos,
};
use crate::world::TileMapPosition;
use crate::{custom_commands::CommandsExt, player::Limb, CustomFlush, GameParam, GameState, YSort};
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_proto::prelude::{ProtoCommands, Prototypes, ReflectSchematic, Schematic};
use combat_shrine::{handle_shrine_rewards, CombatShrineMobDeathEvent};
use rand::Rng;

mod crafting;
pub mod item_actions;

pub mod boss_shrine;
pub mod combat_shrine;
use boss_shrine::*;
pub mod item_upgrades;
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
use self::item_upgrades::{
    handle_burning_ticks, handle_delayed_ranged_attack, handle_on_hit_upgrades,
    handle_spread_arrows_attack,
};
use self::projectile::RangedAttackPlugin;

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
#[derive(Component, Reflect, Debug, Clone, FromReflect, Schematic, Default, Eq, PartialEq)]
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
    Axe,
    Pickaxe,
}
#[derive(Component, Reflect, Debug, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct RequiredEquipmentType(pub EquipmentType);

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
    pub fn is_weapon(&self) -> bool {
        match self {
            EquipmentType::Weapon => true,
            _ => false,
        }
    }
    pub fn is_tool(&self) -> bool {
        match self {
            EquipmentType::Axe => true,
            EquipmentType::Pickaxe => true,
            _ => false,
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
pub struct ActiveMainHandState {
    pub entity: Entity,
    pub item_stack: ItemStack,
}
impl ActiveMainHandState {
    pub fn get_obj(&self) -> WorldObject {
        self.item_stack.obj_type
    }
}

#[derive(
    Component,
    PartialEq,
    Clone,
    Reflect,
    FromReflect,
    Schematic,
    Default,
    Debug,
    Serialize,
    Deserialize,
)]
#[reflect(Schematic, Default)]
pub struct ItemDisplayMetaData {
    pub name: String,
    pub desc: Vec<String>,
    pub level: Option<u8>,
    pub item_ability: Option<ItemAbility>,
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
    EnumIter,
)]
#[reflect(Component, Schematic)]
pub enum WorldObject {
    #[default]
    None,
    GrassTile,
    StoneTile,
    StoneWall,
    StoneWallBlock,
    WaterTile,
    Flint,
    SmallYellowTree,
    SmallGreenTree,
    MediumGreenTree,
    MediumYellowTree,
    RedTree,
    Log,
    Sword,
    BasicStaff,
    FireStaff,
    Chestplate,
    MetalPants,
    MetalShoes,
    LeatherTunic,
    LeatherPants,
    LeatherShoes,
    ForestShirt,
    ForestPants,
    ForestShoes,
    DualStaff,
    Dagger,
    Fireball,
    Ring,
    Pendant,
    SmallPotion,
    LargePotion,
    SmallManaPotion,
    LargeManaPotion,
    Chest,
    ChestBlock,
    DungeonEntrance,
    DungeonEntranceBlock,
    CombatShrine,
    CombatShrineDone,
    Grass,
    Grass2,
    Grass3,
    GrassBlock,
    Boulder,
    SlimeGoo,
    Stick,
    PlantFibre,
    String,
    Bandage,
    DeadSapling,
    Apple,
    WoodBow,
    Arrow,
    ThrowingStar,
    MagicWhip,
    WoodPlank,
    WoodAxe,
    Pebble,
    PebbleBlock,
    Claw,
    FireExplosionAOE,
    Crate,
    Crate2,
    CrateBlock,
    Coal,
    MetalShard,
    CoalBoulder,
    MetalBoulder,
    SlimeGooProjectile,
    StoneChunk,
    WoodSword,
    RedMushroom,
    BrownMushroom,
    RedMushroomBlock,
    BrownMushroomBlock,
    BerryBush,
    Berries,
    MetalBar,
    WoodPickaxe,
    Feather,
    Tusk,
    RawMeat,
    CookedMeat,
    Leather,
    BushlingScale,
    Bush,
    Bush2,
    Boulder2,
    LargeStump,
    LargeMushroomStump,
    YellowFlower,
    YellowFlowerBlock,
    RedFlower,
    RedFlowerBlock,
    PinkFlower,
    PinkFlowerBlock,
    Stump,
    Stump2,
    Cattail,
    Lillypad,
    WaterBoulder,
    WaterBoulder2,
    CraftingTable,
    CraftingTableBlock,
    Anvil,
    AnvilBlock,
    Cauldron,
    CauldronBlock,
    Furnace,
    FurnaceBlock,
    AlchemyTable,
    AlchemyTableBlock,
    RedStew,
    UpgradeTome,
    OrbOfTransformation,
    UpgradeStation,
    UpgradeStationBlock,
    BridgeBlock,
    Bridge,
    DungeonExit,
    WoodWall,
    WoodWallBlock,
    WoodDoor,
    WoodDoorOpen,
    WoodDoorBlock,
    MagicGem,
    MagicTusk,
    Bed,
    BedBlock,
    Essence,
    Key,
    MiracleSeed,

    // Sapplings
    RedSapplingBlock,
    YellowSapplingBlock,
    GreenSapplingBlock,
    RedSapplingStage1,
    RedSapplingStage2,
    RedSapplingStage3,
    YellowSapplingStage1,
    YellowSapplingStage2,
    YellowSapplingStage3,
    GreenSapplingStage1,
    GreenSapplingStage2,
    GreenSapplingStage3,

    // Era 2
    Era2SmallTree,
    Era2MediumTree,
    Era2LargeTree,
    Era2Grass,
    Era2Grass2,
    Era2Grass3,
    Era2DeadBranch,
    Era2BerryBush,
    Era2Stump,
    Era2Stump2,
    Era2BrownMushroom,
    Era2BrownMushroomBlock,
    Era2RedMushroom,
    Era2RedMushroomBlock,
    Era2RedFlower,
    Era2RedFlowerBlock,
    Era2WhiteFlower,
    Era2WhiteFlowerBlock,
    Era2Pebble,
    Era2Boulder,
    Era2Boulder2,
    Era2CoalBoulder,
    Era2MagicBoulder,

    BossShrine,
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
    SmallGreenTree,
    SmallYellowTree,
    MediumGreenTree,
    MediumYellowTree,
    RedTree,

    // Sapplings
    RedSapplingStage1,
    RedSapplingStage2,
    RedSapplingStage3,
    YellowSapplingStage1,
    YellowSapplingStage2,
    YellowSapplingStage3,
    GreenSapplingStage1,
    GreenSapplingStage2,
    GreenSapplingStage3,

    // Era 2
    Era2SmallTree,
    Era2MediumTree,
    Era2LargeTree,
}
impl Default for Foliage {
    fn default() -> Self {
        Self::SmallGreenTree
    }
}
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct FoliageSize(pub Vec2);

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
    WoodWall,
    WoodDoor,
    WoodDoorOpen,
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
}
//TODO: delete this and unify with WorldItemMetadata...

impl WorldObjectResource {
    fn new() -> Self {
        Self {
            properties: HashMap::new(),
        }
    }
}

impl WorldObject {
    pub fn is_wall(&self) -> bool {
        match self {
            WorldObject::StoneWall => true,
            WorldObject::WoodWall => true,
            WorldObject::WoodDoor => true,
            _ => false,
        }
    }
    pub fn is_tree(&self) -> bool {
        match self {
            WorldObject::SmallGreenTree => true,
            WorldObject::SmallYellowTree => true,
            WorldObject::MediumGreenTree => true,
            WorldObject::MediumYellowTree => true,
            WorldObject::RedTree => true,
            _ => false,
        }
    }
    pub fn is_weapon(&self) -> bool {
        match self {
            WorldObject::WoodSword => true,
            WorldObject::Sword => true,
            WorldObject::Dagger => true,
            WorldObject::WoodBow => true,
            WorldObject::Claw => true,
            WorldObject::FireStaff => true,
            WorldObject::BasicStaff => true,
            WorldObject::MagicWhip => true,
            _ => false,
        }
    }
    pub fn is_medium_size(&self, proto_param: &ProtoParam) -> bool {
        proto_param
            .get_component::<SpriteSize, _>(*self)
            .unwrap_or(&SpriteSize::Small)
            .is_medium()
    }
    pub fn get_equip_type(&self, proto_param: &ProtoParam) -> Option<EquipmentType> {
        if let Some(eq_type) = proto_param.get_component::<EquipmentType, _>(*self) {
            return Some(eq_type.clone());
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
        let player_e = game.player_query.single();
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
                level: None,
                desc: vec!["A cool piece of Equipment".to_string()],
                item_ability: None,
            })
            .insert(Equipment(Limb::Hands))
            .insert(YSort(0.))
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

    pub fn get_obj_color(&self) -> Color {
        match self {
            WorldObject::None => BLACK,
            WorldObject::Grass => UI_GRASS_GREEN,
            WorldObject::Grass2 => UI_GRASS_GREEN,
            WorldObject::Grass3 => UI_GRASS_GREEN,
            WorldObject::RedMushroom => RED,
            WorldObject::BrownMushroom => LIGHT_BROWN,
            WorldObject::GrassTile => LIGHT_GREEN,
            WorldObject::DeadSapling => LIGHT_BROWN,
            WorldObject::StoneWall => LIGHT_GREY,
            WorldObject::Boulder => LIGHT_GREY,
            WorldObject::CoalBoulder => LIGHT_GREY,
            WorldObject::MetalBoulder => LIGHT_GREY,
            WorldObject::WaterTile => BLUE,
            WorldObject::SmallGreenTree => DARK_GREEN,
            WorldObject::RedTree => RED,
            WorldObject::SmallYellowTree => YELLOW,
            WorldObject::MediumYellowTree => YELLOW,
            WorldObject::MediumGreenTree => DARK_GREEN,
            WorldObject::PinkFlower => PINK,
            WorldObject::RedFlower => RED,
            WorldObject::YellowFlower => YELLOW,
            WorldObject::BerryBush => DARK_GREEN,
            WorldObject::Bush => DARK_GREEN,
            WorldObject::Bush2 => DARK_GREEN,
            WorldObject::Boulder2 => LIGHT_GREEN,
            WorldObject::Crate => LIGHT_BROWN,
            WorldObject::Crate2 => LIGHT_BROWN,
            WorldObject::CraftingTable => LIGHT_BROWN,
            WorldObject::Anvil => LIGHT_GREY,
            WorldObject::Furnace => LIGHT_GREY,
            WorldObject::Cauldron => LIGHT_GREY,
            WorldObject::UpgradeStation => LIGHT_BROWN,
            WorldObject::Chest => LIGHT_BROWN,
            WorldObject::Bridge => DARK_BROWN,
            WorldObject::Stump => DARK_BROWN,
            WorldObject::Stump2 => DARK_BROWN,
            WorldObject::LargeMushroomStump => DARK_BROWN,
            WorldObject::LargeStump => DARK_BROWN,
            WorldObject::WaterBoulder => LIGHT_GREY,
            WorldObject::WaterBoulder2 => LIGHT_GREY,
            WorldObject::Pebble => LIGHT_GREY,
            WorldObject::Lillypad => UI_GRASS_GREEN,
            WorldObject::Cattail => UI_GRASS_GREEN,
            WorldObject::WoodWall => LIGHT_BROWN,
            WorldObject::WoodDoor => LIGHT_BROWN,

            _ => BLACK,
        }
    }
}

pub struct PlaceItemEvent {
    pub obj: WorldObject,
    pub pos: Vec2,
    pub placed_by_player: bool,
    pub override_existing_obj: bool,
}
pub struct UpdateObjectEvent {
    pub obj: WorldObject,
    pub pos: Vec2,
    pub placed_by_player: bool,
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectResource::new())
            .add_event::<PlaceItemEvent>()
            .add_event::<UpdateObjectEvent>()
            .add_event::<CombatShrineMobDeathEvent>()
            .add_plugin(CraftingPlugin)
            .add_plugin(RangedAttackPlugin)
            .add_plugin(LootTablePlugin)
            .add_system(
                handle_break_object
                    .before(CustomFlush)
                    .after(spawn_obj_death_particles)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_placing_world_object
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                (
                    check_for_items_on_shrine,
                    handle_delayed_spawns.run_if(resource_exists::<DelayedSpawn>()),
                    handle_item_action_success,
                    handle_delayed_ranged_attack,
                    handle_spread_arrows_attack.after(CustomFlush),
                    handle_burning_ticks,
                    handle_shrine_rewards,
                    handle_on_hit_upgrades.after(handle_hits),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
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
    water_colliders: Query<
        (Entity, &Collider, &GlobalTransform),
        (Without<WorldObject>, Without<Mob>, Without<Player>),
    >,
    dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
) {
    for place_event in events.iter() {
        let pos = place_event.pos;

        let tile_pos = world_pos_to_tile_pos(pos);
        if !can_object_be_placed_here(tile_pos, &mut game, place_event.obj, &proto_param)
            && !place_event.override_existing_obj
        {
            continue;
        }

        // Delete old object
        if place_event.override_existing_obj {
            if let Some(old_obj) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
                commands.entity(old_obj).despawn_recursive();
            }
        }

        // Place New Object
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
                commands.entity(item).set_parent(chunk);
                if !place_event.placed_by_player && place_event.obj == WorldObject::Chest {
                    commands
                        .entity(item)
                        .insert(get_random_loot_chest_type(rand::thread_rng()));
                }

                minimap_event.send(UpdateMiniMapEvent {
                    pos: Some(tile_pos),
                    new_tile: Some(place_event.obj),
                });

                if place_event.obj == WorldObject::Bridge {
                    for (e, _c, t) in water_colliders.iter() {
                        if t.translation()
                            .truncate()
                            .distance(tile_pos_to_world_pos(tile_pos, false))
                            <= 6.
                        {
                            commands.entity(e).despawn();
                        }
                    }
                }
            }
        }
        if dungeon_check.get_single().is_err() {
            game.add_object_to_chunk_cache(tile_pos, place_event.obj);
        } else {
            game.add_object_to_dungeon_cache(tile_pos, place_event.obj);
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
    chest_containers: Query<&ChestContainer>,
    xp: Query<&ExperienceReward>,
    mut player_xp: Query<&mut PlayerLevel>,
    particles: Res<Particles>,
) {
    for broken in obj_break_events.iter() {
        let mut rng = rand::thread_rng();

        // Chest
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

        commands.entity(broken.entity).despawn_recursive();
        game_param.remove_object_from_chunk_cache(broken.pos);

        if let Some(_wall) = proto_param.get_component::<Wall, _>(broken.obj) {
            wall_break_event.send(WallBreakEvent { pos: broken.pos })
        }

        minimap_event.send(UpdateMiniMapEvent {
            pos: Some(broken.pos),
            new_tile: None,
        });

        if !broken.give_drops_and_xp {
            continue;
        }
        // Item Drops
        if let Ok(loot_table) = loot_tables.get(broken.entity) {
            for drop in LootTablePlugin::get_drops(loot_table, &proto_param, 0, None) {
                let pos = if broken.obj.is_medium_size(&proto_param) {
                    tile_pos_to_world_pos(
                        TileMapPosition::new(broken.pos.chunk_pos, broken.pos.tile_pos),
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
                    None,
                );
            }
        }

        // EXP Reward
        if let Some(exp) = xp.get(broken.entity).ok() {
            let mut player = player_xp.single_mut();
            player.add_xp(exp.0);
            let t = tile_pos_to_world_pos(broken.pos, true);
            spawn_xp_particles(t, &mut commands, particles.xp_particles.clone());
        }
    }
}
