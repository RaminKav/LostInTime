use item_abilities::handle_item_abilitiy_on_attack;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::ops::{Add, RangeInclusive};
use strum_macros::{Display, EnumIter};

use bevy::{ecs::system::EntityCommands, prelude::*};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
pub mod health_regen;
pub mod modifiers;
use crate::{
    animations::AnimatedTextureMaterial,
    assets::Graphics,
    attributes::attribute_helpers::{build_item_stack_with_parsed_attributes, get_rarity_rng},
    client::GameOverEvent,
    colors::{GREY, LIGHT_BLUE, LIGHT_GREY, LIGHT_RED, ORANGE, UNCOMMON_GREEN},
    inventory::{Inventory, ItemStack},
    item::{Equipment, EquipmentType, WorldObject},
    player::{
        skills::{PlayerSkills, Skill},
        stats::StatType,
        Limb,
    },
    proto::proto_param::ProtoParam,
    ui::{
        scrapper_ui::{Scrap, ScrapsInto},
        stats_ui::StatsButtonState,
        DropOnSlotEvent, InventoryState, RemoveFromSlotEvent, ShowInvPlayerStatsEvent, UIElement,
        UIState,
    },
    CustomFlush, GameParam, GameState, Player,
};
use modifiers::*;
pub mod attribute_helpers;
pub mod hunger;
use hunger::*;
pub mod item_abilities;

use self::health_regen::{handle_health_regen, handle_mana_regen};
pub struct AttributesPlugin;

#[derive(Resource, Reflect, Default, Bundle)]
pub struct BlockAttributeBundle {
    pub health: CurrentHealth,
}
#[derive(
    Component,
    PartialEq,
    Clone,
    Reflect,
    FromReflect,
    Schematic,
    Default,
    Debug,
    Serialize,
    Deserialize,
)]
#[reflect(Schematic, Default)]
pub struct ItemAttributes {
    pub health: AttributeValue,
    pub attack: AttributeValue,
    pub durability: AttributeValue,
    pub max_durability: AttributeValue,
    pub attack_cooldown: f32,
    pub invincibility_cooldown: f32,
    pub crit_chance: AttributeValue,
    pub crit_damage: AttributeValue,
    pub bonus_damage: AttributeValue,
    pub health_regen: AttributeValue,
    pub healing: AttributeValue,
    pub thorns: AttributeValue,
    pub dodge: AttributeValue,
    pub speed: AttributeValue,
    pub lifesteal: AttributeValue,
    pub defence: AttributeValue,
    pub xp_rate: AttributeValue,
    pub loot_rate: AttributeValue,
}

#[derive(PartialEq, Clone, Copy, Reflect, FromReflect, Default, Debug, Serialize, Deserialize)]
pub struct AttributeValue {
    pub value: i32,
    pub quality: AttributeQuality,
    pub range_percentage: f32,
}
impl AttributeValue {
    pub fn new(value: i32, quality: AttributeQuality, range_percentage: f32) -> Self {
        Self {
            value,
            quality,
            range_percentage,
        }
    }
}
impl std::fmt::Display for AttributeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl Add<AttributeValue> for AttributeValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            value: self.value + rhs.value,
            quality: self.quality.get_higher(&rhs.quality),
            range_percentage: f32::max(self.range_percentage, rhs.range_percentage),
        }
    }
}

impl Add<i32> for AttributeValue {
    type Output = Self;

    fn add(self, rhs: i32) -> Self::Output {
        Self {
            value: self.value + rhs,
            quality: self.quality,
            range_percentage: self.range_percentage,
        }
    }
}

#[derive(PartialEq, Clone, Copy, Reflect, FromReflect, Default, Debug, Serialize, Deserialize)]
pub enum AttributeQuality {
    #[default]
    Low,
    Average,
    High,
}

impl AttributeQuality {
    pub fn get_higher(&self, other: &Self) -> Self {
        match self {
            AttributeQuality::Low => match other {
                AttributeQuality::Low => AttributeQuality::Low,
                AttributeQuality::Average => AttributeQuality::Average,
                AttributeQuality::High => AttributeQuality::High,
            },
            AttributeQuality::Average => match other {
                AttributeQuality::Low => AttributeQuality::Average,
                AttributeQuality::Average => AttributeQuality::Average,
                AttributeQuality::High => AttributeQuality::High,
            },
            AttributeQuality::High => match other {
                AttributeQuality::Low => AttributeQuality::High,
                AttributeQuality::Average => AttributeQuality::High,
                AttributeQuality::High => AttributeQuality::High,
            },
        }
    }
    pub fn get_quality(range: RangeInclusive<i32>, value: i32) -> Self {
        let total_range = range.end() - range.start();
        let percent_of_total_range = (value - range.start()) as f32 / total_range as f32;
        if percent_of_total_range < 0.33 {
            AttributeQuality::Low
        } else if percent_of_total_range < 0.66 {
            AttributeQuality::Average
        } else {
            AttributeQuality::High
        }
    }
    pub fn get_color(&self) -> Color {
        match self {
            AttributeQuality::Low => LIGHT_GREY,
            AttributeQuality::Average => GREY,
            AttributeQuality::High => ORANGE,
        }
    }
}

impl ItemAttributes {
    pub fn get_tooltips(
        &self,
        rarity: ItemRarity,
        base_att: Option<&RawItemBaseAttributes>,
        bonus_att: Option<&RawItemBonusAttributes>,
    ) -> (Vec<(String, String, AttributeQuality)>, f32, f32) {
        let mut tooltips: Vec<(String, String, AttributeQuality)> = vec![];
        let is_positive = |val: i32| val > 0;
        let r = rarity.get_rarity_attributes_bonus();
        let mut total_score = 0.;
        let mut total_atts = 0.;
        if self.health.value != 0 {
            tooltips.push((
                format!(
                    "{}{} HP",
                    if is_positive(self.health.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.health.value
                ),
                if let Some(health) = &base_att.unwrap().health {
                    format!(
                        "({}-{})",
                        f32::round(*health.start() as f32 * r) as i32,
                        f32::round(*health.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().health.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().health.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.health.quality,
            ));
            total_atts += 1.;
            total_score += self.health.range_percentage;
        }
        if self.defence.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Defence",
                    if is_positive(self.defence.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.defence.value
                ),
                if let Some(defence) = &base_att.unwrap().defence {
                    format!(
                        "({}-{})",
                        f32::round(*defence.start() as f32 * r) as i32,
                        f32::round(*defence.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().defence.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().defence.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.defence.quality,
            ));
            total_atts += 1.;
            total_score += self.defence.range_percentage;
        }
        if self.attack.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Damage",
                    if is_positive(self.attack.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.attack.value
                ),
                if let Some(attack) = &base_att.unwrap().attack {
                    format!(
                        "({}-{})",
                        f32::round(*attack.start() as f32 * r) as i32,
                        f32::round(*attack.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().attack.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().attack.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.attack.quality,
            ));
            total_atts += 1.;
            total_score += self.attack.range_percentage;
        }
        if self.attack_cooldown != 0. {
            tooltips.push((
                format!("{:.2} Hits/s", 1. / self.attack_cooldown),
                "".to_string(),
                AttributeQuality::Average,
            ));
        }
        if self.dodge.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Dodge",
                    if is_positive(self.dodge.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.dodge.value
                ),
                if let Some(dodge) = &base_att.unwrap().dodge {
                    format!(
                        "({}-{})",
                        f32::round(*dodge.start() as f32 * r) as i32,
                        f32::round(*dodge.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().dodge.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().dodge.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.dodge.quality,
            ));
            total_atts += 1.;
            total_score += self.dodge.range_percentage;
        }
        if self.crit_chance.value != 0 {
            tooltips.push((
                format!(
                    "{}{}% Crit",
                    if is_positive(self.crit_chance.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.crit_chance.value
                ),
                if let Some(crit_chance) = &base_att.unwrap().crit_chance {
                    format!(
                        "({}-{})",
                        f32::round(*crit_chance.start() as f32 * r) as i32,
                        f32::round(*crit_chance.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(
                            *bonus_att.unwrap().crit_chance.clone().unwrap().start() as f32 * r
                        ) as i32,
                        f32::round(
                            *bonus_att.unwrap().crit_chance.clone().unwrap().end() as f32 * r
                        ) as i32
                    )
                },
                self.crit_chance.quality,
            ));
            total_atts += 1.;
            total_score += self.crit_chance.range_percentage;
        }
        if self.crit_damage.value != 0 {
            tooltips.push((
                format!(
                    "{}{}% Crit DMG",
                    if is_positive(self.crit_damage.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.crit_damage.value
                ),
                if let Some(crit_damage) = &base_att.unwrap().crit_damage {
                    format!(
                        "({}-{})",
                        f32::round(*crit_damage.start() as f32 * r) as i32,
                        f32::round(*crit_damage.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(
                            *bonus_att.unwrap().crit_damage.clone().unwrap().start() as f32 * r
                        ) as i32,
                        f32::round(
                            *bonus_att.unwrap().crit_damage.clone().unwrap().end() as f32 * r
                        ) as i32
                    )
                },
                self.crit_damage.quality,
            ));
            total_atts += 1.;
            total_score += self.crit_damage.range_percentage;
        }
        if self.bonus_damage.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Damage",
                    if is_positive(self.bonus_damage.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.bonus_damage.value
                ),
                if let Some(bonus_damage) = &base_att.unwrap().bonus_damage {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_damage.start() as f32 * r) as i32,
                        f32::round(*bonus_damage.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(
                            *bonus_att.unwrap().bonus_damage.clone().unwrap().start() as f32 * r
                        ) as i32,
                        f32::round(
                            *bonus_att.unwrap().bonus_damage.clone().unwrap().end() as f32 * r
                        ) as i32
                    )
                },
                self.bonus_damage.quality,
            ));
            total_atts += 1.;
            total_score += self.bonus_damage.range_percentage;
        }
        if self.health_regen.value != 0 {
            tooltips.push((
                format!(
                    "{}{} HP Regen",
                    if is_positive(self.health_regen.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.health_regen.value
                ),
                if let Some(health_regen) = &base_att.unwrap().health_regen {
                    format!(
                        "({}-{})",
                        f32::round(*health_regen.start() as f32 * r) as i32,
                        f32::round(*health_regen.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(
                            *bonus_att.unwrap().health_regen.clone().unwrap().start() as f32 * r
                        ) as i32,
                        f32::round(
                            *bonus_att.unwrap().health_regen.clone().unwrap().end() as f32 * r
                        ) as i32
                    )
                },
                self.health_regen.quality,
            ));
            total_atts += 1.;
            total_score += self.health_regen.range_percentage;
        }
        if self.healing.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Healing",
                    if is_positive(self.healing.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.healing.value
                ),
                if let Some(healing) = &base_att.unwrap().healing {
                    format!(
                        "({}-{})",
                        f32::round(*healing.start() as f32 * r) as i32,
                        f32::round(*healing.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().healing.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().healing.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.healing.quality,
            ));
            total_atts += 1.;
            total_score += self.healing.range_percentage;
        }
        if self.thorns.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Thorns",
                    if is_positive(self.thorns.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.thorns.value
                ),
                if let Some(thorns) = &base_att.unwrap().thorns {
                    format!(
                        "({}-{})",
                        f32::round(*thorns.start() as f32 * r) as i32,
                        f32::round(*thorns.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().thorns.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().thorns.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.thorns.quality,
            ));
            total_atts += 1.;
            total_score += self.thorns.range_percentage;
        }
        if self.speed.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Speed",
                    if is_positive(self.speed.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.speed.value
                ),
                if let Some(speed) = &base_att.unwrap().speed {
                    format!(
                        "({}-{})",
                        f32::round(*speed.start() as f32 * r) as i32,
                        f32::round(*speed.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().speed.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().speed.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.speed.quality,
            ));
            total_atts += 1.;
            total_score += self.speed.range_percentage;
        }
        if self.lifesteal.value != 0 {
            tooltips.push((
                format!(
                    "{}{} Lifesteal",
                    if is_positive(self.lifesteal.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.lifesteal.value
                ),
                if let Some(lifesteal) = &base_att.unwrap().lifesteal {
                    format!(
                        "({}-{})",
                        f32::round(*lifesteal.start() as f32 * r) as i32,
                        f32::round(*lifesteal.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(
                            *bonus_att.unwrap().lifesteal.clone().unwrap().start() as f32 * r
                        ) as i32,
                        f32::round(*bonus_att.unwrap().lifesteal.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.lifesteal.quality,
            ));
            total_atts += 1.;
            total_score += self.lifesteal.range_percentage;
        }

        if self.xp_rate.value != 0 {
            tooltips.push((
                format!(
                    "{}{}% XP",
                    if is_positive(self.xp_rate.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.xp_rate.value
                ),
                if let Some(xp_rate) = &base_att.unwrap().xp_rate {
                    format!(
                        "({}-{})",
                        f32::round(*xp_rate.start() as f32 * r) as i32,
                        f32::round(*xp_rate.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(*bonus_att.unwrap().xp_rate.clone().unwrap().start() as f32 * r)
                            as i32,
                        f32::round(*bonus_att.unwrap().xp_rate.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.xp_rate.quality,
            ));
            total_atts += 1.;
            total_score += self.xp_rate.range_percentage;
        }
        if self.loot_rate.value != 0 {
            tooltips.push((
                format!(
                    "{}{}% Loot",
                    if is_positive(self.loot_rate.value) {
                        "+"
                    } else {
                        ""
                    },
                    self.loot_rate.value
                ),
                if let Some(loot_rate) = &base_att.unwrap().loot_rate {
                    format!(
                        "({}-{})",
                        f32::round(*loot_rate.start() as f32 * r) as i32,
                        f32::round(*loot_rate.end() as f32 * r) as i32
                    )
                } else {
                    format!(
                        "({}-{})",
                        f32::round(
                            *bonus_att.unwrap().loot_rate.clone().unwrap().start() as f32 * r
                        ) as i32,
                        f32::round(*bonus_att.unwrap().loot_rate.clone().unwrap().end() as f32 * r)
                            as i32
                    )
                },
                self.loot_rate.quality,
            ));
            total_atts += 1.;
            total_score += self.loot_rate.range_percentage;
        }
        let ratio = if total_atts == 0. {
            0.
        } else {
            total_score / total_atts
        };
        (tooltips, ratio, total_atts)
    }
    pub fn get_stats_summary(&self) -> Vec<(String, String)> {
        let mut tooltips: Vec<(String, String)> = vec![];
        tooltips.push(("Health:          ".to_string(), format!("{}", self.health)));
        tooltips.push((
            "Attack:             ".to_string(),
            format!("{}", self.attack + self.bonus_damage),
        ));
        tooltips.push(("Defence:        ".to_string(), format!("{}", self.defence)));
        tooltips.push((
            "Crit Chance:     ".to_string(),
            format!("{}", self.crit_chance),
        ));
        tooltips.push((
            "Crit Damage:   ".to_string(),
            format!("{}", self.crit_damage),
        ));
        tooltips.push((
            "Health Regen:  ".to_string(),
            format!("{}", self.health_regen),
        ));
        tooltips.push(("Healing:         ".to_string(), format!("{}", self.healing)));
        tooltips.push(("Thorns:           ".to_string(), format!("{}", self.thorns)));
        tooltips.push(("Dodge:            ".to_string(), format!("{}", self.dodge)));
        tooltips.push(("Speed:          ".to_string(), format!("{}", self.speed)));
        tooltips.push((
            "Lifesteal:      ".to_string(),
            format!("{}", self.lifesteal),
        ));

        tooltips.push(("XP: ".to_string(), format!("{}", self.xp_rate)));
        tooltips.push(("Loot: ".to_string(), format!("{}", self.loot_rate)));

        tooltips
    }
    pub fn add_attribute_components(
        &self,
        entity: &mut EntityCommands,
        old_max_health: i32,
        skills: &PlayerSkills,
    ) {
        let computed_health = self.health + skills.get_count(Skill::Health) * 25;
        if self.health.value > 0 && computed_health.value != old_max_health {
            entity.insert(MaxHealth(computed_health.value));
        }
        if self.attack_cooldown > 0. {
            entity.insert(AttackCooldown(
                self.attack_cooldown * (1.0 - skills.get_count(Skill::AttackSpeed) as f32 * 0.15),
            ));
        } else {
            entity.remove::<AttackCooldown>();
        }

        entity.insert(Attack(self.attack.value));
        entity.insert(CritChance(
            self.crit_chance.value + skills.get_count(Skill::CritChance) * 10,
        ));
        entity.insert(CritDamage(
            self.crit_damage.value + skills.get_count(Skill::CritDamage) * 15,
        ));
        entity.insert(BonusDamage(self.bonus_damage.value));
        entity.insert(HealthRegen(self.health_regen.value));
        entity.insert(Healing(self.healing.value));
        entity.insert(Thorns(
            self.thorns.value + skills.get_count(Skill::Thorns) * 15,
        ));
        entity.insert(Dodge(i32::min(
            80,
            self.dodge.value + skills.get_count(Skill::DodgeChance) * 10,
        )));
        entity.insert(Speed(
            self.speed.value + skills.get_count(Skill::Speed) * 15,
        ));
        entity.insert(Lifesteal(
            self.lifesteal.value + skills.get_count(Skill::Lifesteal) * 10,
        ));
        entity.insert(Defence(self.defence.value));
        entity.insert(XpRateBonus(self.xp_rate.value));
        entity.insert(LootRateBonus(self.loot_rate.value));
    }
    pub fn change_attribute(&mut self, modifier: AttributeModifier) -> &Self {
        match modifier.modifier.as_str() {
            "health" => self.health.value += modifier.delta,
            "attack" => self.attack.value += modifier.delta,
            "durability" => self.durability.value += modifier.delta,
            "max_durability" => self.max_durability.value += modifier.delta,
            "attack_cooldown" => self.attack_cooldown += modifier.delta as f32,
            "invincibility_cooldown" => self.invincibility_cooldown += modifier.delta as f32,
            _ => warn!("Got an unexpected attribute: {:?}", modifier.modifier),
        }
        self
    }
    pub fn combine(&self, other: &ItemAttributes) -> ItemAttributes {
        ItemAttributes {
            health: self.health + other.health,
            attack: self.attack + other.attack,
            durability: self.durability + other.durability,
            max_durability: self.max_durability + other.max_durability,
            attack_cooldown: self.attack_cooldown + other.attack_cooldown,
            invincibility_cooldown: self.invincibility_cooldown + other.invincibility_cooldown,
            crit_chance: self.crit_chance + other.crit_chance,
            crit_damage: self.crit_damage + other.crit_damage,
            bonus_damage: self.bonus_damage + other.bonus_damage,
            health_regen: self.health_regen + other.health_regen,
            healing: self.healing + other.healing,
            thorns: self.thorns + other.thorns,
            dodge: self.dodge + other.dodge,
            speed: self.speed + other.speed,
            lifesteal: self.lifesteal + other.lifesteal,
            defence: self.defence + other.defence,
            xp_rate: self.xp_rate + other.xp_rate,
            loot_rate: self.loot_rate + other.loot_rate,
        }
    }
}
macro_rules! setup_raw_bonus_attributes {
    (struct $name:ident {
        $($field_name:ident: $field_type:ty,)*
    }) => {
        #[derive(Component, PartialEq, Clone, Reflect, FromReflect, Schematic, Default, Debug)]
        #[reflect(Schematic, Default)]
        pub struct $name {
            pub $($field_name: $field_type,)*
        }

        impl $name {

            pub fn into_item_attributes(
                &self,
                rarity: ItemRarity,
                item_type: &EquipmentType
            ) -> ItemAttributes {
                // take fields of Range<i32> into one i32
                let mut rng = rand::thread_rng();
                let num_bonus_attributes = rarity.get_num_bonus_attributes(item_type);
                let num_attributes = rng.gen_range(num_bonus_attributes);
                let mut item_attributes = ItemAttributes::default();
                let valid_attributes = {
                    let mut v = Vec::new();
                    $(
                        if self.$field_name.is_some() {
                            v.push(stringify!($field_name))
                        }
                    )*
                    v
                };
                let num_valid_attributes = valid_attributes.len();
                let mut already_picked_attributes = Vec::new();
                for _ in 0..num_attributes {
                    let picked_attribute_index = rng.gen_range(0..num_valid_attributes);
                    let mut picked_attribute = valid_attributes[picked_attribute_index];
                    while already_picked_attributes.contains(&picked_attribute) {
                        let picked_attribute_index = rng.gen_range(0..num_valid_attributes);
                        picked_attribute = valid_attributes[picked_attribute_index];
                    }
                    already_picked_attributes.push(picked_attribute);
                    $(
                        {
                            if stringify!($field_name) == picked_attribute {
                                let min = f32::round(*self.$field_name.clone().unwrap().start() as f32 * rarity.get_rarity_attributes_bonus()) as i32;
                                let max = f32::round(*self.$field_name.clone().unwrap().end()  as f32 * rarity.get_rarity_attributes_bonus()) as i32;
                                let range = (min)..=(max);
                                let total_range = range.end() - range.start();
                                let value = rng.gen_range(range.clone());
                                let percent_of_total_range = (value - range.start()) as f32 / total_range as f32;
                                item_attributes.$field_name = AttributeValue::new(
                                        value,
                                        AttributeQuality::get_quality(range, value),
                                        percent_of_total_range
                                    );
                            }
                        }
                    )*
                }

                item_attributes
            }
        }
    }
}
macro_rules! setup_raw_base_attributes {
    (struct $name:ident {
        $($field_name:ident: $field_type:ty,)*
    }) => {
        #[derive(Component, PartialEq, Clone, Reflect, FromReflect, Schematic, Default, Debug)]
        #[reflect(Schematic, Default)]
        pub struct $name {
            pub $($field_name: $field_type,)*
        }

        impl $name {

            pub fn into_item_attributes(
                &self,
                rarity: ItemRarity,
                attack_cooldown: f32,
            ) -> ItemAttributes {
                let mut rng = rand::thread_rng();
                let mut item_attributes = ItemAttributes{ attack_cooldown, ..default()};
                let valid_attributes = {
                    let mut v = Vec::new();
                    $(
                        if self.$field_name.is_some() {
                            v.push(stringify!($field_name))
                        }
                    )*
                    v
                };
                for att in valid_attributes.iter() {
                    $(
                        {
                            if stringify!($field_name) == *att {
                                let min = f32::round(*self.$field_name.clone().unwrap().start() as f32 * rarity.get_rarity_attributes_bonus()) as i32;
                                let max = f32::round(*self.$field_name.clone().unwrap().end()  as f32 * rarity.get_rarity_attributes_bonus()) as i32;
                                let range = (min)..=(max);
                                let total_range = range.end() - range.start();
                                let value = rng.gen_range(range.clone());
                                let percent_of_total_range = (value - range.start()) as f32 / total_range as f32;
                                item_attributes.$field_name = AttributeValue::new(
                                                value,
                                                AttributeQuality::get_quality(range, value),
                                                percent_of_total_range
                                            );
                            }
                        }
                    )*
                }

                item_attributes
            }
        }
    }
}

setup_raw_bonus_attributes! { struct RawItemBonusAttributes {
    attack: Option<RangeInclusive<i32>>,
     health: Option<RangeInclusive<i32>>,
     defence: Option<RangeInclusive<i32>>,
     durability: Option<RangeInclusive<i32>>,
     max_durability: Option<RangeInclusive<i32>>,
    //
     crit_chance: Option<RangeInclusive<i32>>,
     crit_damage: Option<RangeInclusive<i32>>,
     bonus_damage: Option<RangeInclusive<i32>>,
     health_regen: Option<RangeInclusive<i32>>,
     healing: Option<RangeInclusive<i32>>,
     thorns: Option<RangeInclusive<i32>>,
     dodge: Option<RangeInclusive<i32>>,
     speed: Option<RangeInclusive<i32>>,
     lifesteal: Option<RangeInclusive<i32>>,
     xp_rate: Option<RangeInclusive<i32>>,
     loot_rate: Option<RangeInclusive<i32>>,
}}

setup_raw_base_attributes! { struct RawItemBaseAttributes {
    attack: Option<RangeInclusive<i32>>,
     health: Option<RangeInclusive<i32>>,
     defence: Option<RangeInclusive<i32>>,
     durability: Option<RangeInclusive<i32>>,
     max_durability: Option<RangeInclusive<i32>>,
    //
     crit_chance: Option<RangeInclusive<i32>>,
     crit_damage: Option<RangeInclusive<i32>>,
     bonus_damage: Option<RangeInclusive<i32>>,
     health_regen: Option<RangeInclusive<i32>>,
     healing: Option<RangeInclusive<i32>>,
     thorns: Option<RangeInclusive<i32>>,
     dodge: Option<RangeInclusive<i32>>,
     speed: Option<RangeInclusive<i32>>,
     lifesteal: Option<RangeInclusive<i32>>,
     xp_rate: Option<RangeInclusive<i32>>,
     loot_rate: Option<RangeInclusive<i32>>,
}}

#[derive(
    Component,
    Reflect,
    FromReflect,
    Debug,
    Schematic,
    Clone,
    Default,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
)]
#[reflect(Component, Schematic)]
pub enum ItemRarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Legendary,
}
#[derive(EnumIter, Display, Eq, PartialEq, Clone, Hash)]
pub enum ItemGlow {
    Green,
    Blue,
    Red,
}

impl ItemRarity {
    pub fn get_num_bonus_attributes(&self, eqp_type: &EquipmentType) -> RangeInclusive<i32> {
        let acc_offset = if eqp_type.is_accessory() { 1 } else { 0 };
        match self {
            ItemRarity::Common => acc_offset..=(1 + acc_offset),
            ItemRarity::Uncommon => (1 + acc_offset)..=(2 + acc_offset),
            ItemRarity::Rare => (2 + acc_offset)..=(3 + acc_offset),
            ItemRarity::Legendary => (4 + acc_offset)..=(5 + acc_offset),
        }
    }
    fn get_rarity_attributes_bonus(&self) -> f32 {
        match self {
            ItemRarity::Common => 1.0,
            ItemRarity::Uncommon => 1.2,
            ItemRarity::Rare => 1.45,
            ItemRarity::Legendary => 1.8,
        }
    }

    pub fn get_tooltip_ui_element(&self) -> UIElement {
        match self {
            ItemRarity::Common => UIElement::LargeTooltipCommon,
            ItemRarity::Uncommon => UIElement::LargeTooltipUncommon,
            ItemRarity::Rare => UIElement::LargeTooltipRare,
            ItemRarity::Legendary => UIElement::LargeTooltipLegendary,
        }
    }
    pub fn get_color(&self) -> Color {
        match self {
            ItemRarity::Common => LIGHT_GREY,
            ItemRarity::Uncommon => UNCOMMON_GREEN,
            ItemRarity::Rare => LIGHT_BLUE,
            ItemRarity::Legendary => LIGHT_RED,
        }
    }
    pub fn get_next_rarity(&self) -> ItemRarity {
        match self {
            ItemRarity::Common => ItemRarity::Uncommon,
            ItemRarity::Uncommon => ItemRarity::Rare,
            ItemRarity::Rare => ItemRarity::Legendary,
            ItemRarity::Legendary => ItemRarity::Legendary,
        }
    }
    pub fn get_item_glow(&self) -> Option<ItemGlow> {
        match self {
            ItemRarity::Common => None,
            ItemRarity::Uncommon => Some(ItemGlow::Green),
            ItemRarity::Rare => Some(ItemGlow::Blue),
            ItemRarity::Legendary => Some(ItemGlow::Red),
        }
    }
    pub fn get_scrap(&self) -> ScrapsInto {
        match self {
            ItemRarity::Common => ScrapsInto(vec![]),
            ItemRarity::Uncommon => ScrapsInto(vec![Scrap::new(WorldObject::UpgradeTome, 0.06)]),
            ItemRarity::Rare => ScrapsInto(vec![
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::UpgradeTome, 0.17),
            ]),
            ItemRarity::Legendary => ScrapsInto(vec![
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::OrbOfTransformation, 0.5),
                Scrap::new(WorldObject::UpgradeTome, 0.4),
            ]),
        }
    }
}

#[derive(Reflect, FromReflect, Default, Component, Clone, Debug, Copy)]
#[reflect(Component)]
pub struct ItemLevel(pub u8);

pub struct AttributeModifier {
    pub modifier: String,
    pub delta: i32,
}

#[derive(Debug, Clone, Default)]
pub struct AttributeChangeEvent;

#[derive(Bundle, Clone, Debug, Copy, Default)]
pub struct PlayerAttributeBundle {
    pub health: MaxHealth,
    pub mana: Mana,
    pub attack: Attack,
    pub attack_cooldown: AttackCooldown,
    pub defence: Defence,
    pub crit_chance: CritChance,
    pub crit_damage: CritDamage,
    pub bonus_damage: BonusDamage,
    pub health_regen: HealthRegen,
    pub healing: Healing,
    pub thorns: Thorns,
    pub dodge: Dodge,
    pub speed: Speed,
    pub lifesteal: Lifesteal,
    pub xp_rate: XpRateBonus,
    pub mana_regen: ManaRegen,
    pub loot_rate: LootRateBonus,
}

//TODO: Add max health vs curr health
#[derive(
    Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy, Serialize, Deserialize,
)]
#[reflect(Component, Schematic)]
pub struct CurrentHealth(pub i32);
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct Mana {
    pub max: i32,
    pub current: i32,
}
impl Mana {
    pub fn new(max: i32) -> Self {
        Self { max, current: max }
    }
}

#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct MaxHealth(pub i32);
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct Attack(pub i32);
#[derive(Reflect, FromReflect, Default, Component, Clone, Debug, Copy)]
#[reflect(Component)]
pub struct Durability(pub i32);

#[derive(Reflect, FromReflect, Default, Component, Clone, Debug, Copy)]
#[reflect(Component)]
pub struct AttackCooldown(pub f32);
#[derive(Reflect, FromReflect, Default, Component, Clone, Debug, Copy)]
#[reflect(Component)]
pub struct InvincibilityCooldown(pub f32);

#[derive(Default, Component, Clone, Debug, Copy)]
pub struct CritChance(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct CritDamage(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct BonusDamage(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct HealthRegen(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct Healing(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct Thorns(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct Dodge(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct Speed(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct Lifesteal(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct Defence(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct XpRateBonus(pub i32);
#[derive(Default, Component, Clone, Debug, Copy)]
pub struct LootRateBonus(pub i32);

#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct ManaRegen(pub i32);

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AttributeChangeEvent>()
            .add_event::<ModifyHealthEvent>()
            .add_event::<ModifyManaEvent>()
            .add_systems(
                (
                    clamp_health,
                    clamp_mana,
                    handle_actions_drain_hunger,
                    tick_hunger,
                    handle_modify_health_event.before(clamp_health),
                    handle_modify_mana_event.before(clamp_mana),
                    add_current_health_with_max_health,
                    handle_health_regen,
                    handle_mana_regen,
                    update_attributes_with_held_item_change,
                    update_attributes_and_sprite_with_equipment_change,
                    update_sprite_with_equipment_removed,
                    handle_item_abilitiy_on_attack,
                    handle_new_items_raw_attributes.before(CustomFlush),
                    handle_player_item_attribute_change_events.after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

fn clamp_health(
    mut health: Query<(&mut CurrentHealth, &MaxHealth), With<Player>>,
    mut game_over_event: EventWriter<GameOverEvent>,
) {
    for (mut h, max_h) in health.iter_mut() {
        if h.0 <= 0 {
            h.0 = 0;
            game_over_event.send_default();
        } else if h.0 > max_h.0 {
            h.0 = max_h.0;
        }
    }
}
fn clamp_mana(mut health: Query<&mut Mana, With<Player>>) {
    for mut m in health.iter_mut() {
        if m.current < 0 {
            m.current = 0;
        } else if m.current > m.max {
            m.current = m.max;
        }
    }
}
fn handle_player_item_attribute_change_events(
    mut commands: Commands,
    player: Query<(Entity, &Inventory), With<Player>>,
    eqp_attributes: Query<&ItemAttributes, With<Equipment>>,
    mut att_events: EventReader<AttributeChangeEvent>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    player_atts: Query<(&ItemAttributes, &PlayerSkills, &MaxHealth), With<Player>>,
    stat_button: Query<(&UIElement, &StatsButtonState)>,
    ui_state: Res<State<UIState>>,
) {
    for _event in att_events.iter() {
        let (att, skills, old_health) = player_atts.single();
        let mut new_att = att.clone();
        let (player, inv) = player.single();
        let equips: Vec<ItemAttributes> = inv
            .equipment_items
            .items
            .iter()
            .chain(inv.accessory_items.items.iter())
            .flatten()
            .map(|e| e.item_stack.attributes.clone())
            .collect();

        for a in eqp_attributes.iter().chain(equips.iter()) {
            new_att = new_att.combine(a);
        }
        if new_att.attack_cooldown == 0. {
            new_att.attack_cooldown = 0.4;
        }
        new_att.add_attribute_components(&mut commands.entity(player), old_health.0, skills);
        let stat = if let Some((_, stat_state)) = stat_button
            .iter()
            .find(|(ui, _)| ui == &&UIElement::StatsButtonHover)
        {
            Some(StatType::from_index(stat_state.index))
        } else {
            None
        };
        if ui_state.0.is_inv_open() {
            stats_event.send(ShowInvPlayerStatsEvent {
                stat,
                ignore_timer: true,
            });
        }
    }
}

/// Adds a current health component to all entities with a max health component
pub fn add_current_health_with_max_health(
    mut commands: Commands,
    mut health: Query<(Entity, &MaxHealth), (Changed<MaxHealth>, Without<CurrentHealth>)>,
) {
    for (entity, max_health) in health.iter_mut() {
        commands.entity(entity).insert(CurrentHealth(max_health.0));
    }
}

///Tracks player held item changes, spawns new held item entity and updates player attributes
fn update_attributes_with_held_item_change(
    mut commands: Commands,
    mut game_param: GameParam,
    inv_state: Res<InventoryState>,
    mut inv: Query<&mut Inventory>,
    item_stack_query: Query<&ItemAttributes>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    proto: ProtoParam,
) {
    let active_hotbar_slot = inv_state.active_hotbar_slot;
    let active_hotbar_item = inv.single_mut().items.items[active_hotbar_slot].clone();
    let player_data = game_param.player_mut();
    let prev_held_item_data = &player_data.main_hand_slot;
    if let Some(new_item) = active_hotbar_item {
        let new_item_stack = new_item.item_stack.clone();
        if let Some(current_item) = prev_held_item_data {
            let curr_attributes = item_stack_query.get(current_item.entity).unwrap();
            let new_attributes = &(new_item.item_stack.attributes);
            if new_item_stack != current_item.item_stack {
                new_item.spawn_item_on_hand(&mut commands, &mut game_param, &proto);
                att_event.send(AttributeChangeEvent);
            } else if curr_attributes != new_attributes {
                commands
                    .entity(current_item.entity)
                    .insert(new_attributes.clone());
                att_event.send(AttributeChangeEvent);
            }
        } else {
            new_item.spawn_item_on_hand(&mut commands, &mut game_param, &proto);
            att_event.send(AttributeChangeEvent);
        }
    } else if let Some(current_item) = prev_held_item_data {
        commands.entity(current_item.entity).despawn();
        player_data.main_hand_slot = None;
        att_event.send(AttributeChangeEvent);
    }
}
///Tracks player equip or accessory inventory slot changes,
///spawns new held equipment entity, and updates player attributes
fn update_attributes_and_sprite_with_equipment_change(
    player_limbs: Query<(&mut Handle<AnimatedTextureMaterial>, &Limb)>,
    asset_server: Res<AssetServer>,
    proto_param: ProtoParam,
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    mut events: EventReader<DropOnSlotEvent>,
) {
    for drop in events.iter() {
        if drop.drop_target_slot_state.r#type.is_equipment()
            || drop.drop_target_slot_state.r#type.is_accessory()
        {
            let slot = drop.drop_target_slot_state.slot_index;
            let Some(eqp_type) =
                proto_param.get_component::<EquipmentType, _>(drop.dropped_item_stack.obj_type)
            else {
                continue;
            };
            if !eqp_type.is_equipment() || !eqp_type.get_valid_slots().contains(&slot) {
                continue;
            }
            att_event.send(AttributeChangeEvent);
            if drop.drop_target_slot_state.r#type.is_equipment() {
                for (mat, limb) in player_limbs.iter() {
                    if Limb::from_slot(slot).contains(limb) {
                        let mat = materials.get_mut(mat).unwrap();
                        let armor_texture_handle = asset_server.load(format!(
                            "textures/player/{}.png",
                            drop.dropped_item_stack.obj_type
                        ));
                        mat.lookup_texture = Some(armor_texture_handle);
                    }
                }
            }
        }
    }
}
///Tracks player equip or accessory inventory slot changes,
///spawns new held equipment entity, and updates player attributes
fn update_sprite_with_equipment_removed(
    mut removed_inv_item: EventReader<RemoveFromSlotEvent>,
    player_limbs: Query<(&mut Handle<AnimatedTextureMaterial>, &Limb)>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
) {
    for item in removed_inv_item.iter() {
        if item.removed_slot_state.r#type.is_equipment() {
            for (mat, limb) in player_limbs.iter() {
                if Limb::from_slot(item.removed_slot_state.slot_index).contains(limb) {
                    let mat = materials.get_mut(mat).unwrap();
                    let armor_texture_handle = asset_server.load(format!(
                        "textures/player/player-texture-{}.png",
                        if limb == &Limb::Torso || limb == &Limb::Hands {
                            Limb::Torso.to_string().to_lowercase()
                        } else {
                            limb.to_string().to_lowercase()
                        }
                    ));
                    mat.lookup_texture = Some(armor_texture_handle);
                }
            }
        }
    }
}
fn handle_new_items_raw_attributes(
    mut commands: Commands,
    new_items: Query<
        (
            Entity,
            &ItemStack,
            Option<&RawItemBonusAttributes>,
            &RawItemBaseAttributes,
            &EquipmentType,
            Option<&ItemLevel>,
        ),
        Or<(Added<RawItemBaseAttributes>, Added<RawItemBonusAttributes>)>,
    >,
    graphics: Res<Graphics>,
) {
    for (e, stack, raw_bonus_att_option, raw_base_att, eqp_type, item_level) in new_items.iter() {
        let rarity = get_rarity_rng(rand::thread_rng());
        add_item_glows(&mut commands, &graphics, e, rarity.clone());

        let new_stack = build_item_stack_with_parsed_attributes(
            stack,
            raw_base_att,
            raw_bonus_att_option,
            rarity,
            eqp_type,
            item_level.map(|l| l.0),
        );

        commands.entity(e).insert(new_stack);
    }
}

pub fn add_item_glows(
    commands: &mut Commands,
    graphics: &Graphics,
    new_item_e: Entity,
    rarity: ItemRarity,
) -> Option<Entity> {
    rarity.get_item_glow().map(|glow| {
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_item_glow(glow.clone()),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(18., 18.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(0., 0., -1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .set_parent(new_item_e)
            .id()
    })
}
