use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::{
    combat::AttackTimer, custom_commands::CommandsExt, enemy::Mob, player::Player,
    proto::proto_param::ProtoParam, GameParam, GameState,
};

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
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
    Fireball,
    Electricity,
    GreenWhip,
    Arrow,
}
#[derive(Deserialize, FromReflect, Default, Reflect, Clone, Serialize, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct ProjectileState {
    pub speed: f32,
    pub direction: Vec2,
    pub hit_entities: Vec<Entity>,
    pub spawn_offset: Vec2,
}

#[derive(Deserialize, FromReflect, Default, Reflect, Clone, Serialize, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct ArcProjectileData {
    pub size: Vec2,
    pub col_size: Vec2,
    pub arc: Vec2,
    pub col_points: Vec<f32>,
}

pub struct RangedAttackEvent {
    pub projectile: Projectile,
    pub direction: Vec2,
    pub from_enemy: Option<Entity>,
}

#[derive(Clone, Component)]
pub struct EnemyProjectile {
    pub entity: Entity,
}

impl Plugin for RangedAttackPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RangedAttackEvent>().add_systems(
            (handle_ranged_attack_event, handle_translate_projectiles)
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}

fn handle_ranged_attack_event(
    mut events: EventReader<RangedAttackEvent>,
    att_cooldown_query: Query<(Entity, Option<&AttackTimer>), With<Player>>,
    mut proto_commands: ProtoCommands,
    enemy_transforms: Query<&GlobalTransform, With<Mob>>,
    game: GameParam,
    proto: ProtoParam,
    mut commands: Commands,
) {
    for proj_event in events.iter() {
        // if proj is from the player, check if the player is on cooldown
        if proj_event.from_enemy.is_none() {
            if att_cooldown_query.single().1.is_some() {
                continue;
            }
        }
        let t = if let Some(enemy) = proj_event.from_enemy {
            enemy_transforms
                .get(enemy)
                .unwrap()
                .translation()
                .truncate()
        } else {
            game.player().position.truncate()
        };
        let p = proto_commands.spawn_projectile_from_proto(
            proj_event.projectile.clone(),
            &proto,
            t,
            proj_event.direction,
        );
        if let Some(p) = p {
            if let Some(e) = proj_event.from_enemy {
                commands.entity(p).insert(EnemyProjectile { entity: e });
            }
        }
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
