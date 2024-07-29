use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::attributes::{AttributeChangeEvent, ItemAttributes};

#[derive(Clone, Debug)]
pub enum StatType {
    STR,
    DEX,
    AGI,
    VIT,
}
impl StatType {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => StatType::STR,
            1 => StatType::DEX,
            2 => StatType::AGI,
            3 => StatType::VIT,
            _ => StatType::STR,
        }
    }
}
#[derive(Component, Clone, Default, Debug, Serialize, Deserialize)]
pub struct PlayerStats {
    pub str: i32,
    pub dex: i32,
    pub agi: i32,
    pub vit: i32,
}
#[derive(Component, Clone, Default, Serialize, Deserialize, Debug)]
pub struct SkillPoints {
    pub count: u8,
}

impl PlayerStats {
    pub fn new() -> Self {
        PlayerStats {
            str: 0,
            dex: 0,
            agi: 0,
            vit: 0,
        }
    }
    pub fn apply_stats_to_player_attributes(&self, input_att: ItemAttributes) -> ItemAttributes {
        let mut att = input_att;
        att.attack += (self.str) + (self.dex + self.agi + self.vit) / 2;
        // att.defence += self.str;
        att.crit_damage += 5 * self.dex + 2 * self.str;
        att.crit_chance += 2 * self.dex;
        att.speed += 5 * self.agi;
        att.dodge += 2 * self.agi;
        att.health += 5 * self.vit;
        att.health_regen += self.vit;
        att
    }
    pub fn get_stats_from_ui_index(&self, index: i32) -> i32 {
        match index {
            0 => self.str,
            1 => self.dex,
            2 => self.agi,
            3 => self.vit,
            _ => 0,
        }
    }
}

pub fn send_attribute_event_on_stats_update(
    stats: Query<&PlayerStats, Changed<PlayerStats>>,
    mut att_event: EventWriter<AttributeChangeEvent>,
) {
    if stats.get_single().is_ok() {
        att_event.send(AttributeChangeEvent);
    }
}
