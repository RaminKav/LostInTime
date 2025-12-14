use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use item_abilities::handle_item_abilitiy_on_attack;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    cmp::{max, min},
    ops::{Add, RangeInclusive},
};
use strum_macros::{Display, EnumIter};

use bevy::{ecs::system::EntityCommands, prelude::*};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
pub mod health_regen;
pub mod modifiers;
use crate::{
    animations::{AnimatedTextureMaterial, DoneAnimation},
    assets::Graphics,
    attributes::attribute_helpers::{build_item_stack_with_parsed_attributes, get_rarity_rng},
    client::{is_not_paused, GameOverEvent},
    colors::{GREY, LIGHT_BLUE, LIGHT_GREY, LIGHT_RED, ORANGE, UNCOMMON_GREEN},
    inputs::player_move_inputs,
    inventory::{Inventory, ItemStack},
    item::{BonusStatLine, Equipment, EquipmentType, WorldObject},
    juice::ShakeEffect,
    player::{
        levels::{handle_level_up, PlayerLevel},
        skills::{Heirloom, PlayerClass, PlayerSkills},
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
    CustomFlush, Game, GameParam, GameState, Player, TextureCamera,
};
use modifiers::*;
pub mod attribute_helpers;
pub mod hunger;
use hunger::*;
pub mod item_abilities;

use self::health_regen::{handle_health_regen, handle_mana_regen};
pub struct AttributesPlugin;

aseprite!(pub RarityGlows, "textures/effects/RarityGlows.aseprite");
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
    pub shield: AttributeValue,
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
    pub mana: AttributeValue,
    pub size: AttributeValue,
    pub attack_speed: AttributeValue,
    pub mana_regen: AttributeValue,
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
    /// Helper function to format a stat line attribute name and value into a tooltip string
    fn format_stat_line_tooltip(attribute_name: &str, value: i32) -> String {
        let is_positive = value > 0;
        let sign = if is_positive { "+" } else { "" };
        match attribute_name {
            "health" => format!("{}{} HP", sign, value),
            "shield" => format!("{}{} Shield", sign, value),
            "attack" => format!("{}{} Attack", sign, value),
            "crit_chance" => format!("{}{}% Crit Chance", sign, value),
            "crit_damage" => format!("{}{}% Crit DMG", sign, value),
            "bonus_damage" => format!("{}{}% Damage", sign, value),
            "health_regen" => format!("{}{} HP Regen", sign, value),
            "healing" => format!("{}{} Healing", sign, value),
            "thorns" => format!("{}{} Thorns", sign, value),
            "dodge" => format!("{}{}% Dodge", sign, value),
            "speed" => format!("{}{}% Speed", sign, value),
            "lifesteal" => format!("{}{}% Lifesteal", sign, value),
            "defence" => format!("{}{} Defence", sign, value),
            "attack_speed" => format!("{}{}% Attack Speed", sign, value),
            "loot_rate" => format!("{}{}% Luck", sign, value),
            "mana" => format!("{}{} Mana", sign, value),
            "size" | "projectile_size" => format!("{}{}% Size", sign, value),
            "xp_rate" => format!("{}{}% XP", sign, value),
            "mana_regen" => format!("{}{} MP Regen", sign, value),
            "durability" => format!("{}{} Durability", sign, value),
            "max_durability" => format!("{}{} Max Durability", sign, value),
            _ => format!("{}{} {}", sign, value, attribute_name),
        }
    }

    /// Generate tooltips from individual bonus stat lines (allows duplicate stats)
    pub fn get_tooltips_from_stat_lines(
        stat_lines: &[crate::item::BonusStatLine],
        combined_attrs: &ItemAttributes, // Combined base + bonus attributes
        rarity: ItemRarity,
        base_att: Option<&RawItemBaseAttributes>,
        bonus_att: Option<&RawItemBonusAttributes>,
        level: i32,
        obj: WorldObject,
        equip_type: &EquipmentType,
    ) -> (Vec<(String, String, AttributeQuality)>, f32, f32) {
        let mut tooltips: Vec<(String, String, AttributeQuality)> = vec![];
        let r = rarity.get_rarity_attributes_bonus();
        let mut total_score = 0.;
        let mut total_atts = 0.;
        let is_cape = equip_type.is_cape();

        let base_att_lvl_bonus = if equip_type.is_weapon() || equip_type.is_tool() {
            (level - 1) as f32 * obj.get_weapon_levelup_upgrade() as f32
        } else {
            0.
        };
        let (base_hp_lvl_bonus, base_def_lvl_bonus) =
            if equip_type.is_equipment() && !equip_type.is_accessory() {
                if level > 1 {
                    (max(0, (level * 2) - 1) as f32, max(0, level - 1) as f32)
                } else {
                    (0., 0.)
                }
            } else {
                (0., 0.)
            };

        // Track which attributes are base attributes so we can filter them from bonus stat lines
        let mut base_attribute_names = std::collections::HashSet::new();

        // Add base attributes first (check if they exist in raw_base_att and have non-zero values)
        if let Some(base_att) = base_att {
            if base_att.health.is_some() && combined_attrs.health.value != 0 {
                base_attribute_names.insert("health".to_string());
                tooltips.push((
                    format!(
                        "{}{} HP",
                        if combined_attrs.health.value > 0 {
                            "+"
                        } else {
                            ""
                        },
                        combined_attrs.health.value
                    ),
                    if is_cape {
                        "".to_string()
                    } else if let Some(health) = &base_att.health {
                        format!(
                            "({}-{})",
                            f32::round(*health.start() as f32 * r + base_hp_lvl_bonus) as i32,
                            f32::round(*health.end() as f32 * r + base_hp_lvl_bonus) as i32
                        )
                    } else {
                        "".to_string()
                    },
                    combined_attrs.health.quality,
                ));
            }
            if base_att.defence.is_some() && combined_attrs.defence.value != 0 {
                base_attribute_names.insert("defence".to_string());
                tooltips.push((
                    format!(
                        "{}{} Defence",
                        if combined_attrs.defence.value > 0 {
                            "+"
                        } else {
                            ""
                        },
                        combined_attrs.defence.value
                    ),
                    if is_cape {
                        "".to_string()
                    } else if let Some(defence) = &base_att.defence {
                        format!(
                            "({}-{})",
                            f32::round(*defence.start() as f32 * r + base_def_lvl_bonus) as i32,
                            f32::round(*defence.end() as f32 * r + base_def_lvl_bonus) as i32
                        )
                    } else {
                        "".to_string()
                    },
                    combined_attrs.defence.quality,
                ));
            }
            if base_att.attack.is_some() && combined_attrs.attack.value != 0 {
                base_attribute_names.insert("attack".to_string());
                tooltips.push((
                    format!(
                        "{}{} Attack",
                        if combined_attrs.attack.value > 0 {
                            "+"
                        } else {
                            ""
                        },
                        combined_attrs.attack.value
                    ),
                    if is_cape {
                        "".to_string()
                    } else if let Some(attack) = &base_att.attack {
                        format!(
                            "({}-{})",
                            f32::round(*attack.start() as f32 * r + base_att_lvl_bonus) as i32,
                            f32::round(*attack.end() as f32 * r + base_att_lvl_bonus) as i32
                        )
                    } else {
                        "".to_string()
                    },
                    combined_attrs.attack.quality,
                ));
            }
            // Check for speed as a base attribute
            if base_att.speed.is_some() && combined_attrs.speed.value != 0 {
                base_attribute_names.insert("speed".to_string());
                let base_speed_lvl_bonus = if let Some(speed) = &base_att.speed {
                    f32::round(*speed.start() as f32 * r * 0.1)
                } else {
                    0.
                };
                tooltips.push((
                    format!(
                        "{}{} Speed",
                        if combined_attrs.speed.value > 0 {
                            "+"
                        } else {
                            ""
                        },
                        combined_attrs.speed.value
                    ),
                    if is_cape {
                        "".to_string()
                    } else if let Some(speed) = &base_att.speed {
                        format!(
                            "({}-{})",
                            f32::round(*speed.start() as f32 * r + base_speed_lvl_bonus) as i32,
                            f32::round(*speed.end() as f32 * r + base_speed_lvl_bonus) as i32
                        )
                    } else {
                        "".to_string()
                    },
                    combined_attrs.speed.quality,
                ));
            }
        }

        // Add attack cooldown (Hits/s) if it exists
        if combined_attrs.attack_cooldown != 0. {
            tooltips.push((
                format!("{:.2} Hits/s", 1. / combined_attrs.attack_cooldown),
                "".to_string(),
                AttributeQuality::Average,
            ));
        }
        for stat_line in stat_lines {
            // Skip if this is a base attribute (already shown in base attributes section)
            if base_attribute_names.contains(&stat_line.attribute_name) {
                continue;
            }

            let tooltip_text =
                Self::format_stat_line_tooltip(&stat_line.attribute_name, stat_line.value);
            // For bonus attributes, we can show range if available
            let range_text = if is_cape {
                "".to_string()
            } else if let Some(bonus_att) = bonus_att {
                // Try to get range from bonus_att
                Self::get_range_text_for_attribute(&stat_line.attribute_name, bonus_att, r)
            } else {
                "".to_string()
            };
            tooltips.push((tooltip_text, range_text, stat_line.quality));
            total_atts += 1.;
            total_score += stat_line.range_percentage;
        }
        let ratio = if total_atts == 0. {
            0.
        } else {
            total_score / total_atts
        };
        (tooltips, ratio, total_atts)
    }

    /// Helper to get range text for an attribute from RawItemBonusAttributes
    fn get_range_text_for_attribute(
        _attribute_name: &str,
        _bonus_att: &RawItemBonusAttributes,
        _r: f32,
    ) -> String {
        //TODO: Decide if we want range text anymore, if so, gotta fix this.
        "".to_string()
    }

    pub fn get_stats_summary(&self) -> Vec<(String, String)> {
        let mut tooltips: Vec<(String, String)> = vec![];
        tooltips.push(("Health:          ".to_string(), format!("{}", self.health)));
        tooltips.push(("Mana:          ".to_string(), format!("{}", self.mana)));
        tooltips.push((
            "Attack:             ".to_string(),
            format!(
                "{}",
                f32::floor(self.attack.value as f32 * (1. + self.bonus_damage.value as f32 / 100.))
            ),
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

        tooltips.push(("XP: ".to_string(), format!("{}", self.xp_rate)));
        tooltips.push(("Luck: ".to_string(), format!("{}", self.loot_rate)));

        tooltips
    }
    pub fn add_attribute_components(
        &self,
        entity: &mut EntityCommands,
        old_max_health: i32,
        old_max_mana: i32,
        old_shield: i32,
        skills: &PlayerSkills,
        dodge_crit_buff_active: bool,
        coins: u32,
        max_hp_hunt_bonus: i32, // Max HP gained from MaxHPHunt heirloom
    ) {
        // ChaosStats: +10 to many stats per stack
        let chaos_stats_stacks = skills.get_count(Heirloom::ChaosStats);
        let chaos_health_bonus = chaos_stats_stacks * 10;
        let chaos_mana_bonus = chaos_stats_stacks * 10;
        let chaos_dmg_bonus = chaos_stats_stacks * 10; // +10% dmg
        let chaos_defence_bonus = chaos_stats_stacks * 10;
        let chaos_crit_bonus = chaos_stats_stacks * 10; // +10% crit
        let chaos_speed_bonus = chaos_stats_stacks * 10; // +10% speed
        let chaos_dodge_bonus = chaos_stats_stacks * 10;

        // Note: Hallucination stats from LethalBlow are now combined via ItemAttributes::combine()
        // before this function is called, so they're already included in self.

        let computed_health = self.health
            + skills.get_count(Heirloom::Health) * 25
            + chaos_health_bonus
            + max_hp_hunt_bonus;
        let computed_speed = self.speed.value
            + chaos_speed_bonus
            + if dodge_crit_buff_active { 30 } else { 0 } // DodgeCrit speed buff
            - if skills.has(Heirloom::ReinforcedArmor) {
                5
            } else {
                0
            };
        if self.mana.value > 0 && self.mana.value + chaos_mana_bonus != old_max_mana {
            entity.insert(MaxMana(self.mana.value + chaos_mana_bonus));
        }
        if self.health.value > 0 && computed_health.value != old_max_health {
            entity.insert(MaxHealth(computed_health.value));
        }
        if skills.get_count(Heirloom::Shield) * 10 != old_shield {
            entity.insert(MaxShield(skills.get_count(Heirloom::Shield) * 10));
        }
        info!("ATTACK SPEED {:?}", self.attack_speed.value);
        if self.attack_cooldown > 0. {
            let attack_speed_mod = 1. + self.attack_speed.value as f32 / 100.;
            let dodge_crit_attack_speed_mod = if dodge_crit_buff_active { 1.3 } else { 1.0 };
            entity.insert(AttackCooldown(
                self.attack_cooldown
                    * (1.0 - skills.get_count(Heirloom::AttackSpeed) as f32 * 0.15)
                    / attack_speed_mod
                    / dodge_crit_attack_speed_mod,
            ));
        } else {
            entity.remove::<AttackCooldown>();
        }

        entity.insert(Attack(self.attack.value));
        entity.insert(CritChance(
            self.crit_chance.value + skills.get_count(Heirloom::CritChance) * 7 + chaos_crit_bonus,
        ));
        entity.insert(CritDamage(
            self.crit_damage.value + skills.get_count(Heirloom::CritDamage) * 15,
        ));

        // Calculate dynamic damage bonuses
        let mut total_bonus_damage =
            self.bonus_damage.value + skills.get_count(Heirloom::Attack) * 10 + chaos_dmg_bonus;

        // MaxHPDamage: +10% damage per 100 max hp per stack
        let max_hp_damage_stacks = skills.get_count(Heirloom::MaxHPDamage);
        if max_hp_damage_stacks > 0 {
            // Use computed_health which was calculated earlier in this function
            let hp_bonus_percent =
                (computed_health.value as f32 / 100.0) * 10.0 * max_hp_damage_stacks as f32;
            total_bonus_damage += hp_bonus_percent as i32;
        }

        // GoldIntoDamage: +1% damage per 10 coins per stack
        let gold_damage_stacks = skills.get_count(Heirloom::GoldIntoDamage);
        if gold_damage_stacks > 0 {
            let gold_bonus_percent = (coins as f32 / 10.0) * 1.0 * gold_damage_stacks as f32;
            total_bonus_damage += gold_bonus_percent as i32;
        }

        entity.insert(BonusDamage(total_bonus_damage));
        // RegenLifesteal: -5 hp regen, +5% lifesteal per stack
        // Allow negative values - negative regen will damage the player on regen ticks
        let regen_lifesteal_stacks = skills.get_count(Heirloom::RegenLifesteal);
        entity.insert(HealthRegen(
            self.health_regen.value
                + skills.get_count(Heirloom::HPRegen) * 5
                + skills.get_count(Heirloom::HealEcho) * 20
                - regen_lifesteal_stacks * 5,
        ));
        entity.insert(Healing(self.healing.value));

        // ThornArmor: +10 defence per stack, +20% thorns per 10 defence per stack
        // Thorns now work as a percentage of player damage reflected back
        let thorn_armor_stacks = skills.get_count(Heirloom::ThornArmor);
        let base_thorns = self.thorns.value + skills.get_count(Heirloom::Thorns) * 15;
        // Calculate defence first (including ThornArmor bonus) to compute thorn bonus
        let total_defence = self.defence.value
            + skills.get_count(Heirloom::Defence) * 10
            + thorn_armor_stacks * 10
            + chaos_defence_bonus
            + if skills.has(Heirloom::ReinforcedArmor) {
                2 * (min(0, computed_speed) / 5).abs()
            } else {
                0
            };
        let thorn_armor_bonus = if thorn_armor_stacks > 0 {
            (total_defence / 10) * 20 * thorn_armor_stacks // 20% thorns per 10 defence per stack
        } else {
            0
        };
        entity.insert(Thorns(base_thorns + thorn_armor_bonus));
        // Calculate raw dodge value from stats and heirlooms
        let raw_dodge =
            self.dodge.value + skills.get_count(Heirloom::DodgeChance) * 7 + chaos_dodge_bonus;
        // Asymptotic formula: approaches 100 but never reaches it
        // Formula: 100 * raw / (raw + 100)
        // At raw 100 this gives 50%, at raw 200 gives 66.6%
        let effective_dodge = if raw_dodge <= 0 {
            0
        } else {
            (100.0 * raw_dodge as f32 / (raw_dodge as f32 + 100.0)) as i32
        };
        entity.insert(Dodge(effective_dodge));
        entity.insert(Speed(
            computed_speed + skills.get_count(Heirloom::Speed) * 15,
        ));
        // Lifesteal: base + RegenLifesteal (+5% per stack) + LifestealCoins (+5% per stack)
        entity.insert(Lifesteal(
            self.lifesteal.value
                + regen_lifesteal_stacks * 5
                + skills.get_count(Heirloom::LifestealCoins) * 5,
        ));
        // Defence already calculated above for ThornArmor
        entity.insert(Defence(total_defence));
        entity.insert(XpRateBonus(self.xp_rate.value));
        entity.insert(LootRateBonus(
            self.loot_rate.value + skills.get_count(Heirloom::LoadedDice) * 7,
        ));
        entity.insert(ManaRegen(
            self.mana_regen.value + skills.get_count(Heirloom::MPRegen) * 5,
        ));
        entity.insert(ProjectileSize(
            self.size.value + skills.get_count(Heirloom::Gigantify) * 10,
        ));
    }
    pub fn get_random_existing_bonus_attribute_string(
        &self,
        bonus_stat_lines: &Vec<BonusStatLine>,
        filter: &Vec<&str>,
    ) -> Option<String> {
        debug!("Getting random existing attribute from: {:?}", self);
        let existing_attributes = vec![
            ("health", self.health.value),
            ("attack", self.attack.value),
            ("crit_chance", self.crit_chance.value),
            ("crit_damage", self.crit_damage.value),
            ("bonus_damage", self.bonus_damage.value),
            ("health_regen", self.health_regen.value),
            ("healing", self.healing.value),
            ("thorns", self.thorns.value),
            ("dodge", self.dodge.value),
            ("speed", self.speed.value),
            ("lifesteal", self.lifesteal.value),
            ("defence", self.defence.value),
            ("xp_rate", self.xp_rate.value),
            ("attack_speed", self.attack_speed.value),
            ("loot_rate", self.loot_rate.value),
            ("mana", self.mana.value),
            ("projectile_size", self.size.value),
            ("mana_regen", self.mana_regen.value),
            ("durability", self.durability.value),
            ("max_durability", self.max_durability.value),
            ("size", self.size.value),
        ]
        .iter()
        .filter(|(name, val)| {
            (*val > 0
                || bonus_stat_lines
                    .iter()
                    .any(|line| &line.attribute_name == name))
                && !filter.contains(name)
        })
        .map(|(name, _)| name.to_string())
        .collect::<Vec<String>>();
        if existing_attributes.is_empty() {
            debug!("No existing attributes found");
            None
        } else {
            let mut rng = rand::thread_rng();
            let index = rng.gen_range(0..existing_attributes.len());
            debug!(
                "Randomly selected attribute: {}",
                existing_attributes[index]
            );
            Some(existing_attributes[index].clone())
        }
    }
    pub fn change_attribute(&mut self, modifier: AttributeModifier) -> &Self {
        match modifier.modifier.as_str() {
            "health" => self.health.value += modifier.delta,
            "shield" => self.shield.value += modifier.delta,
            "attack" => self.attack.value += modifier.delta,
            "crit_chance" => self.crit_chance.value += modifier.delta,
            "crit_damage" => self.crit_damage.value += modifier.delta,
            "bonus_damage" => self.bonus_damage.value += modifier.delta,
            "health_regen" => self.health_regen.value += modifier.delta,
            "healing" => self.healing.value += modifier.delta,
            "thorns" => self.thorns.value += modifier.delta,
            "dodge" => self.dodge.value += modifier.delta,
            "speed" => self.speed.value += modifier.delta,
            "lifesteal" => self.lifesteal.value += modifier.delta,
            "defence" => self.defence.value += modifier.delta,
            "attack_speed" => self.attack_speed.value += modifier.delta,
            "projectile_size" => self.size.value += modifier.delta,
            "loot_rate" => self.loot_rate.value += modifier.delta,
            "mana" => self.mana.value += modifier.delta,
            "size" => self.size.value += modifier.delta,
            "xp_rate" => self.xp_rate.value += modifier.delta,
            "mana_regen" => self.mana_regen.value += modifier.delta,
            "durability" => self.durability.value += modifier.delta,
            "max_durability" => self.max_durability.value += modifier.delta,
            "attack_cooldown" => self.attack_cooldown += modifier.delta as f32,
            "invincibility_cooldown" => self.invincibility_cooldown += modifier.delta as f32,
            _ => warn!("Got an unexpected attribute: {:?}", modifier.modifier),
        }
        self
    }

    /// Sum up a vector of bonus stat lines into ItemAttributes
    /// This allows duplicate stats (e.g., +5 crit, +9 crit) to be combined
    pub fn from_stat_lines(stat_lines: &[crate::item::BonusStatLine]) -> Self {
        let mut attrs = ItemAttributes::default();
        for stat_line in stat_lines {
            let attr_value = AttributeValue::new(
                stat_line.value,
                stat_line.quality,
                stat_line.range_percentage,
            );
            match stat_line.attribute_name.as_str() {
                "health" => attrs.health = attrs.health + attr_value,
                "shield" => attrs.shield = attrs.shield + attr_value,
                "attack" => attrs.attack = attrs.attack + attr_value,
                "crit_chance" => attrs.crit_chance = attrs.crit_chance + attr_value,
                "crit_damage" => attrs.crit_damage = attrs.crit_damage + attr_value,
                "bonus_damage" => attrs.bonus_damage = attrs.bonus_damage + attr_value,
                "health_regen" => attrs.health_regen = attrs.health_regen + attr_value,
                "healing" => attrs.healing = attrs.healing + attr_value,
                "thorns" => attrs.thorns = attrs.thorns + attr_value,
                "dodge" => attrs.dodge = attrs.dodge + attr_value,
                "speed" => attrs.speed = attrs.speed + attr_value,
                "lifesteal" => attrs.lifesteal = attrs.lifesteal + attr_value,
                "defence" => attrs.defence = attrs.defence + attr_value,
                "attack_speed" => attrs.attack_speed = attrs.attack_speed + attr_value,
                "loot_rate" => attrs.loot_rate = attrs.loot_rate + attr_value,
                "mana" => attrs.mana = attrs.mana + attr_value,
                "size" | "projectile_size" => attrs.size = attrs.size + attr_value,
                "xp_rate" => attrs.xp_rate = attrs.xp_rate + attr_value,
                "mana_regen" => attrs.mana_regen = attrs.mana_regen + attr_value,
                "durability" => attrs.durability = attrs.durability + attr_value,
                "max_durability" => attrs.max_durability = attrs.max_durability + attr_value,
                _ => warn!(
                    "Unknown attribute name in stat line: {}",
                    stat_line.attribute_name
                ),
            }
        }
        attrs
    }
    pub fn combine(&self, other: &ItemAttributes) -> ItemAttributes {
        ItemAttributes {
            health: self.health + other.health,
            shield: self.shield + other.shield,
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
            attack_speed: self.attack_speed + other.attack_speed,
            loot_rate: self.loot_rate + other.loot_rate,
            mana: self.mana + other.mana,
            mana_regen: self.mana_regen + other.mana_regen,
            size: self.size + other.size,
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
                item_type: &EquipmentType,
                stat_lines_out: Option<&mut Vec<crate::item::BonusStatLine>>,
            ) -> ItemAttributes {
                // take fields of Range<i32> into one i32
                let mut rng = rand::thread_rng();
                let num_bonus_attributes = rarity.get_num_bonus_attributes(item_type);
                let num_attributes = rng.gen_range(num_bonus_attributes);
                let mut stat_lines = Vec::new();
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
                for _ in 0..num_attributes {
                    let picked_attribute_index = rng.gen_range(0..num_valid_attributes);
                    let picked_attribute = valid_attributes[picked_attribute_index];
                    $(
                        {
                            if stringify!($field_name) == picked_attribute {
                                let min = f32::round(*self.$field_name.clone().unwrap().start() as f32 * rarity.get_rarity_attributes_bonus()) as i32;
                                let max = f32::round(*self.$field_name.clone().unwrap().end()  as f32 * rarity.get_rarity_attributes_bonus()) as i32;
                                let range = (min)..=(max);
                                let total_range = range.end() - range.start();
                                let value = rng.gen_range(range.clone());
                                let percent_of_total_range = (value - range.start()) as f32 / total_range as f32;
                                let stat_line = crate::item::BonusStatLine {
                                    attribute_name: stringify!($field_name).to_string(),
                                        value,
                                    quality: AttributeQuality::get_quality(range, value),
                                    range_percentage: percent_of_total_range,
                                };
                                stat_lines.push(stat_line);
                            }
                        }
                    )*
                }

                // Store stat lines in output parameter
                if let Some(out) = stat_lines_out {
                    *out = stat_lines.clone();
                }

                ItemAttributes::from_stat_lines(&stat_lines)
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
     mana: Option<RangeInclusive<i32>>,
     mana_regen: Option<RangeInclusive<i32>>,
     size: Option<RangeInclusive<i32>>,
     attack_speed: Option<RangeInclusive<i32>>,
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
     mana: Option<RangeInclusive<i32>>,
     mana_regen: Option<RangeInclusive<i32>>,
     size: Option<RangeInclusive<i32>>,
     attack_speed: Option<RangeInclusive<i32>>,
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
        let acc_offset = if eqp_type.is_accessory() { -1 } else { 0 };
        let acc_max = 2;
        let count = match self {
            ItemRarity::Common => (2 + acc_offset)..=(2 + acc_offset),
            ItemRarity::Uncommon => (3 + acc_offset)..=(3 + acc_offset),
            ItemRarity::Rare => (4 + acc_offset)..=(4 + acc_offset),
            ItemRarity::Legendary => (5 + acc_offset)..=(6 + acc_offset),
        };
        if eqp_type.is_accessory() {
            // max of 2
            RangeInclusive::new(
                i32::min(*count.start(), acc_max),
                i32::min(*count.end(), acc_max),
            )
        } else {
            count
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
            ItemRarity::Common => ScrapsInto(vec![Scrap::new(WorldObject::UpgradeTome, 1.)]),
            ItemRarity::Uncommon => ScrapsInto(vec![
                Scrap::new(WorldObject::OrbOfTransformation, 0.2),
                Scrap::new(WorldObject::UpgradeTome, 1.0),
                Scrap::new(WorldObject::UpgradeTome, 0.5),
            ]),
            ItemRarity::Rare => ScrapsInto(vec![
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::OrbOfTransformation, 0.2),
                Scrap::new(WorldObject::UpgradeTome, 1.),
                Scrap::new(WorldObject::UpgradeTome, 1.0),
                Scrap::new(WorldObject::UpgradeTome, 0.2),
            ]),
            ItemRarity::Legendary => ScrapsInto(vec![
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::OrbOfTransformation, 1.),
                Scrap::new(WorldObject::OrbOfTransformation, 0.2),
                Scrap::new(WorldObject::OrbOfTransformation, 0.2),
                Scrap::new(WorldObject::OrbOfTransformation, 0.2),
                Scrap::new(WorldObject::UpgradeTome, 1.),
                Scrap::new(WorldObject::UpgradeTome, 1.),
                Scrap::new(WorldObject::UpgradeTome, 1.),
                Scrap::new(WorldObject::UpgradeTome, 1.),
                Scrap::new(WorldObject::UpgradeTome, 1.),
                Scrap::new(WorldObject::UpgradeTome, 0.5),
                Scrap::new(WorldObject::UpgradeTome, 0.5),
                Scrap::new(WorldObject::UpgradeTome, 0.5),
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
    pub shield: MaxShield,
    pub mana: MaxMana,
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
    pub size: ProjectileSize,
}

//TODO: Add max health vs curr health
#[derive(
    Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy, Serialize, Deserialize,
)]
#[reflect(Component, Schematic)]
pub struct CurrentHealth(pub i32);
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug)]
#[reflect(Component, Schematic)]
pub struct CurrentShield(pub i32);

#[derive(Component)]
pub struct ShieldRegen {
    pub regen_timer: Timer,
    pub delay_timer: Timer,
}
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct MaxMana(pub i32);
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct CurrentMana(pub i32);

#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct MaxShield(pub i32);
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
pub struct ProjectileSize(pub i32);
impl ProjectileSize {
    pub fn get_multiplier(&self) -> f32 {
        1. + self.0 as f32 / 100.
    }
}
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
                    tick_hunger.run_if(is_not_paused),
                    handle_modify_health_event.before(clamp_health),
                    handle_modify_mana_event.before(clamp_mana),
                    add_current_health_with_max_health,
                    handle_health_regen.run_if(is_not_paused),
                    handle_mana_regen.run_if(is_not_paused),
                    update_attributes_with_held_item_change,
                    update_attributes_and_sprite_with_equipment_change,
                    update_sprite_with_equipment_removed,
                    handle_item_abilitiy_on_attack.after(player_move_inputs),
                    handle_new_items_raw_attributes.before(CustomFlush),
                    handle_player_item_attribute_change_events.after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    add_current_shield_with_max_shield,
                    handle_cape_att_increase_on_level_up.before(handle_level_up),
                    regen_shield,
                    update_player_health_percent,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

#[derive(Component)]
pub struct GameOverSent;

pub fn clamp_health(
    mut commands: Commands,
    mut health: Query<
        (
            Entity,
            &mut CurrentHealth,
            &MaxHealth,
            &mut CurrentShield,
            &MaxShield,
            Option<&GameOverSent>,
            &mut PlayerSkills,
        ),
        With<Player>,
    >,
    mut game_over_event: EventWriter<GameOverEvent>,
    mobs: Query<(Entity, &TextureAtlasSprite), With<crate::enemy::Mob>>,
) {
    for (entity, mut h, max_h, mut s, max_s, game_over_sent, mut skills) in health.iter_mut() {
        if h.0 <= 0 {
            // Check for Death Defiance heirloom
            let defiance_count = skills.get_count(Heirloom::DeathDefiance);
            if defiance_count > 0 {
                // Consume one stack of Death Defiance
                if let Some(idx) = skills
                    .heirlooms
                    .iter()
                    .position(|h| h.heirloom == Heirloom::DeathDefiance)
                {
                    skills.heirlooms.remove(idx);

                    // Restore to 50% max health
                    h.0 = max_h.0 / 2;

                    // Freeze all mobs for 3 seconds with blue tint
                    for (mob_entity, sprite) in mobs.iter() {
                        commands.entity(mob_entity).insert(
                            crate::player::combat_heirlooms::DeathDefianceFrozen {
                                timer: Timer::from_seconds(3.0, TimerMode::Once),
                                original_color: sprite.color,
                            },
                        );
                        // Apply blue tint
                        commands.entity(mob_entity).insert(TextureAtlasSprite {
                            color: Color::rgba(0.5, 0.7, 1.0, 1.0),
                            ..sprite.clone()
                        });
                    }

                    continue; // Don't trigger game over
                }
            }

            h.0 = 0;
            // Only send game over event once
            if game_over_sent.is_none() {
                game_over_event.send_default();
                commands.entity(entity).insert(GameOverSent);
            }
        } else if h.0 > max_h.0 {
            h.0 = max_h.0;
        }
        if s.0 < 0 {
            s.0 = 0;
        } else if s.0 > max_s.0 {
            s.0 = max_s.0;
        }
    }
}
fn clamp_mana(mut health: Query<(&mut CurrentMana, &MaxMana), With<Player>>) {
    for (mut current_mana, max_mana) in health.iter_mut() {
        if current_mana.0 < 0 {
            current_mana.0 = 0;
        } else if current_mana.0 > max_mana.0 {
            current_mana.0 = max_mana.0;
        }
    }
}
/// Calculate inventory buffs from all items in inventory (not hotbar)
/// Returns a combined ItemAttributes with all inventory buffs
fn calculate_inventory_buffs(
    inv: &Inventory,
    proto: &crate::proto::proto_param::ProtoParam,
) -> ItemAttributes {
    use crate::inventory::InventoryItemStack;

    let mut inventory_buffs = ItemAttributes::default();

    // Iterate through inventory items (not hotbar, not equipment, not accessories)
    // Hotbar is slots 0-5, inventory is slots 6+
    for (slot_idx, item_opt) in inv.items.items.iter().enumerate() {
        // Skip hotbar slots (0-5) - only check inventory slots (6+)
        if slot_idx < 6 {
            continue;
        }

        if let Some(InventoryItemStack { item_stack, .. }) = item_opt {
            // Only process equipment items (weapons, armor, accessories)
            if let Some(equip_type) = item_stack.obj_type.get_equip_type(proto) {
                if !equip_type.is_weapon() && !equip_type.is_armor() && !equip_type.is_accessory() {
                    continue;
                }

                // Get the inventory buff line index
                if let Some(line_index) = item_stack.metadata.inventory_buff_line_index {
                    // Extract the attribute value from the specified line
                    if let Some(buff_att) =
                        extract_inventory_buff_attribute(item_stack, line_index, proto)
                    {
                        inventory_buffs = inventory_buffs.combine(&buff_att);
                    }
                }
            }
        }
    }

    inventory_buffs
}

/// Extract the attribute value from a specific bonus stat line index
/// The line_index is an index into stack.metadata.bonus_stat_lines
fn extract_inventory_buff_attribute(
    stack: &ItemStack,
    line_index: usize,
    _proto: &crate::proto::proto_param::ProtoParam,
) -> Option<ItemAttributes> {
    // Get the stat line directly from bonus_stat_lines
    if let Some(stat_line) = stack.metadata.bonus_stat_lines.get(line_index) {
        // Convert the single stat line to ItemAttributes
        let mut buff_att = ItemAttributes::default();
        let attr_value = AttributeValue::new(
            stat_line.value,
            stat_line.quality,
            stat_line.range_percentage,
        );

        // Match attribute name to ItemAttributes field
        match stat_line.attribute_name.as_str() {
            "health" => buff_att.health = attr_value,
            "shield" => buff_att.shield = attr_value,
            "attack" => buff_att.attack = attr_value,
            "crit_chance" => buff_att.crit_chance = attr_value,
            "crit_damage" => buff_att.crit_damage = attr_value,
            "bonus_damage" => buff_att.bonus_damage = attr_value,
            "health_regen" => buff_att.health_regen = attr_value,
            "healing" => buff_att.healing = attr_value,
            "thorns" => buff_att.thorns = attr_value,
            "dodge" => buff_att.dodge = attr_value,
            "speed" => buff_att.speed = attr_value,
            "lifesteal" => buff_att.lifesteal = attr_value,
            "defence" => buff_att.defence = attr_value,
            "attack_speed" => buff_att.attack_speed = attr_value,
            "loot_rate" => buff_att.loot_rate = attr_value,
            "mana" => buff_att.mana = attr_value,
            "size" | "projectile_size" => buff_att.size = attr_value,
            "xp_rate" => buff_att.xp_rate = attr_value,
            "mana_regen" => buff_att.mana_regen = attr_value,
            "durability" => buff_att.durability = attr_value,
            "max_durability" => buff_att.max_durability = attr_value,
            _ => return None, // Unknown attribute
        }

        Some(buff_att)
    } else {
        None
    }
}

fn handle_player_item_attribute_change_events(
    mut commands: Commands,
    player: Query<(Entity, &Inventory), With<Player>>,
    eqp_attributes: Query<&ItemAttributes, With<Equipment>>,
    mut att_events: EventReader<AttributeChangeEvent>,
    mut stats_event: EventWriter<ShowInvPlayerStatsEvent>,
    player_atts: Query<
        (
            &ItemAttributes,
            &PlayerSkills,
            &MaxHealth,
            &MaxMana,
            &MaxShield,
        ),
        With<Player>,
    >,
    stat_button: Query<(&UIElement, &StatsButtonState)>,
    ui_state: Res<State<UIState>>,
    game: Res<Game>,
    dodge_crit_state: Query<&crate::player::combat_heirlooms::DodgeCritState, With<Player>>,
    hallucination_stats: Query<&crate::player::combat_heirlooms::HallucinationStats, With<Player>>,
    max_hp_hunt_tracker: Query<&crate::player::combat_heirlooms::MaxHPHuntTracker, With<Player>>,
    coins: Res<crate::player::currency::CoinCurrency>,
    proto: crate::proto::proto_param::ProtoParam,
) {
    for _event in att_events.iter() {
        let (att, skills, old_health, old_mana, old_shield) = player_atts.single();
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

        // Combine hallucination stats from LethalBlow executes
        if let Ok(hall_stats) = hallucination_stats.get_single() {
            new_att = new_att.combine(hall_stats.as_item_attributes());
        }

        // Calculate inventory buffs from items in inventory (not hotbar)
        let inventory_buffs = calculate_inventory_buffs(&inv, &proto);
        new_att = new_att.combine(&inventory_buffs);

        if new_att.attack_cooldown == 0. {
            new_att.attack_cooldown = 0.4;
        }
        // Check if DodgeCrit buff is active
        let dodge_crit_buff_active = dodge_crit_state
            .get_single()
            .map(|s| s.buff_active)
            .unwrap_or(false);

        // Get MaxHPHunt bonus
        let max_hp_hunt_bonus = max_hp_hunt_tracker
            .get_single()
            .map(|tracker| tracker.total_hp_gained)
            .unwrap_or(0);

        new_att.add_attribute_components(
            &mut commands.entity(player),
            old_health.0,
            old_mana.0,
            old_shield.0,
            skills,
            dodge_crit_buff_active,
            coins.coins,
            max_hp_hunt_bonus,
        );
        if let Some(main_hand) = game.player_state.main_hand_slot.clone() {
            if !main_hand.get_obj().is_weapon() {
                new_att.attack = AttributeValue::new(1, AttributeQuality::Low, 0.);
            }
        }
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
        // Check if entity still exists before inserting components
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(CurrentHealth(max_health.0));
        }
    }
}
/// Adds a current shield component to all entities with a max shield component
pub fn add_current_shield_with_max_shield(
    mut commands: Commands,
    mut shield: Query<(Entity, &MaxShield), Or<(Changed<MaxShield>, Without<CurrentShield>)>>,
) {
    for (entity, max_shield) in shield.iter_mut() {
        info!("Adding CurrentShield component with value {}", max_shield.0);
        // Check if entity still exists before inserting components
        if let Some(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.insert(CurrentShield(max_shield.0));
        }
    }
}

pub fn regen_shield(
    time: Res<Time>,
    mut shield: Query<(&MaxShield, &mut CurrentShield, &mut ShieldRegen)>,
) {
    for (max_shield, mut current_shield, mut timer) in shield.iter_mut() {
        if current_shield.0 < max_shield.0 {
            timer.delay_timer.tick(time.delta());
            if timer.delay_timer.finished() {
                timer.regen_timer.tick(time.delta());
                if timer.regen_timer.finished() {
                    current_shield.0 += 1;
                    timer.regen_timer.reset();
                }
            }
        } else {
            timer.delay_timer.reset();
            timer.regen_timer.reset();
        }
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
            Option<&crate::player::score::StartingWeapon>,
        ),
        Or<(Added<RawItemBaseAttributes>, Added<RawItemBonusAttributes>)>,
    >,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    proto: ProtoParam,
) {
    for (e, stack, raw_bonus_att_option, raw_base_att, eqp_type, item_level, starting_weapon) in
        new_items.iter()
    {
        // Use override rarity for starting weapons, random for others
        // Note: loot_bonus not available here, using 0 as fallback
        let rarity = if let Some(starting_weapon) = starting_weapon {
            starting_weapon.rarity.clone()
        } else {
            get_rarity_rng(rand::thread_rng(), 0)
        };
        add_item_glows(&mut commands, &graphics, e, rarity.clone());

        let new_stack = build_item_stack_with_parsed_attributes(
            stack,
            raw_base_att,
            raw_bonus_att_option,
            rarity,
            eqp_type,
            item_level.map(|l| l.0),
            &mut commands,
            false,
            &proto,
        );

        if new_stack.rarity.clone() == ItemRarity::Rare {
            commands
                .spawn(AsepriteBundle {
                    aseprite: asset_server.load(RarityGlows::PATH),
                    animation: AsepriteAnimation::from(RarityGlows::tags::RARE),
                    transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                    ..Default::default()
                })
                .insert(VisibilityBundle::default())
                .insert(DoneAnimation)
                .set_parent(e);
        } else if new_stack.rarity.clone() == ItemRarity::Legendary {
            commands
                .spawn(AsepriteBundle {
                    aseprite: asset_server.load(RarityGlows::PATH),
                    animation: AsepriteAnimation::from(RarityGlows::tags::RARER),
                    transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                    ..Default::default()
                })
                .insert(VisibilityBundle::default())
                .insert(DoneAnimation)
                .set_parent(e);
            let mut rng = rand::thread_rng();
            let seed = rng.gen_range(0..100000);
            let speed = 10.;
            let max_mag = 90.;
            let noise = 0.5;
            let dir = Vec2::new(1., 1.);
            for e in game_camera.iter_mut() {
                commands.entity(e).insert(ShakeEffect {
                    timer: Timer::from_seconds(2., TimerMode::Once),
                    speed,
                    seed,
                    max_mag,
                    noise,
                    dir,
                });
            }
        }
        commands.entity(e).insert(new_stack);
    }
}

pub fn add_item_glows(
    commands: &mut Commands,
    graphics: &Graphics,
    new_item_e: Entity,
    rarity: ItemRarity,
) -> Option<Entity> {
    // Check if parent entity exists before trying to set parent
    if commands.get_entity(new_item_e).is_none() {
        return None;
    }
    rarity.get_item_glow().map(|glow| {
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_item_glow(glow.clone()),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(20., 20.)),
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

pub fn handle_cape_att_increase_on_level_up(
    mut player: Query<(&mut Inventory, &PlayerLevel, &PlayerClass), Changed<PlayerLevel>>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    proto: ProtoParam,
) {
    for (mut inv, level, class) in player.iter_mut() {
        if level.level == level.next_level {
            let mut cape_stack = proto.get_item_data(class.class.get_cape()).unwrap().clone();
            let level = level.level as i32 - 1;
            cape_stack.attributes = class.class.compute_cape_stats(level);
            if level >= 11 {
                cape_stack.rarity = ItemRarity::Legendary;
            } else if level >= 7 {
                cape_stack.rarity = ItemRarity::Rare;
            } else if level >= 3 {
                cape_stack.rarity = ItemRarity::Uncommon;
            } else {
                cape_stack.rarity = ItemRarity::Common;
            }
            cape_stack.metadata.level = Some((level + 1) as u8);
            inv.equipment_items.with_item_in_slot(3, cape_stack);
            att_event.send(AttributeChangeEvent);
        }
    }
}

/// Updates the PlayerHealthPercent resource based on player's current/max health
/// This runs every frame to keep the resource in sync for damage calculations
pub fn update_player_health_percent(
    player_health: Query<(&CurrentHealth, &MaxHealth), With<crate::player::Player>>,
    mut health_percent: ResMut<crate::PlayerHealthPercent>,
) {
    if let Ok((current, max)) = player_health.get_single() {
        if max.0 > 0 {
            health_percent.percent = current.0 as f32 / max.0 as f32;
        } else {
            health_percent.percent = 1.0;
        }
    }
}
