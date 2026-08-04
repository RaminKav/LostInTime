use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{AnimationRepeat, AseAnimation, Aseprite};
use bevy_rapier2d::prelude::{
    ActiveCollisionTypes, ActiveEvents, Collider, CollisionGroups, Group, RigidBody, Sensor,
};

use crate::{
    animations::DoneAnimation,
    aseprite_helpers::ase_animation,
    attributes::Attack,
    ecs_helpers::SafeHierarchyExt,
    item::projectile::{Projectile, ProjectileState},
};

use super::collisions::PlayerAttackCollider;

#[derive(Component)]
pub struct DespawnTimer(pub Timer);

/// Extra components that can be added when spawning an aseprite collider.
#[derive(Clone)]
pub enum DeferredComponent {
    EnemyProjectile {
        entity: Entity,
        mob: crate::enemy::Mob,
    },
    IceExplosionDmg,
}

pub fn spawn_temp_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    projectile: Projectile,
) -> Entity {
    let category = projectile.animation_category();
    commands
        .spawn(transform)
        .insert(DespawnTimer(Timer::from_seconds(duration, TimerMode::Once)))
        .insert(Attack(attack))
        .insert(projectile)
        .insert(category)
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
            despawn_on_hit: false,
            world_object_pierce_count: 0,
        })
        .insert(collider)
        .id()
}

/// Short-lived sensor hitbox for an enemy melee strike. Uses [`Group::GROUP_1`] membership
/// (hostile mobs) and filters player [`Group::GROUP_2`] plus other hostile colliders.
pub fn spawn_enemy_melee_hitbox(
    commands: &mut Commands,
    world_pos: Vec3,
    duration: f32,
    attack: i32,
    collider: Collider,
    strike_dir: Vec2,
) -> Entity {
    let entity = spawn_temp_collider(
        commands,
        Transform::from_translation(world_pos),
        duration,
        attack,
        collider,
        Projectile::None,
    );
    commands.entity(entity).insert((
        RigidBody::Fixed,
        CollisionGroups::new(Group::GROUP_1, Group::GROUP_1 | Group::GROUP_2),
        ProjectileState {
            speed: 0.,
            direction: strike_dir.normalize_or_zero(),
            hit_entities: vec![],
            spawn_offset: Vec2::ZERO,
            rotating: false,
            mana_bar_full: false,
            despawn_on_hit: false,
            world_object_pierce_count: 0,
        },
    ));
    entity
}

/// Spawn a collider entity with a native aseprite animation.
///
/// Returns the real entity id (not a marker). `Commands` are already deferred to the
/// next sync point — enough for ultra's PostUpdate render / next PreUpdate tick.
pub fn spawn_aseprite_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    handle: Handle<Aseprite>,
    tag: &str,
    repeating_anim: bool,
    projectile: Projectile,
    extra_components: Vec<DeferredComponent>,
    parent: Option<Entity>,
) -> Entity {
    let hitbox_e = spawn_temp_collider(commands, transform, duration, attack, collider, projectile);

    let animation = ase_animation(handle, tag, !repeating_anim);

    commands.entity(hitbox_e).insert((
        animation,
        Sprite::default(),
        PlayerAttackCollider,
        Visibility::default(),
    ));

    if !repeating_anim {
        commands.entity(hitbox_e).insert(DoneAnimation);
    }

    if let Some(parent_entity) = parent {
        commands.entity(hitbox_e).safe_set_parent(parent_entity);
    }

    for component in extra_components {
        match component {
            DeferredComponent::EnemyProjectile {
                entity: parent_entity,
                mob,
            } => {
                commands
                    .entity(hitbox_e)
                    .insert(crate::item::projectile::EnemyProjectile {
                        entity: parent_entity,
                        mob,
                    });
            }
            DeferredComponent::IceExplosionDmg => {
                commands
                    .entity(hitbox_e)
                    .insert(crate::player::mage_skills::IceExplosionDmg);
            }
        }
    }

    hitbox_e
}

/// Spawn using an existing [`AseAnimation`] (tag/repeat already configured).
pub fn spawn_one_time_aseprite_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    handle: Handle<Aseprite>,
    mut animation: AseAnimation,
    repeating_anim: bool,
    projectile: Projectile,
) -> Entity {
    let hitbox_e = spawn_temp_collider(commands, transform, duration, attack, collider, projectile);

    animation.aseprite = handle;
    if !repeating_anim {
        animation.animation.repeat = AnimationRepeat::Count(1);
        animation.animation.start();
    }

    commands.entity(hitbox_e).insert((
        animation,
        Sprite::default(),
        PlayerAttackCollider,
        Visibility::default(),
    ));

    if !repeating_anim {
        commands.entity(hitbox_e).insert(DoneAnimation);
    }
    hitbox_e
}

/// Preferred call-site API — spawns the real entity immediately via `Commands`.
pub fn spawn_deferred_aseprite_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    handle: Handle<Aseprite>,
    tag: &str,
    repeating_anim: bool,
    projectile: Projectile,
    extra_components: Vec<DeferredComponent>,
    parent: Option<Entity>,
) -> Entity {
    spawn_aseprite_collider(
        commands,
        transform,
        duration,
        attack,
        collider,
        handle,
        tag,
        repeating_anim,
        projectile,
        extra_components,
        parent,
    )
}

pub fn tick_despawn_timer(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut DespawnTimer)>,
) {
    for (entity, mut timer) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}
