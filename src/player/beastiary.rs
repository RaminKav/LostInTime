use bevy::prelude::*;
use bevy::utils::HashMap;
use serde::{Deserialize, Serialize};

use crate::enemy::Mob;
use crate::item::WorldObject;
use crate::GameState;

/// One row in the bestiary, tracking lifetime totals for a single mob type.
#[derive(Default, Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct BeastiaryEntry {
    pub cards_collected: u32,
    pub number_killed: u32,
    pub damage_dealt: u32,
    pub damage_taken: u32,
    pub deaths_caused: u32,
}

/// Persistent bestiary stored on `GameData`. Lifetime totals across all runs.
#[derive(Resource, Default, Clone, Serialize, Deserialize, Debug)]
pub struct Beastiary {
    pub entries: HashMap<Mob, BeastiaryEntry>,
}

impl Beastiary {
    pub fn get(&self, mob: &Mob) -> BeastiaryEntry {
        self.entries.get(mob).cloned().unwrap_or_default()
    }
}

/// Run-scoped tracker reset on `OnExit(GameState::MainMenu)` (mirrors
/// [`crate::client::analytics::AnalyticsData`]). Folded into `GameData.beastiary`
/// at end of run in `handle_append_run_data_after_death`.
#[derive(Resource, Default, Clone, Debug)]
pub struct RunBeastiary {
    pub entries: HashMap<Mob, BeastiaryEntry>,
}

impl RunBeastiary {
    fn entry(&mut self, mob: Mob) -> &mut BeastiaryEntry {
        self.entries.entry(mob).or_default()
    }
    pub fn record_kill(&mut self, mob: Mob) {
        self.entry(mob).number_killed += 1;
    }
    pub fn record_damage_dealt(&mut self, mob: Mob, dmg: u32) {
        self.entry(mob).damage_dealt += dmg;
    }
    pub fn record_damage_taken(&mut self, mob: Mob, dmg: u32) {
        self.entry(mob).damage_taken += dmg;
    }
    pub fn record_death_caused(&mut self, mob: Mob) {
        self.entry(mob).deaths_caused += 1;
    }
}

/// Tracks the most recent mob that hit the player so we can attribute the death
/// to it when [`crate::client::GameOverEvent`] fires from `clamp_health`.
#[derive(Resource, Default, Clone, Debug)]
pub struct LastPlayerAttackerMob(pub Option<Mob>);

/// The 9 (card, mob) pairs the bestiary tracks, in display order for the 3x3 grid.
pub const BEASTIARY_MOBS: &[(WorldObject, Mob)] = &[
    (WorldObject::FurDevilCard, Mob::FurDevil),
    (WorldObject::BushlingCard, Mob::Bushling),
    (WorldObject::SpikeSlimeCard, Mob::SpikeSlime),
    (WorldObject::StingflyCard, Mob::StingFly),
    (WorldObject::RedMushlingCard, Mob::RedMushling),
    (WorldObject::LizardCard, Mob::Lizard),
    (WorldObject::SmallCactusCard, Mob::SmallCactus),
    (WorldObject::LargeCactusCard, Mob::BigCactus),
    (WorldObject::BullCard, Mob::Bull),
    (WorldObject::VoidCrawlerCard, Mob::VoidCrawler),
    (WorldObject::StoneGolemCard, Mob::StoneGolem),
];

pub fn mob_for_card(obj: WorldObject) -> Option<Mob> {
    BEASTIARY_MOBS
        .iter()
        .find(|(card, _)| *card == obj)
        .map(|(_, mob)| mob.clone())
}

pub fn card_for_mob(mob: &Mob) -> Option<WorldObject> {
    BEASTIARY_MOBS
        .iter()
        .find(|(_, m)| m == mob)
        .map(|(card, _)| *card)
}

/// Human-readable name for the bestiary detail panel. Adds spaces to PascalCase
/// variants so e.g. `Mob::StingFly` reads as "Sting Fly".
/// Resets the per-run bestiary tracker and last-attacker memo when leaving the
/// main menu (start of a new run). Mirrors `add_analytics_resource_on_start`.
pub fn reset_run_beastiary_on_run_start(mut commands: Commands) {
    commands.insert_resource(RunBeastiary::default());
    commands.insert_resource(LastPlayerAttackerMob::default());
}

pub struct BeastiaryPlugin;

impl Plugin for BeastiaryPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(reset_run_beastiary_on_run_start.in_schedule(OnExit(GameState::MainMenu)));
    }
}

pub fn mob_display_name(mob: &Mob) -> &'static str {
    match mob {
        Mob::FurDevil => "Fur Devil",
        Mob::Bushling => "Bushling",
        Mob::SpikeSlime => "Spike Slime",
        Mob::StingFly => "Sting Fly",
        Mob::RedMushling => "Red Mushling",
        Mob::SmallCactus => "Small Cactus",
        Mob::BigCactus => "Large Cactus",
        Mob::Bull => "Bull",
        Mob::StoneGolem => "Blake Boulder",
        Mob::Slime => "Slime",
        Mob::Hog => "Hog",
        Mob::Fairy => "Fairy",
        Mob::Crow => "Crow",
        Mob::RedMushking => "Red Mushking",
        Mob::Scorpion => "Desert Scorpion",
        Mob::Lizard => "Lizard",
        Mob::VoidCrawler => "Void Crawler",
        Mob::None => "Unknown",
    }
}
