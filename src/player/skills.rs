use std::collections::HashMap;
use std::time::Duration;

use crate::aseprite_assets::{
    PlayerGreyAseprite, PlayerHunterAseprite, PlayerRedAseprite, PlayerRogueAseprite,
    PlayerThiefAseprite, PlayerWizardAseprite,
};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::Aseprite;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use strum_macros::{Display, EnumIter};

use crate::{
    animations::player_sprite::PlayerSpriteHandles,
    attributes::{AttributeQuality, AttributeValue, ItemAttributes, ItemGlow},
    chaos::ChaosTracker,
    colors::{
        COMMON_TOOLTIP_TITLE, LEGENDARY_TOOLTIP_TITLE, RARE_TOOLTIP_TITLE, UNCOMMON_TOOLTIP_TITLE,
    },
    combat::pickup_radius::{
        MagnetPullTimer, BASE_MAGNET_COOLDOWN, MAGNET_COOLDOWN_REDUCTION_PER_STACK,
        MIN_MAGNET_COOLDOWN,
    },
    custom_commands::CommandsExt,
    item::{
        item_upgrades::{ArrowSpeedUpgrade, BowUpgradeSpread, ClawUpgradeMultiThrow},
        projectile::Projectile,
        WorldObject,
    },
    player::time_crystals::TimeCrystals,
    proto::proto_param::ProtoParam,
    ui::{HeirloomDescLine, HeirloomDescLineKind, TooltipDefinition, UIElement},
    Pet,
};

use super::{mage_skills::TeleportState, rogue_skills::ComboCounter};

#[derive(
    Component,
    Debug,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    Hash,
    EnumIter,
    Default,
    Display,
)]
pub enum SkillClass {
    #[default]
    None,

    Warrior, // Sword, Spear, Hammer - +3 size per level
    Wizard,  // Fire Staff, Ice Staff, Basic Staff - +8 max mana per level
    Rogue,   // Dagger - +3 speed per level
    Thief,   // Claw - +3% attack speed per level
    Hunter,  // Bow, Gun - +3% crit dmg per level
}

#[derive(Debug, Clone, Serialize, Deserialize, Resource, Default)]
pub struct PlayerClass {
    pub class: SkillClass,
    pub pets: Vec<Pet>,
}

impl SkillClass {
    pub fn get_cape(&self) -> WorldObject {
        match self {
            SkillClass::Warrior => WorldObject::RedCape,
            SkillClass::Wizard => WorldObject::BlueCape,
            SkillClass::Rogue => WorldObject::GreenCape,
            SkillClass::Thief => WorldObject::GreyCape,
            SkillClass::Hunter => WorldObject::GreenCape,
            _ => WorldObject::GreyCape,
        }
    }
    pub fn get_anim_data(&self, sprites: &PlayerSpriteHandles) -> (Handle<Aseprite>, &str) {
        match self {
            SkillClass::Warrior => (sprites.red.clone(), PlayerRedAseprite::tags::IDLE_FRONT),
            SkillClass::Wizard => (
                sprites.wizard.clone(),
                PlayerWizardAseprite::tags::IDLE_FRONT,
            ),
            SkillClass::Rogue => (sprites.rogue.clone(), PlayerRogueAseprite::tags::IDLE_FRONT),
            SkillClass::Thief => (sprites.thief.clone(), PlayerThiefAseprite::tags::IDLE_FRONT),
            SkillClass::Hunter => (
                sprites.hunter.clone(),
                PlayerHunterAseprite::tags::IDLE_FRONT,
            ),
            _ => (sprites.grey.clone(), PlayerGreyAseprite::tags::IDLE_FRONT),
        }
    }
    /// Returns all starting weapons for the class
    pub fn get_starting_weapons(&self) -> Vec<WorldObject> {
        match self {
            SkillClass::Warrior => {
                vec![WorldObject::Sword, WorldObject::Spear, WorldObject::Hammer]
            }
            SkillClass::Wizard => {
                vec![
                    WorldObject::FireStaff,
                    WorldObject::IceStaff,
                    WorldObject::BasicStaff,
                ]
            }
            SkillClass::Rogue => vec![WorldObject::Dagger],
            SkillClass::Thief => vec![WorldObject::Claw],
            SkillClass::Hunter => vec![WorldObject::WoodBow, WorldObject::Gun],
            _ => vec![WorldObject::Sword],
        }
    }
    /// Returns the first starting weapon (default drop)
    pub fn get_starting_wep(&self) -> WorldObject {
        self.get_starting_weapons()
            .first()
            .cloned()
            .unwrap_or(WorldObject::Sword)
    }
    /// Returns the 4 active skills for this class

    pub fn compute_cape_stats(&self, level: i32) -> ItemAttributes {
        let mut stats = ItemAttributes::default();
        let quality = if level > 35 {
            AttributeQuality::High
        } else if level >= 10 {
            AttributeQuality::Average
        } else {
            AttributeQuality::Low
        };
        // Base damage bonus for all classes
        stats.bonus_damage = AttributeValue::new(f32::floor(level as f32 * 1.) as i32, quality, 1.);

        match self {
            SkillClass::Warrior => {
                stats.size = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Wizard => {
                stats.mana = AttributeValue::new(level * 8, quality, 1.);
            }
            SkillClass::Rogue => {
                stats.speed = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Thief => {
                stats.attack_speed = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Hunter => {
                stats.crit_chance = AttributeValue::new(level * 2, quality, 1.);
            }
            _ => (),
        }
        stats
    }
}

#[derive(
    Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, EnumIter, Display, Deserialize, Default,
)]
pub enum ActiveSkill {
    #[default]
    Roll,
    Parry,
    ParrySpear,
    Sprint,
    SprintLunge,
    Teleport,
    Stealth,
    Rapidfire,
    FirePillar,
    Heal,
    Buckshot,
    IceWall,
    MeteorShower,
    DruidTree,
    Shout,
    PiercingStar,
    LaserBeam,
    // New skills (consolidated classes)
    Lightning,   // Wizard - NEW
    DaggerThrow, // Rogue - NEW
    DaggerSlash, // Rogue - NEW
    TripleThrow, // Thief - NEW
    Fury,        // Thief - NEW
    Bomb,        // Hunter - NEW
    SpinAttack,
    ArrowVolley,    // Hunter - NEW
    PossessedBlade, // Rogue - NEW
    Recall,         // Rogue - NEW (rewind dash)
}

pub fn get_disabled_skills() -> Vec<ActiveSkill> {
    vec![
        ActiveSkill::Parry,
        ActiveSkill::Stealth,
        ActiveSkill::DaggerThrow,
        ActiveSkill::TripleThrow,
        ActiveSkill::PossessedBlade,
        ActiveSkill::Sprint,
        ActiveSkill::Lightning,
    ]
}

/// How many entries from each class's `active_skills` list are used in play
/// (class selection preview, HUD, `PlayerSkills` slots 0..2). The fourth field
/// (`active_skill_slot_3`) stays `None` while this is `3` for an easy revert.
pub const VISIBLE_CLASS_SKILL_COUNT: usize = 3;

/// Percent of base attack for UI (`skill_power_mult * VALUE`) and for damage
/// `attack * skill_power_mult * attack_damage_multiplier(VALUE)`, except
/// [`HEAL_MAX_HEALTH_PERCENT`] which scales max HP healed the same way.
pub mod active_skill_scaling {
    pub const SPIN_ATTACK: f32 = 80.0;
    pub const SHOUT: f32 = 160.0;
    /// Fraction of max health (same numeric as percent for heal formula).
    pub const HEAL_MAX_HEALTH_PERCENT: f32 = 30.0;
    pub const PARRY_SPEAR: f32 = 95.0;
    pub const SPRINT_LUNGE: f32 = 85.0;
    pub const DAGGER_THROW: f32 = 115.0;
    pub const DAGGER_SLASH: f32 = 60.0;
    /// Base number of DaggerSlash hits per cast (before speed scaling).
    pub const DAGGER_SLASH_BASE_SLASHES: u32 = 2;
    /// Player Speed required to gain +1 extra DaggerSlash hit on cast.
    pub const DAGGER_SLASH_SPEED_PER_EXTRA_SLASH: i32 = 35;
    /// Delay (seconds) between successive DaggerSlash hits at 0 bonus cadence.
    pub const DAGGER_SLASH_HIT_INTERVAL: f32 = 0.33;
    /// Each +`this` movement Speed adds +100% slash cadence (shorter gaps), capped below.
    pub const DAGGER_SLASH_CADENCE_SPEED_DIV: f32 = 100.0;
    /// Max cadence multiplier from speed (`hit_interval = base / mult`).
    pub const DAGGER_SLASH_CADENCE_MULT_MAX: f32 = 2.5;

    /// Extra DaggerSlash hits from movement [`crate::attributes::Speed`].
    #[inline]
    pub fn dagger_slash_extra_slashes_from_speed(speed: i32) -> u32 {
        (speed.max(0) / DAGGER_SLASH_SPEED_PER_EXTRA_SLASH) as u32
    }

    #[inline]
    pub fn dagger_slash_total_slashes(speed: i32) -> u32 {
        DAGGER_SLASH_BASE_SLASHES + dagger_slash_extra_slashes_from_speed(speed)
    }

    /// Seconds between follow-up DaggerSlash spawns; shrinks as Speed rises.
    #[inline]
    pub fn dagger_slash_hit_interval_seconds(speed: i32) -> f32 {
        let mult = (1.0 + speed.max(0) as f32 / DAGGER_SLASH_CADENCE_SPEED_DIV)
            .min(DAGGER_SLASH_CADENCE_MULT_MAX);
        DAGGER_SLASH_HIT_INTERVAL / mult
    }
    /// Teleport shock deals one-third of attack (percent = 100/3 for display).
    pub const TELEPORT_SHOCK_ATTACK_PERCENT: f32 = 80.0;
    pub const LIGHTNING: f32 = 120.0;
    pub const FIRE_PILLAR: f32 = 95.0;
    pub const ICE_WALL: f32 = 300.0;
    pub const METEOR_SHOWER: f32 = 85.0;
    /// Number of meteors the first MeteorShower cast spawns; grows +1 every
    /// [`METEOR_SHOWER_CASTS_PER_GROWTH`] casts.
    pub const METEOR_SHOWER_BASE_COUNT: u32 = 3;
    /// Number of casts required to gain +1 meteor.
    pub const METEOR_SHOWER_CASTS_PER_GROWTH: u32 = 5;
    /// Radius (tiles) around the player meteors can land within.
    pub const METEOR_SHOWER_RADIUS_TILES: f32 = 12.0;
    /// The first meteor of each cast always lands within this radius (tiles).
    pub const METEOR_SHOWER_FIRST_RADIUS_TILES: f32 = 6.0;
    /// Default delay (seconds) between successive meteor spawns within a cast.
    /// At low counts meteors spawn this far apart; at high counts the interval
    /// shrinks so the whole shower still finishes within
    /// [`METEOR_SHOWER_MAX_SUMMON_WINDOW_SECS`].
    pub const METEOR_SHOWER_SPAWN_INTERVAL_SECS: f32 = 0.12;
    /// Upper bound (seconds) on how long a full shower takes to finish summoning.
    /// Kept well below the skill's base cooldown so the shower always completes
    /// before the skill comes back up, without extending the cooldown timer
    /// itself (which would desync the per-slot cooldown/charge bookkeeping).
    pub const METEOR_SHOWER_MAX_SUMMON_WINDOW_SECS: f32 = 2.5;

    /// Per-meteor spawn delay for a shower of `count` meteors. Uses the default
    /// interval until the total would exceed [`METEOR_SHOWER_MAX_SUMMON_WINDOW_SECS`],
    /// then compresses so the shower always finishes within that window.
    #[inline]
    pub fn meteor_shower_spawn_interval_secs(count: u32) -> f32 {
        let count = count.max(1) as f32;
        (METEOR_SHOWER_MAX_SUMMON_WINDOW_SECS / count).min(METEOR_SHOWER_SPAWN_INTERVAL_SECS)
    }
    pub const BUCKSHOT_PELLET: f32 = 100.0;
    pub const BOMB: f32 = 220.0;
    pub const PIERCING_STAR: f32 = 150.0;
    pub const TRIPLE_THROW: f32 = 95.0;
    pub const FURY: f32 = 125.0;
    pub const LASER_BEAM: f32 = 60.0;
    pub const ARROW_VOLLEY: f32 = 75.0;
    pub const POSSESSED_BLADE: f32 = 115.0;
    /// Recall dash deals this percent of attack to each enemy in the rewind path.
    pub const RECALL: f32 = 160.0;
    /// How far back in time Recall returns the player, in seconds.
    pub const RECALL_REWIND_SECONDS: f32 = 1.;
    /// Interval between position samples used to reconstruct the rewind target.
    pub const RECALL_SAMPLE_INTERVAL_SECS: f32 =
        RECALL_REWIND_SECONDS / RECALL_HISTORY_CAPACITY as f32 + 0.1;
    /// Capacity of the player's recall position-history ring buffer.
    /// Sized to hold `RECALL_REWIND_SECONDS / RECALL_SAMPLE_INTERVAL_SECS` + a small
    /// safety margin so we always have a sample at age >= [`RECALL_REWIND_SECONDS`] once primed.
    pub const RECALL_HISTORY_CAPACITY: usize = 12;
    /// Minimum recorded samples required to cast (full buffer gives ~[`RECALL_REWIND_SECONDS`] rewind).
    pub const RECALL_MIN_SAMPLES: usize = 2;
    /// World speed (pixels per second) along the retrace polyline. Dash time is
    /// `path_length / RECALL_DASH_SPEED_PX_PER_SEC`, clamped by min/max below.
    pub const RECALL_DASH_SPEED_PX_PER_SEC: f32 = 720.0;
    /// Minimum dash duration so a very short retrace still feels snappy.
    pub const RECALL_DASH_DURATION_MIN_SECS: f32 = 0.06;
    /// Upper bound so a zig-zag path cannot lock movement for too long.
    pub const RECALL_DASH_DURATION_MAX_SECS: f32 = 0.45;
    /// Lifetime (seconds) of the line damage collider spawned along the rewind path.
    /// Set slightly longer than the dash duration so enemies along the line still
    /// register hits as the player sweeps across them.
    pub const RECALL_HITBOX_SECONDS: f32 = 0.22;
    /// Half-width (pixels) of the line damage collider; full width is 2x this.
    pub const RECALL_HITBOX_HALF_WIDTH: f32 = 12.0;
    /// Arc length (pixels) between lunge-style shadow tracers while Shadow Step
    /// retraces — spawned during the dash, not all at cast time.
    pub const RECALL_SHADOW_INTERVAL_ARC_PX: f32 = 24.0;
    /// Duration of the (unbreakable) stealth buff granted on Recall landing.
    pub const RECALL_LANDING_STEALTH_SECS: f32 = 1.;
    /// Added as [`crate::player::skills::RapidfireState::attack_speed_bonus`] multiplier base.
    pub const RAPIDFIRE_ATTACK_SPEED_BONUS_PERCENT: f32 = 80.0;

    #[inline]
    pub fn attack_damage_multiplier(percent_of_attack: f32) -> f32 {
        percent_of_attack * 0.01
    }
}

/// Ground flame from [ActiveSkill::FirePillar]: base duration before max-mana scaling.
pub const FIRE_RING_BASE_DURATION_SECS: f32 = 2.0;
/// Max mana at or below this value gives no duration bonus (only the base).
pub const FIRE_RING_MANA_BASELINE: f32 = 100.0;
/// Extra seconds per 10 max mana above [`FIRE_RING_MANA_BASELINE`].
pub const FIRE_RING_EXTRA_SECS_PER_10_MAX_MANA: f32 = 0.1;

/// Duration (seconds) the Fire Pillar fire ring stays active; scales with [`crate::attributes::MaxMana`].
pub fn fire_ring_duration_seconds(max_mana: i32) -> f32 {
    let excess = (max_mana as f32 - FIRE_RING_MANA_BASELINE).max(0.0);
    FIRE_RING_BASE_DURATION_SECS + (excess / 10.0) * FIRE_RING_EXTRA_SECS_PER_10_MAX_MANA
}

// --- ParrySpear (pull radius scales with [`crate::attributes::ProjectileSize`];
// gameplay in `melee_skills::handle_spear`) ---

pub mod parry_spear_scaling {
    pub const BASE_PULL_RADIUS_PX: f32 = 80.0;
    /// Extra pull radius per point of Size ([`crate::attributes::ProjectileSize`]).
    pub const PULL_RADIUS_PER_SIZE_PX: f32 = 0.5;
}

pub fn parry_spear_pull_radius_px(size: i32) -> f32 {
    parry_spear_scaling::BASE_PULL_RADIUS_PX
        + parry_spear_scaling::PULL_RADIUS_PER_SIZE_PX * size.max(0) as f32
}

// --- Arrow Volley (waves / timing; crit bonus in `combat/collisions`) ---

pub mod arrow_volley_scaling {
    pub const WAVE_COUNT: u32 = 3;
    pub const ARROWS_PER_WAVE: u32 = 3;
    pub const WAVE_INTERVAL_SECS: f32 = 0.4;
    pub const FIRST_WAVE_SPREAD_DEG: f32 = 15.0;
    pub const FOLLOWUP_WAVE_SPREAD_DEG: f32 = 15.0;
}

pub fn arrow_volley_total_arrows() -> u32 {
    arrow_volley_scaling::WAVE_COUNT * arrow_volley_scaling::ARROWS_PER_WAVE
}

// --- Fury (throw cadence scales purely off `BonusAttackSpeed`; see
// `skill_heirlooms::tick_fury_duration_and_throw`). The throw timer ticks at
// `bonus_attack_speed_mult` times real time, where `bonus_attack_speed_mult`
// is the player's `BonusAttackSpeed` multiplier (1.0 = no bonus, 2.0 = +100%).
// The weapon's base attack speed is intentionally NOT a factor so slow weapons
// don't gimp the skill. ---

pub const FURY_DURATION_SECS: f32 = 2.5;
pub const FURY_THROW_TIMER_EFFECTIVE_SECS: f32 = 0.3;
pub const FURY_THROW_SPEED_MIN_MULT: f32 = 1.0;
pub const FURY_THROW_SPEED_MAX_MULT: f32 = 10.0;

/// Bonus AS contributes at 2x rate so Fury scales noticeably faster than basic
/// attacks: `mult = 1 + 2 * (bonus_attack_speed_mult - 1)`. Baseline (no bonus)
/// stays at 1.0 → 8 kunai per cast.
pub const FURY_BONUS_AS_SCALE: f32 = 2.0;

pub fn fury_throw_speed_multiplier(bonus_attack_speed_mult: f32) -> f32 {
    let raw = 1.0 + FURY_BONUS_AS_SCALE * (bonus_attack_speed_mult - 1.0);
    raw.clamp(FURY_THROW_SPEED_MIN_MULT, FURY_THROW_SPEED_MAX_MULT)
}

/// Combine the player's flat [`AttackSpeed`] gear stat (percent integer) with
/// their multiplicative [`BonusAttackSpeed`] component into a single effective
/// attack-speed multiplier. Mirrors how `attack_speed_mod` is computed in
/// `ItemAttributes::update_attributes` (without dodge-crit / tiny-blessing
/// adjustments, which are transient).
///
/// Used as the `bonus_attack_speed_mult` input to Fury so it scales with
/// gear-based AS, not just blessing/potion AS.
pub fn effective_player_attack_speed_multiplier(
    attack_speed_stat: i32,
    bonus_attack_speed_mult: f32,
) -> f32 {
    effective_player_attack_speed_multiplier_with_major(
        attack_speed_stat,
        bonus_attack_speed_mult,
        1.0,
    )
}

/// Same as [`effective_player_attack_speed_multiplier`], with the major blessing AS multiplier.
pub fn effective_player_attack_speed_multiplier_with_major(
    attack_speed_stat: i32,
    bonus_attack_speed_mult: f32,
    major_attack_speed_mult: f32,
) -> f32 {
    (1.0 + attack_speed_stat as f32 / 100.0) * bonus_attack_speed_mult * major_attack_speed_mult
}

/// Approximate kunai spawned over one Fury (duration matches [`FURY_DURATION_SECS`]).
pub fn fury_estimated_kunai_per_cast(bonus_attack_speed_mult: f32) -> f32 {
    let m = fury_throw_speed_multiplier(bonus_attack_speed_mult);
    (m * FURY_DURATION_SECS / FURY_THROW_TIMER_EFFECTIVE_SECS)
        .floor()
        .max(1.)
}

impl ActiveSkill {
    /// Dodge / reposition skills shown on the movement cooldown bar.
    pub fn is_movement_skill(self) -> bool {
        matches!(
            self,
            ActiveSkill::SpinAttack
                | ActiveSkill::Teleport
                | ActiveSkill::Roll
                | ActiveSkill::SprintLunge
                | ActiveSkill::Buckshot
        )
    }

    /// Returns the base cooldown in seconds for this skill
    pub fn get_base_cooldown(&self) -> f32 {
        match self {
            ActiveSkill::Roll => 0.9, // Cooldown matches player_dash_cooldown duration
            ActiveSkill::Parry => 1.2,
            ActiveSkill::ParrySpear => 12.,
            ActiveSkill::Sprint => 8.0,
            ActiveSkill::SprintLunge => 1.2,
            ActiveSkill::Teleport => 1.2,
            ActiveSkill::Stealth => 13.0,
            ActiveSkill::Rapidfire => 12.0,
            ActiveSkill::FirePillar => 12.0,
            ActiveSkill::Heal => 20.0,
            ActiveSkill::Buckshot => 2.3,
            ActiveSkill::IceWall => 7.0,
            ActiveSkill::MeteorShower => 10.0,
            ActiveSkill::DruidTree => 11.0,
            ActiveSkill::Shout => 7.0,
            ActiveSkill::PiercingStar => 7.0,
            ActiveSkill::LaserBeam => 13.0,
            // New skills - placeholder cooldowns
            ActiveSkill::Lightning => 5.0,
            ActiveSkill::DaggerThrow => 7.0,
            ActiveSkill::DaggerSlash => 8.0,
            ActiveSkill::TripleThrow => 1.5,
            ActiveSkill::Fury => 13.0,
            ActiveSkill::Bomb => 5.5,
            ActiveSkill::SpinAttack => 2.5,
            ActiveSkill::ArrowVolley => 8.0,
            ActiveSkill::PossessedBlade => 6.0,
            ActiveSkill::Recall => 5.0,
        }
    }
}

// -----------------------------------------------------------------------------
// Active-skill state markers on the player.
//
// Every one of these toggles on the *single* player entity as that skill is
// cast/used. With table storage each insert/remove moves the player to a new
// archetype, and combinations of concurrently-active skills multiply the
// number of archetype variants the player cycles through every run (this was
// one of the biggest sources of empty archetypes the user was seeing).
//
// Stored as `SparseSet` so the player entity stays in one archetype regardless
// of which skill markers/state components are currently present; queries that
// use them as filters (`With<...>`) are still fast via the sparse set.
// -----------------------------------------------------------------------------
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct StealthState {
    pub duration: Timer,
    /// When true, player-issued attacks do NOT cancel this stealth. Used by
    /// the Recall skill's landing buff so the player can attack out of it.
    pub unbreakable: bool,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct RapidfireState {
    pub duration: Timer,
    pub attack_speed_bonus: f32,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct FirePillarState {
    pub hit_clear_timer: Timer,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct LaserBeamState {
    pub hit_clear_timer: Timer,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct HealSkillState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct BuckshotSkillState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct IceWallSkillState;
/// Tracks how many meteors the next MeteorShower cast spawns. Grows by 1 every
/// [`METEOR_SHOWER_CASTS_PER_GROWTH`] casts. Stored `SparseSet` because it's
/// only present on the player while the MeteorShower skill is equipped.
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct MeteorShowerSkillState {
    pub meteor_count: u32,
    /// Number of casts accumulated toward the next +1 meteor.
    pub casts: u32,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct DruidTreeSkillState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct ShoutSkillState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PiercingStarSkillState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct LightningState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct DaggerThrowState;

/// Tracks kills since last dagger throw cast (max 10).
/// Kills from dagger throw projectiles themselves don't count.
/// Stored `SparseSet` because it's only present on the player while the
/// DaggerThrow skill is equipped.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct DaggerThrowKillTracker {
    pub kill_count: i32,
}

/// Tracks the last projectile that hit an enemy (used to exclude dagger throw
/// kills). Stored `SparseSet` because it's only present on the player while
/// the relevant skill is equipped.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct LastHitProjectile {
    pub projectile: Option<Projectile>,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct SlashState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct TripleThrowState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct FuryState {
    pub duration: Timer,
    pub throw_timer: Timer,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct BombState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct SpinAttackState;
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct ArrowVolleyState {
    pub waves_remaining: u32,
    pub wave_timer: Timer,
}
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PossessedBladeSkillState;

/// Transient phasing buff on the player that disables collisions with mobs.
/// Added on specific skill casts and removed when its timer expires — stored
/// `SparseSet` to keep the player in one archetype while the effect toggles.
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PhasingThroughEnemies {
    pub timer: Timer,
}

impl PhasingThroughEnemies {
    pub fn new(duration: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
        }
    }
}

impl ActiveSkill {
    /// True for skills that target a specific ground/world point (placed at
    /// `cursor.world_coords` when cast) rather than firing in a direction or on self. Used to
    /// gate the keyboard/gamepad hold-to-aim-and-release flow (see `aim.rs` /
    /// `gamepad_input.rs`) — mouse play always aims these instantly at the live cursor
    /// position and is unaffected either way.
    pub fn is_ground_targeted(&self) -> bool {
        matches!(
            self,
            ActiveSkill::FirePillar
                | ActiveSkill::IceWall
                | ActiveSkill::DruidTree
                | ActiveSkill::Bomb
        )
    }

    pub fn get_title(&self) -> String {
        match self {
            ActiveSkill::Roll => "Roll".to_string(),
            ActiveSkill::Parry => "Parry".to_string(),
            ActiveSkill::ParrySpear => "Gravitational Spear".to_string(),
            ActiveSkill::Sprint => "Sprint".to_string(),
            ActiveSkill::SprintLunge => "Lunge".to_string(),
            ActiveSkill::Teleport => "Teleport".to_string(),
            ActiveSkill::Stealth => "Stealth".to_string(),
            ActiveSkill::Rapidfire => "Rapidfire".to_string(),
            ActiveSkill::FirePillar => "Fire Pillar".to_string(),
            ActiveSkill::Heal => "Heal".to_string(),
            ActiveSkill::Buckshot => "Buckshot".to_string(),
            ActiveSkill::IceWall => "Ice Wall".to_string(),
            ActiveSkill::MeteorShower => "Meteor Shower".to_string(),
            ActiveSkill::DruidTree => "Druid Tree".to_string(),
            ActiveSkill::Shout => "Shout".to_string(),
            ActiveSkill::PiercingStar => "Piercing Star".to_string(),
            ActiveSkill::LaserBeam => "Laser Beam".to_string(),
            // New skills
            ActiveSkill::Lightning => "Lightning".to_string(),
            ActiveSkill::DaggerThrow => "Dagger Throw".to_string(),
            ActiveSkill::DaggerSlash => "Slash".to_string(),
            ActiveSkill::TripleThrow => "Triple Throw".to_string(),
            ActiveSkill::Fury => "Fury".to_string(),
            ActiveSkill::Bomb => "Bomb".to_string(),
            ActiveSkill::SpinAttack => "Spin Attack".to_string(),
            ActiveSkill::ArrowVolley => "Arrow Volley".to_string(),
            ActiveSkill::PossessedBlade => "Possessed Blade".to_string(),
            ActiveSkill::Recall => "Shadow Step".to_string(),
        }
    }

    pub fn get_desc(
        &self,
        skill_power: f32,
        max_mana: i32,
        max_health: i32,
        bonus_attack_speed_mult: f32,
        crit_chance: i32,
        speed: i32,
        size: i32,
        meteor_count: u32,
    ) -> Vec<String> {
        use active_skill_scaling::{
            dagger_slash_total_slashes, ARROW_VOLLEY, BOMB, BUCKSHOT_PELLET, DAGGER_SLASH,
            DAGGER_THROW, FIRE_PILLAR, FURY, HEAL_MAX_HEALTH_PERCENT, ICE_WALL, LASER_BEAM,
            LIGHTNING, METEOR_SHOWER, PARRY_SPEAR, PIERCING_STAR, POSSESSED_BLADE,
            RAPIDFIRE_ATTACK_SPEED_BONUS_PERCENT, RECALL, RECALL_REWIND_SECONDS, SHOUT,
            SPIN_ATTACK, SPRINT_LUNGE, TELEPORT_SHOCK_ATTACK_PERCENT, TRIPLE_THROW,
        };

        match self {
            ActiveSkill::Roll => vec![
                "Roll to dodge attacks. You are ".to_string(),
                "invulnerable while rolling.".to_string(),
            ],
            ActiveSkill::SpinAttack => vec![
                "Dash forwards and spin your sword,".to_string(),
                format!(
                    "dealing {:.1}% damage around you.",
                    skill_power * SPIN_ATTACK
                ),
            ],
            ActiveSkill::Shout => vec![
                "SCREAM, releasing a shockwave".to_string(),
                format!("around you, dealing {:.1}% damage.", skill_power * SHOUT),
                "Knocks back enemies hit.".to_string(),
            ],
            ActiveSkill::Heal => vec![
                format!(
                    "Heal yourself for {:.1}% of your max",
                    skill_power * HEAL_MAX_HEALTH_PERCENT
                ),
                "health.".to_string(),
            ],
            ActiveSkill::Parry => vec![
                "Active: Time successfully".to_string(),
                "to Parry attacks ignore".to_string(),
                "damage and stunning.".to_string(),
            ],
            ActiveSkill::ParrySpear => {
                let pull_px = parry_spear_pull_radius_px(size);
                vec![
                    format!(
                        "Pulls enemies from up to {} tiles away",
                        (pull_px / 16.).round()
                    ),
                    format!(
                        "towards you, dealing {:.1}% damage.",
                        skill_power * PARRY_SPEAR
                    ),
                    "Pull distance scales with Size.".to_string(),
                ]
            }
            ActiveSkill::Sprint => vec![
                "You are imbued with a burst of speed.".to_string(),
                "Gain 60% speed temporarily.".to_string(),
            ],
            ActiveSkill::SprintLunge => vec![
                "Lunge through enemies in a line dealing".to_string(),
                format!(
                    "{:.1}% damage. You are invulnerable",
                    skill_power * SPRINT_LUNGE
                ),
                "during the attack.".to_string(),
            ],
            ActiveSkill::DaggerThrow => vec![
                "Throw a dagger at a nearby enemy".to_string(),
                format!(
                    "dealing {:.1}% damage. Throw one more",
                    skill_power * DAGGER_THROW
                ),
                "per mob killed since the last cast.".to_string(),
            ],
            ActiveSkill::DaggerSlash => {
                let slashes = dagger_slash_total_slashes(speed);
                vec![
                    format!("Rapidly slash {} times, each dealing", slashes),
                    format!(
                        "{:.1}% damage. The number of slashes",
                        skill_power * DAGGER_SLASH
                    ),
                    "scales with speed.".to_string(),
                ]
            }
            ActiveSkill::Stealth => vec![
                "Dissapear for a short duration".to_string(),
                "Attacks used during Stealth will".to_string(),
                "always crit but end Stealth early.".to_string(),
            ],
            ActiveSkill::Teleport => vec![
                format!(
                    "Teleport forwards, dealing {:.1}%",
                    skill_power * TELEPORT_SHOCK_ATTACK_PERCENT
                ),
                "damage to enemies you pass through.".to_string(),
                "Can pass through objects or cross".to_string(),
                "water.".to_string(),
                // "through.".to_string(),
            ],
            ActiveSkill::Lightning => vec![
                "Call down lighning on 3 nearby".to_string(),
                format!(
                    "enemies, dealing {:.1}% damage to each.",
                    skill_power * LIGHTNING
                ),
            ],
            ActiveSkill::FirePillar => {
                let ring_secs = fire_ring_duration_seconds(max_mana);
                vec![
                    "Scorch the earth at target area,".to_string(),
                    format!(
                        "dealing {:.1}% damage continuously",
                        skill_power * FIRE_PILLAR
                    ),
                    format!("for {:.1}s. Duration scales with", ring_secs),
                    "Max Mana.".to_string(),
                ]
            }
            ActiveSkill::IceWall => {
                vec![
                    "Summon an ice pillar at target area".to_string(),
                    format!("dealing {:.1}% damage.", skill_power * ICE_WALL),
                ]
            }
            ActiveSkill::MeteorShower => {
                vec![
                    format!("Summon {} meteors, each dealing", meteor_count),
                    format!(
                        "{:.1}% damage. Grows by +1 more",
                        skill_power * METEOR_SHOWER
                    ),
                    "Meteor every 5 casts.".to_string(),
                ]
            }
            ActiveSkill::Buckshot => vec![
                format!(
                    "Fire a shotgun dealing 5x {:.1}%",
                    skill_power * BUCKSHOT_PELLET
                ),
                " damage spread in a cone. Knocks".to_string(),
                "you back a moderate amount.".to_string(),
            ],
            ActiveSkill::Bomb => vec![
                "Throw a bomb at target area that".to_string(),
                "explodes on impact, dealing".to_string(),
                format!("{:.1}% damage and applying Frail.", skill_power * BOMB),
            ],
            ActiveSkill::DruidTree => vec![
                "Place a tree at target area ".to_string(),
                "that taunts enemies towards".to_string(),
                "it for a short duration.".to_string(),
            ],
            ActiveSkill::Rapidfire => vec![
                "Concentrate deeply. Enemies around".to_string(),
                "you move 50% slower briefly.".to_string(),
                format!(
                    "Gain {:.1}% attack speed for the",
                    skill_power * RAPIDFIRE_ATTACK_SPEED_BONUS_PERCENT
                ),
                "duration.".to_string(),
            ],
            ActiveSkill::PiercingStar => vec![
                "Throw a large throwing star that".to_string(),
                "travels in a line then returns".to_string(),
                format!(
                    "back to you, dealing {:.1}% damage.",
                    skill_power * PIERCING_STAR
                ),
            ],
            ActiveSkill::TripleThrow => vec![
                "Throw three small throwing stars".to_string(),
                "in a cone shape in front, dealing".to_string(),
                format!("{:.1}% damage each.", skill_power * TRIPLE_THROW),
            ],
            ActiveSkill::Fury => {
                let kunai_n = fury_estimated_kunai_per_cast(bonus_attack_speed_mult);
                vec![
                    format!("Throw {:.0} kunai rapidly at enemies", kunai_n),
                    format!("around you, dealing {:.1}% damage.", skill_power * FURY),
                    "Kunai count scales with attack speed.".to_string(),
                ]
            }
            ActiveSkill::LaserBeam => vec![
                "Channel a powerful laser beam that".to_string(),
                format!("deals {:.1}% damage rapidly to", skill_power * LASER_BEAM),
                "enemies in front.".to_string(),
            ],
            ActiveSkill::ArrowVolley => {
                let total = arrow_volley_total_arrows();
                vec![
                    "Send out waves of arrows, dealing".to_string(),
                    format!(
                        "{}x {:.1}% damage. Gains +{}% Crit ",
                        total,
                        skill_power * ARROW_VOLLEY,
                        crit_chance
                    ),
                    "Damage, scaling with Crit Chance.".to_string(),
                ]
            }
            ActiveSkill::PossessedBlade => vec![
                "Throw a blade that returns back to".to_string(),
                format!(
                    "you. Deals {:.1}% damage, and triggers",
                    skill_power * POSSESSED_BLADE
                ),
                "lifesteal on kill, up to 3 times.".to_string(),
            ],
            ActiveSkill::Recall => vec![
                format!(
                    "Retrace your steps from the last {:.1}s, ",
                    RECALL_REWIND_SECONDS
                ),
                format!(
                    "slicing enemies for {:.1}% damage. Gain",
                    skill_power * RECALL
                ),
                "stealth briefly afterwards.".to_string(),
            ],
        }
    }
    //TODO: Grav spear, teleport, and lunge skills rely on this, we should remove the reliance
    // and get rid of this function
    pub fn add_skill_components(&self, entity: Entity, commands: &mut Commands) {
        match self {
            ActiveSkill::Sprint => {
                commands
                    .entity(entity)
                    .insert(crate::player::rogue_skills::SprintState {
                        startup_timer: Timer::from_seconds(0.0, TimerMode::Once),
                        sprint_duration_timer: Timer::from_seconds(2.5, TimerMode::Once),
                        speed_bonus: 1.6,
                    });
            }
            ActiveSkill::SprintLunge => {
                commands
                    .entity(entity)
                    .insert(crate::player::rogue_skills::LungeState {
                        lunge_duration: Timer::from_seconds(0.64, TimerMode::Once),
                        lunge_speed: 7.5,
                    });
            }
            ActiveSkill::Teleport => {
                commands
                    .entity(entity)
                    .insert(crate::player::mage_skills::TeleportState {
                        just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                        timer: Timer::from_seconds(0.06, TimerMode::Once),
                    });
            }
            ActiveSkill::Parry => {
                commands
                    .entity(entity)
                    .insert(crate::player::melee_skills::ParryState {
                        parry_timer: Timer::from_seconds(0.7, TimerMode::Once),
                        cooldown_timer: Timer::from_seconds(1.2, TimerMode::Once)
                            .tick(Duration::from_secs(99))
                            .clone(),
                        success: false,
                        active: false,
                    });
            }
            ActiveSkill::ParrySpear => {
                commands
                    .entity(entity)
                    .insert(crate::player::melee_skills::SpearState {
                        spear_timer: Timer::from_seconds(0.5, TimerMode::Once),
                    });
            }
            ActiveSkill::Stealth => {
                commands.entity(entity).insert(StealthState {
                    duration: Timer::from_seconds(2.0, TimerMode::Once),
                    unbreakable: false,
                });
            }
            ActiveSkill::Rapidfire => {
                // let cooldown = ActiveSkill::Rapidfire.get_base_cooldown();
                // // Pre-tick the duration timer so it starts finished (buff not active)
                // let mut duration = Timer::from_seconds(3.0, TimerMode::Once);
                // duration.tick(Duration::from_secs(99));
                // commands.entity(entity).insert(RapidfireState {
                //     duration,
                //     cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                //         .tick(Duration::from_secs(99))
                //         .clone(),
                //     attack_speed_bonus: 0.8,
                // });
            }
            ActiveSkill::FirePillar => {
                commands.entity(entity).insert(FirePillarState {
                    hit_clear_timer: Timer::from_seconds(1.15, TimerMode::Repeating),
                });
            }
            ActiveSkill::Heal => {
                commands.entity(entity).insert(HealSkillState);
            }
            ActiveSkill::Buckshot => {
                commands.entity(entity).insert(BuckshotSkillState);
            }
            ActiveSkill::IceWall => {
                commands.entity(entity).insert(IceWallSkillState);
            }
            ActiveSkill::MeteorShower => {
                commands.entity(entity).insert(MeteorShowerSkillState {
                    meteor_count: active_skill_scaling::METEOR_SHOWER_BASE_COUNT,
                    casts: 0,
                });
            }
            ActiveSkill::DruidTree => {
                commands.entity(entity).insert(DruidTreeSkillState);
            }
            ActiveSkill::Shout => {
                commands.entity(entity).insert(ShoutSkillState);
            }
            ActiveSkill::PiercingStar => {
                commands.entity(entity).insert(PiercingStarSkillState);
            }
            ActiveSkill::LaserBeam => {
                commands.entity(entity).insert(LaserBeamState {
                    hit_clear_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                });
            }
            ActiveSkill::Recall => {
                commands
                    .entity(entity)
                    .insert(crate::player::rogue_skills::PositionHistory::new());
            }
            // New skills - placeholder implementations (default to Roll behavior for now)
            ActiveSkill::Roll
            | ActiveSkill::Lightning
            | ActiveSkill::DaggerThrow
            | ActiveSkill::DaggerSlash
            | ActiveSkill::TripleThrow
            | ActiveSkill::Fury
            | ActiveSkill::SpinAttack
            | ActiveSkill::Bomb
            | ActiveSkill::ArrowVolley
            | ActiveSkill::PossessedBlade => {}
        }
    }

    pub fn get_ui_element(&self, rarity: HeirloomRarity) -> UIElement {
        match rarity {
            HeirloomRarity::Common => UIElement::SkillTooltipBanner,
            HeirloomRarity::Uncommon => UIElement::SkillTooltipBanner,
            HeirloomRarity::Rare => UIElement::SkillTooltipBanner,
            HeirloomRarity::Legendary => UIElement::SkillTooltipBanner,
        }
    }

    pub fn get_ui_element_hover(&self, rarity: HeirloomRarity) -> UIElement {
        match rarity {
            HeirloomRarity::Common => UIElement::SkillTooltipBannerHover,
            HeirloomRarity::Uncommon => UIElement::SkillTooltipBannerHover,
            HeirloomRarity::Rare => UIElement::SkillTooltipBannerHover,
            HeirloomRarity::Legendary => UIElement::SkillTooltipBannerHover,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Default, Debug, Serialize, EnumIter, Display, Deserialize)]
pub enum Heirloom {
    // Passives
    #[default]
    None,

    CritChance,       //tusk
    CritDamage,       //flint
    SkillCDReduction, // placeholder
    LoadedDice,       // increases luck by 7
    Health,           //red mushroom
    Mana,             //blue mushroom
    Shield,           // CD
    Thorns,           //bushling scale
    Lifesteal,        // rose
    Speed,            // feather
    AttackSpeed,      // soda can
    DodgeChance,      // leather
    Defence,          // coal
    Attack,           // anvil
    Gigantify,        // sappling
    Chest,            // loot bag
    XPGain,           //memory chip

    DaggerCombo,         // dagger
    HPRegen,             // red book
    HPRegenCooldown,     // red phone
    MPRegen,             // blue book
    MPRegenCooldown,     // blue phone
    OnHitEcho,           // fossile thing
    Knockback,           // horseshoe
    DiscountMP,          // pen quill
    MinusOneDamageOnHit, // shield

    // Weapon Upgrades
    ChanceToProcExtraAttack, // sceptor
    IncreaseProjectileCount, //whip

    IceStaffAoE,   // frozen tear
    BowArrowSpeed, // yarn

    //magic
    TeleportStatusDMG, //NOT USED

    FrozenAoE,     // lantern
    IceStaffFloor, // weird ice stick thing
    FrozenCrit,    // diamond
    MPBarDMG,      // purple card
    MPBarCrit,     // brown card
    FrozenMPRegen, // mirror

    //rogue
    DodgeCrit,      // telescope
    PoisonDuration, // belt
    PoisonStrength, //ring
    ViralVenum,     //poison sceptor

    //melee
    HealEcho,                   // chalice
    HealSummons,                // 20% on heal trigger all summons once (ant farm, stone orbit)
    FullStomach,                //jam
    SkillEcho,                  // placeholder
    ReinforcedArmor,            // Scale
    SkillPower,                 // placeholder - increases skill effectiveness
    CritSkillCooldownReduction, // reduces class skill cooldown on crit

    // On-Attack Triggers
    WaveAttack,  // hero sword
    CherryBomb,  // cherry bomb lob on attack
    FrailStacks, // skull
    SlowStacks,  // sea shell

    AntFarm,    // ant terrarium
    StoneTooth, // orbiting stone
    SummonRing, // piercing ring that travels and bounces
    Reaper,     // soul harvest

    // Chaos
    ChaosBoost,          // chaos totem item
    PoisonStacks,        // grandma's recipe
    LethalBlow,          // red purple mushroom
    SkillChargeIncrease, // placeholder
    CreditCard,          // gain coin on every skill use

    TeleportShock,
    TeleportCooldown,
    TeleportCount,
    TeleportManaRegen,

    SprintFaster,
    SprintLungeDamage,
    SprintKillReset,

    ParryHPRegen,
    ParryDeflectProj,
    ParryKnockback, // needs art prompt/art
    ParryEcho,

    // New scalable heirlooms
    MaxHPHunt,      // Every 3 kills grants +1 max hp
    SkillPowerHunt, // 3% chance on skill use to gain +1 skill power
    MaxHPDamage,    // +10% dmg per 100 max hp
    GoldIntoDamage, // +1% damage per 10 coins
    DeathDefiance,  // Survive death, freeze all enemies
    RegenLifesteal, // -5 hp regen, +5% lifesteal
    StandStill,     // Standing still increases damage
    ThornArmor,     // 20% thorns per 10 defence, +10 defence
    LifestealCoins, // Lifesteal gives coins, +5% lifesteal

    // Wave 2 heirlooms
    CoinHeal,          // Picking up coins heals
    CrateBreakDamage,  // Breaking crates gives damage boost (tracker needed)
    TomeDoubleUpgrade, // Upgrade tomes work twice
    CritHeal,          // Crits heal
    LowHPDamage,       // More damage at low HP
    ChaosStats,        // +2 chaos, +10 to many stats
    ManaOrbs,
    ManaOrbAttack,    // Mana regen shoots mana orb projectiles
    ItemPickupRadius, // Increases pickup radius by 25%
    GravityScales,    // Converts 25% of pickup range into size per stack
    MagnetPull,       // Periodically pulls all item drops to player

    // Thorns build heirlooms
    ThornsSpikes,    // Taking damage shoots out spikes (from blessings)
    ThornsOnDamage,  // Gain 1% thorns each time you take damage
    ThornsLifesteal, // Thorns damage has +25% chance to lifesteal

    // Lightning strikes archetype
    CoinLightning,      // Picking up a coin causes a lightning strike
    KillLightning,      // Killing an enemy has a 1% chance to spawn lightning
    ManaRegenLightning, // Mana regen has a 10% chance per stack to trigger lightning
    ManaRegenPoison,    // Every 100 mana regen applies poison to all enemies
    SkillManaRegen,     // Using a skill has a 15% chance to trigger mana regen
    /// Chance on each non-heirloom damage hit to restore 1 MP (1% + 1% per stack).
    DamageDealtMp,
    /// Additive multiplier to base Mana Orb drop chance (1 + stacks: 2x, 3x, ...).
    ManaOrbDropMult,
    /// Every 150 player damage dealt fires a homing energy ball (not from energy balls).
    EnergyBallBarrage,
    /// Skill hits spawn a small explosion at the target when the player has enough mana.
    SkillExplosion,
    /// When an enemy drops a coin from its loot table, 10% chance per stack to drop an extra coin.
    GoldenTooth,
}

pub enum HeirloomTrait {
    Ice,     // 6
    Water,   // 6
    Echo,    // 3
    Magic,   // 4
    Healing, // 12
    Poison,  // 4
    Thorns,  // 2
}

impl Heirloom {
    pub fn get_mana_cost(&self) -> i32 {
        match self {
            Heirloom::OnHitEcho => 3,
            Heirloom::ChanceToProcExtraAttack => 5,
            // Heirloom::IncreaseProjectileCount => 5,
            Heirloom::IceStaffAoE => 7,
            Heirloom::FrozenAoE => 7,
            Heirloom::IceStaffFloor => 5,
            Heirloom::ViralVenum => 2,
            Heirloom::HealEcho => 7,
            Heirloom::HealSummons => 7,
            Heirloom::SkillEcho => 3,
            Heirloom::WaveAttack => 5,
            Heirloom::CherryBomb => 3,
            Heirloom::AntFarm => 2,
            Heirloom::StoneTooth => 5,
            Heirloom::SummonRing => 5,
            Heirloom::Reaper => 1,
            Heirloom::CoinLightning => 5,
            Heirloom::KillLightning => 5,
            Heirloom::ManaRegenLightning => 5,
            Heirloom::ManaOrbAttack => 2,
            Heirloom::SkillExplosion => 3,
            _ => 0,
        }
    }

    /// Mana spent when a skill hit triggers an Impact Rune explosion.
    pub fn skill_explosion_mana_cost() -> i32 {
        Heirloom::SkillExplosion.get_mana_cost()
    }

    /// Explosion damage as a fraction of the triggering skill hit (15% per copy).
    pub fn skill_explosion_damage_fraction(stacks: i32) -> f32 {
        stacks.max(1) as f32 * 0.15
    }
    /// Minimum player level required before this heirloom is allowed to appear in any
    /// selection (level-ups, shrines, chests, essence shop, blessings). `0` means no gate.
    pub fn min_player_level(&self) -> u8 {
        match self {
            Heirloom::MPRegen => 7,
            Heirloom::ManaOrbs => 8,
            _ => 0,
        }
    }
    pub fn get_title(&self) -> String {
        match self {
            Heirloom::None => "None".to_string(),
            Heirloom::CritChance => "Tusk".to_string(),
            Heirloom::CritDamage => "Flint".to_string(),
            Heirloom::SkillCDReduction => "Stanley".to_string(),
            Heirloom::LoadedDice => "Loaded Dice".to_string(),
            Heirloom::Health => "Red Mushroom".to_string(),
            Heirloom::Mana => "Blue Mushroom".to_string(),
            Heirloom::Shield => "CDz".to_string(),
            Heirloom::Speed => "Feather".to_string(),
            Heirloom::Thorns => "Bushling Scales".to_string(),
            Heirloom::Lifesteal => "Rose".to_string(),
            Heirloom::AttackSpeed => "Red Soda".to_string(),
            Heirloom::Defence => "Coal".to_string(),
            Heirloom::Chest => "Loot Bag".to_string(),
            Heirloom::Attack => "Anvil ".to_string(),
            Heirloom::DodgeChance => "Leather".to_string(),
            Heirloom::WaveAttack => "Hero Sword".to_string(),
            Heirloom::CherryBomb => "Cherry Bomb".to_string(),
            Heirloom::FrailStacks => "Skull".to_string(),
            Heirloom::SlowStacks => "Sea Shell".to_string(),
            Heirloom::AntFarm => "Ant Farm".to_string(),
            Heirloom::StoneTooth => "Boulder".to_string(),
            Heirloom::SummonRing => "Piercing Ring".to_string(),
            Heirloom::Reaper => "Reaper".to_string(),
            Heirloom::PoisonStacks => "Grandma's Recipe".to_string(),
            Heirloom::LethalBlow => "Deadly Mushroom".to_string(),
            Heirloom::SkillEcho => "Ancient Fossil".to_string(),
            Heirloom::SkillChargeIncrease => "Paintbrush".to_string(),
            Heirloom::SkillPower => "Blue Scroll".to_string(),
            Heirloom::CritSkillCooldownReduction => "Bob's Bell".to_string(),
            Heirloom::TeleportShock => "Shock Step".to_string(),
            Heirloom::TeleportCooldown => "Teleport Faster!".to_string(),
            Heirloom::TeleportCount => "Multi-port".to_string(),
            Heirloom::TeleportManaRegen => "Infused Cast".to_string(),
            Heirloom::TeleportStatusDMG => "Shock Mastery".to_string(),
            Heirloom::ChanceToProcExtraAttack => "Sceptor".to_string(),
            Heirloom::IncreaseProjectileCount => "Whip".to_string(),
            Heirloom::BowArrowSpeed => "Thread".to_string(),
            Heirloom::Gigantify => "Sappling".to_string(),
            Heirloom::XPGain => "Microchip".to_string(),
            Heirloom::CreditCard => "Credit Card".to_string(),

            Heirloom::IceStaffAoE => "Frozen Tear".to_string(),
            Heirloom::SprintFaster => "Faster Sprint".to_string(),
            Heirloom::SprintLungeDamage => "Lunge Mastery".to_string(),
            Heirloom::SprintKillReset => "Kill Reset".to_string(),
            Heirloom::ParryHPRegen => "Rejuvenating Parry".to_string(),
            Heirloom::ParryDeflectProj => "Parry Deflect".to_string(),
            Heirloom::ParryKnockback => "Shield Bash".to_string(),
            Heirloom::ParryEcho => "Parry Echo".to_string(),
            Heirloom::DaggerCombo => "Lost Dagger".to_string(),
            Heirloom::HPRegen => "Red Book ".to_string(),
            Heirloom::HPRegenCooldown => "Red Phone".to_string(),
            Heirloom::MPRegenCooldown => "Blue Phone".to_string(),
            Heirloom::MPRegen => "Blue Book".to_string(),
            Heirloom::OnHitEcho => "Dragon Eye".to_string(),
            Heirloom::Knockback => "Horse Shoe".to_string(),
            Heirloom::DiscountMP => "Ink & Quill".to_string(),
            Heirloom::MinusOneDamageOnHit => "Holy Shield".to_string(),

            Heirloom::FrozenAoE => "Ice Lantern".to_string(),
            Heirloom::IceStaffFloor => "Ice Wand".to_string(),
            Heirloom::FrozenCrit => "Diamond".to_string(),
            Heirloom::MPBarDMG => "Purple Card".to_string(),
            Heirloom::MPBarCrit => "Brown Card".to_string(),
            Heirloom::FrozenMPRegen => "Mirror".to_string(),
            Heirloom::DodgeCrit => "Telescope".to_string(),
            Heirloom::PoisonDuration => "Cursed Belt".to_string(),
            Heirloom::PoisonStrength => "Green Ring".to_string(),
            Heirloom::ViralVenum => "Poison Sceptor".to_string(),
            Heirloom::HealEcho => "Chalice".to_string(),
            Heirloom::HealSummons => "Summoning Wand".to_string(),
            Heirloom::FullStomach => "Jam".to_string(),

            Heirloom::ReinforcedArmor => "Scales".to_string(),
            Heirloom::ChaosBoost => "Cursed Mask".to_string(),

            // New heirlooms
            Heirloom::MaxHPHunt => "Ripe Tomato".to_string(),
            Heirloom::SkillPowerHunt => "Arcane Tome".to_string(),
            Heirloom::MaxHPDamage => "Crusader Shield".to_string(),
            Heirloom::GoldIntoDamage => "Red Envelope".to_string(),
            Heirloom::DeathDefiance => "Cooked Cross".to_string(),
            Heirloom::RegenLifesteal => "Dark Blade".to_string(),
            Heirloom::StandStill => "Tent".to_string(),
            Heirloom::ThornArmor => "Spikey Shield".to_string(),
            Heirloom::LifestealCoins => "Cursed Crown".to_string(),

            // Wave 2 heirlooms
            Heirloom::CoinHeal => "Lucky Penny".to_string(),
            Heirloom::CrateBreakDamage => "Sturdy Crate".to_string(),
            Heirloom::TomeDoubleUpgrade => "Ancient Tome".to_string(),
            Heirloom::CritHeal => "Vampiric Ring".to_string(),
            Heirloom::LowHPDamage => "Beer!".to_string(),
            Heirloom::ChaosStats => "Chaotic Candle".to_string(),
            Heirloom::ManaOrbs => "Mana Dust".to_string(),
            Heirloom::ManaOrbAttack => "Wizard Hat".to_string(),
            Heirloom::EnergyBallBarrage => "Underworld's Hat".to_string(),
            Heirloom::SkillExplosion => "Impact Rune".to_string(),
            Heirloom::ItemPickupRadius => "Magnet".to_string(),
            Heirloom::GravityScales => "Gravity Scales".to_string(),
            Heirloom::MagnetPull => "Gravitation Tome".to_string(),

            // Thorns build heirlooms
            Heirloom::ThornsSpikes => "Spiked Club".to_string(),
            Heirloom::ThornsOnDamage => "Spiked Helmet".to_string(),
            Heirloom::ThornsLifesteal => "Spiked Ring".to_string(),

            // Lightning strikes archetype
            Heirloom::CoinLightning => "Lightning Belt".to_string(),
            Heirloom::KillLightning => "Lightning Ring".to_string(),
            Heirloom::ManaRegenLightning => "Lightning Cape".to_string(),
            Heirloom::ManaRegenPoison => "Toxic Tome".to_string(),
            Heirloom::SkillManaRegen => "Brown Card".to_string(),
            Heirloom::DamageDealtMp => "Blue Card".to_string(),
            Heirloom::ManaOrbDropMult => "Purple Card".to_string(),
            Heirloom::GoldenTooth => "Golden Tooth".to_string(),
        }
    }

    /// Side glossary info boxes for this heirloom (explicit per-variant; no desc parsing).
    pub fn tooltip_definitions(&self) -> &'static [TooltipDefinition] {
        use TooltipDefinition as D;
        match self {
            // Self::CritChance
            // | Self::CritHeal
            // | Self::CritSkillCooldownReduction
            // | Self::MPBarCrit => &[D::CritChance],
            // Self::CritDamage => &[D::CritDamage],
            // Self::Health | Self::HPRegen | Self::MaxHPHunt => &[D::Health],
            Self::Mana
            | Self::ManaOrbs
            | Self::MPBarCrit
            | Self::DamageDealtMp
            | Self::ManaOrbDropMult => &[D::Mana],
            Self::MPRegen | Self::MPRegenCooldown | Self::ManaOrbAttack | Self::MPBarDMG => {
                &[D::ManaRegen]
            }
            Self::FrozenMPRegen => &[D::ManaRegen, D::FreezeChance],
            Self::TeleportManaRegen => &[D::ManaRegen],
            Self::Thorns | Self::ThornsSpikes | Self::ThornsOnDamage | Self::ThornArmor => {
                &[D::Thorns]
            }
            Self::FrailStacks => &[D::Frail],
            Self::AntFarm | Self::StoneTooth | Self::SummonRing | Self::HealSummons => &[D::Summon],
            Self::ThornsLifesteal => &[D::Thorns, D::Lifesteal],
            Self::Lifesteal | Self::RegenLifesteal | Self::LifestealCoins => &[D::Lifesteal],
            // Self::Speed => &[D::Speed],
            Self::AttackSpeed => &[D::AttackSpeed],
            Self::DodgeChance => &[D::Dodge],
            Self::DodgeCrit => &[D::Dodge, D::AttackSpeed],
            Self::Defence | Self::ReinforcedArmor => &[D::Defence],
            // Self::Attack
            // | Self::MaxHPDamage
            // | Self::GoldIntoDamage
            // | Self::LowHPDamage
            // | Self::CrateBreakDamage
            // | Self::StandStill
            // | Self::MPBarDMG => &[D::Attack],
            Self::SkillPower | Self::SkillPowerHunt => &[D::SkillPower, D::Skills],
            // Self::ItemPickupRadius | Self::GravityScales => &[D::PickupRange],
            Self::OnHitEcho | Self::HealEcho | Self::ParryEcho => &[D::Echo],
            Self::Attack
            | Self::GoldIntoDamage
            | Self::CrateBreakDamage
            | Self::LowHPDamage
            | Self::StandStill
            | Self::MaxHPDamage => &[D::Attack],
            Self::ChaosBoost => &[D::Chaos],
            Self::CoinLightning | Self::KillLightning => &[D::Lightning],
            Self::ManaRegenLightning => &[D::Lightning, D::ManaRegen],

            Self::IceStaffAoE => &[D::IceExplosion, D::Weapons],
            Self::FrozenAoE => &[D::IceExplosion, D::FreezeChance],
            Self::SkillExplosion => &[D::Skills],
            Self::SlowStacks | Self::FrozenCrit => &[D::FreezeChance],

            Self::PoisonStacks
            | Self::ViralVenum
            | Self::ManaRegenPoison
            | Self::PoisonDuration
            | Self::PoisonStrength => &[D::Poison],

            Self::ChaosStats => &[D::Attack, D::Chaos, D::Mana, D::Defence, D::Dodge],
            Self::Gigantify | Self::GravityScales => &[D::Size],
            Self::LoadedDice => &[D::Luck],
            Self::CherryBomb | Self::WaveAttack => &[D::Weapons],
            Self::SkillCDReduction | Self::SkillChargeIncrease => &[D::Skills],
            Self::SkillManaRegen => &[D::Skills, D::ManaRegen],
            Self::SkillEcho => &[D::Skills, D::Echo],
            _ => &[],
        }
    }

    fn classify_desc_line(line: String) -> HeirloomDescLine {
        if line.is_empty() {
            return HeirloomDescLine::blank();
        }
        if line.starts_with("Costs ") {
            return HeirloomDescLine::mana(line);
        }
        if line.starts_with('+') {
            return HeirloomDescLine::stat(line);
        }
        HeirloomDescLine::effect(line)
    }

    /// Mana/stat header lines first, blank gap, then effect body.
    fn typed_from_rich(classified: Vec<HeirloomDescLine>) -> Vec<HeirloomDescLine> {
        let mut mana = Vec::new();
        let mut stats = Vec::new();
        let mut effects = Vec::new();
        for line in classified {
            match line.kind {
                HeirloomDescLineKind::Mana => mana.push(line),
                HeirloomDescLineKind::Stat => stats.push(line),
                HeirloomDescLineKind::Blank => {}
                HeirloomDescLineKind::Effect => effects.push(line),
            }
        }

        if effects.is_empty() || (mana.is_empty() && stats.is_empty()) {
            return mana.into_iter().chain(stats).chain(effects).collect();
        }

        let mut result = mana;
        result.extend(stats);
        result.push(HeirloomDescLine::blank());
        result.extend(effects);
        result
    }

    /// Mana/stat header lines first, blank gap, then effect body.
    fn typed_from_strings(lines: Vec<String>) -> Vec<HeirloomDescLine> {
        Self::typed_from_rich(lines.into_iter().map(Self::classify_desc_line).collect())
    }

    pub fn desc_lines(&self) -> Vec<HeirloomDescLine> {
        // max 13 char per line, space included
        let lines = match self {
            Heirloom::None => vec!["No Heirloom".to_string()],
            Heirloom::Chest => vec!["Gain a Loot Chest".to_string()],
            Heirloom::CritChance => vec!["+7% Critical Chance".to_string()],
            Heirloom::CritDamage => vec!["+15% Critical Damage".to_string()],
            Heirloom::SkillCDReduction => {
                vec!["Reduce skill".to_string(), "cooldowns by 8%.".to_string()]
            }
            Heirloom::LoadedDice => {
                vec!["+7 Luck".to_string()]
            }
            Heirloom::Health => vec!["+25 Max Health".to_string()],
            Heirloom::Mana => vec!["+25 Max Mana".to_string()],
            Heirloom::Shield => vec!["+10 Shield".to_string()],
            Heirloom::Speed => vec!["+10 Speed".to_string()],
            Heirloom::Thorns => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![HeirloomDescLine::stat_spans([
                    DescSpan::plain("+25 "),
                    DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                    DescSpan::plain(" "),
                ])]);
            }
            Heirloom::Lifesteal => {
                vec!["+3% Lifesteal".to_string()]
            }
            Heirloom::AttackSpeed => vec!["+15% Attack speed".to_string()],
            Heirloom::XPGain => vec!["+7% XP".to_string()],
            Heirloom::CreditCard => vec![
                "Gain 1 Coin when".to_string(),
                "you use a skill.".to_string(),
            ],
            Heirloom::DodgeChance => vec!["+7% Dodge Chance".to_string()],
            Heirloom::Gigantify => vec!["+8% Size".to_string()],

            Heirloom::WaveAttack => vec![
                "Weapon Attacks have".to_string(),
                "a chance to send a".to_string(),
                "sonic wave attack".to_string(),
                "that travels a".to_string(),
                "short distance.".to_string(),
                format!("Costs {} mana", Heirloom::WaveAttack.get_mana_cost()),
            ],
            Heirloom::CherryBomb => vec![
                "Weapon Attacks have".to_string(),
                "a +25% chance to".to_string(),
                "lob a cherry bomb.".to_string(),
                format!("Costs {} mana", Heirloom::CherryBomb.get_mana_cost()),
            ],
            Heirloom::FrailStacks => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Damage you deal has"),
                    HeirloomDescLine::effect("a +25% chance to"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("apply a "),
                        DescSpan::keyword(TooltipDefinition::Frail, "Frail"),
                        DescSpan::plain(" stack."),
                    ]),
                ]);
            }
            Heirloom::SlowStacks => vec![
                "Damage you deal has".to_string(),
                "a +25% chance to".to_string(),
                "apply a Freeze".to_string(),
                "stack.".to_string(),
            ],
            Heirloom::AntFarm => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::Summon, "Summon"),
                        DescSpan::plain(" ants that"),
                    ]),
                    HeirloomDescLine::effect("rush towards"),
                    HeirloomDescLine::effect("enemies, dealing"),
                    HeirloomDescLine::effect("damage."),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::AntFarm.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::StoneTooth => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::Summon, "Summon"),
                        DescSpan::plain(" rocks that"),
                    ]),
                    HeirloomDescLine::effect("orbit you and deal"),
                    HeirloomDescLine::effect("damage to enemies"),
                    HeirloomDescLine::effect("they hit."),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::StoneTooth.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::SummonRing => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::Summon, "Summon"),
                        DescSpan::plain(" rings that"),
                    ]),
                    HeirloomDescLine::effect("pierce enemies and"),
                    HeirloomDescLine::effect("bounce off objects."),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::SummonRing.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::Reaper => vec![
                "Soul fragments".to_string(),
                "chase enemies".to_string(),
                "after each kill,".to_string(),
                "damaging them.".to_string(),
                format!("Costs {} mana", Heirloom::Reaper.get_mana_cost()),
            ],
            Heirloom::SkillEcho => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Using a skill"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("triggers an "),
                        DescSpan::keyword(TooltipDefinition::Echo, "echo"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::SkillEcho.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::PoisonStacks => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Damage you deal has"),
                    HeirloomDescLine::effect("a +25% chance to"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("apply a "),
                        DescSpan::keyword(TooltipDefinition::Poison, "Poison"),
                    ]),
                    HeirloomDescLine::effect("stack."),
                ]);
            }
            Heirloom::LethalBlow => vec![
                "0.5% chance to".to_string(),
                "execute enemies.".to_string(),
                "Executes cause".to_string(),
                "hallucinations,".to_string(),
                "granting random".to_string(),
                "stat buffs.".to_string(),
            ],
            Heirloom::SkillChargeIncrease => vec![
                "Gain +1 extra".to_string(),
                "charge of your".to_string(),
                "active class skill.".to_string(),
            ],
            Heirloom::SkillPower => {
                vec!["+15% Skill Power".to_string()]
            }
            Heirloom::CritSkillCooldownReduction => vec![
                "Landing a critical".to_string(),
                "hit reduces your".to_string(),
                "active skill cooldown".to_string(),
                "by 0.1 seconds.".to_string(),
            ],
            Heirloom::TeleportShock => vec![
                "Teleporting through".to_string(),
                "enemies damages".to_string(),
                "them.".to_string(),
            ],
            Heirloom::TeleportCooldown => vec![
                "Your Teleport".to_string(),
                "cooldown is".to_string(),
                "reduced.".to_string(),
            ],
            Heirloom::TeleportCount => vec!["Gain +1 Teleport".to_string(), "count.".to_string()],
            Heirloom::TeleportManaRegen => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Attacking right"),
                    HeirloomDescLine::effect("after a Teleport"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("triggers "),
                        DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                        DescSpan::plain("."),
                    ]),
                ]);
            }
            Heirloom::SprintFaster => {
                vec!["Your Sprint ability".to_string(), "is faster.".to_string()]
            }
            Heirloom::SprintLungeDamage => vec![
                "Your Lunge attack".to_string(),
                "does more damage.".to_string(),
            ],
            Heirloom::SprintKillReset => vec![
                "Killing an enemy".to_string(),
                "resets your Sprint".to_string(),
                "and Lunge attack".to_string(),
                "cooldown.".to_string(),
            ],
            Heirloom::ChanceToProcExtraAttack => vec![
                "Attacks have a.".to_string(),
                "chance to trigger".to_string(),
                "another attack.".to_string(),
                format!(
                    "Costs {} mana.",
                    Heirloom::ChanceToProcExtraAttack.get_mana_cost()
                ),
            ],
            Heirloom::IncreaseProjectileCount => vec![
                "Increase all weapon".to_string(),
                "projectile count".to_string(),
                "by 1.".to_string(),
                // format!(
                //     "Costs {} mana.",
                //     Heirloom::IncreaseProjectileCount.get_mana_cost()
                // ),
            ],

            Heirloom::IceStaffAoE => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Your weapons have"),
                    HeirloomDescLine::effect("a 7% chance to"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("trigger an "),
                        DescSpan::keyword(TooltipDefinition::IceExplosion, "Ice Explosion"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::IceStaffAoE.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::BowArrowSpeed => {
                vec!["Your Projectiles".to_string(), "move faster.".to_string()]
            }
            Heirloom::Attack => vec!["+10% Damage".to_string()],
            Heirloom::Defence => vec!["+10 Defence".to_string()],
            Heirloom::ParryHPRegen => vec![
                "A successful".to_string(),
                "parry triggers".to_string(),
                "health regeneration".to_string(),
            ],
            Heirloom::ParryDeflectProj => vec![
                "A successful".to_string(),
                "parry deflects".to_string(),
                "projectiles.".to_string(),
            ],
            Heirloom::ParryKnockback => vec![
                "A successful".to_string(),
                "parry knocks ".to_string(),
                "back enemies.".to_string(),
            ],
            Heirloom::ParryEcho => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("A successful"),
                    HeirloomDescLine::effect("parry triggers"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("an "),
                        DescSpan::keyword(TooltipDefinition::Echo, "echo"),
                        DescSpan::plain(" that"),
                    ]),
                    HeirloomDescLine::effect("damages enemies"),
                    HeirloomDescLine::effect("around you."),
                ]);
            }
            Heirloom::DaggerCombo => vec![
                "Weapon Attacks chained".to_string(),
                "together build ".to_string(),
                "Combo, increasing ".to_string(),
                "your critical ".to_string(),
                "damage.".to_string(),
            ],
            Heirloom::HPRegen => vec!["+5 Health Regen".to_string()],
            Heirloom::MPRegen => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![HeirloomDescLine::stat_spans([
                    DescSpan::plain("+5 "),
                    DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                ])]);
            }
            Heirloom::HPRegenCooldown => {
                vec![
                    "Your Health regen".to_string(),
                    "cooldown is reduced.".to_string(),
                ]
            }
            Heirloom::MPRegenCooldown => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("Your "),
                        DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                    ]),
                    HeirloomDescLine::effect("cooldown is reduced"),
                ]);
            }
            Heirloom::OnHitEcho => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("After taking damage,"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("trigger an "),
                        DescSpan::keyword(TooltipDefinition::Echo, "echo"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::OnHitEcho.get_mana_cost()
                    )),
                ]);
            }

            Heirloom::Knockback => vec![
                "Damage you deal will".to_string(),
                "knockback enemies".to_string(),
                "further. ".to_string(),
            ],
            Heirloom::DiscountMP => vec![
                "Your staffs' attacks".to_string(),
                "cost less mana.".to_string(),
            ],
            Heirloom::MinusOneDamageOnHit => vec![
                "All incoming enemy ".to_string(),
                "damage is reduced".to_string(),
                "by one. ".to_string(),
            ],
            Heirloom::TeleportStatusDMG => vec![
                "Teleporting through".to_string(),
                "an enemy with a".to_string(),
                "status effect deals".to_string(),
                "more damage.".to_string(),
            ],

            Heirloom::FrozenAoE => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Killing a frozen"),
                    HeirloomDescLine::effect("enemy has a 25%"),
                    HeirloomDescLine::effect("chance to trigger an"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::IceExplosion, "Ice Explosion"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::stat("+25% freeze chance"),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::FrozenAoE.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::IceStaffFloor => vec![
                "Killing an enemy has".to_string(),
                "a 10% chance to leave".to_string(),
                "a trail of ice that".to_string(),
                "damages enemies. ".to_string(),
                format!("Costs {} mana", Heirloom::IceStaffFloor.get_mana_cost()),
            ],
            Heirloom::FrozenCrit => vec![
                "Attacking frozen".to_string(),
                "enemies gives you".to_string(),
                "a +25% critical hit".to_string(),
                "chance.".to_string(),
                "+25% freeze chance".to_string(),
            ],
            Heirloom::MPBarDMG => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([DescSpan::keyword(
                        TooltipDefinition::ManaRegen,
                        "Mana Regen",
                    )]),
                    HeirloomDescLine::effect("is stored, adding"),
                    HeirloomDescLine::effect("bonus damage on"),
                    HeirloomDescLine::effect("your next attack."),
                ]);
            }
            Heirloom::MPBarCrit => vec![
                "Your staff's attacks".to_string(),
                "gain +10% critical".to_string(),
                "hit chance if your".to_string(),
                "mana bar is full.".to_string(),
            ],
            Heirloom::FrozenMPRegen => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Killing a frozen"),
                    HeirloomDescLine::effect("enemy has a 20%"),
                    HeirloomDescLine::effect("chance to trigger"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::stat("+25% freeze chance"),
                ]);
            }
            Heirloom::DodgeCrit => vec![
                "Dodging grants 2x".to_string(),
                "attack speed and a".to_string(),
                "speed burst. Next".to_string(),
                "weapon hit does 2x".to_string(),
                "damage.".to_string(),
            ],
            Heirloom::PoisonDuration => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("Your "),
                        DescSpan::keyword(TooltipDefinition::Poison, "Poison"),
                        DescSpan::plain(" effect"),
                    ]),
                    HeirloomDescLine::effect("lasts longer."),
                    HeirloomDescLine::stat_spans([
                        DescSpan::plain("+25% "),
                        DescSpan::keyword(TooltipDefinition::Poison, "Poison"),
                        DescSpan::plain(" chance."),
                    ]),
                ]);
            }
            Heirloom::PoisonStrength => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("Your "),
                        DescSpan::keyword(TooltipDefinition::Poison, "Poison"),
                        DescSpan::plain(" effect"),
                    ]),
                    HeirloomDescLine::effect("does +100% more"),
                    HeirloomDescLine::effect("damage."),
                    HeirloomDescLine::stat_spans([
                        DescSpan::plain("+25% "),
                        DescSpan::keyword(TooltipDefinition::Poison, "Poison"),
                        DescSpan::plain(" chance."),
                    ]),
                ]);
            }
            Heirloom::ViralVenum => vec![
                "Killing a poisoned".to_string(),
                "enemy spreads it's".to_string(),
                "poison to nearby".to_string(),
                "enemies.".to_string(),
                "+25% poison chance.".to_string(),
                format!("Costs {} mana", Heirloom::ViralVenum.get_mana_cost()),
            ],
            Heirloom::HealEcho => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Healing has a 10%"),
                    HeirloomDescLine::effect("chance to trigger an"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::Echo, "echo"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::stat("+5 Health regen."),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::HealEcho.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::HealSummons => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Healing has a 10%"),
                    HeirloomDescLine::effect("chance to trigger"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("all "),
                        DescSpan::keyword(TooltipDefinition::Summon, "Summon"),
                        DescSpan::plain(" heirlooms"),
                    ]),
                    HeirloomDescLine::effect("once."),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::HealSummons.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::FullStomach => vec![
                "You get hungry".to_string(),
                "at a slower rate.".to_string(),
            ],
            Heirloom::ReinforcedArmor => vec![
                "You gain Defence".to_string(),
                "the more speed".to_string(),
                "you have lost. Lose".to_string(),
                "5 speed. ".to_string(),
            ],
            Heirloom::ChaosBoost => vec![
                "Increases chaos,".to_string(),
                "making enemies".to_string(),
                "stronger and".to_string(),
                "more rewarding.".to_string(),
            ],

            // New heirlooms
            Heirloom::MaxHPHunt => {
                vec!["Every 25 kills".to_string(), "gain +1 Max HP.".to_string()]
            }
            Heirloom::SkillPowerHunt => {
                vec![
                    "7% chance when using".to_string(),
                    "a skill to gain +1".to_string(),
                    "Skill Power.".to_string(),
                ]
            }
            Heirloom::MaxHPDamage => vec![
                "Gain +10% Damage".to_string(),
                "for every 100".to_string(),
                "Max HP you have.".to_string(),
            ],
            Heirloom::GoldIntoDamage => vec![
                "Gain +1% Damage".to_string(),
                "for every 10".to_string(),
                "coins you have.".to_string(),
            ],
            Heirloom::DeathDefiance => vec![
                "Survive death once,".to_string(),
                "restore 50% HP,".to_string(),
                "freeze all enemies".to_string(),
                "for 3 seconds.".to_string(),
            ],
            Heirloom::RegenLifesteal => {
                vec!["-7 HP Regen".to_string(), "+7% Lifesteal".to_string()]
            }
            Heirloom::StandStill => vec![
                "Standing still".to_string(),
                "increases damage".to_string(),
                "rapidly.".to_string(),
            ],
            Heirloom::ThornArmor => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("Gain +10 "),
                        DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                    ]),
                    HeirloomDescLine::effect("for every 10"),
                    HeirloomDescLine::effect("Defence you have."),
                    HeirloomDescLine::stat("+10 Defence."),
                ]);
            }
            Heirloom::LifestealCoins => vec![
                "Lifesteal triggers".to_string(),
                "have a 10% chance to".to_string(),
                "give you a coin.".to_string(),
                "+5% Lifesteal.".to_string(),
            ],

            // Wave 2 heirlooms
            Heirloom::CoinHeal => vec![
                "Picking up coins".to_string(),
                "has a 25% chance".to_string(),
                "to heal 1 HP.".to_string(),
            ],
            Heirloom::CrateBreakDamage => vec![
                "Breaking a crate".to_string(),
                "permanently gives".to_string(),
                "you +1.5% damage.".to_string(),
            ],
            Heirloom::EnergyBallBarrage => vec![
                "Every 150 damage you".to_string(),
                "deal fires a homing".to_string(),
                "fire ball at a".to_string(),
                "nearby enemy.".to_string(),
            ],
            Heirloom::SkillExplosion => vec![
                "Skill damage triggers".to_string(),
                "a small explosion at".to_string(),
                "the target.".to_string(),
                format!("Costs {} mana", Heirloom::SkillExplosion.get_mana_cost()),
            ],
            Heirloom::TomeDoubleUpgrade => vec![
                "Upgrade Tomes".to_string(),
                "level up gear".to_string(),
                "an extra time.".to_string(),
            ],
            Heirloom::CritHeal => vec![
                "Critical hits".to_string(),
                "have a 15% chance".to_string(),
                "to heal 1 HP.".to_string(),
            ],
            Heirloom::LowHPDamage => vec![
                "Deal more damage".to_string(),
                "the lower your HP".to_string(),
                "is (up to +75%".to_string(),
                "at 0 HP).".to_string(),
            ],
            Heirloom::ChaosStats => vec![
                "+3 Chaos. +10 HP,".to_string(),
                "+10 MP, +10% dmg,".to_string(),
                "+10 def, +10% crit".to_string(),
                "+10 spd, +10 dodge".to_string(),
            ],
            Heirloom::ManaOrbs => vec!["Mana Orbs restore".to_string(), "5 more Mana.".to_string()],
            Heirloom::ManaOrbAttack => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                        DescSpan::plain(" shoots a"),
                    ]),
                    HeirloomDescLine::effect("mana orb at an enemy."),
                    HeirloomDescLine::effect("It does damage equal"),
                    HeirloomDescLine::effect("to the amount"),
                    HeirloomDescLine::effect("regenerated."),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::ManaOrbAttack.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::ItemPickupRadius => vec![
                "+25% pickup range".to_string(),
                "Increases item".to_string(),
                "pickup radius.".to_string(),
            ],
            Heirloom::GravityScales => vec![
                "Converts 25% of".to_string(),
                "pickup range into".to_string(),
                "size.".to_string(),
            ],
            Heirloom::MagnetPull => vec![
                "Periodically pulls".to_string(),
                "all item drops on".to_string(),
                "the map to you.".to_string(),
                "Cooldown reduces".to_string(),
                "with more copies.".to_string(),
            ],

            // Thorns build heirlooms
            Heirloom::ThornsSpikes => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Taking damage shoots"),
                    HeirloomDescLine::effect("out 2 spikes. Damage"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("scales with "),
                        DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::stat_spans([
                        DescSpan::plain("+15 "),
                        DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                        DescSpan::plain("."),
                    ]),
                ]);
            }
            Heirloom::ThornsOnDamage => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("Gain +1 "),
                        DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                        DescSpan::plain(" each"),
                    ]),
                    HeirloomDescLine::effect("time you take damage."),
                ]);
            }
            Heirloom::ThornsLifesteal => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("Your "),
                        DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                        DescSpan::plain(" damage"),
                    ]),
                    HeirloomDescLine::effect("has +25% lifesteal."),
                    HeirloomDescLine::stat_spans([
                        DescSpan::plain("+15 "),
                        DescSpan::keyword(TooltipDefinition::Thorns, "Thorns"),
                        DescSpan::plain("."),
                    ]),
                ]);
            }

            // Lightning strikes archetype
            Heirloom::CoinLightning => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Picking up coins"),
                    HeirloomDescLine::effect("spawns a"),
                    HeirloomDescLine::effect_spans([DescSpan::keyword(
                        TooltipDefinition::Lightning,
                        "Lightning Strike",
                    )]),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::CoinLightning.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::KillLightning => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Killing an enemy"),
                    HeirloomDescLine::effect("has a 7% chance"),
                    HeirloomDescLine::effect("to spawn a"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::Lightning, "Lightning Strike"),
                        DescSpan::plain("."),
                    ]),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana",
                        Heirloom::KillLightning.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::ManaRegenLightning => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect_spans([
                        DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                        DescSpan::plain(" has a"),
                    ]),
                    HeirloomDescLine::effect("20% chance to Spawn"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("a "),
                        DescSpan::keyword(TooltipDefinition::Lightning, "Lightning Strike"),
                    ]),
                    HeirloomDescLine::mana(format!(
                        "Costs {} mana.",
                        Heirloom::ManaRegenLightning.get_mana_cost()
                    )),
                ]);
            }
            Heirloom::ManaRegenPoison => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Every time you"),
                    HeirloomDescLine::effect("regenerate 75 mana,"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("apply a "),
                        DescSpan::keyword(TooltipDefinition::Poison, "Poison"),
                    ]),
                    HeirloomDescLine::effect("stack to all"),
                    HeirloomDescLine::effect("enemies."),
                ]);
            }
            Heirloom::SkillManaRegen => {
                use crate::ui::desc_spans::DescSpan;
                use crate::ui::TooltipDefinition;
                return Self::typed_from_rich(vec![
                    HeirloomDescLine::effect("Using a skill has"),
                    HeirloomDescLine::effect("a 15% chance to"),
                    HeirloomDescLine::effect_spans([
                        DescSpan::plain("trigger "),
                        DescSpan::keyword(TooltipDefinition::ManaRegen, "Mana Regen"),
                        DescSpan::plain("."),
                    ]),
                ]);
            }
            Heirloom::DamageDealtMp => vec![
                "Damage from Weapons".to_string(),
                "or skills has a 4%".to_string(),
                "chance to restore 3".to_string(),
                "Mana.".to_string(),
            ],
            Heirloom::ManaOrbDropMult => vec![
                "Mana Orb drop".to_string(),
                "chance from enemies".to_string(),
                "is doubled.".to_string(),
            ],
            Heirloom::GoldenTooth => vec![
                "Mobs that drop a".to_string(),
                "coin have a +10%".to_string(),
                "chance to drop an".to_string(),
                "extra coin.".to_string(),
            ],
        };
        Self::typed_from_strings(lines)
    }

    pub fn get_desc(&self) -> Vec<String> {
        self.desc_lines()
            .into_iter()
            .filter(|line| line.kind != HeirloomDescLineKind::Blank)
            .map(|line| line.text)
            .collect()
    }

    pub fn get_instant_drop(&self) -> Option<(WorldObject, usize)> {
        match self {
            Heirloom::Chest => Some((WorldObject::ChestBlock, 1)),
            _ => None,
        }
    }

    /// One-time chaos granted when this heirloom is newly acquired (not on duplicates).
    pub fn acquisition_chaos_bonus(&self) -> Option<f32> {
        match self {
            Heirloom::ChaosBoost => Some(1.5),
            Heirloom::ChaosStats => Some(3.0),
            _ => None,
        }
    }

    pub fn apply_acquisition_effects(&self, chaos_tracker: &mut ChaosTracker) {
        if let Some(amount) = self.acquisition_chaos_bonus() {
            chaos_tracker.add_chaos(amount);
        }
    }

    pub fn add_heirloom_components(
        &self,
        entity: Entity,
        commands: &mut Commands,
        skills: PlayerSkills,
    ) {
        match self {
            Heirloom::IncreaseProjectileCount => {
                commands.entity(entity).insert(ClawUpgradeMultiThrow(
                    Timer::from_seconds(0.12, TimerMode::Once),
                    skills.get_count(self.clone()) as u8,
                ));
                commands
                    .entity(entity)
                    .insert(BowUpgradeSpread(skills.get_count(self.clone()) as u8));
            }
            Heirloom::BowArrowSpeed => {
                commands.entity(entity).insert(ArrowSpeedUpgrade(
                    1. + skills.get_count(self.clone()) as f32 * 0.25,
                ));
            }
            &Heirloom::TeleportCount => {
                // TeleportCount heirloom is deprecated - use SkillChargeIncrease instead
                commands.entity(entity).insert(TeleportState {
                    just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    timer: Timer::from_seconds(0.06, TimerMode::Once),
                });
            }
            &Heirloom::DaggerCombo => {
                // Reset time halved: stacks decay after 0.5s without a hit
                commands.entity(entity).insert(ComboCounter {
                    counter: 0,
                    reset_timer: Timer::from_seconds(0.5, TimerMode::Once),
                });
            }
            Heirloom::AntFarm => {
                // Don't reset the cooldown when gaining extra copies.
                commands
                    .entity(entity)
                    .insert_if_new(crate::player::combat_heirlooms::AntFarmState::default());
            }
            Heirloom::MagnetPull => {
                commands.entity(entity).insert(MagnetPullTimer {
                    cooldown_timer: Timer::from_seconds(
                        (BASE_MAGNET_COOLDOWN
                            - ((skills.get_count(Heirloom::MagnetPull) - 1) as f32
                                * MAGNET_COOLDOWN_REDUCTION_PER_STACK))
                            .max(MIN_MAGNET_COOLDOWN),
                        TimerMode::Repeating,
                    ),
                    duration_timer: Timer::from_seconds(5.0, TimerMode::Once),
                });
            }
            Heirloom::StoneTooth => {
                commands
                    .entity(entity)
                    .insert(crate::player::combat_heirlooms::StoneToothState::default());
            }
            Heirloom::SummonRing => {
                commands
                    .entity(entity)
                    .insert_if_new(crate::player::combat_heirlooms::SummonRingState::default());
            }
            Heirloom::Reaper => {
                commands
                    .entity(entity)
                    .insert(crate::player::combat_heirlooms::ReaperState::default());
            }
            Heirloom::MaxHPHunt => {
                // Only add tracker if this is the first MaxHPHunt heirloom
                // (skills already includes this heirloom when this is called)
                if skills.get_count(Heirloom::MaxHPHunt) == 1 {
                    commands
                        .entity(entity)
                        .insert(crate::player::combat_heirlooms::MaxHPHuntTracker::default());
                }
            }
            Heirloom::SkillPowerHunt => {
                if skills.get_count(Heirloom::SkillPowerHunt) == 1 {
                    commands
                        .entity(entity)
                        .insert(crate::player::combat_heirlooms::SkillPowerHuntTracker::default());
                }
            }
            Heirloom::StandStill => {
                commands
                    .entity(entity)
                    .insert(crate::player::combat_heirlooms::StandStillState::default());
            }
            Heirloom::CrateBreakDamage => {
                // Only add tracker if this is the first one
                if skills.get_count(Heirloom::CrateBreakDamage) == 1 {
                    commands.entity(entity).insert(
                        crate::player::combat_heirlooms::CrateBreakDamageTracker::default(),
                    );
                }
            }
            Heirloom::ThornsOnDamage => {
                // Only add tracker if this is the first one
                if skills.get_count(Heirloom::ThornsOnDamage) == 1 {
                    commands
                        .entity(entity)
                        .insert(crate::player::combat_heirlooms::ThornsOnDamageTracker::default());
                }
            }
            Heirloom::DodgeCrit => {
                // Add the dodge buff state component
                commands
                    .entity(entity)
                    .insert(crate::player::combat_heirlooms::DodgeCritState::default());
            }
            Heirloom::LethalBlow => {
                // Add hallucination stats tracker (only once)
                if skills.get_count(Heirloom::LethalBlow) == 1 {
                    commands
                        .entity(entity)
                        .insert(crate::player::combat_heirlooms::HallucinationStats::default());
                }
            }
            Heirloom::MPBarDMG => {
                // Add mana charge damage state tracker (only once)
                if skills.get_count(Heirloom::MPBarDMG) == 1 {
                    commands
                        .entity(entity)
                        .insert(crate::player::combat_heirlooms::ManaChargeDamageState::default());
                }
            }
            Heirloom::ManaRegenPoison => {
                // Add mana regen poison tracker (only once)
                if skills.get_count(Heirloom::ManaRegenPoison) == 1 {
                    commands
                        .entity(entity)
                        .insert(crate::player::combat_heirlooms::ManaRegenPoisonTracker::default());
                }
            }
            Heirloom::EnergyBallBarrage => {
                if skills.get_count(Heirloom::EnergyBallBarrage) == 1 {
                    commands.entity(entity).insert(
                        crate::player::combat_heirlooms::EnergyBallBarrageTracker::default(),
                    );
                }
            }

            _ => {}
        }
    }
    pub fn get_ui_element(&self, rarity: HeirloomRarity) -> (UIElement, Vec2) {
        match rarity {
            HeirloomRarity::Common => (UIElement::SkillChoice, Vec2::new(164., 191.)),
            HeirloomRarity::Uncommon => (UIElement::SkillChoiceRogue, Vec2::new(164., 191.)),
            HeirloomRarity::Rare => (UIElement::SkillChoiceMagic, Vec2::new(164., 191.)),
            HeirloomRarity::Legendary => (UIElement::SkillChoiceMelee, Vec2::new(164., 191.)),
        }
    }
    pub fn get_ui_element_hover(&self, rarity: HeirloomRarity) -> UIElement {
        match rarity {
            HeirloomRarity::Common => UIElement::SkillChoiceHover,
            HeirloomRarity::Uncommon => UIElement::SkillChoiceRogueHover,
            HeirloomRarity::Rare => UIElement::SkillChoiceMagicHover,
            HeirloomRarity::Legendary => UIElement::SkillChoiceMeleeHover,
        }
    }

    pub fn is_obj_valid(&self, obj: WorldObject) -> bool {
        match self {
            // Heirloom::WaveAttack => obj.is_melee_weapon(),
            // Heirloom::FrailStacks => obj.is_melee_weapon(),
            // Heirloom::LethalBlow => obj.is_melee_weapon(),
            _ => true,
        }
    }
}

#[derive(Message)]
pub struct ActiveSkillUsedEvent {
    pub slot: usize,
    pub cooldown: f32,
}

/// Single source of truth for per-slot charges and cooldown (hotkey slots 0–3).
#[derive(Clone, Debug)]
pub struct SlotSkillRuntime {
    pub current_charges: u32,
    pub max_charges: u32,
    pub cooldown_timer: Timer,
    pub base_cooldown: f32,
    pub tracked_skill: ActiveSkill,
}

impl SlotSkillRuntime {
    pub fn start_cooldown_seconds(&mut self, seconds: f32, should_run: bool) {
        let secs = seconds.max(0.0);
        let mut t = Timer::from_seconds(secs.max(0.0001), TimerMode::Once);
        if !should_run {
            t.tick(Duration::from_secs_f32(secs.max(0.0)));
        }
        self.cooldown_timer = t;
    }
}

impl Default for ClassSkillSlots {
    fn default() -> Self {
        let finished = || {
            let mut t = Timer::from_seconds(1.0, TimerMode::Once);
            t.tick(Duration::from_secs_f32(999.0));
            t
        };
        Self(std::array::from_fn(|_| SlotSkillRuntime {
            current_charges: 1,
            max_charges: 1,
            cooldown_timer: finished(),
            base_cooldown: 0.0,
            tracked_skill: ActiveSkill::Roll,
        }))
    }
}

/// Four class skill slots (`active_skill_slot_0` … `_3`). Cooldowns and charges live here only.
#[derive(Component, Clone, Debug)]
pub struct ClassSkillSlots(pub [SlotSkillRuntime; 4]);

/// +1 charge on the slot that matches `skill`, capped at `max_charges`.
pub fn grant_skill_charge_after_cooldown_complete(
    _player: Entity,
    skill: ActiveSkill,
    slots: &mut ClassSkillSlots,
) {
    for slot in &mut slots.0 {
        if slot.tracked_skill == skill && slot.current_charges < slot.max_charges {
            slot.current_charges += 1;
            break;
        }
    }
}

#[derive(
    Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord, Default, Debug, Serialize, Deserialize,
)]
pub enum HeirloomRarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Legendary,
}

impl HeirloomRarity {
    pub fn get_item_glow(&self) -> Option<ItemGlow> {
        match self {
            HeirloomRarity::Common => None,
            HeirloomRarity::Uncommon => Some(ItemGlow::Blue),
            HeirloomRarity::Rare => Some(ItemGlow::Purple),
            HeirloomRarity::Legendary => Some(ItemGlow::Red),
        }
    }

    pub fn get_color(&self) -> Color {
        match self {
            HeirloomRarity::Common => COMMON_TOOLTIP_TITLE,
            HeirloomRarity::Uncommon => UNCOMMON_TOOLTIP_TITLE,
            HeirloomRarity::Rare => RARE_TOOLTIP_TITLE,
            HeirloomRarity::Legendary => LEGENDARY_TOOLTIP_TITLE,
        }
    }
}
#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct HeirloomChoiceState {
    pub heirloom: Heirloom,
    pub child_heirlooms: Vec<HeirloomChoiceState>,
    pub clashing_heirlooms: Vec<Heirloom>,
    pub is_one_time_heirloom: bool,
    pub rarity: HeirloomRarity,
}

#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct ActiveSkillChoiceState {
    pub active_skill: ActiveSkill,
    pub rarity: HeirloomRarity,
}

impl ActiveSkillChoiceState {
    pub fn new(active_skill: ActiveSkill, rarity: HeirloomRarity) -> Self {
        Self {
            active_skill,
            rarity,
        }
    }
}
impl HeirloomChoiceState {
    pub fn new(heirloom: Heirloom, rarity: HeirloomRarity) -> Self {
        Self {
            heirloom,
            child_heirlooms: Default::default(),
            clashing_heirlooms: Default::default(),
            is_one_time_heirloom: false,
            rarity,
        }
    }
    pub fn with_children(mut self, children: Vec<HeirloomChoiceState>) -> Self {
        self.child_heirlooms = children;
        self
    }
    pub fn set_repeatable(mut self) -> Self {
        self.is_one_time_heirloom = false;
        self
    }
    pub fn _with_clashing(mut self, clashing: Vec<Heirloom>) -> Self {
        self.clashing_heirlooms = clashing;
        self
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct HeirloomChoiceQueue {
    pub queue: Vec<[HeirloomChoiceState; 3]>,
    pub pool: Vec<HeirloomChoiceState>,
    pub active_heirloom_limbo: Option<ActiveSkillChoiceState>,
    #[serde(default)]
    pub banned: HashSet<Heirloom>,
    /// Heirlooms banished this run (for UI). Pool math uses live counts; this is display-only.
    #[serde(default)]
    pub banished_heirlooms: Vec<HeirloomChoiceState>,
    /// Number of consecutive level-up/shrine choice sets that offered NO uncommon heirloom.
    /// Drives the uncommon pity system: odds ramp up per miss and an uncommon is guaranteed
    /// once this reaches [`UNCOMMON_PITY_GUARANTEE`]. Chests/shop/blessings/rerolls don't touch it.
    #[serde(default)]
    pub choices_since_uncommon: u32,
}

/// After this many consecutive choice sets without an uncommon, the next set guarantees one.
pub const UNCOMMON_PITY_GUARANTEE: u32 = 3;
/// Each missed choice lowers the uncommon roll threshold by this many percentage points,
/// gradually increasing uncommon odds before the hard guarantee kicks in.
const UNCOMMON_PITY_STEP: f32 = 8.0;

impl Default for HeirloomChoiceQueue {
    /// An empty queue/pool placeholder. Use [`HeirloomChoiceQueue::new_for_player`] to
    /// build the actual run-time pool gated on the player's [`TimeCrystals`] progress.
    /// `Default` is kept around for serde + save-state defaults and is overwritten
    /// before being used in gameplay.
    fn default() -> Self {
        Self {
            queue: Default::default(),
            active_heirloom_limbo: None,
            pool: Vec::new(),
            banned: HashSet::default(),
            banished_heirlooms: Vec::new(),
            choices_since_uncommon: 0,
        }
    }
}

/// Heirlooms unlocked by completing a single time crystal.
///
/// Single source of truth for both:
/// - [`HeirloomChoiceQueue::new_for_player`] which conditionally appends them to the run pool, and
/// - the post-run Time Crystal progress popup which shows the unlocks.
///
/// Indices outside the configured crystal range return an empty list.
pub fn time_crystal_heirlooms(idx: usize) -> Vec<(Heirloom, HeirloomRarity)> {
    match idx {
        0 => vec![
            (Heirloom::ManaOrbs, HeirloomRarity::Common),
            (Heirloom::ManaOrbAttack, HeirloomRarity::Uncommon),
            (Heirloom::SkillManaRegen, HeirloomRarity::Common),
        ],
        1 => vec![
            (Heirloom::FrailStacks, HeirloomRarity::Uncommon),
            (Heirloom::WaveAttack, HeirloomRarity::Rare),
            (Heirloom::ManaRegenPoison, HeirloomRarity::Rare),
        ],
        2 => vec![
            (Heirloom::SkillCDReduction, HeirloomRarity::Common),
            (Heirloom::GravityScales, HeirloomRarity::Rare),
            (Heirloom::IncreaseProjectileCount, HeirloomRarity::Rare),
        ],
        3 => vec![
            (Heirloom::CritChance, HeirloomRarity::Common),
            (Heirloom::CritDamage, HeirloomRarity::Common),
            (Heirloom::Lifesteal, HeirloomRarity::Common),
        ],
        4 => vec![
            (Heirloom::RegenLifesteal, HeirloomRarity::Uncommon),
            (Heirloom::FrozenMPRegen, HeirloomRarity::Rare),
            (Heirloom::SkillExplosion, HeirloomRarity::Uncommon),
        ],
        5 => vec![
            (Heirloom::CoinLightning, HeirloomRarity::Legendary),
            (Heirloom::GoldIntoDamage, HeirloomRarity::Rare),
        ],
        6 => vec![
            (Heirloom::LowHPDamage, HeirloomRarity::Rare),
            (Heirloom::ThornsLifesteal, HeirloomRarity::Uncommon),
            (Heirloom::ThornsOnDamage, HeirloomRarity::Rare),
        ],
        7 => vec![
            (Heirloom::XPGain, HeirloomRarity::Common),
            (Heirloom::BowArrowSpeed, HeirloomRarity::Uncommon),
            (Heirloom::SkillPowerHunt, HeirloomRarity::Rare),
        ],
        8 => vec![
            (Heirloom::SlowStacks, HeirloomRarity::Uncommon),
            (Heirloom::IceStaffAoE, HeirloomRarity::Rare),
            (Heirloom::FrozenCrit, HeirloomRarity::Rare),
        ],
        9 => vec![
            (Heirloom::IceStaffFloor, HeirloomRarity::Legendary),
            (Heirloom::DaggerCombo, HeirloomRarity::Legendary),
            (Heirloom::CreditCard, HeirloomRarity::Legendary),
        ],
        10 => vec![
            (Heirloom::StandStill, HeirloomRarity::Legendary),
            (Heirloom::TomeDoubleUpgrade, HeirloomRarity::Legendary),
        ],
        11 => vec![
            (Heirloom::DodgeCrit, HeirloomRarity::Rare),
            (Heirloom::ChaosBoost, HeirloomRarity::Uncommon),
            (Heirloom::ChaosStats, HeirloomRarity::Rare),
        ],
        _ => vec![],
    }
}

impl HeirloomChoiceQueue {
    /// Base heirloom pool with **no** time-crystal unlocks (used for banish caps per rarity).
    pub fn base_pool_entries() -> Vec<HeirloomChoiceState> {
        vec![
            HeirloomChoiceState::new(Heirloom::Speed, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Thorns, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Attack, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Knockback, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Defence, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Gigantify, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::HPRegen, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::MPRegen, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Health, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Mana, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::DodgeChance, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::AntFarm, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::SkillPower, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::CoinHeal, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::ItemPickupRadius, HeirloomRarity::Common),
            HeirloomChoiceState::new(Heirloom::Chest, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::HPRegenCooldown, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::MPRegenCooldown, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::FrozenAoE, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::HealEcho, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::AttackSpeed, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::PoisonStacks, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::StoneTooth, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::LoadedDice, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::ThornsSpikes, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::DamageDealtMp, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::KillLightning, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::CherryBomb, HeirloomRarity::Uncommon),
            HeirloomChoiceState::new(Heirloom::OnHitEcho, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::PoisonDuration, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::PoisonStrength, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::SummonRing, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::SkillChargeIncrease, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::SkillEcho, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::CritSkillCooldownReduction, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::MaxHPDamage, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::ThornArmor, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::CritHeal, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::ManaRegenLightning, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::ManaOrbDropMult, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::MagnetPull, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::HealSummons, HeirloomRarity::Rare),
            HeirloomChoiceState::new(Heirloom::ViralVenum, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::Reaper, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::MaxHPHunt, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::DeathDefiance, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::LifestealCoins, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::CrateBreakDamage, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::EnergyBallBarrage, HeirloomRarity::Legendary),
            HeirloomChoiceState::new(Heirloom::GoldenTooth, HeirloomRarity::Uncommon),
        ]
    }

    pub fn base_pool_count_for_rarity(rarity: HeirloomRarity) -> usize {
        Self::base_pool_entries()
            .iter()
            .filter(|x| x.rarity == rarity)
            .count()
    }

    /// Reconstruct the theoretical run pool from `TimeCrystals` (base + crystal unlocks),
    /// minus heirlooms already banished in this run. This is independent of the live
    /// `pool`/`queue` state, which mutates as level-ups draw cards / clashing heirlooms
    /// are pruned / child heirlooms are added.
    fn reconstructed_available_pool(
        &self,
        time_crystals: &TimeCrystals,
    ) -> Vec<HeirloomChoiceState> {
        let mut pool: Vec<HeirloomChoiceState> = Self::base_pool_entries();
        for idx in 0..time_crystals.crystals.len() {
            if !time_crystals.is_complete(idx) {
                continue;
            }
            for (heirloom, rarity) in time_crystal_heirlooms(idx) {
                pool.push(HeirloomChoiceState::new(heirloom, rarity));
            }
        }
        pool.retain(|x| !self.banned.contains(&x.heirloom));
        pool
    }

    /// Count of heirlooms of `rarity` available in the reconstructed run pool.
    pub fn reconstructed_count_for_rarity(
        &self,
        time_crystals: &TimeCrystals,
        rarity: HeirloomRarity,
    ) -> usize {
        self.reconstructed_available_pool(time_crystals)
            .iter()
            .filter(|x| x.rarity == rarity)
            .count()
    }

    /// Max further banishes allowed at this rarity without dropping the pool below `base_count - 1`.
    /// Formula: `current_pool_count - base_pool_count + 1` (clamped).
    /// Uses the **reconstructed** pool (not the live one) so the count is stable across
    /// level-ups, picks, and clashing/child heirloom edits.
    pub fn allowed_banishes_for_rarity(
        &self,
        time_crystals: &TimeCrystals,
        rarity: HeirloomRarity,
    ) -> usize {
        let base = Self::base_pool_count_for_rarity(rarity);
        let current = self.reconstructed_count_for_rarity(time_crystals, rarity);
        current.saturating_add(1).saturating_sub(base)
    }

    /// Whether a specific heirloom offer may be banished (rarity cap + non-placeholder).
    pub fn banish_allowed_for_heirloom(
        &self,
        time_crystals: &TimeCrystals,
        heirloom: &HeirloomChoiceState,
    ) -> bool {
        if heirloom.heirloom == Heirloom::None {
            return false;
        }
        self.allowed_banishes_for_rarity(time_crystals, heirloom.rarity) > 0
    }

    /// Whether the heirloom currently offered in `queue[0][slot]` may be banished (rarity cap + non-placeholder).
    pub fn banish_allowed_for_choice_slot(
        &self,
        time_crystals: &TimeCrystals,
        slot: usize,
    ) -> bool {
        let Some(choices) = self.queue.first() else {
            return false;
        };
        self.banish_allowed_for_heirloom(time_crystals, &choices[slot])
    }

    /// Build the heirloom pool for a new run, gated on the player's persistent
    /// [`TimeCrystals`] progress. Crystal-gated entries are appended only when the
    /// corresponding crystal is complete. Crystals fill in order, so crystal 0 is
    /// unlocked first, then crystal 1, etc.
    pub fn new_for_player(time_crystals: &TimeCrystals) -> Self {
        let mut pool = Self::base_pool_entries();

        // Crystal-gated heirlooms. Mapping lives in `time_crystal_heirlooms`.
        for idx in 0..time_crystals.crystals.len() {
            if !time_crystals.is_complete(idx) {
                continue;
            }
            for (heirloom, rarity) in time_crystal_heirlooms(idx) {
                pool.push(HeirloomChoiceState::new(heirloom, rarity));
            }
        }
        info!("====== Heirloom Summery ======");
        info!(
            " Common: {:?}",
            pool.iter()
                .filter(|x| x.rarity == HeirloomRarity::Common)
                .count()
        );
        info!(
            " Uncommon: {:?}",
            pool.iter()
                .filter(|x| x.rarity == HeirloomRarity::Uncommon)
                .count()
        );
        info!(
            " Rare: {:?}",
            pool.iter()
                .filter(|x| x.rarity == HeirloomRarity::Rare)
                .count()
        );
        info!(
            " Legendary: {:?}",
            pool.iter()
                .filter(|x| x.rarity == HeirloomRarity::Legendary)
                .count()
        );

        Self {
            queue: Default::default(),
            active_heirloom_limbo: None,
            pool,
            banned: HashSet::default(),
            banished_heirlooms: Vec::new(),
            choices_since_uncommon: 0,
        }
    }

    /// Convenience constructor that returns a queue with every crystal-gated heirloom
    /// unlocked. Used by the heirloom card export tool.
    pub fn with_all_unlocks() -> Self {
        Self::new_for_player(&TimeCrystals::all_complete())
    }

    /// Builds the run heirloom pool from [`TimeCrystals`], or the full pool when the
    /// options-menu bypass is enabled.
    pub fn new_for_run(time_crystals: &TimeCrystals, bypass_time_crystals: bool) -> Self {
        if bypass_time_crystals {
            Self::with_all_unlocks()
        } else {
            Self::new_for_player(time_crystals)
        }
    }
}
impl HeirloomChoiceQueue {
    pub fn add_new_skills_after_levelup(
        &mut self,
        rng: &mut rand::rngs::ThreadRng,
        loot_bonus: i32,
        player_level: u8,
    ) {
        //only push if queue is empty
        if self.queue.is_empty() {
            // Uncommon pity: ramp up uncommon odds per missed choice set, and once we've
            // missed `UNCOMMON_PITY_GUARANTEE` times in a row, force one slot to be uncommon.
            let missed = self.choices_since_uncommon;
            let uncommon_bonus = missed as f32 * UNCOMMON_PITY_STEP;
            let guarantee_uncommon = missed >= UNCOMMON_PITY_GUARANTEE;
            let forced_uncommon_slot = if guarantee_uncommon {
                rng.gen_range(0..3)
            } else {
                usize::MAX
            };

            let mut new_skills: [HeirloomChoiceState; 3] = Default::default();
            let mut add_back_to_pool: Vec<HeirloomChoiceState> = vec![];
            for i in 0..3 {
                let rarity = if i == forced_uncommon_slot {
                    HeirloomRarity::Uncommon
                } else {
                    HeirloomChoiceQueue::gen_rarity_with_uncommon_bonus(
                        rng,
                        loot_bonus,
                        uncommon_bonus,
                    )
                };
                if let Some(picked_skill) =
                    self.get_skill_of_rarity(rarity.clone(), rng, player_level, &|s| {
                        // Only check slots that have been explicitly set (0..i)
                        // This avoids the issue where Default::default() initializes all slots with CritChance
                        !new_skills[0..i].iter().any(|existing| existing == s)
                    })
                {
                    if !picked_skill.is_one_time_heirloom {
                        add_back_to_pool.push(picked_skill.clone());
                    }
                    new_skills[i] = picked_skill.clone();
                    self.pool.retain(|x| x != &new_skills[i]);
                }
            }
            for skill in add_back_to_pool
                .iter()
                .filter(|skill| !self.banned.contains(&skill.heirloom))
            {
                self.pool.push(skill.clone());
            }

            // Update the pity counter based on what was actually offered.
            let offered_uncommon = new_skills
                .iter()
                .any(|s| s.heirloom != Heirloom::default() && s.rarity == HeirloomRarity::Uncommon);
            if offered_uncommon {
                self.choices_since_uncommon = 0;
            } else {
                self.choices_since_uncommon = self.choices_since_uncommon.saturating_add(1);
            }

            self.queue.push(new_skills.clone());
        }
    }
    pub fn get_skill_of_rarity(
        &self,
        rarity: HeirloomRarity,
        rng: &mut rand::rngs::ThreadRng,
        player_level: u8,
        filter: &dyn Fn(&HeirloomChoiceState) -> bool,
    ) -> Option<HeirloomChoiceState> {
        let filtered: Vec<_> = self
            .pool
            .iter()
            .filter(|x| {
                let matches_rarity = x.rarity == rarity;
                let passes_filter = filter(x);
                let not_banned = !self.banned.contains(&x.heirloom);
                let level_unlocked = x.heirloom.min_player_level() <= player_level;
                matches_rarity
                    && passes_filter
                    && not_banned
                    && level_unlocked
                    && x.heirloom != Heirloom::default()
            })
            .collect();
        // Convert back to owned values for choose
        let owned_filtered: Vec<HeirloomChoiceState> =
            filtered.iter().map(|x| (*x).clone()).collect();
        owned_filtered.as_slice().choose(rng).cloned()
    }

    /// Pick a random heirloom from the run pool that has `tooltip`, respecting banish
    /// and level gates. Rarity comes from the pool entry (canonical per `skills.rs`).
    pub fn pick_random_heirloom_with_tooltip(
        &self,
        tooltip: TooltipDefinition,
        rng: &mut rand::rngs::ThreadRng,
        player_level: u8,
    ) -> Option<HeirloomWithRarity> {
        let candidates: Vec<HeirloomWithRarity> = self
            .pool
            .iter()
            .filter(|state| {
                state.heirloom.tooltip_definitions().contains(&tooltip)
                    && !self.banned.contains(&state.heirloom)
                    && state.heirloom.min_player_level() <= player_level
                    && state.heirloom != Heirloom::default()
            })
            .map(|state| HeirloomWithRarity {
                heirloom: state.heirloom.clone(),
                rarity: state.rarity.clone(),
            })
            .collect();
        candidates.choose(rng).cloned()
    }

    pub fn gen_rarity(rng: &mut rand::rngs::ThreadRng, loot_bonus: i32) -> HeirloomRarity {
        Self::gen_rarity_with_uncommon_bonus(rng, loot_bonus, 0.0)
    }

    /// Like [`gen_rarity`](Self::gen_rarity) but lowers the uncommon threshold by
    /// `uncommon_bonus` percentage points, increasing the odds of rolling Uncommon
    /// (without affecting Rare/Legendary). Used by the uncommon pity system.
    pub fn gen_rarity_with_uncommon_bonus(
        rng: &mut rand::rngs::ThreadRng,
        loot_bonus: i32,
        uncommon_bonus: f32,
    ) -> HeirloomRarity {
        // Base probabilities: Common 61%, Uncommon 23%, Rare 13%, Legendary 3%
        // Loot bonus increases higher rarity chances
        // Formula: each point of loot increases higher rarity chances by shifting thresholds
        // Each point of loot: +0.1% legendary, +0.2% rare, +0.1% uncommon
        // Since we roll 0-99 (100 values), each 1% = 1.0 threshold point

        let loot_bonus_f = loot_bonus as f32;
        // Calculate adjusted thresholds (lower threshold = more chance for that rarity)
        let legendary_threshold = (99.5 - loot_bonus_f * 0.02).max(0.0);
        let rare_threshold = (95.5 - loot_bonus_f * 0.06).max(0.0);
        // Clamp so the uncommon band can't cross into the rare band.
        let uncommon_threshold = (75.0 - loot_bonus_f * 0.1 - uncommon_bonus)
            .max(0.0)
            .min(rare_threshold);
        let roll = rng.gen_range(0_f32..100_f32);

        if roll >= legendary_threshold {
            HeirloomRarity::Legendary
        } else if roll >= rare_threshold {
            HeirloomRarity::Rare
        } else if roll >= uncommon_threshold {
            HeirloomRarity::Uncommon
        } else {
            HeirloomRarity::Common
        }
    }

    pub fn handle_pick_skill(
        &mut self,
        skill: HeirloomChoiceState,
        commands: &mut Commands,
        proto: &ProtoParam,
        player_pos: Vec2,
        player_skills: &mut PlayerSkills,
        player_level: u8,
    ) {
        player_skills.heirlooms.push(HeirloomWithRarity {
            heirloom: skill.heirloom.clone(),
            rarity: skill.rarity.clone(),
        });
        let mut remaining_choices = self.queue.remove(0).to_vec();
        remaining_choices.retain(|x| x != &skill);
        for choice in remaining_choices.iter() {
            // Never push the banish placeholder (Heirloom::None) back to the pool
            if choice.heirloom != Heirloom::default() {
                self.pool.push(choice.clone());
            }
        }

        for child in skill.child_heirlooms.iter() {
            if !player_skills
                .heirlooms
                .iter()
                .any(|h| h.heirloom == child.heirloom)
            {
                self.pool.push(child.clone());
            }
        }
        for clash in skill.clashing_heirlooms.iter() {
            self.pool.retain(|x| x.heirloom != *clash);
        }

        // handle drops
        if let Some((drop, count)) = skill.heirloom.get_instant_drop() {
            commands.spawn_item_from_proto(
                drop,
                proto,
                player_pos + Vec2::new(0., -18.), // offset so it doesn't spawn on the player
                count,
                Some(player_level),
            );
        }
    }

    /// Grant a heirloom directly from the pool (used by heirloom chests).
    /// Unlike handle_pick_skill, this doesn't manipulate the queue.
    pub fn grant_heirloom_from_pool(
        &mut self,
        skill: HeirloomChoiceState,
        commands: &mut Commands,
        proto: &ProtoParam,
        player_pos: Vec2,
        player_skills: &mut PlayerSkills,
        player_level: u8,
    ) {
        // Add to player's heirlooms
        player_skills.heirlooms.push(HeirloomWithRarity {
            heirloom: skill.heirloom.clone(),
            rarity: skill.rarity.clone(),
        });

        // Remove from pool if it's a one-time heirloom
        if skill.is_one_time_heirloom {
            self.pool.retain(|x| x.heirloom != skill.heirloom);
        }

        // Add child heirlooms to pool
        for child in skill.child_heirlooms.iter() {
            if !player_skills
                .heirlooms
                .iter()
                .any(|h| h.heirloom == child.heirloom)
            {
                self.pool.push(child.clone());
            }
        }

        // Remove clashing heirlooms from pool
        for clash in skill.clashing_heirlooms.iter() {
            self.pool.retain(|x| x.heirloom != *clash);
        }

        // Handle drops
        if let Some((drop, count)) = skill.heirloom.get_instant_drop() {
            commands.spawn_item_from_proto(
                drop,
                proto,
                player_pos + Vec2::new(0., -18.),
                count,
                Some(player_level),
            );
        }
    }

    pub fn handle_reroll_slot(
        &mut self,
        slot: usize,
        rng: &mut rand::rngs::ThreadRng,
        loot_bonus: i32,
        player_level: u8,
    ) {
        if self.queue.is_empty() {
            return;
        }
        let old_skill = self.queue[0][slot].clone();
        let rarity = HeirloomChoiceQueue::gen_rarity(rng, loot_bonus);
        if let Some(picked_skill) =
            self.get_skill_of_rarity(rarity.clone(), rng, player_level, &|s| {
                !self.queue[0].contains(s)
            })
        {
            if picked_skill.is_one_time_heirloom {
                self.pool.retain(|x| x != &picked_skill);
            }

            // Never push the banish placeholder (Heirloom::None) back to the pool
            if old_skill.heirloom != Heirloom::default() {
                self.pool.push(old_skill);
            }
            self.queue[0][slot] = picked_skill;
        }
    }

    /// Reroll every non-banished slot in the current offer (one reroll charge).
    pub fn handle_reroll_all(
        &mut self,
        rng: &mut rand::rngs::ThreadRng,
        loot_bonus: i32,
        player_level: u8,
    ) {
        if self.queue.is_empty() {
            return;
        }
        let slots: Vec<usize> = self.queue[0]
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.heirloom != Heirloom::default())
            .map(|(slot, _)| slot)
            .collect();
        for slot in slots {
            self.handle_reroll_slot(slot, rng, loot_bonus, player_level);
        }
    }

    pub fn banish_slot(
        &mut self,
        time_crystals: &TimeCrystals,
        slot: usize,
    ) -> Option<HeirloomChoiceState> {
        if self.queue.is_empty() {
            return None;
        }
        if !self.banish_allowed_for_choice_slot(time_crystals, slot) {
            return None;
        }
        let mut choices = self.queue.remove(0);
        let banned_choice = choices[slot].clone();
        choices[slot] = HeirloomChoiceState::default();
        self.queue.push(choices.clone());
        self.banned.insert(banned_choice.heirloom.clone());
        self.banished_heirlooms.push(banned_choice.clone());
        self.pool.retain(|x| x.heirloom != banned_choice.heirloom);

        for (index, choice) in choices.into_iter().enumerate() {
            if index == slot {
                continue;
            }
            if !self.banned.contains(&choice.heirloom) && choice.heirloom != Heirloom::default() {
                self.pool.push(choice);
            }
        }

        Some(banned_choice)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeirloomWithRarity {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
}

#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct PlayerSkills {
    pub heirlooms: Vec<HeirloomWithRarity>,
    pub active_skill_slot_0: Option<ActiveSkillChoiceState>,
    pub active_skill_slot_1: Option<ActiveSkillChoiceState>,
    pub active_skill_slot_2: Option<ActiveSkillChoiceState>,
    pub active_skill_slot_3: Option<ActiveSkillChoiceState>,
    /// Slot 4 (bonus slot from blessings, hidden by default)
    pub active_skill_slot_4: Option<ActiveSkillChoiceState>,
}

impl Default for PlayerSkills {
    fn default() -> Self {
        Self {
            heirlooms: vec![],
            active_skill_slot_0: Some(ActiveSkillChoiceState::new(
                ActiveSkill::Roll,
                HeirloomRarity::Common,
            )),
            active_skill_slot_1: None,
            active_skill_slot_2: None,
            active_skill_slot_3: None,
            active_skill_slot_4: None, // Bonus slot - unlocked by blessings
        }
    }
}

impl PlayerSkills {
    pub fn has(&self, heirloom: Heirloom) -> bool {
        self.heirlooms.iter().any(|h| h.heirloom == heirloom)
    }
    pub fn skill_cooldown_multiplier(&self) -> f32 {
        let count = self.get_count(Heirloom::SkillCDReduction).max(0) as i32;
        (0..count).fold(1.0f32, |acc, _| acc * 0.92)
    }
    pub fn effective_skill_cooldown(
        &self,
        skill: &ActiveSkill,
        blessings: &crate::blessings::OwnedBlessings,
    ) -> f32 {
        self.effective_skill_cooldown_with_majors(
            skill,
            blessings,
            &crate::blessings::OwnedMajorBlessings::default(),
        )
    }

    pub fn effective_skill_cooldown_with_majors(
        &self,
        skill: &ActiveSkill,
        blessings: &crate::blessings::OwnedBlessings,
        majors: &crate::blessings::OwnedMajorBlessings,
    ) -> f32 {
        let base = (skill.get_base_cooldown() - majors.skill_base_cooldown_reduction()).max(0.5);
        base * self.skill_cooldown_multiplier() * blessings.get_skill_cooldown_increase()
    }
    pub fn skill_extra_charges(&self) -> u32 {
        self.get_count(Heirloom::SkillChargeIncrease).max(0) as u32
    }
    pub fn calculate_freeze_chance(&self) -> f64 {
        let mut chance = 0.0;
        let freeze_skills = [
            Heirloom::FrozenAoE,
            // Heirloom::IceStaffFloor,
            Heirloom::FrozenCrit,
            Heirloom::FrozenMPRegen,
            Heirloom::SlowStacks,
        ];
        for skill in freeze_skills {
            chance += self.get_count(skill) as f64 * 0.25;
        }
        chance
    }

    pub fn calculate_frail_chance(&self) -> f64 {
        self.get_count(Heirloom::FrailStacks) as f64 * 0.25
    }

    pub fn calculate_poison_chance(&self) -> f64 {
        let mut chance = 0.0;
        let poison_skills = [
            Heirloom::PoisonDuration,
            Heirloom::PoisonStrength,
            Heirloom::ViralVenum,
            Heirloom::PoisonStacks,
        ];
        for skill in poison_skills {
            chance += self.get_count(skill) as f64 * 0.25;
        }
        chance
    }

    /// (label, value) row for the player stats tooltip.
    pub fn poison_chance_stat_summary(&self) -> (String, String) {
        (
            "Poison Chance   ".to_string(),
            format!("{:.0}%", self.calculate_poison_chance() * 100.0),
        )
    }

    /// Rolls how many poison stacks to apply from total poison chance.
    /// Values above 100% guarantee extra stacks (150% → 1 + 50% roll for 2, 300% → 3, etc.).
    pub fn roll_poison_stacks_from_chance(&self, rng: &mut impl rand::Rng) -> u32 {
        Self::roll_stacks_from_chance(self.calculate_poison_chance(), rng)
    }

    pub fn roll_freeze_stacks_from_chance(&self, rng: &mut impl rand::Rng) -> u32 {
        Self::roll_stacks_from_chance(self.calculate_freeze_chance(), rng)
    }

    pub fn roll_frail_stacks_from_chance(&self, rng: &mut impl rand::Rng) -> u32 {
        Self::roll_stacks_from_chance(self.calculate_frail_chance(), rng)
    }

    /// Values above 100% guarantee extra stacks (250% → 2 + 50% for a 3rd).
    pub fn roll_stacks_from_chance(chance: f64, rng: &mut impl rand::Rng) -> u32 {
        if chance <= 0.0 {
            return 0;
        }
        let guaranteed = chance.floor() as u32;
        let remainder = chance - guaranteed as f64;
        guaranteed + u32::from(remainder > 0.0 && rng.gen_bool(remainder))
    }
    /// Hotkey slot for the equipped movement skill, if any.
    pub fn movement_skill_slot(&self) -> Option<usize> {
        for slot in 0..5 {
            if self
                .get_active_skill_in_slot(slot)
                .is_some_and(ActiveSkill::is_movement_skill)
            {
                return Some(slot);
            }
        }
        None
    }

    pub fn has_active_skill(&self, active_skill: ActiveSkill) -> Option<usize> {
        if self
            .active_skill_slot_0
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(0);
        }
        if self
            .active_skill_slot_1
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(1);
        }
        if self
            .active_skill_slot_2
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(2);
        }
        if self
            .active_skill_slot_3
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(3);
        }
        if self
            .active_skill_slot_4
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(4);
        }
        None
    }
    pub fn get_count(&self, heirloom: Heirloom) -> i32 {
        self.heirlooms
            .iter()
            .filter(|h| h.heirloom == heirloom)
            .count() as i32
    }
    pub fn get_heirloom_rarity(&self, heirloom: Heirloom) -> Option<HeirloomRarity> {
        self.heirlooms
            .iter()
            .find(|h| h.heirloom == heirloom)
            .map(|h| h.rarity.clone())
    }
    pub fn get_active_skill_in_slot(&self, slot: usize) -> Option<ActiveSkill> {
        match slot {
            0 => self
                .active_skill_slot_0
                .as_ref()
                .map(|s| s.active_skill.clone()),
            1 => self
                .active_skill_slot_1
                .as_ref()
                .map(|s| s.active_skill.clone()),
            2 => self
                .active_skill_slot_2
                .as_ref()
                .map(|s| s.active_skill.clone()),
            3 => self
                .active_skill_slot_3
                .as_ref()
                .map(|s| s.active_skill.clone()),
            4 => self
                .active_skill_slot_4
                .as_ref()
                .map(|s| s.active_skill.clone()),
            _ => None,
        }
    }
    pub fn get_active_skill_choice_in_slot(&self, slot: usize) -> Option<&ActiveSkillChoiceState> {
        match slot {
            0 => self.active_skill_slot_0.as_ref(),
            1 => self.active_skill_slot_1.as_ref(),
            2 => self.active_skill_slot_2.as_ref(),
            3 => self.active_skill_slot_3.as_ref(),
            4 => self.active_skill_slot_4.as_ref(),
            _ => None,
        }
    }
    pub fn insert_active_skill(&mut self, skill: ActiveSkillChoiceState, slot: usize) {
        match slot {
            0 => self.active_skill_slot_0 = Some(skill),
            1 => self.active_skill_slot_1 = Some(skill),
            2 => self.active_skill_slot_2 = Some(skill),
            3 => self.active_skill_slot_3 = Some(skill),
            4 => self.active_skill_slot_4 = Some(skill),
            _ => {}
        }
    }
}

/// Tracks how many times each heirloom effect has successfully triggered during a run,
/// plus rolling-window HUD orb stats (mana spent, health gained, mana regened).
#[derive(Resource, Default, Clone, Debug)]
pub struct HeirloomTriggerCounts {
    pub counts: HashMap<Heirloom, u32>,
    pub mana_consumed: HashMap<Heirloom, u64>,
    pub weapon_mana_consumed: HashMap<WorldObject, u64>,
    pub health_gained: HashMap<HealthGainSource, u64>,
    pub mana_gained: HashMap<ManaGainSource, u64>,
}

/// Sources tracked on the health orb HUD tooltip (1-minute rolling window).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum HealthGainSource {
    CoinHeal,
    HealthRegen,
    Lifesteal,
    VampiricRing,
}

impl HealthGainSource {
    pub fn label(&self) -> &'static str {
        match self {
            HealthGainSource::CoinHeal => "Coin Heal",
            HealthGainSource::HealthRegen => "Regen",
            HealthGainSource::Lifesteal => "Lifesteal",
            HealthGainSource::VampiricRing => "Vampiric Ring",
        }
    }

    pub fn heirloom_icon(&self) -> Option<Heirloom> {
        match self {
            HealthGainSource::CoinHeal => Some(Heirloom::CoinHeal),
            HealthGainSource::VampiricRing => Some(Heirloom::CritHeal),
            HealthGainSource::HealthRegen | HealthGainSource::Lifesteal => None,
        }
    }
}

/// Sources tracked on the mana orb HUD tooltip gain list (1-minute rolling window).
///
/// Heirlooms such as `FrozenMPRegen` and `SkillManaRegen` manually trigger mana regen, so
/// they are tracked as their own [`ManaGainSource::Heirloom`] entries instead of being lumped
/// into [`ManaGainSource::ManaRegen`] (the natural regen timer).
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum ManaGainSource {
    /// Natural mana regen from the mana regen timer.
    ManaRegen,
    /// Mana orb pickups.
    ManaOrbs,
    /// Mana restored by consumables.
    Potion,
    /// A heirloom that grants/triggers mana (e.g. Blue Card, Mirror, Brown Card).
    Heirloom(Heirloom),
}

impl ManaGainSource {
    pub fn label(&self) -> &'static str {
        match self {
            ManaGainSource::ManaRegen => "Regen",
            ManaGainSource::ManaOrbs => "Mana Orbs",
            ManaGainSource::Potion => "Potion",
            ManaGainSource::Heirloom(_) => "",
        }
    }

    pub fn heirloom_icon(&self) -> Option<Heirloom> {
        match self {
            ManaGainSource::Heirloom(heirloom) => Some(heirloom.clone()),
            _ => None,
        }
    }

    /// Lower numbers sort earlier in the gain list.
    fn display_order(&self) -> u8 {
        match self {
            ManaGainSource::ManaRegen => 0,
            ManaGainSource::ManaOrbs => 1,
            ManaGainSource::Potion => 2,
            ManaGainSource::Heirloom(_) => 3,
        }
    }
}

impl HeirloomTriggerCounts {
    pub fn increment(&mut self, heirloom: Heirloom) {
        *self.counts.entry(heirloom).or_insert(0) += 1;
    }
    pub fn get(&self, heirloom: &Heirloom) -> u32 {
        self.counts.get(heirloom).copied().unwrap_or(0)
    }

    pub fn record_mana(&mut self, heirloom: Heirloom, amount: i32) {
        if amount > 0 {
            *self.mana_consumed.entry(heirloom).or_insert(0) += amount as u64;
        }
    }

    pub fn record_weapon_mana(&mut self, weapon: WorldObject, amount: i32) {
        if amount > 0 {
            *self.weapon_mana_consumed.entry(weapon).or_insert(0) += amount as u64;
        }
    }

    pub fn total_mana_consumed(&self) -> u64 {
        self.mana_consumed.values().sum::<u64>() + self.weapon_mana_consumed.values().sum::<u64>()
    }

    pub fn mana_consumed_percentage(&self, heirloom: &Heirloom) -> u32 {
        let total = self.total_mana_consumed();
        if total == 0 {
            return 0;
        }
        let amount = self.mana_consumed.get(heirloom).copied().unwrap_or(0);
        ((amount as f64 / total as f64) * 100.0).round() as u32
    }

    pub fn weapon_mana_consumed_percentage(&self, weapon: &WorldObject) -> u32 {
        let total = self.total_mana_consumed();
        if total == 0 {
            return 0;
        }
        let amount = self.weapon_mana_consumed.get(weapon).copied().unwrap_or(0);
        ((amount as f64 / total as f64) * 100.0).round() as u32
    }

    pub fn mana_consumed_per_second(&self, window_elapsed_secs: f32) -> f32 {
        Self::per_second(self.total_mana_consumed(), window_elapsed_secs)
    }

    pub fn sorted_mana_entries(&self) -> Vec<(Heirloom, u64)> {
        let mut entries: Vec<_> = self
            .mana_consumed
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .map(|(heirloom, amount)| (heirloom.clone(), *amount))
            .collect();
        entries.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.get_title().cmp(&b.0.get_title()))
        });
        entries
    }

    pub fn sorted_weapon_mana_entries(&self) -> Vec<(WorldObject, u64)> {
        let mut entries: Vec<_> = self
            .weapon_mana_consumed
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .map(|(weapon, amount)| (*weapon, *amount))
            .collect();
        entries.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
        });
        entries
    }

    pub fn reset_mana_consumed(&mut self) {
        self.mana_consumed.clear();
        self.weapon_mana_consumed.clear();
    }

    pub fn record_health_gain(&mut self, source: HealthGainSource, amount: i32) {
        if amount > 0 {
            *self.health_gained.entry(source).or_insert(0) += amount as u64;
        }
    }

    pub fn record_mana_gained(&mut self, source: ManaGainSource, amount: i32) {
        if amount > 0 {
            *self.mana_gained.entry(source).or_insert(0) += amount as u64;
        }
    }

    pub fn total_mana_gained(&self) -> u64 {
        self.mana_gained.values().sum()
    }

    pub fn mana_gained_percentage(&self, source: &ManaGainSource) -> u32 {
        let total = self.total_mana_gained();
        if total == 0 {
            return 0;
        }
        let amount = self.mana_gained.get(source).copied().unwrap_or(0);
        ((amount as f64 / total as f64) * 100.0).round() as u32
    }

    pub fn sorted_mana_gain_entries(&self) -> Vec<(ManaGainSource, u64)> {
        let mut entries: Vec<_> = self
            .mana_gained
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .map(|(source, amount)| (source.clone(), *amount))
            .collect();
        entries.sort_by(|a, b| {
            a.0.display_order()
                .cmp(&b.0.display_order())
                .then_with(|| b.1.cmp(&a.1))
                .then_with(|| a.0.label().cmp(b.0.label()))
        });
        entries
    }

    pub fn total_health_gained(&self) -> u64 {
        self.health_gained.values().sum()
    }

    pub fn health_gained_percentage(&self, source: &HealthGainSource) -> u32 {
        let total = self.total_health_gained();
        if total == 0 {
            return 0;
        }
        let amount = self.health_gained.get(source).copied().unwrap_or(0);
        ((amount as f64 / total as f64) * 100.0).round() as u32
    }

    pub fn sorted_health_gain_entries(&self) -> Vec<(HealthGainSource, u64)> {
        const ORDER: [HealthGainSource; 4] = [
            HealthGainSource::CoinHeal,
            HealthGainSource::HealthRegen,
            HealthGainSource::Lifesteal,
            HealthGainSource::VampiricRing,
        ];
        ORDER
            .into_iter()
            .filter_map(|source| {
                self.health_gained
                    .get(&source)
                    .copied()
                    .filter(|amount| *amount > 0)
                    .map(|amount| (source, amount))
            })
            .collect()
    }

    pub fn per_second(total: u64, window_elapsed_secs: f32) -> f32 {
        if window_elapsed_secs <= 0.0 {
            return 0.0;
        }
        total as f32 / window_elapsed_secs
    }

    pub fn mana_gained_per_second(&self, window_elapsed_secs: f32) -> f32 {
        Self::per_second(self.total_mana_gained(), window_elapsed_secs)
    }

    pub fn health_gained_per_second(&self, window_elapsed_secs: f32) -> f32 {
        Self::per_second(self.total_health_gained(), window_elapsed_secs)
    }

    pub fn reset_hud_orb_window_stats(&mut self) {
        self.mana_consumed.clear();
        self.weapon_mana_consumed.clear();
        self.health_gained.clear();
        self.mana_gained.clear();
    }
}

/// Repeating timer that clears HUD orb rolling-window stats every minute.
#[derive(Resource)]
pub struct ManaTrackerResetTimer(pub Timer);

impl Default for ManaTrackerResetTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(60.0, TimerMode::Repeating))
    }
}

pub fn tick_mana_tracker_reset(
    time: Res<Time>,
    mut timer: ResMut<ManaTrackerResetTimer>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        trigger_counts.reset_hud_orb_window_stats();
    }
}
