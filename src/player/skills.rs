use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::item::WorldObject;

#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub enum Skill {
    // Passives
    #[default]
    CritChance,
    CritDamage,
    Health,
    Speed,
    AttackSpeed,
    CritLoot,
    DodgeChance,

    // On-Attack Triggers
    FireDamage,
    WaveAttack,
    FrailStacks,
    SlowStacks,
    PoisonStacks,

    // Skills
    Teleport,
    TeleportShock,
    TimeSlow,
    // Weapon Upgrades
    ClawDoubleThrow,
    BowMultiShot,
    ChainLightning,
}

impl Skill {
    pub fn get_icon(&self) -> WorldObject {
        match self {
            Skill::CritChance => WorldObject::Arrow,
            Skill::CritDamage => WorldObject::Arrow,
            Skill::Health => WorldObject::LargePotion,
            Skill::Speed => WorldObject::Feather,
            Skill::AttackSpeed => WorldObject::Feather,
            Skill::CritLoot => WorldObject::Chest,
            Skill::DodgeChance => WorldObject::LeatherShoes,
            Skill::FireDamage => WorldObject::Fireball,
            Skill::WaveAttack => WorldObject::Tusk,
            Skill::FrailStacks => WorldObject::Flint,
            Skill::SlowStacks => WorldObject::String,
            Skill::PoisonStacks => WorldObject::SlimeGoo,
            Skill::Teleport => WorldObject::OrbOfTransformation,
            Skill::TeleportShock => WorldObject::OrbOfTransformation,
            Skill::TimeSlow => WorldObject::OrbOfTransformation,
            Skill::ClawDoubleThrow => WorldObject::Claw,
            Skill::BowMultiShot => WorldObject::WoodBow,
            Skill::ChainLightning => WorldObject::BasicStaff,
        }
    }
    pub fn get_title(&self) -> String {
        match self {
            Skill::CritChance => "Crit Chance".to_string(),
            Skill::CritDamage => "Crit Damage".to_string(),
            Skill::Health => "Health".to_string(),
            Skill::Speed => "Speed".to_string(),
            Skill::AttackSpeed => "Attack Speed".to_string(),
            Skill::CritLoot => "Crit Loot".to_string(),
            Skill::DodgeChance => "Dodge Chance".to_string(),
            Skill::FireDamage => "Fire Damage".to_string(),
            Skill::WaveAttack => "Wave Attack".to_string(),
            Skill::FrailStacks => "Frail".to_string(),
            Skill::SlowStacks => "Slow".to_string(),
            Skill::PoisonStacks => "Poison".to_string(),
            Skill::Teleport => "Teleport".to_string(),
            Skill::TeleportShock => "Teleport II".to_string(),
            Skill::TimeSlow => "Time Slow".to_string(),
            Skill::ClawDoubleThrow => "Claw ++".to_string(),
            Skill::BowMultiShot => "Bow ++".to_string(),
            Skill::ChainLightning => "Staff ++".to_string(),
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Skill::CritChance => vec!["+10% Crit".to_string(), "Chance".to_string()],
            Skill::CritDamage => vec!["+15% Crit.".to_string(), "Damage".to_string()],
            Skill::Health => vec!["+25 Health".to_string()],
            Skill::Speed => vec!["+15 Speed".to_string()],
            Skill::AttackSpeed => vec!["+15% Attack".to_string(), "Speed".to_string()],
            Skill::CritLoot => vec![
                "Enemies slayn".to_string(),
                "with a crit.".to_string(),
                "hit have +20%".to_string(),
                "drop chance".to_string(),
            ],
            Skill::DodgeChance => vec!["+10% Dodge".to_string(), "Chance".to_string()],
            Skill::FireDamage => vec![
                "Attks deal a".to_string(),
                "second fire".to_string(),
                "attack".to_string(),
            ],
            Skill::WaveAttack => vec!["Attks send a".to_string(), "Wave attack".to_string()],
            Skill::FrailStacks => vec![
                "Hits apply a".to_string(),
                "Frail stack".to_string(),
                "that gives 3%".to_string(),
                "crit chance".to_string(),
            ],
            Skill::SlowStacks => vec![
                "Hits apply a".to_string(),
                "Slow effect".to_string(),
                "to enemies".to_string(),
            ],
            Skill::PoisonStacks => vec![
                "Hits apply a".to_string(),
                "Poison stack".to_string(),
                "that deals".to_string(),
                "dmg over time".to_string(),
            ],
            Skill::Teleport => vec![
                "Your dodge".to_string(),
                "becomes a".to_string(),
                "Teleport".to_string(),
            ],
            Skill::TeleportShock => vec![
                "Teleporting".to_string(),
                "shocks enemies".to_string(),
                "in your path".to_string(),
            ],
            Skill::TimeSlow => vec!["Slow down".to_string(), "Time briefly".to_string()],
            Skill::ClawDoubleThrow => vec![
                "Gain a Claw.".to_string(),
                "Claws throw".to_string(),
                "2 stars now".to_string(),
            ],
            Skill::BowMultiShot => vec![
                "Gain a Bow.".to_string(),
                "Bows shoot".to_string(),
                "3 arrows now".to_string(),
            ],
            Skill::ChainLightning => vec![
                "Gain a Staff.".to_string(),
                "Lighting now".to_string(),
                "chains to".to_string(),
                "other enemies".to_string(),
            ],
        }
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerSkills {
    pub skills: Vec<Skill>,
}
