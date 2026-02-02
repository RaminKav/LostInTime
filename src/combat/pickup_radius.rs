use bevy::prelude::*;

use crate::{
    attributes::PickupRange,
    item::ItemDrop,
    player::{skills::PlayerSkills, Player},
};

#[derive(Component, Debug, Clone)]
pub struct PickupRadius(pub f32);

pub const BASE_PICKUP_RADIUS: f32 = 30.0;

/// System that updates pickup radius based on ItemPickupRadius heirloom stacks and equipment
pub fn update_pickup_radius(
    mut pickup_radius_query: Query<&mut PickupRadius, With<Player>>,
    pickup_range_attr: Query<&PickupRange, With<Player>>,
) {
    let Ok(mut pickup_radius) = pickup_radius_query.get_single_mut() else {
        return;
    };
    let pickup_range_bonus = 1.0
        + (pickup_range_attr
            .get_single()
            .cloned()
            .unwrap_or_default()
            .0 as f32
            / 100.0);

    pickup_radius.0 = BASE_PICKUP_RADIUS * pickup_range_bonus;
}

/// System that pulls items toward the player when they're within pickup range
pub fn handle_item_pickup_radius(
    mut item_query: Query<(Entity, &mut Transform), (With<ItemDrop>, Without<Player>)>,
    player_query: Query<(&Transform, &PickupRadius), With<Player>>,
    time: Res<Time>,
) {
    let Ok((player_transform, pickup_radius)) = player_query.get_single() else {
        return;
    };

    let player_pos = player_transform.translation.truncate();
    let pickup_range = pickup_radius.0;

    for (_item_entity, mut item_transform) in item_query.iter_mut() {
        let item_pos = item_transform.translation.truncate();
        let distance = player_pos.distance(item_pos);

        // If item is within pickup range, pull it toward the player
        if distance <= pickup_range && distance > 0.0 {
            // Calculate direction from item to player
            let direction = (player_pos - item_pos).normalize();

            // Pull speed increases as item gets closer (faster when closer)
            // 20 /(4000)
            let pull_speed = 10.0 + (pickup_range - distance) / pickup_range * 100.0; // 10.0 to 12.0 speed

            // Move item toward player
            let movement = direction * pull_speed * time.delta().as_secs_f32();
            item_transform.translation += movement.extend(0.0);
        }
    }
}

#[derive(Component)]
pub struct MagnetPullTimer {
    pub cooldown_timer: Timer,
    pub duration_timer: Timer,
}

pub const BASE_MAGNET_COOLDOWN: f32 = 25.0;
pub const MAGNET_COOLDOWN_REDUCTION_PER_STACK: f32 = 2.5;
pub const MIN_MAGNET_COOLDOWN: f32 = 5.0;

/// System that periodically pulls all item drops to the player
pub fn handle_magnet_pull(
    mut magnet_timer_query: Query<(&mut MagnetPullTimer, &Transform, &PlayerSkills), With<Player>>,
    mut item_query: Query<&mut Transform, (With<ItemDrop>, Without<Player>)>,
    time: Res<Time>,
) {
    let Ok((mut magnet_timer, player_transform, player_skills)) =
        magnet_timer_query.get_single_mut()
    else {
        return;
    };

    let stacks = player_skills.get_count(crate::player::skills::Heirloom::MagnetPull);

    if stacks == 0 {
        return;
    }

    magnet_timer.cooldown_timer.tick(time.delta());

    if magnet_timer.cooldown_timer.finished()
        || (!magnet_timer.duration_timer.finished() && magnet_timer.duration_timer.percent() > 0.)
    {
        magnet_timer.duration_timer.tick(time.delta());
        if magnet_timer.duration_timer.finished() {
            magnet_timer.duration_timer.reset();
        }
        let player_pos = player_transform.translation.truncate();

        for mut item_transform in item_query.iter_mut() {
            let item_pos = item_transform.translation.truncate();
            let direction = (player_pos - item_pos).normalize_or_zero();

            let pull_speed = 200.0;
            let movement = direction * pull_speed * time.delta().as_secs_f32();
            item_transform.translation += movement.extend(0.0);
        }
    }
}
