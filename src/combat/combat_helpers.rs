use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, Aseprite};
use bevy_rapier2d::prelude::{ActiveCollisionTypes, ActiveEvents, Collider, Sensor};

use crate::{
    animations::DoneAnimation,
    attributes::Attack,
    item::projectile::{Projectile, ProjectileState},
};

use super::collisions::PlayerAttackCollider;
#[derive(Component)]
pub struct DespawnTimer(pub Timer);

pub fn spawn_temp_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
) -> Entity {
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
            mana_bar_full: false,
        })
        .insert(collider)
        .id()
}

pub fn spawn_one_time_aseprite_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    handle: Handle<Aseprite>,
    animation: AsepriteAnimation,
) -> Entity {
    let hitbox_e = spawn_temp_collider(commands, transform, duration, attack, collider);

    commands
        .entity(hitbox_e)
        .insert((handle, animation, DoneAnimation, PlayerAttackCollider));
    hitbox_e
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
