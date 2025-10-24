use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::{
    attributes::{
        modifiers::ModifyManaEvent, Attack, CurrentMana, ManaRegen, MaxMana, ProjectileSize,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    combat::AttackTimer,
    custom_commands::CommandsExt,
    enemy::Mob,
    player::{
        mage_skills::JustTeleported,
        skills::{PlayerSkills, Skill},
        Player,
    },
    proto::proto_param::ProtoParam,
    GameParam, GameState, Pet,
};

use super::item_upgrades::ArrowSpeedUpgrade;

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
    IceShard,
    Electricity,
    GreenWhip,
    Arrow,
    ThrowingStar,
    IceExplosionAOE,
    SlimeGooProjectile,
    Arc,
    FireAttack,
    TeleportShock,
    Echo,
    SwordProjectile,
    DaggerProjectile1,
    DaggerProjectile2,
    SpearProjectile,
    Bullet,
    HammerProjectile,
    Dart,
}

impl Projectile {
    pub fn is_staff_proj(&self) -> bool {
        match self {
            Projectile::Fireball => true,
            Projectile::GreenWhip => true,
            Projectile::Electricity => true,
            _ => false,
        }
    }
    pub fn is_anchored_to_player_pos(&self) -> bool {
        match self {
            Projectile::Electricity => true,
            Projectile::SwordProjectile => true,
            Projectile::HammerProjectile => true,
            Projectile::DaggerProjectile1 => true,
            Projectile::DaggerProjectile2 => true,
            Projectile::SpearProjectile => true,
            _ => false,
        }
    }
    pub fn is_skill_projectile(&self) -> bool {
        match self {
            Projectile::Arc => true,
            Projectile::Echo => true,
            Projectile::IceExplosionAOE => true,
            _ => false,
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
    pub mana_bar_full: bool,
    pub despawn_on_hit: bool,
}

#[derive(Deserialize, FromReflect, Default, Reflect, Clone, Serialize, Component, Schematic)]
#[reflect(Component, Schematic, Default)]
pub struct ArcProjectileData {
    pub size: Vec2,
    pub col_size: Vec2,
    pub arc: Vec2,
    pub col_points: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct RangedAttackEvent {
    pub projectile: Projectile,
    pub direction: Vec2,
    pub mana_cost: Option<i32>,
    pub from_enemy: bool,
    pub from_entity: Option<Entity>,
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
    pub from_enemy: bool,
    pub from_entity: Option<Entity>,
    pub was_mana_bar_full: bool,
    pub is_followup_proj: bool,
}

#[derive(Component)]
pub struct PetProjectileMarker;

fn handle_ranged_attack_event(
    mut events: EventReader<RangedAttackEvent>,
    player_query: Query<
        (
            Entity,
            &CurrentMana,
            &MaxMana,
            &ManaRegen,
            &PlayerSkills,
            &ProjectileSize,
            Option<&AttackTimer>,
            Option<&JustTeleported>,
        ),
        With<Player>,
    >,
    transforms: Query<&GlobalTransform>,
    game: GameParam,
    mut commands: Commands,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
) {
    for proj_event in events.iter() {
        let (
            _player_e,
            current_mana,
            max_mana,
            mana_regen,
            skills,
            proj_size,
            player_cooldown,
            teleported_option,
        ) = player_query.single();
        // if proj is from the player, check if the player is on cooldown
        if !proj_event.from_enemy
            && proj_event.from_entity.is_none()
            && player_cooldown.is_some()
            && !proj_event.is_followup_proj
        {
            continue;
        }
        if let Some(mana_cost) = proj_event.mana_cost {
            if mana_cost.abs() > current_mana.0 {
                continue;
            }
            modify_mana_event.send(ModifyManaEvent(
                (mana_cost as f32
                    * if skills.has(Skill::DiscountMP) {
                        0.75
                    } else {
                        1.
                    }) as i32,
            ));
        }

        let spawn_transform = if let Some(entity) = proj_event.from_entity {
            transforms.get(entity).unwrap().translation().truncate()
        } else {
            game.player().position.truncate()
        };

        let size = proj_size.get_multiplier();
        commands.spawn(ProjectileSpawnMarker {
            timer: Timer::from_seconds(proj_event.spawn_delay, TimerMode::Once),
            proj: proj_event.projectile.clone(),
            pos: proj_event
                .pos_override
                .map(|v| v * size)
                .unwrap_or(spawn_transform),
            direction: proj_event.direction,
            dmg_override: proj_event.dmg_override,
            from_enemy: proj_event.from_enemy,
            from_entity: proj_event.from_entity,
            was_mana_bar_full: current_mana.0 == max_mana.0,
            is_followup_proj: proj_event.is_followup_proj,
        });

        if proj_event.projectile == Projectile::DaggerProjectile1 {
            commands.spawn(ProjectileSpawnMarker {
                timer: Timer::from_seconds(proj_event.spawn_delay + 0.2, TimerMode::Once),
                proj: Projectile::DaggerProjectile2,
                pos: proj_event
                    .pos_override
                    .map(|v| v * size)
                    .unwrap_or(spawn_transform),
                direction: proj_event.direction,
                dmg_override: proj_event.dmg_override,
                from_enemy: proj_event.from_enemy,
                from_entity: proj_event.from_entity,
                was_mana_bar_full: current_mana.0 == max_mana.0,
                is_followup_proj: false,
            });
        }

        if teleported_option.is_some() {
            modify_mana_event.send(ModifyManaEvent(
                mana_regen.0 + skills.get_count(Skill::MPRegen) * 5,
            ));
        }
    }
}
fn handle_translate_projectiles(
    mut query: Query<(&mut Transform, &ProjectileState), With<Projectile>>,
    speed_modifiers: Query<&ArrowSpeedUpgrade>,
    time: Res<Time>,
) {
    for (mut transform, state) in query.iter_mut() {
        let arrow_speed_upgrade = speed_modifiers
            .get_single()
            .unwrap_or(&ArrowSpeedUpgrade(1.))
            .0;
        let delta = state.direction * (state.speed * arrow_speed_upgrade) * time.delta_seconds();
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
    player: Query<Entity, With<Player>>,
    pet_check: Query<Entity, With<Pet>>,
    mobs: Query<&Mob>,
    asset_server: Res<AssetServer>,
    player_projectile_size: Query<&ProjectileSize, With<Player>>,
) {
    let player_att = player_projectile_size.single();
    for (e, mut proj) in projectiles.iter_mut() {
        proj.timer.tick(time.delta());
        if proj.timer.just_finished() {
            let p = proto_commands.spawn_projectile_from_proto(
                proj.proj.clone(),
                &proto,
                proj.pos,
                proj.direction,
                proj.was_mana_bar_full,
                &asset_server,
                player_att.get_multiplier(),
            );

            if let Some(p) = p {
                if proj.proj.is_anchored_to_player_pos() && !proj.is_followup_proj {
                    let entity = proj.from_entity.unwrap_or(player.single());
                    commands.entity(entity).add_child(p);
                }
                // AUDIO
                if proj.proj == Projectile::Fireball {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::IceStaffCast, 0.4));
                } else if proj.proj == Projectile::Arrow {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::Bow, 0.4));
                } else if proj.proj == Projectile::ThrowingStar {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::Claw, 0.4));
                } else if proj.proj == Projectile::Electricity {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffCast, 0.4));
                }

                if proj.from_enemy {
                    let mob_e = proj.from_entity.unwrap();
                    if let Ok(mob) = mobs.get(mob_e) {
                        commands.entity(p).insert(EnemyProjectile {
                            entity: mob_e,
                            mob: mob.clone(),
                        });
                    }
                }

                if let Some(e) = proj.from_entity {
                    if pet_check.get(e).is_ok() {
                        commands.entity(p).insert(PetProjectileMarker);
                    }
                }

                let mana_full_bonus = if game.has_skill(Skill::MPBarDMG) && proj.was_mana_bar_full {
                    1.25
                } else {
                    1.
                };
                let player_att = game.player_stats.single().0 .0 * mana_full_bonus as i32;
                let computed_dmg = proj.dmg_override.unwrap_or(player_att);
                commands.entity(p).insert(Attack(computed_dmg));
            }
            commands.entity(e).despawn_recursive();
        }
    }
}

//TODO: make global timer resource for this
pub fn handle_reset_proj_hit_enemies_state(
    mut query: Query<&mut ProjectileState>,
    mut timer: Local<Timer>,
    time: Res<Time>,
) {
    if timer.duration().as_secs() == 0 {
        *timer = Timer::from_seconds(1.0, TimerMode::Once);
    }
    timer.tick(time.delta());
    if timer.just_finished() {
        timer.reset();
        for mut state in query.iter_mut() {
            state.hit_entities.clear();
        }
    }
}
