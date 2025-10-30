use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::item::WorldObject;

/// Tracks the current run's score and statistics
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunScore {
    pub score: u32,
    pub mobs_killed: u32,
    pub objs_destroyed: u32,
}

impl RunScore {
    pub fn new() -> Self {
        Self {
            score: 0,
            mobs_killed: 0,
            objs_destroyed: 0,
        }
    }

    pub fn add_mob_kill(&mut self) {
        self.mobs_killed += 1;
        self.score += 1;
    }

    pub fn add_obj_destroyed(&mut self) {
        self.objs_destroyed += 1;
        self.score += 1;
    }
}

/// Tracks high scores across all runs
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct HighScores {
    pub overall_high_score: u32,
    pub class_high_scores: std::collections::HashMap<crate::player::skills::SkillClass, u32>,
}

impl HighScores {
    pub fn update_high_score(&mut self, score: u32, class: &crate::player::skills::SkillClass) {
        if score > self.overall_high_score {
            self.overall_high_score = score;
        }

        let class_score = self.class_high_scores.entry(class.clone()).or_insert(0);
        if score > *class_score {
            *class_score = score;
        }
    }
}

/// Component to mark starting weapons for rarity override
#[derive(Component, Debug)]
pub struct StartingWeapon {
    pub rarity: crate::attributes::ItemRarity,
}

/// System to track mob kills and update score
pub fn track_mob_kills(
    mut run_score: ResMut<RunScore>,
    mut death_events: bevy::ecs::event::EventReader<crate::combat::EnemyDeathEvent>,
) {
    for _death in death_events.iter() {
        run_score.add_mob_kill();
    }
}

/// System to track item destruction (boulders, stumps, etc.)
pub fn track_item_destruction(
    mut run_score: ResMut<RunScore>,
    mut break_events: bevy::ecs::event::EventReader<crate::ObjBreakEvent>,
) {
    for break_event in break_events.iter() {
        // Only count certain items as score-worthy
        match break_event.obj {
            WorldObject::Boulder
            | WorldObject::Boulder2
            | WorldObject::MetalBoulder
            | WorldObject::CoalBoulder
            | WorldObject::Stump
            | WorldObject::Stump2
            | WorldObject::Bush
            | WorldObject::Bush2
            | WorldObject::BerryBush
            | WorldObject::RedMushroom
            | WorldObject::BrownMushroom
            | WorldObject::Crate
            | WorldObject::Crate2
            | WorldObject::Pebble
            | WorldObject::LargeStump
            | WorldObject::LargeMushroomStump
            | WorldObject::SmallGreenTree
            | WorldObject::SmallYellowTree
            | WorldObject::MediumGreenTree
            | WorldObject::RedTree
            | WorldObject::MediumYellowTree => {
                run_score.add_obj_destroyed();
            }
            _ => {} // Other items don't give score
        }
    }
}
