//! Era 3 ice patch slide + post-ice momentum (movement lock, acceleration cap).

use bevy::prelude::Vec2;
use bevy_rapier2d::prelude::KinematicCharacterControllerOutput;

use super::PlayerState;

/// Cap on [`PlayerState::ice_slide_speed_factor`] (multiplier on normal per-frame move delta `s`).
const ICE_SLIDE_MAX_SPEED: f32 = 5.8;
const ICE_SLIDE_ACCEL_PER_SEC: f32 = 7.;
pub(crate) const ICE_MOMENTUM_DURATION_SECS: f32 = 0.15;

const BLOCKED_MIN_DESIRED_SQ: f32 = 0.04;
const BLOCKED_MAX_EFFECTIVE_SQ: f32 = 0.04;

fn clear_all_ice_slide(player: &mut PlayerState) {
    player.ice_slide_direction = None;
    player.ice_momentum_direction = None;
    player.ice_momentum_remaining = 0.0;
    player.ice_slide_speed_factor = 1.0;
}

/// Clears ice lock when physics blocked the slide (e.g. hit a tree), or when the player is on ice
/// with a slide lock but did not actually move last frame while pressing a direction (escape hatch).
pub fn clear_ice_slide_when_stuck(
    player: &mut PlayerState,
    on_ice: bool,
    d_raw: Vec2,
    kcc: Option<&KinematicCharacterControllerOutput>,
) {
    let sliding = on_ice && player.ice_slide_direction.is_some();
    let momentum = player.ice_momentum_remaining > 0.0 && player.ice_momentum_direction.is_some();
    if !(sliding || momentum) {
        return;
    }
    let Some(kcc) = kcc else {
        return;
    };

    let eff_sq = kcc.effective_translation.length_squared();
    let physics_blocked = kcc.desired_translation.length_squared() > BLOCKED_MIN_DESIRED_SQ
        && eff_sq < BLOCKED_MAX_EFFECTIVE_SQ;

    let stuck_on_ice_new_steer = on_ice
        && player.ice_slide_direction.is_some()
        && eff_sq < BLOCKED_MAX_EFFECTIVE_SQ
        && d_raw.length_squared() > 0.01;

    if physics_blocked || stuck_on_ice_new_steer {
        clear_all_ice_slide(player);
    }
}

/// Resolves ice slide / momentum movement into `d`, updating [`PlayerState`].
pub fn tick_ice_slide_movement(
    player: &mut PlayerState,
    on_ice: bool,
    d_raw: Vec2,
    s: f32,
    delta_secs: f32,
    is_dashing: bool,
    hit_tracker_active: bool,
) -> Vec2 {
    if on_ice {
        player.ice_momentum_remaining = 0.0;
        player.ice_momentum_direction = None;
    } else if player.ice_slide_direction.is_some() {
        player.ice_momentum_direction = player.ice_slide_direction.take();
        player.ice_momentum_remaining = ICE_MOMENTUM_DURATION_SECS;
    }

    let mut d = Vec2::ZERO;
    if on_ice
        && !is_dashing
        && !hit_tracker_active
        && (player.ice_slide_direction.is_some() || d_raw.x != 0. || d_raw.y != 0.)
    {
        if player.ice_slide_direction.is_none() && (d_raw.x != 0. || d_raw.y != 0.) {
            player.ice_slide_direction = Some(d_raw.normalize());
            player.ice_slide_speed_factor = 1.0;
        }
        if let Some(dir) = player.ice_slide_direction {
            player.ice_slide_speed_factor += ICE_SLIDE_ACCEL_PER_SEC * delta_secs;

            player.ice_slide_speed_factor = player.ice_slide_speed_factor.min(ICE_SLIDE_MAX_SPEED);
            d = dir * s * player.ice_slide_speed_factor;
            player.is_moving = true;
        }
    } else if player.ice_momentum_remaining > 0.0 && !is_dashing && !hit_tracker_active {
        if let Some(dir) = player.ice_momentum_direction {
            player.ice_slide_speed_factor += ICE_SLIDE_ACCEL_PER_SEC * delta_secs;
            player.ice_slide_speed_factor = player.ice_slide_speed_factor.min(ICE_SLIDE_MAX_SPEED);
            d = dir * s * player.ice_slide_speed_factor;
            player.is_moving = true;
        }
        player.ice_momentum_remaining -= delta_secs;
        if player.ice_momentum_remaining <= 0.0 {
            player.ice_momentum_remaining = 0.0;
            player.ice_momentum_direction = None;
            player.ice_slide_speed_factor = 1.0;
        }
    } else {
        if !on_ice {
            player.ice_slide_direction = None;
        }
        player.ice_slide_speed_factor = 1.0;
        if d_raw.x != 0. || d_raw.y != 0. {
            d = d_raw.normalize() * s;
        }
    }
    d
}
