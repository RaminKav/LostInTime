use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use serde::{Deserialize, Serialize};
use strum_macros::{Display, IntoStaticStr};

use crate::{
    attributes::{
        modifiers::ModifyManaEvent, Attack, CurrentMana, ManaRegen, MaxMana, ProjectileSize,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    client::is_not_paused,
    combat::AttackTimer,
    custom_commands::CommandsExt,
    enemy::Mob,
    item::ItemDropDespawnTimer,
    player::{
        mage_skills::JustTeleported,
        skills::{Heirloom, PlayerSkills, RapidfireState},
        Player,
    },
    proto::proto_param::ProtoParam,
    Game, GameParam, GameState, Pet,
};

use super::ammo::Ammo;
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
    ThrowingStarLarge,
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
    FireRing,
    Buckshot,
    Smoke,
    IceWall,
    PoisonCloud,
    HealHearts,
    AttackSpeed,
    Shout,
    GolemSpike,
    LaserBeam,
    PlasmaBall,
    PlasmaExplosion,
    ThornsProjectile,
    ManaOrbProjectile,
    DaggerThrow,
    DaggerSlash,
    Bomb,
    BombExplosion,
    Lightning,
    FuryKunai,
    SpearGravity,
    SpinAttack,
    IceFloor,
    CrowFeather,
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
            Projectile::ThornsProjectile => true,
            Projectile::LaserBeam => true,
            Projectile::DaggerSlash => true,
            Projectile::SpinAttack => true,
            Projectile::HealHearts => true,
            _ => false,
        }
    }
    /// Rotation offset in radians for projectiles whose sprite is drawn at 45° in the sheet (e.g. kunai, feathers).
    pub fn get_custom_rotation(&self) -> Option<f32> {
        match self {
            Projectile::FuryKunai => Some(-0.7853982),   // -45°
            Projectile::DaggerThrow => Some(-0.7853982), // -45°
            Projectile::CrowFeather => Some(-0.7853982), // -45°
            _ => None,
        }
    }
    pub fn is_skill_projectile(&self) -> bool {
        match self {
            Projectile::Arc => true,
            Projectile::Echo => true,
            Projectile::IceExplosionAOE => true,
            Projectile::FireRing => true,
            Projectile::IceWall => true,
            Projectile::Shout => true,
            Projectile::LaserBeam => true,
            Projectile::BombExplosion => true,
            Projectile::Bomb => true,
            Projectile::DaggerSlash => true,
            Projectile::DaggerThrow => true,
            Projectile::Lightning => true,
            Projectile::FuryKunai => true,
            Projectile::SpinAttack => true,
            Projectile::SpearGravity => true,
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
                handle_translate_projectiles.run_if(is_not_paused),
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

/// Component to track the target position for bomb projectiles
#[derive(Component)]
pub struct BombTarget {
    pub target_pos: Vec2,
}

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
    rapidfire_state: Query<&RapidfireState, With<Player>>,
    transforms: Query<&GlobalTransform>,
    game: Res<Game>,
    mut commands: Commands,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    mut ammo_query: Query<&mut Ammo>,
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
        // Skip this check for skill projectiles (they have their own cooldown system)
        if !proj_event.from_enemy
            && proj_event.from_entity.is_none()
            && player_cooldown.is_some()
            && !proj_event.is_followup_proj
            && !proj_event.projectile.is_skill_projectile()
        {
            continue;
        }
        // Ammo gate for non-staff player shots
        // Skip ammo consumption if Rapidfire is active (duration hasn't finished)
        let is_rapidfire_active = rapidfire_state
            .get_single()
            .map(|r| !r.duration.finished())
            .unwrap_or(false);
        if !proj_event.from_enemy
            && proj_event.from_entity.is_none()
            && !proj_event.projectile.is_staff_proj()
            && !proj_event.is_followup_proj
            && !is_rapidfire_active
            && !proj_event.projectile.is_skill_projectile()
        // Don't consume ammo during Rapidfire or using skill projectiles
        {
            if let Some(main_hand) = game.player_state.main_hand_slot.clone() {
                let held_e = main_hand.entity;
                if let Ok(mut ammo) = ammo_query.get_mut(held_e) {
                    if !ammo.can_fire() {
                        ammo.start_reload();
                        continue;
                    }
                    // consume one round, will auto-reload if hits zero
                    ammo.use_ammo_and_maybe_reload();
                }
            }
        }

        if let Some(mana_cost) = proj_event.mana_cost {
            if mana_cost.abs() > current_mana.0 {
                continue;
            }
            modify_mana_event.send(ModifyManaEvent(
                -(mana_cost as f32
                    * if skills.has(Heirloom::DiscountMP) {
                        0.75
                    } else {
                        1.
                    }) as i32,
            ));
        }

        let spawn_transform = if let Some(entity) = proj_event.from_entity {
            transforms
                .get(entity)
                .unwrap_or(&GlobalTransform::default())
                .translation()
                .truncate()
        } else {
            game.player_state.position.truncate()
        };

        let size = if proj_event.projectile.is_anchored_to_player_pos() {
            proj_size.get_multiplier()
        } else {
            1.
        };
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
                mana_regen.0 + skills.get_count(Heirloom::MPRegen) * 5,
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
                    // Check if parent entity still exists before adding child
                    if let Some(mut entity_commands) = commands.get_entity(entity) {
                        entity_commands.add_child(p);
                    }
                }
                // AUDIO
                if proj.proj == Projectile::Fireball || proj.proj == Projectile::FireRing {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::IceStaffCast, 0.2));
                } else if proj.proj == Projectile::Arrow {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::Bow, 0.2));
                } else if proj.proj == Projectile::ThrowingStar
                    || proj.proj == Projectile::ThrowingStarLarge
                {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::Claw, 0.2));
                } else if proj.proj == Projectile::Electricity
                    || proj.proj == Projectile::PlasmaBall
                {
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffCast, 0.2));
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

                let player_att = game.player_stats.single().0 .0;
                let computed_dmg = proj.dmg_override.unwrap_or(player_att);
                info!(
                    "Spawned projectile {:?} with dmg {}",
                    proj.proj, computed_dmg
                );
                let despawn_secs = if proj.from_enemy && proj.proj == Projectile::CrowFeather {
                    0.3
                } else {
                    5.0
                };
                commands
                    .entity(p)
                    .insert(Attack(computed_dmg))
                    .insert(ItemDropDespawnTimer(Timer::from_seconds(
                        despawn_secs,
                        TimerMode::Once,
                    )));
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
