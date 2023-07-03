use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::{custom_commands::CommandsExt, proto::proto_param::ProtoParam, GameParam, GameState};

#[derive(Component, Reflect, Schematic, FromReflect, Default)]
#[reflect(Component, Schematic)]
pub struct RangedAttack(pub Projectile);

pub struct RangedAttackPlugin;

#[derive(
    Deserialize,
    FromReflect,
    Default,
    Reflect,
    Clone,
    Serialize,
    Component,
    Schematic,
    IntoStaticStr,
    Display,
)]
#[reflect(Component, Schematic)]
pub enum Projectile {
    #[default]
    None,
    Rock,
}
#[derive(Deserialize, FromReflect, Default, Reflect, Clone, Serialize, Component, Schematic)]
#[reflect(Component, Schematic)]
pub struct ProjectileState {
    pub speed: f32,
    pub direction: Vec2,
}

pub struct RangedAttackEvent {
    pub projectile: Projectile,
    pub direction: Vec2,
}

impl Plugin for RangedAttackPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RangedAttackEvent>().add_systems(
            (
                Self::handle_ranged_attack_event,
                Self::handle_translate_projectiles,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}
impl RangedAttackPlugin {
    fn handle_ranged_attack_event(
        mut events: EventReader<RangedAttackEvent>,
        mut proto_commands: ProtoCommands,
        game: GameParam,
        proto: ProtoParam,
    ) {
        for proj_event in events.iter() {
            proto_commands.spawn_projectile_from_proto(
                proj_event.projectile.clone(),
                &proto,
                game.game.player_state.position.truncate(),
                proj_event.direction,
            );
        }
    }
    fn handle_translate_projectiles(
        mut query: Query<(&mut Transform, &ProjectileState), With<Projectile>>,
    ) {
        for (mut transform, state) in query.iter_mut() {
            let delta = state.direction * state.speed;
            transform.translation += delta.extend(0.0);
        }
    }
}
