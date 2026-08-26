use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    chaos::ChaosTracker, combat::damage_tracker::MobStatTracker, item::WorldObject,
    night::InfiniteMode,
};

/// Tracks the current run's score and statistics
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunScore {
    pub score: u32,
    pub mobs_killed: u32,
    pub objs_destroyed: u32,
    /// Snapshot of dev mode at run start; dev runs must not update saved high scores.
    #[serde(default)]
    pub dev_mode: bool,
}

/// Tracks the total elapsed seconds the player has spent in the current run.
/// Ticks only while `GameState::Main` is active (i.e. excludes menus, pauses,
/// game-over screens) and is reset back to 0 whenever a new run begins.
#[derive(Resource, Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct RunTimer {
    pub elapsed_seconds: f64,
}

impl RunScore {
    pub fn new(dev_mode: bool) -> Self {
        Self {
            score: 0,
            mobs_killed: 0,
            objs_destroyed: 0,
            dev_mode,
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

/// Tracks high scores and highest difficulty cleared across all runs
#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct HighScores {
    pub overall_high_score: u32,
    pub class_high_scores: std::collections::HashMap<crate::player::skills::SkillClass, u32>,
    /// Highest run difficulty tier cleared with each class (Era 3 win).
    /// Missing key = never cleared Era 3 with that class. Present with `0` = baseline clear.
    #[serde(default)]
    pub class_highest_difficulty: std::collections::HashMap<crate::player::skills::SkillClass, u8>,
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

    pub fn update_highest_difficulty(
        &mut self,
        class: &crate::player::skills::SkillClass,
        tier: u8,
    ) {
        let entry = self
            .class_highest_difficulty
            .entry(class.clone())
            .or_insert(0);
        if tier > *entry {
            *entry = tier;
        }
    }

    pub fn highest_difficulty(&self, class: &crate::player::skills::SkillClass) -> u8 {
        self.class_highest_difficulty
            .get(class)
            .copied()
            .unwrap_or(0)
    }

    /// True if this class has ever cleared Era 3 (including baseline difficulty).
    pub fn has_cleared_era3(&self, class: &crate::player::skills::SkillClass) -> bool {
        self.class_highest_difficulty.contains_key(class)
    }

    /// Record an Era 3 clear. Returns true if this is a new class clear or a higher tier.
    pub fn record_era3_clear(
        &mut self,
        class: &crate::player::skills::SkillClass,
        tier: u8,
    ) -> bool {
        let newly_cleared = !self.has_cleared_era3(class);
        let before = self.highest_difficulty(class);
        self.update_highest_difficulty(class, tier);
        newly_cleared || self.highest_difficulty(class) > before
    }
}

/// Component to mark starting weapons for rarity override
#[derive(Component, Debug)]
pub struct StartingWeapon {
    pub rarity: crate::attributes::ItemRarity,
}

/// System to reset the run score when starting a new run
pub fn reset_run_score(
    mut run_score: ResMut<RunScore>,
    cheat_settings: Res<crate::ui::CheatSettings>,
) {
    info!("Resetting run score from {} to 0", run_score.score);
    *run_score = RunScore::new(cheat_settings.dev_mode);
}

/// System to reset the run timer when starting a new run
pub fn reset_run_timer(mut run_timer: ResMut<RunTimer>) {
    *run_timer = RunTimer::default();
}

/// Tick the run timer while the player is actively in `GameState::Main`.
pub fn tick_run_timer(time: Res<Time>, mut run_timer: ResMut<RunTimer>) {
    run_timer.elapsed_seconds += time.delta_secs_f64();
}

/// System to track mob kills and update score
pub fn track_mob_kills(
    mut run_score: ResMut<RunScore>,
    mut mob_stat_tracker: ResMut<MobStatTracker>,
    mut death_events: bevy::ecs::message::MessageReader<crate::combat::EnemyDeathEvent>,
    chaos: Res<ChaosTracker>,
    infinite_mode: Res<InfiniteMode>,
) {
    for death in death_events.read() {
        mob_stat_tracker.record_kill(death.mob.clone());
        // Include both global chaos and infinite mode chaos bonus for score
        let total_chaos = chaos.get_chaos() + infinite_mode.get_chaos_bonus();
        run_score.add_mob_kill(1 + total_chaos.trunc() as u32);
    }
}

/// System to track item destruction (boulders, stumps, etc.)
pub fn track_item_destruction(
    mut run_score: ResMut<RunScore>,
    mut break_events: bevy::ecs::message::MessageReader<crate::ObjBreakEvent>,
) {
    for break_event in break_events.read() {
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
