use std::time::Duration;

use crate::attributes::{CurrentHealth, MaxHealth};
use crate::combat::{EnemyDeathEvent, MarkedForDeath};
use crate::custom_commands::CommandsExt;
use crate::enemy::Mob;
use crate::{
    combat::{AttackTimer, HitEvent},
    inputs::CursorPos,
    player::Player,
    proto::proto_param::ProtoParam,
    GameParam,
};
use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};

use super::{
    projectile::{Projectile, RangedAttack, RangedAttackEvent},
    MainHand,
};

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct ClawUpgradeMultiThrow(pub Timer, pub u8);

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct BowUpgradeSpread(pub u8);

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct ArrowSpeedUpgrade(pub f32);

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct FireStaffAOEUpgrade;

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct LightningStaffChainUpgrade;

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct LethalHitUpgrade;

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct BurnOnHitUpgrade;

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct VenomOnHitUpgrade;

#[derive(Component)]
pub struct Burning {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub damage: u8,
}

#[derive(Component)]
pub struct Poisoned {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub damage: u8,
}

pub fn handle_delayed_ranged_attack(
    wep_query: Query<&RangedAttack, With<MainHand>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    game: GameParam,
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    time: Res<Time>,
    mut att_cooldown_query: Query<(&mut ClawUpgradeMultiThrow, Option<&AttackTimer>), With<Player>>,
    mut count: Local<u8>,
) {
    let Ok(ranged_attack) = wep_query.get_single() else {
        return;
    };
    if ranged_attack.0 != Projectile::ThrowingStar {
        return;
    }
    let Ok((mut delayed_ranged_attack, cooldown_option)) = att_cooldown_query.get_single_mut()
    else {
        return;
    };
    if cooldown_option.is_some() && delayed_ranged_attack.0.percent() == 0. {
        *count = 0;
        return;
    }
    if mouse_button_input.pressed(MouseButton::Left) || delayed_ranged_attack.0.percent() != 0. {
        delayed_ranged_attack.0.tick(time.delta());
        if delayed_ranged_attack.0.just_finished() {
            *count += 1;
            ranged_attack_event.send(RangedAttackEvent {
                projectile: ranged_attack.0.clone(),
                direction: (cursor_pos.world_coords.truncate() - game.player().position.truncate())
                    .normalize_or_zero(),
                from_enemy: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: None,
            });

            delayed_ranged_attack.0.reset();
            if *count < delayed_ranged_attack.1 {
                delayed_ranged_attack.0.tick(Duration::from_millis(10));
            }
        }
    }
}
pub fn handle_spread_arrows_attack(
    wep_query: Query<&RangedAttack, With<MainHand>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    game: GameParam,
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    att_cooldown_query: Query<(&BowUpgradeSpread, Option<&AttackTimer>), With<Player>>,
    mut count: Local<u8>,
) {
    let Ok(ranged_attack) = wep_query.get_single() else {
        return;
    };
    if ranged_attack.0 != Projectile::Arrow {
        return;
    }
    let Ok((spread_attack, cooldown_option)) = att_cooldown_query.get_single() else {
        return;
    };
    if cooldown_option.is_none() {
        *count = 0;
    }
    if mouse_button_input.pressed(MouseButton::Left) && *count < spread_attack.0 {
        *count += 1;
        let raw_dir = (cursor_pos.world_coords.truncate() - game.player().position.truncate())
            .normalize_or_zero();
        ranged_attack_event.send(RangedAttackEvent {
            projectile: ranged_attack.0.clone(),
            direction: (raw_dir
                + Vec2::new(
                    0.2 * raw_dir.y.abs() * *count as f32,
                    0.2 * raw_dir.x.abs() * *count as f32,
                ))
            .normalize_or_zero(),
            from_enemy: None,
            is_followup_proj: true,
            mana_cost: None,
            dmg_override: None,
        });
    }
}

pub fn handle_on_hit_upgrades(
    mut hits: EventReader<HitEvent>,
    upgrades: Query<
        (
            Option<&FireStaffAOEUpgrade>,
            Option<&LightningStaffChainUpgrade>,
            Option<&LethalHitUpgrade>,
            Option<&BurnOnHitUpgrade>,
        ),
        With<Player>,
    >,
    proto: ProtoParam,
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    game: GameParam,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth), With<Mob>>,
    mut burn_or_venom_mobs: Query<(Option<&mut Burning>, Option<&mut Poisoned>)>,
    mut elec_count: Local<u8>,
    att_cooldown_query: Query<Option<&AttackTimer>, With<Player>>,
    mut enemy_death_events: EventWriter<EnemyDeathEvent>,
) {
    if *elec_count > 0 && att_cooldown_query.single().is_none() {
        *elec_count = 0;
    }
    for hit in hits.iter() {
        if hit.hit_entity == game.game.player {
            continue;
        }
        let Ok((hit_e, hit_entity_txfm, curr_hp, max_hp)) = mobs.get(hit.hit_entity) else {
            continue;
        };
        let (fire_aoe_option, lightning_chain_option, lethal_option, burn_option) =
            upgrades.single();

        if let Some(_) = lightning_chain_option {
            if hit.hit_with_projectile == Some(Projectile::Electricity) && *elec_count == 0 {
                let Some(nearest_mob_t) = mobs.iter().find(|t| {
                    t.1.translation().distance(hit_entity_txfm.translation()) < 70.
                        && t.0 != hit.hit_entity
                }) else {
                    continue;
                };
                *elec_count += 1;
                proto_commands.spawn_projectile_from_proto(
                    Projectile::Electricity,
                    &proto,
                    hit_entity_txfm.translation().truncate(),
                    (nearest_mob_t.1.translation().truncate()
                        - hit_entity_txfm.translation().truncate())
                    .normalize_or_zero(),
                );
            }
        }

        if let Some(_) = fire_aoe_option {
            if hit.hit_with_projectile == Some(Projectile::Fireball) {
                proto_commands.spawn_projectile_from_proto(
                    Projectile::FireExplosionAOE,
                    &proto,
                    hit_entity_txfm.translation().truncate(),
                    Vec2::ZERO,
                );
            }
        }
        if let Some(_) = lethal_option {
            if curr_hp.0 <= max_hp.0 / 4 {
                commands.entity(hit_e).insert(MarkedForDeath);
                enemy_death_events.send(EnemyDeathEvent {
                    entity: hit_e,
                    enemy_pos: hit_entity_txfm.translation().truncate(),
                });
            }
        }
        let Ok((burning_option, _poisoned_option)) = burn_or_venom_mobs.get_mut(hit.hit_entity)
        else {
            continue;
        };
        if let Some(_) = burn_option {
            if let Some(mut burning) = burning_option {
                (*burning).duration_timer.reset();
            } else {
                commands.entity(hit_e).insert(Burning {
                    tick_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                    duration_timer: Timer::from_seconds(3.0, TimerMode::Once),
                    damage: 1,
                });
            }
        }
    }
}

pub fn handle_burning_ticks(
    mut burning: Query<(Entity, &mut Burning, &mut CurrentHealth)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut burning, mut curr_hp) in burning.iter_mut() {
        burning.duration_timer.tick(time.delta());
        if !burning.duration_timer.just_finished() {
            burning.tick_timer.tick(time.delta());
            if burning.tick_timer.just_finished() {
                curr_hp.0 -= burning.damage as i32;
                burning.tick_timer.reset();
            }
        } else {
            commands.entity(e).remove::<Burning>();
        }
    }
}
