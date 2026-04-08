//! Simulated heirloom VFX and combat particles for performance testing.
//!
//! - `HEIRLOOM_LOAD_TEST=1` — bursts of echoes, lightning, ice explosions, mana orbs, ant farm ants.
//! - `PARTICLE_LOAD_TEST=1` — Hanabi bursts matching mob hit / death particles.
//! - `HEIRLOOM_LOAD_TEST_DIAG=1` — every 5s, log counts (entities, mana orbs, ants, deferred spawns, projectiles).
//!
//! **F9** toggles every enabled load test (including collider test) on or off together.
//!
//! ## Why the heirloom test used to tank FPS (especially with `NO_SPAWN`)
//!
//! Production [`crate::custom_commands::spawn_item_from_proto`] gives **world drops** a
//! [`crate::item::ItemDropDespawnTimer`] of **300 seconds**. Spamming [`WorldObject::ManaOrb`] without
//! picking them up therefore grows the world with thousands of colliding pickup entities. Echo / ice
//! use [`crate::combat::combat_helpers::spawn_deferred_aseprite_collider`] with ~**10.5s** despawn;
//! lightning projectiles use **5s** ([`crate::item::projectile::handle_spawn_projectiles_after_delay`]);
//! ants live **4s** ([`crate::player::combat_heirlooms::ANT_LIFETIME`]). The load test overrides mana
//! orb despawn to [`HEIRLOOM_TEST_MANA_ORB_LIFETIME_SECS`] so the test measures VFX/collider churn,
//! not unbounded orb hoarding.

use std::f32::consts::TAU;

use bevy::prelude::*;
use bevy_hanabi::prelude::{graph, ParticleEffect, ParticleEffectBundle};
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::{Collider, RapierContext};
use rand::Rng;

use crate::{
    animations::ui_animaitons::MoveUIAnimation,
    assets::Graphics,
    attributes::add_item_glows,
    audio::SoundSpawner,
    client::is_not_paused,
    collider_load_test::WAVE_INTERVAL_SECS as COLLIDER_WAVE_INTERVAL_SECS,
    collider_load_test::{ColliderLoadTestActive, ColliderLoadTestState},
    combat::{
        combat_helpers::SpawnAsepriteAnimationCollider,
        pickup_radius::BeingPulledToPlayer,
        status_effects::{Burning, StatusEffect, StatusEffectEvent},
        MarkedForDeath,
    },
    custom_commands::CommandsExt,
    enemy::Mob,
    item::{
        projectile::{Projectile, RangedAttackEvent},
        ItemDrop, ItemDropDespawnTimer, Loot, LootTable, LootTablePlugin, WorldObject,
    },
    player::{
        combat_heirlooms::{spawn_ant_farm_ants, AntFarmAnt},
        mage_skills::spawn_ice_explosion_hitbox,
        melee_skills::spawn_echo_hitbox,
        Player,
    },
    proto::proto_param::ProtoParam,
    ui::damage_numbers::{DamageNumber, QueueFloatingText},
    world::{dimension::ActiveDimension, dungeon::Dungeon, y_sort::YSort},
    GameState,
};

use crate::juice::{ObjectHitParticles, Particles};

/// Shorter than production (300s) so the load test does not simulate infinite mana-orb hoarding.
const HEIRLOOM_TEST_MANA_ORB_LIFETIME_SECS: f32 = 12.0;

const HEIRLOOM_WAVE_INTERVAL_SECS: f32 = 0.32;
const PARTICLE_WAVE_INTERVAL_SECS: f32 = 1.08;
const HEIRLOOM_EFFECTS_PER_WAVE_MIN: u32 = 40;
const HEIRLOOM_EFFECTS_PER_WAVE_MAX: u32 = 45;
const PARTICLE_HIT_BURST_MIN: u32 = 0;
const PARTICLE_HIT_BURST_MAX: u32 = 1;
const PARTICLE_DEATH_BURST_MIN: u32 = 0;
const PARTICLE_DEATH_BURST_MAX: u32 = 1;

const POISON_WAVE_INTERVAL_SECS: f32 = 0.25;
const WEAPON_WAVE_INTERVAL_SECS: f32 = 0.3;
const WEAPON_SWORD_PER_WAVE: u32 = 5;
const WEAPON_SHOUT_PER_WAVE: u32 = 2;

const LOOT_CYCLE_WAVE_INTERVAL_SECS: f32 = 0.3;
const LOOT_CYCLE_DROPS_PER_WAVE: u32 = 20;
/// Items linger long enough to pile up, but not 300s like production.
const LOOT_CYCLE_DESPAWN_SECS: f32 = 40.0;

#[derive(Resource, Default)]
pub struct HeirloomLoadTestActive {
    pub active: bool,
}

#[derive(Resource)]
pub struct HeirloomLoadTestState {
    pub wave_timer: Timer,
    pub pending_first_wave: bool,
}

impl Default for HeirloomLoadTestState {
    fn default() -> Self {
        Self {
            wave_timer: Timer::from_seconds(HEIRLOOM_WAVE_INTERVAL_SECS, TimerMode::Repeating),
            pending_first_wave: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct ParticleLoadTestActive {
    pub active: bool,
}

#[derive(Resource)]
pub struct ParticleLoadTestState {
    pub wave_timer: Timer,
    pub pending_first_wave: bool,
}

impl Default for ParticleLoadTestState {
    fn default() -> Self {
        Self {
            wave_timer: Timer::from_seconds(PARTICLE_WAVE_INTERVAL_SECS, TimerMode::Repeating),
            pending_first_wave: true,
        }
    }
}

// --- Poison load test ---

#[derive(Resource, Default)]
pub struct PoisonLoadTestActive {
    pub active: bool,
}

#[derive(Resource)]
pub struct PoisonLoadTestState {
    pub wave_timer: Timer,
    pub pending_first_wave: bool,
}

impl Default for PoisonLoadTestState {
    fn default() -> Self {
        Self {
            wave_timer: Timer::from_seconds(POISON_WAVE_INTERVAL_SECS, TimerMode::Repeating),
            pending_first_wave: true,
        }
    }
}

// --- Weapon/skill load test ---

#[derive(Resource, Default)]
pub struct WeaponLoadTestActive {
    pub active: bool,
}

#[derive(Resource)]
pub struct WeaponLoadTestState {
    pub wave_timer: Timer,
    pub pending_first_wave: bool,
}

impl Default for WeaponLoadTestState {
    fn default() -> Self {
        Self {
            wave_timer: Timer::from_seconds(WEAPON_WAVE_INTERVAL_SECS, TimerMode::Repeating),
            pending_first_wave: true,
        }
    }
}

// --- Loot cycle load test ---

#[derive(Resource, Default)]
pub struct LootCycleLoadTestActive {
    pub active: bool,
}

#[derive(Resource)]
pub struct LootCycleLoadTestState {
    pub wave_timer: Timer,
    pub pending_first_wave: bool,
}

impl Default for LootCycleLoadTestState {
    fn default() -> Self {
        Self {
            wave_timer: Timer::from_seconds(LOOT_CYCLE_WAVE_INTERVAL_SECS, TimerMode::Repeating),
            pending_first_wave: true,
        }
    }
}

pub struct HeirloomLoadTestPlugin;

fn heirloom_load_test_diag_enabled() -> bool {
    *crate::HEIRLOOM_LOAD_TEST_DIAG
}

impl Plugin for HeirloomLoadTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HeirloomLoadTestActive>()
            .init_resource::<HeirloomLoadTestState>()
            .add_system(
                heirloom_load_test_burst
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(is_not_paused),
            )
            .add_system(
                heirloom_load_test_diag
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(heirloom_load_test_diag_enabled),
            );
        info!(
            "Heirloom load test plugin: F9 toggles (with other tests). Interval {:.2}s, {}–{} random effects/wave",
            HEIRLOOM_WAVE_INTERVAL_SECS,
            HEIRLOOM_EFFECTS_PER_WAVE_MIN,
            HEIRLOOM_EFFECTS_PER_WAVE_MAX
        );
    }
}

pub struct ParticleLoadTestPlugin;

impl Plugin for ParticleLoadTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ParticleLoadTestActive>()
            .init_resource::<ParticleLoadTestState>()
            .add_system(
                particle_load_test_burst
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(is_not_paused),
            );
        info!(
            "Particle load test plugin: F9 toggles. Interval {:.2}s; mob hit + death Hanabi bursts",
            PARTICLE_WAVE_INTERVAL_SECS
        );
    }
}

pub struct PoisonLoadTestPlugin;

impl Plugin for PoisonLoadTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PoisonLoadTestActive>()
            .init_resource::<PoisonLoadTestState>()
            .add_system(
                poison_load_test_tick
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(is_not_paused),
            );
        info!(
            "Poison load test plugin: F9 toggles. Applies 1 poison stack to all enemies every {:.2}s",
            POISON_WAVE_INTERVAL_SECS
        );
    }
}

pub struct WeaponLoadTestPlugin;

impl Plugin for WeaponLoadTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WeaponLoadTestActive>()
            .init_resource::<WeaponLoadTestState>()
            .add_system(
                weapon_load_test_burst
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(is_not_paused),
            );
        info!(
            "Weapon load test plugin: F9 toggles. {} sword + {} shout per wave every {:.2}s",
            WEAPON_SWORD_PER_WAVE, WEAPON_SHOUT_PER_WAVE, WEAPON_WAVE_INTERVAL_SECS
        );
    }
}

pub struct LootCycleLoadTestPlugin;

impl Plugin for LootCycleLoadTestPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LootCycleLoadTestActive>()
            .init_resource::<LootCycleLoadTestState>()
            .add_system(
                loot_cycle_load_test_burst
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(is_not_paused),
            );
        info!(
            "Loot cycle load test plugin: F9 toggles. {} drops/wave every {:.2}s, {}s linger time",
            LOOT_CYCLE_DROPS_PER_WAVE, LOOT_CYCLE_WAVE_INTERVAL_SECS, LOOT_CYCLE_DESPAWN_SECS
        );
    }
}

#[derive(Clone, Copy)]
enum HeirloomSimKind {
    Echo,
    Lightning,
    IceExplosion,
    ManaOrb,
    Ants,
}

pub fn unified_load_tests_f9_toggle(
    keys: Res<Input<KeyCode>>,
    mut collider_active: Option<ResMut<ColliderLoadTestActive>>,
    mut collider_state: Option<ResMut<ColliderLoadTestState>>,
    mut heirloom_active: Option<ResMut<HeirloomLoadTestActive>>,
    mut heirloom_state: Option<ResMut<HeirloomLoadTestState>>,
    mut particle_active: Option<ResMut<ParticleLoadTestActive>>,
    mut particle_state: Option<ResMut<ParticleLoadTestState>>,
    mut poison_active: Option<ResMut<PoisonLoadTestActive>>,
    mut poison_state: Option<ResMut<PoisonLoadTestState>>,
    mut weapon_active: Option<ResMut<WeaponLoadTestActive>>,
    mut weapon_state: Option<ResMut<WeaponLoadTestState>>,
    mut loot_active: Option<ResMut<LootCycleLoadTestActive>>,
    mut loot_state: Option<ResMut<LootCycleLoadTestState>>,
) {
    if !keys.just_pressed(KeyCode::F9) {
        return;
    }

    if let Some(mut a) = collider_active {
        a.active = !a.active;
        if a.active {
            if let Some(mut s) = collider_state {
                s.pending_first_wave = true;
                s.wave_timer =
                    Timer::from_seconds(COLLIDER_WAVE_INTERVAL_SECS, TimerMode::Repeating);
            }
        }
        info!("Collider load test {}", if a.active { "ON" } else { "OFF" });
    }
    if let Some(mut a) = heirloom_active {
        a.active = !a.active;
        if a.active {
            if let Some(mut s) = heirloom_state {
                s.pending_first_wave = true;
                s.wave_timer =
                    Timer::from_seconds(HEIRLOOM_WAVE_INTERVAL_SECS, TimerMode::Repeating);
            }
        }
        info!("Heirloom load test {}", if a.active { "ON" } else { "OFF" });
    }
    if let Some(mut a) = particle_active {
        a.active = !a.active;
        if a.active {
            if let Some(mut s) = particle_state {
                s.pending_first_wave = true;
                s.wave_timer =
                    Timer::from_seconds(PARTICLE_WAVE_INTERVAL_SECS, TimerMode::Repeating);
            }
        }
        info!("Particle load test {}", if a.active { "ON" } else { "OFF" });
    }
    if let Some(mut a) = poison_active {
        a.active = !a.active;
        if a.active {
            if let Some(mut s) = poison_state {
                s.pending_first_wave = true;
                s.wave_timer = Timer::from_seconds(POISON_WAVE_INTERVAL_SECS, TimerMode::Repeating);
            }
        }
        info!("Poison load test {}", if a.active { "ON" } else { "OFF" });
    }
    if let Some(mut a) = weapon_active {
        a.active = !a.active;
        if a.active {
            if let Some(mut s) = weapon_state {
                s.pending_first_wave = true;
                s.wave_timer = Timer::from_seconds(WEAPON_WAVE_INTERVAL_SECS, TimerMode::Repeating);
            }
        }
        info!("Weapon load test {}", if a.active { "ON" } else { "OFF" });
    }
    if let Some(mut a) = loot_active {
        a.active = !a.active;
        if a.active {
            if let Some(mut s) = loot_state {
                s.pending_first_wave = true;
                s.wave_timer =
                    Timer::from_seconds(LOOT_CYCLE_WAVE_INTERVAL_SECS, TimerMode::Repeating);
            }
        }
        info!(
            "Loot cycle load test {}",
            if a.active { "ON" } else { "OFF" }
        );
    }
}

fn heirloom_load_test_diag(
    time: Res<Time>,
    mut elapsed: Local<f32>,
    all_entities: Query<Entity>,
    ants: Query<(), With<AntFarmAnt>>,
    deferred_markers: Query<(), With<SpawnAsepriteAnimationCollider>>,
    item_drops: Query<&WorldObject, With<ItemDrop>>,
    projectiles: Query<&Projectile>,
) {
    *elapsed += time.delta_seconds();
    if *elapsed < 5.0 {
        return;
    }
    *elapsed = 0.0;

    let total = all_entities.iter().count();
    let mana_orbs = item_drops
        .iter()
        .filter(|wo| **wo == WorldObject::ManaOrb)
        .count();
    let ant_count = ants.iter().count();
    let deferred = deferred_markers.iter().count();

    let (mut echo_p, mut lightning_p, mut ice_aoe) = (0usize, 0usize, 0usize);
    for p in projectiles.iter() {
        match *p {
            Projectile::Echo => echo_p += 1,
            Projectile::Lightning => lightning_p += 1,
            Projectile::IceExplosionAOE => ice_aoe += 1,
            _ => {}
        }
    }

    info!(
        "[HEIRLOOM_DIAG] entities={} mana_orb_drops={} ants={} deferred_aseprite_markers={} proj_echo={} proj_lightning={} proj_ice_aoe={}",
        total, mana_orbs, ant_count, deferred, echo_p, lightning_p, ice_aoe
    );
}

fn random_offset(rng: &mut impl Rng, radius: f32) -> Vec2 {
    let a = rng.gen_range(0.0..TAU);
    let r = rng.gen_range(0.0..radius);
    Vec2::new(a.cos(), a.sin()) * r
}

fn heirloom_load_test_burst(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<HeirloomLoadTestState>,
    active: Res<HeirloomLoadTestActive>,
    player: Query<(Entity, &GlobalTransform), With<Player>>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    mut proto_commands: ProtoCommands,
    proto_param: ProtoParam,
    mut ranged_attack: EventWriter<RangedAttackEvent>,
    dungeon: Query<&Dungeon, With<ActiveDimension>>,
) {
    if !active.active {
        return;
    }
    if dungeon.get_single().is_ok() {
        return;
    }
    let Ok((player_e, player_txfm)) = player.get_single() else {
        return;
    };
    let player_pos = player_txfm.translation();

    if state.pending_first_wave {
        state.pending_first_wave = false;
    } else {
        state.wave_timer.tick(time.delta());
        if !state.wave_timer.just_finished() {
            return;
        }
    }

    let atlas = match &graphics.texture_atlas {
        Some(a) => a.clone(),
        None => return,
    };

    let mut rng = rand::thread_rng();
    let n = rng.gen_range(HEIRLOOM_EFFECTS_PER_WAVE_MIN..=HEIRLOOM_EFFECTS_PER_WAVE_MAX);
    let kinds = [
        HeirloomSimKind::Echo,
        HeirloomSimKind::Lightning,
        HeirloomSimKind::IceExplosion,
        // HeirloomSimKind::ManaOrb,
        // HeirloomSimKind::Ants,
    ];

    for _ in 0..n {
        let kind = kinds[rng.gen_range(0..kinds.len())];
        let offset = random_offset(&mut rng, 120.0);
        let world = player_pos + offset.extend(0.0);

        match kind {
            HeirloomSimKind::Echo => {
                let _ = world;
                spawn_echo_hitbox(
                    &mut commands,
                    &asset_server,
                    player_e,
                    1000,
                    rng.gen_range(0.85..1.15),
                );
            }
            HeirloomSimKind::Lightning => {
                let strike = world.truncate() + Vec2::new(0., 48.);
                ranged_attack.send(RangedAttackEvent {
                    projectile: Projectile::Lightning,
                    direction: Vec2::ZERO,
                    mana_cost: None,
                    from_enemy: false,
                    from_entity: None,
                    is_followup_proj: false,
                    dmg_override: Some(1000),
                    pos_override: Some(strike),
                    spawn_delay: 0.0,
                });
            }
            HeirloomSimKind::IceExplosion => {
                if graphics.ice_explosion_ase.is_some() {
                    spawn_ice_explosion_hitbox(
                        &mut commands,
                        &graphics,
                        world,
                        1000,
                        rng.gen_range(0.9..1.2),
                    );
                }
            }
            HeirloomSimKind::ManaOrb => {
                let drop_pos = world.truncate() + random_offset(&mut rng, 24.0);
                if let Some(e) = proto_commands.spawn_item_from_proto(
                    WorldObject::ManaOrb,
                    &proto_param,
                    drop_pos,
                    1,
                    None,
                ) {
                    // Production `spawn_item_from_proto` uses 300s; without pickup that hoards entities.
                    commands
                        .entity(e)
                        .insert(ItemDropDespawnTimer(Timer::from_seconds(
                            HEIRLOOM_TEST_MANA_ORB_LIFETIME_SECS,
                            TimerMode::Once,
                        )));
                }
            }
            HeirloomSimKind::Ants => {
                let mut mana_none: Option<&mut i32> = None;
                spawn_ant_farm_ants(
                    &mut commands,
                    &atlas,
                    &graphics,
                    world,
                    rng.gen_range(6..14),
                    &mut mana_none,
                    0,
                );
            }
        }
    }
}

fn particle_load_test_burst(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<ParticleLoadTestState>,
    active: Res<ParticleLoadTestActive>,
    player: Query<&GlobalTransform, With<Player>>,
    particles: Res<Particles>,
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
    let base = player_txfm.translation();

    if state.pending_first_wave {
        state.pending_first_wave = false;
    } else {
        state.wave_timer.tick(time.delta());
        if !state.wave_timer.just_finished() {
            return;
        }
    }

    let mut rng = rand::thread_rng();
    let mob_colors = [
        Mob::FurDevil,
        Mob::SpikeSlime,
        Mob::Bushling,
        Mob::StingFly,
        Mob::Crow,
    ];

    let hits = rng.gen_range(PARTICLE_HIT_BURST_MIN..=PARTICLE_HIT_BURST_MAX);
    for _ in 0..hits {
        let offset = random_offset(&mut rng, 100.0);
        let p = base + offset.extend(0.0);
        let mob = mob_colors[rng.gen_range(0..mob_colors.len())].clone();
        let color = mob.get_mob_color();

        commands.spawn((
            Name::new("load_test_enemy_hit_particles"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(particles.enemy_hit_particles.clone())
                    .with_properties::<ParticleEffect>(vec![(
                        "my_color".to_string(),
                        graph::Value::Uint(color.as_linear_rgba_u32()),
                    )])
                    .with_z_layer_2d(Some(999.)),
                transform: Transform::from_translation(Vec3::new(p.x, p.y + 4., 2.)),
                ..Default::default()
            },
            YSort(1.),
            ObjectHitParticles {
                despawn_timer: Timer::from_seconds(0.23, TimerMode::Once),
                velocity: Vec3::new(0., 8000., 0.),
            },
        ));
    }

    let deaths = rng.gen_range(PARTICLE_DEATH_BURST_MIN..=PARTICLE_DEATH_BURST_MAX);
    for _ in 0..deaths {
        let offset = random_offset(&mut rng, 100.0);
        let p = base + offset.extend(0.0);

        commands.spawn((
            Name::new("load_test_enemy_death_particles"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(particles.enemy_death_particle.clone())
                    .with_z_layer_2d(Some(999.)),
                transform: Transform::from_translation(Vec3::new(p.x, p.y + 4., 2.)),
                ..Default::default()
            },
            YSort(1.),
            ObjectHitParticles {
                despawn_timer: Timer::from_seconds(1.1, TimerMode::Once),
                velocity: Vec3::new(0., 8000., 0.),
            },
        ));
    }
}

// =============================================================================
// Poison load test
// =============================================================================

fn poison_load_test_tick(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<PoisonLoadTestState>,
    active: Res<PoisonLoadTestActive>,
    enemies: Query<Entity, (With<Mob>, Without<Player>)>,
    mut burning_enemies: Query<&mut Burning>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    if !active.active {
        return;
    }

    if state.pending_first_wave {
        state.pending_first_wave = false;
    } else {
        state.wave_timer.tick(time.delta());
        if !state.wave_timer.just_finished() {
            return;
        }
    }

    for enemy_entity in enemies.iter() {
        if let Ok(mut burning) = burning_enemies.get_mut(enemy_entity) {
            burning.stacks = burning.stacks.saturating_add(1);
            burning.duration_timer.reset();
            status_event.send(StatusEffectEvent {
                entity: enemy_entity,
                effect: StatusEffect::Poison,
                num_stacks: burning.stacks as i32,
            });
        } else {
            commands.entity(enemy_entity).insert(Burning {
                tick_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                duration_timer: Timer::from_seconds(3.0, TimerMode::Once),
                stacks: 1,
            });
            status_event.send(StatusEffectEvent {
                entity: enemy_entity,
                effect: StatusEffect::Poison,
                num_stacks: 1,
            });
        }
    }
}

// =============================================================================
// Weapon / skill effects load test
// =============================================================================

fn weapon_load_test_burst(
    time: Res<Time>,
    mut state: ResMut<WeaponLoadTestState>,
    active: Res<WeaponLoadTestActive>,
    player: Query<(Entity, &GlobalTransform), With<Player>>,
    mut ranged_attack: EventWriter<RangedAttackEvent>,
    dungeon: Query<&Dungeon, With<ActiveDimension>>,
) {
    if !active.active {
        return;
    }
    if dungeon.get_single().is_ok() {
        return;
    }
    let Ok((player_e, player_txfm)) = player.get_single() else {
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

    for _ in 0..WEAPON_SWORD_PER_WAVE {
        let angle = rng.gen_range(0.0..TAU);
        let dir = Vec2::new(angle.cos(), angle.sin());
        ranged_attack.send(RangedAttackEvent {
            projectile: Projectile::SwordProjectile,
            direction: dir,
            mana_cost: None,
            from_enemy: false,
            from_entity: Some(player_e),
            is_followup_proj: false,
            dmg_override: Some(25),
            pos_override: Some(player_pos),
            spawn_delay: 0.0,
        });
    }

    for _ in 0..WEAPON_SHOUT_PER_WAVE {
        let offset = random_offset(&mut rng, 60.0);
        ranged_attack.send(RangedAttackEvent {
            projectile: Projectile::Shout,
            direction: Vec2::ZERO,
            mana_cost: None,
            from_enemy: false,
            from_entity: Some(player_e),
            is_followup_proj: false,
            dmg_override: Some(25),
            pos_override: Some(player_pos + offset),
            spawn_delay: 0.0,
        });
    }
}

// =============================================================================
// Loot cycle load test
// =============================================================================

/// Synthetic loot table matching a typical mob: coins, XP shards, small potion.
fn make_test_loot_table() -> LootTable {
    LootTable {
        drops: vec![
            Loot::new(WorldObject::Coin, 1, 3, 0.6),
            Loot::new(WorldObject::XPShard, 1, 1, 0.8),
            Loot::new(WorldObject::SmallPotion, 1, 1, 0.06),
        ],
    }
}

fn loot_cycle_load_test_burst(
    mut commands: Commands,
    time: Res<Time>,
    mut state: ResMut<LootCycleLoadTestState>,
    active: Res<LootCycleLoadTestActive>,
    player: Query<&GlobalTransform, With<Player>>,
    mut proto_commands: ProtoCommands,
    proto_param: ProtoParam,
    graphics: Res<Graphics>,
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
    let loot_table = make_test_loot_table();

    for _ in 0..LOOT_CYCLE_DROPS_PER_WAVE {
        let drops = LootTablePlugin::get_drops(&loot_table, &proto_param, 0, Some(1), false);

        for drop in drops.iter() {
            let drop_offset = Vec2::new(rng.gen_range(-10.0..10.0), rng.gen_range(-10.0..10.0));
            let offset = random_offset(&mut rng, 80.0);
            let drop_pos = player_pos + offset + drop_offset;

            let drop_e = proto_commands.spawn_item_from_proto(
                drop.obj_type,
                &proto_param,
                drop_pos,
                drop.count,
                Some(1),
            );

            if let Some(drop_e) = drop_e {
                add_item_glows(&mut commands, &graphics, drop_e, drop.rarity.clone());
                commands
                    .entity(drop_e)
                    .insert(ItemDropDespawnTimer(Timer::from_seconds(
                        LOOT_CYCLE_DESPAWN_SECS,
                        TimerMode::Once,
                    )));
            }
        }
    }
}

// =============================================================================
// Diagnostics — entity/component/rapier counters every 5s (DIAGNOSTICS=1)
// =============================================================================

pub fn diagnostics_tick(
    time: Res<Time>,
    mut elapsed: Local<f32>,
    mut prev_total: Local<usize>,
    all_entities: Query<Entity>,
    mobs: Query<(), With<Mob>>,
    item_drops: Query<(), With<ItemDrop>>,
    colliders: Query<(), With<Collider>>,
    damage_numbers: Query<(), With<DamageNumber>>,
    move_ui_anims: Query<(), With<MoveUIAnimation>>,
    sound_spawners: Query<(), With<SoundSpawner>>,
    particle_effects: Query<(), With<ParticleEffect>>,
    marked_for_death: Query<(), With<MarkedForDeath>>,
    burning_and_pulled: Query<(), Or<(With<Burning>, With<BeingPulledToPlayer>)>>,
    queued_texts: Query<(), With<QueueFloatingText>>,
    rapier: Res<RapierContext>,
) {
    *elapsed += time.delta_seconds();
    if *elapsed < 5.0 {
        return;
    }
    *elapsed = 0.0;

    let total = all_entities.iter().count();
    let delta: i64 = total as i64 - *prev_total as i64;
    *prev_total = total;

    let rapier_bodies = rapier.bodies.len();
    let rapier_colliders = rapier.colliders.len();

    info!(
        "\n[DIAG] === Entity / Component Snapshot ===\n\
         [DIAG] total_entities={:<6} (delta {:+})\n\
         [DIAG] mobs={:<4} item_drops={:<4} colliders_bevy={:<4}\n\
         [DIAG] dmg_numbers={:<4} ui_movers={:<4} sound_spawners={:<4}\n\
         [DIAG] particles={:<4} marked_death={:<4} burning_or_pulled={:<4}\n\
         [DIAG] queued_texts={:<4}\n\
         [DIAG] rapier_bodies={:<5} rapier_colliders={:<5}\n\
         [DIAG] ==========================================",
        total, delta,
        mobs.iter().count(), item_drops.iter().count(), colliders.iter().count(),
        damage_numbers.iter().count(), move_ui_anims.iter().count(), sound_spawners.iter().count(),
        particle_effects.iter().count(), marked_for_death.iter().count(), burning_and_pulled.iter().count(),
        queued_texts.iter().count(),
        rapier_bodies, rapier_colliders,
    );
}
