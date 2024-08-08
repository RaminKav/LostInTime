use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionGroups, Group, KinematicCharacterController};

use rand::Rng;
use seldom_state::prelude::*;

use crate::{
    ai::pathfinding::{world_pos_to_AIPos, AIPos_to_world_pos},
    animations::enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState},
    combat::HitEvent,
    enemy::{FollowSpeed, Mob, MobIsAttacking},
    inputs::FacingDirection,
    item::projectile::{Projectile, RangedAttackEvent},
    night::NightTracker,
    status_effects::Slow,
    world::TILE_SIZE,
    Game, GameParam, PLAYER_MOVE_SPEED,
};

use super::pathfinding::get_next_tile_A_star;

// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct LineOfSight {
    pub target: Entity,
    pub range: f32,
}
#[derive(Component)]
pub struct EnemyAttackCooldown(pub Timer);

impl Trigger for LineOfSight {
    type Param<'w, 's> = (
        Query<'w, 's, &'static Transform>,
        Res<'w, Time>,
        Res<'w, NightTracker>,
    );
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(
        &self,
        entity: Entity,
        (transforms, _time, night_tracker): Self::Param<'_, '_>,
    ) -> Result<f32, f32> {
        if night_tracker.is_night() {
            return Ok(0.);
        }
        if let Ok(tfxm) = transforms.get(entity) {
            let delta = transforms.get(self.target).unwrap().translation.truncate()
                - tfxm.translation.truncate();

            let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
            return (distance <= self.range).then_some(distance).ok_or(distance);
        } else {
            Err(0.)
        }
    }
}

// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct NightTimeAggro;

impl Trigger for NightTimeAggro {
    type Param<'w, 's> = Res<'w, NightTracker>;
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(&self, _entity: Entity, night_tracker: Self::Param<'_, '_>) -> Result<f32, f32> {
        if night_tracker.is_night() {
            Ok(1.)
        } else {
            Err(0.)
        }
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
        Query<'w, 's, (&'static Transform, Option<&'static EnemyAttackCooldown>)>,
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
        if let Some(_) = transforms.get(entity).unwrap().1 {
            return Err(0.);
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
    pub direction: FacingDirection,
    pub speed: f32,
    pub is_stopped: bool,
}

// Entities in the `Follow` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct FollowState {
    pub target: Entity,
    pub curr_path: Option<Vec2>,
    pub curr_delta: Option<Vec2>,
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
    mut follows: Query<(
        Entity,
        &mut FollowState,
        &TextureAtlasSprite,
        &CharacterAnimationSpriteSheetData,
        &EnemyAnimationState,
        Option<&EnemyAttackCooldown>,
        Option<&Slow>,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    mut game: GameParam,
    night_tracker: Res<NightTracker>,
) {
    for (entity, mut follow, sprite, anim_data, anim_state, att_cooldown, slowed_option) in
        follows.iter_mut()
    {
        if att_cooldown.is_some() && att_cooldown.unwrap().0.percent() <= 0.5 {
            return;
        }
        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;

        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_collider_offset = Vec2::new(0., -3.);
        let follow_translation = AIPos_to_world_pos(world_pos_to_AIPos(
            follow_transform.translation.truncate() + follow_collider_offset,
        ));

        let next_target_tile = get_next_tile_A_star(
            &target_translation.truncate(),
            &follow_translation,
            &mut game,
        );

        let distance_from_target = (target_translation.truncate() - follow_translation).length();
        let is_far_away = distance_from_target > 12. * TILE_SIZE.x;
        //convert follower txfm to AIPos too
        let target_txfm = if night_tracker.is_night() && is_far_away {
            target_translation.truncate()
        } else {
            next_target_tile.unwrap_or(target_translation.truncate())
        };
        let direct_path_to_target = (target_txfm - follow_translation).normalize_or_zero();
        let delta_override: Option<Vec2> = if let Some(curr_path) = follow.curr_path {
            if curr_path == target_txfm {
                Some(
                    follow
                        .curr_delta
                        .expect("delta should exist if curr_path exists"),
                )
            } else {
                None
            }
        } else {
            None
        };
        let delta = delta_override.unwrap_or(direct_path_to_target);
        let mut mover = mover.get_mut(entity).unwrap();
        if night_tracker.is_night() && is_far_away {
            mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));
        } else {
            mover.filter_groups = Some(CollisionGroups::default());
        }

        follow.curr_path = next_target_tile;
        follow.curr_delta = Some(delta);

        mover.translation = Some(
            delta
                * follow.speed
                * PLAYER_MOVE_SPEED
                * time.delta_seconds()
                * (1. - slowed_option.map_or(0., |s| s.num_stacks as f32 * 0.15)),
        );
        commands
            .entity(entity)
            .insert(FacingDirection::from_translation(delta));
        if sprite.index == anim_data.get_starting_frame_for_animation(anim_state)
            && anim_state != &EnemyAnimationState::Hit
            && anim_state != &EnemyAnimationState::Walk
        {
            commands.entity(entity).insert(EnemyAnimationState::Walk);
        }
    }
}

pub fn leap_attack(
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(
        Entity,
        &Mob,
        &mut KinematicCharacterController,
        &mut LeapAttackState,
        &FollowSpeed,
        &mut TextureAtlasSprite,
        &CharacterAnimationSpriteSheetData,
        &EnemyAnimationState,
        Option<&Slow>,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    _game: Res<Game>,
) {
    for (
        entity,
        mob,
        mut kcc,
        mut attack,
        follow_speed,
        sprite,
        anim_data,
        anim_state,
        slow_option,
    ) in attacks.iter_mut()
    {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation;
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;

        if attack.attack_startup_timer.finished() && !attack.attack_duration_timer.finished() {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(
                    delta.normalize_or_zero().truncate()
                        * attack.speed
                        * time.delta_seconds()
                        * (1. - slow_option.map_or(0., |s| s.num_stacks as f32 * 0.15)),
                );
            }

            kcc.translation = Some(attack.dir.unwrap());
            attack.attack_duration_timer.tick(time.delta());
            if anim_state != &EnemyAnimationState::Attack {
                commands
                    .entity(entity)
                    .insert(EnemyAnimationState::Attack)
                    .insert(MobIsAttacking(mob.clone()));
            }
        }

        if attack.attack_duration_timer.finished() {
            //start attack cooldown timer
            attack.dir = None;
            if anim_data.is_done_current_animation(sprite.index) {
                if follow_speed.0 > 0. {
                    commands.entity(entity).insert(FollowState {
                        target: attack.target,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    });
                }
                commands
                    .entity(entity)
                    .insert(EnemyAnimationState::Walk)
                    .remove::<LeapAttackState>()
                    .remove::<MobIsAttacking>()
                    .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
            }
        } else {
            attack.attack_startup_timer.tick(time.delta());
        }
    }
}
pub fn projectile_attack(
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(
        Entity,
        &FollowSpeed,
        &mut ProjectileAttackState,
        &EnemyAnimationState,
    )>,
    mut events: EventWriter<RangedAttackEvent>,
    time: Res<Time>,
) {
    for (entity, follow_speed, mut attack, anim_state) in attacks.iter_mut() {
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation;
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;
        if anim_state != &EnemyAnimationState::Attack {
            commands.entity(entity).insert(EnemyAnimationState::Attack);
        }
        if attack.attack_startup_timer.finished() && attack.attack_cooldown_timer.percent() == 0. {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(delta.normalize_or_zero().truncate());
            }

            events.send(RangedAttackEvent {
                projectile: attack.projectile.clone(),
                direction: attack.dir.unwrap(),
                from_enemy: Some(entity),
                is_followup_proj: false,
                mana_cost: None,
                dmg_override: None,
                pos_override: None,
            });
            commands
                .entity(entity)
                .insert(EnemyAnimationState::Walk)
                .insert(FollowState {
                    target: attack.target,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .remove::<ProjectileAttackState>()
                .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
        }

        attack.dir = None;
        attack.attack_startup_timer.tick(time.delta());
    }
}
pub fn idle(
    mut transforms: Query<&mut KinematicCharacterController>,
    mut idles: Query<(Entity, &mut IdleState), With<EnemyAnimationState>>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (entity, mut idle) in idles.iter_mut() {
        // Get the positions of the follower and target
        idle.walk_timer.tick(time.delta());
        let mut idle_transform = transforms.get_mut(entity).unwrap();
        if !idle.is_stopped {
            let s = idle.speed * PLAYER_MOVE_SPEED * time.delta_seconds();
            match idle.direction {
                FacingDirection::Left => idle_transform.translation = Some(Vec2::new(-s, 0.)),
                FacingDirection::Right => idle_transform.translation = Some(Vec2::new(s, 0.)),
                FacingDirection::Up => idle_transform.translation = Some(Vec2::new(0., s)),
                FacingDirection::Down => idle_transform.translation = Some(Vec2::new(0., -s)),
            }
        }

        if idle.walk_timer.just_finished() {
            let mut rng = rand::thread_rng();
            idle.walk_timer
                .set_duration(Duration::from_secs_f32(rng.gen_range(0.3..3.0)));
            if rng.gen_ratio(1, 2) {
                idle.is_stopped = true;
                commands.entity(entity).insert(EnemyAnimationState::Idle);
            } else {
                idle.is_stopped = false;

                let new_dir = idle.direction.get_next_rand_dir(rand::thread_rng()).clone();
                idle.direction = new_dir.clone();
                commands
                    .entity(entity)
                    .insert(new_dir)
                    .insert(EnemyAnimationState::Walk);
            }
        }
    }
}
pub fn tick_enemy_attack_cooldowns(
    mut commands: Commands,
    mut attacks: Query<(Entity, &mut EnemyAttackCooldown)>,
    time: Res<Time>,
) {
    for (e, mut attack) in attacks.iter_mut() {
        attack.0.tick(time.delta());
        if attack.0.finished() {
            commands.entity(e).remove::<EnemyAttackCooldown>();
        }
    }
}
