use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

use crate::{
    client::{analytics::AnalyticsData, handle_append_run_data_after_death},
    enemy::Mob,
    GameState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumIter)]
pub enum Achievement {
    FirstRunComplete,
    Kill100FurDevils,
    SlimePet,
    FairyPet,
    // Add more achievements here as needed
}

#[derive(Resource, Debug, Clone, Serialize, Deserialize, Default)]
pub struct Achievements {
    pub unlocked: Vec<Achievement>,
}
impl Achievement {
    pub fn get_name(&self) -> String {
        match self {
            Achievement::FirstRunComplete => "That was weird".to_string(),
            Achievement::Kill100FurDevils => "Fur Devil Slayer".to_string(),
            Achievement::SlimePet => "Slimed".to_string(),
            Achievement::FairyPet => "Fairy Friend".to_string(),
        }
    }
    pub fn get_desc(&self) -> String {
        match self {
            Achievement::FirstRunComplete => "Complete your first run.".to_string(),
            Achievement::Kill100FurDevils => "Defeat 100 Fur Devils.".to_string(),
            Achievement::SlimePet => "Find the Slime in Act 1.".to_string(),
            Achievement::FairyPet => "Find the Fairy in Act 2.".to_string(),
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

pub struct AchievementsPlugin;

impl Plugin for AchievementsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AchievementUnlockedEvent>().add_systems(
            (
                check_achievements,
                check_first_run_achievement.before(handle_append_run_data_after_death),
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}

/// System to check and award achievements based on game events
pub fn check_achievements(
    mut achievements: ResMut<Achievements>,
    analytics: Option<Res<AnalyticsData>>,
    mut achievement_events: EventWriter<AchievementUnlockedEvent>,
) {
    // Check for kill 100 FurDevils achievement
    if let Some(analytics_data) = analytics.as_ref() {
        if let Some(kills) = analytics_data.mobs_killed.get(&Mob::FurDevil) {
            if *kills >= 100 && !achievements.has(Achievement::Kill100FurDevils) {
                achievements.unlock(Achievement::Kill100FurDevils);
                achievement_events.send(AchievementUnlockedEvent {
                    achievement: Achievement::Kill100FurDevils,
                });
                info!("Achievement unlocked: {:?}", Achievement::Kill100FurDevils);
            }
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
        if !achievements.has(Achievement::FirstRunComplete) {
            achievements.unlock(Achievement::FirstRunComplete);
            achievement_events.send(AchievementUnlockedEvent {
                achievement: Achievement::FirstRunComplete,
            });
            info!("Achievement unlocked: {:?}", Achievement::FirstRunComplete);
        }
    }
}

pub struct AchievementUnlockedEvent {
    pub achievement: Achievement,
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
        crate::player::skills::SkillClass::Paladin,
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
