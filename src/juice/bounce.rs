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
            // bosses
            0.5
        } else {
            // other mobs
            2.
        };
        bounce_on_hit.timer.tick(time.delta());
        if bounce_on_hit.timer.percent() < 0.5 {
            t.scale.x += 2.5 * time.delta_seconds() * modifier;
            t.scale.y += 2.5 * time.delta_seconds() * modifier;
        } else {
            t.scale.x -= 2.5 * time.delta_seconds() * modifier;
            t.scale.y -= 2.5 * time.delta_seconds() * modifier;
        }
        if bounce_on_hit.timer.finished() {
            t.scale.x = 1.0;
            t.scale.y = 1.0;
            bounce_on_hit.is_active = false;
        }
        t.scale.x = t.scale.x.clamp(1., 2.5);
        t.scale.y = t.scale.y.clamp(1., 2.5);
    }
}
