use std::time::Duration;

use bevy::prelude::*;
use bevy_aseprite::Aseprite;
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use strum_macros::{Display, EnumIter};

use crate::{
    animations::player_sprite::{
        PlayerBlueAseprite, PlayerGreenAseprite, PlayerGreyAseprite, PlayerRedAseprite,
        PlayerSpriteHandles,
    },
    attributes::{AttributeQuality, AttributeValue, ItemAttributes, ItemGlow},
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
    proto::proto_param::ProtoParam,
    ui::UIElement,
    Pet,
};

use super::{mage_skills::TeleportState, rogue_skills::ComboCounter};

#[derive(Component, Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash, EnumIter)]
pub enum SkillClass {
    None,

    Warrior, // Sword, Spear, Hammer - +5 HP per level
    Wizard,  // Fire Staff, Ice Staff, Basic Staff - +5 MP per level
    Rogue,   // Dagger - +3% crit chance per level
    Thief,   // Claw - +3% attack speed per level
    Hunter,  // Bow, Gun - +3% crit dmg per level
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, Resource)]
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
            SkillClass::Wizard => (sprites.blue.clone(), PlayerBlueAseprite::tags::IDLE_FRONT),
            SkillClass::Rogue => (sprites.green.clone(), PlayerGreenAseprite::tags::IDLE_FRONT),
            SkillClass::Thief => (sprites.grey.clone(), PlayerGreyAseprite::tags::IDLE_FRONT),
            SkillClass::Hunter => (sprites.green.clone(), PlayerGreenAseprite::tags::IDLE_FRONT),
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
        let quality = if level > 10 {
            AttributeQuality::High
        } else if level >= 5 {
            AttributeQuality::Average
        } else {
            AttributeQuality::Low
        };
        // Base damage bonus for all classes
        stats.bonus_damage = AttributeValue::new(f32::floor(level as f32 * 1.) as i32, quality, 1.);

        match self {
            SkillClass::Warrior => {
                // +5 HP per level
                stats.health = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Wizard => {
                // +5 MP per level
                stats.mana = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Rogue => {
                // +3% crit chance per level
                stats.crit_chance = AttributeValue::new(level * 1, quality, 1.);
            }
            SkillClass::Thief => {
                // +3% attack speed per level
                stats.attack_speed = AttributeValue::new(level * 2, quality, 1.);
            }
            SkillClass::Hunter => {
                // +3% crit dmg per level
                stats.crit_damage = AttributeValue::new(level * 1, quality, 1.);
            }
            _ => (),
        }
        stats
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, EnumIter, Display, Deserialize, Default)]
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
}

impl ActiveSkill {
    /// Returns the base cooldown in seconds for this skill
    pub fn get_base_cooldown(&self) -> f32 {
        match self {
            ActiveSkill::Roll => 1.2, // Cooldown matches player_dash_cooldown duration
            ActiveSkill::Parry => 1.2,
            ActiveSkill::ParrySpear => 12.,
            ActiveSkill::Sprint => 8.0,
            ActiveSkill::SprintLunge => 1.2,
            ActiveSkill::Teleport => 1.2,
            ActiveSkill::Stealth => 11.0,
            ActiveSkill::Rapidfire => 12.0,
            ActiveSkill::FirePillar => 12.0,
            ActiveSkill::Heal => 45.0,
            ActiveSkill::Buckshot => 3.0,
            ActiveSkill::IceWall => 10.0,
            ActiveSkill::DruidTree => 11.0,
            ActiveSkill::Shout => 7.0,
            ActiveSkill::PiercingStar => 8.0,
            ActiveSkill::LaserBeam => 13.0,
            // New skills - placeholder cooldowns
            ActiveSkill::Lightning => 5.0,
            ActiveSkill::DaggerThrow => 7.0,
            ActiveSkill::DaggerSlash => 5.0,
            ActiveSkill::TripleThrow => 1.5,
            ActiveSkill::Fury => 13.0,
            ActiveSkill::Bomb => 5.5,
            ActiveSkill::SpinAttack => 2.5,
        }
    }
}

#[derive(Component, Clone)]
pub struct StealthState {
    pub duration: Timer,
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct RapidfireState {
    pub duration: Timer,
    pub cooldown_timer: Timer,
    pub attack_speed_bonus: f32,
}
#[derive(Component, Clone)]
pub struct FirePillarState {
    pub cooldown_timer: Timer,
    pub hit_clear_timer: Timer,
}
#[derive(Component, Clone)]
pub struct LaserBeamState {
    pub cooldown_timer: Timer,
    pub hit_clear_timer: Timer,
}
#[derive(Component, Clone)]
pub struct HealSkillState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct BuckshotSkillState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct IceWallSkillState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct DruidTreeSkillState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct ShoutSkillState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct PiercingStarSkillState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct LightningState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct DaggerThrowState {
    pub cooldown_timer: Timer,
}

/// Tracks kills since last dagger throw cast (max 10)
/// Kills from dagger throw projectiles themselves don't count
#[derive(Component, Default)]
pub struct DaggerThrowKillTracker {
    pub kill_count: i32,
}

/// Tracks the last projectile that hit an enemy (used to exclude dagger throw kills)
#[derive(Component)]
pub struct LastHitProjectile {
    pub projectile: Option<Projectile>,
}
#[derive(Component, Clone)]
pub struct SlashState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct TripleThrowState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct FuryState {
    pub cooldown_timer: Timer,
    pub duration: Timer,
    pub throw_timer: Timer,
}
#[derive(Component, Clone)]
pub struct BombState {
    pub cooldown_timer: Timer,
}
#[derive(Component, Clone)]
pub struct SpinAttackState {
    pub cooldown_timer: Timer,
}

impl ActiveSkill {
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
        }
    }

    pub fn get_desc(&self, skill_power: f32) -> Vec<String> {
        match self {
            ActiveSkill::Roll => vec![
                "Roll to dodge attacks. You are".to_string(),
                "invulnerable while rolling.".to_string(),
            ],
            ActiveSkill::SpinAttack => vec![
                "Gain a burst of speed and dash".to_string(),
                "forwards while spinning a sword".to_string(),
                format!("dealing {:.1}% damage around you.", skill_power * 80.0),
            ],
            ActiveSkill::Shout => vec![
                "SCREAM, releasing a shockwave".to_string(),
                format!("around you, dealing {:.1}% damage.", skill_power * 160.0),
            ],
            ActiveSkill::Heal => vec![
                format!("Heal yourself for {:.1}% of your", skill_power * 30.0),
                "max health.".to_string(),
            ],
            ActiveSkill::Parry => vec![
                "Active: Time successfully".to_string(),
                "to Parry attacks ignore".to_string(),
                "damage and stunning.".to_string(),
            ],
            ActiveSkill::ParrySpear => vec![
                "".to_string(),
                "Launch a spear that pulls nearby".to_string(),
                "enemies towards the impact area".to_string(),
                format!("and deals {:.1}% damage.", skill_power * 185.0),
            ],
            ActiveSkill::Sprint => vec![
                "You are imbued with a burst of speed.".to_string(),
                "Gain 60% speed temporarily.".to_string(),
            ],
            ActiveSkill::SprintLunge => vec![
                "Lunge quickly through enemies in a".to_string(),
                format!("line dealing {:.1}% damage. You are", skill_power * 85.0),
                "invulnerable during the attack.".to_string(),
            ],
            ActiveSkill::DaggerThrow => vec![
                "Throw a dagger at a nearby enemy".to_string(),
                format!("dealing {:.1}% damage. Throw one more", skill_power * 115.0),
                "per mob killed since the last cast.".to_string(),
            ],
            ActiveSkill::DaggerSlash => vec![
                "Quickly slash in front of you,".to_string(),
                format!("dealing {:.1}% damage in an area.", skill_power * 175.0),
            ],
            ActiveSkill::Stealth => vec![
                "Dissapear for a short duration,".to_string(),
                "ignoring all damage. Attacks used".to_string(),
                "during Stealth will always crit.".to_string(),
            ],
            ActiveSkill::Teleport => vec![
                format!("Teleport forwards, dealing {:.1}%", skill_power * 33.0),
                "damage to enemies you pass through.".to_string(),
                // "through.".to_string(),
            ],
            ActiveSkill::Lightning => vec![
                "Call down lighning on 3 nearby".to_string(),
                format!(
                    "enemies, dealing {:.1}% damage to each.",
                    skill_power * 85.0
                ),
            ],
            ActiveSkill::FirePillar => {
                vec![
                    "Scorch the earth at target area,".to_string(),
                    format!("dealing {:.1}% damage continuously.", skill_power * 55.0),
                ]
            }
            ActiveSkill::IceWall => {
                vec![
                    "Summon an ice pillar at target".to_string(),
                    format!("area dealing {:.1}% damage.", skill_power * 300.0),
                ]
            }
            ActiveSkill::Buckshot => vec![
                "Fire a shotgun round dealing".to_string(),
                format!("5x {:.1}% damage spread in a cone.", skill_power * 100.0),
                "Knocks you back a moderate amount.".to_string(),
            ],
            ActiveSkill::Bomb => vec![
                "Throw a bomb at target area that".to_string(),
                format!("explodes on impact, dealing {:.1}%", skill_power * 220.0),
                "damage and applying frail.".to_string(),
            ],
            ActiveSkill::DruidTree => vec![
                "Place a target dummy at target area ".to_string(),
                "that taunts enemies towards it".to_string(),
                "for a short duration.".to_string(),
            ],
            ActiveSkill::Rapidfire => vec![
                "Concentrate deeply. Enemies around ".to_string(),
                "you move 50% slower briefly. Gain".to_string(),
                format!("{:.1}% attack speed and unlimited", skill_power * 80.0),
                "ammo for the duration.".to_string(),
            ],
            ActiveSkill::PiercingStar => vec![
                "Throw a large, piercing throwing".to_string(),
                "star that travels in a line, dealing".to_string(),
                format!("{:.1}% damage.", skill_power * 150.0),
            ],
            ActiveSkill::TripleThrow => vec![
                "Throw three small throwing stars".to_string(),
                "in a cone shape in front, dealing".to_string(),
                format!("{:.1}% damage each.", skill_power * 135.0),
            ],
            ActiveSkill::Fury => vec![
                "Enter fury for a short duration,".to_string(),
                "throwing kunai rapidly at enemies".to_string(),
                format!("around you, dealing {:.1}% damage.", skill_power * 165.0),
                "Kunai count scales with attack speed.".to_string(),
            ],
            ActiveSkill::LaserBeam => vec![
                "Channel a powerful laser beam that deals".to_string(),
                format!(
                    "{:.1}% damage rapidly to enemies in front.",
                    skill_power * 60.0
                ),
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
                        sprint_cooldown_timer: Timer::from_seconds(
                            ActiveSkill::Sprint.get_base_cooldown(),
                            TimerMode::Once,
                        )
                        .tick(Duration::from_secs(99))
                        .clone(),
                        speed_bonus: 1.6,
                    });
            }
            ActiveSkill::SprintLunge => {
                commands
                    .entity(entity)
                    .insert(crate::player::rogue_skills::LungeState {
                        lunge_cooldown_timer: Timer::from_seconds(
                            ActiveSkill::SprintLunge.get_base_cooldown(),
                            TimerMode::Once,
                        )
                        .tick(Duration::from_secs(99))
                        .clone(),
                        lunge_duration: Timer::from_seconds(0.42, TimerMode::Once),
                        lunge_speed: 9.5,
                    });
            }
            ActiveSkill::Teleport => {
                commands
                    .entity(entity)
                    .insert(crate::player::mage_skills::TeleportState {
                        just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                        cooldown_timer: Timer::from_seconds(1.5, TimerMode::Once)
                            .tick(Duration::from_secs(99))
                            .clone(),
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
                        cooldown_timer: Timer::from_seconds(5.2, TimerMode::Once)
                            .tick(Duration::from_secs(99))
                            .clone(),
                        spear_timer: Timer::from_seconds(0.5, TimerMode::Once),
                    });
            }
            ActiveSkill::Stealth => {
                let cooldown = ActiveSkill::Stealth.get_base_cooldown();
                commands.entity(entity).insert(StealthState {
                    duration: Timer::from_seconds(2.0, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
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
                let cooldown = ActiveSkill::FirePillar.get_base_cooldown();
                commands.entity(entity).insert(FirePillarState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    hit_clear_timer: Timer::from_seconds(0.75, TimerMode::Repeating),
                });
            }
            ActiveSkill::Heal => {
                let cooldown = ActiveSkill::Heal.get_base_cooldown();
                commands.entity(entity).insert(HealSkillState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                });
            }
            ActiveSkill::Buckshot => {
                let cooldown = ActiveSkill::Buckshot.get_base_cooldown();
                commands.entity(entity).insert(BuckshotSkillState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                });
            }
            ActiveSkill::IceWall => {
                let cooldown = ActiveSkill::IceWall.get_base_cooldown();
                commands.entity(entity).insert(IceWallSkillState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                });
            }
            ActiveSkill::DruidTree => {
                let cooldown = ActiveSkill::DruidTree.get_base_cooldown();
                commands.entity(entity).insert(DruidTreeSkillState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                });
            }
            ActiveSkill::Shout => {
                let cooldown = ActiveSkill::Shout.get_base_cooldown();
                commands.entity(entity).insert(ShoutSkillState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                });
            }
            ActiveSkill::PiercingStar => {
                let cooldown = ActiveSkill::PiercingStar.get_base_cooldown();
                commands.entity(entity).insert(PiercingStarSkillState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                });
            }
            ActiveSkill::LaserBeam => {
                let cooldown = ActiveSkill::LaserBeam.get_base_cooldown();
                commands.entity(entity).insert(LaserBeamState {
                    cooldown_timer: Timer::from_seconds(cooldown, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    hit_clear_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                });
            }
            // New skills - placeholder implementations (default to Roll behavior for now)
            ActiveSkill::Roll
            | ActiveSkill::Lightning
            | ActiveSkill::DaggerThrow
            | ActiveSkill::DaggerSlash
            | ActiveSkill::TripleThrow
            | ActiveSkill::Fury
            | ActiveSkill::SpinAttack
            | ActiveSkill::Bomb => {}
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
    FullStomach,                //jam
    SkillEcho,                  // placeholder
    ReinforcedArmor,            // Scale
    SkillPower,                 // placeholder - increases skill effectiveness
    CritSkillCooldownReduction, // reduces class skill cooldown on crit

    // On-Attack Triggers
    WaveAttack,  // hero sword
    FrailStacks, // skull
    SlowStacks,  // sea shell

    AntFarm,    // ant terrarium
    StoneTooth, // orbiting stone
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
    SkillManaRegen,     // Using a skill has a 20% chance to trigger mana regen
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
            Heirloom::OnHitEcho => 5,
            Heirloom::ChanceToProcExtraAttack => 5,
            // Heirloom::IncreaseProjectileCount => 5,
            Heirloom::IceStaffAoE => 5,
            Heirloom::FrozenAoE => 5,
            Heirloom::IceStaffFloor => 3,
            Heirloom::ViralVenum => 5,
            Heirloom::HealEcho => 5,
            Heirloom::SkillEcho => 5,
            Heirloom::WaveAttack => 5,
            Heirloom::AntFarm => 4,
            Heirloom::StoneTooth => 4,
            Heirloom::Reaper => 5,
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
            Heirloom::FrailStacks => "Skull".to_string(),
            Heirloom::SlowStacks => "Sea Shell".to_string(),
            Heirloom::AntFarm => "Ant Farm".to_string(),
            Heirloom::StoneTooth => "Stone Tooth".to_string(),
            Heirloom::Reaper => "Reaper".to_string(),
            Heirloom::PoisonStacks => "Grandma's Recipe".to_string(),
            Heirloom::LethalBlow => "Deadly Mushroom".to_string(),
            Heirloom::SkillEcho => "Dragon Eye".to_string(),
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
            Heirloom::OnHitEcho => "Ancient Fossil ".to_string(),
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
            Heirloom::FullStomach => "Jam".to_string(),

            Heirloom::ReinforcedArmor => "Scales".to_string(),
            Heirloom::ChaosBoost => "Cursed Mask".to_string(),

            // New heirlooms
            Heirloom::MaxHPHunt => "Ripe Tomato".to_string(),
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
            Heirloom::ItemPickupRadius => "Magnet".to_string(),
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
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Heirloom::None => vec!["No Heirloom".to_string()],
            Heirloom::Chest => vec!["Gain a Loot Chest".to_string()],
            Heirloom::CritChance => vec![
                "Gain +7% Critical".to_string(),
                "Chance, ".to_string(),
                "permanantly.".to_string(),
            ],
            Heirloom::CritDamage => vec![
                "Gain +15% Critical".to_string(),
                "Damage, permanently".to_string(),
            ],
            Heirloom::SkillCDReduction => {
                vec!["Reduce skill".to_string(), "cooldowns by 15%.".to_string()]
            }
            Heirloom::LoadedDice => {
                vec!["Gain +7 Luck,".to_string(), "permanently.".to_string()]
            }
            Heirloom::Health => vec!["Gain +25 Health,".to_string(), "permanently.".to_string()],
            Heirloom::Mana => vec!["Gain +25 Mana,".to_string(), "permanently.".to_string()],
            Heirloom::Shield => vec!["Gain +10 Shield,".to_string(), "permanently.".to_string()],
            Heirloom::Speed => vec!["Gain +10 Speed,".to_string(), "permanently.".to_string()],
            Heirloom::Thorns => vec!["Gain +15 Thorns, ".to_string(), "permanently.".to_string()],
            Heirloom::Lifesteal => {
                vec![
                    "Gain +2% Lifesteal,".to_string(),
                    "permanently.".to_string(),
                ]
            }
            Heirloom::AttackSpeed => vec![
                "Gain +15% Attack".to_string(),
                "Speed, permanently. ".to_string(),
            ],
            Heirloom::XPGain => vec!["Gain +7% XP".to_string(), "permanently. ".to_string()],
            Heirloom::CreditCard => vec![
                "Gain 1 Coin when".to_string(),
                "you use your active".to_string(),
                "skill.".to_string(),
            ],
            Heirloom::DodgeChance => vec![
                "Gain +7% Dodge".to_string(),
                "Chance,".to_string(),
                "permanently.".to_string(),
            ],
            Heirloom::Gigantify => vec![
                "Your Attacks gain".to_string(),
                "+10% Size".to_string(),
                "permanently.".to_string(),
            ],

            Heirloom::WaveAttack => vec![
                "Your Attacks have".to_string(),
                "a chance to send a".to_string(),
                "sonic wave attack".to_string(),
                "that travels a".to_string(),
                "short distance.".to_string(),
                format!("Costs {} mana.", Heirloom::WaveAttack.get_mana_cost()),
            ],
            Heirloom::FrailStacks => vec![
                "Your Attacks have".to_string(),
                "a chance to apply".to_string(),
                "a Frail stack that".to_string(),
                "gives +10% Damage".to_string(),
                "on hits.".to_string(),
            ],
            Heirloom::SlowStacks => vec![
                "Your Attacks have".to_string(),
                "a chance to apply".to_string(),
                "a Slow stack to".to_string(),
                "enemies, reducing".to_string(),
                "speed by 15%.".to_string(),
            ],
            Heirloom::AntFarm => vec![
                "Spawn ants that".to_string(),
                "rush towards".to_string(),
                "enemies, dealing".to_string(),
                "damage.".to_string(),
                format!("Costs {} mana.", Heirloom::AntFarm.get_mana_cost()),
            ],
            Heirloom::StoneTooth => vec![
                "Spawn rocks that".to_string(),
                "orbit you and deal".to_string(),
                "damage to enemies".to_string(),
                "they hit.".to_string(),
                format!("Costs {} mana.", Heirloom::StoneTooth.get_mana_cost()),
            ],
            Heirloom::Reaper => vec![
                "Soul fragments".to_string(),
                "chase enemies".to_string(),
                "after each kill,".to_string(),
                "damaging them.".to_string(),
                format!("Costs {} mana.", Heirloom::Reaper.get_mana_cost()),
            ],
            Heirloom::SkillEcho => {
                vec![
                    "Using a skill triggers".to_string(),
                    "an echo that damages".to_string(),
                    "enemies around you".to_string(),
                    format!("Costs {} mana.", Heirloom::SkillEcho.get_mana_cost()),
                ]
            }
            Heirloom::PoisonStacks => vec![
                "Your Attacks have".to_string(),
                "a chance to apply".to_string(),
                "Poison to enemies.".to_string(),
                "Poisoned enemies".to_string(),
                "lose health over".to_string(),
                "time.".to_string(),
            ],
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
                vec!["Increases skill".to_string(), "power by +15%".to_string()]
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
            Heirloom::TeleportManaRegen => vec![
                "Attacking right".to_string(),
                "after a Teleport".to_string(),
                "triggers mana".to_string(),
                "regeneration.".to_string(),
            ],
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

            Heirloom::IceStaffAoE => vec![
                "Your Attacks have".to_string(),
                "a 3% chance to ".to_string(),
                "trigger an ice".to_string(),
                "explosion that".to_string(),
                "damages enemies. ".to_string(),
                format!("Costs {} mana.", Heirloom::IceStaffAoE.get_mana_cost()),
            ],
            Heirloom::BowArrowSpeed => {
                vec!["Your Projectiles".to_string(), "move faster.".to_string()]
            }
            Heirloom::Attack => vec!["Gain +10% Damage,".to_string(), "permanently.".to_string()],
            Heirloom::Defence => vec!["Gain +10 Defence,".to_string(), "permanently.".to_string()],
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
            Heirloom::ParryEcho => vec![
                "A successful".to_string(),
                "parry triggers".to_string(),
                "an echo that".to_string(),
                "damages enemies".to_string(),
                "around you.".to_string(),
            ],
            Heirloom::DaggerCombo => vec![
                "Attacks chained".to_string(),
                "together build ".to_string(),
                "Combo, increasing ".to_string(),
                "your critical ".to_string(),
                "damage.".to_string(),
            ],
            Heirloom::HPRegen => vec![
                "Gain +5 Health".to_string(),
                "regeneration, ".to_string(),
                "permanently.".to_string(),
            ],
            Heirloom::MPRegen => vec![
                "Gain +5 Mana ".to_string(),
                "regeneration,".to_string(),
                "permanently.".to_string(),
            ],
            Heirloom::HPRegenCooldown => vec![
                "Your Health".to_string(),
                "regeneration".to_string(),
                "cooldown is.".to_string(),
                "reduced.".to_string(),
            ],
            Heirloom::MPRegenCooldown => vec![
                "Your Mana".to_string(),
                "regeneration".to_string(),
                "cooldown is.".to_string(),
                "reduced.".to_string(),
            ],
            Heirloom::OnHitEcho => vec![
                "After taking ".to_string(),
                "damage, trigger ".to_string(),
                "an echo that".to_string(),
                "damages enemies ".to_string(),
                "around you.".to_string(),
                format!("Costs {} mana.", Heirloom::OnHitEcho.get_mana_cost()),
            ],

            Heirloom::Knockback => vec![
                "Your attacks".to_string(),
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

            Heirloom::FrozenAoE => vec![
                "Killing a frozen".to_string(),
                "enemy has a 25% chance".to_string(),
                "to trigger an ice".to_string(),
                "explosion.".to_string(),
                "+25% freeze chance.".to_string(),
                format!("Costs {} mana.", Heirloom::FrozenAoE.get_mana_cost()),
            ],
            Heirloom::IceStaffFloor => vec![
                "Your Attacks have".to_string(),
                "a chance to leave".to_string(),
                "a trail of ice that".to_string(),
                "damages enemies. ".to_string(),
                "+25% freeze chance.".to_string(),
                format!("Costs {} mana.", Heirloom::IceStaffFloor.get_mana_cost()),
            ],
            Heirloom::FrozenCrit => vec![
                "Attacking frozen".to_string(),
                "enemies gives you".to_string(),
                "+15% critical hit".to_string(),
                "chance.".to_string(),
                "+25% freeze chance.".to_string(),
            ],
            Heirloom::MPBarDMG => vec![
                "Mana regeneration".to_string(),
                "is stored, adding".to_string(),
                "bonus damage on".to_string(),
                "your next attack.".to_string(),
            ],
            Heirloom::MPBarCrit => vec![
                "Your staff's attacks".to_string(),
                "gain +10% critical".to_string(),
                "hit chance if your".to_string(),
                "mana bar is full.".to_string(),
            ],
            Heirloom::FrozenMPRegen => vec![
                "Killing a frozen".to_string(),
                "enemy has a 20%".to_string(),
                "chance to trigger".to_string(),
                "mana regeneration.".to_string(),
                "+25% freeze chance.".to_string(),
            ],
            Heirloom::DodgeCrit => vec![
                "Dodging grants".to_string(),
                "+30% atk speed,".to_string(),
                "+30 speed, next".to_string(),
                "hit does 2x dmg.".to_string(),
            ],
            Heirloom::PoisonDuration => vec![
                "Your poison effect".to_string(),
                "lasts longer.".to_string(),
                "+25% poison chance.".to_string(),
            ],
            Heirloom::PoisonStrength => vec![
                "Your poison effect".to_string(),
                "does more damage.".to_string(),
                "+25% poison chance.".to_string(),
            ],
            Heirloom::ViralVenum => vec![
                "Killing a poisoned".to_string(),
                "enemy spreads it's".to_string(),
                "poison to nearby".to_string(),
                "enemies.".to_string(),
                "+25% poison chance.".to_string(),
                format!("Costs {} mana.", Heirloom::ViralVenum.get_mana_cost()),
            ],
            Heirloom::HealEcho => vec![
                "Healing has a 25%".to_string(),
                "chance to trigger an".to_string(),
                "echo that damages".to_string(),
                "enemies around you.".to_string(),
                "+20 Health regen.".to_string(),
                format!("Costs {} mana.", Heirloom::HealEcho.get_mana_cost()),
            ],
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
                vec!["Every 5 kills".to_string(), "gain +1 Max HP.".to_string()]
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
            Heirloom::RegenLifesteal => vec![
                "Lose 5 HP Regen,".to_string(),
                "gain 5% Lifesteal.".to_string(),
            ],
            Heirloom::StandStill => vec![
                "Standing still".to_string(),
                "increases damage".to_string(),
                "rapidly.".to_string(),
            ],
            Heirloom::ThornArmor => vec![
                "Gain +10 Thorns".to_string(),
                "for every 10".to_string(),
                "Defence you have.".to_string(),
                "Gain +10 Defence.".to_string(),
            ],
            Heirloom::LifestealCoins => vec![
                "Lifesteal triggers".to_string(),
                "give you a coin.".to_string(),
                "Gain +5% Lifesteal.".to_string(),
            ],

            // Wave 2 heirlooms
            Heirloom::CoinHeal => vec![
                "Picking up coins".to_string(),
                "has a 25% chance".to_string(),
                "to heal 1 HP.".to_string(),
            ],
            Heirloom::CrateBreakDamage => vec![
                "Breaking crates".to_string(),
                "permanently gives".to_string(),
                "+1% damage.".to_string(),
            ],
            Heirloom::TomeDoubleUpgrade => vec![
                "Upgrade Tomes".to_string(),
                "level up gear".to_string(),
                "an extra time.".to_string(),
            ],
            Heirloom::CritHeal => vec![
                "Critical hits".to_string(),
                "have a 25% chance".to_string(),
                "to heal 1 HP.".to_string(),
            ],
            Heirloom::LowHPDamage => vec![
                "Deal more damage".to_string(),
                "the lower your".to_string(),
                "HP is (up to".to_string(),
                "+75% at 0 HP).".to_string(),
            ],
            Heirloom::ChaosStats => vec![
                "+2 Chaos. +10 HP,".to_string(),
                "+10 MP, +10% dmg,".to_string(),
                "+10 def, +10% crit".to_string(),
                "+10 spd, +10 dodge".to_string(),
            ],
            Heirloom::ManaOrbs => vec![
                "Mana Orbs drop".to_string(),
                "from mobs with a.".to_string(),
                "+10% chance. They".to_string(),
                "trigger mana regen.".to_string(),
            ],
            Heirloom::ManaOrbAttack => vec![
                "Mana regeneration".to_string(),
                "shoots a mana orb".to_string(),
                "at an enemy. It does".to_string(),
                "damage equal to the".to_string(),
            ],
            Heirloom::ItemPickupRadius => vec![
                "Increases item".to_string(),
                "pickup radius by".to_string(),
                "+25%.".to_string(),
            ],
            Heirloom::MagnetPull => vec![
                "Periodically pulls".to_string(),
                "all item drops on".to_string(),
                "the map to you.".to_string(),
                "Cooldown reduces".to_string(),
                "with more copies.".to_string(),
            ],

            // Thorns build heirlooms
            Heirloom::ThornsSpikes => vec![
                "Taking damage shoots".to_string(),
                "out 2 spikes. Damage".to_string(),
                "scales with thorns".to_string(),
                "stat.".to_string(),
            ],
            Heirloom::ThornsOnDamage => vec![
                "Gain +1 Thorns each".to_string(),
                "time you take damage".to_string(),
            ],
            Heirloom::ThornsLifesteal => vec![
                "Your thorns damage".to_string(),
                "has +25% lifesteal.".to_string(),
            ],

            // Lightning strikes archetype
            Heirloom::CoinLightning => vec![
                "Picking up coins".to_string(),
                "spawns a lightning".to_string(),
                "strike on a random".to_string(),
                "nearby enemy.".to_string(),
                format!("Costs {} mana.", 5),
            ],
            Heirloom::KillLightning => vec![
                "Killing an enemy".to_string(),
                "has a 5% chance".to_string(),
                "to spawn a lightning".to_string(),
                "strike on a random".to_string(),
                "nearby enemy.".to_string(),
                format!("Costs {} mana.", 5),
            ],
            Heirloom::ManaRegenLightning => vec![
                "Mana regeneration".to_string(),
                "has a 20% chance to".to_string(),
                "spawn a lightning".to_string(),
                "strike on a random".to_string(),
                "nearby enemy.".to_string(),
                format!("Costs {} mana.", 5),
            ],
            Heirloom::ManaRegenPoison => vec![
                "Every time you".to_string(),
                "regenerate 100 mana,".to_string(),
                "apply a poison".to_string(),
                "stack to all".to_string(),
                "enemies.".to_string(),
            ],
            Heirloom::SkillManaRegen => vec![
                "Using a skill has".to_string(),
                "a 20% chance to".to_string(),
                "trigger mana".to_string(),
                "regeneration.".to_string(),
            ],
        }
    }
    pub fn get_instant_drop(&self) -> Option<(WorldObject, usize)> {
        match self {
            Heirloom::Chest => Some((WorldObject::ChestBlock, 1)),
            _ => None,
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
                // This is kept for compatibility but TeleportState now uses SkillChargeTracker
                commands.entity(entity).insert(TeleportState {
                    just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.5, TimerMode::Once),
                    timer: Timer::from_seconds(0.06, TimerMode::Once),
                });
            }
            &Heirloom::DaggerCombo => {
                commands.entity(entity).insert(ComboCounter {
                    counter: 0,
                    reset_timer: Timer::from_seconds(1., TimerMode::Once),
                });
            }
            Heirloom::AntFarm => {
                commands
                    .entity(entity)
                    .insert(crate::player::combat_heirlooms::AntFarmState::default());
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

            _ => {}
        }
    }
    pub fn get_ui_element(&self, rarity: HeirloomRarity) -> UIElement {
        match rarity {
            HeirloomRarity::Common => UIElement::SkillChoice,
            HeirloomRarity::Uncommon => UIElement::SkillChoiceRogue,
            HeirloomRarity::Rare => UIElement::SkillChoiceMagic,
            HeirloomRarity::Legendary => UIElement::SkillChoiceMelee,
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
            Heirloom::WaveAttack => obj.is_melee_weapon(),
            Heirloom::FrailStacks => obj.is_melee_weapon(),
            Heirloom::LethalBlow => obj.is_melee_weapon(),
            _ => true,
        }
    }
}

pub struct ActiveSkillUsedEvent {
    pub slot: usize,
    pub cooldown: f32,
}

/// Tracks skill charges for slot 1 (class skill, not Roll)
/// Charges allow immediate skill activation without waiting for cooldown
/// This is the shared data structure used by both slot trackers
#[derive(Clone, Debug)]
pub struct SkillChargeTracker {
    pub current_charges: u32,
    pub max_charges: u32,
    pub cooldown_timer: Timer,
    pub base_cooldown: f32,
    pub tracked_skill: ActiveSkill, // Track which skill this tracker is for
}

/// Charge tracker for slot 1 (active_skill_slot_1)
#[derive(Component, Clone, Debug)]
pub struct Slot1ChargeTracker(pub SkillChargeTracker);

/// Charge tracker for slot 2 (active_skill_slot_2)
#[derive(Component, Clone, Debug)]
pub struct Slot2ChargeTracker(pub SkillChargeTracker);

/// Charge tracker for slot 3 (active_skill_slot_3)
#[derive(Component, Clone, Debug)]
pub struct Slot3ChargeTracker(pub SkillChargeTracker);

/// Charge tracker for slot 4 (active_skill_slot_4)
#[derive(Component, Clone, Debug)]
pub struct Slot4ChargeTracker(pub SkillChargeTracker);

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Default, Debug, Serialize, Deserialize)]
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
            HeirloomRarity::Uncommon => Some(ItemGlow::Green),
            HeirloomRarity::Rare => Some(ItemGlow::Blue),
            HeirloomRarity::Legendary => Some(ItemGlow::Red),
        }
    }

    pub fn get_color(&self) -> Color {
        use crate::colors::{LIGHT_BLUE, LIGHT_GREY, LIGHT_RED, UNCOMMON_GREEN};
        match self {
            HeirloomRarity::Common => LIGHT_GREY,
            HeirloomRarity::Uncommon => UNCOMMON_GREEN,
            HeirloomRarity::Rare => LIGHT_BLUE,
            HeirloomRarity::Legendary => LIGHT_RED,
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
}

impl Default for HeirloomChoiceQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            active_heirloom_limbo: None,
            pool: vec![
                HeirloomChoiceState::new(Heirloom::Defence, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::ManaOrbs, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::ManaOrbAttack, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Attack, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Gigantify, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Chest, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::HPRegen, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::XPGain, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::HPRegenCooldown, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::MPRegenCooldown, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::MPRegen, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::DodgeCrit, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::Knockback, HeirloomRarity::Common),
                // HeirloomChoiceState::new(Heirloom::DiscountMP, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::OnHitEcho, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::HealEcho, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::CritChance, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::CritDamage, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::FrailStacks, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Health, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Mana, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Shield, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Lifesteal, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Thorns, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Speed, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::AttackSpeed, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::WaveAttack, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::MPBarDMG, HeirloomRarity::Rare),
                // HeirloomChoiceState::new(Heirloom::MPBarCrit, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::LethalBlow, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::DodgeChance, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::SlowStacks, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::AntFarm, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::FrozenAoE, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::FrozenCrit, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::FrozenMPRegen, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::IceStaffFloor, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::PoisonStacks, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::PoisonDuration, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::PoisonStrength, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::ViralVenum, HeirloomRarity::Legendary),
                // HeirloomChoiceState::new(Heirloom::ChanceToProcExtraAttack, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::IncreaseProjectileCount, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::BowArrowSpeed, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::IceStaffAoE, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::FullStomach, HeirloomRarity::Uncommon),
                // HeirloomChoiceState::new(Heirloom::ReinforcedArmor, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::DaggerCombo, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::StoneTooth, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Reaper, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::ChaosBoost, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::SkillCDReduction, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::LoadedDice, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::SkillChargeIncrease, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::SkillEcho, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::SkillPower, HeirloomRarity::Common),
                HeirloomChoiceState::new(
                    Heirloom::CritSkillCooldownReduction,
                    HeirloomRarity::Rare,
                ),
                HeirloomChoiceState::new(Heirloom::CreditCard, HeirloomRarity::Legendary),
                // New heirlooms
                HeirloomChoiceState::new(Heirloom::MaxHPHunt, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::MaxHPDamage, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::GoldIntoDamage, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::DeathDefiance, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::RegenLifesteal, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::StandStill, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::ThornArmor, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::LifestealCoins, HeirloomRarity::Legendary),
                // Wave 2 heirlooms
                HeirloomChoiceState::new(Heirloom::CoinHeal, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::CrateBreakDamage, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::TomeDoubleUpgrade, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::CritHeal, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::LowHPDamage, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::ChaosStats, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::ItemPickupRadius, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::MagnetPull, HeirloomRarity::Rare),
                // Thorns build heirlooms
                HeirloomChoiceState::new(Heirloom::ThornsSpikes, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::ThornsOnDamage, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::ThornsLifesteal, HeirloomRarity::Uncommon),
                // Lightning strikes archetype
                // HeirloomChoiceState::new(Heirloom::CoinLightning, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::CoinLightning, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::KillLightning, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::ManaRegenLightning, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::ManaRegenPoison, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::SkillManaRegen, HeirloomRarity::Uncommon),
            ],
            banned: HashSet::default(),
        }
    }
}
impl HeirloomChoiceQueue {
    pub fn add_new_skills_after_levelup(
        &mut self,
        rng: &mut rand::rngs::ThreadRng,
        loot_bonus: i32,
    ) {
        //only push if queue is empty
        if self.queue.is_empty() {
            let mut new_skills: [HeirloomChoiceState; 3] = Default::default();
            let mut add_back_to_pool: Vec<HeirloomChoiceState> = vec![];
            for i in 0..3 {
                let rarity = HeirloomChoiceQueue::gen_rarity(rng, loot_bonus);
                if let Some(picked_skill) = self.get_skill_of_rarity(rarity.clone(), rng, &|s| {
                    // Only check slots that have been explicitly set (0..i)
                    // This avoids the issue where Default::default() initializes all slots with CritChance
                    !new_skills[0..i].iter().any(|existing| existing == s)
                }) {
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

            self.queue.push(new_skills.clone());
        }
    }
    pub fn get_skill_of_rarity(
        &self,
        rarity: HeirloomRarity,
        rng: &mut rand::rngs::ThreadRng,
        filter: &dyn Fn(&HeirloomChoiceState) -> bool,
    ) -> Option<HeirloomChoiceState> {
        let filtered: Vec<_> = self
            .pool
            .iter()
            .filter(|x| {
                let matches_rarity = x.rarity == rarity;
                let passes_filter = filter(x);
                let not_banned = !self.banned.contains(&x.heirloom);
                matches_rarity && passes_filter && not_banned && x.heirloom != Heirloom::default()
            })
            .collect();
        // Convert back to owned values for choose
        let owned_filtered: Vec<HeirloomChoiceState> =
            filtered.iter().map(|x| (*x).clone()).collect();
        owned_filtered.as_slice().choose(rng).cloned()
    }
    pub fn gen_rarity(rng: &mut rand::rngs::ThreadRng, loot_bonus: i32) -> HeirloomRarity {
        // Base probabilities: Common 61%, Uncommon 23%, Rare 13%, Legendary 3%
        // Loot bonus increases higher rarity chances
        // Formula: each point of loot increases higher rarity chances by shifting thresholds
        // Each point of loot: +0.1% legendary, +0.2% rare, +0.1% uncommon
        // Since we roll 0-99 (100 values), each 1% = 1.0 threshold point

        let loot_bonus_f = loot_bonus as f32;
        // Calculate adjusted thresholds (lower threshold = more chance for that rarity)
        let legendary_threshold = (99.0 - loot_bonus_f * 0.08).max(0.0);
        let rare_threshold = (92.0 - loot_bonus_f * 0.12).max(0.0);
        let uncommon_threshold = (68.0 - loot_bonus_f * 0.1).max(0.0);
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
        proto_commands: &mut ProtoCommands,
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
            self.pool.push(choice.clone());
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
            proto_commands.spawn_item_from_proto(
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
        proto_commands: &mut ProtoCommands,
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
            proto_commands.spawn_item_from_proto(
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
    ) {
        if self.queue.is_empty() {
            return;
        }
        let old_skill = self.queue[0][slot].clone();
        let rarity = HeirloomChoiceQueue::gen_rarity(rng, loot_bonus);
        if let Some(picked_skill) =
            self.get_skill_of_rarity(rarity.clone(), rng, &|s| !self.queue[0].contains(s))
        {
            if picked_skill.is_one_time_heirloom {
                self.pool.retain(|x| x != &picked_skill);
            }

            self.pool.push(old_skill);
            self.queue[0][slot] = picked_skill;
        }
    }

    pub fn banish_slot(&mut self, slot: usize) -> Option<HeirloomChoiceState> {
        if self.queue.is_empty() {
            return None;
        }
        let mut choices = self.queue.remove(0);
        let banned_choice = choices[slot].clone();
        choices[slot] = HeirloomChoiceState::default();
        self.queue.push(choices.clone());
        self.banned.insert(banned_choice.heirloom.clone());
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
            active_skill_slot_1: Some(ActiveSkillChoiceState::new(
                ActiveSkill::Roll,
                HeirloomRarity::Common,
            )),
            active_skill_slot_2: Some(ActiveSkillChoiceState::new(
                ActiveSkill::Roll,
                HeirloomRarity::Common,
            )),
            active_skill_slot_3: Some(ActiveSkillChoiceState::new(
                ActiveSkill::Roll,
                HeirloomRarity::Common,
            )),
            active_skill_slot_4: None, // Bonus slot - unlocked by blessings
        }
    }
}

impl PlayerSkills {
    pub fn has(&self, heirloom: Heirloom) -> bool {
        self.heirlooms.iter().any(|h| h.heirloom == heirloom)
    }
    pub fn skill_cooldown_multiplier(&self) -> f32 {
        // 10% multiplicative reduction per item: 0.9^count
        let count = self.get_count(Heirloom::SkillCDReduction).max(0) as i32;
        (0..count).fold(1.0f32, |acc, _| acc * 0.85)
    }
    pub fn skill_extra_charges(&self) -> u32 {
        self.get_count(Heirloom::SkillChargeIncrease).max(0) as u32
    }
    pub fn calculate_freeze_chance(&self) -> f64 {
        let mut chance = 0.0;
        let freeze_skills = vec![
            Heirloom::FrozenAoE,
            Heirloom::IceStaffFloor,
            Heirloom::FrozenCrit,
            Heirloom::FrozenMPRegen,
            Heirloom::SlowStacks,
        ];
        for skill in freeze_skills.iter() {
            chance += self.get_count(skill.clone()) as f64 * 0.25;
        }
        chance
    }
    pub fn calculate_poison_chance(&self) -> f64 {
        let mut chance = 0.0;
        let poison_skills = vec![
            Heirloom::PoisonDuration,
            Heirloom::PoisonStrength,
            Heirloom::ViralVenum,
            Heirloom::PoisonStacks,
        ];
        for skill in poison_skills.iter() {
            chance += self.get_count(skill.clone()) as f64 * 0.25;
        }
        chance
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
