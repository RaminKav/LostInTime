use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::{
    attributes::{modifiers::ModifyManaEvent, Attack, Mana, ManaRegen},
    combat::AttackTimer,
    custom_commands::CommandsExt,
    enemy::Mob,
    inventory::Inventory,
    player::{
        skills::{PlayerSkills, Skill},
        teleport::JustTeleported,
        Player,
    },
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
    SlimeGooProjectile,
    Arc,
    FireAttack,
    TeleportShock,
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
    pub mana_cost: Option<i32>,
    pub from_enemy: Option<Entity>,
    pub is_followup_proj: bool,
    pub dmg_override: Option<i32>,
    pub pos_override: Option<Vec2>,
    pub spawn_delay: f32,
}

#[derive(Clone, Component)]
pub struct EnemyProjectile {
    pub entity: Entity,
    pub mob: Mob,
}

impl Plugin for RangedAttackPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<RangedAttackEvent>().add_systems(
            (
                handle_ranged_attack_event,
                handle_translate_projectiles,
                handle_spawn_projectiles_after_delay,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}

#[derive(Component)]
pub struct ProjectileSpawnMarker {
    pub timer: Timer,
    pub proj: Projectile,
    pub pos: Vec2,
    pub direction: Vec2,
    pub dmg_override: Option<i32>,
    pub from_enemy: Option<Entity>,
}

fn handle_ranged_attack_event(
    mut events: EventReader<RangedAttackEvent>,
    player_query: Query<
        (
            Entity,
            &Mana,
            &ManaRegen,
            &PlayerSkills,
            Option<&AttackTimer>,
            Option<&JustTeleported>,
        ),
        With<Player>,
    >,
    enemy_transforms: Query<(&GlobalTransform, &Mob), With<Mob>>,
    game: GameParam,
    proto: ProtoParam,
    mut commands: Commands,
    mut inv: Query<&mut Inventory>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
) {
    for proj_event in events.iter() {
        let (_player_e, mana, mana_regen, skills, player_cooldown, teleported_option) =
            player_query.single();
        // if proj is from the player, check if the player is on cooldown
        if proj_event.from_enemy.is_none()
            && player_cooldown.is_some()
            && !proj_event.is_followup_proj
        {
            continue;
        }
        if let Some(mana_cost) = proj_event.mana_cost {
            if mana_cost.abs() > mana.current {
                continue;
            }
            modify_mana_event.send(ModifyManaEvent(mana_cost));
        }
        if proto
            .get_component::<ConsumableItem, _>(proj_event.projectile.clone())
            .is_some()
        {
            if let Some(proj_slot) = inv
                .single()
                .items
                .get_slot_for_item_in_container(&proj_event.projectile.get_world_object())
            {
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
                .0
                .translation()
                .truncate()
        } else {
            game.player().position.truncate()
        };
        commands.spawn(ProjectileSpawnMarker {
            timer: Timer::from_seconds(proj_event.spawn_delay, TimerMode::Once),
            proj: proj_event.projectile.clone(),
            pos: proj_event.pos_override.unwrap_or(t),
            direction: proj_event.direction,
            dmg_override: proj_event.dmg_override,
            from_enemy: proj_event.from_enemy,
        });

        if teleported_option.is_some() {
            modify_mana_event.send(ModifyManaEvent(
                mana_regen.0 + skills.get_count(Skill::MPRegen) * 5,
            ));
        }
    }
}
fn handle_translate_projectiles(
    mut query: Query<(&mut Transform, &ProjectileState, &Projectile), With<Projectile>>,
    speed_modifiers: Query<&ArrowSpeedUpgrade>,
    time: Res<Time>,
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
        let delta = state.direction * (state.speed + arrow_speed_upgrade) * time.delta_seconds();
        transform.translation += delta.extend(0.0);
    }
}

fn handle_spawn_projectiles_after_delay(
    mut projectiles: Query<(Entity, &mut ProjectileSpawnMarker)>,
    time: Res<Time>,
    proto: ProtoParam,
    mut proto_commands: ProtoCommands,
    game: GameParam,
    mut commands: Commands,
    enemy_transforms: Query<(&GlobalTransform, &Mob), With<Mob>>,
) {
    for (e, mut proj) in projectiles.iter_mut() {
        proj.timer.tick(time.delta());
        if proj.timer.just_finished() {
            let p = proto_commands.spawn_projectile_from_proto(
                proj.proj.clone(),
                &proto,
                proj.pos,
                proj.direction,
            );
            if let Some(p) = p {
                if let Some(e) = proj.from_enemy {
                    commands.entity(p).insert(EnemyProjectile {
                        entity: e,
                        mob: enemy_transforms.get(e).unwrap().1.clone(),
                    });
                }
                commands.entity(p).insert(Attack(
                    proj.dmg_override
                        .unwrap_or(game.calculate_player_damage(0).0 as i32),
                ));
            }
            commands.entity(e).despawn_recursive();
        }
    }
}
