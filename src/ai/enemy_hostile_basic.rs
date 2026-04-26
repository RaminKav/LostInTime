use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionGroups, Group, KinematicCharacterController};

use rand::Rng;
use seldom_state::prelude::*;

use crate::Game;
use crate::{
    attributes::Attack,
    ai::pathfinding::{world_pos_to_AIPos, AIPos_to_world_pos},
    animations::enemy_sprites::{
        spawn_attack_warning_aseprite, CharacterAnimationSpriteSheetData, EnemyAnimationState,
    },
    combat::{status_effects::MobStatusEffects, HitEvent},
    enemy::{FollowSpeed, Mob, MobIsAttacking},
    inputs::FacingDirection,
    item::projectile::{Projectile, RangedAttackEvent},
    night::NightTracker,
    player::{
        melee_skills::Parried,
        skills::{Heirloom, PlayerSkills},
    },
    world::TILE_SIZE,
    PLAYER_MOVE_SPEED,
};

/// Per-enemy AI data computed once per frame to avoid redundant work in seldom_state transitions.
#[derive(Resource, Default)]
pub struct EnemyAICacheMap {
    pub map: std::collections::HashMap<bevy::prelude::Entity, EnemyAICache>,
}

#[derive(Clone, Copy, Debug)]
pub struct EnemyAICache {
    pub distance_to_player_sq: f32,
    pub attack_cooldown_active: bool,
}

/// Populates `EnemyAICacheMap` once per frame in PreUpdate so transition triggers can read from cache
/// instead of doing repeated Transform queries (reduces cost of seldom_state::machine::transition).
pub fn update_enemy_ai_cache(
    game: Res<Game>,
    transforms: Query<&Transform>,
    mobs: Query<(Entity, &Transform, Option<&EnemyAttackCooldown>), With<crate::enemy::Mob>>,
    mut cache: ResMut<EnemyAICacheMap>,
) {
    let Ok(player_t) = transforms.get(game.player) else {
        return;
    };
    let player_pos = player_t.translation.truncate();
    cache.map.clear();
    for (entity, transform, cooldown) in mobs.iter() {
        let delta = player_pos - transform.translation.truncate();
        cache.map.insert(
            entity,
            EnemyAICache {
                distance_to_player_sq: delta.length_squared(),
                attack_cooldown_active: cooldown.is_some(),
            },
        );
    }
}

/// Cached version of LineOfSight: reads from EnemyAICacheMap (one distance calc per enemy per frame).
#[derive(Clone, Copy, Reflect)]
pub struct CachedLineOfSight {
    pub range_sq: f32,
}

impl Trigger for CachedLineOfSight {
    type Param<'w, 's> = Res<'w, EnemyAICacheMap>;
    type Ok = f32;
    type Err = f32;

    fn trigger(&self, entity: Entity, cache: Self::Param<'_, '_>) -> Result<f32, f32> {
        let Some(entry) = cache.map.get(&entity) else {
            return Err(0.);
        };
        let d_sq = entry.distance_to_player_sq;
        let d = d_sq.sqrt();
        if d_sq <= self.range_sq {
            Ok(d)
        } else {
            Err(d)
        }
    }
}

/// Cached version of AttackDistance: reads from EnemyAICacheMap (avoids per-transition queries).
#[derive(Clone, Copy, Reflect)]
pub struct CachedAttackDistance {
    pub range_sq: f32,
}

impl Trigger for CachedAttackDistance {
    type Param<'w, 's> = Res<'w, EnemyAICacheMap>;
    type Ok = f32;
    type Err = f32;

    fn trigger(&self, entity: Entity, cache: Self::Param<'_, '_>) -> Result<f32, f32> {
        let Some(entry) = cache.map.get(&entity) else {
            return Err(0.);
        };
        if entry.attack_cooldown_active {
            return Err(0.);
        }
        let d_sq = entry.distance_to_player_sq;
        let d = d_sq.sqrt();
        if d_sq <= self.range_sq {
            Ok(d)
        } else {
            Err(d)
        }
    }
}

// This trigger checks if the enemy is within the the given range of the target
#[derive(Clone, Copy, Reflect)]
pub struct LineOfSight {
    pub target: Entity,
    pub range: f32,
}
/// Post-attack cooldown timer inserted on a mob after each attack and removed
/// when it elapses. Very high churn (every mob, every attack), so stored
/// `SparseSet` to keep mobs in their base archetype across attack cycles.
#[derive(Component)]
#[component(storage = "SparseSet")]
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
        if let Ok(tfxm) = transforms.get(entity) {
            let delta = transforms.get(self.target).unwrap().translation.truncate()
                - tfxm.translation.truncate();

            let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
            (distance <= self.range).then_some(distance).ok_or(distance)
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
        Ok(1.)
        // if night_tracker.is_night() {
        // } else {
        //     Err(0.)
        // }
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
        false
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
        if transforms.get(entity).unwrap().1.is_some() {
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
    pub attack_preview_entity: Option<Entity>,
}

// Entities in the `Attack` state should move towards the given entity at the given speed
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct ProjectileAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub projectile_delay_timer: Timer,
    pub dir: Option<Vec2>,
    pub projectile: Projectile,
}

/// Small Cactus: stops, plays attack anim, spawns a circle hitbox ~16px in front, then returns to follow.
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct CircleAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub dir: Option<Vec2>,
    pub spawned_hitbox: bool,
    pub hitbox_delay_timer: Timer,
}

/// Big Cactus: triple-hit leap. Performs 3 successive short lunges in the same direction.
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct MultiLeapAttackState {
    pub target: Entity,
    pub attack_startup_timer: Timer,
    pub attack_duration_timer: Timer,
    pub attack_cooldown_timer: Timer,
    /// Starts after startup; wall-clock cap for the full attack clip (`MultiLeapAttack::attack_anim_duration`).
    pub attack_clip_timer: Timer,
    pub speed: f32,
    pub dir: Option<Vec2>,
    pub hits_remaining: u8,
    pub hit_pause_timer: Timer,
    pub lunge_delay_timer: Timer,
    pub current_phase: MultiLeapPhase,
}

#[derive(Clone, Reflect, PartialEq, Debug)]
pub enum MultiLeapPhase {
    Startup,
    LungeWindup,
    Lunging,
    Pausing,
}

/// Bull: charges in a straight line past the stored player position, then decelerates.
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct BullChargeState {
    pub target: Entity,
    pub charge_target_pos: Option<Vec2>,
    pub charge_dir: Option<Vec2>,
    pub charge_speed: f32,
    pub attack_startup_timer: Timer,
    pub attack_cooldown_timer: Timer,
    pub deceleration_timer: Timer,
    pub phase: BullChargePhase,
}

#[derive(Clone, Reflect, PartialEq, Debug)]
pub enum BullChargePhase {
    WindUp,
    Charging,
    Stopping,
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
        Option<&MobStatusEffects>,
        Option<&Parried>,
        Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    night_tracker: Res<NightTracker>,
) {
    for (
        entity,
        mut follow,
        sprite,
        anim_data,
        anim_state,
        att_cooldown,
        status_option,
        parried_option,
        defiance_frozen_option,
    ) in follows.iter_mut()
    {
        // Skip movement if frozen by Death Defiance or Freeze blessing
        if defiance_frozen_option.is_some()
            || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }
        if att_cooldown.is_some() && att_cooldown.unwrap().0.percent() <= 0.5 {
            continue;
        }
        if let Some(_) = parried_option {
            continue;
        }
        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;

        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_collider_offset = Vec2::new(0., -3.);
        let follow_translation = AIPos_to_world_pos(world_pos_to_AIPos(
            follow_transform.translation.truncate() + follow_collider_offset,
        ));

        // let next_target_tile = get_next_tile_A_star(
        //     &target_translation.truncate(),
        //     &follow_translation,
        //     &mut game,
        // );

        let distance_from_target = (target_translation.truncate() - follow_translation).length();
        let is_far_away = distance_from_target > 10.5 * TILE_SIZE.x;
        // let is_on_water_tile = if let Some(tile_data) =
        //     game.get_tile_data(world_pos_to_tile_pos(follow_translation))
        // {
        //     tile_data.block_type.contains(&WorldObject::WaterTile)
        // } else {
        //     true
        // };
        //convert follower txfm to AIPos too
        let target_txfm = target_translation.truncate();
        let direct_path_to_target = (target_txfm - follow_translation).normalize_or_zero();
        // let delta_override: Option<Vec2> = if let Some(curr_path) = follow.curr_path {
        //     if curr_path == target_txfm {
        //         Some(
        //             follow
        //                 .curr_delta
        //                 .expect("delta should exist if curr_path exists"),
        //         )
        //     } else {
        //         None
        //     }
        // } else {
        //     None
        // };
        let delta = direct_path_to_target;
        let mut mover = mover.get_mut(entity).unwrap();
        mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));
        // if (night_tracker.is_night() && is_far_away) || is_on_water_tile {
        // } else {
        //     mover.filter_groups = Some(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1));
        // }

        follow.curr_path = Some(direct_path_to_target);
        follow.curr_delta = Some(delta);
        // add directional offset so they dont get stuck on walls...
        mover.translation = Some(
            (delta
                + Vec2::new(
                    if delta.y == 1. {
                        0.1
                    } else if delta.y == -1. {
                        -0.1
                    } else {
                        0.
                    },
                    if delta.x == 1. {
                        0.1
                    } else if delta.x == -1. {
                        -0.1
                    } else {
                        0.
                    },
                ))
                * follow.speed
                * PLAYER_MOVE_SPEED
                * time.delta_seconds()
                * status_option
                    .map(|s| s.movement_speed_multiplier())
                    .unwrap_or(1.0)
                * if night_tracker.is_night() { 2. } else { 1. },
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
    mut transforms: Query<&mut GlobalTransform>,
    mut attacks: Query<(
        Entity,
        &Mob,
        &mut KinematicCharacterController,
        &mut LeapAttackState,
        &FollowSpeed,
        &mut TextureAtlasSprite,
        &CharacterAnimationSpriteSheetData,
        &EnemyAnimationState,
        Option<&MobStatusEffects>,
        Option<&mut Parried>,
        Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    skills: Query<&PlayerSkills>,
    asset_server: Res<AssetServer>,
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
        status_option,
        mut parried_option,
        defiance_frozen_option,
    ) in attacks.iter_mut()
    {
        // Skip if frozen by Death Defiance or Freeze blessing
        if defiance_frozen_option.is_some()
            || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }
        // Get the positions of the attacker and target
        let target_translation = transforms.get(attack.target).unwrap().translation();
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation();

        if attack.attack_startup_timer.finished() && !attack.attack_duration_timer.finished() {
            let delta = target_translation - attack_translation;
            if attack.dir.is_none() {
                attack.dir = Some(
                    delta.normalize_or_zero().truncate()
                        * attack.speed
                        * time.delta_seconds()
                        * status_option
                            .map(|s| 1.0 - s.slow_stacks() as f32 * 0.15)
                            .unwrap_or(1.0),
                );
            }
            if let Some(ref mut parried) = parried_option {
                if !parried.kb_applied {
                    parried.kb_applied = true;
                    let mult = if skills.single().has(Heirloom::ParryKnockback) {
                        1.
                    } else {
                        0.5
                    };
                    attack.dir = Some(-attack.dir.unwrap() * mult);
                }
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
            if anim_data.is_done_current_animation(sprite.index) || parried_option.is_some() {
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
            if attack.attack_startup_timer.percent() == 0. {
                spawn_attack_warning_aseprite(
                    &mut commands,
                    &asset_server,
                    Vec3::new(0., 12., 10.),
                    entity,
                    attack.attack_startup_timer.duration().as_secs_f32() + 0.01,
                );
            }
            attack.attack_startup_timer.tick(time.delta());
        }
    }
}
pub fn projectile_attack(
    mut commands: Commands,
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(
        Entity,
        &Attack,
        &FollowSpeed,
        &mut ProjectileAttackState,
        &EnemyAnimationState,
        Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
        Option<&MobStatusEffects>,
    )>,
    mut events: EventWriter<RangedAttackEvent>,
    time: Res<Time>,
) {
    for (
        entity,
        mob_attack,
        follow_speed,
        mut attack,
        anim_state,
        defiance_frozen_option,
        status_option,
    ) in attacks.iter_mut()
    {
        // Skip if frozen by Death Defiance or Freeze blessing
        if defiance_frozen_option.is_some()
            || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }
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
                from_entity: Some(entity),
                from_enemy: true,
                is_followup_proj: false,
                mana_cost: None,
                dmg_override: Some(mob_attack.0),
                pos_override: None,
                spawn_delay: 0.1,
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
    mut idles: Query<
        (
            Entity,
            &mut IdleState,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&MobStatusEffects>,
        ),
        With<EnemyAnimationState>,
    >,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (entity, mut idle, defiance_frozen_option, status_option) in idles.iter_mut() {
        // Skip if frozen by Death Defiance or Freeze blessing
        if defiance_frozen_option.is_some()
            || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }
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
