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
}

pub fn send_attribute_event_on_stats_update(
    stats: Query<&PlayerStats, Changed<PlayerStats>>,
    mut att_event: EventWriter<AttributeChangeEvent>,
) {
    if stats.get_single().is_ok() {
        att_event.send(AttributeChangeEvent);
    }
}
