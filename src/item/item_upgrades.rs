use rand::Rng;

use crate::animations::player_sprite::PlayerAnimation;
use crate::animations::AttackEvent;
use crate::assets::Graphics;
use crate::attributes::{
    modifiers::ModifyHealthEvent, CurrentHealth, CurrentMana, ItemAttributes, ProjectileSize,
};
use crate::audio::{AudioSoundEffect, SoundSpawner};
use crate::blessings::OwnedBlessings;
use crate::custom_commands::CommandsExt;
use crate::enemy::Mob;
use crate::inputs::{attack_aim_direction, AttackAutoTargetState, AutoAttackState};
use crate::aim::ManualAimOverride;
use crate::item::ammo::Ammo;
use crate::item::WorldObject;
use crate::player::combat_heirlooms::hit_is_weapon_damage;
use crate::player::mage_skills::spawn_ice_explosion_hitbox;
use crate::player::skills::{Heirloom, PlayerSkills};
use crate::status_effects::{
    try_add_slow_stacks, Burning, Frail, MobStatusEffects, StatusEffect, StatusEffectEvent,
};
use crate::Game;
use crate::{
    combat::{AttackTimer, HitEvent, LifestealEvent},
    cursor::CursorPos,
    player::Player,
    proto::proto_param::ProtoParam,
    GameParam,
};
use bevy::prelude::*;

use super::{
    projectile::{Projectile, RangedAttack, RangedAttackEvent},
    MainHand,
};

#[derive(Component, Reflect, FromReflect, Default, Clone)]
#[reflect(Component)]
pub struct ClawUpgradeMultiThrow(pub Timer, pub u8);

#[derive(Component, Reflect, FromReflect, Default, Clone)]
#[reflect(Component)]
pub struct BowUpgradeSpread(pub u8);

#[derive(Component, Reflect, FromReflect, Default, Clone)]
#[reflect(Component)]
pub struct ArrowSpeedUpgrade(pub f32);

// Local state for throttling ice explosions per frame
#[derive(Default)]
pub struct IceExplosionThrottle {
    count: u8,
    sound_played: bool,
}

/// Fires the claw's bonus throwing stars after a real attack.
///
/// This is driven by [`AttackEvent`] rather than by polling `AttackTimer` presence. The old
/// approach free-ran whenever the player had no `AttackTimer` (e.g. mid/after a movement skill
/// like Roll or SpinAttack, or during a frame drop) and auto-attack was on, spamming stars.
/// Keying off the actual attack event means a follow-up volley can only ever start as the
/// direct result of an attack, so it cannot spam.
pub fn handle_delayed_ranged_attack(
    wep_query: Query<(&RangedAttack, Option<&Ammo>), With<MainHand>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut attack_events: EventReader<AttackEvent>,
    game: GameParam,
    cursor_pos: Res<CursorPos>,
    auto_target: Res<AttackAutoTargetState>,
    manual_aim: Res<ManualAimOverride>,
    enemies: Query<&GlobalTransform, With<Mob>>,
    time: Res<Time>,
    mut multi_throw_query: Query<&mut ClawUpgradeMultiThrow, With<Player>>,
    mut remaining: Local<u8>,
) {
    let attacked = attack_events.iter().count() > 0;

    let Ok((ranged_attack, ammo_option)) = wep_query.get_single() else {
        *remaining = 0;
        return;
    };
    if let Some(ammo) = ammo_option {
        if ammo.reloading {
            return;
        }
    }
    let Ok(mut multi_throw) = multi_throw_query.get_single_mut() else {
        return;
    };
    if ranged_attack.0 == Projectile::Arrow || ranged_attack.0 == Projectile::Electricity {
        return;
    }
    let num_bonus_projs = multi_throw.1
        + if ranged_attack.0 == Projectile::ThrowingStar {
            1
        } else {
            0
        };
    if num_bonus_projs == 0 {
        return;
    }

    // Begin a fresh volley only on a real attack this frame.
    if attacked {
        *remaining = num_bonus_projs;
        multi_throw.0.reset();
    }

    if *remaining > 0 {
        multi_throw.0.tick(time.delta());
        if multi_throw.0.just_finished() {
            *remaining -= 1;
            ranged_attack_event.send(RangedAttackEvent {
                projectile: ranged_attack.0.clone(),
                direction: attack_aim_direction(
                    game.player().position.truncate(),
                    cursor_pos.world_coords.truncate(),
                    auto_target.0,
                    manual_aim.active,
                    &enemies,
                ),
                from_enemy: false,
                is_followup_proj: true,
                mana_cost: None,
                mana_cost_heirloom: None,
                from_entity: None,
                dmg_override: None,
                pos_override: None,
                spawn_delay: 0.05,
            });
            multi_throw.0.reset();
        }
    }
}
pub fn handle_spread_arrows_attack(
    wep_query: Query<&RangedAttack, With<MainHand>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    game: GameParam,
    mouse_button_input: Res<Input<MouseButton>>,
    auto_attack: Res<AutoAttackState>,
    auto_target: Res<AttackAutoTargetState>,
    manual_aim: Res<ManualAimOverride>,
    cursor_pos: Res<CursorPos>,
    enemies: Query<&GlobalTransform, With<Mob>>,
    att_cooldown_query: Query<
        (&BowUpgradeSpread, &PlayerAnimation, Option<&AttackTimer>),
        With<Player>,
    >,
    mut count: Local<u8>,
) {
    let Ok(ranged_attack) = wep_query.get_single() else {
        return;
    };
    if ranged_attack.0 != Projectile::Arrow {
        return;
    }
    let Ok((spread_attack, anim, cooldown_option)) = att_cooldown_query.get_single() else {
        return;
    };
    if cooldown_option.is_none() && !anim.is_shooting_bow() {
        *count = 0;
    }
    if anim.is_shooting_bow()
        && (mouse_button_input.pressed(MouseButton::Left) || auto_attack.0)
        && *count < spread_attack.0
    {
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
        let raw_dir = attack_aim_direction(
            game.player().position.truncate(),
            cursor_pos.world_coords.truncate(),
            auto_target.0,
            manual_aim.active,
            &enemies,
        );

        let new_dir = rotate(raw_dir, spread_factor * if flip { -1. } else { 1. });

        ranged_attack_event.send(RangedAttackEvent {
            projectile: ranged_attack.0.clone(),
            direction: new_dir,
            from_enemy: false,
            is_followup_proj: true,
            mana_cost: None,
            mana_cost_heirloom: None,
            from_entity: None,
            dmg_override: None,
            pos_override: None,
            spawn_delay: 0.36,
        });
    }
}

pub fn handle_on_hit_upgrades(
    mut hits: EventReader<HitEvent>,
    mut upgrades: Query<
        (
            Entity,
            &PlayerSkills,
            &ProjectileSize,
            Option<&AttackTimer>,
            &mut CurrentMana,
        ),
        With<Player>,
    >,
    proto: ProtoParam,
    mut commands: Commands,
    game: Res<Game>,
    mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    mut burn_or_venom_mobs: Query<&mut MobStatusEffects>,
    mut elec_count: Local<u8>,
    graphics: Res<Graphics>,
    player_att_blessings: Query<(&ItemAttributes, &OwnedBlessings, &CurrentHealth), With<Player>>,
    asset_server: Res<AssetServer>,
    mut events: ParamSet<(
        EventWriter<RangedAttackEvent>,
        EventWriter<StatusEffectEvent>,
        EventWriter<LifestealEvent>,
        EventWriter<ModifyHealthEvent>,
    )>,
    mut throttle: Local<IceExplosionThrottle>, // Track explosions spawned this frame
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
) {
    // Reset counters at start of frame
    throttle.count = 0;
    throttle.sound_played = false;

    let (player_e, skills, projectile_size, att_cooldown, mut current_mana) = upgrades.single_mut();
    if *elec_count > 0 && att_cooldown.is_none() {
        *elec_count = 0;
    }
    let Ok((player_attributes, player_blessings, current_hp)) = player_att_blessings.get_single()
    else {
        return;
    };
    for hit in hits.iter() {
        // Skip DoT tick damage (e.g. poison) so it does not re-trigger on-hit effects.
        if matches!(hit.from_heirloom_effect, Some(Heirloom::PoisonStacks)) {
            continue;
        }
        let mut rng = rand::thread_rng();

        if hit.hit_entity == player_e {
            continue;
        }
        let Ok((hit_e, hit_entity_txfm)) = mobs.get(hit.hit_entity) else {
            continue;
        };
        let kevin_chance = player_blessings.get_kevin_self_damage_chance();
        if current_hp.0 > 1 && kevin_chance > 0.0 && rng.gen_bool(kevin_chance as f64) {
            events.p3().send(ModifyHealthEvent(-1));
        }
        if skills.has(Heirloom::IncreaseProjectileCount)
            && hit.hit_with_projectile == Some(Projectile::Electricity)
            && *elec_count == 0
        {
            let Some(nearest_mob_t) = mobs.iter().find(|t| {
                t.1.translation().distance(hit_entity_txfm.translation()) < 70.
                    && t.0 != hit.hit_entity
            }) else {
                continue;
            };
            *elec_count += 1;
            commands.spawn_projectile_from_proto(
                Projectile::Electricity,
                &proto,
                hit_entity_txfm.translation().truncate(),
                (nearest_mob_t.1.translation().truncate()
                    - hit_entity_txfm.translation().truncate())
                .normalize_or_zero(),
                false,
                &asset_server,
                1. + player_attributes.size.value as f32 / 100.,
            );
            events.p0().send(RangedAttackEvent {
                projectile: Projectile::Electricity,
                direction: (nearest_mob_t.1.translation().truncate()
                    - hit_entity_txfm.translation().truncate())
                .normalize_or_zero(),
                from_enemy: false,
                from_entity: None,
                is_followup_proj: true,
                mana_cost: None,
                mana_cost_heirloom: None,
                dmg_override: Some(hit.damage),
                pos_override: Some(hit_entity_txfm.translation().truncate()),
                spawn_delay: 0.1,
            });
        }
        let Some(main_hand) = game.player_state.main_hand_slot.clone() else {
            continue;
        };
        if hit_is_weapon_damage(hit)
            && skills.has(Heirloom::IceStaffAoE)
            && rng.gen_bool((skills.get_count(Heirloom::IceStaffAoE) as f64 * 0.07).clamp(0., 1.))
        {
            let mana_cost = Heirloom::IceStaffAoE.get_mana_cost();
            if current_mana.0 >= mana_cost {
                current_mana.0 -= mana_cost;
                trigger_counts.record_mana(Heirloom::IceStaffAoE, mana_cost);
                // Throttle explosions per frame to prevent lag when hitting many enemies
                const MAX_ICE_EXPLOSIONS_PER_FRAME: u8 = 8;
                if throttle.count < MAX_ICE_EXPLOSIONS_PER_FRAME {
                    trigger_counts.increment(Heirloom::IceStaffAoE);
                    throttle.count += 1;
                    spawn_ice_explosion_hitbox(
                        &mut commands,
                        &graphics,
                        hit_entity_txfm.translation(),
                        hit.damage / 2,
                        projectile_size.get_multiplier(),
                    );
                    // Only play sound once per frame to avoid audio spam
                    if !throttle.sound_played {
                        throttle.sound_played = true;
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.2));
                    }
                }
            }
        }
        let Ok(mut status) = burn_or_venom_mobs.get_mut(hit.hit_entity) else {
            continue;
        };
        // Calculate poison chance with blessing bonus
        let blessing_poison = player_blessings.get_poison_bonus_chance();
        let bonus_stack = if rng.gen_bool(blessing_poison) { 1 } else { 0 };
        let is_dart = hit.hit_with_projectile.clone().unwrap_or_default() == Projectile::Dart;
        let mut stacks_to_apply = skills.roll_poison_stacks_from_chance(&mut rng);
        if is_dart && stacks_to_apply == 0 {
            stacks_to_apply = 1;
        }
        if is_dart || stacks_to_apply > 0 {
            if let Some(burning) = status.burning.as_mut() {
                // Increment stacks and reset duration
                burning.stacks = burning
                    .stacks
                    .saturating_add(stacks_to_apply as u128 + bonus_stack as u128);
                burning.duration_timer.reset();
                let stacks = burning.stacks as i32;
                events.p1().send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Poison,
                    num_stacks: stacks,
                });
            } else if Heirloom::PoisonStacks.is_obj_valid(main_hand.get_obj()) {
                let duration_bonus = skills.get_count(Heirloom::PoisonDuration) as f32 * 0.5 + 1.;
                let initial_stacks = stacks_to_apply.max(1) as u128;
                status.burning = Some(Burning {
                    tick_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                    duration_timer: Timer::from_seconds(3.0 * duration_bonus, TimerMode::Once),
                    stacks: initial_stacks,
                });
                events.p1().send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Poison,
                    num_stacks: initial_stacks as i32,
                });
            }
        }
        if main_hand.get_obj() == WorldObject::Hammer
            || (skills.has(Heirloom::FrailStacks)
                && rng.gen_bool(
                    (skills.get_count(Heirloom::FrailStacks) as f64 * 0.25).clamp(0.0, 0.99),
                ))
        {
            if let Some(frail_stacks) = status.frail.as_mut() {
                if frail_stacks.num_stacks < 3
                    && Heirloom::FrailStacks.is_obj_valid(main_hand.get_obj())
                {
                    frail_stacks.num_stacks += 1;
                    frail_stacks.timer.reset();
                    let stacks = frail_stacks.num_stacks as i32;
                    events.p1().send(StatusEffectEvent {
                        entity: hit_e,
                        effect: StatusEffect::Frail,
                        num_stacks: stacks,
                    });
                }
            } else {
                status.frail = Some(Frail {
                    num_stacks: 1,
                    timer: Timer::from_seconds(1.2, TimerMode::Repeating),
                });
                events.p1().send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Frail,
                    num_stacks: 1,
                });
            }
        }
        if main_hand.get_obj() == WorldObject::IceStaff
            || rng.gen_bool(skills.calculate_freeze_chance().clamp(0., 1.))
        {
            try_add_slow_stacks(hit_e, status.as_mut(), &mut events.p1());
        }

        events.p2().send(LifestealEvent {
            thorns_lifesteal_stacks: 0,
            is_direct_player_damage: true,
        });
    }
}
