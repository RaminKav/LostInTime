use crate::aseprite_assets::{
    BigCactusAse, BullAse, Crow, LizardAse, SmallCactusAse, VoidCrawlerAse, VoidWormAse,
};
use crate::aseprite_helpers::{
    ase_animation, aseprite_bundle, collect_finished, is_paused, pause, play_loop, play_once, start,
};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{AnimationEvents, AnimationState, AseAnimation, Aseprite};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group, KinematicCharacterController};
use seldom_state::prelude::{always, IntoTrigger, StateMachine};

use crate::{
    ai::{
        cached_attack_distance, cached_line_of_sight, hurt_by_player, night_time_aggro,
        BullChargePhase, BullChargeState, CircleAttackState, EnemyAttackCooldown, FollowState,
        IdleState, LeapAttackState, MultiLeapAttackState, MultiLeapPhase, ProjectileAttackState,
    },
    animations::{enemy_sprites::spawn_attack_warning_aseprite, HitAnimationTracker},
    attributes::Attack,
    combat::{combat_helpers::spawn_temp_collider, status_effects::MobStatusEffects},
    enemy::{
        void_worm::VoidWormLaserState, BullChargeAttack, CircleAttack, CombatAlignment,
        FollowSpeed, LaserAttack, LeapAttack, Mob, MobIsAttacking, MultiLeapAttack,
        ProjectileAttack,
    },
    inputs::FacingDirection,
    item::projectile::{EnemyProjectile, Projectile, RangedAttackEvent},
    night::NightTracker,
    player::{
        melee_skills::Parried,
        skills::{Heirloom, PlayerSkills},
    },
    world::dungeon::Dungeon,
    GameParam, PLAYER_MOVE_SPEED,
};

// Each "basic" aseprite enemy (walk + lunge) must be declared here so the macro runs at compile time.

/// Fixed animation tag names for the shared aseprite basic enemy behavior.
/// Aseprite files must use these exact tag names: WalkUp, WalkDown, WalkSide, AttackUp, AttackDown, AttackSide.
const WALK_UP: &str = "WalkUp";
const WALK_DOWN: &str = "WalkDown";
const WALK_SIDE: &str = "WalkSide";
const ATTACK_UP: &str = "AttackUp";
const ATTACK_DOWN: &str = "AttackDown";
const ATTACK_SIDE: &str = "AttackSide";
const ATTACK_STOP_UP: &str = "AttackStopUp";
const ATTACK_STOP_DOWN: &str = "AttackStopDown";
const ATTACK_STOP_SIDE: &str = "AttackStopSide";
const HIT_UP: &str = "HitUp";
const HIT_DOWN: &str = "HitDown";
const HIT_SIDE: &str = "HitSide";

/// Marker for enemies that use the shared aseprite walk + lunge behavior.
/// Setup is done in code via `get_aseprite_basic_config` (each such mob needs an `aseprite!` macro).
#[derive(Component, Default, Debug)]
pub struct AsepriteBasicEnemy;

/// Tracks the currently active aseprite animation tag to avoid redundantly
/// resetting the same animation every frame.
#[derive(Component, Default, Debug)]
pub struct CurrentAsepriteTag(pub String);

/// Returns (aseprite path, initial walk tag) for mobs that use the shared aseprite basic behavior.
/// Add a new match arm and an `` at the top of this file for each new enemy.
fn get_aseprite_basic_config(mob: &Mob) -> Option<(&'static str, &'static str)> {
    match mob {
        Mob::Crow => Some((Crow::PATH, WALK_DOWN)),
        Mob::SmallCactus => Some((SmallCactusAse::PATH, WALK_DOWN)),
        Mob::BigCactus => Some((BigCactusAse::PATH, WALK_DOWN)),
        Mob::Bull => Some((BullAse::PATH, WALK_DOWN)),
        Mob::Lizard => Some((LizardAse::PATH, WALK_DOWN)),
        Mob::VoidCrawler => Some((VoidCrawlerAse::PATH, WALK_DOWN)),
        Mob::VoidWorm => Some((VoidWormAse::PATH, WALK_DOWN)),
        _ => None,
    }
}

/// Returns true if this mob uses the shared aseprite basic behavior (so generic state machine should not be applied).
pub fn is_aseprite_basic_mob(mob: &Mob) -> bool {
    get_aseprite_basic_config(mob).is_some()
}

/// Initializes aseprite basic enemies: when a Mob in our registry is spawned, insert native aseprite components and marker.
pub fn aseprite_enemy_setup(
    mut commands: Commands,
    new_mobs: Query<(Entity, &Mob, &Transform), Added<Mob>>,
    asset_server: Res<AssetServer>,
) {
    for (entity, mob, transform) in new_mobs.iter() {
        let Some((path, initial_tag)) = get_aseprite_basic_config(mob) else {
            continue;
        };

        commands
            .entity(entity)
            .insert(aseprite_bundle(
                asset_server.load(path),
                initial_tag,
                *transform,
                Visibility::Inherited,
                false,
            ))
            .insert(CurrentAsepriteTag(initial_tag.to_string()))
            .insert(AsepriteBasicEnemy);
    }
}

fn set_animation_tag(anim: &mut AseAnimation, current_tag: &mut CurrentAsepriteTag, new_tag: &str) {
    if current_tag.0 != new_tag {
        // Preserve aseprite handle — never rebuild via `from()` (wipes Handle::default).
        play_loop(anim, new_tag);
        current_tag.0 = new_tag.to_string();
    }
}

/// True if the tag is one of the directional walk tags (mob is in follow/idle, not attacking).
fn is_walk_tag(tag: &str) -> bool {
    matches!(tag, WALK_UP | WALK_DOWN | WALK_SIDE)
}

/// Maps the mob's facing direction to the matching hit-react tag.
fn facing_to_hit_tag(facing: &FacingDirection) -> &'static str {
    match facing {
        FacingDirection::Up => HIT_UP,
        FacingDirection::Down => HIT_DOWN,
        FacingDirection::Left | FacingDirection::Right => HIT_SIDE,
    }
}

/// Maps an attack tag to the matching walk tag so post-attack we keep the same direction.
fn attack_tag_to_walk_tag(attack_tag: &str) -> &'static str {
    match attack_tag {
        ATTACK_UP | ATTACK_STOP_UP => WALK_UP,
        ATTACK_DOWN | ATTACK_STOP_DOWN => WALK_DOWN,
        ATTACK_SIDE | ATTACK_STOP_SIDE => WALK_SIDE,
        _ => WALK_DOWN,
    }
}

/// Sets up state machines for generic aseprite enemies (those with `AsepriteBasicEnemy`).
pub fn handle_new_aseprite_enemy_state_machine(
    mut commands: Commands,
    game: GameParam,
    spawn_events: Query<
        (
            Entity,
            &CombatAlignment,
            &FollowSpeed,
            Option<&LeapAttack>,
            Option<&ProjectileAttack>,
            Option<&CircleAttack>,
            Option<&MultiLeapAttack>,
            Option<&BullChargeAttack>,
            Option<&LaserAttack>,
        ),
        Added<AsepriteBasicEnemy>,
    >,
    dungeon_check: Query<&Dungeon>,
) {
    for (
        e,
        alignment,
        follow_speed,
        leap_attack_option,
        proj_attack_option,
        circle_attack_option,
        multi_leap_option,
        bull_charge_option,
        laser_attack_option,
    ) in spawn_events.iter()
    {
        let mut alignment = alignment.clone();
        commands
            .entity(e)
            .insert(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1));
        if dungeon_check.single().is_ok() {
            alignment = CombatAlignment::Hostile;
        }

        let mut state_machine = StateMachine::default().set_trans_logging(false);

        match alignment {
            CombatAlignment::Neutral => {
                state_machine = state_machine
                    .trans::<IdleState, _>(
                        hurt_by_player,
                        FollowState {
                            target: game.game.player,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed.0,
                        },
                    )
                    .trans::<FollowState, _>(
                        cached_line_of_sight(130. * 130.).not(),
                        IdleState {
                            walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                            speed: 0.5,
                            is_stopped: false,
                        },
                    );
            }
            CombatAlignment::Hostile => {
                // Hostiles chase immediately — no aggro/LoS distance gate.
                state_machine = state_machine.trans::<IdleState, _>(
                    always,
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
            }
            CombatAlignment::Passive => {}
        }

        if let Some(leap_attack) = leap_attack_option {
            // Complete the lunge once entered — see `handle_new_mob_state_machine`.
            state_machine = state_machine.trans::<FollowState, _>(
                cached_attack_distance(
                    leap_attack.activation_distance * leap_attack.activation_distance,
                ),
                LeapAttackState {
                    target: game.game.player,
                    attack_startup_timer: Timer::from_seconds(leap_attack.startup, TimerMode::Once),
                    attack_duration_timer: Timer::from_seconds(
                        leap_attack.duration,
                        TimerMode::Once,
                    ),
                    attack_cooldown_timer: Timer::from_seconds(
                        leap_attack.cooldown,
                        TimerMode::Once,
                    ),
                    dir: None,
                    speed: leap_attack.speed,
                    attack_preview_entity: None,
                },
            );
        }

        if let Some(proj_attack) = proj_attack_option {
            state_machine = state_machine
                .trans::<FollowState, _>(
                    cached_attack_distance(
                        proj_attack.activation_distance * proj_attack.activation_distance,
                    ),
                    ProjectileAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(
                            proj_attack.attack_startup,
                            TimerMode::Once,
                        ),
                        attack_cooldown_timer: Timer::from_seconds(
                            proj_attack.cooldown,
                            TimerMode::Once,
                        ),
                        projectile_delay_timer: Timer::from_seconds(
                            proj_attack.projectile_delay,
                            TimerMode::Once,
                        ),
                        dir: None,
                        projectile: proj_attack.projectile.clone(),
                    },
                )
                .trans::<ProjectileAttackState, _>(
                    cached_attack_distance((proj_attack.activation_distance + 30.).powi(2)).not(),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
        }

        if let Some(circle_attack) = circle_attack_option {
            state_machine = state_machine
                .trans::<FollowState, _>(
                    cached_attack_distance(
                        circle_attack.activation_distance * circle_attack.activation_distance,
                    ),
                    CircleAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(
                            circle_attack.startup,
                            TimerMode::Once,
                        ),
                        attack_cooldown_timer: Timer::from_seconds(
                            circle_attack.cooldown,
                            TimerMode::Once,
                        ),
                        dir: None,
                        spawned_hitbox: false,
                        hitbox_delay_timer: Timer::from_seconds(
                            circle_attack.hitbox_delay,
                            TimerMode::Once,
                        ),
                    },
                )
                .trans::<CircleAttackState, _>(
                    cached_attack_distance((circle_attack.activation_distance + 32.).powi(2)).not(),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
        }

        if let Some(multi_leap) = multi_leap_option {
            state_machine = state_machine
                .trans::<FollowState, _>(
                    cached_attack_distance(
                        multi_leap.activation_distance * multi_leap.activation_distance,
                    ),
                    MultiLeapAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(
                            multi_leap.startup,
                            TimerMode::Once,
                        ),
                        attack_duration_timer: Timer::from_seconds(
                            multi_leap.duration_per_hit,
                            TimerMode::Once,
                        ),
                        attack_cooldown_timer: Timer::from_seconds(
                            multi_leap.cooldown,
                            TimerMode::Once,
                        ),
                        attack_clip_timer: Timer::from_seconds(
                            multi_leap.attack_anim_duration,
                            TimerMode::Once,
                        ),
                        speed: multi_leap.speed,
                        dir: None,
                        hits_remaining: multi_leap.num_hits,
                        hit_pause_timer: Timer::from_seconds(
                            multi_leap.pause_between_hits,
                            TimerMode::Once,
                        ),
                        lunge_delay_timer: Timer::from_seconds(
                            multi_leap.lunge_delay,
                            TimerMode::Once,
                        ),
                        current_phase: MultiLeapPhase::Startup,
                    },
                )
                .trans::<MultiLeapAttackState, _>(
                    cached_attack_distance((multi_leap.activation_distance + 32.).powi(2)).not(),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
        }

        if let Some(bull_charge) = bull_charge_option {
            state_machine = state_machine.trans::<FollowState, _>(
                cached_attack_distance(
                    bull_charge.activation_distance * bull_charge.activation_distance,
                ),
                BullChargeState {
                    target: game.game.player,
                    charge_target_pos: None,
                    charge_dir: None,
                    charge_speed: bull_charge.charge_speed,
                    attack_startup_timer: Timer::from_seconds(bull_charge.startup, TimerMode::Once),
                    attack_cooldown_timer: Timer::from_seconds(
                        bull_charge.cooldown,
                        TimerMode::Once,
                    ),
                    deceleration_timer: Timer::from_seconds(
                        bull_charge.stop_duration,
                        TimerMode::Once,
                    ),
                    phase: BullChargePhase::WindUp,
                },
            );
        }

        if let Some(laser_attack) = laser_attack_option {
            // Pick a fixed random stop distance for this worm's lifetime, between
            // min and max. The worm walks until within this distance, then fires.
            let stop_distance = {
                use rand::Rng;
                rand::thread_rng()
                    .gen_range(laser_attack.min_stop_distance..=laser_attack.max_stop_distance)
            };
            state_machine = state_machine.trans::<FollowState, _>(
                cached_attack_distance(stop_distance * stop_distance),
                VoidWormLaserState::new(
                    game.game.player,
                    laser_attack.laser_duration,
                    laser_attack.walk_duration,
                ),
            );
            // Note: the laser->follow transition is handled imperatively in
            // `void_worm::void_worm_laser_attack` (insert FollowState + cooldown).
        }

        if alignment != CombatAlignment::Passive {
            state_machine = state_machine.trans::<IdleState, _>(
                night_time_aggro,
                FollowState {
                    target: game.game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                },
            );
        }

        commands.entity(e).insert(state_machine);
    }
}

/// Shared follow behavior for aseprite basic enemies (WalkUp / WalkDown / WalkSide).
pub fn aseprite_follow(
    mut transforms: Query<&mut Transform>,
    mut mover: Query<&mut KinematicCharacterController>,
    mut follows: Query<
        (
            Entity,
            &mut FollowState,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&MobStatusEffects>,
            Option<&Parried>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            &HitAnimationTracker,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    time: Res<Time>,
    night_tracker: Res<NightTracker>,
    grid: Res<crate::ai::steering::EnemySpatialGrid>,
    game: Res<crate::Game>,
) {
    for (
        entity,
        mut follow,
        mut anim,
        mut current_tag,
        status_option,
        parried_option,
        defiance_frozen_option,
        hit,
    ) in follows.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }
        if parried_option.is_some() {
            continue;
        }
        // Hit-react owns the animation/movement during its window (see `aseprite_hit_react`).
        if hit.is_active {
            continue;
        }

        let target = if transforms.get(follow.target).is_ok() {
            follow.target
        } else {
            game.player
        };
        follow.target = target;
        let Ok(target_translation) = transforms.get(target) else {
            continue;
        };
        let enemy_translation = transforms.get(entity).unwrap().translation;
        let to_player = target_translation.translation.truncate() - enemy_translation.truncate();
        let dist_to_player = to_player.length();
        let seek_dir = to_player.normalize_or_zero();
        let steered = crate::ai::steering::steer_chase(
            &grid,
            entity,
            enemy_translation.truncate(),
            seek_dir,
            dist_to_player,
        );
        let delta = match follow.curr_delta {
            Some(prev) => prev
                .lerp(steered, crate::ai::steering::STEERING_SMOOTHING)
                .normalize_or_zero(),
            None => steered,
        };

        let mut mover = mover.get_mut(entity).unwrap();
        mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));

        follow.curr_path = Some(delta);
        follow.curr_delta = Some(delta);

        mover.translation = Some(
            delta
                * follow.speed
                * PLAYER_MOVE_SPEED
                * time.delta_secs()
                * status_option
                    .map(|s| s.movement_speed_multiplier())
                    .unwrap_or(1.0)
                * if night_tracker.is_night() { 2. } else { 1. },
        );

        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.try_insert(FacingDirection::from_translation(delta));
        }

        // Fixed tag names: WalkUp, WalkDown, WalkSide
        let to_target = target_translation.translation.truncate() - enemy_translation.truncate();
        if to_target.length_squared() >= 4.0 {
            let abs_x = to_target.x.abs();
            let abs_y = to_target.y.abs();

            let desired_tag = if abs_x > abs_y * 1.1 {
                WALK_SIDE
            } else if to_target.y > 0. {
                WALK_UP
            } else {
                WALK_DOWN
            };

            set_animation_tag(&mut anim, &mut current_tag, desired_tag);

            let mut transform = transforms.get_mut(entity).unwrap();
            if abs_x > 1.0 {
                if to_target.x < 0. && transform.scale.x > 0. {
                    transform.scale.x = -1.0;
                } else if to_target.x > 0. && transform.scale.x < 0. {
                    transform.scale.x = 1.0;
                }
            }
        }
    }
}

/// Shared idle behavior for aseprite basic enemies (fixed tags WalkUp / WalkDown / WalkSide).
pub fn aseprite_idle(
    mut transforms: Query<&mut KinematicCharacterController>,
    mut idles: Query<
        (
            Entity,
            &mut IdleState,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&MobStatusEffects>,
            &HitAnimationTracker,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (entity, mut idle, mut anim, mut current_tag, defiance_frozen_option, status_option, hit) in
        idles.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }
        // Hit-react owns the animation during its window (see `aseprite_hit_react`).
        if hit.is_active {
            continue;
        }

        idle.walk_timer.tick(time.delta());
        let mut idle_kcc = transforms.get_mut(entity).unwrap();
        if !idle.is_stopped {
            let s = idle.speed * PLAYER_MOVE_SPEED * time.delta_secs();
            match idle.direction {
                FacingDirection::Left => idle_kcc.translation = Some(Vec2::new(-s, 0.)),
                FacingDirection::Right => idle_kcc.translation = Some(Vec2::new(s, 0.)),
                FacingDirection::Up => idle_kcc.translation = Some(Vec2::new(0., s)),
                FacingDirection::Down => idle_kcc.translation = Some(Vec2::new(0., -s)),
            }
        }

        if idle.walk_timer.just_finished() {
            use rand::Rng;
            use std::time::Duration;
            let mut rng = rand::thread_rng();

            idle.walk_timer
                .set_duration(Duration::from_secs_f32(rng.gen_range(0.3..3.0)));
            if rng.gen_ratio(1, 2) {
                idle.is_stopped = true;
                set_animation_tag(&mut anim, &mut current_tag, WALK_DOWN);
                if let Ok(mut entity_commands) = commands.get_entity(entity) {
                    entity_commands.try_insert(FacingDirection::Down);
                }
            } else {
                idle.is_stopped = false;
                let new_dir = idle.direction.get_next_rand_dir(rand::thread_rng()).clone();
                idle.direction = new_dir.clone();

                let tag = match new_dir {
                    FacingDirection::Left | FacingDirection::Right => WALK_SIDE,
                    FacingDirection::Up => WALK_UP,
                    FacingDirection::Down => WALK_DOWN,
                };
                set_animation_tag(&mut anim, &mut current_tag, tag);
                if let Ok(mut entity_commands) = commands.get_entity(entity) {
                    entity_commands.try_insert(new_dir);
                }
            }
        }
    }
}

/// Shared hit-react animation for aseprite basic enemies (fixed tags HitUp / HitDown / HitSide).
///
/// Plays for the duration of the shared [`HitAnimationTracker`] window (set on hit in combat).
/// Only engages while the mob is in a walk tag (i.e. following/idling), so it never interrupts an
/// in-progress attack — matching the legacy spritesheet behavior. The knockback itself is applied
/// by `animate_hit`; here we only swap the animation tag. When the window ends, `aseprite_follow` /
/// `aseprite_idle` resume and restore the walk tag automatically.
pub fn aseprite_hit_react(
    mut hits: Query<
        (
            &HitAnimationTracker,
            &FacingDirection,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
        ),
        With<AsepriteBasicEnemy>,
    >,
) {
    for (hit, facing, mut anim, mut current_tag) in hits.iter_mut() {
        if !hit.is_active {
            continue;
        }
        // Only start the hit clip from a walk tag; if attacking, leave the attack animation alone.
        if !is_walk_tag(&current_tag.0) {
            continue;
        }
        let hit_tag = facing_to_hit_tag(facing);
        set_animation_tag(&mut anim, &mut current_tag, hit_tag);
    }
}

/// Shared leap attack behavior for aseprite basic enemies (fixed tags AttackUp / AttackDown / AttackSide).
pub fn aseprite_leap_attack(
    mut transforms: Query<&mut GlobalTransform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &mut KinematicCharacterController,
            &mut LeapAttackState,
            &FollowSpeed,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&MobStatusEffects>,
            Option<&mut Parried>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    time: Res<Time>,
    skills: Query<&PlayerSkills>,
    asset_server: Res<AssetServer>,
    mut finished_events: MessageReader<AnimationEvents>,
) {
    let finished = collect_finished(&mut finished_events);
    for (
        entity,
        mob,
        mut kcc,
        mut attack,
        follow_speed,
        mut anim,
        mut current_tag,
        status_option,
        mut parried_option,
        defiance_frozen_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }

        let target_translation = transforms.get(attack.target).unwrap().translation();
        let attack_translation = transforms.get_mut(entity).unwrap().translation();

        if attack.attack_startup_timer.is_finished() && !attack.attack_duration_timer.is_finished()
        {
            let delta = target_translation - attack_translation;
            let delta_xy = delta.truncate();
            if attack.dir.is_none() {
                attack.dir = Some(
                    delta_xy.normalize_or_zero()
                        * attack.speed
                        * time.delta_secs()
                        * status_option
                            .map(|s| 1.0 - s.slow_stacks() as f32 * 0.15)
                            .unwrap_or(1.0),
                );
            }
            if let Some(ref mut parried) = parried_option {
                if !parried.kb_applied {
                    parried.kb_applied = true;
                    let mult = if skills
                        .single()
                        .map(|skills| skills.has(Heirloom::ParryKnockback))
                        .unwrap_or(false)
                    {
                        1.
                    } else {
                        0.5
                    };
                    attack.dir = Some(-attack.dir.unwrap() * mult);
                }
            }

            kcc.translation = Some(attack.dir.unwrap());
            attack.attack_duration_timer.tick(time.delta());

            // Lock the attack-animation direction to the frozen leap direction so the
            // tag stays constant for the whole lunge. Recomputing it from the live
            // delta each frame makes the tag flip as the mob nears/passes the player,
            // which resets the animation to frame 0 and looks like it plays twice.
            let attack_tag = direction_to_attack_tag(attack.dir.unwrap_or(delta_xy));
            set_animation_tag(&mut anim, &mut current_tag, attack_tag);
            commands.entity(entity).insert(MobIsAttacking(mob.clone()));
        }

        if attack.attack_duration_timer.is_finished() {
            attack.dir = None;
            if finished.contains(&entity) || parried_option.is_some() {
                if follow_speed.0 > 0. {
                    commands.entity(entity).insert(FollowState {
                        target: attack.target,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    });
                }
                let walk_tag = attack_tag_to_walk_tag(&current_tag.0);
                set_animation_tag(&mut anim, &mut current_tag, walk_tag);
                commands
                    .entity(entity)
                    .remove::<LeapAttackState>()
                    .remove::<MobIsAttacking>()
                    .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
            }
        } else {
            if attack.attack_startup_timer.fraction() == 0. {
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

/// Tracks whether the projectile has already been fired during this attack cycle,
/// so we only fire once but let the animation play to completion. Stored
/// `SparseSet` because it toggles on and off every ranged-attack cycle on
/// every aseprite mob.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct AsepriteProjectileFired;

/// Projectile attack for aseprite basic enemies. Uses same attack tags (AttackUp / AttackDown / AttackSide).
/// Flow: startup timer (with attack warning) -> switch to attack anim -> fire when projectile_delay elapses -> wait for `just_finished()` -> back to follow.
pub fn aseprite_projectile_attack(
    transforms: Query<&GlobalTransform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &Attack,
            &mut ProjectileAttackState,
            &FollowSpeed,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&AsepriteProjectileFired>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&MobStatusEffects>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    mut events: MessageWriter<RangedAttackEvent>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
    mut finished_events: MessageReader<AnimationEvents>,
) {
    const FEATHER_SPREAD_RAD: f32 = 0.15;
    let finished = collect_finished(&mut finished_events);

    for (
        entity,
        mob,
        enemy_attack,
        mut attack,
        follow_speed,
        mut anim,
        mut current_tag,
        fired_option,
        defiance_frozen_option,
        status_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }

        let target_translation = transforms.get(attack.target).unwrap().translation();
        let attack_translation = transforms.get(entity).unwrap().translation();
        let delta =
            (target_translation.truncate() - attack_translation.truncate()).normalize_or_zero();

        // Phase 1: startup timer (pre-animation wind-up, enemy stays in walk anim; show attack warning)
        if !attack.attack_startup_timer.is_finished() {
            if attack.attack_startup_timer.fraction() == 0. {
                spawn_attack_warning_aseprite(
                    &mut commands,
                    &asset_server,
                    Vec3::new(0., 12., 10.),
                    entity,
                    attack.attack_startup_timer.duration().as_secs_f32() + 0.01,
                );
            }
            attack.attack_startup_timer.tick(time.delta());
            continue;
        }

        // Phase 2: play attack animation
        let attack_tag = if delta.x.abs() > delta.y.abs() * 1.1 {
            ATTACK_SIDE
        } else if delta.y > 0. {
            ATTACK_UP
        } else {
            ATTACK_DOWN
        };
        set_animation_tag(&mut anim, &mut current_tag, attack_tag);

        if attack.dir.is_none() {
            attack.dir = Some(delta);
        }

        // Phase 3: tick projectile delay, fire once the delay elapses
        attack.projectile_delay_timer.tick(time.delta());
        if attack.projectile_delay_timer.is_finished() && fired_option.is_none() {
            let dir = attack.dir.unwrap();

            let num_projectiles = if *mob == Mob::Crow { 2 } else { 1 };
            let base_angle = dir.y.atan2(dir.x);
            for i in 0..num_projectiles {
                let proj_dir = if num_projectiles == 2 {
                    let angle = if i == 0 {
                        base_angle - FEATHER_SPREAD_RAD
                    } else {
                        base_angle + FEATHER_SPREAD_RAD
                    };
                    Vec2::new(angle.cos(), angle.sin())
                } else {
                    dir
                };
                events.write(RangedAttackEvent {
                    projectile: attack.projectile.clone(),
                    direction: proj_dir,
                    from_entity: Some(entity),
                    from_enemy: true,
                    is_followup_proj: false,
                    mana_cost: None,
                    mana_cost_heirloom: None,
                    dmg_override: Some(enemy_attack.0),
                    pos_override: None,
                    spawn_delay: 0.,
                });
            }
            commands.entity(entity).insert(AsepriteProjectileFired);
        }

        // Phase 4: wait for attack animation to finish, then transition back
        if finished.contains(&entity) {
            let walk_tag = attack_tag_to_walk_tag(&current_tag.0);
            commands
                .entity(entity)
                .remove::<AsepriteProjectileFired>()
                .insert(FollowState {
                    target: attack.target,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .remove::<ProjectileAttackState>()
                .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
            set_animation_tag(&mut anim, &mut current_tag, walk_tag);
        }
    }
}

fn direction_to_attack_tag(delta: Vec2) -> &'static str {
    if delta.x.abs() > delta.y.abs() * 1.1 {
        ATTACK_SIDE
    } else if delta.y > 0. {
        ATTACK_UP
    } else {
        ATTACK_DOWN
    }
}

fn direction_to_attack_stop_tag(delta: Vec2) -> &'static str {
    if delta.x.abs() > delta.y.abs() * 1.1 {
        ATTACK_STOP_SIDE
    } else if delta.y > 0. {
        ATTACK_STOP_UP
    } else {
        ATTACK_STOP_DOWN
    }
}

/// Horizontal flip for side-dominant direction — must match [`aseprite_follow`] so charge/stop
/// match walk facing (bull has no [`FollowState`] during [`BullChargeState`], so follow does not run).
fn apply_horizontal_sprite_flip_for_dir(transform: &mut Transform, dir: Vec2) {
    let abs_x = dir.x.abs();
    let abs_y = dir.y.abs();
    if abs_x > abs_y * 1.1 && abs_x > f32::EPSILON {
        if dir.x < 0. && transform.scale.x > 0. {
            transform.scale.x = -1.0;
        } else if dir.x > 0. && transform.scale.x < 0. {
            transform.scale.x = 1.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Small Cactus: circle attack (spawn hitbox in front of self)
// ---------------------------------------------------------------------------
pub fn aseprite_circle_attack(
    transforms: Query<&GlobalTransform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &Attack,
            &mut CircleAttackState,
            &FollowSpeed,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&MobStatusEffects>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    circle_configs: Query<&CircleAttack>,
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut finished_events: MessageReader<AnimationEvents>,
) {
    let finished = collect_finished(&mut finished_events);
    for (
        entity,
        mob,
        enemy_attack,
        mut attack,
        follow_speed,
        mut anim,
        mut current_tag,
        defiance_frozen_option,
        status_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }

        let target_pos = transforms.get(attack.target).unwrap().translation();
        let my_pos = transforms.get(entity).unwrap().translation();
        let delta = (target_pos.truncate() - my_pos.truncate()).normalize_or_zero();

        // Phase 1: startup (show warning)
        if !attack.attack_startup_timer.is_finished() {
            if attack.attack_startup_timer.fraction() == 0. {
                spawn_attack_warning_aseprite(
                    &mut commands,
                    &asset_server,
                    Vec3::new(0., 12., 10.),
                    entity,
                    attack.attack_startup_timer.duration().as_secs_f32() + 0.01,
                );
            }
            attack.attack_startup_timer.tick(time.delta());
            continue;
        }

        // Phase 2: play attack animation
        let attack_tag = direction_to_attack_tag(delta);
        set_animation_tag(&mut anim, &mut current_tag, attack_tag);

        if attack.dir.is_none() {
            attack.dir = Some(delta);
        }

        commands.entity(entity).insert(MobIsAttacking(mob.clone()));

        // Phase 3: spawn circle hitbox after delay
        attack.hitbox_delay_timer.tick(time.delta());
        if attack.hitbox_delay_timer.is_finished() && !attack.spawned_hitbox {
            attack.spawned_hitbox = true;

            let config = circle_configs.get(entity).ok();
            let offset_dist = config.map_or(16., |c| c.hitbox_offset);
            let radius = config.map_or(10., |c| c.hitbox_radius);

            let dir = attack.dir.unwrap();
            let hitbox_pos = my_pos + (dir * offset_dist).extend(0.);

            let hitbox = spawn_temp_collider(
                &mut commands,
                Transform::from_translation(hitbox_pos),
                0.3,
                enemy_attack.0,
                Collider::ball(radius),
                Projectile::CactusSlam,
            );
            commands.entity(hitbox).insert(EnemyProjectile {
                entity,
                mob: mob.clone(),
            });
        }

        // Phase 4: wait for anim to finish
        if finished.contains(&entity) {
            let walk_tag = attack_tag_to_walk_tag(&current_tag.0);
            commands
                .entity(entity)
                .insert(FollowState {
                    target: attack.target,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .remove::<CircleAttackState>()
                .remove::<MobIsAttacking>()
                .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
            set_animation_tag(&mut anim, &mut current_tag, walk_tag);
        }
    }
}

// ---------------------------------------------------------------------------
// Big Cactus: multi-hit leap (3 successive lunges)
// ---------------------------------------------------------------------------
pub fn aseprite_multi_leap_attack(
    global_transforms: Query<&GlobalTransform>,
    mut local_transforms: Query<&mut Transform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &mut KinematicCharacterController,
            &mut MultiLeapAttackState,
            &FollowSpeed,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&MobStatusEffects>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
) {
    for (
        entity,
        mob,
        mut kcc,
        mut attack,
        follow_speed,
        mut anim,
        mut current_tag,
        status_option,
        defiance_frozen_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }

        let target_pos = global_transforms.get(attack.target).unwrap().translation();
        let my_pos = global_transforms.get(entity).unwrap().translation();

        let delta_xy = (target_pos.truncate() - my_pos.truncate()).normalize_or_zero();

        // Wall-clock cap for the full attack clip (after startup). Avoids aseprite forward-tags looping past one play.
        if attack.current_phase != MultiLeapPhase::Startup {
            attack.attack_clip_timer.tick(time.delta());
        }

        match attack.current_phase {
            MultiLeapPhase::Startup => {
                // Show warning icon, wait for startup timer — no movement, walk anim continues.
                if attack.attack_startup_timer.fraction() == 0. {
                    // Switch to attack anim facing the player, wait for lunge_delay before moving.
                    let attack_tag = direction_to_attack_tag(delta_xy);
                    set_animation_tag(&mut anim, &mut current_tag, attack_tag);
                    if let Ok(mut tf) = local_transforms.get_mut(entity) {
                        apply_horizontal_sprite_flip_for_dir(&mut tf, delta_xy);
                    }
                    spawn_attack_warning_aseprite(
                        &mut commands,
                        &asset_server,
                        Vec3::new(0., 12., 10.),
                        entity,
                        attack.attack_startup_timer.duration().as_secs_f32() + 0.01,
                    );
                }
                attack.attack_startup_timer.tick(time.delta());
                if attack.attack_startup_timer.is_finished() {
                    attack.current_phase = MultiLeapPhase::LungeWindup;
                    attack.lunge_delay_timer.reset();
                }
            }
            MultiLeapPhase::LungeWindup => {
                // Switch to attack anim facing the player, wait for lunge_delay before moving.
                let attack_tag = direction_to_attack_tag(delta_xy);
                set_animation_tag(&mut anim, &mut current_tag, attack_tag);
                if let Ok(mut tf) = local_transforms.get_mut(entity) {
                    apply_horizontal_sprite_flip_for_dir(&mut tf, delta_xy);
                }
                commands.entity(entity).insert(MobIsAttacking(mob.clone()));

                attack.lunge_delay_timer.tick(time.delta());
                if attack.lunge_delay_timer.is_finished() {
                    attack.current_phase = MultiLeapPhase::Lunging;
                    attack.attack_duration_timer.reset();
                    attack.dir = None;
                }
            }
            MultiLeapPhase::Lunging => {
                // Move forward for duration_per_hit.
                if attack.dir.is_none() {
                    attack.dir = Some(
                        delta_xy
                            * attack.speed
                            * time.delta_secs()
                            * status_option
                                .map(|s| 1.0 - s.slow_stacks() as f32 * 0.15)
                                .unwrap_or(1.0),
                    );
                }

                kcc.translation = Some(attack.dir.unwrap());
                attack.attack_duration_timer.tick(time.delta());

                let attack_tag = direction_to_attack_tag(attack.dir.unwrap_or(delta_xy));
                set_animation_tag(&mut anim, &mut current_tag, attack_tag);
                if let Ok(mut tf) = local_transforms.get_mut(entity) {
                    apply_horizontal_sprite_flip_for_dir(&mut tf, attack.dir.unwrap_or(delta_xy));
                }

                if attack.attack_duration_timer.is_finished() {
                    attack.hits_remaining = attack.hits_remaining.saturating_sub(1);
                    if attack.hits_remaining > 0 {
                        attack.current_phase = MultiLeapPhase::Pausing;
                        attack.hit_pause_timer.reset();
                        attack.dir = None;
                    }
                }
            }
            MultiLeapPhase::Pausing => {
                // Brief gap between hits, then start the next lunge windup.
                attack.hit_pause_timer.tick(time.delta());
                if attack.hit_pause_timer.is_finished() {
                    attack.current_phase = MultiLeapPhase::LungeWindup;
                    attack.lunge_delay_timer.reset();
                    attack.dir = None;
                }
            }
        }

        let hits_complete = attack.hits_remaining == 0
            && attack.current_phase == MultiLeapPhase::Lunging
            && attack.attack_duration_timer.is_finished();

        if hits_complete || attack.attack_clip_timer.is_finished() {
            attack.dir = None;
            kcc.translation = None;
            let walk_tag = attack_tag_to_walk_tag(&current_tag.0);
            commands
                .entity(entity)
                .insert(FollowState {
                    target: attack.target,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .remove::<MultiLeapAttackState>()
                .remove::<MobIsAttacking>()
                .insert(EnemyAttackCooldown(attack.attack_cooldown_timer.clone()));
            set_animation_tag(&mut anim, &mut current_tag, walk_tag);
        }
    }
}

// ---------------------------------------------------------------------------
// Bull: charge attack (straight-line dash past player, then decelerate)
// ---------------------------------------------------------------------------
pub fn aseprite_bull_charge(
    global_transforms: Query<&GlobalTransform>,
    mut local_transforms: Query<&mut Transform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &Attack,
            &mut KinematicCharacterController,
            &mut BullChargeState,
            &FollowSpeed,
            &mut AseAnimation,
            &mut CurrentAsepriteTag,
            Option<&MobStatusEffects>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    bull_configs: Query<&BullChargeAttack>,
    mut commands: Commands,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
) {
    for (
        entity,
        mob,
        _enemy_attack,
        mut kcc,
        mut charge,
        follow_speed,
        mut anim,
        mut current_tag,
        status_option,
        defiance_frozen_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }

        let config = bull_configs.get(entity).ok();
        let overshoot = config.map_or(32., |c| c.overshoot);

        match charge.phase {
            BullChargePhase::WindUp => {
                if charge.attack_startup_timer.fraction() == 0. {
                    spawn_attack_warning_aseprite(
                        &mut commands,
                        &asset_server,
                        Vec3::new(0., 12., 10.),
                        entity,
                        charge.attack_startup_timer.duration().as_secs_f32() + 0.01,
                    );
                }
                // No FollowState during charge — keep sprite facing the player during wind-up.
                if let (Ok(target_tf), Ok(my_tf), Ok(mut tf)) = (
                    global_transforms.get(charge.target),
                    global_transforms.get(entity),
                    local_transforms.get_mut(entity),
                ) {
                    let to_target =
                        target_tf.translation().truncate() - my_tf.translation().truncate();
                    if to_target.length_squared() >= 4.0 {
                        apply_horizontal_sprite_flip_for_dir(
                            &mut tf,
                            to_target.normalize_or_zero(),
                        );
                    }
                }
                charge.attack_startup_timer.tick(time.delta());
                if charge.attack_startup_timer.is_finished() {
                    let target_pos = global_transforms.get(charge.target).unwrap().translation();
                    let my_pos = global_transforms.get(entity).unwrap().translation();
                    let dir = (target_pos.truncate() - my_pos.truncate()).normalize_or_zero();
                    let destination = target_pos.truncate() + dir * overshoot;
                    charge.charge_target_pos = Some(destination);
                    charge.charge_dir = Some(dir);
                    charge.phase = BullChargePhase::Charging;

                    let attack_tag = direction_to_attack_tag(dir);
                    set_animation_tag(&mut anim, &mut current_tag, attack_tag);
                    commands.entity(entity).insert(MobIsAttacking(mob.clone()));
                    if let Ok(mut tf) = local_transforms.get_mut(entity) {
                        apply_horizontal_sprite_flip_for_dir(&mut tf, dir);
                    }
                }
            }
            BullChargePhase::Charging => {
                let my_pos = global_transforms.get(entity).unwrap().translation();
                let Some(target_pos) = charge.charge_target_pos else {
                    continue;
                };
                let Some(dir) = charge.charge_dir else {
                    continue;
                };

                let dist_remaining = (target_pos - my_pos.truncate()).dot(dir);

                if dist_remaining <= 0. {
                    charge.phase = BullChargePhase::Stopping;
                    charge.deceleration_timer.reset();
                    let stop_tag = direction_to_attack_stop_tag(dir);
                    set_animation_tag(&mut anim, &mut current_tag, stop_tag);
                    if let Ok(mut tf) = local_transforms.get_mut(entity) {
                        apply_horizontal_sprite_flip_for_dir(&mut tf, dir);
                    }
                } else {
                    let speed = charge.charge_speed
                        * time.delta_secs()
                        * status_option
                            .map(|s| 1.0 - s.slow_stacks() as f32 * 0.15)
                            .unwrap_or(1.0);
                    kcc.translation = Some(dir * speed);
                    kcc.filter_groups = Some(bevy_rapier2d::prelude::CollisionGroups::new(
                        bevy_rapier2d::prelude::Group::NONE,
                        bevy_rapier2d::prelude::Group::NONE,
                    ));
                    if let Ok(mut tf) = local_transforms.get_mut(entity) {
                        apply_horizontal_sprite_flip_for_dir(&mut tf, dir);
                    }
                }
            }
            BullChargePhase::Stopping => {
                charge.deceleration_timer.tick(time.delta());

                let dir = charge.charge_dir.unwrap_or(Vec2::ZERO);
                let t = charge.deceleration_timer.fraction();
                let decel_speed = charge.charge_speed * (1.0 - t) * 0.3 * time.delta_secs();
                if decel_speed > 0.1 {
                    kcc.translation = Some(dir * decel_speed);
                }
                if dir.length_squared() > f32::EPSILON {
                    if let Ok(mut tf) = local_transforms.get_mut(entity) {
                        apply_horizontal_sprite_flip_for_dir(&mut tf, dir);
                    }
                }

                if charge.deceleration_timer.is_finished() {
                    // Re-initiate: go back to wind-up with a fresh target
                    charge.phase = BullChargePhase::WindUp;
                    charge.attack_startup_timer.reset();
                    charge.charge_target_pos = None;
                    charge.charge_dir = None;

                    let walk_tag = attack_tag_to_walk_tag(&current_tag.0);
                    commands
                        .entity(entity)
                        .remove::<MobIsAttacking>()
                        .insert(EnemyAttackCooldown(charge.attack_cooldown_timer.clone()));

                    // Transition back to follow for re-approach
                    commands
                        .entity(entity)
                        .insert(FollowState {
                            target: charge.target,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed.0,
                        })
                        .remove::<BullChargeState>();
                    set_animation_tag(&mut anim, &mut current_tag, walk_tag);
                }
            }
        }
    }
}
