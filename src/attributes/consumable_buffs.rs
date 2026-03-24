use crate::inventory::ItemStack;
use crate::player::Player;
use bevy::prelude::*;

use super::modifiers::ModifyHealthEvent;
use super::{AttributeChangeEvent, AttributeQuality, AttributeValue, ItemAttributes};

/// One active consumable (food, potion, or skill-granted) buff row.
#[derive(Clone, Debug)]
pub struct ConsumableBuffEntry {
    pub display_timer: Timer,
    /// When set, show this buff in the HUD with the consumed item's icon/tooltip.
    pub item_stack: Option<ItemStack>,
    pub effect: ConsumableBuffEffect,
}

#[derive(Clone, Debug)]
pub enum ConsumableBuffEffect {
    /// Added on top of [`super::BonusAttackSpeed::get_multiplier`] (legacy potion / blessing behavior).
    AttackSpeedAdd(f32),
    /// Multiplied with other movement buffs (product).
    MovementSpeedMult(f32),
    FlatThorns(i32),
    FlatSpeed(i32),
    PeriodicHeal {
        heal_per_tick: i32,
        interval: Timer,
    },
}

#[derive(Component, Clone, Debug, Default)]
pub struct ActiveConsumableBuffs {
    pub entries: Vec<ConsumableBuffEntry>,
}

impl ActiveConsumableBuffs {
    pub fn movement_multiplier_product(&self) -> f32 {
        let mut p = 1.0_f32;
        for e in &self.entries {
            if let ConsumableBuffEffect::MovementSpeedMult(m) = e.effect {
                p *= m;
            }
        }
        p
    }
}
//TODO: why is attack speed separate here
/// Flat [`ItemAttributes`] from consumable buffs plus attack-speed addends (not stored on [`super::BonusAttackSpeed`]).
#[derive(Clone, Debug, Default)]
pub struct ConsumableBuffAttributeSummary {
    pub item_layer: ItemAttributes,
    pub attack_speed_add: f32,
}

pub fn summarize_active_consumable_buffs(
    buffs: &ActiveConsumableBuffs,
) -> ConsumableBuffAttributeSummary {
    let mut summary = ConsumableBuffAttributeSummary::default();
    let mut thorns = 0i32;
    let mut speed = 0i32;

    for entry in &buffs.entries {
        match &entry.effect {
            ConsumableBuffEffect::AttackSpeedAdd(v) => summary.attack_speed_add += v,
            ConsumableBuffEffect::FlatThorns(v) => thorns += v,
            ConsumableBuffEffect::FlatSpeed(v) => speed += v,
            ConsumableBuffEffect::MovementSpeedMult(_)
            | ConsumableBuffEffect::PeriodicHeal { .. } => {}
        }
    }

    if thorns != 0 {
        summary.item_layer.thorns = AttributeValue::new(thorns, AttributeQuality::Low, 0.);
    }
    if speed != 0 {
        summary.item_layer.speed = AttributeValue::new(speed, AttributeQuality::Low, 0.);
    }

    summary
}

/// Merges equipment-derived intrinsic stats with the consumable flat stat layer.
pub fn merge_intrinsic_stats_with_consumable_buff_layer(
    intrinsic: ItemAttributes,
    summary: &ConsumableBuffAttributeSummary,
) -> ItemAttributes {
    intrinsic.combine(&summary.item_layer)
}

pub fn tick_active_consumable_buffs(
    time: Res<Time>,
    mut players: Query<&mut ActiveConsumableBuffs, With<Player>>,
    mut modify_health: EventWriter<ModifyHealthEvent>,
    mut attr_events: EventWriter<AttributeChangeEvent>,
) {
    for mut buffs in players.iter_mut() {
        let mut changed_stats = false;
        buffs.entries.retain_mut(|entry| {
            entry.display_timer.tick(time.delta());
            if entry.display_timer.finished() {
                if matches!(
                    entry.effect,
                    ConsumableBuffEffect::AttackSpeedAdd(_)
                        | ConsumableBuffEffect::FlatThorns(_)
                        | ConsumableBuffEffect::FlatSpeed(_)
                ) {
                    changed_stats = true;
                }
                return false;
            }

            if let ConsumableBuffEffect::PeriodicHeal {
                heal_per_tick,
                ref mut interval,
            } = entry.effect
            {
                interval.tick(time.delta());
                if interval.just_finished() {
                    modify_health.send(ModifyHealthEvent(heal_per_tick));
                }
            }

            true
        });

        if changed_stats {
            attr_events.send_default();
        }
    }
}
