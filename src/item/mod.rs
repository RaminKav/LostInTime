use crate::ai::pathfinding::world_pos_to_AIPos;
use crate::assets::{SpriteAnchor, SpriteSize, WorldObjectData};
use crate::attributes::item_abilities::ItemAbility;
use crate::client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent};
use crate::client::is_not_paused;
use crate::colors::{
    BLACK, BLUE, DARK_BROWN, DARK_GREEN, LIGHT_BROWN, LIGHT_GREEN, LIGHT_GREY, RED, YELLOW,
};
use crate::combat::{handle_hits, ObjBreakEvent};

use crate::container::ContainerRegistry;
use crate::enemy::Mob;

use crate::inventory::ItemStack;
use crate::juice::{spawn_obj_death_particles, spawn_xp_particles};
use crate::player::levels::ExperienceReward;
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;

use crate::schematic::loot_chests::get_random_loot_chest_type;
use crate::status_effects::{
    handle_burning_ticks, handle_frail_stack_ticks, handle_slow_stack_ticks,
};
use crate::ui::damage_numbers::spawn_screen_locked_icon;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{ChestContainer, InventorySlotType};
use crate::world::dungeon::Dungeon;
use crate::world::generation::WallBreakEvent;
use crate::world::world_helpers::{
    can_object_be_placed_here, tile_pos_to_world_pos, world_pos_to_tile_pos,
};
use crate::world::{TileMapPosition, CHUNK_SIZE};
use crate::{custom_commands::CommandsExt, player::Limb, CustomFlush, GameParam, GameState};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::HashMap;
use bevy_proto::prelude::{ProtoCommands, Prototypes, ReflectSchematic, Schematic};
use combat_shrine::{
    add_shrine_visuals_on_spawn, handle_combat_shrine_activate_animation, handle_shrine_rewards,
    CombatShrineMobDeathEvent,
};
use gamble_shrine::{add_gamble_visuals_on_spawn, handle_gamble_shrine_rewards, GambleShrineEvent};
use projectile::handle_reset_proj_hit_enemies_state;
use rand::Rng;

mod crafting;
pub mod item_actions;

pub mod boss_shrine;
pub mod combat_shrine;
pub mod gamble_shrine;
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
    handle_delayed_ranged_attack, handle_on_hit_upgrades, handle_spread_arrows_attack,
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
    Cape,
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
            EquipmentType::Ring => vec![2, 1],
            EquipmentType::Pendant => vec![0],
            EquipmentType::Trinket => vec![3],
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
    pub fn is_cape(&self) -> bool {
        match self {
            EquipmentType::Cape => true,
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
    pub fn is_armor(&self) -> bool {
        match self {
            EquipmentType::Head => true,
            EquipmentType::Chest => true,
            EquipmentType::Legs => true,
            EquipmentType::Feet => true,
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
    pub fn get_attack_anim_offset(&self) -> f32 {
        match self.item_stack.obj_type {
            WorldObject::WoodSword => 1.,
            WorldObject::Sword => 2.,
            WorldObject::WoodAxe => 2.,
            WorldObject::WoodPickaxe => 2.,
            WorldObject::Dagger => 3.,
            WorldObject::IceStaff => 4.,
            WorldObject::BasicStaff => 0.,
            WorldObject::Claw => 1.,
            WorldObject::WoodBow => 1.,
            _ => 0.,
        }
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
    TypeUuid,
)]
#[reflect(Component, Schematic)]
#[uuid = "413be529-bfeb-41b3-9dc0-4b8b380a4c36"]
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
    IceStaff,
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
    GambleShrine,
    GambleShrineDone,
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
    DirtPath,
    TimeGate,
    TimeFragment,
    InventoryBag,
    Dodge,
    TooltipInspect,
    TimePortal,

    Scrapper,
    ScrapperBlock,
    // Capes
    GreyCape,
    RedCape,
    GreenCape,
    BlueCape,

    YellowBeacon,
    YellowBeaconBlock,
    RedBeacon,
    RedBeaconBlock,
    PinkBeacon,
    PinkBeaconBlock,
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
            WorldObject::Era2SmallTree => true,
            WorldObject::Era2MediumTree => true,
            WorldObject::Era2LargeTree => true,
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
            WorldObject::IceStaff => true,
            WorldObject::BasicStaff => true,
            WorldObject::MagicWhip => true,
            _ => false,
        }
    }
    pub fn is_cape(&self) -> bool {
        match self {
            WorldObject::GreyCape => true,
            WorldObject::RedCape => true,
            WorldObject::GreenCape => true,
            WorldObject::BlueCape => true,
            _ => false,
        }
    }
    pub fn is_melee_weapon(&self) -> bool {
        match self {
            WorldObject::WoodSword => true,
            WorldObject::Sword => true,
            WorldObject::Dagger => true,
            WorldObject::Claw => false,
            _ => false,
        }
    }
    pub fn is_ranged_weapon(&self) -> bool {
        match self {
            WorldObject::WoodBow => true,
            WorldObject::Claw => true,
            WorldObject::IceStaff => true,
            WorldObject::BasicStaff => true,
            WorldObject::MagicWhip => true,
            _ => false,
        }
    }
    pub fn is_magic_weapon(&self) -> bool {
        match self {
            WorldObject::IceStaff => true,
            WorldObject::BasicStaff => true,
            WorldObject::MagicWhip => true,
            _ => false,
        }
    }
    pub fn is_sword(&self) -> bool {
        match self {
            WorldObject::WoodSword => true,
            WorldObject::Sword => true,
            _ => false,
        }
    }
    pub fn is_tool(&self) -> bool {
        match self {
            WorldObject::WoodAxe => true,
            WorldObject::WoodPickaxe => true,
            _ => false,
        }
    }
    pub fn is_unique_object(&self) -> bool {
        match self {
            WorldObject::TimeGate => true,
            WorldObject::BossShrine => true,
            WorldObject::DungeonEntrance => true,
            _ => false,
        }
    }
    pub fn is_structure(&self) -> bool {
        match self {
            WorldObject::CombatShrine => true,
            WorldObject::CombatShrineDone => true,
            WorldObject::GambleShrine => true,
            WorldObject::GambleShrineDone => true,
            _ => false,
        }
    }
    pub fn is_beacon(&self) -> bool {
        match self {
            WorldObject::YellowBeacon => true,
            WorldObject::YellowBeaconBlock => true,
            WorldObject::RedBeacon => true,
            WorldObject::RedBeaconBlock => true,
            WorldObject::PinkBeacon => true,
            WorldObject::PinkBeaconBlock => true,
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
    pub fn get_weapon_levelup_upgrade(&self) -> i32 {
        match self {
            WorldObject::WoodSword => 2,
            WorldObject::Sword => 3,
            WorldObject::Dagger => 1,
            WorldObject::Claw => 2,
            WorldObject::WoodBow => 2,
            WorldObject::IceStaff => 2,
            WorldObject::BasicStaff => 2,
            WorldObject::MagicWhip => 2,
            _ => 0,
        }
    }
    pub fn is_water_placeable(&self) -> bool {
        match self {
            WorldObject::Bridge => true,
            WorldObject::WoodWallBlock => true,
            WorldObject::WoodWall => true,
            WorldObject::StoneWallBlock => true,
            WorldObject::StoneWall => true,
            WorldObject::PinkFlowerBlock => true,
            WorldObject::PinkFlower => true,
            WorldObject::RedFlowerBlock => true,
            WorldObject::RedFlower => true,
            WorldObject::YellowFlowerBlock => true,
            WorldObject::YellowFlower => true,
            WorldObject::WoodDoorBlock => true,
            WorldObject::WoodDoor => true,
            WorldObject::WoodDoorOpen => true,
            WorldObject::Pebble => true,
            WorldObject::PebbleBlock => true,
            WorldObject::Cattail => true,
            WorldObject::WaterBoulder => true,
            WorldObject::WaterBoulder2 => true,
            WorldObject::Lillypad => true,
            _ => false,
        }
    }

    pub fn get_obj_color(&self) -> Color {
        match self {
            WorldObject::None => BLACK,
            WorldObject::Grass => LIGHT_GREEN,
            WorldObject::Grass2 => LIGHT_GREEN,
            WorldObject::Grass3 => LIGHT_GREEN,
            WorldObject::RedMushroom => LIGHT_GREEN,
            WorldObject::BrownMushroom => LIGHT_GREEN,
            WorldObject::GrassTile => LIGHT_GREEN,
            WorldObject::DeadSapling => LIGHT_GREEN,
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
            WorldObject::PinkFlower => LIGHT_GREEN,
            WorldObject::RedFlower => LIGHT_GREEN,
            WorldObject::YellowFlower => LIGHT_GREEN,
            WorldObject::BerryBush => LIGHT_GREEN,
            WorldObject::Bush => DARK_GREEN,
            WorldObject::Bush2 => DARK_GREEN,
            WorldObject::Boulder2 => LIGHT_GREY,
            WorldObject::Crate => LIGHT_GREEN,
            WorldObject::Crate2 => LIGHT_GREEN,
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
            WorldObject::WaterBoulder => BLUE,
            WorldObject::WaterBoulder2 => BLUE,
            WorldObject::Pebble => LIGHT_GREEN,
            WorldObject::Lillypad => BLUE,
            WorldObject::Cattail => BLUE,
            WorldObject::WoodWall => LIGHT_BROWN,
            WorldObject::WoodDoor => LIGHT_BROWN,
            WorldObject::CombatShrine => LIGHT_GREY,
            WorldObject::CombatShrineDone => LIGHT_GREY,
            WorldObject::GambleShrine => LIGHT_GREY,
            WorldObject::GambleShrineDone => LIGHT_GREY,

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

#[derive(Component)]
pub struct ItemDrop;

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectResource::new())
            .add_event::<PlaceItemEvent>()
            .add_event::<UpdateObjectEvent>()
            .add_event::<CombatShrineMobDeathEvent>()
            .add_event::<GambleShrineEvent>()
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
                    handle_pay_shrine_cost,
                    handle_delayed_spawns.run_if(resource_exists::<DelayedSpawn>()),
                    handle_item_action_success.run_if(is_not_paused),
                    handle_delayed_ranged_attack.run_if(is_not_paused),
                    handle_spread_arrows_attack
                        .after(CustomFlush)
                        .run_if(is_not_paused),
                    handle_burning_ticks.run_if(is_not_paused),
                    handle_shrine_rewards,
                    add_shrine_visuals_on_spawn,
                    handle_gamble_shrine_rewards,
                    add_gamble_visuals_on_spawn,
                    handle_frail_stack_ticks.run_if(is_not_paused),
                    handle_slow_stack_ticks.run_if(is_not_paused),
                    handle_combat_shrine_activate_animation,
                    handle_on_hit_upgrades
                        .after(handle_hits)
                        .run_if(is_not_paused),
                    handle_reset_proj_hit_enemies_state.run_if(is_not_paused),
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
    asset_server: Res<AssetServer>,
    mut events: EventReader<PlaceItemEvent>,
    water_colliders: Query<
        (Entity, &Collider, &GlobalTransform),
        (Without<WorldObject>, Without<Mob>, Without<Player>),
    >,
    container_reg: Res<ContainerRegistry>,
    dungeon_check: Query<&Dungeon>,
) {
    for place_event in events.iter() {
        let pos = place_event.pos;
        let tile_pos = world_pos_to_tile_pos(pos);
        if !place_event.override_existing_obj
            && place_event.placed_by_player
            && !can_object_be_placed_here(tile_pos, &mut game, place_event.obj, &proto_param)
        {
            continue;
        }

        // Delete old object
        if place_event.override_existing_obj {
            //TODO: this fn is slow, optimize get_obj_entity_at_tile
            if let Some((old_obj, _)) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
                commands.entity(old_obj).despawn_recursive();
            }
        }

        // Place New Object
        if let Some(chunk) = game.get_chunk_entity(tile_pos.chunk_pos) {
            let mut is_touching_air = true;
            if let Ok(dungeon) = dungeon_check.get_single() {
                is_touching_air = false;
                for x in -1_i32..2 {
                    for y in -1_i32..2 {
                        let original_y = ((CHUNK_SIZE) as i32 * (4 - tile_pos.chunk_pos.y)
                            - 1
                            - (tile_pos.tile_pos.y as i32))
                            as usize;
                        let original_x = ((3 * CHUNK_SIZE) as i32
                            + (tile_pos.chunk_pos.x * CHUNK_SIZE as i32)
                            + tile_pos.tile_pos.x as i32)
                            as usize;
                        if dungeon.grid[(original_y as i32 + y).clamp(0, 127) as usize]
                            [(original_x as i32 + x).clamp(0, 127) as usize]
                            == 1
                        {
                            is_touching_air = true
                        }
                    }
                }
            }
            let item = proto_commands.spawn_object_from_proto(
                place_event.obj,
                pos,
                &prototypes,
                &mut proto_param,
                is_touching_air,
            );
            if let Some(item_e) = item {
                //TODO: do what old game data did, add obj to registry
                commands.entity(item_e).set_parent(chunk);
                if !place_event.placed_by_player
                    && container_reg.containers.get(&tile_pos).is_none()
                    && place_event.obj == WorldObject::Chest
                {
                    commands
                        .entity(item_e)
                        .insert(get_random_loot_chest_type(rand::thread_rng()));
                }
                if place_event.obj.is_beacon() {
                    spawn_screen_locked_icon(
                        item_e,
                        &mut commands,
                        &game.graphics,
                        &asset_server,
                        place_event.obj.clone(),
                    );
                }

                if place_event.obj.is_medium_size(&proto_param) {
                    minimap_event.send(UpdateMiniMapEvent {
                        pos: Some(tile_pos),
                        new_tile: Some(place_event.obj),
                    });
                    for q in 0..3 {
                        minimap_event.send(UpdateMiniMapEvent {
                            pos: Some(tile_pos.get_neighbour_tiles_for_medium_objects()[q]),
                            new_tile: Some(place_event.obj),
                        });
                    }
                } else {
                    minimap_event.send(UpdateMiniMapEvent {
                        pos: Some(tile_pos),
                        new_tile: Some(place_event.obj),
                    });
                }

                if place_event.obj.is_water_placeable() {
                    for (e, _c, t) in water_colliders.iter() {
                        if t.translation()
                            .truncate()
                            .distance(tile_pos_to_world_pos(tile_pos, false))
                            <= 6.
                        {
                            commands.entity(e).insert(Sensor);
                        }
                    }
                }
            }
        } else {
            info!("no chunk when spawn {:?} {:?}", tile_pos, place_event.obj);
        }

        game.add_object_to_chunk_cache(tile_pos, place_event.obj);
    }
}
pub fn handle_break_object(
    mut commands: Commands,
    proto_param: ProtoParam,
    mut game: GameParam,
    mut proto_commands: ProtoCommands,
    mut obj_break_events: EventReader<ObjBreakEvent>,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    mut wall_break_event: EventWriter<WallBreakEvent>,
    loot_tables: Query<&LootTable>,
    chest_containers: Query<&ChestContainer>,
    xp: Query<&ExperienceReward>,
    mut analytics_events: EventWriter<AnalyticsUpdateEvent>,
    water_colliders: Query<
        (Entity, &Collider, &GlobalTransform),
        (Without<WorldObject>, Without<Mob>, Without<Player>),
    >,
    anchor: Query<&SpriteAnchor>,
) {
    for broken in obj_break_events.iter() {
        let mut rng = rand::thread_rng();
        let world_pos = tile_pos_to_world_pos(broken.pos, false);
        // Chest
        if broken.obj == WorldObject::Chest {
            if let Ok(chest) = chest_containers.get(broken.entity) {
                for item_option in chest.items.items.iter() {
                    if let Some(item) = item_option {
                        item.item_stack
                            .spawn_as_drop(&mut commands, &mut game, world_pos);
                    }
                }
            }
        }

        // Water Placeable Objs
        if let Some(tile_data) = game.get_tile_data(broken.pos) {
            if tile_data.block_type.contains(&WorldObject::WaterTile)
                && broken.obj.is_water_placeable()
            {
                for (e, _c, t) in water_colliders.iter() {
                    if t.translation().truncate().distance(world_pos) <= 6. {
                        commands.entity(e).remove::<Sensor>();
                    }
                }
            }
        }

        commands.entity(broken.entity).despawn_recursive();
        game.remove_object_from_chunk_cache(broken.pos);

        if let Some(_wall) = proto_param.get_component::<Wall, _>(broken.obj) {
            wall_break_event.send(WallBreakEvent { pos: broken.pos })
        }

        if broken.obj.is_medium_size(&proto_param) {
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos),
                new_tile: None,
            });
            for q in 0..3 {
                minimap_event.send(UpdateMiniMapEvent {
                    pos: Some(broken.pos.get_neighbour_tiles_for_medium_objects()[q]),
                    new_tile: None,
                });
            }
        } else {
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(broken.pos),
                new_tile: None,
            });
        }

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
                    world_pos
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
                    Some(game.get_player_level()),
                );
            }
        }

        // EXP Reward
        if let Ok(exp) = xp.get(broken.entity) {
            let mut player_xp = game.get_player_level_mut();
            let did_level = player_xp.add_xp(exp.0);
            let t = tile_pos_to_world_pos(broken.pos, true);
            spawn_xp_particles(t, &mut commands, exp.0, did_level);
        }

        // Analytics
        analytics_events.send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::ObjectBroken(broken.obj),
        });

        // AI Pos Cache
        for quads in &[(-4., -4.), (-4., 4.), (4., 4.), (4., -4.)] {
            let anchor_offset = if let Ok(anchor) = anchor.get(broken.entity) {
                if broken.obj.is_tree() {
                    anchor.0
                } else {
                    Vec2::new(0., 0.)
                }
            } else {
                Vec2::new(0., 0.)
            };
            let offset_pos = world_pos + Vec2::new(quads.0, quads.1) - anchor_offset;
            let ai_pos = world_pos_to_AIPos(offset_pos);
            game.set_pos_validity_for_pathfinding(ai_pos, true);
        }
    }
}
