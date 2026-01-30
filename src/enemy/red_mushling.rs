use bevy::prelude::*;
use bevy_rapier2d::geometry::{Collider, Sensor};
use seldom_state::{
    prelude::StateMachine,
    trigger::{BoolTrigger, Trigger},
};

use crate::{
    ai::{FollowState, HurtByPlayer, IdleState, LineOfSight, NightTimeAggro},
    animations::enemy_sprites::spawn_attack_warning_aseprite,
    attributes::Attack,
    inputs::FacingDirection,
    Game,
};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use super::{Mob, MobIsAttacking};

aseprite!(pub RedMushling, "textures/redmushling/red_mushling.ase");

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
        let mut animation = AsepriteAnimation::from(RedMushling::tags::SPURTING);
        animation.pause();
        e_cmds
            .insert(AsepriteBundle {
                aseprite: asset_server.load(RedMushling::PATH),
                animation,
                transform: *transform,
                ..Default::default()
            })
            .insert(WaitingToSproutState)
            .insert(MushlingWakeupState {
                time_alive: Timer::from_seconds(15.0, TimerMode::Once),
                wakeup_check_timer: Timer::from_seconds(2.0, TimerMode::Repeating), // Check every 2 seconds
                is_awake: false,
            });
        let state_machine = StateMachine::default()
            .with_state::<GasAttackState>()
            .set_trans_logging(false)
            .trans::<WaitingToSproutState>(HurtByPlayer, SproutingState)
            .trans::<WaitingToSproutState>(
                LineOfSight {
                    target: game.player,
                    range: 40.,
                }
                .or(MushkingSummoned),
                SproutingState,
            )
            .trans::<IdleState>(
                LineOfSight {
                    target: game.player,
                    range: 30.,
                },
                GasAttackState {
                    hitbox: None,
                    speed_up_anim: false,
                    cooldown: Timer::from_seconds(0.6, TimerMode::Once),
                },
            )
            .trans::<FollowState>(
                LineOfSight {
                    target: game.player,
                    range: 20.,
                }
                .and(MushkingSummoned.not()),
                GasAttackState {
                    hitbox: None,
                    speed_up_anim: false,
                    cooldown: Timer::from_seconds(0.6, TimerMode::Once),
                },
            )
            .trans::<IdleState>(
                MushkingSummoned,
                FollowState {
                    target: game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: 1.,
                },
            )
            .trans::<IdleState>(
                NightTimeAggro,
                FollowState {
                    target: game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: 0.4,
                },
            )
            .trans::<FollowState>(
                MushkingSummoned.and(LineOfSight {
                    target: game.player,
                    range: 20.,
                }),
                GasAttackState {
                    hitbox: None,
                    speed_up_anim: true,
                    cooldown: Timer::from_seconds(0.0, TimerMode::Once),
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
        (Entity, &mut AsepriteAnimation, Option<&MushlingWakeupState>),
        With<SproutingState>,
    >,
    mut commands: Commands,
    game: Res<Game>,
) {
    for (entity, mut anim, wakeup_state_option) in sprouts.iter_mut() {
        if anim.is_paused() {
            anim.play();
        }

        if anim.current_frame() >= 16 {
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
                *anim = AsepriteAnimation::from(RedMushling::tags::IDLE_FRONT);
            } else {
                commands
                    .entity(entity)
                    .remove::<SproutingState>()
                    .insert(GasAttackState {
                        hitbox: None,
                        speed_up_anim: false,
                        cooldown: Timer::from_seconds(0.6, TimerMode::Once),
                    });
                *anim = AsepriteAnimation::from(RedMushling::tags::ATTACK);
            }
        }
    }
}

pub fn gas_attack(
    mut sprouts: Query<(Entity, &mut AsepriteAnimation, &Attack, &mut GasAttackState)>,
    mut commands: Commands,
    time: Res<Time>,
) {
    for (entity, mut anim, attack, mut gas_state) in sprouts.iter_mut() {
        gas_state.cooldown.tick(time.delta());
        if !gas_state.cooldown.finished() {
            continue;
        }

        if !gas_state.speed_up_anim && (anim.current_frame() < 18 || anim.current_frame() > 46) {
            *anim = AsepriteAnimation::from(RedMushling::tags::ATTACK);
        }
        if gas_state.speed_up_anim && (anim.current_frame() < 66 || anim.current_frame() > 77) {
            *anim = AsepriteAnimation::from(RedMushling::tags::NUKE);
        }
        if anim.is_paused() {
            anim.play();
        }
        if anim.current_frame() >= 33 && anim.current_frame() < 39
            || (gas_state.speed_up_anim && anim.current_frame() >= 66)
        {
            if let Some(hitbox) = gas_state.hitbox {
                if let Some(mut hit_e) = commands.get_entity(hitbox) {
                    hit_e.insert(Collider::capsule(Vec2::ZERO, Vec2::ZERO, 24.));
                }
            } else {
                let hitbox = commands
                    .spawn((
                        TransformBundle::default(),
                        *attack,
                        Collider::capsule(Vec2::ZERO, Vec2::ZERO, 7.),
                        MobIsAttacking(Mob::RedMushling),
                        Sensor,
                    ))
                    .set_parent(entity)
                    .id();
                gas_state.hitbox = Some(hitbox);
            }
        }
        if anim.just_finished() {
            if gas_state.speed_up_anim {
                commands.entity(entity).despawn_recursive();
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
            *anim = AsepriteAnimation::from(RedMushling::tags::IDLE_FRONT);
        } else if anim.current_frame() == 39 {
            if let Some(hitbox) = gas_state.hitbox {
                if let Some(hitbox) = commands.get_entity(hitbox) {
                    hitbox.despawn_recursive();
                }
            }
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct MushkingSummoned;

impl BoolTrigger for MushkingSummoned {
    type Param<'w, 's> = Query<'w, 's, &'static Mob>;

    fn trigger(&self, _entity: Entity, query: Self::Param<'_, '_>) -> bool {
        for mob in query.iter() {
            if mob == &Mob::RedMushking {
                return true;
            }
        }
        false
    }
}

#[derive(Component)]
pub struct MushlingRushWarning;

pub fn handle_mushling_rush_warnings(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mushlings: Query<(Entity, &Mob, Option<&FollowState>, Option<&GasAttackState>)>,
    mushking_exists: Query<&Mob>,
    existing_warnings: Query<(Entity, &Parent), With<MushlingRushWarning>>,
) {
    // Check if mushking exists
    let mushking_active = mushking_exists.iter().any(|mob| mob == &Mob::RedMushking);

    // Build map of existing warnings by parent mushling entity
    let mut warnings_by_mushling: std::collections::HashMap<Entity, Entity> =
        std::collections::HashMap::new();
    for (warning_entity, parent) in existing_warnings.iter() {
        warnings_by_mushling.insert(parent.get(), warning_entity);
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
            commands.entity(*warning_entity).despawn_recursive();
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
            &mut AsepriteAnimation,
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

        if wakeup_state.time_alive.finished() {
            wakeup_state.wakeup_check_timer.tick(time.delta());

            if wakeup_state.wakeup_check_timer.just_finished() {
                if rng.gen_bool(0.3) {
                    wakeup_state.is_awake = true;
                    commands
                        .entity(entity)
                        .remove::<WaitingToSproutState>()
                        .insert(SproutingState);
                    if anim.is_paused() {
                        anim.play();
                    }
                }
            }
        }
    }
}
