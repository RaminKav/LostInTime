use rand::Rng;
use std::time::Duration;

use crate::animations::player_sprite::PlayerAnimation;
use crate::attributes::modifiers::ModifyHealthEvent;
use crate::attributes::ItemAttributes;
use crate::combat_helpers::spawn_one_time_aseprite_collider;
use crate::custom_commands::CommandsExt;
use crate::enemy::Mob;
use crate::item::WorldObject;
use crate::player::mage_skills::{spawn_ice_explosion_hitbox, IceExplosionDmg, IceFloor};
use crate::player::skills::{Heirloom, PlayerSkills};
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

// Local state for throttling ice explosions per frame
#[derive(Default)]
pub struct IceExplosionThrottle {
    count: u8,
    sound_played: bool,
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

    let Ok((mut delayed_ranged_attack, cooldown_option)) = att_cooldown_query.get_single_mut()
    else {
        return;
    };
    if cooldown_option.is_some() && delayed_ranged_attack.0.percent() == 0. {
        *count = 0;
        return;
    }
    if ranged_attack.0 == Projectile::Arrow || ranged_attack.0 == Projectile::Electricity {
        return;
    }
    // TODO: add custom delays per proj type
    if mouse_button_input.pressed(MouseButton::Left) || delayed_ranged_attack.0.percent() != 0. {
        delayed_ranged_attack.0.tick(time.delta());
        if delayed_ranged_attack.0.just_finished() {
            *count += 1;
            ranged_attack_event.send(RangedAttackEvent {
                projectile: ranged_attack.0.clone(),
                direction: (cursor_pos.world_coords.truncate() - game.player().position.truncate())
                    .normalize_or_zero(),
                from_enemy: false,
                is_followup_proj: true,
                mana_cost: None,
                from_entity: None,
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
        && mouse_button_input.pressed(MouseButton::Left)
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
        let raw_dir = (cursor_pos.world_coords.truncate() - game.player().position.truncate())
            .normalize_or_zero();

        let new_dir = rotate(raw_dir, spread_factor * if flip { -1. } else { 1. });

        ranged_attack_event.send(RangedAttackEvent {
            projectile: ranged_attack.0.clone(),
            direction: new_dir,
            from_enemy: false,
            is_followup_proj: true,
            mana_cost: None,
            from_entity: None,
            dmg_override: None,
            pos_override: None,
            spawn_delay: 0.36,
        });
    }
}

pub fn handle_on_hit_upgrades(
    mut hits: EventReader<HitEvent>,
    upgrades: Query<
        (
            &PlayerSkills,
            &GlobalTransform,
            &crate::attributes::ProjectileSize,
        ),
        With<Player>,
    >,
    proto: ProtoParam,
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    game: GameParam,
    mobs: Query<(Entity, &GlobalTransform), With<Mob>>,
    mut burn_or_venom_mobs: Query<(
        Option<&mut Burning>,
        Option<&mut Poisoned>,
        Option<&mut Frail>,
        Option<&mut Slow>,
    )>,
    mut elec_count: Local<u8>,
    att_cooldown_query: Query<Option<&AttackTimer>, With<Player>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut status_event: EventWriter<StatusEffectEvent>,
    asset_server: Res<AssetServer>,
    player_att: Query<&ItemAttributes, With<Player>>,
    mut modify_health_events: EventWriter<ModifyHealthEvent>,
    mut throttle: Local<IceExplosionThrottle>, // Track explosions spawned this frame
) {
    // Reset counters at start of frame
    throttle.count = 0;
    throttle.sound_played = false;

    if *elec_count > 0 && att_cooldown_query.single().is_none() {
        *elec_count = 0;
    }
    let player_attributes = player_att.single();
    let (skills, player_txfm, projectile_size) = upgrades.single();
    let player_pos = player_txfm.translation().truncate();
    for hit in hits.iter() {
        // Skip damage from heirloom effects (e.g., poison, burning)
        if hit.from_heirloom_effect {
            continue;
        }
        let mut rng = rand::thread_rng();

        if hit.hit_entity == game.game.player {
            continue;
        }
        let Ok((hit_e, hit_entity_txfm)) = mobs.get(hit.hit_entity) else {
            continue;
        };
        if let Some(proj) = &hit.hit_with_projectile {
            if proj.is_skill_projectile() {
                continue;
            }
        }

        if skills.has(Heirloom::IncreaseProjectilCount)
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
            proto_commands.spawn_projectile_from_proto(
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
            ranged_attack_event.send(RangedAttackEvent {
                projectile: Projectile::Electricity,
                direction: (nearest_mob_t.1.translation().truncate()
                    - hit_entity_txfm.translation().truncate())
                .normalize_or_zero(),
                from_enemy: false,
                from_entity: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: Some(hit.damage),
                pos_override: Some(hit_entity_txfm.translation().truncate()),
                spawn_delay: 0.1,
            });
        }
        let Some(main_hand) = game.player().main_hand_slot else {
            continue;
        };
        if hit.hit_with_projectile.clone().unwrap_or_default() != Projectile::IceExplosionAOE
            && skills.has(Heirloom::IceStaffAoE)
            && rng.gen_bool((skills.get_count(Heirloom::IceStaffAoE) as f64 * 0.1).clamp(0., 1.))
        {
            // Throttle explosions per frame to prevent lag when hitting many enemies
            const MAX_ICE_EXPLOSIONS_PER_FRAME: u8 = 8;
            if throttle.count < MAX_ICE_EXPLOSIONS_PER_FRAME {
                throttle.count += 1;
                spawn_ice_explosion_hitbox(
                    &mut commands,
                    &game.graphics,
                    hit_entity_txfm.translation(),
                    hit.damage / 4,
                    projectile_size.get_multiplier(),
                );
                // Only play sound once per frame to avoid audio spam
                if !throttle.sound_played {
                    throttle.sound_played = true;
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::IceExplosion,
                        0.4,
                    ));
                }
            }
        }
        if skills.has(Heirloom::IceStaffFloor)
            && rng.gen_bool((skills.get_count(Heirloom::IceStaffFloor) as f64 * 0.1).clamp(0., 1.))
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
                Projectile::IceExplosionAOE,
            );
            commands
                .entity(ice)
                .insert(YSort(-0.1))
                .insert(IceExplosionDmg);
        }
        let Ok((burning_option, _poisoned_option, frailed_option, mut slowed_option)) =
            burn_or_venom_mobs.get_mut(hit.hit_entity)
        else {
            continue;
        };
        if (hit.hit_with_projectile.clone().unwrap_or_default() == Projectile::Dart)
            || rng.gen_bool(skills.calculate_poison_chance().clamp(0., 1.))
        {
            if let Some(mut burning) = burning_option {
                // Increment stacks and reset duration
                burning.stacks += 1;
                burning.duration_timer.reset();
                status_event.send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Poison,
                    num_stacks: burning.stacks as i32,
                });
            } else if Heirloom::PoisonStacks.is_obj_valid(main_hand.get_obj()) {
                let duration_bonus = if skills.has(Heirloom::PoisonDuration) {
                    1.5
                } else {
                    1.
                };
                // Start with 1 stack
                commands.entity(hit_e).insert(Burning {
                    tick_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                    duration_timer: Timer::from_seconds(3.0 * duration_bonus, TimerMode::Once),
                    stacks: 1,
                });
                status_event.send(StatusEffectEvent {
                    entity: hit_e,
                    effect: StatusEffect::Poison,
                    num_stacks: 1,
                });
            }
        }
        if main_hand.get_obj() == WorldObject::Hammer
            || (skills.has(Heirloom::FrailStacks)
                && rng.gen_bool(
                    (skills.get_count(Heirloom::FrailStacks) as f64 * 0.25).clamp(0.0, 0.99),
                ))
        {
            if let Some(mut frail_stacks) = frailed_option {
                if frail_stacks.num_stacks < 3
                    && Heirloom::FrailStacks.is_obj_valid(main_hand.get_obj())
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
        if main_hand.get_obj() == WorldObject::IceStaff
            || rng.gen_bool(skills.calculate_freeze_chance().clamp(0., 1.))
        {
            try_add_slow_stacks(
                hit_e,
                &mut commands,
                &mut status_event,
                slowed_option.as_deref_mut(),
            );
        }

        // Calculate total lifesteal: base from heirloom (10% per stack) + equipment lifesteal attribute
        let heirloom_lifesteal = skills.get_count(Heirloom::Lifesteal) * 10; // 10% per stack
        let equipment_lifesteal = player_attributes.lifesteal.value;
        let total_lifesteal = heirloom_lifesteal + equipment_lifesteal;

        if total_lifesteal > 0 {
            // Lifesteal can go over 100%
            // >= 100% = guaranteed 1 HP heal
            // For each additional 100% over 100%, guaranteed another HP
            // Remainder is a random chance for +1 more HP
            let mut heal_amount = 0;
            let mut remaining_lifesteal = total_lifesteal;

            // Process full 100% chunks
            while remaining_lifesteal >= 100 {
                heal_amount += 1;
                remaining_lifesteal -= 100;
            }

            // Random roll for remainder
            if remaining_lifesteal > 0
                && rng.gen_bool((remaining_lifesteal as f64 / 100.0).clamp(0.0, 1.0))
            {
                heal_amount += 1;
            }

            if heal_amount > 0 {
                modify_health_events.send(ModifyHealthEvent(heal_amount));

                // LifestealCoins: Spawn a coin for each lifesteal proc
                if skills.has(Heirloom::LifestealCoins) {
                    let d = 32.0;
                    let drop_offset = Vec2::new(rng.gen_range(-d..d), rng.gen_range(-d..d));
                    proto_commands.spawn_item_from_proto(
                        WorldObject::Coin,
                        &proto,
                        player_pos + drop_offset,
                        1,
                        None,
                    );
                }
            }
        }
    }
}
