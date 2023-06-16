use bevy::{ecs::system::EntityCommands, prelude::*};
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use crate::{
    inventory::{Inventory, InventoryItemStack},
    item::{Equipment, ItemDisplayMetaData},
    ui::InventorySlotState,
    CoreGameSet, CustomFlush, Player,
};

pub struct AttributesPlugin;

#[derive(Resource, Reflect, Default, Bundle)]
pub struct BlockAttributeBundle {
    pub health: Health,
}
#[derive(Resource, Component, PartialEq, Clone, Debug)]
pub struct ItemAttributes {
    pub health: i32,
    pub attack: i32,
    pub durability: i32,
    pub max_durability: i32,
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
    pub fn add_attribute_components(&self, entity: &mut EntityCommands) {
        if self.health > 0 {
            entity.insert(Health(self.health));
        }
        if self.attack > 0 {
            entity.insert(Attack(self.attack));
        } else {
            entity.remove::<Attack>();
        }
        if self.attack_cooldown > 0. {
            entity.insert(AttackCooldown(self.attack_cooldown));
        } else {
            entity.remove::<AttackCooldown>();
        }
    }
    pub fn change_attribute(&mut self, modifier: AttributeModifier) -> &Self {
        match modifier.modifier.as_str() {
            "health" => self.health += modifier.delta,
            "attack" => self.attack += modifier.delta,
            "durability" => self.durability += modifier.delta,
            "max_durability" => self.max_durability += modifier.delta,
            "attack_cooldown" => self.attack_cooldown += modifier.delta as f32,
            "invincibility_cooldown" => self.invincibility_cooldown += modifier.delta as f32,
            _ => warn!("Got an unexpected attribute: {:?}", modifier.modifier),
        }
        self
    }
}
pub struct AttributeModifier {
    pub modifier: String,
    pub delta: i32,
}

#[derive(Debug, Clone, Default)]
pub struct AttributeChangeEvent;

#[derive(Reflect, FromReflect, Bundle, Clone, Debug, Copy)]
pub struct PlayerAttributeBundle {
    pub health: Health,
    pub attack: Attack,
    pub attack_cooldown: AttackCooldown,
}

//TODO: Add max health vs curr health
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct Health(pub i32);
#[derive(Reflect, FromReflect, Default, Component, Clone, Debug, Copy)]
#[reflect(Component)]

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

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app
            // .register_type::<BlockAttributeBundle>()
            .add_event::<AttributeChangeEvent>()
            .add_systems(
                (
                    Self::clamp_health,
                    Self::handle_item_attribute_change,
                    Self::handle_attribute_change_events.after(CustomFlush),
                )
                    .in_base_set(CoreGameSet::Main),
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
    fn handle_attribute_change_events(
        mut commands: Commands,
        player: Query<Entity, With<Player>>,
        eqp_attributes: Query<&ItemAttributes, With<Equipment>>,
        mut att_events: EventReader<AttributeChangeEvent>,
    ) {
        for _event in att_events.iter() {
            let mut new_att = ItemAttributes::default();
            for a in eqp_attributes.iter() {
                new_att.health += a.health;
                new_att.attack += a.attack;
                new_att.attack_cooldown += a.attack_cooldown;
                new_att.invincibility_cooldown += a.invincibility_cooldown;
            }
            if new_att.attack_cooldown == 0. {
                new_att.attack_cooldown = 0.4;
            }
            let player = player.single();
            new_att.add_attribute_components(&mut commands.entity(player));
        }
    }
    fn handle_item_attribute_change(
        mut inv: Query<&mut Inventory, Changed<Inventory>>,
        mut inv_slot_state: Query<&mut InventorySlotState>,
        mut att_event: EventWriter<AttributeChangeEvent>,
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
                    att_event.send(AttributeChangeEvent);
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
