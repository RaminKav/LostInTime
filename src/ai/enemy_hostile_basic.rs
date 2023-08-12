use bevy::prelude::*;
use bevy_rapier2d::prelude::KinematicCharacterController;
use rand::{rngs::ThreadRng, Rng};
use seldom_state::prelude::*;

use crate::{
    combat::HitEvent,
    item::projectile::{Projectile, RangedAttackEvent},
};

// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct LineOfSight {
    pub target: Entity,
    pub range: f32,
}

impl Trigger for LineOfSight {
    type Param<'w, 's> = (Query<'w, 's, &'static Transform>, Res<'w, Time>);
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(
        &self,
        entity: Entity,
        (transforms, _time): Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        let delta = transforms.get(self.target).unwrap().translation.truncate()
            - transforms.get(entity).unwrap().translation.truncate();

        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
        (distance <= self.range).then_some(distance).ok_or(distance)
    }
}
// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct HurtByPlayer;

impl BoolTrigger for HurtByPlayer {
    type Param<'w, 's> = EventReader<'w, 's, HitEvent>;

    fn trigger(&self, entity: Entity, mut hit_events: Self::Param<'_, '_>) -> bool {
        for hit in hit_events.iter() {
            if hit.hit_entity == entity {
                return true;
            }
        }
        return false;
    }
}
// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct AttackDistance {
    pub target: Entity,
    pub range: f32,
}

impl Trigger for AttackDistance {
    type Param<'w, 's> = (
        Query<'w, 's, (&'static Transform, Option<&'static LeapAttackState>)>,
        Res<'w, Time>,
    );
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(
        &self,
        entity: Entity,
        (transforms, _time): Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        if let Some(attack) = transforms.get(entity).unwrap().1 {
            if !attack.attack_cooldown_timer.finished() {
                return Ok(0.);
            }
        }
        let delta = transforms
            .get(self.target)
            .unwrap()
            .0
            .translation
            .truncate()
            - transforms.get(entity).unwrap().0.translation.truncate();

        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
        (distance <= self.range).then_some(distance).ok_or(distance)
    }
}

// Entities in the `Idle` state should walk in a given direction,
// then change direction after a set timer
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct IdleState {
    pub walk_timer: Timer,
    pub direction: MoveDirection,
    pub speed: f32,
}
#[derive(Clone, Copy, Debug, Reflect, PartialEq, Eq)]
pub enum MoveDirection {
    Left,
    Right,
    Up,
    Down,
}
impl MoveDirection {
    fn get_next_rand_dir(self, mut rng: ThreadRng) -> Self {
        let mut new_dir = self;
        while new_dir == self {
            let rng = rng.gen_range(0..=4);
            if rng <= 1 {
                new_dir = Self::Left;
            } else if rng <= 2 {
                new_dir = Self::Right;
            } else if rng <= 3 {
                new_dir = Self::Up;
            } else if rng <= 4 {
                new_dir = Self::Down;
            }
        }
        new_dir
    }
    pub fn new_rand_dir(mut rng: ThreadRng) -> Self {
        let mut new_dir = Self::Left;

        let rng = rng.gen_range(0..=4);
        if rng <= 1 {
            new_dir = Self::Left;
        } else if rng <= 2 {
            new_dir = Self::Right;
        } else if rng <= 3 {
            new_dir = Self::Up;
        } else if rng <= 4 {
            new_dir = Self::Down;
        }
        new_dir
    }
    // pub fn from_translation(t: Vec2) -> Self {

    // }
}

// Entities in the `Follow` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct FollowState {
    pub target: Entity,
    pub speed: f32,
}
// Entities in the `Attack` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct LeapAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_duration_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub speed: f32,
    pub dir: Option<Vec2>,
}

// Entities in the `Attack` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct ProjectileAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub dir: Option<Vec2>,
    pub projectile: Projectile,
}

pub fn follow(
    mut transforms: Query<&mut Transform>,
    mut mover: Query<&mut KinematicCharacterController>,
    follows: Query<(Entity, &FollowState)>,
) {
    for (entity, follow) in &follows {
        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;
        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_translation = follow_transform.translation;
        // Find the direction from the follower to the target and go that way
        mover.get_mut(entity).unwrap().translation = Some(
            (target_translation - follow_translation)
                .normalize_or_zero()
                .truncate()
                * follow.speed,
        );
        // * time.delta_seconds();
    }
}

pub fn leap_attack(
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(
        Entity,
        &mut KinematicCharacterController,
        &mut LeapAttackState,
    )>,
    time: Res<Time>,
) {
    for (entity, mut kcc, mut attack) in attacks.iter_mut() {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation;
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;

        let hit = false;
        if attack.attack_startup_timer.finished() && !attack.attack_duration_timer.finished() {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(delta.normalize_or_zero().truncate() * attack.speed);
            }
            kcc.translation = Some(attack.dir.unwrap());
            attack.attack_duration_timer.tick(time.delta());
        }

        if attack.attack_cooldown_timer.finished() {
            attack.attack_startup_timer.reset();
            attack.attack_duration_timer.reset();
            attack.attack_cooldown_timer.reset();
        }

        if hit || attack.attack_duration_timer.finished() {
            //start attack cooldown timer
            attack.dir = None;
            attack.attack_cooldown_timer.tick(time.delta());
        } else {
            attack.attack_startup_timer.tick(time.delta());
        }
    }
}
pub fn projectile_attack(
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(Entity, &mut ProjectileAttackState)>,
    mut events: EventWriter<RangedAttackEvent>,
    time: Res<Time>,
) {
    for (entity, mut attack) in attacks.iter_mut() {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation;
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;

        if attack.attack_startup_timer.finished() && attack.attack_cooldown_timer.percent() == 0. {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(delta.normalize_or_zero().truncate());
            }

            events.send(RangedAttackEvent {
                projectile: attack.projectile.clone(),
                direction: attack.dir.unwrap(),
                from_enemy: Some(entity),
            });
        }
        if attack.attack_startup_timer.finished() {
            attack.attack_cooldown_timer.tick(time.delta());
        }

        if attack.attack_cooldown_timer.finished() {
            attack.attack_startup_timer.reset();
            attack.attack_cooldown_timer.reset();
        }

        attack.dir = None;
        attack.attack_startup_timer.tick(time.delta());
    }
}
pub fn idle(
    mut transforms: Query<&mut KinematicCharacterController>,
    mut idles: Query<(Entity, &mut IdleState)>,
    time: Res<Time>,
) {
    for (entity, mut idle) in idles.iter_mut() {
        // Get the positions of the follower and target
        idle.walk_timer.tick(time.delta());
        let mut idle_transform = transforms.get_mut(entity).unwrap();

        let s = idle.speed; //* time.delta_seconds();
        match idle.direction {
            MoveDirection::Left => idle_transform.translation = Some(Vec2::new(-s, 0.)),
            MoveDirection::Right => idle_transform.translation = Some(Vec2::new(s, 0.)),
            MoveDirection::Up => idle_transform.translation = Some(Vec2::new(0., s)),
            MoveDirection::Down => idle_transform.translation = Some(Vec2::new(0., -s)),
        }

        if idle.walk_timer.just_finished() {
            idle.direction = idle.direction.get_next_rand_dir(rand::thread_rng());
        }
    }
}
