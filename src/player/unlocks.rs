use bevy::prelude::*;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

use super::{
    achievements::{Achievement, Achievements},
    currency::TimeFragmentCurrency,
};
use crate::datafiles;
use crate::item::WorldObject;
use crate::player::skills::SkillClass;

#[derive(Resource, Debug, Clone, Default)]
pub struct UnlockedClasses {
    classes: HashSet<SkillClass>,
}

impl UnlockedClasses {
    pub fn new<I: IntoIterator<Item = SkillClass>>(classes: I) -> Self {
        Self {
            classes: classes.into_iter().collect(),
        }
    }

    pub fn contains(&self, class: &SkillClass) -> bool {
        self.classes.contains(class)
    }

    pub fn insert(&mut self, class: SkillClass) -> bool {
        self.classes.insert(class)
    }

    pub fn _iter(&self) -> impl Iterator<Item = &SkillClass> {
        self.classes.iter()
    }

    pub fn to_vec(&self) -> Vec<SkillClass> {
        self.classes.iter().cloned().collect()
    }

    pub fn ensure_defaults(&mut self, defaults: &[SkillClass]) {
        for class in defaults {
            self.classes.insert(class.clone());
        }
    }
}

/// Per-class unlocked active skill slots beyond the default (0, 1). Persists
/// legacy per-class purchases of the 4th class skill (slot 3) using Time
/// Fragments. Slot 2 is unlocked globally via [`UnlockUpgrades::third_skill_slot_unlocked`].
/// Slots 0 and 1 are always considered unlocked and are not tracked here.
#[derive(Resource, Debug, Clone, Default, Serialize, Deserialize)]
pub struct UnlockedSkills {
    /// Set of `(class, slot_index)` pairs that have been purchased. Only slot
    /// indices 2 and 3 are ever inserted; 0 and 1 are free.
    pub entries: HashSet<(SkillClass, usize)>,
}

impl UnlockedSkills {
    pub fn new<I: IntoIterator<Item = (SkillClass, usize)>>(entries: I) -> Self {
        Self {
            entries: entries.into_iter().collect(),
        }
    }

    /// Returns true if the given class skill slot is available to the player.
    /// Slots 0 and 1 are always unlocked. Slot 2 requires the global
    /// [`UnlockUpgrades::third_skill_slot_unlocked`] shop purchase (or a legacy
    /// per-class entry). Slot 3 still requires a per-class purchase. Any other
    /// slot index is considered unlocked (e.g. blessing-granted slot 4).
    pub fn is_unlocked(
        &self,
        class: &SkillClass,
        slot: usize,
        unlock_upgrades: &UnlockUpgrades,
    ) -> bool {
        match slot {
            0 | 1 => true,
            2 => {
                unlock_upgrades.third_skill_slot_unlocked
                    || self.entries.contains(&(class.clone(), slot))
            }
            3 => self.entries.contains(&(class.clone(), slot)),
            _ => true,
        }
    }

    pub fn insert(&mut self, class: SkillClass, slot: usize) -> bool {
        self.entries.insert((class, slot))
    }

    pub fn to_vec(&self) -> Vec<(SkillClass, usize)> {
        self.entries.iter().cloned().collect()
    }
}

#[derive(Asset, TypePath, Deserialize, Clone, Debug)]
pub struct ClassUnlockConfig {
    pub classes: HashMap<SkillClass, ClassUnlockEntry>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ClassUnlockEntry {
    pub achievements: Vec<Achievement>,
    pub cost: u32,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ClassUnlockData {
    pub classes: HashMap<SkillClass, ClassUnlockEntry>,
}

impl ClassUnlockData {
    pub fn from_config(config: &ClassUnlockConfig) -> Self {
        Self {
            classes: config.classes.clone(),
        }
    }

    pub fn entry(&self, class: &SkillClass) -> Option<&ClassUnlockEntry> {
        self.classes.get(class)
    }
}

const UNLOCK_COST_SCALE: f32 = 1.75;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnlockUpgradeKind {
    Reroll,
    Banish,
    StartSupplies,
    StartStatBoosts,
    StartTome,
    StartOrb,
    StartingTools,
    MapMarkers,
    ThirdSkillSlot,
}

impl UnlockUpgradeKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            UnlockUpgradeKind::Reroll => "Reroll",
            UnlockUpgradeKind::Banish => "Banish",
            UnlockUpgradeKind::StartSupplies => "Start with Supplies",
            UnlockUpgradeKind::StartStatBoosts => "Start with Stat Boosts",
            UnlockUpgradeKind::StartTome => "Start with Tomes",
            UnlockUpgradeKind::StartOrb => "Start with Orbs",
            UnlockUpgradeKind::StartingTools => "Starting Tools",
            UnlockUpgradeKind::MapMarkers => "Map Markers",
            UnlockUpgradeKind::ThirdSkillSlot => "3rd Skill Slot",
        }
    }
    pub fn is_disabled(&self) -> bool {
        match self {
            UnlockUpgradeKind::Reroll => false,
            UnlockUpgradeKind::Banish => false,
            UnlockUpgradeKind::StartSupplies => false,
            UnlockUpgradeKind::StartStatBoosts => false,
            UnlockUpgradeKind::StartTome => true,
            UnlockUpgradeKind::StartOrb => true,
            UnlockUpgradeKind::StartingTools => false,
            UnlockUpgradeKind::MapMarkers => false,
            UnlockUpgradeKind::ThirdSkillSlot => false,
        }
    }
}

/// Persisted in `game_data.json`. `#[serde(default)]` keeps older saves loadable when new
/// tier fields are added (missing keys deserialize as `0` / `false`).
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UnlockUpgrades {
    pub reroll_tier: u32,
    pub banish_tier: u32,
    pub food_tier: u32,
    pub stat_boost_tier: u32,
    pub tome_tier: u32,
    pub orb_tier: u32,
    pub starting_tools_tier: u32,
    #[serde(alias = "second_active_skill_slot_unlocked")]
    pub third_skill_slot_unlocked: bool,
    /// Each tier unlocks one extra map marker beyond the first (max tier 2 -> 3 markers).
    pub map_marker_tier: u32,
}

impl UnlockUpgrades {
    fn base_cost(kind: UnlockUpgradeKind) -> u32 {
        match kind {
            UnlockUpgradeKind::Reroll => 5,
            UnlockUpgradeKind::Banish => 20,
            UnlockUpgradeKind::StartSupplies => 8,
            UnlockUpgradeKind::StartStatBoosts => 10,
            UnlockUpgradeKind::StartTome => 25,
            UnlockUpgradeKind::StartOrb => 30,
            UnlockUpgradeKind::StartingTools => 50, // Tier 1: WoodAxe, Tier 2: Pickaxe
            UnlockUpgradeKind::MapMarkers => 40,
            UnlockUpgradeKind::ThirdSkillSlot => 100,
        }
    }

    pub fn tier(&self, kind: UnlockUpgradeKind) -> u32 {
        match kind {
            UnlockUpgradeKind::Reroll => self.reroll_tier,
            UnlockUpgradeKind::Banish => self.banish_tier,
            UnlockUpgradeKind::StartSupplies => self.food_tier,
            UnlockUpgradeKind::StartStatBoosts => self.stat_boost_tier,
            UnlockUpgradeKind::StartTome => self.tome_tier,
            UnlockUpgradeKind::StartOrb => self.orb_tier,
            UnlockUpgradeKind::StartingTools => self.starting_tools_tier,
            UnlockUpgradeKind::MapMarkers => self.map_marker_tier,
            UnlockUpgradeKind::ThirdSkillSlot => u32::from(self.third_skill_slot_unlocked),
        }
    }

    /// Total number of map markers the player can place (1 base + one per tier, capped at 3).
    pub fn map_marker_count(&self) -> u32 {
        (1 + self.map_marker_tier).min(3)
    }

    pub fn increment(&mut self, kind: UnlockUpgradeKind) {
        match kind {
            UnlockUpgradeKind::Reroll => self.reroll_tier = self.reroll_tier.saturating_add(1),
            UnlockUpgradeKind::Banish => self.banish_tier = self.banish_tier.saturating_add(1),
            UnlockUpgradeKind::StartSupplies => self.food_tier = self.food_tier.saturating_add(1),
            UnlockUpgradeKind::StartStatBoosts => {
                self.stat_boost_tier = self.stat_boost_tier.saturating_add(1)
            }
            UnlockUpgradeKind::StartTome => self.tome_tier = self.tome_tier.saturating_add(1),
            UnlockUpgradeKind::StartOrb => self.orb_tier = self.orb_tier.saturating_add(1),
            UnlockUpgradeKind::StartingTools => {
                // Cap at tier 2 (Wood Axe, then Pickaxe).
                self.starting_tools_tier = (self.starting_tools_tier + 1).min(2);
            }
            UnlockUpgradeKind::MapMarkers => {
                self.map_marker_tier = (self.map_marker_tier + 1).min(2);
            }
            UnlockUpgradeKind::ThirdSkillSlot => self.third_skill_slot_unlocked = true,
        }
    }

    pub fn is_unlocked(&self, kind: UnlockUpgradeKind) -> bool {
        match kind {
            UnlockUpgradeKind::ThirdSkillSlot => self.third_skill_slot_unlocked,
            _ => self.tier(kind) > 0,
        }
    }

    pub fn has_wood_axe(&self) -> bool {
        self.starting_tools_tier >= 1
    }

    pub fn has_pickaxe(&self) -> bool {
        self.starting_tools_tier >= 2
    }

    pub fn next_cost(&self, kind: UnlockUpgradeKind) -> u32 {
        let tier = self.tier(kind) as i32;
        match kind {
            UnlockUpgradeKind::StartingTools => {
                match tier {
                    0 => 50,  // Tier 1: WoodAxe
                    1 => 100, // Tier 2: Pickaxe (70 more)
                    _ => 0,   // Max tier reached
                }
            }
            UnlockUpgradeKind::MapMarkers => {
                match tier {
                    0 => 20, // 2nd marker
                    1 => 40, // 3rd marker
                    _ => 0,  // Max tier reached
                }
            }
            UnlockUpgradeKind::ThirdSkillSlot => {
                if self.third_skill_slot_unlocked {
                    0
                } else {
                    100
                }
            }
            UnlockUpgradeKind::Reroll => {
                match tier {
                    0 => 15,
                    1 => 25,
                    2 => 50,
                    3 => 100,
                    4 => 200,
                    5 => 400,
                    6 => 500,
                    _ => 999999, // max tier
                }
            }
            UnlockUpgradeKind::Banish => {
                match tier {
                    0 => 20,
                    1 => 50,
                    2 => 85,
                    3 => 125,
                    4 => 275,
                    5 => 550,
                    _ => 999999, // max tier
                }
            }
            UnlockUpgradeKind::StartTome => {
                match tier {
                    0 => 25,
                    1 => 50,
                    2 => 75,
                    3 => 100,
                    4 => 200,
                    5 => 400,
                    _ => 999999, // max tier
                }
            }
            UnlockUpgradeKind::StartOrb => {
                match tier {
                    0 => 30,
                    1 => 60,
                    2 => 90,
                    3 => 120,
                    4 => 240,
                    5 => 480,
                    _ => 999999, // max tier
                }
            }

            _ => {
                let base = Self::base_cost(kind) as f32;
                let tier = self.tier(kind) as i32;
                let scaled = base * UNLOCK_COST_SCALE.powi(tier);
                scaled.round().max(1.) as u32
            }
        }
    }

    pub fn reroll_total(&self) -> u32 {
        3 + self.reroll_tier
    }

    pub fn banish_total(&self) -> u32 {
        self.banish_tier
    }

    pub fn supplies_count(&self) -> u32 {
        self.food_tier
    }

    pub fn stat_boost_count(&self) -> u32 {
        self.stat_boost_tier
    }

    pub fn tome_count(&self) -> u32 {
        self.tome_tier
    }

    pub fn orb_count(&self) -> u32 {
        self.orb_tier
    }

    /// Check if an unlock is maxed out (cannot be purchased further)
    pub fn is_maxed(&self, kind: UnlockUpgradeKind) -> bool {
        match kind {
            UnlockUpgradeKind::StartingTools => self.starting_tools_tier >= 2,
            UnlockUpgradeKind::Reroll => self.reroll_tier >= 7,
            UnlockUpgradeKind::Banish => self.banish_tier >= 5,
            UnlockUpgradeKind::StartSupplies => self.food_tier >= 6,
            UnlockUpgradeKind::StartStatBoosts => self.stat_boost_tier >= 5,
            UnlockUpgradeKind::StartTome => self.tome_tier >= 5,
            UnlockUpgradeKind::StartOrb => self.orb_tier >= 5,
            UnlockUpgradeKind::MapMarkers => self.map_marker_tier >= 2,
            UnlockUpgradeKind::ThirdSkillSlot => self.third_skill_slot_unlocked,
        }
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct RunUnlockState {
    pub rerolls_total: u32,
    pub rerolls_remaining: u32,
    pub banishes_total: u32,
    pub banishes_remaining: u32,
    pub pending_supplies: u32,
    pub pending_stat_boosts: u32,
    pub pending_tomes: u32,
    pub pending_orbs: u32,
    pub pending_rewards: bool,
}

#[derive(Clone, Copy)]
pub struct SupplyDrop {
    pub object: WorldObject,
    pub count: usize,
}

pub const STARTING_SUPPLY_POOL: [SupplyDrop; 6] = [
    SupplyDrop {
        object: WorldObject::Apple,
        count: 3,
    },
    SupplyDrop {
        object: WorldObject::BerryJam,
        count: 1,
    },
    SupplyDrop {
        object: WorldObject::RedStew,
        count: 1,
    },
    SupplyDrop {
        object: WorldObject::SmallPotion,
        count: 1,
    },
    SupplyDrop {
        object: WorldObject::MovementSpeedPotion,
        count: 1,
    },
    SupplyDrop {
        object: WorldObject::AttackSpeedPotion,
        count: 1,
    },
];

pub const STARTING_STAT_BOOST_POOL: [WorldObject; 12] = [
    WorldObject::SpeedFood,
    WorldObject::HealthFood,
    WorldObject::ManaFood,
    WorldObject::ThornsFood,
    WorldObject::CritChanceFood,
    WorldObject::LifestealFood,
    WorldObject::SkillPowerFood,
    WorldObject::ManaRegenFood,
    WorldObject::DodgeFood,
    WorldObject::DefenceFood,
    WorldObject::SizeFood,
    WorldObject::AttackSpeedFood,
];

/// Pick `tier` unique supply drops from [`STARTING_SUPPLY_POOL`] (max 6).
pub fn roll_starting_supplies(tier: u32) -> Vec<SupplyDrop> {
    let pick_count = tier.min(STARTING_SUPPLY_POOL.len() as u32) as usize;
    if pick_count == 0 {
        return Vec::new();
    }
    let mut pool: Vec<SupplyDrop> = STARTING_SUPPLY_POOL.to_vec();
    let mut rng = rand::thread_rng();
    pool.shuffle(&mut rng);
    pool.truncate(pick_count);
    pool
}

/// Pick `tier` random stat-boost foods from [`STARTING_STAT_BOOST_POOL`] (max 5, duplicates allowed).
pub fn roll_starting_stat_boosts(tier: u32) -> Vec<WorldObject> {
    let pick_count = tier.min(5) as usize;
    if pick_count == 0 {
        return Vec::new();
    }
    let mut rng = rand::thread_rng();
    (0..pick_count)
        .map(|_| {
            *STARTING_STAT_BOOST_POOL
                .choose(&mut rng)
                .unwrap_or(&WorldObject::DodgeFood)
        })
        .collect()
}

impl RunUnlockState {
    /// `banishes_from_time_crystals`: +1 starting banish per **completed** time crystal
    /// ([`crate::player::time_crystals::TimeCrystals::completed_count`]), on top of shop tiers.
    pub fn reset_for_run(&mut self, upgrades: &UnlockUpgrades, banishes_from_time_crystals: u32) {
        self.rerolls_total = upgrades.reroll_total();
        self.rerolls_remaining = self.rerolls_total;
        self.banishes_total = upgrades
            .banish_total()
            .saturating_add(banishes_from_time_crystals);
        self.banishes_remaining = self.banishes_total;
        self.pending_supplies = upgrades.supplies_count();
        self.pending_stat_boosts = upgrades.stat_boost_count();
        self.pending_tomes = upgrades.tome_count();
        self.pending_orbs = upgrades.orb_count();
        self.pending_rewards = self.pending_supplies > 0
            || self.pending_stat_boosts > 0
            || self.pending_tomes > 0
            || self.pending_orbs > 0;
    }
}

pub fn persist_unlock_data(
    time_fragment_currency: Option<&TimeFragmentCurrency>,
    unlocked_classes: Option<&UnlockedClasses>,
    achievements: Option<&Achievements>,
    unlock_upgrades: Option<&UnlockUpgrades>,
    unlocked_skills: Option<&UnlockedSkills>,
) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        crate::client::GameData::default()
    };

    if let Some(achievements) = achievements {
        game_data.achievements = achievements.clone();
    }
    if let Some(currency) = time_fragment_currency {
        game_data.time_fragments = currency.time_fragments as u128;
    }
    if let Some(classes) = unlocked_classes {
        game_data.unlocked_classes = classes.to_vec();
    }
    if let Some(upgrades) = unlock_upgrades {
        game_data.unlock_upgrades = upgrades.clone();
    }
    if let Some(skills) = unlocked_skills {
        game_data.unlocked_skills = skills.clone();
    }

    match File::create(&path) {
        Ok(file) => {
            if let Err(err) = serde_json::to_writer(file, &game_data) {
                error!("Failed to write game_data.json: {err:?}");
            }
        }
        Err(err) => error!("Failed to create game_data.json: {err:?}"),
    }
}
