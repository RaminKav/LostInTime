use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    Collider, KinematicCharacterController, MoveShapeOptions, QueryFilter, RapierContext,
};
use rand::{rngs::ThreadRng, Rng};
use seldom_state::prelude::*;

use crate::{combat::HitEvent, inventory::ItemStack, Player};

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
        (transforms, _time): &Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        let delta = transforms.get(self.target).unwrap().translation.truncate()
            - transforms.get(entity).unwrap().translation.truncate();

        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
        (distance <= self.range).then_some(distance).ok_or(distance)
    }
}
// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct AttackDistance {
    pub target: Entity,
    pub range: f32,
}

impl Trigger for AttackDistance {
    type Param<'w, 's> = (Query<'w, 's, &'static Transform>, Res<'w, Time>);
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(
        &self,
        entity: Entity,
        (transforms, _time): &Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        let delta = transforms.get(self.target).unwrap().translation.truncate()
            - transforms.get(entity).unwrap().translation.truncate();

        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
        (distance <= self.range).then_some(distance).ok_or(distance)
    }
}

// Entities in the `Idle` state should walk in a given direction,
// then change direction after a set timer
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct Idle {
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
pub struct Follow {
    pub target: Entity,
    pub speed: f32,
}
// Entities in the `Attack` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct Attack {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub speed: f32,
    pub damage: u8,
}

pub fn follow(
    mut transforms: Query<&mut Transform>,
    mut mover: Query<&mut KinematicCharacterController>,
    follows: Query<(Entity, &Follow)>,
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

pub fn attack(
    mut transforms: Query<(&mut Transform, &Collider)>,
    mut attacks: Query<(Entity, &mut Attack)>,
    player_query: Query<Entity, With<Player>>,
    item_stack_query: Query<Entity, With<ItemStack>>,
    mut context: ResMut<RapierContext>,
    mut hit_event: EventWriter<HitEvent>,
    time: Res<Time>,
) {
    for (entity, mut attack) in attacks.iter_mut() {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().0.translation;
        let (mut attack_transform, attack_col) = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;

        let delta = target_translation - attack_translation;

        let mut hit = false;
        if attack.attack_startup_timer.finished() {
            let output = context.move_shape(
                delta.normalize_or_zero().truncate() * attack.speed,
                attack_col,
                attack_translation.truncate(),
                0.,
                0.,
                &MoveShapeOptions::default(),
                QueryFilter {
                    // flags: QueryFilterFlags::EXCLUDE_SENSORS,
                    predicate: Some(&|e| {
                        if item_stack_query.get(e).is_ok() || e == entity {
                            false
                        } else {
                            true
                        }
                    }),
                    ..default()
                },
                |col| {
                    let p_e = player_query.single();
                    if col.entity == p_e && !hit {
                        hit = true;

                        //send hit event
                        hit_event.send(HitEvent {
                            hit_entity: p_e,
                            damage: attack.damage,
                            dir: delta.normalize_or_zero().truncate(),
                        });
                    }
                },
            );
            attack_transform.translation += output.effective_translation.extend(0.);
            attack.attack_cooldown_timer.tick(time.delta());
        }

        if attack.attack_cooldown_timer.finished() || hit {
            attack.attack_cooldown_timer.reset();
            attack.attack_startup_timer.reset();
        }

        if hit || attack.attack_cooldown_timer.percent() != 0. {
            //start attack cooldown timer
            attack.attack_cooldown_timer.tick(time.delta());
        } else {
            attack.attack_startup_timer.tick(time.delta());
        }
    }
}
pub fn idle(
    mut transforms: Query<&mut KinematicCharacterController>,
    mut idles: Query<(Entity, &mut Idle)>,
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
// let output_ws = context.move_shape(
//     Vec2::new(0., d.y),
//     player_collider,
//     raw_pos.0,
//     0.,
//     0.,
//     &MoveShapeOptions::default(),
//     QueryFilter {
//         // flags: QueryFilterFlags::EXCLUDE_SENSORS,
//         exclude_collider: Some(ent),
//         predicate: Some(&|e| {
//             if let Some(c) = children {
//                 !c.iter().any(|cc| *cc == e)
//             } else {
//                 true
//             }
//         }),
//         ..default()
//     },
//     |col| {
//         for (item_stack_entity, _, _) in game.items_query.iter() {
//             if col.entity == item_stack_entity && !collected_drops.contains(&col.entity) {
//                 collected_drops.insert(col.entity);
//             }
//         }
//     },
// );
