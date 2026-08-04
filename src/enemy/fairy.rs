use crate::aseprite_assets::Fairy;
use crate::aseprite_helpers::{
    ase_animation, aseprite_bundle, collect_finished, is_paused, pause, play_loop, play_once, start,
};
use bevy_aseprite_ultra::prelude::{AnimationState, AseAnimation, Aseprite};
use std::time::Duration;

use bevy::prelude::*;
use bevy_rapier2d::control::KinematicCharacterController;
use rand::Rng;
use seldom_state::prelude::StateMachine;

use crate::{
    ai::IdleState, inputs::FacingDirection, ui::SubmitMerchantPurchase, PLAYER_MOVE_SPEED,
};

use super::Mob;

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
        e_cmds
            .insert(aseprite_bundle(
                asset_server.load(Fairy::PATH),
                Fairy::tags::IDLE_FRONT,
                *transform,
                Visibility::Inherited,
                false,
            ))
            .insert(IdleState {
                walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                direction: FacingDirection::new_rand_dir(rand::thread_rng()),
                speed: idle_state.speed,
                is_stopped: false,
            });
        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .trans::<IdleState, _>(
                player_finished_trade,
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
    mut idles: Query<(Entity, &mut IdleState, &mut AseAnimation)>,
    time: Res<Time>,
) {
    for (entity, mut idle, mut anim) in idles.iter_mut() {
        if idle.speed <= 0. {
            continue;
        }
        idle.walk_timer.tick(time.delta());
        let mut idle_transform = transforms.get_mut(entity).unwrap();
        if !idle.is_stopped {
            let s = idle.speed * PLAYER_MOVE_SPEED * time.delta_secs();
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
                let tag = match idle.direction {
                    FacingDirection::Left | FacingDirection::Right => Fairy::tags::IDLE_SIDE,
                    FacingDirection::Up => Fairy::tags::IDLE_BACK,
                    FacingDirection::Down => Fairy::tags::IDLE_FRONT,
                };
                play_loop(&mut *anim, tag);
            } else {
                idle.is_stopped = false;

                let new_dir = idle.direction.get_next_rand_dir(rand::thread_rng()).clone();
                idle.direction = new_dir.clone();
                let tag = match new_dir {
                    FacingDirection::Left | FacingDirection::Right => Fairy::tags::WALK_SIDE,
                    FacingDirection::Up => Fairy::tags::WALK_BACK,
                    FacingDirection::Down => Fairy::tags::WALK_FRONT,
                };
                play_loop(&mut *anim, tag);
            }
        }
    }
}

pub fn trade_anim(
    mut trades: Query<(Entity, &mut TradeState, &mut AseAnimation)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut trade, mut anim) in trades.iter_mut() {
        trade.startup_timer.tick(time.delta());
        trade.despawn_timer.tick(time.delta());

        if trade.startup_timer.just_finished() {
            play_loop(&mut *anim, Fairy::tags::FRONT_TRADE);
        }
        if trade.despawn_timer.just_finished() {
            commands.entity(e).despawn();
        }
    }
}

fn player_finished_trade(mut trade_events: MessageReader<SubmitMerchantPurchase>) -> bool {
    !trade_events.is_empty()
}
