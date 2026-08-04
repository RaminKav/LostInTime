use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};

use crate::{
    attributes::{CritChance, MaxHealth, Thorns},
    chaos::ChaosTracker,
    client::{analytics::AnalyticsData, handle_append_run_data_after_death, GameData},
    combat::{
        damage_tracker::{format_damage, DamageSource, DamageTracker},
        status_effects::MobStatusEffects,
    },
    datafiles,
    enemy::Mob,
    item::WorldObject,
    night::InfiniteMode,
    player::{
        combat_heirlooms::{
            DeathDefianceSurvivedEvent, LegendaryEquipmentRankedEvent, OrbitingStone,
            StoneToothRockLifetime,
        },
        currency::CoinCurrency,
        skills::MeteorShowerSkillState,
        TimeFragmentCurrency,
    },
    world::dimension::Era,
    world::portal::BossKillTracker,
    BounceEvent, GameState, Player,
};

const LIGHTNING_DAMAGE_ACHIEVEMENT_THRESHOLD: i64 = 5_000_000;
const ENDLESS_SURVIVAL_ACHIEVEMENT_SECONDS: f32 = 12.0 * 60.0;
const ERA1_BOSS_SPEED_RUN_SECONDS: f64 = 180.0;

fn fmt_count(n: u32) -> String {
    format_damage(n as i64)
}

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
    VoidCrawlerSlayer1,
    VoidCrawlerSlayer2,
    VoidCrawlerSlayer3,
    VoidWormSlayer1,
    SmallCactusSlayer1,
    BigCactusSlayer1,
    BullSlayer1,
    ScorpionSlayer1,
    LizardSlayer1,
    Chaotic,
    DungeonCrawler,
    FindHammer,
    FindSpear,
    FindClaw,
    FindIceStaff,
    FindBasicStaff,
    FindMagicWhip,
    FindDagger,
    FindBow,
    FindBlowdart,
    Thorns500,
    MaxCritChance300,
    OneHitThousand,
    TenBoulders,
    Poison500Stacks,
    MeteorShower100,
    LegendaryEquipment,
    MaxHp1000,
    SurviveDeath,
    Gold5000,
    LightningFiveMillion,
    FlawlessEra1,
    Endless12Minutes,
    Era1BossUnder3Min,
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

    /// Achievements whose requirements are met (completed but unclaimed, or already claimed).
    pub fn finished_count(&self) -> usize {
        Achievement::iter()
            .filter(|achievement| self.is_completed(*achievement) || self.is_claimed(*achievement))
            .count()
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
            Achievement::VoidCrawlerSlayer1 => "Void Slayer".to_string(),
            Achievement::VoidCrawlerSlayer2 => "Void Slayer II".to_string(),
            Achievement::VoidCrawlerSlayer3 => "Void Slayer III".to_string(),
            Achievement::VoidWormSlayer1 => "Void Worm Slayer".to_string(),
            Achievement::SmallCactusSlayer1 => "Small Cactus Slayer".to_string(),
            Achievement::BigCactusSlayer1 => "Large Cactus Slayer".to_string(),
            Achievement::BullSlayer1 => "Bull Slayer".to_string(),
            Achievement::ScorpionSlayer1 => "Desert Scorpion Slayer".to_string(),
            Achievement::LizardSlayer1 => "Lizard Slayer".to_string(),
            Achievement::Chaotic => "Chaotic".to_string(),
            Achievement::DungeonCrawler => "Dungeon Crawler".to_string(),
            Achievement::FindSpear => "Find Spear".to_string(),
            Achievement::FindHammer => "Find Hammer".to_string(),
            Achievement::FindClaw => "Find Claw".to_string(),
            Achievement::FindIceStaff => "Find Ice Staff".to_string(),
            Achievement::FindBasicStaff => "Find Basic Staff".to_string(),
            Achievement::FindMagicWhip => "Find Magic Whip".to_string(),
            Achievement::FindDagger => "Find Dagger".to_string(),
            Achievement::FindBow => "Find Bow".to_string(),
            Achievement::FindBlowdart => "Find Blowdart".to_string(),
            Achievement::Thorns500 => "Prickly".to_string(),
            Achievement::MaxCritChance300 => "Perfect Aim".to_string(),
            Achievement::OneHitThousand => "One Hit Wonder".to_string(),
            Achievement::TenBoulders => "Boulder Buddies".to_string(),
            Achievement::Poison500Stacks => "Super Toxic".to_string(),
            Achievement::MeteorShower100 => "Meteor Storm".to_string(),
            Achievement::LegendaryEquipment => "Legendary".to_string(),
            Achievement::MaxHp1000 => "Unbreakable".to_string(),
            Achievement::SurviveDeath => "Second Wind".to_string(),
            Achievement::Gold5000 => "Hoarder".to_string(),
            Achievement::LightningFiveMillion => "Storm Bringer".to_string(),
            Achievement::FlawlessEra1 => "Perfection".to_string(),
            Achievement::Endless12Minutes => "Will it end?".to_string(),
            Achievement::Era1BossUnder3Min => "Speed Runner".to_string(),
        }
    }
    pub fn get_desc(&self) -> String {
        match self {
            Achievement::FirstRunComplete => "Complete your first run.".to_string(),
            Achievement::Kill100FurDevils => {
                format!("Defeat {} Fur Devils.", fmt_count(5000))
            }
            Achievement::SlimePet => "Find the Slime in Act 1.".to_string(),
            Achievement::FairyPet => "Find the Fairy in Act 2.".to_string(),
            Achievement::PorkipinePet => "Find the Porkipine.".to_string(),
            Achievement::GoldenPigPet => "Find the Golden Pig.".to_string(),
            Achievement::Bouncy => format!("Bounce on {} pink petals.", fmt_count(100)),
            Achievement::Bouncy2 => "Chain three pink petal bounces.".to_string(),
            Achievement::Act1 => "Defeat the Act 1 boss.".to_string(),
            Achievement::Act2 => "Defeat the Act 2 boss.".to_string(),
            Achievement::Act3 => "Defeat the Act 3 boss.".to_string(),
            Achievement::BushlingSlayer1 => format!("Defeat {} Bushlings.", fmt_count(5000)),
            Achievement::StingflySlayer => format!("Defeat {} Stingflies.", fmt_count(5000)),
            Achievement::MushlingSlayer => {
                format!("Defeat {} Red Mushlings.", fmt_count(5000))
            }
            Achievement::VoidCrawlerSlayer1 => {
                format!("Defeat {} Void Crawlers.", fmt_count(5000))
            }
            Achievement::VoidCrawlerSlayer2 => {
                format!("Defeat {} Void Crawlers.", fmt_count(50_000))
            }
            Achievement::VoidCrawlerSlayer3 => {
                format!("Defeat {} Void Crawlers.", fmt_count(500_000))
            }
            Achievement::VoidWormSlayer1 => format!("Defeat {} Void Worms.", fmt_count(1000)),
            Achievement::SmallCactusSlayer1 => {
                format!("Defeat {} Small Cacti.", fmt_count(5000))
            }
            Achievement::BigCactusSlayer1 => {
                format!("Defeat {} Large Cacti.", fmt_count(5000))
            }
            Achievement::BullSlayer1 => format!("Defeat {} Bulls.", fmt_count(5000)),
            Achievement::ScorpionSlayer1 => {
                format!("Defeat {} Desert Scorpions.", fmt_count(5000))
            }
            Achievement::LizardSlayer1 => format!("Defeat {} Lizards.", fmt_count(5000)),
            Achievement::Chaotic => format!("Reach {} total Chaos.", fmt_count(20)),
            Achievement::DungeonCrawler => "Find the key & clear the dungeon.".to_string(),
            Achievement::FindSpear => "Find a Spear.".to_string(),
            Achievement::FindHammer => "Find a Hammer.".to_string(),
            Achievement::FindClaw => "Find a Claw.".to_string(),
            Achievement::FindIceStaff => "Find an Ice Staff.".to_string(),
            Achievement::FindBasicStaff => "Find a Basic Staff.".to_string(),
            Achievement::FindMagicWhip => "Find a Magic Whip.".to_string(),
            Achievement::FindDagger => "Find a Dagger.".to_string(),
            Achievement::FindBow => "Find a Bow.".to_string(),
            Achievement::FindBlowdart => "Find a Blowdart.".to_string(),
            Achievement::Thorns500 => format!("Get {} thorns in a run.", fmt_count(500)),
            Achievement::MaxCritChance300 => {
                format!("Get max crit chance ({}).", fmt_count(300))
            }
            Achievement::OneHitThousand => {
                format!("Deal {} damage in one hit.", fmt_count(1000))
            }
            Achievement::TenBoulders => {
                format!("Have {} Boulders summoned at once.", fmt_count(10))
            }
            Achievement::Poison500Stacks => {
                format!("Get an enemy to {} stacks of poison.", fmt_count(500))
            }
            Achievement::MeteorShower100 => {
                format!("Get Meteor Shower to {} meteors.", fmt_count(100))
            }
            Achievement::LegendaryEquipment => {
                "Rank up a piece of equipment to\nLegendary rarity.".to_string()
            }
            Achievement::MaxHp1000 => format!("Get {} Max HP in a run.", fmt_count(1000)),
            Achievement::SurviveDeath => "Survive Death...".to_string(),
            Achievement::Gold5000 => format!("Have {} gold at once.", fmt_count(5000)),
            Achievement::LightningFiveMillion => format!(
                "Deal {} damage with lightning in\na run.",
                fmt_count(LIGHTNING_DAMAGE_ACHIEVEMENT_THRESHOLD as u32)
            ),
            Achievement::FlawlessEra1 => "Beat Era 1 without taking any damage.".to_string(),
            Achievement::Endless12Minutes => {
                format!(
                    "Survive {} minutes in Endless mode.",
                    ENDLESS_SURVIVAL_ACHIEVEMENT_SECONDS as u32 / 60
                )
            }
            Achievement::Era1BossUnder3Min => {
                format!(
                    "Defeat the Era 1 boss in under {}\nminutes.",
                    ERA1_BOSS_SPEED_RUN_SECONDS as u32 / 60
                )
            }
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
            Achievement::VoidCrawlerSlayer1 => 7,
            Achievement::VoidCrawlerSlayer2 => 15,
            Achievement::VoidCrawlerSlayer3 => 25,
            Achievement::VoidWormSlayer1 => 7,
            Achievement::SmallCactusSlayer1 => 7,
            Achievement::BigCactusSlayer1 => 7,
            Achievement::BullSlayer1 => 7,
            Achievement::ScorpionSlayer1 => 7,
            Achievement::LizardSlayer1 => 7,
            Achievement::Thorns500 => 5,
            Achievement::MaxCritChance300 => 10,
            Achievement::OneHitThousand => 10,
            Achievement::TenBoulders => 5,
            Achievement::Poison500Stacks => 7,
            Achievement::MeteorShower100 => 10,
            Achievement::LegendaryEquipment => 3,
            Achievement::MaxHp1000 => 7,
            Achievement::SurviveDeath => 8,
            Achievement::Gold5000 => 10,
            Achievement::LightningFiveMillion => 5,
            Achievement::FlawlessEra1 => 25,
            Achievement::Endless12Minutes => 20,
            Achievement::Era1BossUnder3Min => 20,
            Achievement::Chaotic => 5,
            Achievement::FindSpear
            | Achievement::FindClaw
            | Achievement::FindHammer
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
        meteor_shower_state: Option<&MeteorShowerSkillState>,
    ) -> Option<(u32, u32)> {
        // Helper to get combined mob kills from both cumulative and current run
        let get_mob_kills = |mob: &Mob| -> u32 {
            let cumulative_kills = cumulative_analytics
                .and_then(|c| c.mobs_killed.get(mob).copied())
                .unwrap_or(0);
            let current_kills = current_run_analytics
                .and_then(|c| c.mobs_killed.get(mob).copied())
                .unwrap_or(0);
            // Add current run kills to cumulative total
            cumulative_kills + current_kills
        };

        match self {
            // Achievements that need analytics
            Achievement::Kill100FurDevils => Some((get_mob_kills(&Mob::FurDevil), 5000)),
            Achievement::BushlingSlayer1 => Some((get_mob_kills(&Mob::Bushling), 5000)),
            Achievement::StingflySlayer => Some((get_mob_kills(&Mob::StingFly), 5000)),
            Achievement::MushlingSlayer => Some((get_mob_kills(&Mob::RedMushling), 5000)),
            Achievement::VoidCrawlerSlayer1 => Some((get_mob_kills(&Mob::VoidCrawler), 5000)),
            Achievement::VoidCrawlerSlayer2 => Some((get_mob_kills(&Mob::VoidCrawler), 50000)),
            Achievement::VoidCrawlerSlayer3 => Some((get_mob_kills(&Mob::VoidCrawler), 500000)),
            Achievement::VoidWormSlayer1 => Some((get_mob_kills(&Mob::VoidWorm), 5000)),
            Achievement::SmallCactusSlayer1 => Some((get_mob_kills(&Mob::SmallCactus), 5000)),
            Achievement::BigCactusSlayer1 => Some((get_mob_kills(&Mob::BigCactus), 5000)),
            Achievement::BullSlayer1 => Some((get_mob_kills(&Mob::Bull), 5000)),
            Achievement::ScorpionSlayer1 => Some((get_mob_kills(&Mob::Scorpion), 5000)),
            Achievement::LizardSlayer1 => Some((get_mob_kills(&Mob::Lizard), 5000)),
            Achievement::MeteorShower100 => {
                meteor_shower_state.map(|state| (state.meteor_count.min(100), 100))
            }
            // Achievements that don't need analytics
            Achievement::Bouncy => {
                if let Some(bounce) = bounce_tracker {
                    Some((bounce.total_pink_bounces, 100))
                } else {
                    Some((0, 100))
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
    achievement_events: &mut MessageWriter<AchievementUnlockedEvent>,
) -> bool {
    if achievements.complete(achievement) {
        persist_achievements_state(achievements);
        achievement_events.write(AchievementUnlockedEvent {
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
        app.add_message::<AchievementUnlockedEvent>()
            .add_message::<DeathDefianceSurvivedEvent>()
            .add_message::<LegendaryEquipmentRankedEvent>()
            .init_resource::<BounceAchievementTracker>()
            .add_systems(
                Update,
                (
                    track_bounce_achievements,
                    track_death_defiance_achievement,
                    track_legendary_equipment_achievement,
                    check_achievements,
                    check_first_run_achievement.before(handle_append_run_data_after_death),
                    handle_achievement_rewards,
                )
                    .run_if(in_state(GameState::Main)),
            );
    }
}

/// System to check and award achievements based on game events
pub fn check_achievements(
    mut achievements: ResMut<Achievements>,
    analytics: Option<Res<AnalyticsData>>,
    chaos_tracker: Option<Res<ChaosTracker>>,
    boss_kill_tracker: Option<Res<BossKillTracker>>,
    damage_tracker: Option<Res<DamageTracker>>,
    coins: Option<Res<CoinCurrency>>,
    infinite_mode: Option<Res<InfiniteMode>>,
    mut achievement_events: MessageWriter<AchievementUnlockedEvent>,
    game_data: Option<Res<crate::client::GameData>>,
    player_stats: Query<(&Thorns, &CritChance, &MaxHealth), With<Player>>,
    meteor_shower_state: Query<&MeteorShowerSkillState, With<Player>>,
    player_entity: Query<Entity, With<Player>>,
    orbiting_stones: Query<(&OrbitingStone, Option<&StoneToothRockLifetime>)>,
    mob_status: Query<&MobStatusEffects, With<Mob>>,
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
            .and_then(|c| c.mobs_killed.get(mob).copied())
            .unwrap_or(0);
        let current_kills = current_run
            .and_then(|c| c.mobs_killed.get(mob).copied())
            .unwrap_or(0);
        // Add current run kills to cumulative total
        cumulative_kills + current_kills
    };

    let get_item_collected = |object: &WorldObject| -> u32 {
        let cumulative_count = cumulative
            .and_then(|c| c.items_collected.get(object).copied())
            .unwrap_or(0);
        let current_count = current_run
            .and_then(|c| c.items_collected.get(object).copied())
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

    check_mob_kill(Mob::FurDevil, 5000, Achievement::Kill100FurDevils);
    check_mob_kill(Mob::Bushling, 5000, Achievement::BushlingSlayer1);
    check_mob_kill(Mob::StingFly, 5000, Achievement::StingflySlayer);
    check_mob_kill(Mob::RedMushling, 5000, Achievement::MushlingSlayer);
    check_mob_kill(Mob::VoidCrawler, 5000, Achievement::VoidCrawlerSlayer1);
    check_mob_kill(Mob::VoidCrawler, 50000, Achievement::VoidCrawlerSlayer2);
    check_mob_kill(Mob::VoidCrawler, 500000, Achievement::VoidCrawlerSlayer3);
    check_mob_kill(Mob::VoidWorm, 1000, Achievement::VoidWormSlayer1);
    check_mob_kill(Mob::SmallCactus, 5000, Achievement::SmallCactusSlayer1);
    check_mob_kill(Mob::BigCactus, 5000, Achievement::BigCactusSlayer1);
    check_mob_kill(Mob::Bull, 5000, Achievement::BullSlayer1);
    check_mob_kill(Mob::Scorpion, 5000, Achievement::ScorpionSlayer1);
    check_mob_kill(Mob::Lizard, 5000, Achievement::LizardSlayer1);

    // Check item collected achievements
    let mut check_item_collected = |object: WorldObject, achievement: Achievement| {
        if get_item_collected(&object) > 0 {
            try_unlock(&mut achievements, achievement, &mut achievement_events);
        }
    };

    check_item_collected(WorldObject::Spear, Achievement::FindSpear);
    check_item_collected(WorldObject::Claw, Achievement::FindClaw);
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

    if let Ok((thorns, crit_chance, max_health)) = player_stats.single() {
        if thorns.0 >= 500 {
            try_unlock(
                &mut achievements,
                Achievement::Thorns500,
                &mut achievement_events,
            );
        }
        if crit_chance.0 >= 300 {
            try_unlock(
                &mut achievements,
                Achievement::MaxCritChance300,
                &mut achievement_events,
            );
        }
        if max_health.0 >= 1000 {
            try_unlock(
                &mut achievements,
                Achievement::MaxHp1000,
                &mut achievement_events,
            );
        }
    }

    if let Ok(state) = meteor_shower_state.single() {
        if state.meteor_count >= 100 {
            try_unlock(
                &mut achievements,
                Achievement::MeteorShower100,
                &mut achievement_events,
            );
        }
    }

    if let Some(coins) = coins.as_ref() {
        if coins.coins >= 5000 {
            try_unlock(
                &mut achievements,
                Achievement::Gold5000,
                &mut achievement_events,
            );
        }
    }

    if let Some(tracker) = damage_tracker.as_ref() {
        if tracker.max_single_hit >= 1000 {
            try_unlock(
                &mut achievements,
                Achievement::OneHitThousand,
                &mut achievement_events,
            );
        }
        let lightning_damage = tracker
            .totals
            .get(&DamageSource::Lightning)
            .copied()
            .unwrap_or(0);
        if lightning_damage >= LIGHTNING_DAMAGE_ACHIEVEMENT_THRESHOLD {
            try_unlock(
                &mut achievements,
                Achievement::LightningFiveMillion,
                &mut achievement_events,
            );
        }
    }

    if let Some(mode) = infinite_mode.as_ref() {
        if mode.active && mode.elapsed_seconds >= ENDLESS_SURVIVAL_ACHIEVEMENT_SECONDS {
            try_unlock(
                &mut achievements,
                Achievement::Endless12Minutes,
                &mut achievement_events,
            );
        }
    }

    if let Ok(player_e) = player_entity.single() {
        let active_boulders = orbiting_stones
            .iter()
            .filter(|(stone, lifetime)| {
                stone.owner == player_e
                    && lifetime.map(|l| !l.lifetime.is_finished()).unwrap_or(false)
            })
            .count();
        if active_boulders >= 10 {
            try_unlock(
                &mut achievements,
                Achievement::TenBoulders,
                &mut achievement_events,
            );
        }
    }

    for status in mob_status.iter() {
        if status
            .burning
            .as_ref()
            .is_some_and(|burning| burning.stacks >= 500)
        {
            try_unlock(
                &mut achievements,
                Achievement::Poison500Stacks,
                &mut achievement_events,
            );
            break;
        }
    }

    if let Some(boss_kills) = boss_kill_tracker.as_ref() {
        if boss_kills.is_boss_killed(&Era::Main) {
            try_unlock(
                &mut achievements,
                Achievement::Act1,
                &mut achievement_events,
            );
            if current_run
                .map(|run| run.total_damage_taken == 0)
                .unwrap_or(false)
            {
                try_unlock(
                    &mut achievements,
                    Achievement::FlawlessEra1,
                    &mut achievement_events,
                );
            }
            if boss_kills
                .era1_boss_kill_elapsed_seconds
                .is_some_and(|elapsed| elapsed <= ERA1_BOSS_SPEED_RUN_SECONDS)
            {
                try_unlock(
                    &mut achievements,
                    Achievement::Era1BossUnder3Min,
                    &mut achievement_events,
                );
            }
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
    mut game_over_events: MessageReader<crate::client::GameOverEvent>,
    mut achievement_events: MessageWriter<AchievementUnlockedEvent>,
) {
    for _ in game_over_events.read() {
        try_unlock(
            &mut achievements,
            Achievement::FirstRunComplete,
            &mut achievement_events,
        );
    }
}

pub fn track_death_defiance_achievement(
    mut events: MessageReader<DeathDefianceSurvivedEvent>,
    mut achievements: ResMut<Achievements>,
    mut achievement_events: MessageWriter<AchievementUnlockedEvent>,
) {
    for _ in events.read() {
        try_unlock(
            &mut achievements,
            Achievement::SurviveDeath,
            &mut achievement_events,
        );
    }
}

pub fn track_legendary_equipment_achievement(
    mut events: MessageReader<LegendaryEquipmentRankedEvent>,
    mut achievements: ResMut<Achievements>,
    mut achievement_events: MessageWriter<AchievementUnlockedEvent>,
) {
    for _ in events.read() {
        try_unlock(
            &mut achievements,
            Achievement::LegendaryEquipment,
            &mut achievement_events,
        );
    }
}

pub fn track_bounce_achievements(
    mut bounce_events: MessageReader<BounceEvent>,
    time: Res<Time>,
    mut tracker: ResMut<BounceAchievementTracker>,
    mut achievements: ResMut<Achievements>,
    mut achievement_events: MessageWriter<AchievementUnlockedEvent>,
) {
    const MIN_BOUNCE_INTERVAL_SECS: f64 = 0.2;
    const CONSECUTIVE_INTERVAL_SECS: f64 = 0.38;

    let mut unlocked_bouncy = false;
    let mut unlocked_bouncy2 = false;

    for _event in bounce_events.read() {
        let now = time.elapsed_secs_f64();

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
    mut _events: MessageReader<AchievementUnlockedEvent>,
    _currency: Option<ResMut<TimeFragmentCurrency>>,
) {
    // Rewards are now claimed when player clicks on achievement row in UI
}

#[derive(Message)]
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

/// Returns which classes are unlocked by default (Warrior and Wizard).
pub fn get_default_unlocked_classes() -> Vec<crate::player::skills::SkillClass> {
    vec![
        crate::player::skills::SkillClass::Warrior,
        crate::player::skills::SkillClass::Wizard,
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
    if matches!(
        pet,
        crate::pets::state::Pet::Porkipine | crate::pets::state::Pet::GoldenPig
    ) && achievements.has(Achievement::Act2)
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
