use bevy::prelude::*;

use crate::{enemy::Mob, item::WorldObject, proto::proto_param::ProtoParam};

/// Per-entity bounce-on-hit state.
///
/// Always-present on mobs (see `ensure_mob_components`) to avoid archetype
/// churn from repeated insert/remove on hit. May still be transiently
/// inserted on UI elements/shrines where churn isn't a concern; once present
/// it's never removed — the `is_active` flag gates whether the animation
/// runs.
#[derive(Component, Debug, Clone)]
pub struct BounceOnHit {
    pub is_active: bool,
    timer: Timer,
}

impl BounceOnHit {
    pub fn new() -> Self {
        Self {
            is_active: true,
            timer: Timer::from_seconds(0.17, TimerMode::Once),
        }
    }

    /// Activate or re-activate the bounce animation.
    pub fn activate(&mut self) {
        self.is_active = true;
        self.timer = Timer::from_seconds(0.17, TimerMode::Once);
    }
}

impl Default for BounceOnHit {
    fn default() -> Self {
        Self {
            is_active: false,
            timer: Timer::from_seconds(0.17, TimerMode::Once),
        }
    }
}

pub fn bounce_on_hit(
    time: Res<Time>,
    mut bounce_on_hit_query: Query<(
        &mut Transform,
        &mut BounceOnHit,
        Option<&Mob>,
        Option<&WorldObject>,
    )>,
    proto_param: ProtoParam,
) {
    for (mut t, mut bounce_on_hit, mob_option, obj_option) in bounce_on_hit_query.iter_mut() {
        if !bounce_on_hit.is_active {
            continue;
        }
        let mut max_bounce = 2.5;
        let modifier = if let Some(obj) = obj_option {
            if obj.is_medium_size(&proto_param) {
                // large objects
                0.5
            } else if obj.is_tree() {
                // trees are not medium but very large
                0.25
            } else {
                // other small obj, crates, etc
                1.
            }
        } else if mob_option.is_some() && mob_option.unwrap().is_boss() {
            max_bounce = 1.35;
            // bosses
            0.5
        } else {
            // other mobs
            2.
        };
        bounce_on_hit.timer.tick(time.delta());
        // Bounce magnitude only; negative scale.x is used for horizontal flip (e.g. scorpion).
        // Old code used `.clamp(1., max)` on signed scale, which forced left-facing sprites to +1.
        let sign_x = t.scale.x.signum();
        let sign_x = if sign_x == 0. { 1. } else { sign_x };
        let sign_y = t.scale.y.signum();
        let sign_y = if sign_y == 0. { 1. } else { sign_y };
        let mut mag_x = t.scale.x.abs();
        let mut mag_y = t.scale.y.abs();
        let bump = 2.5 * time.delta_seconds() * modifier;
        if bounce_on_hit.timer.percent() < 0.5 {
            mag_x += bump;
            mag_y += bump;
        } else {
            mag_x -= bump;
            mag_y -= bump;
        }
        mag_x = mag_x.clamp(1., max_bounce);
        mag_y = mag_y.clamp(1., max_bounce);
        t.scale.x = sign_x * mag_x;
        t.scale.y = sign_y * mag_y;
        if bounce_on_hit.timer.finished() {
            t.scale.x = sign_x * 1.0;
            t.scale.y = sign_y * 1.0;
            bounce_on_hit.is_active = false;
        }
    }
}
