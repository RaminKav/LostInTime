use bevy::{prelude::*, reflect::TypeUuid};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::achievements::Achievement;
use crate::player::skills::SkillClass;

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnlockCurrency {
    pub amount: u32,
}

impl UnlockCurrency {
    pub fn add(&mut self, value: u32) {
        self.amount = self.amount.saturating_add(value);
    }

    pub fn can_spend(&self, value: u32) -> bool {
        self.amount >= value
    }

    pub fn spend(&mut self, value: u32) -> bool {
        if self.can_spend(value) {
            self.amount -= value;
            true
        } else {
            false
        }
    }
}

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
