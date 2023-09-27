use bevy::prelude::*;

use crate::attributes::{AttributeChangeEvent, ItemAttributes};

#[derive(Component, Clone, Debug)]
pub struct PlayerStats {
    pub str: i32,
    pub dex: i32,
    pub agi: i32,
    pub vit: i32,
}
#[derive(Component, Clone, Debug)]
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
        att.attack += self.str;
        att.defence += self.str;
        att.crit_damage += 2 * self.dex;
        att.crit_chance += 2 * self.dex;
        att.speed += 2 * self.agi;
        att.dodge += 2 * self.agi;
        att.health += 3 * self.vit;
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
