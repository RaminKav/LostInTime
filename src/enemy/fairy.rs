use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier2d::control::KinematicCharacterController;
use rand::Rng;
use seldom_state::{prelude::StateMachine, trigger::BoolTrigger};

use crate::{ai::IdleState, inputs::FacingDirection, ui::SubmitEssenceChoice, PLAYER_MOVE_SPEED};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use super::Mob;

aseprite!(pub Fairy, "textures/fairy/fairy.ase");

pub fn handle_new_fairy_state_machine(
    mut commands: Commands,
    spawn_events: Query<(Entity, &Mob, &Transform, &IdleState), Added<Mob>>,
    asset_server: Res<AssetServer>,
) {
    for (e, mob, transform, idle_state) in spawn_events.iter() {
        if mob != &Mob::Fairy {
            continue;
        }
        let mut e_cmds = commands.entity(e);
        let mut animation = AsepriteAnimation::from(Fairy::tags::IDLE_FRONT);
        animation.play();
        e_cmds
            .insert(AsepriteBundle {
                aseprite: asset_server.load(Fairy::PATH),
                animation,
                transform: *transform,
                ..Default::default()
            })
            .insert(IdleState {
                walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                speed: idle_state.speed,
                is_stopped: false,
            });
        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .trans::<IdleState>(
                PlayerFinishedTrade,
                TradeState {
                    startup_timer: Timer::from_seconds(0.7, TimerMode::Once),
                    despawn_timer: Timer::from_seconds(2., TimerMode::Once),
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
pub struct TradeState {
    startup_timer: Timer,
    despawn_timer: Timer,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct WaitingToSproutState;

pub fn new_idle(
    mut transforms: Query<&mut KinematicCharacterController>,
    mut idles: Query<(Entity, &mut IdleState, &mut AsepriteAnimation)>,
    time: Res<Time>,
) {
    for (entity, mut idle, mut anim) in idles.iter_mut() {
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
                match idle.direction {
                    FacingDirection::Left => {
                        *anim = AsepriteAnimation::from(Fairy::tags::IDLE_SIDE)
                    }
                    FacingDirection::Right => {
                        *anim = AsepriteAnimation::from(Fairy::tags::IDLE_SIDE)
                    }
                    FacingDirection::Up => *anim = AsepriteAnimation::from(Fairy::tags::IDLE_BACK),
                    FacingDirection::Down => {
                        *anim = AsepriteAnimation::from(Fairy::tags::IDLE_FRONT)
                    }
                }
            } else {
                idle.is_stopped = false;

                let new_dir = idle.direction.get_next_rand_dir(rand::thread_rng()).clone();
                idle.direction = new_dir.clone();
                match new_dir {
                    FacingDirection::Left => {
                        *anim = AsepriteAnimation::from(Fairy::tags::WALK_SIDE)
                    }
                    FacingDirection::Right => {
                        *anim = AsepriteAnimation::from(Fairy::tags::WALK_SIDE)
                    }
                    FacingDirection::Up => *anim = AsepriteAnimation::from(Fairy::tags::WALK_BACK),
                    FacingDirection::Down => {
                        *anim = AsepriteAnimation::from(Fairy::tags::WALK_FRONT)
                    }
                }
            }
        }
    }
}

pub fn trade_anim(
    mut trades: Query<(Entity, &mut TradeState, &mut AsepriteAnimation)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut trade, mut anim) in trades.iter_mut() {
        trade.startup_timer.tick(time.delta());
        trade.despawn_timer.tick(time.delta());

        if trade.startup_timer.just_finished() {
            *anim = AsepriteAnimation::from(Fairy::tags::FRONT_TRADE);
        }
        if trade.despawn_timer.just_finished() {
            commands.entity(e).despawn_recursive();
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct PlayerFinishedTrade;

impl BoolTrigger for PlayerFinishedTrade {
    type Param<'w, 's> = EventReader<'w, 's, SubmitEssenceChoice>;

    fn trigger(&self, _entity: Entity, trade_event: Self::Param<'_, '_>) -> bool {
        if !trade_event.is_empty() {
            return true;
        }
        false
    }
}
