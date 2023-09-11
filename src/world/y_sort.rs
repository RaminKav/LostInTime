use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use crate::assets::SpriteAnchor;

pub struct YSortPlugin;

impl Plugin for YSortPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(Self::y_sort);
    }
}

#[derive(Component, Reflect, Schematic, FromReflect, Default)]
#[reflect(Component, Schematic)]
pub struct YSort(pub f32);

impl YSortPlugin {
    fn y_sort(
        mut q: Query<(
            &mut Transform,
            &GlobalTransform,
            Option<&SpriteAnchor>,
            &YSort,
        )>,
    ) {
        for (mut tf, gtf, anchor_option, y_sort) in q.iter_mut() {
            // tf.translation.z = 1. - 1.0f32 / (1.0f32 + (2.0f32.powf(-0.01 * tf.translation.y)));
            let anchor_offset = anchor_option.map(|a| a.0.y).unwrap_or(0.);
            tf.translation.z = y_sort.0 + 900.
                - 900.0f32
                    / (1.0f32 + (2.0f32.powf(-0.00001 * (gtf.translation().y - anchor_offset))));
        }
    }
}
