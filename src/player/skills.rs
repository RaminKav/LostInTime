use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

use crate::{
    attributes::{AttributeQuality, AttributeValue, ItemAttributes},
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
    melee_skills::ParryState,
    sprint::{ComboCounter, SprintState},
    teleport::TeleportState,
};

#[derive(Component, Debug, Clone, Eq, PartialEq)]
pub enum SkillClass {
    None,
    Melee,
    Rogue,
    Magic,
}

impl SkillClass {
    pub fn get_cape(&self) -> WorldObject {
        match self {
            SkillClass::Melee => WorldObject::RedCape,
            SkillClass::Rogue => WorldObject::GreenCape,
            SkillClass::Magic => WorldObject::BlueCape,
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
                stats.defence = AttributeValue::new(level, quality, 1.);
                stats.health = AttributeValue::new(level * 10, quality, 1.);
            }
            SkillClass::Rogue => {
                stats.attack =
                    AttributeValue::new(f32::floor(level as f32 * 1.) as i32, quality, 1.);
                stats.speed = AttributeValue::new(level * 5, quality, 1.);
                stats.dodge = AttributeValue::new(level, quality, 1.);
                stats.crit_chance = AttributeValue::new(level * 2, quality, 1.);
                stats.crit_damage =
                    AttributeValue::new(f32::floor(level as f32 * 3.5) as i32, quality, 1.);
            }
            SkillClass::Magic => {
                stats.attack =
                    AttributeValue::new(f32::floor(level as f32 * 1.5) as i32, quality, 1.);
                stats.mana = AttributeValue::new(level * 5, quality, 1.);
                stats.mana_regen =
                    AttributeValue::new(f32::floor(level as f32 * 0.5) as i32, quality, 1.);
            }
            _ => (),
        }
        stats
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Default, Debug, Serialize, EnumIter, Display, Deserialize)]
pub enum Skill {
    // Passives
    #[default]
    CritChance,
    CritDamage,
    Health,
    Thorns,
    Lifesteal,
    Speed,
    AttackSpeed,
    CritLoot,
    DodgeChance,
    Defence,
    Attack,

    // On-Attack Triggers
    FireDamage,
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
    SplitDamage, //TODO animation missing
    Knockback,   // needs art prompt/art
    DiscountMP,
    MinusOneDamageOnHit, // needs art prompt/art
    ChanceToNotConsumeAmmo,

    // Weapon Upgrades
    ClawDoubleThrow,
    BowMultiShot,
    ChainLightning,
    IceStaffAoE,
    BowArrowSpeed,

    //magic
    TeleportStatusDMG,
    StaffDMG,
    FrozenAoE,
    IceStaffFloor,
    FrozenCrit,
    TeleportIceAoe,
    TeleportIceAoeEnd,
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
    SwordDMG,
    FullStomach,
    WideSwing, //TODO animation missing
    ReinforcedArmor,
    //Your echos are stronger
    // your echos are bigger
}

impl Skill {
    pub fn get_class(&self) -> SkillClass {
        match self {
            Skill::Health => SkillClass::Melee,
            Skill::Lifesteal => SkillClass::Melee,
            Skill::Attack => SkillClass::Melee,
            Skill::Defence => SkillClass::Melee,
            Skill::FireDamage => SkillClass::Melee,
            Skill::WaveAttack => SkillClass::Melee,
            Skill::FrailStacks => SkillClass::Melee,
            Skill::LethalBlow => SkillClass::Melee,
            Skill::Parry => SkillClass::Melee,
            Skill::ParryHPRegen => SkillClass::Melee,
            Skill::ParrySpear => SkillClass::Melee,
            Skill::ParryDeflectProj => SkillClass::Melee,
            Skill::ParryKnockback => SkillClass::Melee,
            Skill::ParryEcho => SkillClass::Melee,
            Skill::HPRegen => SkillClass::Melee,
            Skill::OnHitEcho => SkillClass::Melee,
            Skill::Knockback => SkillClass::Melee,
            Skill::MinusOneDamageOnHit => SkillClass::Melee,
            Skill::HPRegenCooldown => SkillClass::Melee,
            Skill::HealEcho => SkillClass::Melee,
            Skill::SwordDMG => SkillClass::Melee,
            Skill::FullStomach => SkillClass::Melee,
            Skill::WideSwing => SkillClass::Melee,
            Skill::ReinforcedArmor => SkillClass::Melee,

            Skill::CritChance => SkillClass::Rogue,
            Skill::CritDamage => SkillClass::Rogue,
            Skill::Thorns => SkillClass::Rogue,
            Skill::Speed => SkillClass::Rogue,
            Skill::AttackSpeed => SkillClass::Rogue,
            Skill::CritLoot => SkillClass::Rogue,
            Skill::DodgeChance => SkillClass::Rogue,
            Skill::PoisonStacks => SkillClass::Rogue,
            Skill::DaggerCombo => SkillClass::Rogue,
            Skill::Sprint => SkillClass::Rogue,
            Skill::SprintFaster => SkillClass::Rogue,
            Skill::SprintLunge => SkillClass::Rogue,
            Skill::SprintLungeDamage => SkillClass::Rogue,
            Skill::SprintKillReset => SkillClass::Rogue,
            Skill::SplitDamage => SkillClass::Rogue,
            Skill::ChanceToNotConsumeAmmo => SkillClass::Rogue,
            Skill::ClawDoubleThrow => SkillClass::Rogue,
            Skill::BowMultiShot => SkillClass::Rogue,
            Skill::BowArrowSpeed => SkillClass::Rogue,
            Skill::DodgeCrit => SkillClass::Rogue,
            Skill::PoisonDuration => SkillClass::Rogue,
            Skill::PoisonStrength => SkillClass::Rogue,
            Skill::ViralVenum => SkillClass::Rogue,

            Skill::SlowStacks => SkillClass::Magic,
            Skill::Teleport => SkillClass::Magic,
            Skill::TeleportShock => SkillClass::Magic,
            Skill::TeleportCooldown => SkillClass::Magic,
            Skill::TeleportCount => SkillClass::Magic,
            Skill::TeleportManaRegen => SkillClass::Magic,
            Skill::MPRegen => SkillClass::Magic,
            Skill::DiscountMP => SkillClass::Magic,
            Skill::ChainLightning => SkillClass::Magic,
            Skill::IceStaffAoE => SkillClass::Magic,
            Skill::MPRegenCooldown => SkillClass::Magic,
            Skill::TeleportStatusDMG => SkillClass::Magic,
            Skill::StaffDMG => SkillClass::Magic,
            Skill::FrozenAoE => SkillClass::Magic,
            Skill::IceStaffFloor => SkillClass::Magic,
            Skill::FrozenCrit => SkillClass::Magic,
            Skill::TeleportIceAoe => SkillClass::Magic,
            Skill::TeleportIceAoeEnd => SkillClass::Magic,
            Skill::MPBarDMG => SkillClass::Magic,
            Skill::MPBarCrit => SkillClass::Magic,
            Skill::FrozenMPRegen => SkillClass::Magic,
        }
    }
    pub fn get_title(&self) -> String {
        match self {
            Skill::CritChance => "Keen Eyes".to_string(),
            Skill::CritDamage => "Powerful Blows".to_string(),
            Skill::Health => "Healthly".to_string(),
            Skill::Speed => "Nimble Feet".to_string(),
            Skill::Thorns => "Forest Scales".to_string(),
            Skill::Lifesteal => "Drain Blood".to_string(),
            Skill::AttackSpeed => "Swift Blows".to_string(),
            Skill::CritLoot => "Eye on the Prize".to_string(),
            Skill::Defence => "Defence!".to_string(),
            Skill::Attack => "Strength! ".to_string(),
            Skill::DodgeChance => "Evasion".to_string(),
            Skill::FireDamage => "Fire Aspect".to_string(),
            Skill::WaveAttack => "Sonic Wave".to_string(),
            Skill::FrailStacks => "Frail Blow".to_string(),
            Skill::SlowStacks => "Freezing Blow".to_string(),
            Skill::PoisonStacks => "Toxic Blow".to_string(),
            Skill::LethalBlow => "Lethal Blow".to_string(),
            Skill::Teleport => "Teleport".to_string(),
            Skill::TeleportShock => "Shock Step".to_string(),
            Skill::TeleportCooldown => "Teleport Faster!".to_string(),
            Skill::TeleportCount => "Multi-port".to_string(),
            Skill::TeleportManaRegen => "Infused Cast".to_string(),
            Skill::TeleportStatusDMG => "Shock Mastery".to_string(),
            Skill::ClawDoubleThrow => "Double Throw".to_string(),
            Skill::BowMultiShot => "Multi Shot".to_string(),
            Skill::BowArrowSpeed => "Piercing Arrows".to_string(),
            Skill::ChainLightning => "Chain Lightning".to_string(),
            Skill::IceStaffAoE => "Explosive Blast".to_string(),
            Skill::Sprint => "Sprint".to_string(),
            Skill::SprintFaster => "Faster Sprint".to_string(),
            Skill::SprintLunge => "Lunge".to_string(),
            Skill::SprintLungeDamage => "Lunge Mastery".to_string(),
            Skill::SprintKillReset => "Kill Reset".to_string(),
            Skill::Parry => "Parry".to_string(),
            Skill::ParryHPRegen => "Rejuvenating Parry".to_string(),
            Skill::ParrySpear => "Gravitational Spear".to_string(),
            Skill::ParryDeflectProj => "Parry Deflect".to_string(),
            Skill::ParryKnockback => "Shield Bash".to_string(),
            Skill::ParryEcho => "Parry Echo".to_string(),
            Skill::DaggerCombo => "Combo!".to_string(),
            Skill::HPRegen => "Health Regeneration ".to_string(),
            Skill::HPRegenCooldown => "HP Regen Cooldown".to_string(),
            Skill::MPRegenCooldown => "MP Regen Cooldown".to_string(),
            Skill::MPRegen => "Mana Regeneration".to_string(),
            Skill::OnHitEcho => "War Cry ".to_string(),
            Skill::SplitDamage => "Double Stab ".to_string(),
            Skill::Knockback => "Heavy Strike".to_string(),
            Skill::DiscountMP => "Mana Discount".to_string(),
            Skill::MinusOneDamageOnHit => "Polished Armor".to_string(),
            Skill::ChanceToNotConsumeAmmo => "Ammo Mastery".to_string(),
            Skill::StaffDMG => "Staff Mastery".to_string(),
            Skill::FrozenAoE => "Ice Burst".to_string(),
            Skill::IceStaffFloor => "Ice Trail ".to_string(),
            Skill::FrozenCrit => "Frozen Wounds".to_string(),
            Skill::TeleportIceAoe => "Ice Explosion".to_string(),
            Skill::TeleportIceAoeEnd => "Dual Explosion".to_string(),
            Skill::MPBarDMG => "Mana Infusion".to_string(),
            Skill::MPBarCrit => "Empowered Spells ".to_string(),
            Skill::FrozenMPRegen => "Mana Frost".to_string(),
            Skill::DodgeCrit => "Vengeful Strike".to_string(),
            Skill::PoisonDuration => "Venum Endurance".to_string(),
            Skill::PoisonStrength => "Venumous Edge".to_string(),
            Skill::ViralVenum => "Viral Venum".to_string(),
            Skill::HealEcho => "Internal Echo".to_string(),
            Skill::SwordDMG => "Powerful Strike".to_string(),
            Skill::FullStomach => "Full Stomach".to_string(),
            Skill::WideSwing => "Wide Swing ".to_string(),
            Skill::ReinforcedArmor => "Reinforced Armor".to_string(),
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Skill::CritChance => vec![
                "Gain +10% Critical".to_string(),
                "Chance, ".to_string(),
                "permanantly.".to_string(),
            ],
            Skill::CritDamage => vec![
                "Gain +15% Critical".to_string(),
                "Damage, permanently".to_string(),
            ],
            Skill::Health => vec!["Gain +25 Health,".to_string(), "permanently.".to_string()],
            Skill::Speed => vec!["Gain +15 Speed,".to_string(), "permanently.".to_string()],
            Skill::Thorns => vec!["Gain +15% Thorns, ".to_string(), "permanently.".to_string()],
            Skill::Lifesteal => vec!["Gain +1 Lifesteal,".to_string(), "permanently.".to_string()],
            Skill::AttackSpeed => vec![
                "Gain +15% Attack".to_string(),
                "Speed, permanently. ".to_string(),
            ],
            Skill::CritLoot => vec![
                "Enemies slayn with a".to_string(),
                "critical hit have a".to_string(),
                "+25% loot drop".to_string(),
                "chance.".to_string(),
            ],
            Skill::DodgeChance => vec![
                "Gain +10% Dodge".to_string(),
                "Chance,".to_string(),
                "permanently.".to_string(),
            ],
            Skill::FireDamage => vec![
                "Your melee attacks".to_string(),
                "deal a second fire ".to_string(),
                "attack to enemies.".to_string(),
            ],
            Skill::WaveAttack => vec![
                "Your melee Attacks".to_string(),
                "send a sonic wave ".to_string(),
                "attack that travels".to_string(),
                "a short distance.".to_string(),
            ],
            Skill::FrailStacks => vec![
                "Melee attacks apply".to_string(),
                "a Frail stack that ".to_string(),
                "gives +3% critical".to_string(),
                "chance on hits".to_string(),
            ],
            Skill::SlowStacks => vec![
                "Melee attacks apply".to_string(),
                "a Slow stack to".to_string(),
                "enemies, reducing".to_string(),
                "speed by 15%.".to_string(),
            ],
            Skill::PoisonStacks => vec![
                "Melee attacks apply".to_string(),
                "Poison to enemies.".to_string(),
                "Poisoned enemies".to_string(),
                "lose health over".to_string(),
                "time.".to_string(),
            ],
            Skill::LethalBlow => vec![
                "Melee attacks ".to_string(),
                "execute enemies ".to_string(),
                "below 25% health.".to_string(),
            ],
            Skill::Teleport => vec![
                "Your Roll ability".to_string(),
                "becomes Teleport.".to_string(),
            ],
            Skill::TeleportShock => vec![
                "Teleporting through".to_string(),
                "enemies damages".to_string(),
                "them.".to_string(),
            ],
            Skill::TeleportCooldown => vec![
                "Your Teleport".to_string(),
                "cooldown is".to_string(),
                "reduced.".to_string(),
            ],
            Skill::TeleportCount => vec!["Gain +1 Teleport".to_string(), "count.".to_string()],
            Skill::TeleportManaRegen => vec![
                "Attacking right".to_string(),
                "after a Teleport".to_string(),
                "triggers mana".to_string(),
                "regeneration.".to_string(),
            ],
            Skill::Sprint => vec![
                "Your Roll ability".to_string(),
                "becomes Sprint, ".to_string(),
                "allowing you to".to_string(),
                "move very fast.".to_string(),
            ],
            Skill::SprintFaster => {
                vec!["Your Sprint ability".to_string(), "is faster.".to_string()]
            }
            Skill::SprintLunge => vec![
                "Right-click while".to_string(),
                "sprinting to do".to_string(),
                "a quick lunge".to_string(),
                "attack.".to_string(),
            ],
            Skill::SprintLungeDamage => vec![
                "Your Lunge attack".to_string(),
                "does more damage.".to_string(),
            ],
            Skill::SprintKillReset => vec![
                "Killing an enemy".to_string(),
                "resets your Sprint".to_string(),
                "and Lunge attack".to_string(),
                "cooldown.".to_string(),
            ],
            Skill::ClawDoubleThrow => vec![
                "Gain a Claw.".to_string(),
                "Your Claws throw 2".to_string(),
                "stars in quick".to_string(),
                "succession. ".to_string(),
            ],
            Skill::BowMultiShot => vec![
                "Gain a Bow.".to_string(),
                "Your Bows shoot 3".to_string(),
                "arrows in a spread ".to_string(),
                "pattern.".to_string(),
            ],
            Skill::ChainLightning => vec![
                "Gain a Lightning ".to_string(),
                "Staff.".to_string(),
                "Lighting Staff ".to_string(),
                "bolts now chain to".to_string(),
                "nearby enemies. ".to_string(),
            ],
            Skill::IceStaffAoE => vec![
                "Gain an Ice Staff".to_string(),
                "Ice Staff attacks".to_string(),
                "triggers an ice".to_string(),
                "explosion that".to_string(),
                "damages enemies. ".to_string(),
            ],
            Skill::BowArrowSpeed => {
                vec!["Your Bow's Arrows".to_string(), "move faster.".to_string()]
            }
            Skill::Attack => vec!["Gain +3 Attack,".to_string(), "permanently.".to_string()],
            Skill::Defence => vec!["Gain +10 Defence,".to_string(), "permanently.".to_string()],
            Skill::Parry => vec![
                "Your Roll ability".to_string(),
                "becomes Parry,".to_string(),
                "allowing you to".to_string(),
                "ignore damage and".to_string(),
                "stun attackers if".to_string(),
                "successful ".to_string(),
            ],
            Skill::ParryHPRegen => vec![
                "A successful".to_string(),
                "parry triggers".to_string(),
                "health regeneration".to_string(),
            ],
            Skill::ParrySpear => vec![
                "A successful".to_string(),
                "parry allows you".to_string(),
                "to Right-Click to".to_string(),
                "do a Spear Attack".to_string(),
                "that pulls enemies".to_string(),
                "towards the impact.".to_string(),
            ],
            Skill::ParryDeflectProj => vec![
                "A successful".to_string(),
                "parry deflects".to_string(),
                "projectiles.".to_string(),
            ],
            Skill::ParryKnockback => vec![
                "A successful".to_string(),
                "parry knocks ".to_string(),
                "back enemies.".to_string(),
            ],
            Skill::ParryEcho => vec![
                "A successful".to_string(),
                "parry triggers".to_string(),
                "an echo that".to_string(),
                "damages enemies".to_string(),
                "around you.".to_string(),
            ],
            Skill::DaggerCombo => vec![
                "Attacks chained".to_string(),
                "together build ".to_string(),
                "Combo, increasing ".to_string(),
                "your critical ".to_string(),
                "damage.".to_string(),
            ],
            Skill::HPRegen => vec![
                "Gain +5 Health".to_string(),
                "regeneration, ".to_string(),
                "permanently.".to_string(),
            ],
            Skill::MPRegen => vec![
                "Gain +5 Mana ".to_string(),
                "regeneration,".to_string(),
                "permanently.".to_string(),
            ],
            Skill::HPRegenCooldown => vec![
                "Your Health".to_string(),
                "regeneration".to_string(),
                "cooldown is.".to_string(),
                "reduced.".to_string(),
            ],
            Skill::MPRegenCooldown => vec![
                "Your Mana".to_string(),
                "regeneration".to_string(),
                "cooldown is.".to_string(),
                "reduced.".to_string(),
            ],
            Skill::OnHitEcho => vec![
                "After taking ".to_string(),
                "damage, trigger ".to_string(),
                "an echo that".to_string(),
                "damages enemies ".to_string(),
                "around you.".to_string(),
            ],
            Skill::SplitDamage => vec![
                "Your melee".to_string(),
                "attacks are split".to_string(),
                "into two separate".to_string(),
                "hits, each dealing".to_string(),
                "half the damage. ".to_string(),
            ],
            Skill::Knockback => vec![
                "Your attacks".to_string(),
                "knockback enemies".to_string(),
                "further. ".to_string(),
            ],
            Skill::DiscountMP => vec![
                "Your staffs' attacks".to_string(),
                "cost less mana.".to_string(),
            ],
            Skill::MinusOneDamageOnHit => vec![
                "All incoming enemy ".to_string(),
                "damage is reduced".to_string(),
                "by one. ".to_string(),
            ],
            Skill::ChanceToNotConsumeAmmo => vec![
                "You have a 33%".to_string(),
                "chance to not".to_string(),
                "consume ammo.".to_string(),
            ],
            Skill::TeleportStatusDMG => vec![
                "Teleporting through".to_string(),
                "an enemy with a".to_string(),
                "status effect deals".to_string(),
                "more damage.".to_string(),
            ],
            Skill::StaffDMG => vec!["Your Staffs deal".to_string(), "+3 damage.".to_string()],
            Skill::FrozenAoE => vec![
                "Killing a frozen".to_string(),
                "enemy triggers an".to_string(),
                "ice explosion that".to_string(),
                "damages enemies.".to_string(),
            ],
            Skill::IceStaffFloor => vec![
                "Your Ice Staff's".to_string(),
                "attacks leave a".to_string(),
                "trail of ice that".to_string(),
                "damages enemies. ".to_string(),
            ],
            Skill::FrozenCrit => vec![
                "Attacking frozen".to_string(),
                "enemies gives you".to_string(),
                "+10% critical hit".to_string(),
                "chance.".to_string(),
            ],
            Skill::TeleportIceAoe => vec![
                "Teleporting triggers".to_string(),
                "an ice explosion".to_string(),
                "that damages enemies".to_string(),
                "where your teleport".to_string(),
                "starts.".to_string(),
            ],
            Skill::TeleportIceAoeEnd => vec![
                "Teleporting triggers".to_string(),
                "a second ice".to_string(),
                "explosion where".to_string(),
                "your teleport ends.".to_string(),
            ],
            Skill::MPBarDMG => vec![
                "Your staff's attacks".to_string(),
                "gain +25% damage".to_string(),
                "if your mana bar".to_string(),
                "is full.".to_string(),
            ],
            Skill::MPBarCrit => vec![
                "Your staff's attacks".to_string(),
                "gain +10% critical".to_string(),
                "hit chance if your".to_string(),
                "mana bar is full.".to_string(),
            ],
            Skill::FrozenMPRegen => vec![
                "Killing a frozen".to_string(),
                "enemy triggers".to_string(),
                "mana regeneration.".to_string(),
            ],
            Skill::DodgeCrit => vec![
                "The next attack".to_string(),
                "after dodging".to_string(),
                "is a critical hit.".to_string(),
            ],
            Skill::PoisonDuration => vec![
                "Your poison effect".to_string(),
                "lasts longer.".to_string(),
            ],
            Skill::PoisonStrength => vec![
                "Your poison effect".to_string(),
                "does more damage.".to_string(),
            ],
            Skill::ViralVenum => vec![
                "Killing a poisoned".to_string(),
                "enemy spreads it's".to_string(),
                "poison to nearby".to_string(),
                "enemies.".to_string(),
            ],
            Skill::HealEcho => vec![
                "Healing triggers".to_string(),
                "an echo that".to_string(),
                "damages enemies ".to_string(),
                "around you.".to_string(),
            ],
            Skill::SwordDMG => vec![
                "Your Swords deal".to_string(),
                "+3 Damage but you".to_string(),
                "lose 5 speed.".to_string(),
            ],
            Skill::FullStomach => vec![
                "You get hungry".to_string(),
                "at a slower rate.".to_string(),
            ],
            Skill::WideSwing => vec![
                "Your Swords' attacks".to_string(),
                "are wider and".to_string(),
                "larger, but you".to_string(),
                "lose 5 speed.".to_string(),
            ],
            Skill::ReinforcedArmor => vec![
                "You gain Defence".to_string(),
                "the more speed".to_string(),
                "you have lost. Lose".to_string(),
                "5 speed. ".to_string(),
            ],
        }
    }

    pub fn get_instant_drop(&self) -> Option<(WorldObject, usize)> {
        match self {
            Skill::ClawDoubleThrow => Some((WorldObject::Claw, 1)),
            Skill::BowMultiShot => Some((WorldObject::WoodBow, 1)),
            Skill::ChainLightning => Some((WorldObject::BasicStaff, 1)),
            Skill::IceStaffAoE => Some((WorldObject::IceStaff, 1)),
            Skill::BowArrowSpeed => Some((WorldObject::Arrow, 24)),
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
            Skill::ClawDoubleThrow => {
                commands.entity(entity).insert(ClawUpgradeMultiThrow(
                    Timer::from_seconds(0.12, TimerMode::Once),
                    1,
                ));
            }
            Skill::BowMultiShot => {
                commands.entity(entity).insert(BowUpgradeSpread(2));
            }
            Skill::BowArrowSpeed => {
                commands.entity(entity).insert(ArrowSpeedUpgrade(1.));
            }
            Skill::Sprint => {
                commands.entity(entity).insert(SprintState {
                    startup_timer: Timer::from_seconds(0.17, TimerMode::Once),
                    sprint_duration_timer: Timer::from_seconds(3.5, TimerMode::Once),
                    sprint_cooldown_timer: Timer::from_seconds(0.1, TimerMode::Once),
                    lunge_duration: Timer::from_seconds(0.69, TimerMode::Once),
                    speed_bonus: 1.6,
                    lunge_speed: 2.9,
                });
            }
            Skill::Teleport => {
                commands.entity(entity).insert(TeleportState {
                    just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.5, TimerMode::Once),
                    count: 1,
                    max_count: 1,
                    timer: Timer::from_seconds(0.27, TimerMode::Once),
                    second_explosion_timer: Timer::from_seconds(0.4, TimerMode::Once),
                });
            }
            &Skill::TeleportCount => {
                commands.entity(entity).insert(TeleportState {
                    just_teleported_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.5, TimerMode::Once),
                    count: skills.get_count(Skill::TeleportCount) as u32,
                    max_count: 2,
                    timer: Timer::from_seconds(0.27, TimerMode::Once),
                    second_explosion_timer: Timer::from_seconds(0.4, TimerMode::Once),
                });
            }
            &Skill::DaggerCombo => {
                commands.entity(entity).insert(ComboCounter {
                    counter: 0,
                    reset_timer: Timer::from_seconds(2., TimerMode::Once),
                });
            }
            &Skill::WideSwing => {
                //resets collider
                game.player_state.main_hand_slot = None;
            }
            &Skill::Parry => {
                commands.entity(entity).insert(ParryState {
                    parry_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(1.2, TimerMode::Once),
                    spear_timer: Timer::from_seconds(0.5, TimerMode::Once),
                    success: false,
                    active: false,
                });
            }

            _ => {}
        }
    }
    pub fn get_ui_element(&self) -> UIElement {
        match self.get_class() {
            SkillClass::None => UIElement::SkillChoice,
            SkillClass::Melee => UIElement::SkillChoice,
            SkillClass::Rogue => UIElement::SkillChoice,
            SkillClass::Magic => UIElement::SkillChoice,
        }
    }
    pub fn get_ui_element_hover(&self) -> UIElement {
        match self.get_class() {
            SkillClass::None => UIElement::SkillChoiceMeleeHover,
            SkillClass::Melee => UIElement::SkillChoiceMeleeHover,
            SkillClass::Rogue => UIElement::SkillChoiceRogueHover,
            SkillClass::Magic => UIElement::SkillChoiceMagicHover,
        }
    }

    pub fn is_obj_valid(&self, obj: WorldObject) -> bool {
        match self {
            Skill::FireDamage => obj.is_melee_weapon(),
            Skill::WaveAttack => obj.is_melee_weapon(),
            Skill::FrailStacks => obj.is_melee_weapon(),
            Skill::SlowStacks => obj.is_melee_weapon(),
            Skill::PoisonStacks => obj.is_melee_weapon(),
            Skill::LethalBlow => obj.is_melee_weapon(),
            _ => true,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct SkillChoiceState {
    pub skill: Skill,
    pub child_skills: Vec<SkillChoiceState>,
    pub clashing_skills: Vec<Skill>,
    pub is_one_time_skill: bool,
}
impl SkillChoiceState {
    pub fn new(skill: Skill) -> Self {
        Self {
            skill,
            child_skills: Default::default(),
            clashing_skills: Default::default(),
            is_one_time_skill: true,
        }
    }
    pub fn with_children(mut self, children: Vec<SkillChoiceState>) -> Self {
        self.child_skills = children;
        self
    }
    pub fn set_repeatable(mut self) -> Self {
        self.is_one_time_skill = false;
        self
    }
    pub fn with_clashing(mut self, clashing: Vec<Skill>) -> Self {
        self.clashing_skills = clashing;
        self
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct SkillChoiceQueue {
    pub queue: Vec<[SkillChoiceState; 3]>,
    pub rerolls: [bool; 3],
    pub pool: Vec<SkillChoiceState>,
}

impl Default for SkillChoiceQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            rerolls: [true; 3],
            pool: vec![
                SkillChoiceState::new(Skill::Defence),
                SkillChoiceState::new(Skill::Attack),
                SkillChoiceState::new(Skill::HPRegen),
                SkillChoiceState::new(Skill::HPRegenCooldown),
                SkillChoiceState::new(Skill::MPRegenCooldown),
                SkillChoiceState::new(Skill::MPRegen),
                SkillChoiceState::new(Skill::SplitDamage),
                SkillChoiceState::new(Skill::DodgeCrit),
                SkillChoiceState::new(Skill::Knockback),
                SkillChoiceState::new(Skill::DiscountMP),
                SkillChoiceState::new(Skill::ChanceToNotConsumeAmmo).set_repeatable(),
                SkillChoiceState::new(Skill::MinusOneDamageOnHit),
                SkillChoiceState::new(Skill::OnHitEcho),
                SkillChoiceState::new(Skill::HealEcho),
                SkillChoiceState::new(Skill::Sprint)
                    .with_children(vec![
                        SkillChoiceState::new(Skill::SprintFaster),
                        SkillChoiceState::new(Skill::SprintLunge).with_children(vec![
                            SkillChoiceState::new(Skill::SprintKillReset),
                            SkillChoiceState::new(Skill::SprintLungeDamage),
                        ]),
                    ])
                    .with_clashing(vec![Skill::Teleport, Skill::Parry]),
                SkillChoiceState::new(Skill::CritChance).set_repeatable(),
                SkillChoiceState::new(Skill::CritDamage).set_repeatable(),
                SkillChoiceState::new(Skill::CritLoot),
                SkillChoiceState::new(Skill::FrailStacks),
                SkillChoiceState::new(Skill::Health)
                    .set_repeatable()
                    .with_children(vec![SkillChoiceState::new(Skill::Lifesteal)]),
                SkillChoiceState::new(Skill::Thorns).set_repeatable(),
                SkillChoiceState::new(Skill::Speed).set_repeatable(),
                SkillChoiceState::new(Skill::AttackSpeed).set_repeatable(),
                SkillChoiceState::new(Skill::WaveAttack),
                SkillChoiceState::new(Skill::MPBarDMG),
                SkillChoiceState::new(Skill::MPBarCrit),
                SkillChoiceState::new(Skill::LethalBlow),
                SkillChoiceState::new(Skill::DodgeChance).set_repeatable(),
                SkillChoiceState::new(Skill::FireDamage),
                SkillChoiceState::new(Skill::SlowStacks).with_children(vec![
                    SkillChoiceState::new(Skill::FrozenAoE),
                    SkillChoiceState::new(Skill::FrozenCrit),
                    SkillChoiceState::new(Skill::FrozenMPRegen),
                ]),
                SkillChoiceState::new(Skill::IceStaffFloor),
                SkillChoiceState::new(Skill::PoisonStacks).with_children(vec![
                    SkillChoiceState::new(Skill::PoisonDuration),
                    SkillChoiceState::new(Skill::PoisonStrength).set_repeatable(),
                    SkillChoiceState::new(Skill::ViralVenum),
                ]),
                SkillChoiceState::new(Skill::Teleport)
                    .with_children(vec![
                        SkillChoiceState::new(Skill::TeleportShock)
                            .with_children(vec![SkillChoiceState::new(Skill::TeleportStatusDMG)]),
                        SkillChoiceState::new(Skill::TeleportCooldown),
                        SkillChoiceState::new(Skill::TeleportCount).set_repeatable(),
                        SkillChoiceState::new(Skill::TeleportManaRegen),
                        SkillChoiceState::new(Skill::TeleportIceAoe),
                    ])
                    .with_clashing(vec![Skill::Sprint, Skill::Parry]),
                SkillChoiceState::new(Skill::ClawDoubleThrow),
                SkillChoiceState::new(Skill::BowMultiShot)
                    .with_children(vec![SkillChoiceState::new(Skill::BowArrowSpeed)]),
                SkillChoiceState::new(Skill::ChainLightning),
                SkillChoiceState::new(Skill::IceStaffAoE),
                SkillChoiceState::new(Skill::StaffDMG),
                SkillChoiceState::new(Skill::SwordDMG),
                SkillChoiceState::new(Skill::FullStomach),
                SkillChoiceState::new(Skill::WideSwing),
                SkillChoiceState::new(Skill::ReinforcedArmor),
                SkillChoiceState::new(Skill::DaggerCombo),
                SkillChoiceState::new(Skill::Parry)
                    .with_children(vec![
                        SkillChoiceState::new(Skill::ParryHPRegen),
                        SkillChoiceState::new(Skill::ParrySpear),
                        SkillChoiceState::new(Skill::ParryDeflectProj),
                        SkillChoiceState::new(Skill::ParryKnockback),
                        SkillChoiceState::new(Skill::ParryEcho),
                    ])
                    .with_clashing(vec![Skill::Sprint, Skill::Teleport]),
            ],
        }
    }
}
impl SkillChoiceQueue {
    pub fn add_new_skills_after_levelup(&mut self, rng: &mut rand::rngs::ThreadRng) {
        //only push if queue is empty
        if self.queue.is_empty() {
            self.rerolls = [true; 3];
            let mut new_skills: [SkillChoiceState; 3] = Default::default();
            let mut add_back_to_pool: Vec<SkillChoiceState> = vec![];
            for i in 0..3 {
                if let Some(picked_skill) = self.pool.iter().choose(rng) {
                    if !picked_skill.is_one_time_skill {
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

    pub fn handle_pick_skill(
        &mut self,
        skill: SkillChoiceState,
        proto_commands: &mut ProtoCommands,
        proto: &ProtoParam,
        player_pos: Vec2,
        player_skills: &mut PlayerSkills,
        player_level: u8,
    ) {
        player_skills.skills.push(skill.skill.clone());
        player_skills.increment_class_count(skill.skill.clone());

        let mut remaining_choices = self.queue.remove(0).to_vec();
        remaining_choices.retain(|x| x != &skill);
        for choice in remaining_choices.iter() {
            self.pool.push(choice.clone());
        }
        for child in skill.child_skills.iter() {
            if !player_skills.skills.contains(&child.skill) {
                self.pool.push(child.clone());
            }
        }
        for clash in skill.clashing_skills.iter() {
            self.pool.retain(|x| x.skill != *clash);
        }
        // handle drops
        if let Some((drop, count)) = skill.skill.get_instant_drop() {
            proto_commands.spawn_item_from_proto(
                drop,
                proto,
                player_pos + Vec2::new(0., -18.), // offset so it doesn't spawn on the player
                count,
                Some(player_level),
            );
        }
        //repopulate the queue after each skill selection, if there are skills missing
        if player_skills.skills.len() < player_level as usize - 1 {
            self.add_new_skills_after_levelup(&mut rand::thread_rng());
        }
    }
    pub fn handle_reroll_slot(&mut self, slot: usize) {
        if self.rerolls[slot] {
            self.rerolls[slot] = false;
            let old_skill = self.queue[0][slot].clone();
            let new_skill = self
                .pool
                .iter()
                .choose(&mut rand::thread_rng())
                .unwrap()
                .clone();
            self.pool.retain(|x| x != &new_skill);
            self.pool.push(old_skill);
            self.queue[0][slot] = new_skill;
        }
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerSkills {
    pub skills: Vec<Skill>,
    pub melee_skill_count: usize,
    pub rogue_skill_count: usize,
    pub magic_skill_count: usize,
}

impl PlayerSkills {
    pub fn has(&self, skill: Skill) -> bool {
        self.skills.contains(&skill)
    }
    pub fn get_count(&self, skill: Skill) -> i32 {
        self.skills.iter().filter(|&s| *s == skill).count() as i32
    }
    pub fn get_class_affinity(&self, prev_class: SkillClass) -> SkillClass {
        //return the highest count class
        let (melee, rogue, magic) = (
            self.melee_skill_count,
            self.rogue_skill_count,
            self.magic_skill_count,
        );
        if melee == 0 && rogue == 0 && magic == 0 {
            return SkillClass::None;
        }
        //if there is a tie with the prev class, always return the prev class
        if prev_class == SkillClass::Melee && melee >= rogue && melee >= magic {
            SkillClass::Melee
        } else if prev_class == SkillClass::Rogue && rogue >= melee && rogue >= magic {
            SkillClass::Rogue
        } else if prev_class == SkillClass::Magic && magic >= melee && magic >= rogue {
            SkillClass::Magic
        } else {
            if melee >= rogue && melee >= magic {
                SkillClass::Melee
            } else if rogue >= melee && rogue >= magic {
                SkillClass::Rogue
            } else {
                SkillClass::Magic
            }
        }
    }
    pub fn increment_class_count(&mut self, skill: Skill) {
        match skill.get_class() {
            SkillClass::None => unreachable!(),
            SkillClass::Melee => self.melee_skill_count += 1,
            SkillClass::Rogue => self.rogue_skill_count += 1,
            SkillClass::Magic => self.magic_skill_count += 1,
        }
    }
}
