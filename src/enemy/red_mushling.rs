use bevy::prelude::*;
use bevy_rapier2d::geometry::{Collider, Sensor};
use seldom_state::{
    prelude::StateMachine,
    trigger::{BoolTrigger, Trigger},
};

use crate::{
    ai::{HurtByPlayer, LineOfSight},
    attributes::Attack,
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
                MushkingSummoned.and(LineOfSight {
                    target: game.player,
                    range: 40.,
                }),
                SproutingState,
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
                .insert(GasAttackState { hitbox: None });
            *anim = AsepriteAnimation::from(RedMushling::tags::ATTACK);
        }
    }
}

pub fn gas_attack(
    mut sprouts: Query<(Entity, &mut AsepriteAnimation, &Attack, &mut GasAttackState)>,
    mut commands: Commands,
) {
    for (entity, mut anim, attack, mut gas_state) in sprouts.iter_mut() {
        if anim.is_paused() {
            anim.play();
        }
        if anim.current_frame() >= 33 && anim.current_frame() <= 40 {
            if let Some(hitbox) = gas_state.hitbox {
                if let Some(mut hit_e) = commands.get_entity(hitbox) {
                    hit_e.insert(Collider::capsule(Vec2::ZERO, Vec2::ZERO, 24.));
                }
            } else {
                let hitbox = commands
                    .spawn((
                        TransformBundle::default(),
                        attack.clone(),
                        Collider::capsule(Vec2::ZERO, Vec2::ZERO, 7.),
                        MobIsAttacking,
                        Sensor,
                    ))
                    .set_parent(entity)
                    .id();
                gas_state.hitbox = Some(hitbox);
            }
        }
        if anim.current_frame() >= 46 {
            commands
                .entity(entity)
                .remove::<GasAttackState>()
                .insert(WaitingToSproutState);
            *anim = AsepriteAnimation::from(RedMushling::tags::ATTACK);
            anim.pause();
        } else if anim.current_frame() == 41 {
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
