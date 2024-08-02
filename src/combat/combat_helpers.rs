use bevy::prelude::*;
use bevy_rapier2d::prelude::{ActiveCollisionTypes, ActiveEvents, Collider, Sensor};

use crate::{
    attributes::Attack,
    item::projectile::{Projectile, ProjectileState},
};
#[derive(Component)]
pub struct DespawnTimer(pub Timer);

pub fn spawn_temp_collider(
    commands: &mut Commands,
    transform: Transform,
    size: Vec2,
    duration: f32,
    attack: i32,
) {
    commands
        .spawn(TransformBundle {
            local: transform,
            ..Default::default()
        })
        .insert(DespawnTimer(Timer::from_seconds(duration, TimerMode::Once)))
        .insert(Attack(attack))
        .insert(Projectile::TeleportShock)
        .insert(Sensor)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(ActiveCollisionTypes::all())
        .insert(ProjectileState {
            speed: 0.,
            direction: Vec2::ZERO,
            hit_entities: vec![],
            spawn_offset: Vec2::ZERO,
            rotating: false,
        })
        .insert(Collider::cuboid(size.x / 2., size.y / 2.));
}

pub fn tick_despawn_timer(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnTimer)>,
) {
    for (entity, mut timer) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
