use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};
use strum_macros::{Display, EnumIter};

use crate::{
    chaos::ChaosTracker,
    client::{analytics::AnalyticsData, handle_append_run_data_after_death, GameData},
    datafiles,
    enemy::Mob,
    item::WorldObject,
    player::TimeFragmentCurrency,
    world::dimension::Era,
    world::portal::BossKillTracker,
    BounceEvent, GameState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumIter)]
pub enum Achievement {
    FirstRunComplete,
    Kill100FurDevils,
    SlimePet,
    FairyPet,
    PorkipinePet,
    GoldenPigPet,
    Bouncy,
    Bouncy2,
    Act1,
    Act2,
    Act3,
    BushlingSlayer1,
    StingflySlayer,
    MushlingSlayer,
    Chaotic,
    DungeonCrawler,
    FindHammer,
    FindSpear,
    FindClaw,
    FindGun,
    FindIceStaff,
    FindBasicStaff,
    FindMagicWhip,
    FindDagger,
    FindBow,
    FindBlowdart,
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct Achievements {
    pub unlocked: Vec<Achievement>, // Kept for backwards compatibility, maps to claimed
    pub completed: Vec<Achievement>, // Completed but not yet claimed
    pub claimed: Vec<Achievement>,  // Completed and claimed
}

impl Achievements {
    pub fn new() -> Self {
        Self {
            unlocked: Vec::new(),
            completed: Vec::new(),
            claimed: Vec::new(),
        }
    }
}

#[derive(Resource, Default, Debug, Clone, Serialize, Deserialize)]
pub struct BounceAchievementTracker {
    pub total_pink_bounces: u32,
    #[serde(skip)] // Don't persist this as it's only relevant within a single run
    pub consecutive_pink_bounces: u32,
    #[serde(skip)] // Don't persist this as it's only relevant within a single run
    pub last_bounce_time: Option<f64>,
}
impl Achievement {
    pub fn get_name(&self) -> String {
        match self {
            Achievement::FirstRunComplete => "That was weird".to_string(),
            Achievement::Kill100FurDevils => "Fur Devil Slayer".to_string(),
            Achievement::SlimePet => "Slimed".to_string(),
            Achievement::FairyPet => "Fairy Friend".to_string(),
            Achievement::PorkipinePet => "Porkipine".to_string(),
            Achievement::GoldenPigPet => "Golden Pig".to_string(),
            Achievement::Bouncy => "Bouncy".to_string(),
            Achievement::Bouncy2 => "Bouncy II".to_string(),
            Achievement::Act1 => "Act I".to_string(),
            Achievement::Act2 => "Act II".to_string(),
            Achievement::Act3 => "Act III".to_string(),
            Achievement::BushlingSlayer1 => "Bushling Slayer".to_string(),
            Achievement::StingflySlayer => "Stingfly Slayer".to_string(),
            Achievement::MushlingSlayer => "Mushling Slayer".to_string(),
            Achievement::Chaotic => "Chaotic".to_string(),
            Achievement::DungeonCrawler => "Dungeon Crawler".to_string(),
            Achievement::FindSpear => "Find Spear".to_string(),
            Achievement::FindHammer => "Find Hammer".to_string(),
            Achievement::FindClaw => "Find Claw".to_string(),
            Achievement::FindGun => "Find Gun".to_string(),
            Achievement::FindIceStaff => "Find Ice Staff".to_string(),
            Achievement::FindBasicStaff => "Find Basic Staff".to_string(),
            Achievement::FindMagicWhip => "Find Magic Whip".to_string(),
            Achievement::FindDagger => "Find Dagger".to_string(),
            Achievement::FindBow => "Find Bow".to_string(),
            Achievement::FindBlowdart => "Find Blowdart".to_string(),
        }
    }
    pub fn get_desc(&self) -> String {
        match self {
            Achievement::FirstRunComplete => "Complete your first run.".to_string(),
            Achievement::Kill100FurDevils => "Defeat 1000 Fur Devils.".to_string(),
            Achievement::SlimePet => "Find the Slime in Act 1.".to_string(),
            Achievement::FairyPet => "Find the Fairy in Act 2.".to_string(),
            Achievement::PorkipinePet => "Find the Porkipine.".to_string(),
            Achievement::GoldenPigPet => "Find the Golden Pig.".to_string(),
            Achievement::Bouncy => "Bounce on 100 pink petals.".to_string(),
            Achievement::Bouncy2 => "Chain three pink petal bounces.".to_string(),
            Achievement::Act1 => "Defeat the Act 1 boss.".to_string(),
            Achievement::Act2 => "Defeat the Act 2 boss.".to_string(),
            Achievement::Act3 => "Defeat the Act 3 boss.".to_string(),
            Achievement::BushlingSlayer1 => "Eliminate 1000 Bushlings.".to_string(),
            Achievement::StingflySlayer => "Eliminate 1000 Stingflies.".to_string(),
            Achievement::MushlingSlayer => "Eliminate 1000 Red Mushlings.".to_string(),
            Achievement::Chaotic => "Reach 20 total Chaos.".to_string(),
            Achievement::DungeonCrawler => "Find the key & clear the dungeon.".to_string(),
            Achievement::FindSpear => "Find a Spear.".to_string(),
            Achievement::FindHammer => "Find a Hammer.".to_string(),
            Achievement::FindClaw => "Find a Claw.".to_string(),
            Achievement::FindGun => "Find a Gun.".to_string(),
            Achievement::FindIceStaff => "Find an Ice Staff.".to_string(),
            Achievement::FindBasicStaff => "Find a Basic Staff.".to_string(),
            Achievement::FindMagicWhip => "Find a Magic Whip.".to_string(),
            Achievement::FindDagger => "Find a Dagger.".to_string(),
            Achievement::FindBow => "Find a Bow.".to_string(),
            Achievement::FindBlowdart => "Find a Blowdart.".to_string(),
        }
    }

    pub fn reward_currency(&self) -> u32 {
        match self {
            Achievement::DungeonCrawler => 5,
            Achievement::FirstRunComplete => 2,
            Achievement::Kill100FurDevils => 5,
            Achievement::SlimePet => 5,
            Achievement::FairyPet => 7,
            Achievement::PorkipinePet => 5,
            Achievement::GoldenPigPet => 5,
            Achievement::Bouncy => 5,
            Achievement::Bouncy2 => 3,
            Achievement::Act1 => 10,
            Achievement::Act2 => 15,
            Achievement::Act3 => 20,
            Achievement::BushlingSlayer1 => 7,
            Achievement::StingflySlayer => 10,
            Achievement::MushlingSlayer => 10,
            Achievement::Chaotic => 25,
            Achievement::FindSpear
            | Achievement::FindClaw
            | Achievement::FindHammer
            | Achievement::FindGun
            | Achievement::FindIceStaff
            | Achievement::FindBasicStaff
            | Achievement::FindMagicWhip
            | Achievement::FindDagger
            | Achievement::FindBow
            | Achievement::FindBlowdart => 3,
            _ => 0,
        }
    }

    /// Get progress for this achievement (current, target)
    /// Returns None if this achievement doesn't have progress tracking
    /// Combines cumulative analytics (from GameData) with current run analytics
    pub fn get_progress(
        &self,
        cumulative_analytics: Option<&crate::client::analytics::AnalyticsData>,
        current_run_analytics: Option<&crate::client::analytics::AnalyticsData>,
        bounce_tracker: Option<&BounceAchievementTracker>,
    ) -> Option<(u32, u32)> {
        // Helper to get combined mob kills from both cumulative and current run
        let get_mob_kills = |mob: &Mob| -> u32 {
            let cumulative_kills = cumulative_analytics
                .and_then(|c| c.mobs_killed.get(mob))
                .copied()
                .unwrap_or(0);
            let current_kills = current_run_analytics
                .and_then(|c| c.mobs_killed.get(mob))
                .copied()
                .unwrap_or(0);
            // Add current run kills to cumulative total
            cumulative_kills + current_kills
        };

        match self {
            // Achievements that need analytics
            Achievement::Kill100FurDevils => Some((get_mob_kills(&Mob::FurDevil), 1000)),
            Achievement::BushlingSlayer1 => Some((get_mob_kills(&Mob::Bushling), 1000)),
            Achievement::StingflySlayer => Some((get_mob_kills(&Mob::StingFly), 1000)),
            Achievement::MushlingSlayer => Some((get_mob_kills(&Mob::RedMushling), 1000)),
            // Achievements that don't need analytics
            Achievement::Bouncy => {
                if let Some(bounce) = bounce_tracker {
                    Some((bounce.total_pink_bounces, 100))
                } else {
                    Some((0, 100))
                }
            }
            Achievement::Bouncy2 => {
                if let Some(bounce) = bounce_tracker {
                    Some((bounce.consecutive_pink_bounces, 3))
                } else {
                    Some((0, 3))
                }
            }
            _ => None, // No progress tracking for other achievements
        }
    }
}

impl Achievements {
    pub fn has(&self, achievement: Achievement) -> bool {
        // Check if claimed (backwards compatibility)
        self.claimed.contains(&achievement) || self.unlocked.contains(&achievement)
    }

    pub fn is_completed(&self, achievement: Achievement) -> bool {
        self.completed.contains(&achievement)
    }

    pub fn is_claimed(&self, achievement: Achievement) -> bool {
        self.claimed.contains(&achievement) || self.unlocked.contains(&achievement)
    }

    pub fn has_unclaimed_completed(&self) -> bool {
        !self.completed.is_empty()
    }

    pub fn complete(&mut self, achievement: Achievement) -> bool {
        // Only complete if not already completed or claimed
        if !self.is_completed(achievement) && !self.is_claimed(achievement) {
            self.completed.push(achievement);
            return true;
        }
        false
    }

    pub fn claim(&mut self, achievement: Achievement) -> bool {
        // Move from completed to claimed
        if let Some(pos) = self.completed.iter().position(|&a| a == achievement) {
            self.completed.remove(pos);
            if !self.claimed.contains(&achievement) {
                self.claimed.push(achievement);
            }
            return true;
        }
        false
    }

    // Legacy method for backwards compatibility
    pub fn unlock(&mut self, achievement: Achievement) -> bool {
        if !self.has(achievement) {
            self.unlocked.push(achievement);
            if !self.claimed.contains(&achievement) {
                self.claimed.push(achievement);
            }
            return true;
        }
        false
    }
}

pub fn persist_achievements_state(achievements: &Achievements) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        GameData::default()
    };

    game_data.achievements = achievements.clone();

    match File::create(&path) {
        Ok(file) => {
            if let Err(err) = serde_json::to_writer(file, &game_data) {
                error!("Failed to persist achievements to game_data.json: {err:?}");
            }
        }
        Err(err) => error!("Failed to create game_data.json while saving achievements: {err:?}"),
    }
}

fn try_unlock(
    achievements: &mut Achievements,
    achievement: Achievement,
    achievement_events: &mut EventWriter<AchievementUnlockedEvent>,
) -> bool {
    if achievements.complete(achievement) {
        persist_achievements_state(achievements);
        achievement_events.send(AchievementUnlockedEvent {
            achievement,
            reward_currency: achievement.reward_currency(),
        });
        info!("Achievement completed: {:?}", achievement);
        true
    } else {
        false
    }
}

pub struct AchievementsPlugin;

impl Plugin for AchievementsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AchievementUnlockedEvent>()
            .init_resource::<BounceAchievementTracker>()
            .add_systems(
                (
                    track_bounce_achievements,
                    check_achievements,
                    check_first_run_achievement.before(handle_append_run_data_after_death),
                    handle_achievement_rewards,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

/// System to check and award achievements based on game events
pub fn check_achievements(
    mut achievements: ResMut<Achievements>,
    analytics: Option<Res<AnalyticsData>>,
    chaos_tracker: Option<Res<ChaosTracker>>,
    boss_kill_tracker: Option<Res<BossKillTracker>>,
    mut achievement_events: EventWriter<AchievementUnlockedEvent>,
    game_data: Option<Res<crate::client::GameData>>,
) {
    // Get both cumulative and current run analytics
    let cumulative = game_data
        .as_ref()
        .and_then(|gd| gd.cumulative_analytics.as_ref());
    let current_run = analytics.as_ref().map(|a| a.as_ref());

    // Helper to get combined count from both cumulative and current run
    // Cumulative has totals from previous runs, current_run has this run's stats
    // We ADD them together to get the true total
    let get_mob_kills = |mob: &Mob| -> u32 {
        let cumulative_kills = cumulative
            .and_then(|c| c.mobs_killed.get(mob))
            .copied()
            .unwrap_or(0);
        let current_kills = current_run
            .and_then(|c| c.mobs_killed.get(mob))
            .copied()
            .unwrap_or(0);
        // Add current run kills to cumulative total
        cumulative_kills + current_kills
    };

    let get_item_collected = |object: &WorldObject| -> u32 {
        let cumulative_count = cumulative
            .and_then(|c| c.items_collected.get(object))
            .copied()
            .unwrap_or(0);
        let current_count = current_run
            .and_then(|c| c.items_collected.get(object))
            .copied()
            .unwrap_or(0);
        // Add current run items to cumulative total
        cumulative_count + current_count
    };

    // Check mob kill achievements
    let mut check_mob_kill = |mob: Mob, threshold: u32, achievement: Achievement| {
        if get_mob_kills(&mob) >= threshold {
            try_unlock(&mut achievements, achievement, &mut achievement_events);
        }
    };

    check_mob_kill(Mob::FurDevil, 1000, Achievement::Kill100FurDevils);
    check_mob_kill(Mob::Bushling, 1000, Achievement::BushlingSlayer1);
    check_mob_kill(Mob::StingFly, 1000, Achievement::StingflySlayer);
    check_mob_kill(Mob::RedMushling, 1000, Achievement::MushlingSlayer);

    // Check item collected achievements
    let mut check_item_collected = |object: WorldObject, achievement: Achievement| {
        if get_item_collected(&object) > 0 {
            try_unlock(&mut achievements, achievement, &mut achievement_events);
        }
    };

    check_item_collected(WorldObject::Spear, Achievement::FindSpear);
    check_item_collected(WorldObject::Claw, Achievement::FindClaw);
    check_item_collected(WorldObject::Gun, Achievement::FindGun);
    check_item_collected(WorldObject::IceStaff, Achievement::FindIceStaff);
    check_item_collected(WorldObject::BasicStaff, Achievement::FindBasicStaff);
    check_item_collected(WorldObject::MagicWhip, Achievement::FindMagicWhip);
    check_item_collected(WorldObject::Dagger, Achievement::FindDagger);
    check_item_collected(WorldObject::Hammer, Achievement::FindHammer);
    check_item_collected(WorldObject::WoodBow, Achievement::FindBow);
    check_item_collected(WorldObject::Blowdart, Achievement::FindBlowdart);

    // Check dungeon crawler achievement
    let found_key = get_item_collected(&WorldObject::Key) > 0;
    if found_key {
        if let Some(boss_kills) = boss_kill_tracker.as_ref() {
            if boss_kills.is_boss_killed(&Era::DungeonMain) {
                try_unlock(
                    &mut achievements,
                    Achievement::DungeonCrawler,
                    &mut achievement_events,
                );
            }
        }
    }

    if let Some(chaos) = chaos_tracker.as_ref() {
        if chaos.get_chaos() >= 20.0 {
            try_unlock(
                &mut achievements,
                Achievement::Chaotic,
                &mut achievement_events,
            );
        }
    }

    if let Some(boss_kills) = boss_kill_tracker.as_ref() {
        if boss_kills.is_boss_killed(&Era::Main) {
            try_unlock(
                &mut achievements,
                Achievement::Act1,
                &mut achievement_events,
            );
        }
        if boss_kills.is_boss_killed(&Era::Second) {
            try_unlock(
                &mut achievements,
                Achievement::Act2,
                &mut achievement_events,
            );
        }
        if boss_kills.is_boss_killed(&Era::Third) {
            try_unlock(
                &mut achievements,
                Achievement::Act3,
                &mut achievement_events,
            );
        }
    }
}

/// System to award FirstRunComplete achievement when player dies for the first time
pub fn check_first_run_achievement(
    mut achievements: ResMut<Achievements>,
    mut game_over_events: EventReader<crate::client::GameOverEvent>,
    mut achievement_events: EventWriter<AchievementUnlockedEvent>,
) {
    for _ in game_over_events.iter() {
        try_unlock(
            &mut achievements,
            Achievement::FirstRunComplete,
            &mut achievement_events,
        );
    }
}

pub fn track_bounce_achievements(
    mut bounce_events: EventReader<BounceEvent>,
    time: Res<Time>,
    mut tracker: ResMut<BounceAchievementTracker>,
    mut achievements: ResMut<Achievements>,
    mut achievement_events: EventWriter<AchievementUnlockedEvent>,
) {
    const MIN_BOUNCE_INTERVAL_SECS: f64 = 0.2;
    const CONSECUTIVE_INTERVAL_SECS: f64 = 0.38;

    let mut unlocked_bouncy = false;
    let mut unlocked_bouncy2 = false;

    for _event in bounce_events.iter() {
        let now = time.elapsed_seconds_f64();

        if let Some(last) = tracker.last_bounce_time {
            let delta = now - last;
            if delta > CONSECUTIVE_INTERVAL_SECS {
                tracker.last_bounce_time = None;
                tracker.consecutive_pink_bounces = 0;
            }
        }

        let should_count = tracker
            .last_bounce_time
            .map_or(true, |last| now - last >= MIN_BOUNCE_INTERVAL_SECS);
        if !should_count {
            continue;
        }

        if let Some(last) = tracker.last_bounce_time {
            let delta = now - last;
            info!("Time since last bounce: {}", delta);
        }

        tracker.total_pink_bounces = tracker.total_pink_bounces.saturating_add(1);
        tracker.consecutive_pink_bounces = tracker.consecutive_pink_bounces.saturating_add(1);
        tracker.last_bounce_time = Some(now);

        if !unlocked_bouncy && tracker.total_pink_bounces >= 100 {
            unlocked_bouncy = try_unlock(
                &mut achievements,
                Achievement::Bouncy,
                &mut achievement_events,
            );
        }

        if !unlocked_bouncy2 && tracker.consecutive_pink_bounces >= 3 {
            unlocked_bouncy2 = try_unlock(
                &mut achievements,
                Achievement::Bouncy2,
                &mut achievement_events,
            );
        }
    }
}

// Removed - rewards are now claimed when player clicks on achievement row
// This function is kept for backwards compatibility but does nothing
pub fn handle_achievement_rewards(
    mut _commands: Commands,
    mut _events: EventReader<AchievementUnlockedEvent>,
    _currency: Option<ResMut<TimeFragmentCurrency>>,
) {
    // Rewards are now claimed when player clicks on achievement row in UI
}

pub struct AchievementUnlockedEvent {
    pub achievement: Achievement,
    pub reward_currency: u32,
}

/// Maps achievements to the classes they unlock
impl Achievement {
    pub fn unlocks_class(&self) -> Option<crate::player::skills::SkillClass> {
        match self {
            Achievement::Kill100FurDevils => Some(crate::player::skills::SkillClass::Thief),
            // Add more mappings as needed
            _ => None,
        }
    }

    /// Maps achievements to the pets they unlock
    pub fn unlocks_pet(&self) -> Option<crate::pets::state::Pet> {
        match self {
            Achievement::SlimePet => Some(crate::pets::state::Pet::Slime),
            Achievement::FairyPet => Some(crate::pets::state::Pet::Fairy),
            Achievement::PorkipinePet => Some(crate::pets::state::Pet::Porkipine),
            Achievement::GoldenPigPet => Some(crate::pets::state::Pet::GoldenPig),
            _ => None,
        }
    }
}

/// Returns which classes are unlocked by default (first two classes)
pub fn get_default_unlocked_classes() -> Vec<crate::player::skills::SkillClass> {
    vec![
        crate::player::skills::SkillClass::Warrior,
        crate::player::skills::SkillClass::Wizard,
        crate::player::skills::SkillClass::Rogue,
        crate::player::skills::SkillClass::Thief,
        crate::player::skills::SkillClass::Hunter,
    ]
}

/// Check if a class is unlocked based on achievements
pub fn is_class_unlocked(
    class: &crate::player::skills::SkillClass,
    achievements: &Achievements,
) -> bool {
    return true;
    // Default unlocked classes
    if get_default_unlocked_classes().contains(class) {
        return true;
    }

    // Check if any achievement unlocks this class
    for achievement in &achievements.unlocked {
        if let Some(unlocked_class) = achievement.unlocks_class() {
            if unlocked_class == *class {
                return true;
            }
        }
    }

    false
}

/// Check if a pet is unlocked based on achievements
pub fn is_pet_unlocked(pet: &crate::pets::state::Pet, achievements: &Achievements) -> bool {
    if matches!(pet, crate::pets::state::Pet::Goliath) {
        return true;
    }

    // Porkipine and GoldenPig are available to anyone who has completed Act 2
    if matches!(pet, crate::pets::state::Pet::Porkipine | crate::pets::state::Pet::GoldenPig)
        && achievements.has(Achievement::Act2)
    {
        return true;
    }

    // Check if any achievement unlocks this pet
    for achievement in &achievements.unlocked {
        if let Some(unlocked_pet) = achievement.unlocks_pet() {
            if unlocked_pet == *pet {
                return true;
            }
        }
    }

    false
}
