use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{CurrentHealth, MaxHealth},
    colors::{GREEN, RED, UI_GRASS_GREEN, YELLOW},
};

#[derive(Component)]
pub struct DamageNumber(pub Timer);

#[derive(Component)]
pub struct PreviousHealth(i32);

pub fn add_previous_health(
    mut commands: Commands,
    query: Query<(Entity, &MaxHealth), Added<MaxHealth>>,
) {
    for (entity, max_health) in query.iter() {
        commands.entity(entity).insert(PreviousHealth(max_health.0));
    }
}
// a function that adds damage numbers to the screen in response to a [HitEvent].
// the damage numbers are [Text2DBundle]s with a [DamageNumber] component.
// the [DamageNumber] component is used to delete the damage number after a short delay.

pub fn handle_add_damage_numbers_after_hit(
    mut commands: Commands,
    mut changed_health: Query<
        (Entity, &CurrentHealth, &mut PreviousHealth),
        Changed<CurrentHealth>,
    >,
    txfms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
) {
    for (e, changed_health, mut prev_health) in changed_health.iter_mut() {
        let delta = changed_health.0 - prev_health.0;
        if delta == 0 {
            continue;
        }
        let mut rng = rand::thread_rng();
        let drop_spread = 10.;
        let pos_offset = Vec3::new(
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            1.,
        );
        prev_health.0 = changed_health.0;
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    if delta < 0 {
                        format!("{}", delta.abs())
                    } else {
                        format!("+{}", delta)
                    },
                    TextStyle {
                        font: asset_server.load("fonts/Kitchen Sink.ttf"),
                        font_size: 8.0,
                        color: if delta > 0 { UI_GRASS_GREEN } else { RED },
                    },
                ),
                transform: Transform {
                    translation: txfms.get(e).unwrap().translation() + pos_offset,
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(DamageNumber(Timer::from_seconds(0.5, TimerMode::Once)));
    }
}

// function that ticks the [DamageNumber] timer and deletes the [DamageNumber] when the timer is done.
pub fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DamageNumber, &mut Transform)>,
) {
    for (entity, mut damage_number, mut t) in query.iter_mut() {
        damage_number.0.tick(time.delta());
        t.translation.y += 10. * time.delta_seconds();
        if damage_number.0.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
