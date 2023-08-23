use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::{
    combat::AttackTimer,
    custom_commands::CommandsExt,
    enemy::Mob,
    inventory::{Inventory, InventoryPlugin},
    player::Player,
    proto::proto_param::ProtoParam,
    GameParam, GameState,
};

use super::{item_actions::ConsumableItem, item_upgrades::ArrowSpeedUpgrade, WorldObject};

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
    Debug,
    PartialEq,
    Eq,
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
    ThrowingStar,
    FireExplosionAOE,
}

impl Projectile {
    fn get_world_object(&self) -> WorldObject {
        match self {
            Projectile::Fireball => WorldObject::Fireball,
            Projectile::Arrow => WorldObject::Arrow,
            Projectile::ThrowingStar => WorldObject::ThrowingStar,
            _ => panic!("Projectile {:?} not implemented", self),
        }
    }
}
#[derive(Deserialize, FromReflect, Default, Reflect, Clone, Serialize, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct ProjectileState {
    pub speed: f32,
    pub direction: Vec2,
    pub hit_entities: Vec<Entity>,
    pub spawn_offset: Vec2,
    pub rotating: bool,
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
    pub is_followup_proj: bool,
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
    mut inv: Query<&mut Inventory>,
) {
    for proj_event in events.iter() {
        // if proj is from the player, check if the player is on cooldown
        if proj_event.from_enemy.is_none() {
            if att_cooldown_query.single().1.is_some() && !proj_event.is_followup_proj {
                continue;
            }
        }
        if proto
            .get_component::<ConsumableItem, _>(proj_event.projectile.clone())
            .is_some()
        {
            if let Some(proj_slot) = InventoryPlugin::get_slot_for_item_in_container(
                &inv.single().items,
                &proj_event.projectile.get_world_object(),
            ) {
                let held_item_option = inv.single().items.items[proj_slot].clone();
                inv.single_mut().items.items[proj_slot] =
                    held_item_option.unwrap().modify_count(-1);
            } else {
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
    mut query: Query<(&mut Transform, &ProjectileState, &Projectile), With<Projectile>>,
    speed_modifiers: Query<&ArrowSpeedUpgrade>,
) {
    for (mut transform, state, proj) in query.iter_mut() {
        let arrow_speed_upgrade = if proj == &Projectile::Arrow {
            speed_modifiers
                .get_single()
                .unwrap_or(&ArrowSpeedUpgrade(0.))
                .0
        } else {
            0.
        };
        let delta = state.direction * (state.speed + arrow_speed_upgrade);
        transform.translation += delta.extend(0.0);
    }
}
