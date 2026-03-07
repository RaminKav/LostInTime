use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum::IntoEnumIterator;

use crate::{attributes::ItemRarity, player::skills::SkillClass};

/// Tracks the rank and experience for a specific class
#[derive(Component, Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassRank {
    pub rank: u32,
    pub experience: u32,
    pub total_experience: u32, // Cumulative experience across all ranks
}

impl ClassRank {
    pub fn new() -> Self {
        Self {
            rank: 1,
            experience: 0,
            total_experience: 0,
        }
    }

    /// Add experience to this class rank
    pub fn add_experience(&mut self, exp: u32) -> bool {
        self.experience += exp;
        self.total_experience += exp;

        let old_rank = self.rank;
        self.update_rank();

        // Return true if rank increased
        old_rank < self.rank
    }

    /// Calculate rank based on total experience
    /// Every 5 ranks = 1 rarity tier upgrade
    /// Rank formula: rank = (total_experience / 1200) + 1 (~20% more XP per rank than before)
    fn update_rank(&mut self) {
        self.rank = (self.total_experience / 1200) + 1;
    }

    /// Get the rarity tier for this class rank
    /// Every 5 ranks = 1 rarity tier (0-4 = Common, 5-9 = Uncommon, 10-14 = Rare; capped at Rare for starting weapons)
    pub fn get_rarity_tier(&self) -> u32 {
        self.rank / 5
    }

    /// Get the ItemRarity for this class rank (capped at Rare for starting weapons)
    pub fn get_starting_weapon_rarity(&self) -> ItemRarity {
        match self.get_rarity_tier().min(2) {
            0 => ItemRarity::Common,
            1 => ItemRarity::Uncommon,
            _ => ItemRarity::Rare,
        }
    }
}

/// Resource that tracks all class ranks
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClassRankSystem {
    pub class_ranks: HashMap<SkillClass, ClassRank>,
}

impl ClassRankSystem {
    pub fn new() -> Self {
        let mut system = Self {
            class_ranks: HashMap::new(),
        };

        // Initialize all classes with rank 0
        for class in SkillClass::iter() {
            if class != SkillClass::None {
                system.class_ranks.insert(class, ClassRank::new());
            }
        }

        system
    }

    /// Get the rank for a specific class
    pub fn get_class_rank(&self, class: &SkillClass) -> ClassRank {
        self.class_ranks.get(class).cloned().unwrap_or_default()
    }

    /// Get mutable rank for a specific class
    pub fn get_class_rank_mut(&mut self, class: &SkillClass) -> &mut ClassRank {
        self.class_ranks
            .entry(class.clone())
            .or_insert_with(ClassRank::new)
    }

    /// Add experience to a specific class
    pub fn add_class_experience(&mut self, class: &SkillClass, exp: u32) -> bool {
        if class == &SkillClass::None {
            return false;
        }

        let class_rank = self.get_class_rank_mut(class);
        class_rank.add_experience(exp)
    }

    /// Get the starting weapon rarity for a class
    pub fn get_starting_weapon_rarity(&self, class: &SkillClass) -> ItemRarity {
        if class == &SkillClass::None {
            return ItemRarity::Common;
        }

        self.get_class_rank(class).get_starting_weapon_rarity()
    }
}
