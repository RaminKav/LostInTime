//! Stress test for enemy spawn → kill → loot cycle. Enable with `COLLIDER_LOAD_TEST=1 cargo run`.
//!
//! Spawns mobs around the player, then kills them via real `HitEvent` damage so they go through
//! the full death pipeline: `handle_hits` → `MarkedForDeath` → `EnemyDeathEvent` →
//! `handle_enemy_death` (loot drops, on-kill heirlooms) → `cleanup_marked_for_death_entities`.
//!
//! **F9** toggles this and any other enabled load tests (starts off).

use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use rand::Rng;

use crate::{
    client::is_not_paused,
    combat::HitEvent,
    custom_commands::CommandsExt,
    enemy::Mob,
    item::WorldObject,
    player::{skills::Heirloom, Player},
    proto::proto_param::ProtoParam,
    world::{dimension::ActiveDimension, dungeon::Dungeon},
    GameState,
};

const MOB_COUNT: usize = 100;
const ITEM_COUNT: usize = 0;
/// Delay before mobs are hit with lethal damage, giving them time to fully initialize.
const MOB_KILL_DELAY_SECS: f32 = 0.3;
const ITEM_LIFETIME_SECS: f32 = 0.1;
/// Public for unified F9 toggle reset (`gameplay_load_tests`).
pub const WAVE_INTERVAL_SECS: f32 = 0.3;

#[derive(Resource, Default)]
pub struct ColliderLoadTestActive {
    pub active: bool,
}

#[derive(Component)]
pub struct ColliderLoadTestMarker;

/// Mobs with this component will be killed via HitEvent after the timer expires.
#[derive(Component)]
pub struct ColliderLoadTestKillTimer {
    pub timer: Timer,
}

/// Items (not mobs) still use forced despawn since they're not part of the death pipeline.
#[derive(Component)]
pub struct ColliderLoadTestDespawn {
    pub timer: Timer,
}

#[derive(Resource)]
pub struct ColliderLoadTestState {
    pub wave_timer: Timer,
    pub pending_first_wave: bool,
}

impl Default for ColliderLoadTestState {
    fn default() -> Self {
        Self {
            wave_timer: Timer::from_seconds(WAVE_INTERVAL_SECS, TimerMode::Repeating),
            pending_first_wave: true,
        }
    }
}

pub struct ColliderLoadTestPlugin;

impl Plugin for ColliderLoadTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ColliderLoadTestState>()
            .init_resource::<ColliderLoadTestActive>()
            .add_system(
                collider_load_test_spawn_wave
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(is_not_paused),
            )
            .add_system(
                tick_collider_load_test_kill
                    .in_set(OnUpdate(GameState::Main))
                    .after(collider_load_test_spawn_wave),
            )
            .add_system(
                tick_collider_load_test_despawn
                    .in_set(OnUpdate(GameState::Main))
                    .after(collider_load_test_spawn_wave),
            );
        info!(
            "Collider load test plugin: {} mobs killed via HitEvent after {}s, {} items force-despawned @ {}s, wave every {}s",
            MOB_COUNT, MOB_KILL_DELAY_SECS, ITEM_COUNT, ITEM_LIFETIME_SECS, WAVE_INTERVAL_SECS
        );
    }
}

fn collider_load_test_spawn_wave(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<ColliderLoadTestState>,
    active: Res<ColliderLoadTestActive>,
    player: Query<&GlobalTransform, With<Player>>,
    mut proto_commands: ProtoCommands,
    prototypes: Prototypes,
    proto_param: ProtoParam,
    dungeon: Query<&Dungeon, With<ActiveDimension>>,
) {
    if !active.active {
        return;
    }
    if dungeon.get_single().is_ok() {
        return;
    }
    let Ok(player_txfm) = player.get_single() else {
        return;
    };
    let player_pos = player_txfm.translation().truncate();

    if state.pending_first_wave {
        state.pending_first_wave = false;
    } else {
        state.wave_timer.tick(time.delta());
        if !state.wave_timer.just_finished() {
            return;
        }
    }

    let mut rng = rand::thread_rng();
    let non_boss_mobs = [
        // Mob::Slime,
        Mob::SpikeSlime,
        // Mob::StoneGolem,
        // Mob::FurDevil,
        // Mob::StingFly,
        // Mob::Bushling,
        // Mob::Fairy,
        Mob::RedMushling,
        // Mob::Crow,
    ];

    for i in 0..MOB_COUNT {
        let angle = (i as f32) * 0.0628 + rng.gen_range(-0.02..0.02);
        let r = 48. + (i % 25) as f32 * 5.5;
        let pos = player_pos
            + Vec2::from_angle(angle) * r
            + Vec2::new(rng.gen_range(-6.0..6.0), rng.gen_range(-6.0..6.0));

        let mob = non_boss_mobs[rng.gen_range(0..non_boss_mobs.len())].clone();
        if let Some(e) = proto_commands.spawn_from_proto(mob, &prototypes, pos) {
            commands.entity(e).insert((
                ColliderLoadTestMarker,
                ColliderLoadTestKillTimer {
                    timer: Timer::from_seconds(MOB_KILL_DELAY_SECS, TimerMode::Once),
                },
            ));
        }
    }

    for i in 0..ITEM_COUNT {
        let angle = (i as f32) * 0.0628 + 0.31 + rng.gen_range(-0.02..0.02);
        let r = 52. + (i % 25) as f32 * 5.5;
        let pos = player_pos
            + Vec2::from_angle(angle) * r
            + Vec2::new(rng.gen_range(-6.0..6.0), rng.gen_range(-6.0..6.0));

        if let Some(e) =
            proto_commands.spawn_item_from_proto(WorldObject::XPShard, &proto_param, pos, 1, None)
        {
            commands.entity(e).insert((
                ColliderLoadTestMarker,
                ColliderLoadTestDespawn {
                    timer: Timer::from_seconds(ITEM_LIFETIME_SECS, TimerMode::Once),
                },
            ));
        }
    }
}

/// Ticks kill timers and sends HitEvent with 999 damage to route mobs through the real death pipeline.
fn tick_collider_load_test_kill(
    time: Res<Time>,
    mut q: Query<(Entity, &mut ColliderLoadTestKillTimer)>,
    mut hit_events: EventWriter<HitEvent>,
    mut commands: Commands,
) {
    for (entity, mut kill) in q.iter_mut() {
        kill.timer.tick(time.delta());
        if kill.timer.finished() {
            hit_events.send(HitEvent {
                hit_entity: entity,
                damage: 999,
                dir: Vec2::X,
                hit_with_melee: None,
                hit_with_projectile: None,
                hit_by_mob: None,
                hit_by_pet: None,
                was_crit: false,
                was_overcrit: false,
                ignore_tool: false,
                from_heirloom_effect: Some(Heirloom::PoisonStacks),
                from_active_skill: false,
            });
            commands
                .entity(entity)
                .remove::<ColliderLoadTestKillTimer>();
        }
    }
}

fn tick_collider_load_test_despawn(
    mut commands: Commands,
    time: Res<Time>,
    mut q: Query<(Entity, &mut ColliderLoadTestDespawn)>,
) {
    for (entity, mut despawn) in q.iter_mut() {
        despawn.timer.tick(time.delta());
        if despawn.timer.finished() {
            if let Some(ec) = commands.get_entity(entity) {
                ec.despawn_recursive();
            }
        }
    }
}
