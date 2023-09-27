use bevy::prelude::*;

use crate::{item::WorldObject, proto::proto_param::ProtoParam};

#[derive(Component)]
pub struct BounceOnHit {
    timer: Timer,
}

impl BounceOnHit {
    pub fn new() -> Self {
        Self {
            timer: Timer::from_seconds(0.17, TimerMode::Once),
        }
    }
}

pub fn bounce_on_hit(
    mut commands: Commands,
    time: Res<Time>,
    mut bounce_on_hit_query: Query<(
        Entity,
        &mut Transform,
        &mut BounceOnHit,
        Option<&WorldObject>,
    )>,
    proto_param: ProtoParam,
) {
    for (e, mut t, mut bounce_on_hit, obj_option) in bounce_on_hit_query.iter_mut() {
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
        } else {
            // mobs
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
            commands.entity(e).remove::<BounceOnHit>();
        }
    }
}
