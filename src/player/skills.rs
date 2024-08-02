use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::item::{
    item_upgrades::{
        ArrowSpeedUpgrade, BowUpgradeSpread, BurnOnHitUpgrade, ClawUpgradeMultiThrow,
        FireStaffAOEUpgrade, FrailOnHitUpgrade, LethalHitUpgrade, LightningStaffChainUpgrade,
        SlowOnHitUpgrade,
    },
    WorldObject,
};

#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
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
    TimeSlow,
    // Weapon Upgrades
    ClawDoubleThrow,
    BowMultiShot,
    ChainLightning,
    FireStaffAoE,
    BowArrowSpeed,
    //Chance to not use arrow
    //
}

impl Skill {
    pub fn get_icon(&self) -> WorldObject {
        match self {
            Skill::CritChance => WorldObject::Arrow,
            Skill::CritDamage => WorldObject::Arrow,
            Skill::Health => WorldObject::LargePotion,
            Skill::Speed => WorldObject::Feather,
            Skill::Thorns => WorldObject::BushlingScale,
            Skill::Lifesteal => WorldObject::LargePotion,
            Skill::AttackSpeed => WorldObject::Feather,
            Skill::CritLoot => WorldObject::Chest,
            Skill::DodgeChance => WorldObject::LeatherShoes,
            Skill::FireDamage => WorldObject::Fireball,
            Skill::WaveAttack => WorldObject::Tusk,
            Skill::FrailStacks => WorldObject::Flint,
            Skill::SlowStacks => WorldObject::String,
            Skill::PoisonStacks => WorldObject::SlimeGoo,
            Skill::LethalBlow => WorldObject::Sword,
            Skill::Teleport => WorldObject::OrbOfTransformation,
            Skill::TeleportShock => WorldObject::OrbOfTransformation,
            Skill::TimeSlow => WorldObject::OrbOfTransformation,
            Skill::ClawDoubleThrow => WorldObject::Claw,
            Skill::BowMultiShot => WorldObject::WoodBow,
            Skill::BowArrowSpeed => WorldObject::WoodBow,
            Skill::ChainLightning => WorldObject::BasicStaff,
            Skill::FireStaffAoE => WorldObject::FireStaff,
        }
    }
    pub fn get_title(&self) -> String {
        match self {
            Skill::CritChance => "Crit Chance".to_string(),
            Skill::CritDamage => "Crit Damage".to_string(),
            Skill::Health => "Health".to_string(),
            Skill::Speed => "Speed".to_string(),
            Skill::Thorns => "Thorns".to_string(),
            Skill::Lifesteal => "Lifesteal".to_string(),
            Skill::AttackSpeed => "Attack Speed".to_string(),
            Skill::CritLoot => "Crit Loot".to_string(),
            Skill::DodgeChance => "Dodge Chance".to_string(),
            Skill::FireDamage => "Fire Damage".to_string(),
            Skill::WaveAttack => "Wave Attack".to_string(),
            Skill::FrailStacks => "Frail".to_string(),
            Skill::SlowStacks => "Slow".to_string(),
            Skill::PoisonStacks => "Poison".to_string(),
            Skill::LethalBlow => "Lethal Blow".to_string(),
            Skill::Teleport => "Teleport".to_string(),
            Skill::TeleportShock => "Teleport II".to_string(),
            Skill::TimeSlow => "Time Slow".to_string(),
            Skill::ClawDoubleThrow => "Claw ++".to_string(),
            Skill::BowMultiShot => "Bow ++".to_string(),
            Skill::BowArrowSpeed => "Faster Arrows".to_string(),
            Skill::ChainLightning => "Staff ++".to_string(),
            Skill::FireStaffAoE => "Fire Staff ++".to_string(),
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Skill::CritChance => vec!["+10% Crit".to_string(), "Chance".to_string()],
            Skill::CritDamage => vec!["+15% Crit.".to_string(), "Damage".to_string()],
            Skill::Health => vec!["+25 Health".to_string()],
            Skill::Speed => vec!["+15 Speed".to_string()],
            Skill::Thorns => vec!["+15% Thorns".to_string()],
            Skill::Lifesteal => vec!["+1 Lifesteal".to_string()],
            Skill::AttackSpeed => vec!["+15% Attack".to_string(), "Speed".to_string()],
            Skill::CritLoot => vec![
                "Enemies slayn".to_string(),
                "with a crit.".to_string(),
                "hit have +25%".to_string(),
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
            Skill::LethalBlow => vec![
                "Hits have a".to_string(),
                "chance to".to_string(),
                "execute low".to_string(),
                "hp enemies".to_string(),
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
            Skill::FireStaffAoE => vec![
                "Gain a Fire".to_string(),
                "Staff.".to_string(),
                "Its fireballs".to_string(),
                "explode on hit".to_string(),
            ],
            Skill::BowArrowSpeed => vec!["Arrows move".to_string(), "faster.".to_string()],
        }
    }

    pub fn get_instant_drop(&self) -> Option<WorldObject> {
        match self {
            Skill::ClawDoubleThrow => Some(WorldObject::Claw),
            Skill::BowMultiShot => Some(WorldObject::WoodBow),
            Skill::ChainLightning => Some(WorldObject::BasicStaff),
            Skill::FireStaffAoE => Some(WorldObject::FireStaff),
            _ => None,
        }
    }

    pub fn add_skill_components(&self, entity: Entity, commands: &mut Commands) {
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
            Skill::ChainLightning => {
                commands.entity(entity).insert(LightningStaffChainUpgrade);
            }
            Skill::FireStaffAoE => {
                commands.entity(entity).insert(FireStaffAOEUpgrade);
            }
            Skill::PoisonStacks => {
                commands.entity(entity).insert(BurnOnHitUpgrade);
            }
            Skill::LethalBlow => {
                commands.entity(entity).insert(LethalHitUpgrade);
            }
            Skill::BowArrowSpeed => {
                commands.entity(entity).insert(ArrowSpeedUpgrade(1.));
            }
            Skill::FrailStacks => {
                commands.entity(entity).insert(FrailOnHitUpgrade);
            }
            Skill::SlowStacks => {
                commands.entity(entity).insert(SlowOnHitUpgrade);
            }
            _ => {}
        }
    }
}
#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct SkillChoiceState {
    pub skill: Skill,
    pub child_skills: Vec<SkillChoiceState>,
}
impl SkillChoiceState {
    pub fn new(skill: Skill) -> Self {
        Self {
            skill,
            child_skills: Default::default(),
        }
    }
    pub fn with_children(mut self, children: Vec<SkillChoiceState>) -> Self {
        self.child_skills = children;
        self
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct SkillChoiceQueue {
    pub queue: Vec<[SkillChoiceState; 3]>,
    pub pool: Vec<SkillChoiceState>,
}

impl Default for SkillChoiceQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            pool: vec![
                SkillChoiceState::new(Skill::CritChance).with_children(vec![
                    SkillChoiceState::new(Skill::CritDamage),
                    SkillChoiceState::new(Skill::CritLoot),
                ]),
                SkillChoiceState::new(Skill::Health).with_children(vec![
                    SkillChoiceState::new(Skill::Lifesteal),
                    SkillChoiceState::new(Skill::Thorns),
                ]),
                SkillChoiceState::new(Skill::Speed),
                SkillChoiceState::new(Skill::AttackSpeed),
                SkillChoiceState::new(Skill::LethalBlow),
                SkillChoiceState::new(Skill::DodgeChance),
                SkillChoiceState::new(Skill::FireDamage),
                SkillChoiceState::new(Skill::WaveAttack),
                SkillChoiceState::new(Skill::FrailStacks),
                SkillChoiceState::new(Skill::SlowStacks),
                SkillChoiceState::new(Skill::PoisonStacks),
                SkillChoiceState::new(Skill::Teleport).with_children(vec![
                    SkillChoiceState::new(Skill::TeleportShock),
                    // SkillChoiceState::new(Skill::TimeSlow),
                ]),
                SkillChoiceState::new(Skill::ClawDoubleThrow),
                SkillChoiceState::new(Skill::BowMultiShot),
                SkillChoiceState::new(Skill::ChainLightning),
                SkillChoiceState::new(Skill::FireStaffAoE),
            ],
        }
    }
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerSkills {
    pub skills: Vec<Skill>,
}

impl PlayerSkills {
    pub fn get(&self, skill: Skill) -> bool {
        self.skills.contains(&skill)
    }
}
