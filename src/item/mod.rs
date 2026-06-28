use crate::assets::{SpriteAnchor, SpriteSize, WorldObjectData};
use crate::attributes::item_abilities::ItemAbility;
use crate::chaos::ChaosTracker;
use crate::client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent};
use crate::client::is_not_paused;
use crate::colors::{
    BLACK, BLUE, DARK_BROWN, DARK_GREEN, GREY, LIGHT_BROWN, LIGHT_GREEN, LIGHT_GREY, RED,
    SHIELD_BLUE, SNOW_BLUE, SNOW_DARK, SNOW_GREEN, WHITE, YELLOW,
};
use crate::combat::ObjBreakEvent;
use crate::ecs_helpers::safe_set_parent;

use crate::container::ContainerRegistry;
use crate::enemy::Mob;

use crate::inventory::{BreakDropFilter, ItemStack};
use crate::item::ammo::AmmoMemory;
use crate::juice::{spawn_obj_death_particles, spawn_xp_particles};
use crate::player::levels::ExperienceReward;
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;

use crate::status_effects::{
    check_freeze_on_slow_stacks, ensure_mob_status_effects, handle_burning_ticks,
    handle_frail_stack_ticks, handle_frozen_ticks, handle_slow_stack_ticks,
};
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{EssenceShopChoices, FlashExpBarEvent, InventorySlotType};
use crate::world::dimension::Era;
use crate::world::generation::WallBreakEvent;
use crate::world::grass_patches::{
    desert_patch_parent_local_offset, desert_patch_water_radius, grass_patch_parent_local_offset,
    spawn_ground_patch, GroundPatch, GroundPatchPlacement, GroundPatchesGraphics,
    GROUND_PATCH_CHILD_Z_OBJECT_BONUS, GROUND_PATCH_WATER_CHECK_RADIUS_TILES,
    GROUND_PATCH_YSORT_KEY_7, GROUND_PATCH_YSORT_KEY_8, GROUND_PATCH_YSORT_KEY_9,
};
use crate::world::world_helpers::{
    can_object_be_placed_here, object_within_tile_radius_of_water, tile_pos_to_world_pos,
    world_pos_to_tile_pos,
};
use crate::world::TileMapPosition;
use crate::{custom_commands::CommandsExt, player::Limb, CustomFlush, GameParam, GameState};
use crate::{handle_pink_flower_animation_loop, spawn_pink_flower_aseprite};
use active_skill_shrine::{
    add_active_skill_shrine_visuals_on_spawn, handle_active_skill_shrine_completion,
    handle_active_skill_shrine_esc,
};
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::HashMap;
use bevy_proto::prelude::{ProtoCommands, Prototypes, ReflectSchematic, Schematic};
use combat_shrine::{
    add_shrine_visuals_on_spawn, enhance_combat_shrine_mobs, handle_combat_shrine_activate_animation,
    handle_shrine_rewards, CombatShrineMobDeathEvent,
};
use dungeon_shrine::{
    add_dungeon_shrine_visuals_on_spawn, handle_dungeon_shrine_activation,
    handle_dungeon_shrine_rewards, DungeonShrineMobDeathEvent,
};
use gamble_shrine::{add_gamble_visuals_on_spawn, handle_gamble_shrine_rewards, GambleShrineEvent};
use heirloom_shrine::{add_heirloom_shrine_visuals_on_spawn, handle_heirloom_shrine_completion};
use microwave_shrine::{
    add_microwave_shrine_visuals_on_spawn, handle_microwave_shrine_completion,
    handle_microwave_shrine_esc,
};
use projectile::handle_reset_proj_hit_enemies_state;
use rand::Rng;

mod crafting;
pub mod food_recipes;
pub mod item_actions;

pub mod active_skill_shrine;
pub mod ammo;
pub mod boss_shrine;
pub mod bridge_placement;
pub mod combat_shrine;
pub mod dungeon_shrine;
pub mod gamble_shrine;
pub mod heirloom_shrine;
pub mod item_drop_outline;
pub mod microwave_shrine;
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

use self::ammo::tick_reload;
use self::bridge_placement::{
    handle_bridge_placement_mode, handle_bridge_preview_dragging, BridgePlacementMode,
};
use self::crafting::CraftingPlugin;
use self::item_actions::handle_item_action_success;
use self::item_drop_outline::ItemDropOutlinePlugin;
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
            // Weapon slot (and pet slot) are single-slot containers; only index 0 is valid.
            EquipmentType::Weapon => vec![0],
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
            // Weapons route to the Weapon slot. The Pet slot accepts the same item
            // category at validate-time (see `InventoryItemStack::validate`), so
            // dragging a weapon to either slot is permitted.
            EquipmentType::Weapon => InventorySlotType::Weapon,
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

    /// Short name for "Craft a/an ___ first" when hitting tool-gated world objects.
    pub fn craft_hint_name(&self) -> Option<&'static str> {
        match self {
            EquipmentType::Axe => Some("Axe"),
            EquipmentType::Pickaxe => Some("Pickaxe"),
            _ => None,
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
/// Marker set on the currently-equipped weapon entity. Cycles whenever the
/// player swaps weapons/tools in the hotbar — stored `SparseSet` so weapon
/// swaps don't move every other tool through a duplicated archetype variant.
#[derive(Component)]
#[component(storage = "SparseSet")]
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

/// Represents a single bonus stat line on an item
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
pub struct BonusStatLine {
    /// The attribute name (e.g., "crit_chance", "dodge", "health")
    pub attribute_name: String,
    /// The value for this stat line
    pub value: i32,
    /// Quality of this stat line
    pub quality: crate::attributes::AttributeQuality,
    /// Range percentage for this stat line
    pub range_percentage: f32,
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
    /// Index of the bonus attribute line selected for inventory buff (0-based)
    pub inventory_buff_line_index: Option<usize>,
    /// Individual bonus stat lines (allows duplicate stats like +5 crit, +9 crit)
    pub bonus_stat_lines: Vec<BonusStatLine>,
}
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
    Spear,
    Hammer,
    FireStaff,
    PlasmaStaff,
    Blowdart,
    Gun,
    IceShard,
    Dart,
    Bullet,
    Dagger,
    Fireball,
    Ring,
    Pendant,
    SmallPotion,
    LargePotion,
    SmallManaPotion,
    LargeManaPotion,
    AttackSpeedPotion,
    MovementSpeedPotion,
    Chest,
    ChestBlock,
    HeirloomChest,
    HeirloomChestBlock,
    DungeonEntrance,
    DungeonEntranceBlock,
    CombatShrine,
    CombatShrineDone,
    GambleShrine,
    GambleShrineDone,
    ActiveSkillShrine,
    ActiveSkillShrineDone,
    WeaponShrine,
    WeaponShrineDone,
    ArmorShrine,
    ArmorShrineDone,
    AccessoryShrine,
    AccessoryShrineDone,
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
    DesertMetalBoulder,
    SnowMetalBoulder,
    SlimeGooProjectile,
    StoneChunk,
    WoodSword,
    RedMushroom,
    BrownMushroom,
    RedMushroomBlock,
    BrownMushroomBlock,
    BlueberryBush,
    Blueberries,
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
    PinkFlowerStew,
    YellowFlowerStew,
    BerryJam,
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
    BoulderHeirloom,

    // Saplings
    RedSaplingBlock,
    YellowSaplingBlock,
    GreenSaplingBlock,
    RedSaplingStage1,
    RedSaplingStage2,
    RedSaplingStage3,
    YellowSaplingStage1,
    YellowSaplingStage2,
    YellowSaplingStage3,
    GreenSaplingStage1,
    GreenSaplingStage2,
    GreenSaplingStage3,

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
    ManaOrb,
    XPShard,
    XPShardMedium,
    XPShardLarge,
    XPJug,
    InventoryBag,
    Dodge,
    TooltipInspect,
    TimePortal,
    ReaperSoul,

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
    BlacksmithMerchant,
    BlacksmithMerchantDone,
    HeirloomShrine,
    HeirloomShrineDone,
    MicrowaveShrine,
    MicrowaveShrineDone,
    ChaosTotem,
    ChaosTotemDone,
    Coin,
    DaggerThrow,
    Bomb,
    FuryKunai,
    CrowFeather,
    PossessedBlade,
    ArrowVolleyShot,

    // Desert
    MedCactus1,
    MedCactus2,
    MedCactus3,
    MedCactus4,
    MedCactus5,
    SmlCactus1,
    SmlCactus2,
    SmlCactus3,
    FlowerCactus1,
    FlowerCactus2,
    FlowerCactus3,
    FlowerCactus4,
    MedFruitCactus1,
    MedFruitCactus2,
    SmlFruitCactus1,
    SmlFruitCactus2,
    SmlFruitCactus3,
    SmlFruitCactus4,

    DesertGrass1,
    DesertGrass2,
    DesertGrass3,
    DesertGrass4,
    DesertMedBoulder1,
    DesertMedBoulder2,
    DesertMedBoulder3,
    DesertSmlBoulder1,
    DesertSmlBoulder2,
    DesertSmlBoulder3,
    DesertSmlBoulder4,
    DesertSmlBoulder5,
    DesertSmlBoulder6,

    DesertDriftWood1,
    DesertDriftWood2,
    DesertDriftWood3,
    DesertFence1,
    DesertFence2,
    DesertFence3,

    DesertSkull1,
    DesertSkull2,
    DesertSkull3,
    DesertSkull4,
    DesertBones1,
    DesertBones2,
    DesertBones3,
    Tumbleweed1,
    Tumbleweed2,
    Tumbleweed3,
    Tumbleweed4,
    DesertCrate,
    DesertCrate2,
    DesertLargeCactus1,
    DesertLargeCactus2,
    DesertLargeCactus3,
    DesertLargeCactus4,
    DesertLargeCactus5,
    DesertLargeCactus6,
    DesertLargeCactus7,
    DesertLargeCactus8,
    Bones,
    CactusFlower,
    CactusBerry,

    // SNOW
    SnowGrass1,
    SnowGrass2,
    SnowGrass3,
    SnowGrass4,

    SnowMedBoulder1,
    SnowMedBoulder2,
    SnowMedBoulder3,
    SnowSmlBoulder1,
    SnowSmlBoulder2,
    SnowSmlBoulder3,
    SnowSmlBoulder4,
    SnowSmlBoulder5,
    SnowSmlBoulder6,

    SnowLeaflessTree1,
    SnowLeaflessTree2,
    SnowLeaflessTree3,
    SnowTree1,
    SnowTree2,
    SnowTree3,
    SnowTree4,

    SnowBush1,
    SnowBush2,
    SnowBush3,
    SnowBush4,
    SnowBushSnow1,
    SnowBushSnow2,
    SnowBushSnow3,
    SnowBushSnow4,
    SnowBushMed1,
    SnowBushMed2,
    SnowBushSnowMed1,
    SnowBushSnowMed2,
    SnowBerryBushSml1,
    SnowBerryBushSml2,
    SnowBerryBushSml3,
    SnowBerryBushSml4,
    SnowBerryBushMed1,
    SnowBerryBushMed2,
    SnowMushroom1,
    SnowMushroom2,
    BlueMushroom,

    SnowCrate1,
    SnowCrate2,
    SnowCrate3,
    SnowCrate4,
    SnowCrystalSml1,
    SnowCrystalSml2,
    SnowCrystalSml3,
    SnowCrystalSml4,
    SnowCrystalMed1,
    SnowCrystalMed2,

    SnowFlower,
    SnowFlower2,
    SnowFlowerBlock,
    SnowFlower2Block,
    SnowDeadSapling,
    IcePatch,
    YellowBerries,

    SpeedFood,
    HealthFood,
    ManaFood,
    ThornsFood,
    CritChanceFood,
    LifestealFood,
    SkillPowerFood,
    ManaRegenFood,
    DodgeFood,
    DefenceFood,
    SizeFood,
    AttackSpeedFood,

    //Monster Cards
    FurDevilCard,
    BushlingCard,
    SpikeSlimeCard,
    StingflyCard,
    RedMushlingCard,
    LizardCard,
    SmallCactusCard,
    LargeCactusCard,
    BullCard,
    VoidCrawlerCard,
    VoidWormCard,
    StoneGolemCard,
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

    // Saplings
    RedSaplingStage1,
    RedSaplingStage2,
    RedSaplingStage3,
    YellowSaplingStage1,
    YellowSaplingStage2,
    YellowSaplingStage3,
    GreenSaplingStage1,
    GreenSaplingStage2,
    GreenSaplingStage3,

    // Era 2
    Era2SmallTree,
    Era2MediumTree,
    Era2LargeTree,

    // Snow Biome
    SnowLeaflessTree1,
    SnowLeaflessTree2,
    SnowLeaflessTree3,
    SnowTree1,
    SnowTree2,
    SnowTree3,
    SnowTree4,
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

/// Objects that receive key-7 grass decoration under them when placed (`GroundPatch::GrassPatch2`).
pub const OBJECTS_WITH_GRASS_PATCH_7: &[WorldObject] = &[
    WorldObject::Crate,
    WorldObject::Crate2,
    // WorldObject::BerryBush,
    // WorldObject::BlueberryBush,
    // WorldObject::Stump,
    // WorldObject::Stump2,
    WorldObject::LargeStump,
    WorldObject::LargeMushroomStump,
    WorldObject::Boulder,
    WorldObject::Boulder2,
    WorldObject::CoalBoulder,
    WorldObject::MetalBoulder,
    WorldObject::Bush,
    WorldObject::Bush2,
    WorldObject::XPJug,
];

/// Era 2 desert objects that receive a sand patch under them when placed.
pub const OBJECTS_WITH_DESERT_PATCH_UNDER: &[WorldObject] = &[
    WorldObject::DesertMedBoulder1,
    WorldObject::DesertMedBoulder2,
    WorldObject::DesertMedBoulder3,
    WorldObject::DesertMetalBoulder,
    WorldObject::DesertCrate,
    WorldObject::DesertCrate2,
];

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
            WorldObject::SnowLeaflessTree1 => true,
            WorldObject::SnowLeaflessTree2 => true,
            WorldObject::SnowLeaflessTree3 => true,
            WorldObject::SnowTree1 => true,
            WorldObject::SnowTree2 => true,
            WorldObject::SnowTree3 => true,
            WorldObject::SnowTree4 => true,
            WorldObject::DesertLargeCactus1 => true,
            WorldObject::DesertLargeCactus2 => true,
            WorldObject::DesertLargeCactus3 => true,
            WorldObject::DesertLargeCactus4 => true,
            WorldObject::DesertLargeCactus5 => true,
            WorldObject::DesertLargeCactus6 => true,
            WorldObject::DesertLargeCactus7 => true,
            WorldObject::DesertLargeCactus8 => true,
            _ => false,
        }
    }

    pub fn is_large_desert_cactus(&self) -> bool {
        matches!(
            self,
            WorldObject::DesertLargeCactus1
                | WorldObject::DesertLargeCactus2
                | WorldObject::DesertLargeCactus3
                | WorldObject::DesertLargeCactus4
                | WorldObject::DesertLargeCactus5
                | WorldObject::DesertLargeCactus6
                | WorldObject::DesertLargeCactus7
                | WorldObject::DesertLargeCactus8
        )
    }

    pub fn spawns_grass_patch_7_under(&self) -> bool {
        OBJECTS_WITH_GRASS_PATCH_7.contains(self)
    }

    pub fn spawns_desert_patch_under(&self) -> bool {
        OBJECTS_WITH_DESERT_PATCH_UNDER.contains(self)
    }

    /// Era-2 objects that use the smaller 32×32 desert ground patch.
    pub fn spawns_small_desert_patch(&self) -> bool {
        matches!(
            self,
            WorldObject::HeirloomShrine
                | WorldObject::HeirloomShrineDone
                | WorldObject::ActiveSkillShrine
                | WorldObject::ActiveSkillShrineDone
                | WorldObject::DesertLargeCactus1
                | WorldObject::DesertLargeCactus2
                | WorldObject::DesertLargeCactus3
                | WorldObject::DesertLargeCactus4
                | WorldObject::DesertLargeCactus5
                | WorldObject::DesertLargeCactus6
                | WorldObject::DesertCrate
                | WorldObject::DesertCrate2
                | WorldObject::DesertSkull1
                | WorldObject::DesertSkull2
                | WorldObject::DesertSkull3
                | WorldObject::DesertSkull4
                | WorldObject::Tumbleweed1
                | WorldObject::Tumbleweed2
                | WorldObject::Tumbleweed3
                | WorldObject::Tumbleweed4
        )
    }

    /// Which desert ground patch to spawn under this object in era 2, if any.
    pub fn desert_ground_patch(&self) -> Option<GroundPatch> {
        if self.spawns_small_desert_patch() || self.spawns_grass_patch_9_shrine_under() {
            return Some(GroundPatch::DesertPatchSmall);
        }
        if self.is_large_desert_cactus() || self.spawns_desert_patch_under() {
            return Some(GroundPatch::DesertPatch1);
        }
        None
    }

    /// Shrines that get key-9 grass (`GroundPatch::GrassPatch4`) under them; offset from proto [`SpriteAnchor`].
    pub fn spawns_grass_patch_9_shrine_under(&self) -> bool {
        matches!(
            self,
            WorldObject::CombatShrine
                | WorldObject::CombatShrineDone
                | WorldObject::GambleShrine
                | WorldObject::GambleShrineDone
                | WorldObject::ActiveSkillShrine
                | WorldObject::ActiveSkillShrineDone
                | WorldObject::BlacksmithMerchant
                | WorldObject::BlacksmithMerchantDone
                | WorldObject::HeirloomShrine
                | WorldObject::HeirloomShrineDone
                | WorldObject::MicrowaveShrine
                | WorldObject::MicrowaveShrineDone
                | WorldObject::BossShrine
        )
    }
    pub fn is_weapon(&self) -> bool {
        match self {
            // WorldObject::WoodSword => true,
            WorldObject::Sword => true,
            WorldObject::Spear => true,
            WorldObject::Dagger => true,
            // WorldObject::Gun => true,
            WorldObject::Blowdart => true,
            WorldObject::FireStaff => true,
            WorldObject::PlasmaStaff => true,
            WorldObject::Hammer => true,
            WorldObject::WoodBow => true,
            WorldObject::Claw => true,
            WorldObject::IceStaff => true,
            WorldObject::BasicStaff => true,
            WorldObject::MagicWhip => true,
            _ => false,
        }
    }
    pub fn is_armor(&self) -> bool {
        match self {
            WorldObject::Chestplate => true,
            WorldObject::MetalPants => true,
            WorldObject::MetalShoes => true,
            WorldObject::LeatherTunic => true,
            WorldObject::LeatherPants => true,
            WorldObject::LeatherShoes => true,
            WorldObject::ForestShirt => true,
            WorldObject::ForestPants => true,
            WorldObject::ForestShoes => true,
            _ => false,
        }
    }
    pub fn is_accessory(&self) -> bool {
        match self {
            WorldObject::Ring => true,
            WorldObject::Pendant => true,
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
            WorldObject::Spear => true,
            WorldObject::Dagger => true,
            WorldObject::Hammer => true,
            WorldObject::Claw => false,
            _ => false,
        }
    }
    pub fn is_breakable_by_projectile(&self) -> bool {
        match self {
            WorldObject::Crate => true,
            WorldObject::Crate2 => true,
            WorldObject::Pebble => true,
            WorldObject::BerryBush => true,
            WorldObject::BlueberryBush => true,
            WorldObject::DeadSapling => true,
            WorldObject::RedMushroom => true,
            WorldObject::BrownMushroom => true,
            WorldObject::RedFlower => true,
            WorldObject::YellowFlower => true,

            WorldObject::Era2BerryBush => true,
            WorldObject::Era2DeadBranch => true,
            WorldObject::Era2Pebble => true,
            WorldObject::XPJug => true,

            WorldObject::DesertSkull1 => true,
            WorldObject::DesertSkull2 => true,
            WorldObject::DesertSkull3 => true,
            WorldObject::DesertSkull4 => true,
            WorldObject::Tumbleweed1 => true,
            WorldObject::Tumbleweed2 => true,
            WorldObject::Tumbleweed3 => true,
            WorldObject::Tumbleweed4 => true,
            WorldObject::DesertBones1 => true,
            WorldObject::DesertBones2 => true,
            WorldObject::DesertBones3 => true,
            WorldObject::DesertDriftWood1 => true,
            WorldObject::DesertDriftWood2 => true,
            WorldObject::DesertDriftWood3 => true,
            WorldObject::DesertFence1 => true,
            WorldObject::DesertFence2 => true,
            WorldObject::DesertFence3 => true,
            WorldObject::DesertSmlBoulder1 => true,
            WorldObject::DesertSmlBoulder2 => true,
            WorldObject::DesertSmlBoulder3 => true,
            WorldObject::DesertSmlBoulder4 => true,
            WorldObject::DesertSmlBoulder5 => true,
            WorldObject::DesertSmlBoulder6 => true,
            WorldObject::DesertMedBoulder1 => true,
            WorldObject::DesertMedBoulder2 => true,
            WorldObject::DesertMedBoulder3 => true,
            WorldObject::SmlCactus1 => true,
            WorldObject::SmlCactus2 => true,
            WorldObject::SmlCactus3 => true,
            WorldObject::FlowerCactus1 => true,
            WorldObject::FlowerCactus2 => true,
            WorldObject::FlowerCactus3 => true,
            WorldObject::FlowerCactus4 => true,
            WorldObject::SmlFruitCactus1 => true,
            WorldObject::SmlFruitCactus2 => true,
            WorldObject::SmlFruitCactus3 => true,
            WorldObject::SmlFruitCactus4 => true,
            WorldObject::MedFruitCactus1 => true,
            WorldObject::MedFruitCactus2 => true,
            WorldObject::MedCactus1 => true,
            WorldObject::MedCactus2 => true,
            WorldObject::MedCactus3 => true,
            WorldObject::MedCactus4 => true,
            WorldObject::MedCactus5 => true,

            WorldObject::SnowSmlBoulder1 => true,
            WorldObject::SnowSmlBoulder2 => true,
            WorldObject::SnowSmlBoulder3 => true,
            WorldObject::SnowSmlBoulder4 => true,
            WorldObject::SnowSmlBoulder5 => true,
            WorldObject::SnowSmlBoulder6 => true,
            WorldObject::SnowMedBoulder1 => true,
            WorldObject::SnowMedBoulder2 => true,
            WorldObject::SnowMedBoulder3 => true,
            WorldObject::SnowBush1 => true,
            WorldObject::SnowBush2 => true,
            WorldObject::SnowBush3 => true,
            WorldObject::SnowBush4 => true,
            WorldObject::SnowBushSnow1 => true,
            WorldObject::SnowBushSnow2 => true,
            WorldObject::SnowBushSnow3 => true,
            WorldObject::SnowBushSnow4 => true,
            WorldObject::SnowBushMed1 => true,
            WorldObject::SnowBushMed2 => true,
            WorldObject::SnowBushSnowMed1 => true,
            WorldObject::SnowBushSnowMed2 => true,
            WorldObject::SnowBerryBushSml1 => true,
            WorldObject::SnowBerryBushSml2 => true,
            WorldObject::SnowBerryBushSml3 => true,
            WorldObject::SnowBerryBushSml4 => true,
            WorldObject::SnowBerryBushMed1 => true,
            WorldObject::SnowBerryBushMed2 => true,
            WorldObject::SnowMushroom1 => true,
            WorldObject::SnowMushroom2 => true,
            WorldObject::DesertCrate => true,
            WorldObject::DesertCrate2 => true,
            WorldObject::SnowCrate1 => true,
            WorldObject::SnowCrate2 => true,
            WorldObject::SnowCrate3 => true,
            WorldObject::SnowCrate4 => true,
            WorldObject::SnowCrystalSml1 => true,
            WorldObject::SnowCrystalSml2 => true,
            WorldObject::SnowCrystalSml3 => true,
            WorldObject::SnowCrystalSml4 => true,
            WorldObject::SnowCrystalMed1 => true,
            WorldObject::SnowCrystalMed2 => true,

            _ => false,
        }
    }
    pub fn is_ranged_weapon(&self) -> bool {
        match self {
            WorldObject::WoodBow => true,
            WorldObject::Claw => true,
            WorldObject::IceStaff => true,
            WorldObject::FireStaff => true,
            WorldObject::PlasmaStaff => true,
            WorldObject::BasicStaff => true,
            WorldObject::MagicWhip => true,
            WorldObject::Gun => true,
            WorldObject::Blowdart => true,
            _ => false,
        }
    }
    pub fn get_ammo(&self) -> (u32, f32) {
        match self {
            WorldObject::Gun => (6, 0.9),
            WorldObject::WoodBow => (6, 1.25),
            WorldObject::Claw => (10, 1.25),
            WorldObject::Blowdart => (8, 1.25),
            _ => (0, 0.0),
        }
    }
    pub fn is_magic_weapon(&self) -> bool {
        match self {
            WorldObject::IceStaff => true,
            WorldObject::FireStaff => true,
            WorldObject::PlasmaStaff => true,
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
    /// Desert biome cactuses that deal contact damage to the player.
    pub fn is_desert_cactus(&self) -> bool {
        match self {
            WorldObject::MedCactus1
            | WorldObject::MedCactus2
            | WorldObject::MedCactus3
            | WorldObject::MedCactus4
            | WorldObject::MedCactus5
            | WorldObject::SmlCactus1
            | WorldObject::SmlCactus2
            | WorldObject::SmlCactus3
            | WorldObject::FlowerCactus1
            | WorldObject::FlowerCactus2
            | WorldObject::FlowerCactus3
            | WorldObject::FlowerCactus4
            | WorldObject::MedFruitCactus1
            | WorldObject::MedFruitCactus2
            | WorldObject::SmlFruitCactus1
            | WorldObject::SmlFruitCactus2
            | WorldObject::SmlFruitCactus3
            | WorldObject::SmlFruitCactus4
            | WorldObject::DesertLargeCactus1
            | WorldObject::DesertLargeCactus2
            | WorldObject::DesertLargeCactus3
            | WorldObject::DesertLargeCactus4
            | WorldObject::DesertLargeCactus5
            | WorldObject::DesertLargeCactus6
            | WorldObject::DesertLargeCactus7
            | WorldObject::DesertLargeCactus8 => true,
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
            // WorldObject::WoodSword => 2,
            // WorldObject::Sword => 2,
            // WorldObject::Spear => 2,
            // WorldObject::Dagger => 1,
            // WorldObject::Hammer => 2,
            // WorldObject::Gun => 2,
            // WorldObject::Blowdart => 2,
            // WorldObject::FireStaff => 2,
            // WorldObject::Claw => 2,
            // WorldObject::WoodBow => 2,
            // WorldObject::IceStaff => 2,
            // WorldObject::BasicStaff => 1,
            // WorldObject::MagicWhip => 1,
            _ => 1,
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
            WorldObject::CombatShrine => GREY,
            WorldObject::CombatShrineDone => GREY,
            WorldObject::GambleShrine => GREY,
            WorldObject::GambleShrineDone => GREY,
            WorldObject::ActiveSkillShrine => GREY,
            WorldObject::ActiveSkillShrineDone => GREY,
            WorldObject::HeirloomShrine => GREY,
            WorldObject::HeirloomShrineDone => GREY,
            WorldObject::MicrowaveShrine => GREY,
            WorldObject::MicrowaveShrineDone => GREY,
            WorldObject::BlacksmithMerchant => GREY,
            WorldObject::BlacksmithMerchantDone => GREY,
            WorldObject::BossShrine => RED,
            WorldObject::DungeonEntrance => DARK_GREEN,
            WorldObject::TimeGate => BLUE,
            //era2
            WorldObject::Era2Boulder => LIGHT_GREY,
            WorldObject::Era2Boulder2 => LIGHT_GREY,
            WorldObject::Era2CoalBoulder => LIGHT_GREY,
            WorldObject::Era2MagicBoulder => LIGHT_GREY,

            WorldObject::Era2Grass => LIGHT_GREEN,
            WorldObject::Era2Grass2 => LIGHT_GREEN,
            WorldObject::Era2Grass3 => LIGHT_GREEN,

            WorldObject::Era2Pebble => LIGHT_GREEN,
            WorldObject::Era2RedMushroom => LIGHT_GREEN,
            WorldObject::Era2BrownMushroom => LIGHT_GREEN,
            WorldObject::Era2Stump => LIGHT_BROWN,
            WorldObject::Era2Stump2 => LIGHT_BROWN,
            WorldObject::Era2DeadBranch => LIGHT_BROWN,

            // Desert
            WorldObject::MedCactus1 => LIGHT_GREEN,
            WorldObject::MedCactus2 => LIGHT_GREEN,
            WorldObject::MedCactus3 => LIGHT_GREEN,
            WorldObject::MedCactus4 => LIGHT_GREEN,
            WorldObject::MedCactus5 => LIGHT_GREEN,
            WorldObject::SmlCactus1 => LIGHT_GREEN,
            WorldObject::SmlCactus2 => LIGHT_GREEN,
            WorldObject::SmlCactus3 => LIGHT_GREEN,
            WorldObject::FlowerCactus1 => LIGHT_GREEN,
            WorldObject::FlowerCactus2 => LIGHT_GREEN,
            WorldObject::FlowerCactus3 => LIGHT_GREEN,
            WorldObject::FlowerCactus4 => LIGHT_GREEN,
            WorldObject::MedFruitCactus1 => LIGHT_GREEN,
            WorldObject::MedFruitCactus2 => LIGHT_GREEN,
            WorldObject::SmlFruitCactus1 => LIGHT_GREEN,
            WorldObject::SmlFruitCactus2 => LIGHT_GREEN,
            WorldObject::SmlFruitCactus3 => LIGHT_GREEN,
            WorldObject::SmlFruitCactus4 => LIGHT_GREEN,

            WorldObject::DesertLargeCactus1 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus2 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus3 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus4 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus5 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus6 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus7 => LIGHT_GREEN,
            WorldObject::DesertLargeCactus8 => LIGHT_GREEN,

            WorldObject::DesertMedBoulder1 => LIGHT_BROWN,
            WorldObject::DesertMedBoulder2 => LIGHT_BROWN,
            WorldObject::DesertMedBoulder3 => LIGHT_BROWN,
            WorldObject::DesertSmlBoulder1 => LIGHT_BROWN,
            WorldObject::DesertSmlBoulder2 => LIGHT_BROWN,
            WorldObject::DesertSmlBoulder3 => LIGHT_BROWN,
            WorldObject::DesertSmlBoulder4 => LIGHT_BROWN,
            WorldObject::DesertSmlBoulder5 => LIGHT_BROWN,
            WorldObject::DesertSmlBoulder6 => LIGHT_BROWN,

            WorldObject::DesertDriftWood1 => DARK_BROWN,
            WorldObject::DesertDriftWood2 => DARK_BROWN,
            WorldObject::DesertDriftWood3 => DARK_BROWN,
            WorldObject::DesertFence1 => DARK_BROWN,
            WorldObject::DesertFence2 => DARK_BROWN,
            WorldObject::DesertFence3 => DARK_BROWN,

            WorldObject::DesertSkull1 => WHITE,
            WorldObject::DesertSkull2 => WHITE,
            WorldObject::DesertSkull3 => WHITE,
            WorldObject::DesertSkull4 => WHITE,
            WorldObject::DesertBones1 => WHITE,
            WorldObject::DesertBones2 => WHITE,
            WorldObject::DesertBones3 => WHITE,

            WorldObject::Tumbleweed1 => LIGHT_BROWN,
            WorldObject::Tumbleweed2 => LIGHT_BROWN,
            WorldObject::Tumbleweed3 => LIGHT_BROWN,
            WorldObject::Tumbleweed4 => LIGHT_BROWN,

            //Snow
            WorldObject::SnowMedBoulder1 => SNOW_DARK,
            WorldObject::SnowMedBoulder2 => SNOW_DARK,
            WorldObject::SnowMedBoulder3 => SNOW_DARK,
            WorldObject::SnowSmlBoulder1 => SNOW_DARK,
            WorldObject::SnowSmlBoulder2 => SNOW_DARK,
            WorldObject::SnowSmlBoulder3 => SNOW_DARK,
            WorldObject::SnowSmlBoulder4 => SNOW_DARK,
            WorldObject::SnowSmlBoulder5 => SNOW_DARK,
            WorldObject::SnowSmlBoulder6 => SNOW_DARK,

            WorldObject::SnowLeaflessTree1 => LIGHT_BROWN,
            WorldObject::SnowLeaflessTree2 => LIGHT_BROWN,
            WorldObject::SnowLeaflessTree3 => LIGHT_BROWN,
            WorldObject::SnowTree1 => SNOW_GREEN,
            WorldObject::SnowTree2 => SNOW_GREEN,
            WorldObject::SnowTree3 => SNOW_GREEN,
            WorldObject::SnowTree4 => SNOW_GREEN,
            WorldObject::SnowBush1 => SNOW_GREEN,
            WorldObject::SnowBush2 => SNOW_GREEN,
            WorldObject::SnowBush3 => SNOW_GREEN,
            WorldObject::SnowBush4 => SNOW_GREEN,
            WorldObject::SnowBushMed1 => SNOW_GREEN,
            WorldObject::SnowBushMed2 => SNOW_GREEN,
            WorldObject::SnowBerryBushSml1 => SNOW_GREEN,
            WorldObject::SnowBerryBushSml2 => SNOW_GREEN,
            WorldObject::SnowBerryBushSml3 => SNOW_GREEN,
            WorldObject::SnowBerryBushSml4 => SNOW_GREEN,
            WorldObject::SnowBerryBushMed1 => SNOW_GREEN,
            WorldObject::SnowBerryBushMed2 => SNOW_GREEN,

            WorldObject::SnowBushSnow1 => WHITE,
            WorldObject::SnowBushSnow2 => WHITE,
            WorldObject::SnowBushSnow3 => WHITE,
            WorldObject::SnowBushSnow4 => WHITE,
            WorldObject::SnowBushSnowMed1 => WHITE,
            WorldObject::SnowBushSnowMed2 => WHITE,
            WorldObject::SnowFlower => WHITE,
            WorldObject::SnowCrate1 => LIGHT_BROWN,
            WorldObject::SnowCrate2 => LIGHT_BROWN,
            WorldObject::SnowCrate3 => LIGHT_BROWN,
            WorldObject::SnowCrate4 => LIGHT_BROWN,
            WorldObject::SnowDeadSapling => LIGHT_BROWN,

            WorldObject::SnowCrystalSml1 => SNOW_BLUE,
            WorldObject::SnowCrystalSml2 => SNOW_BLUE,
            WorldObject::SnowCrystalSml3 => SNOW_BLUE,
            WorldObject::SnowCrystalSml4 => SNOW_BLUE,
            WorldObject::SnowCrystalMed1 => SNOW_BLUE,
            WorldObject::SnowCrystalMed2 => SNOW_BLUE,
            WorldObject::SnowMushroom1 => SNOW_BLUE,
            WorldObject::SnowMushroom2 => SNOW_BLUE,

            WorldObject::IcePatch => SHIELD_BLUE,

            _ => BLACK,
        }
    }
    pub fn override_material_drop_toggle(&self) -> bool {
        match self {
            WorldObject::XPJug => true,
            WorldObject::SnowCrystalMed1 => true,
            WorldObject::SnowCrystalMed2 => true,
            WorldObject::SnowCrystalSml1 => true,
            WorldObject::SnowCrystalSml2 => true,
            WorldObject::SnowCrystalSml3 => true,
            WorldObject::SnowCrystalSml4 => true,
            WorldObject::DesertSkull1 => true,
            WorldObject::DesertSkull2 => true,
            WorldObject::DesertSkull3 => true,
            WorldObject::DesertSkull4 => true,
            _ => false,
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

/// Timer component for despawning ItemDrop entities after a certain time
/// This helps reduce lag in endless mode by cleaning up uncollected items
#[derive(Component)]
pub struct ItemDropDespawnTimer(pub Timer);

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldObjectResource::new())
            .init_resource::<BossSummonTracker>()
            .init_resource::<BreakDropFilter>()
            .insert_resource(AmmoMemory::default())
            .init_resource::<BridgePlacementMode>()
            .add_event::<PlaceItemEvent>()
            .add_event::<UpdateObjectEvent>()
            .add_event::<CombatShrineMobDeathEvent>()
            .add_event::<DungeonShrineMobDeathEvent>()
            .add_event::<GambleShrineEvent>()
            .add_plugin(CraftingPlugin)
            .add_plugin(RangedAttackPlugin)
            .add_plugin(LootTablePlugin)
            .add_plugin(ItemDropOutlinePlugin)
            .add_system(
                handle_break_object
                    .before(CustomFlush)
                    .after(spawn_obj_death_particles)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_placing_world_object
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_systems(
                (
                    handle_pay_shrine_cost,
                    handle_delayed_spawns.run_if(resource_exists::<DelayedSpawn>()),
                    handle_item_action_success,
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
                    handle_frozen_ticks.run_if(is_not_paused),
                    check_freeze_on_slow_stacks.run_if(is_not_paused),
                    handle_combat_shrine_activate_animation,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                enhance_combat_shrine_mobs.in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_bridge_placement_mode
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_bridge_preview_dragging
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(ensure_mob_status_effects.in_set(OnUpdate(GameState::Main)))
            .add_systems(
                (
                    update_boss_shrine_guide_cost,
                    ensure_boss_shrine_sprite_on_spawn,
                    handle_on_hit_upgrades.run_if(is_not_paused),
                    handle_reset_proj_hit_enemies_state.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_item_drop_despawn_timer
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                tick_reload
                    .run_if(is_not_paused)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    spawn_pink_flower_aseprite,
                    handle_pink_flower_animation_loop,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    add_active_skill_shrine_visuals_on_spawn,
                    handle_active_skill_shrine_completion,
                    handle_active_skill_shrine_esc,
                    handle_dungeon_shrine_rewards,
                    add_dungeon_shrine_visuals_on_spawn,
                    handle_dungeon_shrine_activation,
                    add_heirloom_shrine_visuals_on_spawn,
                    handle_heirloom_shrine_completion,
                    add_microwave_shrine_visuals_on_spawn,
                    handle_microwave_shrine_completion,
                    handle_microwave_shrine_esc,
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
    ground_patch_graphics: Res<GroundPatchesGraphics>,
    mut events: EventReader<PlaceItemEvent>,
    water_colliders: Query<
        (Entity, &Collider, &GlobalTransform),
        (Without<WorldObject>, Without<Mob>, Without<Player>),
    >,
    container_reg: Res<ContainerRegistry>,
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
        let chunk_entity = game.get_chunk_entity(tile_pos.chunk_pos);
        match chunk_entity {
            Some(chunk) => {
                let is_touching_air = true;
                let item = proto_commands.spawn_object_from_proto(
                    place_event.obj,
                    pos,
                    &prototypes,
                    &mut proto_param,
                    is_touching_air,
                );
                match item {
                    Some(item_e) => {
                        // Successfully spawned - register in cache
                        game.add_object_to_chunk_cache(tile_pos, place_event.obj);
                        //TODO: do what old game data did, add obj to registry
                        safe_set_parent(&mut commands, item_e, chunk);

                        if place_event.obj == WorldObject::BlacksmithMerchant {
                            commands.entity(item_e).insert(EssenceShopChoices {
                                tile_pos: Some(tile_pos),
                                ..default()
                            });
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

                        let suppress_grass_patch_near_water = object_within_tile_radius_of_water(
                            tile_pos,
                            place_event.obj,
                            &game,
                            &proto_param,
                            GROUND_PATCH_WATER_CHECK_RADIUS_TILES,
                        );
                        match game.era.current_era {
                            Era::Main if !suppress_grass_patch_near_water => {
                                let anchor = proto_param
                                    .get_component::<SpriteAnchor, _>(place_event.obj)
                                    .map(|a| a.0)
                                    .unwrap_or(Vec2::ZERO);
                                if let Some((patch, placement, y_sort, extra_child_z)) =
                                    if place_event.obj.is_tree() {
                                        Some((
                                            GroundPatch::GrassPatch3,
                                            GroundPatchPlacement::Tree,
                                            GROUND_PATCH_YSORT_KEY_8,
                                            0.,
                                        ))
                                    } else if place_event.obj.spawns_grass_patch_9_shrine_under() {
                                        Some((
                                            GroundPatch::GrassPatch4,
                                            GroundPatchPlacement::Shrine { anchor },
                                            GROUND_PATCH_YSORT_KEY_9,
                                            0.,
                                        ))
                                    } else if place_event.obj.spawns_grass_patch_7_under() {
                                        Some((
                                            GroundPatch::GrassPatch2,
                                            GroundPatchPlacement::Object,
                                            GROUND_PATCH_YSORT_KEY_7,
                                            GROUND_PATCH_CHILD_Z_OBJECT_BONUS,
                                        ))
                                    } else {
                                        None
                                    }
                                {
                                    spawn_ground_patch(
                                        &mut commands,
                                        &ground_patch_graphics,
                                        patch,
                                        pos,
                                        y_sort,
                                        Some(item_e),
                                        grass_patch_parent_local_offset(placement),
                                        extra_child_z,
                                    );
                                }
                            }
                            Era::Second => {
                                if let Some(desert_patch) = place_event.obj.desert_ground_patch() {
                                    let suppress_desert_patch_near_water =
                                        object_within_tile_radius_of_water(
                                            tile_pos,
                                            place_event.obj,
                                            &game,
                                            &proto_param,
                                            desert_patch_water_radius(desert_patch),
                                        );
                                    if !suppress_desert_patch_near_water {
                                        let anchor = proto_param
                                            .get_component::<SpriteAnchor, _>(place_event.obj)
                                            .map(|a| a.0)
                                            .unwrap_or(Vec2::ZERO);
                                        let placement = if place_event.obj.is_large_desert_cactus()
                                        {
                                            GroundPatchPlacement::Tree
                                        } else if place_event
                                            .obj
                                            .spawns_grass_patch_9_shrine_under()
                                        {
                                            GroundPatchPlacement::Shrine { anchor }
                                        } else {
                                            GroundPatchPlacement::Object
                                        };
                                        let (y_sort, extra_child_z) = match placement {
                                            GroundPatchPlacement::Tree => {
                                                (GROUND_PATCH_YSORT_KEY_8, 0.)
                                            }
                                            GroundPatchPlacement::Shrine { .. } => {
                                                (GROUND_PATCH_YSORT_KEY_9, 0.)
                                            }
                                            GroundPatchPlacement::Object => (
                                                GROUND_PATCH_YSORT_KEY_7,
                                                GROUND_PATCH_CHILD_Z_OBJECT_BONUS,
                                            ),
                                        };
                                        spawn_ground_patch(
                                            &mut commands,
                                            &ground_patch_graphics,
                                            desert_patch,
                                            pos,
                                            y_sort,
                                            Some(item_e),
                                            desert_patch_parent_local_offset(
                                                placement,
                                                desert_patch,
                                                anchor,
                                            ),
                                            extra_child_z,
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    None => {
                        // spawn_object_from_proto returned None - prototype not ready or other issue
                        // Cache for later spawning
                        game.add_object_to_chunk_cache(tile_pos, place_event.obj);
                    }
                }
            }
            None => {
                // Chunk entity not found - cache objects that can't be placed yet (chunk not ready)
                // so they can be spawned later when chunk is ready
                game.add_object_to_chunk_cache(tile_pos, place_event.obj);
            }
        }
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
    xp: Query<&ExperienceReward>,
    mut analytics_events: EventWriter<AnalyticsUpdateEvent>,
    water_colliders: Query<
        (Entity, &Collider, &GlobalTransform),
        (Without<WorldObject>, Without<Mob>, Without<Player>),
    >,
    mut chaos_tracker: ResMut<ChaosTracker>,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    break_drop_filter: Res<BreakDropFilter>,
    mobs: Query<&Mob>,
) {
    for broken in obj_break_events.iter() {
        let mut rng = rand::thread_rng();
        let world_pos = tile_pos_to_world_pos(broken.pos, false);
        let is_mob = mobs.get(broken.entity).is_ok();
        let filter_non_mob_drops = !is_mob && !break_drop_filter.0.is_empty();
        // Chest

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

        if let Some(entity_commands) = commands.get_entity(broken.entity) {
            entity_commands.despawn_recursive();
        }
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
        // Item drops: per-item filter for non-mobs (and crates); special breakables bypass via
        // `override_material_drop_toggle` (XP jugs, crystals, desert skulls).
        if let Ok(loot_table) = loot_tables.get(broken.entity) {
            let bypass_filter = is_mob || broken.obj.override_material_drop_toggle();
            for drop in LootTablePlugin::get_drops(loot_table, &proto_param, 0, None, false, false)
            {
                if filter_non_mob_drops
                    && !bypass_filter
                    && break_drop_filter.is_blocked(drop.obj_type)
                {
                    continue;
                }
                // TEMP DIAGNOSTIC (drop-filter leak): fires whenever a *filterable* material
                // reaches the ground. Shows whether the filter is populated and why this item
                // slipped through (not blocked vs. bypassed). Remove once the leak is resolved.
                if !is_mob && crate::inventory::BREAK_DROP_FILTER_ITEMS.contains(&drop.obj_type) {
                    warn!(
                        "[DROP-FILTER] spawned {:?} from {:?} | filter_active={} bypass={} blocked={} filter_size={} contents={:?}",
                        drop.obj_type,
                        broken.obj,
                        filter_non_mob_drops,
                        bypass_filter,
                        break_drop_filter.is_blocked(drop.obj_type),
                        break_drop_filter.0.len(),
                        break_drop_filter.0,
                    );
                }
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
            let xp_rate_bonus = game.get_xp_rate_bonus();
            let mut player_xp = game.get_player_level_mut();
            let (did_level, gained_xp) =
                player_xp.add_xp(exp.0, xp_rate_bonus, &mut chaos_tracker);
            let t = tile_pos_to_world_pos(broken.pos, true);
            spawn_xp_particles(t, &mut commands, gained_xp, did_level);
            flash_event.send(FlashExpBarEvent {
                amount: gained_xp,
                did_level,
            });
        }

        // Analytics
        analytics_events.send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::ObjectBroken(broken.obj),
        });
    }
}

/// System to handle despawning ItemDrop entities after their timer expires
/// This helps reduce lag in endless mode by cleaning up uncollected items
fn handle_item_drop_despawn_timer(
    mut commands: Commands,
    time: Res<Time>,
    mut item_drops: Query<(Entity, &mut ItemDropDespawnTimer)>,
) {
    for (entity, mut timer) in item_drops.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            if let Some(commands) = commands.get_entity(entity) {
                commands.despawn_recursive();
            }
        }
    }
}
