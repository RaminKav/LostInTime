use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    chaos::ChaosTracker,
    combat::damage_tracker::MobStatTracker,
    item::WorldObject,
    night::InfiniteMode,
};

/// Tracks the current run's score and statistics
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunScore {
    pub score: u32,
    pub mobs_killed: u32,
    pub objs_destroyed: u32,
}

/// Tracks the total elapsed seconds the player has spent in the current run.
/// Ticks only while `GameState::Main` is active (i.e. excludes menus, pauses,
/// game-over screens) and is reset back to 0 whenever a new run begins.
#[derive(Resource, Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RunTimer {
    pub elapsed_seconds: f64,
}

impl RunScore {
    pub fn new() -> Self {
        Self {
            score: 0,
            mobs_killed: 0,
            objs_destroyed: 0,
        }
    }

    pub fn add_mob_kill(&mut self, chaos_level: u32) {
        self.mobs_killed += 1;
        self.score += 1 * chaos_level;
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

/// System to reset the run score when starting a new run
pub fn reset_run_score(mut run_score: ResMut<RunScore>) {
    info!("Resetting run score from {} to 0", run_score.score);
    *run_score = RunScore::new();
}

/// System to reset the run timer when starting a new run
pub fn reset_run_timer(mut run_timer: ResMut<RunTimer>) {
    *run_timer = RunTimer::default();
}

/// Tick the run timer while the player is actively in `GameState::Main`.
pub fn tick_run_timer(time: Res<Time>, mut run_timer: ResMut<RunTimer>) {
    run_timer.elapsed_seconds += time.delta_seconds_f64();
}

/// System to track mob kills and update score
pub fn track_mob_kills(
    mut run_score: ResMut<RunScore>,
    mut mob_stat_tracker: ResMut<MobStatTracker>,
    mut death_events: bevy::ecs::event::EventReader<crate::combat::EnemyDeathEvent>,
    chaos: Res<ChaosTracker>,
    infinite_mode: Res<InfiniteMode>,
) {
    for death in death_events.iter() {
        mob_stat_tracker.record_kill(death.mob.clone());
        // Include both global chaos and infinite mode chaos bonus for score
        let total_chaos = chaos.get_chaos() + infinite_mode.get_chaos_bonus();
        run_score.add_mob_kill(1 + total_chaos.trunc() as u32);
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
