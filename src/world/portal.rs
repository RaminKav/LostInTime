use crate::aseprite_assets::Portal;
use crate::aseprite_helpers::play_loop;
use crate::blessings::PendingMajorBlessings;
use crate::combat::EnemyDeathEvent;
use crate::custom_commands::CommandsExt;
use crate::enemy::{spawner::MobSpawningPaused, Mob};
use crate::item::WorldObject;
use crate::night::EraTimer;
use crate::player::levels::PlayerLevel;
use crate::player::score::RunTimer;
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;
use crate::ui::tips::{SeenTips, Tip, TipEvent};
use crate::world::dimension::{Era, EraManager};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{AnimationState, AseAnimation};
use rand::Rng;
use std::collections::HashSet;

/// Multiplier on remaining-time farm conversion. `1.0` ≈ break-even with average farm;
/// `> 1.0` slightly rewards finishing early.
pub const TIME_BONUS_K: f32 = 1.15;
/// Coin pickups are spawned in stacks of this size (final stack may be smaller).
pub const TIME_BONUS_COIN_STACK_SIZE: usize = 50;
pub const XP_SHARD_LARGE_VALUE: u32 = 500;
pub const XP_SHARD_MEDIUM_VALUE: u32 = 23;
pub const XP_SHARD_SMALL_VALUE: u32 = 7;

/// Sustained XP/s from a full-era farm (level-curve derived; includes shrine vacuums).
pub fn era_time_bonus_xp_per_sec(era: &Era) -> f32 {
    match era {
        Era::Main => 18.0,
        Era::Second => 40.0,
        Era::Third => 65.0,
        Era::DungeonMain => 0.0,
    }
}

/// Sustained coins/s from kill drop expectations at era average kill rates.
pub fn era_time_bonus_coins_per_sec(era: &Era) -> f32 {
    match era {
        Era::Main => 0.43,
        Era::Second => 1.70,
        Era::Third => 1.85,
        Era::DungeonMain => 0.0,
    }
}

/// Convert remaining era time into bonus XP + coins (before packing into drops).
pub fn calculate_era_time_bonus_amounts(era: &Era, remaining_seconds: f32) -> (u32, u32) {
    let t = remaining_seconds.max(0.0);
    let xp = (era_time_bonus_xp_per_sec(era) * t * TIME_BONUS_K).floor() as u32;
    let coins = (era_time_bonus_coins_per_sec(era) * t * TIME_BONUS_K).floor() as u32;
    (xp, coins)
}

/// Pack raw XP into largest shard denominations (large → medium → small).
pub fn pack_xp_into_shards(mut xp: u32) -> (u32, u32, u32) {
    let large = xp / XP_SHARD_LARGE_VALUE;
    xp %= XP_SHARD_LARGE_VALUE;
    let medium = xp / XP_SHARD_MEDIUM_VALUE;
    xp %= XP_SHARD_MEDIUM_VALUE;
    let small = xp / XP_SHARD_SMALL_VALUE;
    (large, medium, small)
}

/// Pack coins into stacks of [`TIME_BONUS_COIN_STACK_SIZE`] (last stack may be smaller).
pub fn pack_coins_into_stacks(coins: u32) -> Vec<usize> {
    if coins == 0 {
        return Vec::new();
    }
    let full = (coins as usize) / TIME_BONUS_COIN_STACK_SIZE;
    let rem = (coins as usize) % TIME_BONUS_COIN_STACK_SIZE;
    let mut stacks = vec![TIME_BONUS_COIN_STACK_SIZE; full];
    if rem > 0 {
        stacks.push(rem);
    }
    stacks
}

fn spawn_time_bonus_drop(
    commands: &mut Commands,
    proto_param: &ProtoParam,
    obj: WorldObject,
    pos: Vec2,
    count: usize,
    player_level: Option<u8>,
) {
    let mut rng = rand::thread_rng();
    let drop_offset = Vec2::new(rng.gen_range(-30.0..30.0), rng.gen_range(-30.0..30.0));
    let _ =
        commands.spawn_item_from_proto(obj, proto_param, pos + drop_offset, count, player_level);
}

/// Extra loot for clearing the era boss with time remaining — does not replace boss loot table drops.
fn drop_era_time_bonus_loot(
    commands: &mut Commands,
    proto_param: &ProtoParam,
    era: &Era,
    remaining_seconds: f32,
    drop_pos: Vec2,
    player_level: Option<u8>,
) {
    let (xp, coins) = calculate_era_time_bonus_amounts(era, remaining_seconds);
    if xp == 0 && coins == 0 {
        return;
    }

    let (large, medium, small) = pack_xp_into_shards(xp);
    info!(
        "Era time bonus ({:?}, {:.0}s left): {} XP → {}L/{}M/{}S shards, {} coins",
        era, remaining_seconds, xp, large, medium, small, coins
    );

    for _ in 0..large {
        spawn_time_bonus_drop(
            commands,
            proto_param,
            WorldObject::XPShardLarge,
            drop_pos,
            1,
            player_level,
        );
    }
    for _ in 0..medium {
        spawn_time_bonus_drop(
            commands,
            proto_param,
            WorldObject::XPShardMedium,
            drop_pos,
            1,
            player_level,
        );
    }
    for _ in 0..small {
        spawn_time_bonus_drop(
            commands,
            proto_param,
            WorldObject::XPShard,
            drop_pos,
            1,
            player_level,
        );
    }
    for stack_count in pack_coins_into_stacks(coins) {
        spawn_time_bonus_drop(
            commands,
            proto_param,
            WorldObject::Coin,
            drop_pos,
            stack_count,
            player_level,
        );
    }
}

pub fn handle_player_near_portal(
    player_query: Query<&Transform, With<Player>>,
    mut portal_query: Query<
        (&GlobalTransform, &mut AseAnimation, &AnimationState),
        With<TimePortal>,
    >,
) {
    for player_transform in player_query.iter() {
        for (portal_transform, mut anim, state) in portal_query.iter_mut() {
            let distance = player_transform
                .translation
                .distance(portal_transform.translation());
            if distance <= 32. && usize::from(state.current_frame()) <= 8 {
                play_loop(&mut anim, Portal::tags::ERA2);
            }
        }
    }
    for (_portal_transform, mut anim, state) in portal_query.iter_mut() {
        if usize::from(state.current_frame()) == 35 {
            play_loop(&mut anim, Portal::tags::IDLE);
        }
    }
}

/// Component marker for the time portal entity
#[derive(Component, Default, Debug)]
pub struct TimePortal;

/// Resource to track which eras have had their bosses killed
#[derive(Resource, Default, Debug, Clone)]
pub struct BossKillTracker {
    pub killed_eras: HashSet<Era>,
    /// Run elapsed time when the Era 1 boss was defeated, if recorded this run.
    pub era1_boss_kill_elapsed_seconds: Option<f64>,
}

impl BossKillTracker {
    pub fn mark_boss_killed(&mut self, era: Era) {
        self.killed_eras.insert(era);
    }

    pub fn mark_era1_boss_killed(&mut self, elapsed_seconds: f64) {
        self.killed_eras.insert(Era::Main);
        self.era1_boss_kill_elapsed_seconds = Some(elapsed_seconds);
    }

    pub fn is_boss_killed(&self, era: &Era) -> bool {
        self.killed_eras.contains(era)
    }
}

/// System to track boss kills and update the tracker
pub fn track_boss_kills(
    mut death_events: MessageReader<EnemyDeathEvent>,
    mob_query: Query<&Mob>,
    era_manager: Res<EraManager>,
    mut boss_kill_tracker: ResMut<BossKillTracker>,
    run_timer: Res<RunTimer>,
    era_timer: Res<EraTimer>,
    mut mob_spawning_paused: ResMut<MobSpawningPaused>,
    mut tip_event: MessageWriter<TipEvent>,
    seen_tips: Res<SeenTips>,
    mut commands: Commands,
    proto_param: ProtoParam,
    player_level: Query<&PlayerLevel, With<Player>>,
    mut pending_major_blessings: ResMut<PendingMajorBlessings>,
) {
    for death_event in death_events.read() {
        if let Ok(mob) = mob_query.get(death_event.entity) {
            // StoneGolem is a boss but doesn't count for era completion
            if !mob.is_boss() || mob == &Mob::StoneGolem {
                continue;
            }

            let era = era_manager.current_era.clone();
            let first_clear = !boss_kill_tracker.is_boss_killed(&era);

            if mob == &Mob::RedMushking && era == Era::Main {
                boss_kill_tracker.mark_era1_boss_killed(run_timer.elapsed_seconds);
            } else {
                boss_kill_tracker.mark_boss_killed(era.clone());
            }
            info!("Boss killed in era {:?}", era);

            if first_clear {
                let level = player_level.single().ok().map(|l| l.level);
                drop_era_time_bonus_loot(
                    &mut commands,
                    &proto_param,
                    &era,
                    era_timer.remaining_seconds,
                    death_event.enemy_pos,
                    level,
                );

                // Major blessings: first clear of Era 1 / Era 2 bosses only (not Era 3).
                if matches!(era, Era::Main | Era::Second) {
                    pending_major_blessings.0 =
                        pending_major_blessings.0.saturating_add(1);
                    info!(
                        "Queued major blessing (pending={})",
                        pending_major_blessings.0
                    );
                }
            }

            if (era == Era::Main || era == Era::Second) && era_timer.remaining_seconds > 0.0 {
                mob_spawning_paused.paused = true;

                if !seen_tips.has_seen(&Tip::PeacefulPeriod) {
                    tip_event.write(TipEvent {
                        tip: Tip::PeacefulPeriod,
                        pos: Vec3::new(-184., -116., 95.),
                    });
                }
            }
        }
    }
}
