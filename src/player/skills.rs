use std::time::Duration;

use bevy::prelude::*;
use bevy_aseprite::Aseprite;
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, Rng};
use serde::{Deserialize, Serialize};
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

    Warrior, //spear
    Paladin, //hammer
    Knight,  //sword

    Thief,      //claw
    Gunslinger, //gun

    FireMage, //fire staff
    IceMage,  //ice staff
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
                    AttributeValue::new(f32::floor(level as f32 * 8.) as i32, quality, 1.);
            }
            SkillClass::Paladin => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.health = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::Knight => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.defence = AttributeValue::new(level * 3, quality, 1.);
            }

            SkillClass::Rogue => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.speed = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Archer => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.crit_damage = AttributeValue::new(level * 4, quality, 1.);
            }
            SkillClass::Kid => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.dodge = AttributeValue::new(level * 3, quality, 1.);
            }

            SkillClass::Thief => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.crit_chance = AttributeValue::new(level * 3, quality, 1.);
            }
            SkillClass::Gunslinger => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.attack_speed = AttributeValue::new(level * 3, quality, 1.);
            }

            SkillClass::FireMage => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.mana = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::IceMage => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.mana = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::Wizard => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
                stats.mana_regen =
                    AttributeValue::new(f32::floor(level as f32 * 0.5) as i32, quality, 1.);
            }
            SkillClass::Druid => {
                stats.bonus_damage =
                    AttributeValue::new(f32::floor(level as f32 * 5.) as i32, quality, 1.);
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
        }
    }

    pub fn get_desc(&self) -> Vec<String> {
        match self {
            ActiveSkill::Roll => vec!["Roll to dodge".to_string(), "attacks.".to_string()],
            ActiveSkill::Parry => vec![
                "Active: Parry".to_string(),
                "enemy attacks,".to_string(),
                "ignore damage, and".to_string(),
                "stun attackers if".to_string(),
                "timed successfully.".to_string(),
            ],
            ActiveSkill::ParrySpear => vec![
                "Active: Spear Attack".to_string(),
                "that pulls enemies".to_string(),
                "towards the impact.".to_string(),
            ],
            ActiveSkill::Sprint => vec![
                "Active: Hold Sprint".to_string(),
                "to move 60% faster.".to_string(),
            ],
            ActiveSkill::SprintLunge => vec![
                "Active: dash through".to_string(),
                "enemies with a quick".to_string(),
                "lunge attack.".to_string(),
            ],
            ActiveSkill::Teleport => vec![
                "Active: Teleport a".to_string(),
                "short distance to".to_string(),
                "dodge attacks or".to_string(),
                "move around quickly.".to_string(),
            ],
        }
    }

    pub fn add_skill_components(&self, entity: Entity, commands: &mut Commands) {
        match self {
            ActiveSkill::Sprint => {
                commands
                    .entity(entity)
                    .insert(crate::player::rogue_skills::SprintState {
                        startup_timer: Timer::from_seconds(0.17, TimerMode::Once),
                        sprint_duration_timer: Timer::from_seconds(2.5, TimerMode::Once),
                        sprint_cooldown_timer: Timer::from_seconds(6., TimerMode::Once)
                            .tick(Duration::from_secs(99))
                            .clone(),
                        speed_bonus: 1.6,
                    });
            }
            ActiveSkill::SprintLunge => {
                commands
                    .entity(entity)
                    .insert(crate::player::rogue_skills::LungeState {
                        lunge_cooldown_timer: Timer::from_seconds(4., TimerMode::Once)
                            .tick(Duration::from_secs(99))
                            .clone(),
                        lunge_duration: Timer::from_seconds(0.69, TimerMode::Once),
                        lunge_speed: 3.9,
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
                        count: 1,
                        max_count: 1,
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
            HeirloomRarity::Common => UIElement::SkillChoice,
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
    CritDamage,  //flint
    Health,      //red mushroom
    Shield,      // CD
    Thorns,      //bushling scale
    Lifesteal,   // rose
    Speed,       // feather
    AttackSpeed, // soda can
    DodgeChance, // leather
    Defence,     // coal
    Attack,      // anvil
    Gigantify,   // sappling
    Chest,       // loot bag
    XPGain,      //memory chip

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
    HealEcho,        // chalice
    FullStomach,     //jam
    ReinforcedArmor, // Scale

    // On-Attack Triggers
    WaveAttack,  // hero sword
    FrailStacks, // skull
    SlowStacks,  // sea shell

    // Chaos
    ChaosBoost,   // chaos totem item
    PoisonStacks, // grandma's recipe
    LethalBlow,   // red purple mushroom

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
    //Your echos are stronger
    // your echos are bigger
}

impl Heirloom {
    pub fn get_title(&self) -> String {
        match self {
            Heirloom::CritChance => "Tusk".to_string(),
            Heirloom::CritDamage => "Flint".to_string(),
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
            Heirloom::PoisonStacks => "Grandma's Recipe".to_string(),
            Heirloom::LethalBlow => "Deadly Mushroom".to_string(),
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
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Heirloom::Chest => vec!["Gain a Loot Chest".to_string()],
            Heirloom::CritChance => vec![
                "Gain +10% Critical".to_string(),
                "Chance, ".to_string(),
                "permanantly.".to_string(),
            ],
            Heirloom::CritDamage => vec![
                "Gain +15% Critical".to_string(),
                "Damage, permanently".to_string(),
            ],
            Heirloom::Health => vec!["Gain +25 Health,".to_string(), "permanently.".to_string()],
            Heirloom::Shield => vec!["Gain +10 Shield,".to_string(), "permanently.".to_string()],
            Heirloom::Speed => vec!["Gain +15 Speed,".to_string(), "permanently.".to_string()],
            Heirloom::Thorns => vec!["Gain +15% Thorns, ".to_string(), "permanently.".to_string()],
            Heirloom::Lifesteal => {
                vec!["Gain +1 Lifesteal,".to_string(), "permanently.".to_string()]
            }
            Heirloom::AttackSpeed => vec![
                "Gain +15% Attack".to_string(),
                "Speed, permanently. ".to_string(),
            ],
            Heirloom::XPGain => vec!["Gain +10% XP".to_string(), "permanently. ".to_string()],
            Heirloom::DodgeChance => vec![
                "Gain +10% Dodge".to_string(),
                "Chance,".to_string(),
                "permanently.".to_string(),
            ],
            Heirloom::Gigantify => vec![
                "Your Attacks gain".to_string(),
                "+15% Size".to_string(),
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
                "gives +3% critical".to_string(),
                "chance on hits".to_string(),
            ],
            Heirloom::SlowStacks => vec![
                "Your Attacks have".to_string(),
                "a chance to apply".to_string(),
                "a Slow stack to".to_string(),
                "enemies, reducing".to_string(),
                "speed by 15%.".to_string(),
            ],
            Heirloom::PoisonStacks => vec![
                "Your Attacks have".to_string(),
                "a chance to apply".to_string(),
                "Poison to enemies.".to_string(),
                "Poisoned enemies".to_string(),
                "lose health over".to_string(),
                "time.".to_string(),
            ],
            Heirloom::LethalBlow => vec![
                "Melee attacks ".to_string(),
                "execute enemies ".to_string(),
                "below 20% health.".to_string(),
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
                "+10% critical hit".to_string(),
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
                "The next attack".to_string(),
                "after dodging".to_string(),
                "is a critical hit.".to_string(),
                "+10% dodge chance.".to_string(),
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
                commands.entity(entity).insert(TeleportState {
                    just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.5, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    count: skills.get_count(Heirloom::TeleportCount) as u32,
                    max_count: 2,
                    timer: Timer::from_seconds(0.27, TimerMode::Once),
                    second_explosion_timer: Timer::from_seconds(0.4, TimerMode::Once),
                });
            }
            &Heirloom::DaggerCombo => {
                commands.entity(entity).insert(ComboCounter {
                    counter: 0,
                    reset_timer: Timer::from_seconds(2., TimerMode::Once),
                });
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
            HeirloomRarity::Common => UIElement::SkillChoice,
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
            is_one_time_heirloom: true,
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
    pub rerolls: [bool; 3],
    pub pool: Vec<HeirloomChoiceState>,
    pub active_heirloom_limbo: Option<ActiveSkillChoiceState>,
}

impl Default for HeirloomChoiceQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            rerolls: [true; 3],
            active_heirloom_limbo: None,
            pool: vec![
                HeirloomChoiceState::new(Heirloom::Defence, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Attack, HeirloomRarity::Common).set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Gigantify, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Chest, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::HPRegen, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::XPGain, HeirloomRarity::Common).set_repeatable(),
                HeirloomChoiceState::new(Heirloom::HPRegenCooldown, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::MPRegenCooldown, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::MPRegen, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::DodgeCrit, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Knockback, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::DiscountMP, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::OnHitEcho, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::HealEcho, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::CritChance, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::CritDamage, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::FrailStacks, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Health, HeirloomRarity::Common).set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Shield, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Lifesteal, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Thorns, HeirloomRarity::Common).set_repeatable(),
                HeirloomChoiceState::new(Heirloom::Speed, HeirloomRarity::Common).set_repeatable(),
                HeirloomChoiceState::new(Heirloom::AttackSpeed, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::WaveAttack, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::MPBarDMG, HeirloomRarity::Rare).set_repeatable(),
                HeirloomChoiceState::new(Heirloom::MPBarCrit, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::LethalBlow, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::DodgeChance, HeirloomRarity::Common)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::SlowStacks, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::FrozenAoE, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::FrozenCrit, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::FrozenMPRegen, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::IceStaffFloor, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::PoisonStacks, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::PoisonDuration, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::PoisonStrength, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::ViralVenum, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::ChanceToProcExtraAttack, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::IncreaseProjectilCount, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::BowArrowSpeed, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::IceStaffAoE, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::FullStomach, HeirloomRarity::Uncommon)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::ReinforcedArmor, HeirloomRarity::Rare)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::DaggerCombo, HeirloomRarity::Legendary)
                    .set_repeatable(),
                HeirloomChoiceState::new(Heirloom::ChaosBoost, HeirloomRarity::Uncommon)
                    .set_repeatable(),
            ],
        }
    }
}
impl HeirloomChoiceQueue {
    pub fn add_new_skills_after_levelup(&mut self, rng: &mut rand::rngs::ThreadRng) {
        //only push if queue is empty
        if self.queue.is_empty() {
            self.rerolls = [true; 3];
            let mut new_skills: [HeirloomChoiceState; 3] = Default::default();
            let mut add_back_to_pool: Vec<HeirloomChoiceState> = vec![];
            for i in 0..3 {
                let rarity = HeirloomChoiceQueue::gen_rarity(rng);
                if let Some(picked_skill) = self
                    .get_skill_of_rarity(rarity.clone(), rng, &|s| !new_skills.clone().contains(s))
                {
                    if !picked_skill.is_one_time_heirloom {
                        add_back_to_pool.push(picked_skill.clone());
                    }
                    new_skills[i] = picked_skill.clone();
                    self.pool.retain(|x| x != &new_skills[i]);
                }
            }
            for skill in add_back_to_pool.iter() {
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
        self.pool
            .iter()
            .filter(|x| x.rarity == rarity && filter(x))
            .choose(rng)
            .cloned()
    }
    pub fn gen_rarity(rng: &mut rand::rngs::ThreadRng) -> HeirloomRarity {
        match rng.gen_range(0..100) {
            0..=60 => HeirloomRarity::Common,
            61..=83 => HeirloomRarity::Uncommon,
            84..=96 => HeirloomRarity::Rare,
            _ => HeirloomRarity::Legendary,
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

        //repopulate the queue after each skill selection, if there are skills missing
        if player_skills.heirlooms.len() < player_level as usize - 1 {
            self.add_new_skills_after_levelup(&mut rand::thread_rng());
        }
    }
    pub fn handle_reroll_slot(&mut self, slot: usize, rng: &mut rand::rngs::ThreadRng) {
        if self.rerolls[slot] {
            self.rerolls[slot] = false;
            let old_skill = self.queue[0][slot].clone();
            let rarity = HeirloomChoiceQueue::gen_rarity(rng);
            //TODO: consolidate this code with the main skill picking area?
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
        }
    }
}

impl PlayerSkills {
    pub fn has(&self, heirloom: Heirloom) -> bool {
        self.heirlooms.iter().any(|h| h.heirloom == heirloom)
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
    pub fn insert_active_skill(&mut self, skill: ActiveSkillChoiceState, slot: usize) {
        match slot {
            1 => self.active_skill_slot_1 = Some(skill),
            2 => self.active_skill_slot_2 = Some(skill),
            _ => {}
        }
    }
}
