use bevy::{
    prelude::Plugin,
    reflect::{FromReflect, Reflect},
    time::{Timer, TimerMode},
};
use bevy_proto::prelude::{PrototypesMut, ReflectSchematic, Schematic};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};

use crate::{
    ai::{IdleState, MoveDirection},
    animations::{AnimationFrameTracker, AnimationTimer},
    attributes::Health,
    enemy::{HostileMob, Mob, NeutralMob, PassiveMob},
    YSort,
};
pub struct ProtoPlugin;

impl Plugin for ProtoPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.register_type::<Mob>()
            .register_type::<NeutralMob>()
            .register_type::<PassiveMob>()
            .register_type::<HostileMob>()
            .register_type::<AnimationFrameTracker>()
            .register_type::<Health>()
            .register_type::<YSort>()
            .register_type::<IdleStateProto>()
            .register_type::<KCC>()
            .register_type::<ColliderProto>()
            .register_type::<AnimationTimerProto>()
            .add_plugin(bevy_proto::prelude::ProtoPlugin::new())
            .add_startup_system(Self::load_prototypes);
    }
}

impl ProtoPlugin {
    fn load_prototypes(mut prototypes: PrototypesMut) {
        prototypes.load("proto/mob_basic.prototype.ron");
        prototypes.load("proto/slime_neutral.prototype.ron");
    }
}
#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = KinematicCharacterController)]
struct KCC;

impl From<KCC> for KinematicCharacterController {
    fn from(_: KCC) -> KinematicCharacterController {
        KinematicCharacterController::default()
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = IdleState)]
struct IdleStateProto {
    walk_dir_change_time: f32,
    speed: f32,
}

impl From<IdleStateProto> for IdleState {
    fn from(idle_state: IdleStateProto) -> IdleState {
        IdleState {
            walk_timer: Timer::from_seconds(idle_state.walk_dir_change_time, TimerMode::Repeating),
            direction: MoveDirection::new_rand_dir(rand::thread_rng()),
            speed: idle_state.speed,
        }
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = AnimationTimer)]
struct AnimationTimerProto {
    secs: f32,
}

impl From<AnimationTimerProto> for AnimationTimer {
    fn from(state: AnimationTimerProto) -> AnimationTimer {
        AnimationTimer(Timer::from_seconds(state.secs, TimerMode::Repeating))
    }
}

#[derive(Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
#[schematic(into = Collider)]
struct ColliderProto {
    x: f32,
    y: f32,
}

impl From<ColliderProto> for Collider {
    fn from(col_state: ColliderProto) -> Collider {
        Collider::cuboid(col_state.x, col_state.y)
    }
}
