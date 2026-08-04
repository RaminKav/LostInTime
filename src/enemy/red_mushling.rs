use crate::aseprite_assets::RedMushling;
use crate::aseprite_helpers::{
    ase_animation, aseprite_bundle, collect_finished, is_paused, pause, play_loop, play_once, start,
};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{AnimationEvents, AnimationState, AseAnimation, Aseprite};
use bevy_rapier2d::geometry::{Collider, Sensor};
use seldom_state::prelude::{IntoTrigger, StateMachine};

use crate::{
    ai::{hurt_by_player, line_of_sight, night_time_aggro, FollowState, IdleState},
    animations::enemy_sprites::spawn_attack_warning_aseprite,
    attributes::Attack,
    ecs_helpers::SafeHierarchyExt,
    inputs::FacingDirection,
    Game,
};

use super::{Mob, MobIsAttacking};

pub fn handle_new_red_mushling_state_machine(
    mut commands: Commands,
    spawn_events: Query<(Entity, &Mob, &Transform), Added<Mob>>,
    asset_server: Res<AssetServer>,
    game: Res<Game>,
) {
    for (e, mob, transform) in spawn_events.iter() {
        if mob != &Mob::RedMushling {
            continue;
        }
        let mut e_cmds = commands.entity(e);
        let handle = asset_server.load(RedMushling::PATH);
        let mut animation = ase_animation(handle, RedMushling::tags::SPURTING, false);
        pause(&mut animation);
        e_cmds
            .insert((
                animation,
                Sprite::default(),
                *transform,
                GlobalTransform::default(),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .insert(WaitingToSproutState)
            .insert(MushlingWakeupState {
                time_alive: Timer::from_seconds(15.0, TimerMode::Once),
                wakeup_check_timer: Timer::from_seconds(2.0, TimerMode::Repeating), // Check every 2 seconds
                is_awake: false,
            });
        let state_machine = StateMachine::default()
            .with_state::<GasAttackState>()
            .set_trans_logging(false)
            .trans::<WaitingToSproutState, _>(hurt_by_player, SproutingState)
            .trans::<WaitingToSproutState, _>(
                line_of_sight(game.player, 40.).or(mushking_summoned),
                SproutingState,
            )
            .trans::<IdleState, _>(
                line_of_sight(game.player, 30.),
                GasAttackState {
                    hitbox: None,
                    speed_up_anim: false,
                    cooldown: Timer::from_seconds(0.6, TimerMode::Once),
                },
            )
            .trans::<FollowState, _>(
                line_of_sight(game.player, 20.).and(mushking_summoned.not()),
                GasAttackState {
                    hitbox: None,
                    speed_up_anim: false,
                    cooldown: Timer::from_seconds(0.6, TimerMode::Once),
                },
            )
            .trans::<IdleState, _>(
                mushking_summoned,
                FollowState {
                    target: game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: 1.,
                },
            )
            .trans::<IdleState, _>(
                night_time_aggro,
                FollowState {
                    target: game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: 0.4,
                },
            )
            .trans::<FollowState, _>(
                IntoTrigger::and(mushking_summoned, line_of_sight(game.player, 20.)),
                GasAttackState {
                    hitbox: None,
                    speed_up_anim: true,
                    cooldown: Timer::from_seconds(0.2, TimerMode::Once),
                },
            );

        e_cmds.insert(state_machine);
    }
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct SproutingState;

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct GasAttackState {
    hitbox: Option<Entity>,
    speed_up_anim: bool,
    cooldown: Timer,
}
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct WaitingToSproutState;

/// Component to track wakeup state for RedMushling
#[derive(Component)]
pub struct MushlingWakeupState {
    pub time_alive: Timer,
    pub wakeup_check_timer: Timer,
    pub is_awake: bool,
}

pub fn sprout(
    mut sprouts: Query<
        (
            Entity,
            &mut AseAnimation,
            &AnimationState,
            Option<&MushlingWakeupState>,
        ),
        With<SproutingState>,
    >,
    mut commands: Commands,
    game: Res<Game>,
) {
    for (entity, mut anim, state, wakeup_state_option) in sprouts.iter_mut() {
        if is_paused(&anim) {
            start(&mut anim);
        }

        if usize::from(state.current_frame()) >= 16 {
            let has_wakeup_state = wakeup_state_option.is_some();

            if has_wakeup_state {
                commands
                    .entity(entity)
                    .remove::<SproutingState>()
                    .insert(FollowState {
                        target: game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: 0.4,
                    });
                play_loop(&mut anim, RedMushling::tags::IDLE_FRONT);
            } else {
                commands
                    .entity(entity)
                    .remove::<SproutingState>()
                    .insert(GasAttackState {
                        hitbox: None,
                        speed_up_anim: false,
                        cooldown: Timer::from_seconds(0.6, TimerMode::Once),
                    });
                play_loop(&mut anim, RedMushling::tags::ATTACK);
            }
        }
    }
}

pub fn gas_attack(
    mut sprouts: Query<(
        Entity,
        &mut AseAnimation,
        &AnimationState,
        &Attack,
        &mut GasAttackState,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    mut finished_events: MessageReader<AnimationEvents>,
) {
    let finished = collect_finished(&mut finished_events);
    for (entity, mut anim, state, attack, mut gas_state) in sprouts.iter_mut() {
        gas_state.cooldown.tick(time.delta());
        if !gas_state.cooldown.is_finished() {
            continue;
        }

        let frame = usize::from(state.current_frame());
        if !gas_state.speed_up_anim && (frame < 18 || frame > 46) {
            play_loop(&mut anim, RedMushling::tags::ATTACK);
        }
        if gas_state.speed_up_anim && (frame < 66 || frame > 77) {
            play_loop(&mut anim, RedMushling::tags::NUKE);
        }
        if is_paused(&anim) {
            start(&mut anim);
        }
        if frame >= 33 && frame < 39 || (gas_state.speed_up_anim && frame >= 66) {
            if let Some(hitbox) = gas_state.hitbox {
                if let Ok(mut hit_e) = commands.get_entity(hitbox) {
                    hit_e.insert(Collider::capsule(Vec2::ZERO, Vec2::ZERO, 24.));
                }
            } else {
                let hitbox = commands
                    .spawn((
                        Transform::default(),
                        *attack,
                        Collider::capsule(Vec2::ZERO, Vec2::ZERO, 7.),
                        MobIsAttacking(Mob::RedMushling),
                        Sensor,
                    ))
                    .safe_set_parent(entity)
                    .id();
                gas_state.hitbox = Some(hitbox);
            }
        }
        if finished.contains(&entity) {
            if gas_state.speed_up_anim {
                commands.entity(entity).despawn();
                continue;
            }
            commands
                .entity(entity)
                .remove::<GasAttackState>()
                .insert(IdleState {
                    walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                    direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                    speed: 0.,
                    is_stopped: true,
                });
            play_loop(&mut anim, RedMushling::tags::IDLE_FRONT);
        } else if frame == 39 {
            if let Some(hitbox) = gas_state.hitbox {
                if let Ok(mut hitbox) = commands.get_entity(hitbox) {
                    hitbox.despawn();
                }
            }
        }
    }
}

fn mushking_summoned(query: Query<&Mob>) -> bool {
    query.iter().any(|mob| mob == &Mob::RedMushking)
}

/// Short-lived warning marker on the rush hitbox entity before it deals
/// damage. `SparseSet` so the rush-entity archetype isn't duplicated for
/// every swing.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct MushlingRushWarning;

pub fn handle_mushling_rush_warnings(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mushlings: Query<(Entity, &Mob, Option<&FollowState>, Option<&GasAttackState>)>,
    mushking_exists: Query<&Mob>,
    existing_warnings: Query<(Entity, &ChildOf), With<MushlingRushWarning>>,
) {
    // Check if mushking exists
    let mushking_active = mushking_exists.iter().any(|mob| mob == &Mob::RedMushking);

    // Build map of existing warnings by parent mushling entity
    let mut warnings_by_mushling: std::collections::HashMap<Entity, Entity> =
        std::collections::HashMap::new();
    for (warning_entity, parent) in existing_warnings.iter() {
        warnings_by_mushling.insert(parent.parent(), warning_entity);
    }

    // Track which mushlings should have warnings
    let mut should_have_warning: std::collections::HashSet<Entity> =
        std::collections::HashSet::new();

    // Check each mushling
    for (entity, mob, follow_state, gas_attack_state) in mushlings.iter() {
        if mob != &Mob::RedMushling {
            continue;
        }

        // Check if mushling is in rush state:
        // 1. Has FollowState while MushkingSummoned is active
        // 2. Has GasAttackState with speed_up_anim: true
        let in_rush_state = if mushking_active {
            if let Some(gas_state) = gas_attack_state {
                gas_state.speed_up_anim
            } else if follow_state.is_some() {
                true // In FollowState while mushking is active
            } else {
                false
            }
        } else {
            false
        };

        if in_rush_state {
            should_have_warning.insert(entity);
        }
    }

    // Despawn warnings for mushlings that no longer need them
    for (mushling_entity, warning_entity) in warnings_by_mushling.iter() {
        if !should_have_warning.contains(mushling_entity) {
            commands.entity(*warning_entity).despawn();
        }
    }

    // Spawn warnings for mushlings that need them but don't have them
    for (entity, mob, follow_state, gas_attack_state) in mushlings.iter() {
        if mob != &Mob::RedMushling {
            continue;
        }

        let in_rush_state = if mushking_active {
            if let Some(gas_state) = gas_attack_state {
                gas_state.speed_up_anim
            } else if follow_state.is_some() {
                true
            } else {
                false
            }
        } else {
            false
        };

        if in_rush_state && !warnings_by_mushling.contains_key(&entity) {
            // Spawn warning animation above the mushling
            let warning_pos = Vec3::new(0., 12., 10.);
            let warning_entity = spawn_attack_warning_aseprite(
                &mut commands,
                &asset_server,
                warning_pos,
                entity,
                999999.0, // Very long duration so it persists
            );
            commands.entity(warning_entity).insert(MushlingRushWarning);
        }
    }
}

/// System to handle RedMushling wakeup timers and random wakeup
pub fn handle_mushling_wakeup_timers(
    mut mushlings: Query<
        (
            Entity,
            &Mob,
            &mut MushlingWakeupState,
            &mut AseAnimation,
            Option<&WaitingToSproutState>,
        ),
        Without<FollowState>,
    >,
    mut commands: Commands,
    time: Res<Time>,
) {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    for (entity, mob, mut wakeup_state, mut anim, waiting_state) in mushlings.iter_mut() {
        if mob != &Mob::RedMushling {
            continue;
        }

        if waiting_state.is_none() || wakeup_state.is_awake {
            continue;
        }

        wakeup_state.time_alive.tick(time.delta());

        if wakeup_state.time_alive.is_finished() {
            wakeup_state.wakeup_check_timer.tick(time.delta());

            if wakeup_state.wakeup_check_timer.just_finished() {
                if rng.gen_bool(0.3) {
                    wakeup_state.is_awake = true;
                    commands
                        .entity(entity)
                        .remove::<WaitingToSproutState>()
                        .insert(SproutingState);
                    if is_paused(&anim) {
                        start(&mut anim);
                    }
                }
            }
        }
    }
}
