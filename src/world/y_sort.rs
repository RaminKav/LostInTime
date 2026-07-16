use bevy::prelude::*;

use crate::assets::SpriteAnchor;

pub struct YSortPlugin;

impl Plugin for YSortPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(Self::y_sort);
    }
}

#[derive(Component, Reflect, FromReflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct YSort(pub f32);

/// Depth used by [`Transparent2d`] for Y-sorted world sprites and Hanabi FX.
pub fn y_sort_depth(bias: f32, world_y: f32, world_x: f32, anchor_offset_y: f32) -> f32 {
    bias + 900.
        - (900.0f32
            / (1.0f32 + (2.0f32.powf(-0.00001 * (world_y - anchor_offset_y)))))
        - 0.00001 * world_x
}

impl YSortPlugin {
    pub fn y_sort(
        mut q: Query<(
            &mut Transform,
            &GlobalTransform,
            Option<&SpriteAnchor>,
            &YSort,
        )>,
    ) {
        for (mut tf, gtf, anchor_option, y_sort) in q.iter_mut() {
            let anchor_offset = anchor_option.map(|a| a.0.y).unwrap_or(0.);
            let pos = gtf.translation();
            tf.translation.z = y_sort_depth(y_sort.0, pos.y, pos.x, anchor_offset);
        }
    }
}
