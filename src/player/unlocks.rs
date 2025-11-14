use bevy::{prelude::*, reflect::TypeUuid};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;

use super::{
    achievements::{Achievement, Achievements},
    currency::TimeFragmentCurrency,
};
use crate::datafiles;
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

#[derive(Deserialize, TypeUuid, Clone, Debug)]
#[uuid = "2fd56698-6f3b-45aa-8a7f-6f9f8700b5a1"]
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
    StartFood,
    StartTome,
    StartOrb,
}

impl UnlockUpgradeKind {
    pub fn display_name(&self) -> &'static str {
        match self {
            UnlockUpgradeKind::Reroll => "Reroll",
            UnlockUpgradeKind::Banish => "Banish",
            UnlockUpgradeKind::StartFood => "Start with Food",
            UnlockUpgradeKind::StartTome => "Start with Tomes",
            UnlockUpgradeKind::StartOrb => "Start with Orbs",
        }
    }
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnlockUpgrades {
    pub reroll_tier: u32,
    pub banish_tier: u32,
    pub food_tier: u32,
    pub tome_tier: u32,
    pub orb_tier: u32,
}

impl UnlockUpgrades {
    fn base_cost(kind: UnlockUpgradeKind) -> u32 {
        match kind {
            UnlockUpgradeKind::Reroll => 5,
            UnlockUpgradeKind::Banish => 5,
            UnlockUpgradeKind::StartFood => 8,
            UnlockUpgradeKind::StartTome => 10,
            UnlockUpgradeKind::StartOrb => 10,
        }
    }

    pub fn tier(&self, kind: UnlockUpgradeKind) -> u32 {
        match kind {
            UnlockUpgradeKind::Reroll => self.reroll_tier,
            UnlockUpgradeKind::Banish => self.banish_tier,
            UnlockUpgradeKind::StartFood => self.food_tier,
            UnlockUpgradeKind::StartTome => self.tome_tier,
            UnlockUpgradeKind::StartOrb => self.orb_tier,
        }
    }

    pub fn increment(&mut self, kind: UnlockUpgradeKind) {
        match kind {
            UnlockUpgradeKind::Reroll => self.reroll_tier = self.reroll_tier.saturating_add(1),
            UnlockUpgradeKind::Banish => self.banish_tier = self.banish_tier.saturating_add(1),
            UnlockUpgradeKind::StartFood => self.food_tier = self.food_tier.saturating_add(1),
            UnlockUpgradeKind::StartTome => self.tome_tier = self.tome_tier.saturating_add(1),
            UnlockUpgradeKind::StartOrb => self.orb_tier = self.orb_tier.saturating_add(1),
        }
    }

    pub fn next_cost(&self, kind: UnlockUpgradeKind) -> u32 {
        let base = Self::base_cost(kind) as f32;
        let tier = self.tier(kind) as i32;
        let scaled = base * UNLOCK_COST_SCALE.powi(tier);
        scaled.round().max(1.) as u32
    }

    pub fn reroll_total(&self) -> u32 {
        3 + self.reroll_tier
    }

    pub fn banish_total(&self) -> u32 {
        self.banish_tier
    }

    pub fn food_count(&self) -> u32 {
        self.food_tier
    }

    pub fn tome_count(&self) -> u32 {
        self.tome_tier
    }

    pub fn orb_count(&self) -> u32 {
        self.orb_tier
    }
}

#[derive(Resource, Debug, Clone, Default)]
pub struct RunUnlockState {
    pub rerolls_total: u32,
    pub rerolls_remaining: u32,
    pub banishes_total: u32,
    pub banishes_remaining: u32,
    pub pending_food: u32,
    pub pending_tomes: u32,
    pub pending_orbs: u32,
    pub pending_rewards: bool,
}

impl RunUnlockState {
    pub fn reset_for_run(&mut self, upgrades: &UnlockUpgrades) {
        self.rerolls_total = upgrades.reroll_total();
        self.rerolls_remaining = self.rerolls_total;
        self.banishes_total = upgrades.banish_total();
        self.banishes_remaining = self.banishes_total;
        self.pending_food = upgrades.food_count();
        self.pending_tomes = upgrades.tome_count();
        self.pending_orbs = upgrades.orb_count();
        self.pending_rewards =
            self.pending_food > 0 || self.pending_tomes > 0 || self.pending_orbs > 0;
    }
}

pub fn persist_unlock_data(
    time_fragment_currency: Option<&TimeFragmentCurrency>,
    unlocked_classes: Option<&UnlockedClasses>,
    achievements: Option<&Achievements>,
    unlock_upgrades: Option<&UnlockUpgrades>,
) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        serde_json::from_reader::<_, crate::client::GameData>(reader).unwrap_or_default()
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

    match File::create(&path) {
        Ok(file) => {
            if let Err(err) = serde_json::to_writer(file, &game_data) {
                error!("Failed to write game_data.json: {err:?}");
            }
        }
        Err(err) => error!("Failed to create game_data.json: {err:?}"),
    }
}
