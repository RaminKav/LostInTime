use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

pub struct YSortPlugin;

impl Plugin for YSortPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(Self::y_sort);
    }
}

#[derive(Component, Reflect, Schematic, FromReflect, Default)]
#[reflect(Component, Schematic)]
pub struct YSort;

impl YSortPlugin {
    fn y_sort(mut q: Query<(&mut Transform, &GlobalTransform), With<YSort>>) {
        for (mut tf, gtf) in q.iter_mut() {
            // tf.translation.z = 1. - 1.0f32 / (1.0f32 + (2.0f32.powf(-0.01 * tf.translation.y)));
            tf.translation.z =
                900. - 900.0f32 / (1.0f32 + (2.0f32.powf(-0.00001 * gtf.translation().y)));
        }
    }
}
