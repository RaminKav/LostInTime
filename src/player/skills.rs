use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::seq::IteratorRandom;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

use crate::{
    custom_commands::CommandsExt,
    item::{
        item_upgrades::{ArrowSpeedUpgrade, BowUpgradeSpread, ClawUpgradeMultiThrow},
        WorldObject,
    },
    proto::proto_param::ProtoParam,
};

use super::sprint::SprintUpgrade;

pub enum SkillClass {
    Melee,
    Rogue,
    Magic,
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
    AttackDamage,

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
    TeleportManaSteal,

    Sprint,
    SprintFaster,
    SprintStartupFaster,
    SprintLunge,
    SprintLungeDamage,
    SprintLungePierce,
    SprintKillReset,

    DashDeflectProj,
    DashInvulnerable,
    DashFurther,
    DashCount,
    DashKnockback,
    DaggerCombo,

    RegenHPFaster,
    RegenMPFaster,
    OnHitAoEBurst,
    SplitDamage,
    Knockback,
    DiscountMP,
    MinusOneDamageOnHit,
    ChanceToNotConsumeAmmo,

    // Weapon Upgrades
    ClawDoubleThrow,
    BowMultiShot,
    ChainLightning,
    FireStaffAoE,
    BowArrowSpeed,
    //BLUE: ManaGain
    //GREEN: chance to not consume arrows/throwing stars
    //YELLOW: bonus dash
    //YELLOW: first hit after a dash is crit
    //RED: Your swords do bonus dmg
    TimeSlow,
}

impl Skill {
    pub fn get_class(&self) -> SkillClass {
        match self {
            Skill::CritChance => SkillClass::Rogue,
            Skill::CritDamage => SkillClass::Rogue,
            Skill::Health => SkillClass::Melee,
            Skill::Thorns => SkillClass::Rogue,
            Skill::Lifesteal => SkillClass::Melee,
            Skill::Speed => SkillClass::Rogue,
            Skill::AttackSpeed => SkillClass::Rogue,
            Skill::CritLoot => SkillClass::Rogue,
            Skill::DodgeChance => SkillClass::Rogue,
            Skill::AttackDamage => SkillClass::Melee,
            Skill::Defence => SkillClass::Melee,
            Skill::FireDamage => SkillClass::Melee,
            Skill::WaveAttack => SkillClass::Melee,
            Skill::FrailStacks => SkillClass::Melee,
            Skill::SlowStacks => SkillClass::Magic,
            Skill::PoisonStacks => SkillClass::Rogue,
            Skill::LethalBlow => SkillClass::Melee,
            Skill::Teleport => SkillClass::Magic,
            Skill::TeleportShock => SkillClass::Magic,
            Skill::TeleportCooldown => SkillClass::Magic,
            Skill::TeleportCount => SkillClass::Magic,
            Skill::TeleportManaSteal => SkillClass::Magic,
            Skill::Sprint => SkillClass::Rogue,
            Skill::SprintFaster => SkillClass::Rogue,
            Skill::SprintStartupFaster => SkillClass::Rogue,
            Skill::SprintLunge => SkillClass::Rogue,
            Skill::SprintLungeDamage => SkillClass::Rogue,
            Skill::SprintLungePierce => SkillClass::Rogue,
            Skill::SprintKillReset => SkillClass::Rogue,
            Skill::DashDeflectProj => SkillClass::Melee,
            Skill::DashInvulnerable => SkillClass::Melee,
            Skill::DashFurther => SkillClass::Melee,
            Skill::DashCount => SkillClass::Melee,
            Skill::DashKnockback => SkillClass::Melee,
            Skill::DaggerCombo => SkillClass::Rogue,
            Skill::RegenHPFaster => SkillClass::Melee,
            Skill::RegenMPFaster => SkillClass::Magic,
            Skill::OnHitAoEBurst => SkillClass::Melee,
            Skill::SplitDamage => SkillClass::Rogue,
            Skill::Knockback => SkillClass::Melee,
            Skill::DiscountMP => SkillClass::Magic,
            Skill::MinusOneDamageOnHit => SkillClass::Melee,
            Skill::ChanceToNotConsumeAmmo => SkillClass::Rogue,
            Skill::ClawDoubleThrow => SkillClass::Rogue,
            Skill::BowMultiShot => SkillClass::Rogue,
            Skill::ChainLightning => SkillClass::Magic,
            Skill::FireStaffAoE => SkillClass::Magic,
            Skill::BowArrowSpeed => SkillClass::Rogue,
            Skill::TimeSlow => SkillClass::Magic,
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
            Skill::DodgeChance => "Evasion".to_string(),
            Skill::FireDamage => "Fire Aspect".to_string(),
            Skill::WaveAttack => "Sonic Wave".to_string(),
            Skill::FrailStacks => "Frail Blow".to_string(),
            Skill::SlowStacks => "Freezing Blow".to_string(),
            Skill::PoisonStacks => "Toxic Blow".to_string(),
            Skill::LethalBlow => "Lethal Blow".to_string(),
            Skill::Teleport => "Teleport".to_string(),
            Skill::TeleportShock => "Shock Step".to_string(),
            Skill::TimeSlow => "Time Slow".to_string(),
            Skill::ClawDoubleThrow => "Double Throw".to_string(),
            Skill::BowMultiShot => "Multi Shot".to_string(),
            Skill::BowArrowSpeed => "Piercing Arrows".to_string(),
            Skill::ChainLightning => "Chain Lightning".to_string(),
            Skill::FireStaffAoE => "Explosive Blast".to_string(),
            Skill::Sprint => "Sprint".to_string(),
            Skill::SprintFaster => "Faster Sprint".to_string(),
            Skill::SprintLunge => "Lunge".to_string(),
            Skill::SprintKillReset => "Kill Reset".to_string(),
            _ => "Unknown".to_string(),
        }
    }
    pub fn get_desc(&self) -> Vec<String> {
        // max 13 char per line, space included
        match self {
            Skill::CritChance => vec![
                "Grants +10% Critical".to_string(),
                "Chance, permanantly. ".to_string(),
            ],
            Skill::CritDamage => vec![
                "Grants +15% Critical".to_string(),
                "Damage, permanently".to_string(),
            ],
            Skill::Health => vec!["Grants +25 Health,".to_string(), "permanently.".to_string()],
            Skill::Speed => vec!["Grants +15 Speed,".to_string(), "permanently.".to_string()],
            Skill::Thorns => vec![
                "Grants +15% Thorns, ".to_string(),
                "permanently.".to_string(),
            ],
            Skill::Lifesteal => vec![
                "Grants +1 Lifesteal,".to_string(),
                "permanently.".to_string(),
            ],
            Skill::AttackSpeed => vec![
                "Grants +15% Attack".to_string(),
                "Speed, permanently.".to_string(),
            ],
            Skill::CritLoot => vec![
                "Enemies slayn with a".to_string(),
                "critical hit have a".to_string(),
                "+25% loot drop".to_string(),
                "chance.".to_string(),
            ],
            Skill::DodgeChance => vec![
                "Grants +10% Dodge".to_string(),
                "Chance, permanently.".to_string(),
            ],
            Skill::FireDamage => vec![
                "Your melee attacks".to_string(),
                "deal a second fire".to_string(),
                "attack to enemies.".to_string(),
            ],
            Skill::WaveAttack => vec![
                "Your melee Attacks".to_string(),
                "send a sonic wave ".to_string(),
                "attackt hat travels".to_string(),
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
                "execute enemies".to_string(),
                "below 25% health.".to_string(),
            ],
            Skill::Teleport => vec![
                "Your dodge action".to_string(),
                "becomes Teleport.".to_string(),
            ],
            Skill::TeleportShock => vec![
                "Teleporting through".to_string(),
                "enemies damages".to_string(),
                "them.".to_string(),
            ],
            Skill::TimeSlow => vec!["Slow down".to_string(), "Time briefly".to_string()],
            Skill::ClawDoubleThrow => vec![
                "Gain a Claw.".to_string(),
                "Your Claws throw 2".to_string(),
                "stars in quick".to_string(),
                "succession.".to_string(),
            ],
            Skill::BowMultiShot => vec![
                "Gain a Bow.".to_string(),
                "Your Bows shoot 3".to_string(),
                "arrows in a spread".to_string(),
                "pattern.".to_string(),
            ],
            Skill::ChainLightning => vec![
                "Gain a Lightning ".to_string(),
                "Staff.".to_string(),
                "Lighting Staff".to_string(),
                "bolts now chain to".to_string(),
                "nearby enemies.".to_string(),
            ],
            Skill::FireStaffAoE => vec![
                "Gain a Fire Staff".to_string(),
                "Fire Staff fireballs".to_string(),
                "explode on contact,".to_string(),
                "dealing dmg in an".to_string(),
                "area.".to_string(),
            ],
            Skill::BowArrowSpeed => {
                vec!["Your Bow's Arrows".to_string(), "move faster.".to_string()]
            }
            _ => vec!["Unknown".to_string()],
        }
    }

    pub fn get_instant_drop(&self) -> Option<(WorldObject, usize)> {
        match self {
            Skill::ClawDoubleThrow => Some((WorldObject::Claw, 1)),
            Skill::BowMultiShot => Some((WorldObject::WoodBow, 1)),
            Skill::ChainLightning => Some((WorldObject::BasicStaff, 1)),
            Skill::FireStaffAoE => Some((WorldObject::FireStaff, 1)),
            Skill::BowArrowSpeed => Some((WorldObject::Arrow, 24)),
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
            Skill::BowArrowSpeed => {
                commands.entity(entity).insert(ArrowSpeedUpgrade(1.));
            }
            Skill::Sprint => {
                commands.entity(entity).insert(SprintUpgrade {
                    startup_timer: Timer::from_seconds(0.17, TimerMode::Once),
                    sprint_duration_timer: Timer::from_seconds(3.5, TimerMode::Once),
                    sprint_cooldown_timer: Timer::from_seconds(0.1, TimerMode::Once),
                    lunge_duration: Timer::from_seconds(0.69, TimerMode::Once),
                    speed_bonus: 1.6,
                    lunge_speed: 2.9,
                });
            }

            _ => {}
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
    pub pool: Vec<SkillChoiceState>,
}

impl Default for SkillChoiceQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            pool: vec![
                SkillChoiceState::new(Skill::Sprint)
                    .with_children(vec![
                        SkillChoiceState::new(Skill::SprintFaster),
                        SkillChoiceState::new(Skill::SprintLunge)
                            .with_children(vec![SkillChoiceState::new(Skill::SprintKillReset)]),
                    ])
                    .with_clashing(vec![Skill::Teleport]),
                SkillChoiceState::new(Skill::CritChance)
                    .set_repeatable()
                    .with_children(vec![
                        SkillChoiceState::new(Skill::CritDamage).set_repeatable(),
                        SkillChoiceState::new(Skill::CritLoot),
                        SkillChoiceState::new(Skill::FrailStacks),
                    ]),
                SkillChoiceState::new(Skill::Health)
                    .set_repeatable()
                    .with_children(vec![
                        SkillChoiceState::new(Skill::Lifesteal),
                        SkillChoiceState::new(Skill::Thorns).set_repeatable(),
                    ]),
                SkillChoiceState::new(Skill::Speed)
                    .set_repeatable()
                    .with_children(vec![
                        SkillChoiceState::new(Skill::AttackSpeed).set_repeatable(),
                        SkillChoiceState::new(Skill::WaveAttack),
                    ]),
                SkillChoiceState::new(Skill::LethalBlow),
                SkillChoiceState::new(Skill::DodgeChance).set_repeatable(),
                SkillChoiceState::new(Skill::FireDamage),
                SkillChoiceState::new(Skill::SlowStacks),
                SkillChoiceState::new(Skill::PoisonStacks),
                SkillChoiceState::new(Skill::Teleport)
                    .with_children(vec![
                        SkillChoiceState::new(Skill::TeleportShock),
                        // SkillChoiceState::new(Skill::TimeSlow),
                    ])
                    .with_clashing(vec![Skill::Sprint]),
                SkillChoiceState::new(Skill::ClawDoubleThrow),
                SkillChoiceState::new(Skill::BowMultiShot)
                    .with_children(vec![SkillChoiceState::new(Skill::BowArrowSpeed)]),
                SkillChoiceState::new(Skill::ChainLightning),
                SkillChoiceState::new(Skill::FireStaffAoE),
            ],
        }
    }
}
impl SkillChoiceQueue {
    pub fn add_new_skills_after_levelup(&mut self, rng: &mut rand::rngs::ThreadRng) {
        //only push if queue is empty
        if self.queue.is_empty() {
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
    pub fn get_class_affinity(&self) -> SkillClass {
        //return the highest count class
        let (melee, rogue, magic) = (
            self.melee_skill_count,
            self.rogue_skill_count,
            self.magic_skill_count,
        );
        if melee >= rogue && melee >= magic {
            SkillClass::Melee
        } else if rogue >= melee && rogue >= magic {
            SkillClass::Rogue
        } else {
            SkillClass::Magic
        }
    }
    pub fn increment_class_count(&mut self, skill: Skill) {
        match skill.get_class() {
            SkillClass::Melee => self.melee_skill_count += 1,
            SkillClass::Rogue => self.rogue_skill_count += 1,
            SkillClass::Magic => self.magic_skill_count += 1,
        }
    }
}
