use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Number of crystals tracked across all runs.
pub const TIME_CRYSTAL_COUNT: usize = 12;
/// Number of shards required to complete each crystal.
pub const SHARDS_PER_CRYSTAL: u32 = 6;
/// Run must last at least this many seconds for the survival shard.
pub const SURVIVAL_SHARD_THRESHOLD_SECONDS: f64 = 6.0 * 60.0;

/// Each threshold grants +1 shard when [`crate::player::score::RunScore::score`] reaches it
/// (checked at game over; all that apply stack).
pub const SCORE_SHARD_THRESHOLDS: &[u32] = &[2000, 8000, 16_000, 100_000];

/// A single time crystal, completed once enough shards are collected.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeCrystal {
    pub shards: u32,
    pub required_shards: u32,
}

impl TimeCrystal {
    pub fn new(required_shards: u32) -> Self {
        Self {
            shards: 0,
            required_shards,
        }
    }

    /// Returns a fully completed crystal (used for tooling/exports).
    pub fn complete() -> Self {
        Self {
            shards: SHARDS_PER_CRYSTAL,
            required_shards: SHARDS_PER_CRYSTAL,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.shards >= self.required_shards
    }

    /// Adds shards up to the required amount. Returns the leftover (overflow) shards
    /// that didn't fit into this crystal so they can be passed on to the next one.
    pub fn add_shards(&mut self, count: u32) -> u32 {
        if self.is_complete() {
            return count;
        }
        let space = self.required_shards.saturating_sub(self.shards);
        let to_add = count.min(space);
        self.shards = self.shards.saturating_add(to_add);
        count.saturating_sub(to_add)
    }
}

/// Persistent meta-progression resource: which time crystals the player has completed
/// and how many shards they've collected toward the next one.
///
/// Crystals fill in order; shards always go into the lowest-index incomplete crystal.
#[derive(Resource, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimeCrystals {
    pub crystals: Vec<TimeCrystal>,
}

impl Default for TimeCrystals {
    fn default() -> Self {
        Self {
            crystals: (0..TIME_CRYSTAL_COUNT)
                .map(|_| TimeCrystal::new(SHARDS_PER_CRYSTAL))
                .collect(),
        }
    }
}

impl TimeCrystals {
    /// Returns a TimeCrystals where every crystal is complete. Used by the heirloom
    /// card export tool to dump all heirlooms regardless of unlock state.
    pub fn all_complete() -> Self {
        Self {
            crystals: (0..TIME_CRYSTAL_COUNT)
                .map(|_| TimeCrystal::complete())
                .collect(),
        }
    }

    pub fn is_complete(&self, idx: usize) -> bool {
        self.crystals
            .get(idx)
            .map(|c| c.is_complete())
            .unwrap_or(false)
    }

    /// Adds shards into the next incomplete crystal(s) in order. Overflow rolls over
    /// into subsequent crystals; surplus past the final crystal is dropped.
    pub fn add_shards(&mut self, mut count: u32) {
        for crystal in self.crystals.iter_mut() {
            if count == 0 {
                break;
            }
            count = crystal.add_shards(count);
        }
    }

    pub fn completed_count(&self) -> usize {
        self.crystals.iter().filter(|c| c.is_complete()).count()
    }

    /// Index of the next crystal that's still incomplete (the one currently being filled).
    /// Returns `None` if every crystal is complete.
    pub fn current_focus_idx(&self) -> Option<usize> {
        self.crystals.iter().position(|c| !c.is_complete())
    }
}

/// Snapshot of a player's [`TimeCrystals`] before and after the last run, plus how
/// many shards were awarded. Inserted on game-over so the post-run popup can show
/// what the player gained without reaching back through saved data.
///
/// Consumed (removed) when the player closes the post-run progress popup.
#[derive(Resource, Clone, Debug)]
pub struct LastRunCrystalProgress {
    /// Snapshot of the crystals BEFORE this run's shards were applied.
    pub before: TimeCrystals,
    /// Snapshot of the crystals AFTER this run's shards were applied.
    pub after: TimeCrystals,
    /// Total shards awarded for the run.
    pub shards_earned: u32,
}

impl LastRunCrystalProgress {
    /// Indices of crystals that went from incomplete to complete during this run.
    pub fn newly_completed_indices(&self) -> Vec<usize> {
        (0..self.after.crystals.len())
            .filter(|i| !self.before.is_complete(*i) && self.after.is_complete(*i))
            .collect()
    }
}
