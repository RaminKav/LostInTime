use bevy::prelude::*;

use crate::{enemy::Mob, item::WorldObject, proto::proto_param::ProtoParam};

/// Default peak scale for generic entities (mobs / UI without overrides).
pub const DEFAULT_BOUNCE_MAX: f32 = 1.35;
/// Default ramp multiplier for generic entities (non-boss mobs, UI without overrides).
pub const DEFAULT_BOUNCE_MODIFIER: f32 = 1.5;
/// Base bump rate multiplied by modifier each frame during the bounce.
pub const DEFAULT_BOUNCE_BUMP_RATE: f32 = 2.5;

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
    /// When set, skips Mob/WorldObject derivation for peak scale.
    pub max_bounce: Option<f32>,
    /// When set, skips Mob/WorldObject derivation for ramp speed.
    pub modifier: Option<f32>,
    /// When set, overrides [`DEFAULT_BOUNCE_BUMP_RATE`].
    pub bump_rate: Option<f32>,
}

impl BounceOnHit {
    pub fn new() -> Self {
        Self {
            is_active: true,
            timer: Timer::from_seconds(0.17, TimerMode::Once),
            max_bounce: None,
            modifier: None,
            bump_rate: None,
        }
    }

    /// Activate or re-activate the bounce animation.
    pub fn activate(&mut self) {
        self.is_active = true;
        self.timer = Timer::from_seconds(0.17, TimerMode::Once);
    }

    /// Subtle hover bounce for active skill shrine tooltips (~30% of default strength).
    pub fn shrine_hover() -> Self {
        Self::with_strength_fraction(0.3)
    }

    /// Scale default bounce peak, ramp, and bump rate by `fraction` (e.g. `0.3` = 30%).
    pub fn with_strength_fraction(fraction: f32) -> Self {
        Self {
            is_active: false,
            timer: Timer::from_seconds(0.17, TimerMode::Once),
            max_bounce: Some(1.0 + (DEFAULT_BOUNCE_MAX - 1.0) * fraction),
            modifier: Some(DEFAULT_BOUNCE_MODIFIER * fraction),
            bump_rate: Some(DEFAULT_BOUNCE_BUMP_RATE * fraction),
        }
    }
}

impl Default for BounceOnHit {
    fn default() -> Self {
        Self {
            is_active: false,
            timer: Timer::from_seconds(0.17, TimerMode::Once),
            max_bounce: None,
            modifier: None,
            bump_rate: None,
        }
    }
}

fn bounce_strength(
    bounce_on_hit: &BounceOnHit,
    mob_option: Option<&Mob>,
    obj_option: Option<&WorldObject>,
    proto_param: &ProtoParam,
) -> (f32, f32, f32) {
    if bounce_on_hit.max_bounce.is_some()
        || bounce_on_hit.modifier.is_some()
        || bounce_on_hit.bump_rate.is_some()
    {
        return (
            bounce_on_hit.max_bounce.unwrap_or(DEFAULT_BOUNCE_MAX),
            bounce_on_hit.modifier.unwrap_or(DEFAULT_BOUNCE_MODIFIER),
            bounce_on_hit.bump_rate.unwrap_or(DEFAULT_BOUNCE_BUMP_RATE),
        );
    }

    let mut max_bounce = DEFAULT_BOUNCE_MAX;
    let modifier = if let Some(obj) = obj_option {
        if obj.is_medium_size(proto_param) {
            0.5
        } else if obj.is_tree() {
            0.25
        } else {
            1.
        }
    } else if mob_option.is_some() && mob_option.unwrap().is_boss() {
        max_bounce = 1.25;
        0.5
    } else {
        DEFAULT_BOUNCE_MODIFIER
    };

    (max_bounce, modifier, DEFAULT_BOUNCE_BUMP_RATE)
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
        let (max_bounce, modifier, bump_rate) =
            bounce_strength(&bounce_on_hit, mob_option, obj_option, &proto_param);
        bounce_on_hit.timer.tick(time.delta());
        // Bounce magnitude only; negative scale.x is used for horizontal flip (e.g. scorpion).
        // Old code used `.clamp(1., max)` on signed scale, which forced left-facing sprites to +1.
        let sign_x = t.scale.x.signum();
        let sign_x = if sign_x == 0. { 1. } else { sign_x };
        let sign_y = t.scale.y.signum();
        let sign_y = if sign_y == 0. { 1. } else { sign_y };
        let mut mag_x = t.scale.x.abs();
        let mut mag_y = t.scale.y.abs();
        let bump = bump_rate * time.delta_seconds() * modifier;
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
