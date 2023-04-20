use bevy::{prelude::*, time::FixedTimestep};
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

use crate::{
    inventory::{Inventory, InventoryItemStack},
    item::ItemDisplayMetaData,
    ui::InventorySlotState,
    GameState, Player, TIME_STEP,
};

pub struct AttributesPlugin;

#[derive(Bundle, Inspectable)]
pub struct BlockAttributeBundle {
    pub health: Health,
}
#[derive(Component, PartialEq, Clone, Debug, Inspectable)]
pub struct ItemAttributes {
    pub health: i8,
    pub attack: u8,
    pub durability: u64,
    pub max_durability: u64,
    pub attack_cooldown: f32,
    pub invincibility_cooldown: f32,
}
impl Default for ItemAttributes {
    fn default() -> Self {
        Self {
            health: 0,
            attack: 0,
            durability: 0,
            max_durability: 0,
            attack_cooldown: 0.,
            invincibility_cooldown: 0.,
        }
    }
}
impl ItemAttributes {
    pub fn get_tooltips(&self) -> Vec<String> {
        let mut tooltips: Vec<String> = vec![];
        if self.health > 0 {
            tooltips.push(format!("+{} HP", self.health));
        }
        if self.attack > 0 {
            tooltips.push(format!("+{} Att", self.attack));
        }
        if self.attack_cooldown > 0. {
            tooltips.push(format!("{} Hits/s", 1. / self.attack_cooldown));
        }

        tooltips
    }
    pub fn get_durability_tooltip(&self) -> String {
        format!("{}/{}", self.durability, self.max_durability)
    }
}

#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct Health(pub i8);
#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct Attack(pub u8);
#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct Durability(pub u8);

#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct AttackCooldown(pub f32);
#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct InvincibilityCooldown(pub f32);

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<BlockAttributeBundle>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::clamp_health)
                    .with_system(Self::handle_attribute_change),
            );
    }
}

impl AttributesPlugin {
    fn clamp_health(mut health: Query<&mut Health, With<Player>>) {
        for mut h in health.iter_mut() {
            if h.0 < 0 {
                h.0 = 0;
            } else if h.0 > 100 {
                h.0 = 100;
            }
        }
    }
    fn handle_attribute_change(
        mut inv: Query<&mut Inventory, Changed<Inventory>>,
        mut inv_slot_state: Query<&mut InventorySlotState>,
    ) {
        if let Ok(mut inv) = inv.get_single_mut() {
            for inv_item_option in inv.clone().items.iter() {
                if let Some(inv_item) = inv_item_option {
                    let mut item = inv_item.item_stack.clone();
                    let tooltips = item.attributes.get_tooltips();
                    let durability_tooltip = item.attributes.get_durability_tooltip();

                    let new_meta = ItemDisplayMetaData {
                        name: item.metadata.name.clone(),
                        desc: item.metadata.desc.clone(),
                        attributes: tooltips,
                        durability: durability_tooltip,
                    };
                    item.metadata = new_meta;
                    inv.items[inv_item.slot] = Some(InventoryItemStack {
                        item_stack: item,
                        slot: inv_item.slot,
                    });
                    for mut slot_state in inv_slot_state.iter_mut() {
                        if slot_state.slot_index == inv_item.slot {
                            slot_state.dirty = true;
                        }
                    }
                }
            }
        }
    }
}
