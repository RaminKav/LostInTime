use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::{CollisionGroups, Group, KinematicCharacterController};
use seldom_state::prelude::{StateMachine, Trigger};

use crate::{
    ai::{
        CachedAttackDistance, CachedLineOfSight, EnemyAttackCooldown, FollowState, HurtByPlayer,
        IdleState, LeapAttackState, NightTimeAggro, ProjectileAttackState,
    },
    animations::enemy_sprites::spawn_attack_warning_aseprite,
    attributes::Attack,
    combat::status_effects::Frozen,
    enemy::{CombatAlignment, FollowSpeed, LeapAttack, Mob, MobIsAttacking, ProjectileAttack},
    inputs::FacingDirection,
    item::projectile::{Projectile, RangedAttackEvent},
    night::NightTracker,
    player::{
        melee_skills::Parried,
        skills::{Heirloom, PlayerSkills},
    },
    status_effects::Slow,
    world::dungeon::Dungeon,
    GameParam, PLAYER_MOVE_SPEED,
};

// Each "basic" aseprite enemy (walk + lunge) must be declared here so the macro runs at compile time.
aseprite!(pub Crow, "textures/crow.ase");

/// Fixed animation tag names for the shared aseprite basic enemy behavior.
/// Aseprite files must use these exact tag names: WalkUp, WalkDown, WalkSide, AttackUp, AttackDown, AttackSide.
const WALK_UP: &str = "WalkUp";
const WALK_DOWN: &str = "WalkDown";
const WALK_SIDE: &str = "WalkSide";
const ATTACK_UP: &str = "AttackUp";
const ATTACK_DOWN: &str = "AttackDown";
const ATTACK_SIDE: &str = "AttackSide";

/// Marker for enemies that use the shared aseprite walk + lunge behavior.
/// Setup is done in code via `get_aseprite_basic_config` (each such mob needs an `aseprite!` macro).
#[derive(Component, Default, Debug)]
pub struct AsepriteBasicEnemy;

/// Tracks the currently active aseprite animation tag to avoid redundantly
/// resetting the same animation every frame.
#[derive(Component, Default, Debug)]
pub struct CurrentAsepriteTag(pub String);

/// Returns (aseprite path, initial walk tag) for mobs that use the shared aseprite basic behavior.
/// Add a new match arm and an `aseprite!(pub Name, "path")` at the top of this file for each new enemy.
fn get_aseprite_basic_config(mob: &Mob) -> Option<(&'static str, &'static str)> {
    match mob {
        Mob::Crow => Some((Crow::PATH, WALK_DOWN)),
        _ => None,
    }
}

/// Returns true if this mob uses the shared aseprite basic behavior (so generic state machine should not be applied).
pub fn is_aseprite_basic_mob(mob: &Mob) -> bool {
    get_aseprite_basic_config(mob).is_some()
}

/// Initializes aseprite basic enemies: when a Mob in our registry is spawned, insert AsepriteBundle and marker.
pub fn aseprite_enemy_setup(
    mut commands: Commands,
    new_mobs: Query<(Entity, &Mob, &Transform), Added<Mob>>,
    asset_server: Res<AssetServer>,
) {
    for (entity, mob, transform) in new_mobs.iter() {
        let Some((path, initial_tag)) = get_aseprite_basic_config(mob) else {
            continue;
        };

        let mut animation = AsepriteAnimation::from(initial_tag);
        animation.play();

        commands
            .entity(entity)
            .insert(AsepriteBundle {
                aseprite: asset_server.load(path),
                animation,
                transform: *transform,
                ..Default::default()
            })
            .insert(CurrentAsepriteTag(initial_tag.to_string()))
            .insert(AsepriteBasicEnemy);
    }
}

fn set_animation_tag(
    anim: &mut AsepriteAnimation,
    current_tag: &mut CurrentAsepriteTag,
    new_tag: &str,
) {
    if current_tag.0 != new_tag {
        *anim = AsepriteAnimation::from(new_tag);
        if anim.is_paused() {
            anim.play();
        }
        current_tag.0 = new_tag.to_string();
    }
}

/// Maps an attack tag to the matching walk tag so post-attack we keep the same direction.
fn attack_tag_to_walk_tag(attack_tag: &str) -> &'static str {
    match attack_tag {
        ATTACK_UP => WALK_UP,
        ATTACK_DOWN => WALK_DOWN,
        ATTACK_SIDE => WALK_SIDE,
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
        ),
        Added<AsepriteBasicEnemy>,
    >,
    dungeon_check: Query<&Dungeon>,
) {
    for (e, alignment, follow_speed, leap_attack_option, proj_attack_option) in spawn_events.iter()
    {
        let mut alignment = alignment.clone();
        commands
            .entity(e)
            .insert(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1));
        if dungeon_check.get_single().is_ok() {
            alignment = CombatAlignment::Hostile;
        }

        let mut state_machine = StateMachine::default().set_trans_logging(false);

        match alignment {
            CombatAlignment::Neutral => {
                state_machine = state_machine
                    .trans::<IdleState>(
                        HurtByPlayer,
                        FollowState {
                            target: game.game.player,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed.0,
                        },
                    )
                    .trans::<FollowState>(
                        Trigger::not(CachedLineOfSight {
                            range_sq: 130. * 130.,
                        }),
                        IdleState {
                            walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                            speed: 0.5,
                            is_stopped: false,
                        },
                    );
            }
            CombatAlignment::Hostile => {
                state_machine = state_machine.trans::<IdleState>(
                    CachedLineOfSight {
                        range_sq: 130. * 130.,
                    },
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
            state_machine = state_machine
                .trans::<FollowState>(
                    CachedAttackDistance {
                        range_sq: leap_attack.activation_distance * leap_attack.activation_distance,
                    },
                    LeapAttackState {
                        target: game.game.player,
                        attack_startup_timer: Timer::from_seconds(
                            leap_attack.startup,
                            TimerMode::Once,
                        ),
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
                )
                .trans::<LeapAttackState>(
                    Trigger::not(CachedAttackDistance {
                        range_sq: (leap_attack.activation_distance + 32.).powi(2),
                    }),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
        }

        if let Some(proj_attack) = proj_attack_option {
            state_machine = state_machine
                .trans::<FollowState>(
                    CachedAttackDistance {
                        range_sq: proj_attack.activation_distance * proj_attack.activation_distance,
                    },
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
                .trans::<ProjectileAttackState>(
                    Trigger::not(CachedAttackDistance {
                        range_sq: (proj_attack.activation_distance + 30.).powi(2),
                    }),
                    FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed.0,
                    },
                );
        }

        if alignment != CombatAlignment::Passive {
            state_machine = state_machine.trans::<IdleState>(
                NightTimeAggro,
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
            &mut AsepriteAnimation,
            &mut CurrentAsepriteTag,
            Option<&Slow>,
            Option<&Parried>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&Frozen>,
            Option<&crate::combat::status_effects::RapidfireSlow>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    time: Res<Time>,
    night_tracker: Res<NightTracker>,
) {
    for (
        entity,
        mut follow,
        mut anim,
        mut current_tag,
        slowed_option,
        parried_option,
        defiance_frozen_option,
        blessing_frozen_option,
        rapidfire_slow_option,
    ) in follows.iter_mut()
    {
        if defiance_frozen_option.is_some() || blessing_frozen_option.is_some() {
            continue;
        }
        if parried_option.is_some() {
            continue;
        }

        let Ok(target_translation) = transforms.get(follow.target) else {
            continue;
        };
        let enemy_translation = transforms.get(entity).unwrap().translation;
        let delta = (target_translation.translation.truncate() - enemy_translation.truncate())
            .normalize_or_zero();

        let mut mover = mover.get_mut(entity).unwrap();
        mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));

        follow.curr_path = Some(delta);
        follow.curr_delta = Some(delta);

        mover.translation = Some(
            delta
                * follow.speed
                * PLAYER_MOVE_SPEED
                * time.delta_seconds()
                * (1. - slowed_option.map_or(0., |s| s.num_stacks as f32 * 0.15))
                * if rapidfire_slow_option.is_some() {
                    0.5
                } else {
                    1.0
                }
                * if night_tracker.is_night() { 2. } else { 1. },
        );

        commands
            .entity(entity)
            .insert(FacingDirection::from_translation(delta));

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
            &mut AsepriteAnimation,
            &mut CurrentAsepriteTag,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&Frozen>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (
        entity,
        mut idle,
        mut anim,
        mut current_tag,
        defiance_frozen_option,
        blessing_frozen_option,
    ) in idles.iter_mut()
    {
        if defiance_frozen_option.is_some() || blessing_frozen_option.is_some() {
            continue;
        }

        idle.walk_timer.tick(time.delta());
        let mut idle_kcc = transforms.get_mut(entity).unwrap();
        if !idle.is_stopped {
            let s = idle.speed * PLAYER_MOVE_SPEED * time.delta_seconds();
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
                commands.entity(entity).insert(FacingDirection::Down);
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
                commands.entity(entity).insert(new_dir);
            }
        }
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
            &mut AsepriteAnimation,
            &mut CurrentAsepriteTag,
            Option<&Slow>,
            Option<&mut Parried>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&Frozen>,
        ),
        With<AsepriteBasicEnemy>,
    >,
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
        mut anim,
        mut current_tag,
        slow_option,
        mut parried_option,
        defiance_frozen_option,
        blessing_frozen_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || blessing_frozen_option.is_some() {
            continue;
        }

        let target_translation = transforms.get(attack.target).unwrap().translation();
        let attack_translation = transforms.get_mut(entity).unwrap().translation();

        if attack.attack_startup_timer.finished() && !attack.attack_duration_timer.finished() {
            let delta = target_translation - attack_translation;
            let delta_xy = delta.truncate();
            if attack.dir.is_none() {
                attack.dir = Some(
                    delta_xy.normalize_or_zero()
                        * attack.speed
                        * time.delta_seconds()
                        * (1. - slow_option.map_or(0., |s| s.num_stacks as f32 * 0.15)),
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

            let attack_tag = if delta_xy.x.abs() > delta_xy.y.abs() * 1.1 {
                ATTACK_SIDE
            } else if delta_xy.y > 0. {
                ATTACK_UP
            } else {
                ATTACK_DOWN
            };
            set_animation_tag(&mut anim, &mut current_tag, attack_tag);
            commands.entity(entity).insert(MobIsAttacking(mob.clone()));
        }

        if attack.attack_duration_timer.finished() {
            attack.dir = None;
            if anim.just_finished() || parried_option.is_some() {
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

/// Tracks whether the projectile has already been fired during this attack cycle,
/// so we only fire once but let the animation play to completion.
#[derive(Component, Default)]
pub struct AsepriteProjectileFired;

/// Projectile attack for aseprite basic enemies. Uses same attack tags (AttackUp / AttackDown / AttackSide).
/// Flow: startup timer (with attack warning) -> switch to attack anim -> fire when projectile_delay elapses -> wait for `just_finished()` -> back to follow.
pub fn aseprite_projectile_attack(
    mut transforms: Query<&GlobalTransform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &Attack,
            &mut ProjectileAttackState,
            &FollowSpeed,
            &mut AsepriteAnimation,
            &mut CurrentAsepriteTag,
            Option<&AsepriteProjectileFired>,
            Option<&crate::player::combat_heirlooms::DeathDefianceFrozen>,
            Option<&Frozen>,
        ),
        With<AsepriteBasicEnemy>,
    >,
    mut commands: Commands,
    mut events: EventWriter<RangedAttackEvent>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    const FEATHER_SPREAD_RAD: f32 = 0.15;

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
        blessing_frozen_option,
    ) in attacks.iter_mut()
    {
        if defiance_frozen_option.is_some() || blessing_frozen_option.is_some() {
            continue;
        }

        let target_translation = transforms.get(attack.target).unwrap().translation();
        let attack_translation = transforms.get(entity).unwrap().translation();
        let delta =
            (target_translation.truncate() - attack_translation.truncate()).normalize_or_zero();

        // Phase 1: startup timer (pre-animation wind-up, enemy stays in walk anim; show attack warning)
        if !attack.attack_startup_timer.finished() {
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
        if attack.projectile_delay_timer.finished() && fired_option.is_none() {
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
                events.send(RangedAttackEvent {
                    projectile: attack.projectile.clone(),
                    direction: proj_dir,
                    from_entity: Some(entity),
                    from_enemy: true,
                    is_followup_proj: false,
                    mana_cost: None,
                    dmg_override: Some(enemy_attack.0),
                    pos_override: None,
                    spawn_delay: 0.,
                });
            }
            commands.entity(entity).insert(AsepriteProjectileFired);
        }

        // Phase 4: wait for attack animation to finish, then transition back
        if anim.just_finished() {
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
