//! Run difficulty ladder (menu-selected), separate from endless `InfiniteMode.difficulty_level`.
//!
//! Baseline nerfs always apply to normal (non-boss, non-endless) mobs. Ladder modifiers stack
//! on top once the player has unlocked the difficulty modal (first Era 3 win).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Highest selectable ladder tier (Difficulty Max).
pub const MAX_DIFFICULTY_TIER: u8 = 6;

/// Baseline multipliers applied to normal overworld/dungeon mobs (not bosses / endless).
pub const BASELINE_SPEED_MULT: f32 = 0.80;
pub const BASELINE_SPAWN_RATE_MULT: f32 = 0.75;
pub const BASELINE_ELITE_RATE_MULT: f32 = 0.75;

/// Ladder restore / buff multipliers (applied on top of baseline when unlocked).
pub const DIFF_SPAWN_RESTORE: f32 = 1.0 / BASELINE_SPAWN_RATE_MULT;
pub const DIFF_SPEED_RESTORE: f32 = 1.0 / BASELINE_SPEED_MULT;
pub const DIFF_DAMAGE_MULT: f32 = 1.20;
pub const DIFF_HP_MULT: f32 = 1.20;
pub const DIFF_SHOP_MULT: f32 = 1.15;
pub const DIFF_ELITE_RESTORE: f32 = 1.0 / BASELINE_ELITE_RATE_MULT;
pub const MEGA_ELITE_CHANCE: f32 = 0.10;
pub const MEGA_ELITE_SPEED_MULT: f32 = 1.50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum RunDifficulty {
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
    Five = 5,
    Max = 6,
}

impl RunDifficulty {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::One),
            2 => Some(Self::Two),
            3 => Some(Self::Three),
            4 => Some(Self::Four),
            5 => Some(Self::Five),
            6 => Some(Self::Max),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn prev(self) -> Option<Self> {
        Self::from_u8(self.as_u8().saturating_sub(1))
    }

    pub fn next(self) -> Option<Self> {
        Self::from_u8(self.as_u8() + 1)
    }

    /// Centered modifier lines active at this tier (cumulative).
    pub fn modifier_lines(self) -> &'static [&'static str] {
        match self {
            Self::One => &["Increased spawn rate"],
            Self::Two => &["Increased spawn rate", "Increased enemy speed"],
            Self::Three => &[
                "Increased spawn rate",
                "Increased enemy speed",
                "Enemies deal more damage",
            ],
            Self::Four => &[
                "Increased spawn rate",
                "Increased enemy speed",
                "Enemies deal more damage",
                "Enemies have more health",
            ],
            Self::Five => &[
                "Increased spawn rate",
                "Increased enemy speed",
                "Enemies deal more damage",
                "Enemies have more health",
                "Shop prices increased",
            ],
            Self::Max => &[
                "Increased spawn rate",
                "Increased enemy speed",
                "Enemies deal more damage",
                "Enemies have more health",
                "Shop prices increased",
                "Increased elite spawns",
                "Mega elites can appear",
            ],
        }
    }
}

/// Difficulty chosen for the active run. `selected: None` = pre-unlock / first clears (baseline only).
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ActiveRunDifficulty {
    pub selected: Option<RunDifficulty>,
}

impl ActiveRunDifficulty {
    pub fn new(selected: Option<RunDifficulty>) -> Self {
        Self { selected }
    }

    pub fn is_max(self) -> bool {
        self.selected == Some(RunDifficulty::Max)
    }

    pub fn tier(self) -> u8 {
        self.selected.map(RunDifficulty::as_u8).unwrap_or(0)
    }

    pub fn spawn_rate_multiplier(self) -> f32 {
        let mut m = BASELINE_SPAWN_RATE_MULT;
        if self.tier() >= RunDifficulty::One as u8 {
            m *= DIFF_SPAWN_RESTORE;
        }
        m
    }

    pub fn speed_multiplier(self) -> f32 {
        let mut m = BASELINE_SPEED_MULT;
        if self.tier() >= RunDifficulty::Two as u8 {
            m *= DIFF_SPEED_RESTORE;
        }
        m
    }

    pub fn damage_multiplier(self) -> f32 {
        if self.tier() >= RunDifficulty::Three as u8 {
            DIFF_DAMAGE_MULT
        } else {
            1.0
        }
    }

    pub fn hp_multiplier(self) -> f32 {
        if self.tier() >= RunDifficulty::Four as u8 {
            DIFF_HP_MULT
        } else {
            1.0
        }
    }

    pub fn shop_price_multiplier(self) -> f32 {
        if self.tier() >= RunDifficulty::Five as u8 {
            DIFF_SHOP_MULT
        } else {
            1.0
        }
    }

    pub fn elite_rate_multiplier(self) -> f32 {
        let mut m = BASELINE_ELITE_RATE_MULT;
        if self.tier() >= RunDifficulty::Max as u8 {
            m *= DIFF_ELITE_RESTORE;
        }
        m
    }

    pub fn allows_mega_elites(self) -> bool {
        self.selected == Some(RunDifficulty::Max)
    }
}

/// Menu picker state (clamped to unlocked tiers).
#[derive(Resource, Debug, Clone, Copy)]
pub struct DifficultySelectState {
    pub selected: RunDifficulty,
    pub max_unlocked: u8,
}

impl Default for DifficultySelectState {
    fn default() -> Self {
        Self {
            selected: RunDifficulty::One,
            max_unlocked: 0,
        }
    }
}

impl DifficultySelectState {
    pub fn can_go_left(self) -> bool {
        self.selected.as_u8() > 1
    }

    pub fn can_go_right(self) -> bool {
        self.selected.as_u8() < self.max_unlocked.max(1)
    }

    pub fn step_left(&mut self) {
        if let Some(prev) = self.selected.prev() {
            self.selected = prev;
        }
    }

    pub fn step_right(&mut self) {
        if let Some(next) = self.selected.next() {
            if next.as_u8() <= self.max_unlocked {
                self.selected = next;
            }
        }
    }
}

pub fn difficulty_options_unlocked(max_unlocked_difficulty: u8) -> bool {
    max_unlocked_difficulty >= 1
}

/// Run condition: online leaderboard submits only Max-difficulty runs.
pub fn run_is_max_difficulty(difficulty: Res<ActiveRunDifficulty>) -> bool {
    difficulty.is_max()
}

/// After an Era 3 boss kill, unlock the next ladder tier when appropriate.
pub fn unlock_difficulty_after_era3_win(max_unlocked: u8, run_tier: u8) -> Option<u8> {
    if max_unlocked == 0 {
        // First clear (baseline run) unlocks Difficulty 1 + the modal.
        return Some(1);
    }
    if run_tier >= max_unlocked && max_unlocked < MAX_DIFFICULTY_TIER {
        return Some(max_unlocked + 1);
    }
    None
}

/// Persist next ladder tier when the Era 3 boss is killed on a new highest difficulty.
/// Also records highest difficulty cleared for the active class.
pub fn unlock_difficulty_on_era3_boss_kill(
    boss_kill_tracker: Option<Res<crate::world::portal::BossKillTracker>>,
    mut game_data: Option<ResMut<crate::client::GameData>>,
    mut high_scores: Option<ResMut<crate::player::score::HighScores>>,
    player_class: Option<Res<crate::player::skills::PlayerClass>>,
    difficulty: Res<ActiveRunDifficulty>,
) {
    let Some(tracker) = boss_kill_tracker else {
        return;
    };
    if !tracker.is_boss_killed(&crate::world::dimension::Era::Third) {
        return;
    }
    let Some(ref mut data) = game_data else {
        return;
    };

    let run_tier = difficulty.tier();
    let mut changed = false;
    if let Some(pc) = player_class.as_deref() {
        if pc.class != crate::player::skills::SkillClass::None {
            if data.high_scores.record_era3_clear(&pc.class, run_tier) {
                changed = true;
                if let Some(ref mut scores) = high_scores {
                    scores.record_era3_clear(&pc.class, run_tier);
                }
            }
        }
    }

    if let Some(new_max) = unlock_difficulty_after_era3_win(data.max_unlocked_difficulty, run_tier)
    {
        info!(
            "Unlocked difficulty tier {} (was {}, run tier {})",
            new_max, data.max_unlocked_difficulty, run_tier
        );
        data.max_unlocked_difficulty = new_max;
        if data.last_selected_difficulty == 0 {
            data.last_selected_difficulty = 1;
        }
        changed = true;
    }
    if changed {
        persist_difficulty_progress(data);
    }
}

pub fn persist_difficulty_progress(game_data: &crate::client::GameData) {
    use std::fs::{File, OpenOptions};
    use std::io::{BufReader, BufWriter};

    let path = crate::datafiles::game_data();
    let mut on_disk = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        crate::client::GameData::default()
    };
    on_disk.max_unlocked_difficulty = game_data.max_unlocked_difficulty;
    on_disk.last_selected_difficulty = game_data.last_selected_difficulty;
    on_disk.high_scores = game_data.high_scores.clone();
    if let Ok(file) = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
    {
        let writer = BufWriter::new(file);
        if let Err(err) = serde_json::to_writer_pretty(writer, &on_disk) {
            error!("Failed to persist difficulty progress: {err:?}");
        }
    }
}

pub struct DifficultyPlugin;

impl Plugin for DifficultyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveRunDifficulty>()
            .init_resource::<DifficultySelectState>()
            .add_systems(
                Update,
                unlock_difficulty_on_era3_boss_kill.run_if(in_state(crate::GameState::Main)),
            )
            .add_systems(
                OnEnter(crate::GameState::MainMenu),
                reset_active_run_difficulty,
            );
    }
}

fn reset_active_run_difficulty(mut difficulty: ResMut<ActiveRunDifficulty>) {
    *difficulty = ActiveRunDifficulty::default();
}
