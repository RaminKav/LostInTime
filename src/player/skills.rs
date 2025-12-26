use std::time::Duration;

use bevy::prelude::*;
use bevy_aseprite::Aseprite;
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, seq::SliceRandom, Rng};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use strum_macros::{Display, EnumIter};

use crate::{
    animations::player_sprite::{
        PlayerBlueAseprite, PlayerGreenAseprite, PlayerGreyAseprite, PlayerRedAseprite,
        PlayerSpriteHandles,
    },
    attributes::{AttributeQuality, AttributeValue, ItemAttributes, ItemGlow},
    custom_commands::CommandsExt,
    item::{
        item_upgrades::{ArrowSpeedUpgrade, BowUpgradeSpread, ClawUpgradeMultiThrow},
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

    Warrior, //Sword
    Paladin, //hammer
    Knight,  //Spear

    Thief,      //claw
    Gunslinger, //gun

    IceMage,  //ice staff
    FireMage, //fire staff
    Wizard,   //lightning
    Druid,    // whip

    Rogue,  //dagger
    Archer, //bow
    Kid,    // blowdart
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
            SkillClass::Paladin => WorldObject::RedCape,
            SkillClass::Knight => WorldObject::RedCape,

            SkillClass::Thief => WorldObject::GreyCape,
            SkillClass::Gunslinger => WorldObject::GreyCape,

            SkillClass::FireMage => WorldObject::BlueCape,
            SkillClass::IceMage => WorldObject::BlueCape,
            SkillClass::Wizard => WorldObject::BlueCape,
            SkillClass::Druid => WorldObject::BlueCape,

            SkillClass::Rogue => WorldObject::GreenCape,
            SkillClass::Archer => WorldObject::GreenCape,
            SkillClass::Kid => WorldObject::GreenCape,
            _ => WorldObject::GreyCape,
        }
    }
    pub fn get_anim_data(&self, sprites: &PlayerSpriteHandles) -> (Handle<Aseprite>, &str) {
        match self {
            SkillClass::Warrior => (sprites.red.clone(), PlayerRedAseprite::tags::IDLE_FRONT),
            SkillClass::Paladin => (sprites.red.clone(), PlayerRedAseprite::tags::IDLE_FRONT),
            SkillClass::Knight => (sprites.red.clone(), PlayerRedAseprite::tags::IDLE_FRONT),

            SkillClass::Thief => (sprites.grey.clone(), PlayerGreyAseprite::tags::IDLE_FRONT),
            SkillClass::Gunslinger => (sprites.grey.clone(), PlayerGreyAseprite::tags::IDLE_FRONT),

            SkillClass::FireMage => (sprites.blue.clone(), PlayerBlueAseprite::tags::IDLE_FRONT),
            SkillClass::IceMage => (sprites.blue.clone(), PlayerBlueAseprite::tags::IDLE_FRONT),
            SkillClass::Wizard => (sprites.blue.clone(), PlayerBlueAseprite::tags::IDLE_FRONT),
            SkillClass::Druid => (sprites.blue.clone(), PlayerBlueAseprite::tags::IDLE_FRONT),

            SkillClass::Rogue => (sprites.green.clone(), PlayerGreenAseprite::tags::IDLE_FRONT),
            SkillClass::Archer => (sprites.green.clone(), PlayerGreenAseprite::tags::IDLE_FRONT),
            SkillClass::Kid => (sprites.green.clone(), PlayerGreenAseprite::tags::IDLE_FRONT),
            _ => (sprites.grey.clone(), PlayerGreyAseprite::tags::IDLE_FRONT),
        }
    }
    pub fn get_starting_wep(&self) -> WorldObject {
        match self {
            SkillClass::Warrior => WorldObject::Sword,
            SkillClass::Paladin => WorldObject::Hammer,
            SkillClass::Knight => WorldObject::Spear,
            SkillClass::Thief => WorldObject::Claw,
            SkillClass::Rogue => WorldObject::Dagger,
            SkillClass::Gunslinger => WorldObject::Gun,
            SkillClass::FireMage => WorldObject::FireStaff,
            SkillClass::IceMage => WorldObject::IceStaff,
            SkillClass::Wizard => WorldObject::BasicStaff,
            SkillClass::Druid => WorldObject::MagicWhip,
            SkillClass::Archer => WorldObject::WoodBow,
            SkillClass::Kid => WorldObject::Blowdart,
            _ => WorldObject::Sword,
        }
    }

    pub fn compute_cape_stats(&self, level: i32) -> ItemAttributes {
        let mut stats = ItemAttributes::default();
        let quality = if level > 10 {
            AttributeQuality::High
        } else if level >= 5 {
            AttributeQuality::Average
        } else {
            AttributeQuality::Low
        };
        match self {
            SkillClass::Warrior => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 6.) as i32, quality, 1.);
            }
            SkillClass::Paladin => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.health = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::Knight => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.defence = AttributeValue::new(level * 3, quality, 1.);
            }

            SkillClass::Rogue => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.speed = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Archer => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.crit_damage = AttributeValue::new(level * 4, quality, 1.);
            }
            SkillClass::Kid => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.dodge = AttributeValue::new(level * 3, quality, 1.);
            }

            SkillClass::Thief => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.crit_chance = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Gunslinger => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.attack_speed = AttributeValue::new(level * 3, quality, 1.);
            }

            SkillClass::FireMage => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.mana = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::IceMage => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.mana = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::Wizard => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.mana_regen =
                    AttributeValue::new(f32::floor(level as f32 * 0.5) as i32, quality, 1.);
            }
            SkillClass::Druid => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.) as i32, quality, 1.);
                stats.health_regen = AttributeValue::new(level * 1, quality, 1.);
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
}

impl ActiveSkill {
    /// Returns the base cooldown in seconds for this skill
    pub fn get_base_cooldown(&self) -> f32 {
        match self {
            ActiveSkill::Roll => 0.0, // Handled separately in player_move_inputs
            ActiveSkill::Parry => 1.2,
            ActiveSkill::ParrySpear => 12.,
            ActiveSkill::Sprint => 12.0,
            ActiveSkill::SprintLunge => 3.5,
            ActiveSkill::Teleport => 2.0,
            ActiveSkill::Stealth => 11.0,
            ActiveSkill::Rapidfire => 12.0,
            ActiveSkill::FirePillar => 12.0,
            ActiveSkill::Heal => 60.0,
            ActiveSkill::Buckshot => 6.0,
            ActiveSkill::IceWall => 10.0,
            ActiveSkill::DruidTree => 14.0,
            ActiveSkill::Shout => 7.0,
            ActiveSkill::PiercingStar => 8.0,
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
        }
    }

    pub fn get_desc(&self) -> Vec<String> {
        match self {
            ActiveSkill::Roll => vec!["Roll to dodge".to_string(), "attacks.".to_string()],
            ActiveSkill::Parry => vec![
                "Active: Time successfully".to_string(),
                "to Parry attacks ignore".to_string(),
                "damage and stunning.".to_string(),
            ],
            ActiveSkill::ParrySpear => vec![
                "Active: Spear Attack that".to_string(),
                "pulls, enemies towards ".to_string(),
                "the impact.".to_string(),
            ],
            ActiveSkill::Sprint => vec![
                "Active: Hold Sprint to".to_string(),
                "move 60% faster.".to_string(),
            ],
            ActiveSkill::SprintLunge => vec![
                "Active: dash through".to_string(),
                "enemies with a quick".to_string(),
                "lunge attack.".to_string(),
            ],
            ActiveSkill::Teleport => vec![
                "Active: Teleport a short".to_string(),
                "distance.".to_string(),
            ],
            ActiveSkill::Stealth => vec![
                "Active: Enter stealth".to_string(),
                "for 2 seconds, dodging".to_string(),
                "all damage.".to_string(),
            ],
            ActiveSkill::Rapidfire => vec![
                "Active: +80% attack".to_string(),
                "speed and unlimited ammo".to_string(),
                "for 3 seconds.".to_string(),
            ],
            ActiveSkill::FirePillar => {
                vec!["Active: Summon a ring".to_string(), "of fire.".to_string()]
            }
            ActiveSkill::Heal => vec![
                "Active: Heal yourself".to_string(),
                "for a moderate".to_string(),
                "amount.".to_string(),
            ],
            ActiveSkill::Buckshot => vec![
                "Active: Fire a shotgun".to_string(),
                "and bounce back.".to_string(),
            ],
            ActiveSkill::IceWall => {
                vec!["Active: Summon an".to_string(), "ice pillar.".to_string()]
            }
            ActiveSkill::DruidTree => vec![
                "Active: Summon a".to_string(),
                "tree dummy that".to_string(),
                "taunts enemies.".to_string(),
            ],
            ActiveSkill::Shout => vec![
                "Active: Release an AoE".to_string(),
                "burst of damage".to_string(),
                "around you.".to_string(),
            ],
            ActiveSkill::PiercingStar => vec![
                "Active: Throw a large".to_string(),
                "piercing star projectile.".to_string(),
            ],
        }
    }

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
                        timer: Timer::from_seconds(0.17, TimerMode::Once),
                        second_explosion_timer: Timer::from_seconds(0.4, TimerMode::Once),
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
            ActiveSkill::Roll => {}
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
}

#[derive(Clone, Eq, PartialEq, Hash, Default, Debug, Serialize, EnumIter, Display, Deserialize)]
pub enum Heirloom {
    // Passives
    #[default]
    CritChance, //tusk
    CritDamage,       //flint
    SkillCDReduction, // placeholder
    LoadedDice,       // increases luck by 7
    Health,           //red mushroom
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
    IncreaseProjectilCount,  //whip

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
}

impl Heirloom {
    pub fn get_title(&self) -> String {
        match self {
            Heirloom::CritChance => "Tusk".to_string(),
            Heirloom::CritDamage => "Flint".to_string(),
            Heirloom::SkillCDReduction => "Stanley".to_string(),
            Heirloom::LoadedDice => "Loaded Dice".to_string(),
            Heirloom::Health => "Weird Mushroom".to_string(),
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
            Heirloom::IncreaseProjectilCount => "Whip".to_string(),
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
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
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
            Heirloom::Shield => vec!["Gain +10 Shield,".to_string(), "permanently.".to_string()],
            Heirloom::Speed => vec!["Gain +15 Speed,".to_string(), "permanently.".to_string()],
            Heirloom::Thorns => vec!["Gain +15% Thorns, ".to_string(), "permanently.".to_string()],
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
            ],
            Heirloom::StoneTooth => vec![
                "Spawn rocks that".to_string(),
                "orbit you and deal".to_string(),
                "damage to enemies".to_string(),
                "they hit.".to_string(),
            ],
            Heirloom::Reaper => vec![
                "Soul fragments".to_string(),
                "chase enemies".to_string(),
                "after each kill,".to_string(),
                "damaging them.".to_string(),
            ],
            Heirloom::SkillEcho => {
                vec!["Using a skill".to_string(), "summons an echo.".to_string()]
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
                "2% chance to".to_string(),
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
                vec!["Skills gain +15%".to_string(), "effectivness.".to_string()]
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
            ],
            Heirloom::IncreaseProjectilCount => vec![
                "Increase all weapon".to_string(),
                "projectile count".to_string(),
                "by 1.".to_string(),
            ],

            Heirloom::IceStaffAoE => vec![
                "Your Attacks have".to_string(),
                "a chance to ".to_string(),
                "trigger an ice".to_string(),
                "explosion that".to_string(),
                "damages enemies. ".to_string(),
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
                "enemy triggers an".to_string(),
                "ice explosion that".to_string(),
                "damages enemies.".to_string(),
                "+25% freeze chance.".to_string(),
            ],
            Heirloom::IceStaffFloor => vec![
                "Your Attacks have".to_string(),
                "a chance to leave".to_string(),
                "a trail of ice that".to_string(),
                "damages enemies. ".to_string(),
                "+25% freeze chance.".to_string(),
            ],
            Heirloom::FrozenCrit => vec![
                "Attacking frozen".to_string(),
                "enemies gives you".to_string(),
                "+15% critical hit".to_string(),
                "chance.".to_string(),
                "+25% freeze chance.".to_string(),
            ],
            Heirloom::MPBarDMG => vec![
                "Your staff's attacks".to_string(),
                "gain +25% damage".to_string(),
                "if your mana bar".to_string(),
                "is full.".to_string(),
            ],
            Heirloom::MPBarCrit => vec![
                "Your staff's attacks".to_string(),
                "gain +10% critical".to_string(),
                "hit chance if your".to_string(),
                "mana bar is full.".to_string(),
            ],
            Heirloom::FrozenMPRegen => vec![
                "Killing a frozen".to_string(),
                "enemy triggers".to_string(),
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
            ],
            Heirloom::HealEcho => vec![
                "Healing triggers".to_string(),
                "an echo that".to_string(),
                "damages enemies ".to_string(),
                "around you.".to_string(),
                "+20 Health regen.".to_string(),
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
                "Gain +20% Thorns".to_string(),
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
        }
    }
    pub fn get_instant_drop(&self) -> Option<(WorldObject, usize)> {
        match self {
            Heirloom::Chest => Some((WorldObject::ChestBlock, 1)),
            _ => None,
        }
    }

    pub fn add_skill_components(
        &self,
        entity: Entity,
        commands: &mut Commands,
        skills: PlayerSkills,
    ) {
        match self {
            Heirloom::IncreaseProjectilCount => {
                commands.entity(entity).insert(ClawUpgradeMultiThrow(
                    Timer::from_seconds(0.12, TimerMode::Once),
                    1,
                ));
                commands.entity(entity).insert(BowUpgradeSpread(1));
            }
            Heirloom::BowArrowSpeed => {
                commands.entity(entity).insert(ArrowSpeedUpgrade(1.25));
            }
            &Heirloom::TeleportCount => {
                // TeleportCount heirloom is deprecated - use SkillChargeIncrease instead
                // This is kept for compatibility but TeleportState now uses SkillChargeTracker
                commands.entity(entity).insert(TeleportState {
                    just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.5, TimerMode::Once),
                    timer: Timer::from_seconds(0.27, TimerMode::Once),
                    second_explosion_timer: Timer::from_seconds(0.4, TimerMode::Once),
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
}

/// Charge tracker for slot 1 (active_skill_slot_2)
#[derive(Component, Clone, Debug)]
pub struct Slot1ChargeTracker(pub SkillChargeTracker);

/// Charge tracker for slot 2 (active_skill_slot_3)
#[derive(Component, Clone, Debug)]
pub struct Slot2ChargeTracker(pub SkillChargeTracker);

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
                HeirloomChoiceState::new(Heirloom::DiscountMP, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::OnHitEcho, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::HealEcho, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::CritChance, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::CritDamage, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::FrailStacks, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Health, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Shield, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Lifesteal, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Thorns, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Speed, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::AttackSpeed, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::WaveAttack, HeirloomRarity::Rare),
                // HeirloomChoiceState::new(Heirloom::MPBarDMG, HeirloomRarity::Rare),
                // HeirloomChoiceState::new(Heirloom::MPBarCrit, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::LethalBlow, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::DodgeChance, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::SlowStacks, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::AntFarm, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::FrozenAoE, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::FrozenCrit, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::FrozenMPRegen, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::IceStaffFloor, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::PoisonStacks, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::PoisonDuration, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::PoisonStrength, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::ViralVenum, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::ChanceToProcExtraAttack, HeirloomRarity::Rare),
                HeirloomChoiceState::new(
                    Heirloom::IncreaseProjectilCount,
                    HeirloomRarity::Legendary,
                ),
                HeirloomChoiceState::new(Heirloom::BowArrowSpeed, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::IceStaffAoE, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::FullStomach, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::ReinforcedArmor, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::DaggerCombo, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::StoneTooth, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::Reaper, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::ChaosBoost, HeirloomRarity::Uncommon),
                HeirloomChoiceState::new(Heirloom::SkillCDReduction, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::LoadedDice, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::SkillChargeIncrease, HeirloomRarity::Legendary),
                HeirloomChoiceState::new(Heirloom::SkillEcho, HeirloomRarity::Rare),
                HeirloomChoiceState::new(Heirloom::SkillPower, HeirloomRarity::Common),
                HeirloomChoiceState::new(
                    Heirloom::CritSkillCooldownReduction,
                    HeirloomRarity::Rare,
                ),
                HeirloomChoiceState::new(Heirloom::CreditCard, HeirloomRarity::Rare),
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
                matches_rarity && passes_filter && not_banned
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
        let choices = self.queue.remove(0);
        let banned_choice = choices[slot].clone();
        self.banned.insert(banned_choice.heirloom.clone());
        self.pool.retain(|x| x.heirloom != banned_choice.heirloom);

        for (index, choice) in choices.into_iter().enumerate() {
            if index == slot {
                continue;
            }
            if !self.banned.contains(&choice.heirloom) {
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
    pub active_skill_slot_1: Option<ActiveSkillChoiceState>,
    pub active_skill_slot_2: Option<ActiveSkillChoiceState>,
    pub active_skill_slot_3: Option<ActiveSkillChoiceState>,
}

impl Default for PlayerSkills {
    fn default() -> Self {
        Self {
            heirlooms: vec![],
            active_skill_slot_1: Some(ActiveSkillChoiceState::new(
                ActiveSkill::Roll,
                HeirloomRarity::Common,
            )),
            active_skill_slot_2: None,
            active_skill_slot_3: None,
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
    pub fn skill_power_multiplier(&self) -> f32 {
        // 15% additive increase per item: 1.0 + 0.15 * count
        let count = self.get_count(Heirloom::SkillPower).max(0) as f32;
        1.0 + (0.15 * count)
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
            .active_skill_slot_1
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(0);
        }
        if self
            .active_skill_slot_2
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(1);
        }
        if self
            .active_skill_slot_3
            .as_ref()
            .is_some_and(|s| s.active_skill == active_skill)
        {
            return Some(2);
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
                .active_skill_slot_1
                .as_ref()
                .map(|s| s.active_skill.clone()),
            1 => self
                .active_skill_slot_2
                .as_ref()
                .map(|s| s.active_skill.clone()),
            2 => self
                .active_skill_slot_3
                .as_ref()
                .map(|s| s.active_skill.clone()),
            _ => None,
        }
    }
    pub fn insert_active_skill(&mut self, skill: ActiveSkillChoiceState, slot: usize) {
        match slot {
            1 => self.active_skill_slot_1 = Some(skill),
            2 => self.active_skill_slot_2 = Some(skill),
            _ => {}
        }
    }
}
