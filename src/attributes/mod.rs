use bevy::{ecs::system::EntityCommands, prelude::*};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
pub mod modifiers;
use crate::{
    animations::AnimatedTextureMaterial,
    inventory::Inventory,
    item::Equipment,
    player::Limb,
    proto::proto_param::ProtoParam,
    ui::{DropOnSlotEvent, InventoryState},
    CustomFlush, GameParam, GameState, Player,
};

use modifiers::*;

pub struct AttributesPlugin;

#[derive(Resource, Reflect, Default, Bundle)]
pub struct BlockAttributeBundle {
    pub health: CurrentHealth,
}
#[derive(Component, PartialEq, Clone, Reflect, FromReflect, Schematic, Debug)]
#[reflect(Schematic, Default)]
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
            entity.insert(MaxHealth(self.health));
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
    pub health: MaxHealth,
    pub attack: Attack,
    pub attack_cooldown: AttackCooldown,
}

//TODO: Add max health vs curr health
#[derive(Reflect, FromReflect, Default, Schematic, Component, Clone, Debug, Copy)]
#[reflect(Component, Schematic)]
pub struct CurrentHealth(pub i32);
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

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AttributeChangeEvent>()
            .add_event::<ModifyHealthEvent>()
            .add_systems(
                (
                    clamp_health,
                    handle_modify_health_event.before(clamp_health),
                    add_current_health_with_max_health,
                    update_attributes_with_held_item_change,
                    update_attributes_and_sprite_with_equipment_change,
                    handle_player_item_attribute_change_events.after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

fn clamp_health(mut health: Query<(&mut CurrentHealth, &MaxHealth), With<Player>>) {
    for (mut h, max_h) in health.iter_mut() {
        if h.0 < 0 {
            h.0 = 0;
        } else if h.0 > max_h.0 {
            h.0 = max_h.0;
        }
    }
}
fn handle_player_item_attribute_change_events(
    mut commands: Commands,
    player: Query<(Entity, &Inventory), With<Player>>,
    eqp_attributes: Query<&ItemAttributes, With<Equipment>>,
    mut att_events: EventReader<AttributeChangeEvent>,
    player_atts: Query<&ItemAttributes, With<Player>>,
) {
    for _event in att_events.iter() {
        let mut new_att = player_atts.single().clone();
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
            new_att.health += a.health;
            new_att.attack += a.attack;
            new_att.attack_cooldown += a.attack_cooldown;
            new_att.invincibility_cooldown += a.invincibility_cooldown;
        }
        if new_att.attack_cooldown == 0. {
            new_att.attack_cooldown = 0.4;
        }
        new_att.add_attribute_components(&mut commands.entity(player));
    }
}

/// Adds a current health component to all entities with a max health component
fn add_current_health_with_max_health(
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
    let player_data = &mut game_param.game.player_state;
    let prev_held_item_data = &player_data.main_hand_slot;
    if let Some(new_item) = active_hotbar_item {
        let new_item_obj = new_item.get_obj();
        if let Some(current_item) = prev_held_item_data {
            let curr_attributes = item_stack_query.get(current_item.entity).unwrap();
            let new_attributes = &new_item.item_stack.attributes;
            if new_item_obj != &current_item.obj {
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
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    mut events: EventReader<DropOnSlotEvent>,
) {
    for drop in events.iter() {
        if drop.drop_target_slot_state.r#type.is_equipment()
            || drop.drop_target_slot_state.r#type.is_accessory()
        {
            let slot = drop.drop_target_slot_state.slot_index;
            att_event.send(AttributeChangeEvent);
            if drop.drop_target_slot_state.r#type.is_equipment() {
                for (mat, limb) in player_limbs.iter() {
                    if limb == &Limb::from_slot(slot) || (limb == &Limb::Hands && slot == 2) {
                        let mut mat = materials.get_mut(mat).unwrap();
                        let armor_texture_handle = asset_server.load(format!(
                            "textures/player/{}.png",
                            drop.dropped_item_stack.obj_type.to_string()
                        ));
                        mat.lookup_texture = Some(armor_texture_handle);
                    }
                }
            }
        }
    }
}
