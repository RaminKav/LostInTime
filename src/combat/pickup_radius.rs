use bevy::prelude::*;

use crate::{
    attributes::PickupRange,
    inventory::{player_can_accept_ground_item_pickup, BreakDropFilter, Inventory, ItemStack},
    item::{object_actions::TouchTriggerObjectAction, ItemDrop},
    pets::state::Pet,
    player::{skills::PlayerSkills, Player},
    proto::proto_param::ProtoParam,
};

#[derive(Component, Debug, Clone)]
pub struct PickupRadius(pub f32);

pub const BASE_PICKUP_RADIUS: f32 = 36.0;

/// System that updates pickup radius based on pickup range (equipment, Magnet heirloom, etc.)
pub fn update_pickup_radius(
    mut pickup_radius_query: Query<&mut PickupRadius, With<Player>>,
    pickup_range_attr: Query<&PickupRange, With<Player>>,
) {
    let Ok(mut pickup_radius) = pickup_radius_query.single_mut() else {
        return;
    };
    let pickup_range_bonus =
        1.0 + (pickup_range_attr.single().cloned().unwrap_or_default().0 as f32 / 100.0);

    pickup_radius.0 = BASE_PICKUP_RADIUS * pickup_range_bonus;
}

/// Component that marks an item drop as being pulled to the player.
/// Tracks how long it's been pulled to increase speed over time.
///
/// Stored as `SparseSet` because item drops churn in and out of pickup range
/// every frame (especially with wide pickup radius heirlooms / magnet pulls).
/// Keeping item drops in their original archetype avoids a full
/// archetype/table move per item per frame.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct BeingPulledToPlayer {
    /// Time elapsed since the item started being pulled
    pub time_pulled: f32,
}

impl Default for BeingPulledToPlayer {
    fn default() -> Self {
        Self { time_pulled: 0.0 }
    }
}

pub const MIN_PULL_SPEED: f32 = 10.0;
pub const MAX_PULL_SPEED: f32 = 500.0;
pub const PULL_ACCELERATION: f32 = 200.0;

pub fn mark_items_in_pickup_range(
    mut commands: Commands,
    item_query: Query<
        (Entity, &Transform, &ItemStack),
        (
            With<ItemDrop>,
            Without<Player>,
            Without<BeingPulledToPlayer>,
            Without<TouchTriggerObjectAction>,
        ),
    >,
    player_query: Query<(&Transform, &PickupRadius), With<Player>>,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
    proto: ProtoParam,
    break_drop_filter: Res<BreakDropFilter>,
) {
    let Ok((player_transform, pickup_radius)) = player_query.single() else {
        return;
    };
    let Ok(inv) = inv.single() else {
        return;
    };
    let player_has_pet = pets.iter().next().is_some();

    let player_pos = player_transform.translation.truncate();
    let pickup_range = pickup_radius.0;

    for (item_entity, item_transform, item_stack) in item_query.iter() {
        if break_drop_filter.blocks_ground_pickup(item_stack.obj_type) {
            let item_pos = item_transform.translation.truncate();
            if player_pos.distance(item_pos) <= pickup_range {
                commands.entity(item_entity).despawn();
            }
            continue;
        }
        if !player_can_accept_ground_item_pickup(item_stack, inv, player_has_pet, &proto) {
            continue;
        }
        let item_pos = item_transform.translation.truncate();
        let distance = player_pos.distance(item_pos);

        if distance <= pickup_range && distance > 0.0 {
            commands
                .entity(item_entity)
                .insert(BeingPulledToPlayer::default());
        }
    }
}

pub fn handle_item_pickup_radius(
    mut item_query: Query<
        (Entity, &mut Transform, &mut BeingPulledToPlayer, &ItemStack),
        With<BeingPulledToPlayer>,
    >,
    player_query: Query<&Transform, (With<Player>, Without<BeingPulledToPlayer>)>,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
    mut commands: Commands,
    time: Res<Time>,
    proto: ProtoParam,
    break_drop_filter: Res<BreakDropFilter>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(inv) = inv.single() else {
        return;
    };
    let player_has_pet = pets.iter().next().is_some();

    let player_pos = player_transform.translation.truncate();

    for (item_entity, mut item_transform, mut pull_state, item_stack) in item_query.iter_mut() {
        if break_drop_filter.blocks_ground_pickup(item_stack.obj_type) {
            commands.entity(item_entity).despawn();
            continue;
        }
        if !player_can_accept_ground_item_pickup(item_stack, inv, player_has_pet, &proto) {
            commands.entity(item_entity).remove::<BeingPulledToPlayer>();
            continue;
        }
        let item_pos = item_transform.translation.truncate();
        let distance = player_pos.distance(item_pos);

        if distance <= 0.1 {
            commands.entity(item_entity).remove::<BeingPulledToPlayer>();
            continue;
        }

        pull_state.time_pulled += time.delta_secs();

        let pull_speed =
            (MIN_PULL_SPEED + pull_state.time_pulled * PULL_ACCELERATION).min(MAX_PULL_SPEED);

        let direction = (player_pos - item_pos).normalize_or_zero();

        let movement = direction * pull_speed * time.delta_secs();
        item_transform.translation += movement.extend(0.0);
    }
}

/// MagnetPull heirloom cooldown/duration timers on the player. Only present
/// while the heirloom is equipped — stored `SparseSet` to keep the player
/// in a single archetype across (un)equips.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct MagnetPullTimer {
    pub cooldown_timer: Timer,
    pub duration_timer: Timer,
}

pub const BASE_MAGNET_COOLDOWN: f32 = 25.0;
pub const MAGNET_COOLDOWN_REDUCTION_PER_STACK: f32 = 2.5;
pub const MIN_MAGNET_COOLDOWN: f32 = 5.0;

/// Map-wide loot pull: marks every eligible ground item for pickup (boss kills, gamble shrine, etc.).
pub fn pull_all_eligible_ground_items_to_player(
    commands: &mut Commands,
    item_drop_query: &Query<(Entity, &ItemStack), (With<ItemDrop>, Without<BeingPulledToPlayer>)>,
    inv: &Query<&Inventory, With<Player>>,
    pets: &Query<(), With<Pet>>,
    proto: &ProtoParam,
) {
    let Ok(inv) = inv.single() else {
        return;
    };
    let player_has_pet = pets.iter().next().is_some();
    for (item_entity, item_stack) in item_drop_query.iter() {
        if !player_can_accept_ground_item_pickup(item_stack, inv, player_has_pet, proto) {
            continue;
        }
        commands
            .entity(item_entity)
            .insert(BeingPulledToPlayer::default());
    }
}

/// System that periodically pulls all item drops to the player
/// Adds BeingPulledToPlayer component to all items when the timer triggers
pub fn handle_magnet_pull(
    mut magnet_timer_query: Query<(&mut MagnetPullTimer, &PlayerSkills), With<Player>>,
    item_query: Query<
        (Entity, &ItemStack),
        (
            With<ItemDrop>,
            Without<Player>,
            Without<BeingPulledToPlayer>,
            Without<TouchTriggerObjectAction>,
        ),
    >,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
    mut commands: Commands,
    time: Res<Time>,
    proto: ProtoParam,
    break_drop_filter: Res<BreakDropFilter>,
) {
    let Ok((mut magnet_timer, player_skills)) = magnet_timer_query.single_mut() else {
        return;
    };
    let Ok(inv) = inv.single() else {
        return;
    };
    let player_has_pet = pets.iter().next().is_some();

    let stacks = player_skills.get_count(crate::player::skills::Heirloom::MagnetPull);

    if stacks == 0 {
        return;
    }

    magnet_timer.cooldown_timer.tick(time.delta());

    if magnet_timer.cooldown_timer.is_finished() {
        magnet_timer.cooldown_timer.reset();
        magnet_timer.duration_timer.reset();

        for (item_entity, item_stack) in item_query.iter() {
            if break_drop_filter.blocks_ground_pickup(item_stack.obj_type) {
                continue;
            }
            if !player_can_accept_ground_item_pickup(item_stack, inv, player_has_pet, &proto) {
                continue;
            }
            commands
                .entity(item_entity)
                .insert(BeingPulledToPlayer::default());
        }
    }
}
