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
            .insert(WaitingToSproutState);
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

pub fn sprout(
    mut sprouts: Query<(Entity, &mut AsepriteAnimation), With<SproutingState>>,
    mut commands: Commands,
) {
    for (entity, mut anim) in sprouts.iter_mut() {
        if anim.is_paused() {
            anim.play();
        }

        if anim.current_frame() >= 16 {
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
