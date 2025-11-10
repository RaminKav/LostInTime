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
    player::UnlockCurrency,
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
    FindFireStaff,
    FindIceStaff,
    FindBasicStaff,
    FindMagicWhip,
    FindDagger,
    FindBow,
    FindBlowdart,
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct Achievements {
    pub unlocked: Vec<Achievement>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct BounceAchievementTracker {
    pub total_pink_bounces: u32,
    pub consecutive_pink_bounces: u32,
    pub last_bounce_time: Option<f64>,
}
impl Achievement {
    pub fn get_name(&self) -> String {
        match self {
            Achievement::FirstRunComplete => "That was weird".to_string(),
            Achievement::Kill100FurDevils => "Fur Devil Slayer".to_string(),
            Achievement::SlimePet => "Slimed".to_string(),
            Achievement::FairyPet => "Fairy Friend".to_string(),
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
            Achievement::FindFireStaff => "Find Fire Staff".to_string(),
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
            Achievement::Kill100FurDevils => "Defeat 100 Fur Devils.".to_string(),
            Achievement::SlimePet => "Find the Slime in Act 1.".to_string(),
            Achievement::FairyPet => "Find the Fairy in Act 2.".to_string(),
            Achievement::Bouncy => "Bounce on 100 pink petals.".to_string(),
            Achievement::Bouncy2 => "Chain three pink petal bounces.".to_string(),
            Achievement::Act1 => "Defeat the Act 1 boss.".to_string(),
            Achievement::Act2 => "Defeat the Act 2 boss.".to_string(),
            Achievement::Act3 => "Defeat the Act 3 boss.".to_string(),
            Achievement::BushlingSlayer1 => "Eliminate 100 Bushlings.".to_string(),
            Achievement::StingflySlayer => "Eliminate 100 Stingflies.".to_string(),
            Achievement::MushlingSlayer => "Eliminate 100 Red Mushlings.".to_string(),
            Achievement::Chaotic => "Reach 20 total Chaos.".to_string(),
            Achievement::DungeonCrawler => "Find the key & clear the dungeon.".to_string(),
            Achievement::FindSpear => "Find a Spear.".to_string(),
            Achievement::FindHammer => "Find a Hammer.".to_string(),
            Achievement::FindClaw => "Find a Claw.".to_string(),
            Achievement::FindGun => "Find a Gun.".to_string(),
            Achievement::FindFireStaff => "Find a Fire Staff.".to_string(),
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
            Achievement::FindSpear
            | Achievement::FindClaw
            | Achievement::FindHammer
            | Achievement::FindGun
            | Achievement::FindFireStaff
            | Achievement::FindIceStaff
            | Achievement::FindBasicStaff
            | Achievement::FindMagicWhip
            | Achievement::FindDagger
            | Achievement::FindBow
            | Achievement::FindBlowdart => 10,
            _ => 0,
        }
    }
}

impl Achievements {
    pub fn has(&self, achievement: Achievement) -> bool {
        self.unlocked.contains(&achievement)
    }

    pub fn unlock(&mut self, achievement: Achievement) -> bool {
        if !self.has(achievement) {
            self.unlocked.push(achievement);
            return true;
        }
        false
    }
}

fn persist_achievements_state(achievements: &Achievements) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        serde_json::from_reader::<_, GameData>(reader).unwrap_or_default()
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
    if achievements.unlock(achievement) {
        persist_achievements_state(achievements);
        achievement_events.send(AchievementUnlockedEvent {
            achievement,
            reward_currency: achievement.reward_currency(),
        });
        info!("Achievement unlocked: {:?}", achievement);
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
) {
    if let Some(analytics_data) = analytics.as_ref() {
        let mut check_mob_kill = |mob: Mob, threshold: u32, achievement: Achievement| {
            if let Some(kills) = analytics_data.mobs_killed.get(&mob) {
                if *kills >= threshold {
                    try_unlock(&mut achievements, achievement, &mut achievement_events);
                }
            }
        };

        check_mob_kill(Mob::FurDevil, 100, Achievement::Kill100FurDevils);
        check_mob_kill(Mob::Bushling, 100, Achievement::BushlingSlayer1);
        check_mob_kill(Mob::StingFly, 100, Achievement::StingflySlayer);
        check_mob_kill(Mob::RedMushling, 100, Achievement::MushlingSlayer);

        let mut check_item_collected = |object: WorldObject, achievement: Achievement| {
            if analytics_data
                .items_collected
                .get(&object)
                .map_or(false, |count| *count > 0)
            {
                try_unlock(&mut achievements, achievement, &mut achievement_events);
            }
        };

        check_item_collected(WorldObject::Spear, Achievement::FindSpear);
        check_item_collected(WorldObject::Claw, Achievement::FindClaw);
        check_item_collected(WorldObject::Gun, Achievement::FindGun);
        check_item_collected(WorldObject::FireStaff, Achievement::FindFireStaff);
        check_item_collected(WorldObject::IceStaff, Achievement::FindIceStaff);
        check_item_collected(WorldObject::BasicStaff, Achievement::FindBasicStaff);
        check_item_collected(WorldObject::MagicWhip, Achievement::FindMagicWhip);
        check_item_collected(WorldObject::Dagger, Achievement::FindDagger);
        check_item_collected(WorldObject::Hammer, Achievement::FindHammer);
        check_item_collected(WorldObject::WoodBow, Achievement::FindBow);
        check_item_collected(WorldObject::Blowdart, Achievement::FindBlowdart);

        let found_key = analytics_data
            .items_collected
            .get(&WorldObject::Key)
            .map_or(false, |count| *count > 0);

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

pub fn handle_achievement_rewards(
    mut achievement_events: EventReader<AchievementUnlockedEvent>,
    mut currency: Option<ResMut<UnlockCurrency>>,
) {
    if let Some(mut currency_res) = currency {
        for event in achievement_events.iter() {
            if event.reward_currency > 0 {
                currency_res.add(event.reward_currency);
            }
        }
    }
}

pub struct AchievementUnlockedEvent {
    pub achievement: Achievement,
    pub reward_currency: u32,
}

/// Maps achievements to the classes they unlock
impl Achievement {
    pub fn unlocks_class(&self) -> Option<crate::player::skills::SkillClass> {
        match self {
            Achievement::FirstRunComplete => Some(crate::player::skills::SkillClass::Knight),
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
            _ => None,
        }
    }
}

/// Returns which classes are unlocked by default (first two classes)
pub fn get_default_unlocked_classes() -> Vec<crate::player::skills::SkillClass> {
    vec![
        crate::player::skills::SkillClass::Warrior,
        crate::player::skills::SkillClass::FireMage,
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
