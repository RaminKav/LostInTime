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

/// Marker component for deferred Aseprite animation spawning
/// This allows systems to queue animation spawns that will be processed in PreUpdate
#[derive(Component)]
pub struct SpawnAsepriteAnimationCollider {
    pub transform: Transform,
    pub duration: f32,
    pub attack: i32,
    pub collider: Collider,
    pub handle: Handle<Aseprite>,
    pub animation: AsepriteAnimation,
    pub repeating_anim: bool,
    pub projectile: Projectile,
    pub extra_components: Vec<DeferredComponent>,
    pub parent: Option<Entity>,
}

/// Enum for extra components that can be added to the spawned entity
#[derive(Clone)]
pub enum DeferredComponent {
    EnemyProjectile {
        entity: Entity,
        mob: crate::enemy::Mob,
    },
    IceExplosionDmg,
    // Add more as needed
}

pub fn spawn_temp_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    projectile: Projectile,
) -> Entity {
    commands
        .spawn(TransformBundle {
            local: transform,
            ..Default::default()
        })
        .insert(DespawnTimer(Timer::from_seconds(duration, TimerMode::Once)))
        .insert(Attack(attack))
        .insert(projectile)
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
    mut animation: AsepriteAnimation,
    repeating_anim: bool,
    projectile: Projectile,
) -> Entity {
    let hitbox_e = spawn_temp_collider(commands, transform, duration, attack, collider, projectile);

    // Force animation to start at frame 0
    animation.current_frame = 0;

    commands
        .entity(hitbox_e)
        .insert((handle, animation, PlayerAttackCollider))
        .insert(VisibilityBundle::default());

    if !repeating_anim {
        commands.entity(hitbox_e).insert(DoneAnimation);
    }
    hitbox_e
}

/// Helper to queue an Aseprite animation collider spawn for PreUpdate processing
/// Returns the marker entity that will be replaced with the actual entity
pub fn spawn_deferred_aseprite_collider(
    commands: &mut Commands,
    transform: Transform,
    duration: f32,
    attack: i32,
    collider: Collider,
    handle: Handle<Aseprite>,
    animation: AsepriteAnimation,
    repeating_anim: bool,
    projectile: Projectile,
    extra_components: Vec<DeferredComponent>,
    parent: Option<Entity>,
) -> Entity {
    commands
        .spawn(SpawnAsepriteAnimationCollider {
            transform,
            duration,
            attack,
            collider,
            handle,
            animation,
            repeating_anim,
            projectile,
            extra_components,
            parent,
        })
        .id()
}

/// System to process deferred Aseprite animation spawns in PreUpdate
/// This ensures animations are spawned before bevy_aseprite processes them
pub fn handle_deferred_aseprite_spawns(
    mut commands: Commands,
    mut query: Query<(Entity, &mut SpawnAsepriteAnimationCollider)>,
) {
    for (marker_entity, mut spawn_data) in query.iter_mut() {
        // Take ownership of the data to avoid cloning
        let transform = spawn_data.transform;
        let duration = spawn_data.duration;
        let attack = spawn_data.attack;
        let collider = std::mem::replace(&mut spawn_data.collider, Collider::ball(0.0));
        let handle = spawn_data.handle.clone();
        let animation = std::mem::replace(&mut spawn_data.animation, AsepriteAnimation::default());
        let repeating_anim = spawn_data.repeating_anim;
        let projectile = std::mem::replace(&mut spawn_data.projectile, Projectile::None);
        let extra_components = std::mem::take(&mut spawn_data.extra_components);
        let parent = spawn_data.parent;

        // Spawn the actual entity with animation
        let entity = spawn_one_time_aseprite_collider(
            &mut commands,
            transform,
            duration,
            attack,
            collider,
            handle,
            animation,
            repeating_anim,
            projectile,
        );

        // Set parent if specified
        if let Some(parent_entity) = parent {
            // Use safe entity access to avoid issues if parent doesn't exist
            if let Some(mut entity_commands) = commands.get_entity(entity) {
                entity_commands.set_parent(parent_entity);
            }
        }

        // Add extra components based on the deferred data
        for component in extra_components {
            match component {
                DeferredComponent::EnemyProjectile {
                    entity: parent_entity,
                    mob,
                } => {
                    commands
                        .entity(entity)
                        .insert(crate::item::projectile::EnemyProjectile {
                            entity: parent_entity,
                            mob,
                        });
                }
                DeferredComponent::IceExplosionDmg => {
                    commands
                        .entity(entity)
                        .insert(crate::player::mage_skills::IceExplosionDmg);
                }
            }
        }

        // Despawn the marker entity
        commands.entity(marker_entity).despawn();
    }
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
