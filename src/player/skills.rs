use std::time::Duration;

use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::{seq::IteratorRandom, Rng};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

use crate::{
    attributes::{AttributeQuality, AttributeValue, ItemAttributes, ItemGlow},
    custom_commands::CommandsExt,
    item::{
        item_upgrades::{ArrowSpeedUpgrade, BowUpgradeSpread, ClawUpgradeMultiThrow},
        WorldObject,
    },
    proto::proto_param::ProtoParam,
    ui::UIElement,
    Game,
};

use super::{
    mage_skills::TeleportState,
    melee_skills::{ParryState, SpearState},
    rogue_skills::{ComboCounter, LungeState, SprintState},
};

#[derive(Component, Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum SkillClass {
    None,
    Melee,
    Rogue,
    Thief,
    Magic,
}

#[derive(Component, Debug, Clone, Serialize, Deserialize, Resource)]
pub struct PlayerClass {
    pub class: SkillClass,
}

impl SkillClass {
    pub fn get_cape(&self) -> WorldObject {
        match self {
            SkillClass::Melee => WorldObject::RedCape,
            SkillClass::Rogue => WorldObject::GreenCape,
            SkillClass::Magic => WorldObject::BlueCape,
            SkillClass::Thief => WorldObject::GreyCape,
            _ => WorldObject::GreyCape,
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
            SkillClass::Melee => {
                stats.attack =
                    AttributeValue::new(f32::floor(level as f32 * 1.5) as i32, quality, 1.);
                // stats.defence = AttributeValue::new(level, quality, 1.);
                stats.health = AttributeValue::new(level * 5, quality, 1.);
            }
            SkillClass::Rogue => {
                stats.attack =
                    AttributeValue::new(f32::floor(level as f32 * 1.) as i32, quality, 1.);
                stats.speed = AttributeValue::new(level * 2, quality, 1.);
                stats.dodge = AttributeValue::new(level * 3, quality, 1.);
                // stats.crit_chance = AttributeValue::new(level * 3, quality, 1.);
                // stats.crit_damage =
                //     AttributeValue::new(f32::floor(level as f32 * 3.5) as i32, quality, 1.);
            }
            SkillClass::Magic => {
                stats.attack =
                    AttributeValue::new(f32::floor(level as f32 * 1.) as i32, quality, 1.);
                stats.mana = AttributeValue::new(level * 5, quality, 1.);
                stats.mana_regen =
                    AttributeValue::new(f32::floor(level as f32 * 0.5) as i32, quality, 1.);
            }
            SkillClass::Thief => {
                stats.attack =
                    AttributeValue::new(f32::floor(level as f32 * 1.) as i32, quality, 1.);
                stats.crit_chance = AttributeValue::new(level * 3, quality, 1.);
                stats.crit_damage =
                    AttributeValue::new(f32::floor(level as f32 * 2.) as i32, quality, 1.);
            }
            _ => (),
        }
        stats
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Default, Debug, Serialize, EnumIter, Display, Deserialize)]
pub enum Heirloom {
    Roll,
    // Passives
    #[default]
    CritChance,
    CritDamage,
    Health,
    Shield,
    Thorns,
    Lifesteal,
    Speed,
    AttackSpeed,
    DodgeChance,
    Defence,
    Attack,
    Gigantify,
    Chest,

    // On-Attack Triggers
    WaveAttack,
    FrailStacks,
    SlowStacks,
    PoisonStacks,
    LethalBlow,

    // Skills
    Teleport,
    TeleportShock,
    TeleportCooldown,
    TeleportCount,
    TeleportManaRegen,

    Sprint,
    SprintFaster,
    SprintLunge,
    SprintLungeDamage,
    SprintKillReset,

    Parry,
    ParryHPRegen,
    ParrySpear,
    ParryDeflectProj,
    ParryKnockback, // needs art prompt/art
    ParryEcho,

    DaggerCombo,
    HPRegen,
    HPRegenCooldown,
    MPRegen,
    MPRegenCooldown,
    OnHitEcho,
    Knockback, // needs art prompt/art
    DiscountMP,
    MinusOneDamageOnHit, // needs art prompt/art

    // Weapon Upgrades
    ChanceToProcExtraAttack,
    IncreaseProjectilCount,

    IceStaffAoE,
    BowArrowSpeed,

    //magic
    TeleportStatusDMG,

    FrozenAoE,
    IceStaffFloor,
    FrozenCrit,
    MPBarDMG,
    MPBarCrit,
    FrozenMPRegen,

    //rogue
    DodgeCrit,
    PoisonDuration,
    PoisonStrength,
    ViralVenum,

    //melee
    HealEcho,
    FullStomach,
    ReinforcedArmor,
    //Your echos are stronger
    // your echos are bigger
}

impl Heirloom {
    pub fn get_title(&self) -> String {
        match self {
            Heirloom::CritChance => "Keen Eyes".to_string(),
            Heirloom::CritDamage => "Powerful Blows".to_string(),
            Heirloom::Health => "Healthly".to_string(),
            Heirloom::Shield => "Shielded".to_string(),
            Heirloom::Speed => "Nimble Feet".to_string(),
            Heirloom::Thorns => "Forest Scales".to_string(),
            Heirloom::Lifesteal => "Drain Blood".to_string(),
            Heirloom::AttackSpeed => "Swift Blows".to_string(),
            Heirloom::Defence => "Defence!".to_string(),
            Heirloom::Chest => "Chest!".to_string(),
            Heirloom::Attack => "Strength! ".to_string(),
            Heirloom::DodgeChance => "Evasion".to_string(),
            Heirloom::WaveAttack => "Sonic Wave".to_string(),
            Heirloom::FrailStacks => "Frail Blow".to_string(),
            Heirloom::SlowStacks => "Freezing Blow".to_string(),
            Heirloom::PoisonStacks => "Toxic Blow".to_string(),
            Heirloom::LethalBlow => "Lethal Blow".to_string(),
            Heirloom::Teleport => "Teleport".to_string(),
            Heirloom::TeleportShock => "Shock Step".to_string(),
            Heirloom::TeleportCooldown => "Teleport Faster!".to_string(),
            Heirloom::TeleportCount => "Multi-port".to_string(),
            Heirloom::TeleportManaRegen => "Infused Cast".to_string(),
            Heirloom::TeleportStatusDMG => "Shock Mastery".to_string(),
            Heirloom::ChanceToProcExtraAttack => "Double Throw".to_string(),
            Heirloom::IncreaseProjectilCount => "Multi Shot".to_string(),
            Heirloom::BowArrowSpeed => "Piercing Arrows".to_string(),
            Heirloom::Gigantify => "Gigantify".to_string(),

            Heirloom::IceStaffAoE => "Explosive Blast".to_string(),
            Heirloom::Sprint => "Sprint".to_string(),
            Heirloom::SprintFaster => "Faster Sprint".to_string(),
            Heirloom::SprintLunge => "Lunge".to_string(),
            Heirloom::SprintLungeDamage => "Lunge Mastery".to_string(),
            Heirloom::SprintKillReset => "Kill Reset".to_string(),
            Heirloom::Parry => "Parry".to_string(),
            Heirloom::ParryHPRegen => "Rejuvenating Parry".to_string(),
            Heirloom::ParrySpear => "Gravitational Spear".to_string(),
            Heirloom::ParryDeflectProj => "Parry Deflect".to_string(),
            Heirloom::ParryKnockback => "Shield Bash".to_string(),
            Heirloom::ParryEcho => "Parry Echo".to_string(),
            Heirloom::DaggerCombo => "Combo!".to_string(),
            Heirloom::HPRegen => "Health Regeneration ".to_string(),
            Heirloom::HPRegenCooldown => "HP Regen Cooldown".to_string(),
            Heirloom::MPRegenCooldown => "MP Regen Cooldown".to_string(),
            Heirloom::MPRegen => "Mana Regeneration".to_string(),
            Heirloom::OnHitEcho => "War Cry ".to_string(),
            Heirloom::Knockback => "Heavy Strike".to_string(),
            Heirloom::DiscountMP => "Mana Discount".to_string(),
            Heirloom::MinusOneDamageOnHit => "Polished Armor".to_string(),

            Heirloom::FrozenAoE => "Ice Burst".to_string(),
            Heirloom::IceStaffFloor => "Ice Trail ".to_string(),
            Heirloom::FrozenCrit => "Frozen Wounds".to_string(),
            Heirloom::MPBarDMG => "Mana Infusion".to_string(),
            Heirloom::MPBarCrit => "Empowered Spells ".to_string(),
            Heirloom::FrozenMPRegen => "Mana Frost".to_string(),
            Heirloom::DodgeCrit => "Vengeful Strike".to_string(),
            Heirloom::PoisonDuration => "Venum Endurance".to_string(),
            Heirloom::PoisonStrength => "Venumous Edge".to_string(),
            Heirloom::ViralVenum => "Viral Venum".to_string(),
            Heirloom::HealEcho => "Internal Echo".to_string(),
            Heirloom::FullStomach => "Full Stomach".to_string(),

            Heirloom::ReinforcedArmor => "Reinforced Armor".to_string(),
            Heirloom::Roll => "Roll".to_string(),
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Heirloom::Roll => vec!["Roll to dodge".to_string(), "attacks.".to_string()],
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
            Heirloom::Teleport => vec![
                "Active: Teleport a".to_string(),
                "short distance to".to_string(),
                "dodge attacks or".to_string(),
                "move around quickly.".to_string(),
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
            Heirloom::Sprint => vec![
                "Active: Hold Sprint".to_string(),
                "to move 60% faster.".to_string(),
                // "Allows you to attack".to_string(),
                // "while sprinting.".to_string(),
            ],
            Heirloom::SprintFaster => {
                vec!["Your Sprint ability".to_string(), "is faster.".to_string()]
            }
            Heirloom::SprintLunge => vec![
                "Active: dash through".to_string(),
                "enemies with a quick".to_string(),
                "lunge attack.".to_string(),
            ],
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
            Heirloom::Parry => vec![
                "Active: Parry".to_string(),
                "enemy attacks,".to_string(),
                "ignore damage, and".to_string(),
                "stun attackers if".to_string(),
                "timed successfully.".to_string(),
            ],
            Heirloom::ParryHPRegen => vec![
                "A successful".to_string(),
                "parry triggers".to_string(),
                "health regeneration".to_string(),
            ],
            Heirloom::ParrySpear => vec![
                "Active: Spear Attack".to_string(),
                "that pulls enemies".to_string(),
                "towards the impact.".to_string(),
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
        }
    }
    pub fn is_active_skill(&self) -> bool {
        match self {
            Heirloom::Roll => true,
            Heirloom::Parry => true,
            Heirloom::ParrySpear => true,
            Heirloom::Sprint => true,
            Heirloom::SprintLunge => true,
            Heirloom::Teleport => true,
            _ => false,
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
        game: &mut Game,
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
            Heirloom::Sprint => {
                commands.entity(entity).insert(SprintState {
                    startup_timer: Timer::from_seconds(0.17, TimerMode::Once),
                    sprint_duration_timer: Timer::from_seconds(2.5, TimerMode::Once),
                    sprint_cooldown_timer: Timer::from_seconds(6., TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    speed_bonus: 1.6,
                });
            }
            Heirloom::SprintLunge => {
                commands.entity(entity).insert(LungeState {
                    lunge_cooldown_timer: Timer::from_seconds(4., TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    lunge_duration: Timer::from_seconds(0.69, TimerMode::Once),
                    lunge_speed: 3.9,
                });
            }
            Heirloom::Teleport => {
                commands.entity(entity).insert(TeleportState {
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
            &Heirloom::Parry => {
                commands.entity(entity).insert(ParryState {
                    parry_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.2, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    success: false,
                    active: false,
                });
            }
            &Heirloom::ParrySpear => {
                commands.entity(entity).insert(SpearState {
                    cooldown_timer: Timer::from_seconds(5.2, TimerMode::Once)
                        .tick(Duration::from_secs(99))
                        .clone(),
                    spear_timer: Timer::from_seconds(0.5, TimerMode::Once),
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

#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
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
    pub active_heirloom_limbo: Option<HeirloomChoiceState>,
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
                HeirloomChoiceState::new(Heirloom::Sprint, HeirloomRarity::Common),
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
                HeirloomChoiceState::new(Heirloom::Teleport, HeirloomRarity::Common),
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
                HeirloomChoiceState::new(Heirloom::ParrySpear, HeirloomRarity::Common),
                HeirloomChoiceState::new(Heirloom::Parry, HeirloomRarity::Common),
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

        // handle active skills
        if skill.heirloom.is_active_skill() {
            if player_skills.active_skill_slot_1.is_none() {
                player_skills.insert_active_skill(skill.clone(), 1);
            } else if player_skills.active_skill_slot_2.is_none() {
                player_skills.insert_active_skill(skill.clone(), 2);
            } else {
                self.active_heirloom_limbo = Some(skill);
            }
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
    pub active_skill_slot_1: Option<HeirloomChoiceState>,
    pub active_skill_slot_2: Option<HeirloomChoiceState>,
}

impl Default for PlayerSkills {
    fn default() -> Self {
        Self {
            heirlooms: vec![],
            active_skill_slot_1: Some(HeirloomChoiceState::new(
                Heirloom::Roll,
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
    pub fn has_active_heirloom(&self, heirloom: Heirloom) -> Option<usize> {
        if self
            .active_skill_slot_1
            .as_ref()
            .is_some_and(|s| s.heirloom == heirloom)
        {
            return Some(0);
        }
        if self
            .active_skill_slot_2
            .as_ref()
            .is_some_and(|s| s.heirloom == heirloom)
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
    pub fn insert_active_skill(&mut self, skill: HeirloomChoiceState, slot: usize) {
        match slot {
            1 => self.active_skill_slot_1 = Some(skill),
            2 => self.active_skill_slot_2 = Some(skill),
            _ => {}
        }
    }
}
