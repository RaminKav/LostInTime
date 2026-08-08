use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{modifiers::ModifyManaEvent, Attack, CurrentMana, CurrentShield, Thorns},
    blessings::{BlessingTriggerCounts, MajorBlessing, OwnedMajorBlessings},
    combat::HitEvent,
    custom_commands::CommandsExt,
    item::WorldObject,
    player::skills::Heirloom,
    proto::proto_param::ProtoParam,
    GameState, Player,
};

/// Per-mob ICD for [`MajorBlessing::TouchThorns`].
#[derive(Resource, Default)]
pub struct TouchThornsCooldowns {
    pub timers: HashMap<Entity, Timer>,
}

#[derive(Component)]
pub struct ManaDrainShieldTimer(pub Timer);

/// Marker on lightning spawned by CoinLightning.
#[derive(Component)]
pub struct FromCoinLightning;

/// Fired when a [`Projectile::Lightning`] strike damages a target.
/// Carries world position so readers don't depend on the hit entity still existing
/// (HitEvents are double-buffered and often arrive after lethal targets despawn).
#[derive(Debug, Clone, Message)]
pub struct LightningStrikeHitEvent {
    pub pos: Vec2,
}

pub fn tick_touch_thorns_cooldowns(time: Res<Time>, mut cds: ResMut<TouchThornsCooldowns>) {
    let finished: Vec<Entity> = cds
        .timers
        .iter_mut()
        .filter_map(|(e, t)| {
            t.tick(time.delta());
            t.is_finished().then_some(*e)
        })
        .collect();
    for e in finished {
        cds.timers.remove(&e);
    }
}

/// Chance for lightning hits to drop a coin at the strike location.
const LIGHTNING_COIN_CHANCE: f64 = 0.15;

pub fn handle_lightning_coin_chance(
    mut strikes: MessageReader<LightningStrikeHitEvent>,
    majors: Query<&OwnedMajorBlessings, With<Player>>,
    mut commands: Commands,
    proto: ProtoParam,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok(majors) = majors.single() else {
        return;
    };
    if !majors.has(MajorBlessing::LightningCoinChance) {
        return;
    }
    let mut rng = rand::thread_rng();
    for strike in strikes.read() {
        if !rng.gen_bool(LIGHTNING_COIN_CHANCE) {
            continue;
        }
        blessing_triggers.increment(MajorBlessing::LightningCoinChance);
        commands.spawn_item_from_proto(WorldObject::Coin, &proto, strike.pos, 1, None);
    }
}

const MANA_DRAIN_SHIELD_INTERVAL_SECS: f32 = 2.5;

/// Every [`MANA_DRAIN_SHIELD_INTERVAL_SECS`] drain 50% remaining mana and **add** that
/// amount to CurrentShield (does not replace existing shield).
pub fn handle_mana_drain_shield(
    mut commands: Commands,
    time: Res<Time>,
    mut player: Query<
        (
            Entity,
            &OwnedMajorBlessings,
            &CurrentMana,
            Option<&mut CurrentShield>,
            Option<&mut ManaDrainShieldTimer>,
        ),
        With<Player>,
    >,
    mut modify_mana: MessageWriter<ModifyManaEvent>,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok((entity, majors, current_mana, shield_opt, timer_opt)) = player.single_mut() else {
        return;
    };
    if !majors.has(MajorBlessing::ManaDrainShield) {
        return;
    }

    if timer_opt.is_none() {
        commands
            .entity(entity)
            .insert(ManaDrainShieldTimer(Timer::from_seconds(
                MANA_DRAIN_SHIELD_INTERVAL_SECS,
                TimerMode::Repeating,
            )));
        return;
    }
    let Some(mut timer) = timer_opt else {
        return;
    };
    // Hot-reload / balance tweaks: keep the live timer on the configured interval.
    if (timer.0.duration().as_secs_f32() - MANA_DRAIN_SHIELD_INTERVAL_SECS).abs() > 0.01 {
        timer.0.set_duration(std::time::Duration::from_secs_f32(
            MANA_DRAIN_SHIELD_INTERVAL_SECS,
        ));
    }
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    let drain = current_mana.0 / 2;
    if drain <= 0 {
        return;
    }
    blessing_triggers.increment(MajorBlessing::ManaDrainShield);
    modify_mana.write(ModifyManaEvent::new(-drain));
    if let Some(mut shield) = shield_opt {
        shield.0 = shield.0.saturating_add(drain);
    } else {
        commands.entity(entity).insert(CurrentShield(drain));
    }
}

/// Touch-thorns damage on player↔mob overlap (1s per-mob ICD). Runs independently of mob attack state.
pub fn handle_touch_thorns(
    majors: Query<&OwnedMajorBlessings, With<Player>>,
    player: Query<(Entity, &Transform, &Thorns, &Attack), With<Player>>,
    mobs: Query<&Transform, (With<crate::enemy::Mob>, Without<Player>)>,
    rapier_context: bevy_rapier2d::prelude::ReadRapierContext,
    mut hit_event: MessageWriter<HitEvent>,
    mut cds: ResMut<TouchThornsCooldowns>,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    let Ok(majors) = majors.single() else {
        return;
    };
    if !majors.has(MajorBlessing::TouchThorns) {
        return;
    }
    let Ok((player_e, player_txfm, thorns, player_attack)) = player.single() else {
        return;
    };
    if thorns.0 <= 0 {
        return;
    }
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };

    for (e1, e2, _) in rapier_context.intersection_pairs_with(player_e) {
        for (e1, e2) in [(e1, e2), (e2, e1)] {
            if e1 != player_e {
                continue;
            }
            let Ok(mob_txfm) = mobs.get(e2) else {
                continue;
            };
            if cds.timers.contains_key(&e2) {
                continue;
            }
            cds.timers
                .insert(e2, Timer::from_seconds(1.0, TimerMode::Once));

            let thorns_damage = f32::ceil(player_attack.0 as f32 * thorns.0 as f32 / 100.) as i32;
            if thorns_damage <= 0 {
                continue;
            }
            blessing_triggers.increment(MajorBlessing::TouchThorns);
            let delta = player_txfm.translation - mob_txfm.translation;
            hit_event.write(HitEvent {
                hit_by_pet: None,
                hit_entity: e2,
                damage: thorns_damage,
                dir: delta.normalize_or_zero().truncate(),
                hit_with_melee: None,
                hit_with_projectile: None,
                ignore_tool: false,
                hit_by_mob: None,
                was_crit: false,
                was_overcrit: false,
                from_heirloom_effect: Some(Heirloom::Thorns),
                from_active_skill: false,
            });
        }
    }
}

pub struct MajorEffectSystemsPlugin;

impl Plugin for MajorEffectSystemsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TouchThornsCooldowns>()
            .add_message::<LightningStrikeHitEvent>()
            .add_systems(
                Update,
                (
                    tick_touch_thorns_cooldowns,
                    handle_lightning_coin_chance,
                    handle_mana_drain_shield,
                    handle_touch_thorns,
                )
                    .run_if(in_state(GameState::Main))
                    .run_if(crate::client::is_not_paused),
            );
    }
}
