use std::time::Duration;

use crate::attributes::{CurrentHealth, MaxHealth};
use crate::combat::{EnemyDeathEvent, MarkedForDeath};
use crate::combat_helpers::spawn_one_time_aseprite_collider;
use crate::custom_commands::CommandsExt;
use crate::enemy::Mob;
use crate::player::skills::{PlayerSkills, Skill};
use crate::player::teleport::{spawn_ice_explosion_hitbox, IceExplosionDmg, IceFloor};
use crate::status_effects::{
    try_add_slow_stacks, Burning, Frail, Poisoned, Slow, StatusEffect, StatusEffectEvent,
};
use crate::world::y_sort::YSort;
use crate::{
    combat::{AttackTimer, HitEvent},
    inputs::CursorPos,
    player::Player,
    proto::proto_param::ProtoParam,
    GameParam,
};
use bevy::prelude::*;
use bevy_aseprite::anim::AsepriteAnimation;
use bevy_aseprite::Aseprite;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};
use bevy_rapier2d::prelude::Collider;

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
pub struct VenomOnHitUpgrade;

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
                pos_override: None,
                spawn_delay: 0.05,
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
        let rotate = |val: Vec2, angle: f32| -> Vec2 {
            let cos_angle = angle.cos();
            let sin_angle = angle.sin();
            Vec2::new(
                val.x * cos_angle - val.y * sin_angle,
                val.x * sin_angle + val.y * cos_angle,
            )
        };
        let spread_factor = 0.2 * (f32::floor(*count as f32 / 2.0) + 1.);
        *count += 1;
        let flip = *count % 2 == 0;
        let raw_dir = (cursor_pos.world_coords.truncate() - game.player().position.truncate())
            .normalize_or_zero();

        let new_dir = rotate(raw_dir, spread_factor * if flip { -1. } else { 1. });

        ranged_attack_event.send(RangedAttackEvent {
            projectile: ranged_attack.0.clone(),
            direction: new_dir,
            from_enemy: None,
            is_followup_proj: true,
            mana_cost: None,
            dmg_override: None,
            pos_override: None,
            spawn_delay: 0.36,
        });
    }
}

pub fn handle_on_hit_upgrades(
    mut hits: EventReader<HitEvent>,
    upgrades: Query<&PlayerSkills, With<Player>>,
    proto: ProtoParam,
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    game: GameParam,
    mobs: Query<(Entity, &Mob, &GlobalTransform, &CurrentHealth, &MaxHealth), With<Mob>>,
    mut burn_or_venom_mobs: Query<(
        Option<&mut Burning>,
        Option<&mut Poisoned>,
        Option<&mut Frail>,
        Option<&mut Slow>,
    )>,
    mut elec_count: Local<u8>,
    att_cooldown_query: Query<Option<&AttackTimer>, With<Player>>,
    mut enemy_death_events: EventWriter<EnemyDeathEvent>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut status_event: EventWriter<StatusEffectEvent>,
    asset_server: Res<AssetServer>,
) {
    if *elec_count > 0 && att_cooldown_query.single().is_none() {
        *elec_count = 0;
    }
    for hit in hits.iter() {
        if hit.hit_entity == game.game.player {
            continue;
        }
        let Ok((hit_e, mob, hit_entity_txfm, curr_hp, max_hp)) = mobs.get(hit.hit_entity) else {
            continue;
        };
        let skills = upgrades.single();

        if skills.has(Skill::ChainLightning)
            && hit.hit_with_projectile == Some(Projectile::Electricity)
            && *elec_count == 0
        {
            let Some(nearest_mob_t) = mobs.iter().find(|t| {
                t.2.translation().distance(hit_entity_txfm.translation()) < 70.
                    && t.0 != hit.hit_entity
            }) else {
                continue;
            };
            *elec_count += 1;
            proto_commands.spawn_projectile_from_proto(
                Projectile::Electricity,
                &proto,
                hit_entity_txfm.translation().truncate(),
                (nearest_mob_t.2.translation().truncate()
                    - hit_entity_txfm.translation().truncate())
                .normalize_or_zero(),
                false,
            );
            ranged_attack_event.send(RangedAttackEvent {
                projectile: Projectile::Electricity,
                direction: (nearest_mob_t.2.translation().truncate()
                    - hit_entity_txfm.translation().truncate())
                .normalize_or_zero(),
                from_enemy: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: Some(hit.damage / 4),
                pos_override: Some(hit_entity_txfm.translation().truncate()),
                spawn_delay: 0.1,
            });
        }
        let Some(main_hand) = game.player().main_hand_slot else {
            continue;
        };
        if skills.has(Skill::IceStaffAoE) && hit.hit_with_projectile == Some(Projectile::Fireball) {
            spawn_ice_explosion_hitbox(
                &mut commands,
                &asset_server,
                hit_entity_txfm.translation(),
                hit.damage / 3,
            );
        }
        if skills.has(Skill::IceStaffFloor) && hit.hit_with_projectile == Some(Projectile::Fireball)
        {
            let ice = spawn_one_time_aseprite_collider(
                &mut commands,
                Transform::from_translation(hit_entity_txfm.translation()),
                6.5,
                hit.damage / 5,
                Collider::capsule(Vec2::ZERO, Vec2::ZERO, 14.),
                asset_server.load::<Aseprite, _>(IceFloor::PATH),
                AsepriteAnimation::from(IceFloor::tags::ICE_FLOOR),
                true,
            );
            commands
                .entity(ice)
                .insert(YSort(-0.1))
                .insert(IceExplosionDmg);
        }
        if skills.has(Skill::LethalBlow)
            && curr_hp.0 <= max_hp.0 / 5
            && !mob.is_boss()
            && Skill::LethalBlow.is_obj_valid(main_hand.get_obj())
        {
            commands.entity(hit_e).insert(MarkedForDeath);
            enemy_death_events.send(EnemyDeathEvent {
                entity: hit_e,
                enemy_pos: hit_entity_txfm.translation().truncate(),
                killed_by_crit: false,
            });
        }
        let Ok((burning_option, _poisoned_option, frailed_option, mut slowed_option)) =
            burn_or_venom_mobs.get_mut(hit.hit_entity)
        else {
            continue;
        };
        if skills.has(Skill::PoisonStacks) {
            if let Some(mut burning) = burning_option {
                burning.duration_timer.reset();
            } else if Skill::PoisonStacks.is_obj_valid(main_hand.get_obj()) {
                let duration_bonus = if skills.has(Skill::PoisonDuration) {
                    1.5
                } else {
                    1.
                };
                let damage_bonus = skills.get_count(Skill::PoisonStrength) as u8;
                commands.entity(hit_e).insert(Burning {
                    tick_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                    duration_timer: Timer::from_seconds(3.0 * duration_bonus, TimerMode::Once),
                    damage: 1 + damage_bonus,
                });
                status_event.send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Poison,
                    num_stacks: 1,
                });
            }
        }
        if skills.has(Skill::FrailStacks) {
            if let Some(mut frail_stacks) = frailed_option {
                if frail_stacks.num_stacks < 3
                    && Skill::FrailStacks.is_obj_valid(main_hand.get_obj())
                {
                    frail_stacks.num_stacks += 1;
                    frail_stacks.timer.reset();
                    status_event.send(StatusEffectEvent {
                        entity: hit_e,
                        effect: StatusEffect::Frail,
                        num_stacks: frail_stacks.num_stacks as i32,
                    });
                }
            } else {
                commands.entity(hit_e).insert(Frail {
                    num_stacks: 1,
                    timer: Timer::from_seconds(1.2, TimerMode::Repeating),
                });
                status_event.send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Frail,
                    num_stacks: 1,
                });
            }
        }
        if skills.has(Skill::SlowStacks) {
            try_add_slow_stacks(
                hit_e,
                &mut commands,
                &mut status_event,
                slowed_option.as_deref_mut(),
            );
        }
    }
}
