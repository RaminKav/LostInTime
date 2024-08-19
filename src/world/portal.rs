use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite};

use crate::player::Player;

#[derive(Component)]
pub struct TimePortal;

aseprite!(pub Portal, "textures/portal/portal.ase");

pub fn handle_player_near_portal(
    player_query: Query<&Transform, With<Player>>,
    mut portal_query: Query<(&GlobalTransform, &mut AsepriteAnimation), With<TimePortal>>,
) {
    for player_transform in player_query.iter() {
        for (portal_transform, mut anim) in portal_query.iter_mut() {
            let distance = player_transform
                .translation
                .distance(portal_transform.translation());
            if distance <= 32. && anim.current_frame() <= 8 {
                *anim = AsepriteAnimation::from(Portal::tags::ERA2);
            }
        }
    }
    for (_portal_transform, mut anim) in portal_query.iter_mut() {
        if anim.current_frame() == 35 {
            *anim = AsepriteAnimation::from(Portal::tags::IDLE);
        }
    }
}
