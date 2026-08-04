use crate::aseprite_assets::{
    BombSprite, CherryBombExplosionSprite, CherryBombSprite, EnergyBallEffect,
};
use crate::aseprite_helpers::aseprite_bundle;
use bevy_aseprite_ultra::prelude::Aseprite;
use std::{collections::HashSet, f32::consts::TAU};

use bevy::prelude::*;
use bevy_rapier2d::prelude::{
    Collider, RapierContext, ReadRapierContext, RigidBody, Sensor, WriteRapierContext,
};
use rand::{seq::SliceRandom, Rng};

use crate::{
    animations::{AttackEvent, DoneAnimation},
    assets::Graphics,
    attributes::{
        modifiers::ModifyManaEvent, Attack, BonusAttackSpeed, CurrentHealth, CurrentMana,
        ManaRegen, MaxHealth, ProjectileSize,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    combat::{
        combat_helpers::{spawn_deferred_aseprite_collider, DespawnTimer},
        status_effects::{
            Burning, DeathDefianceTint, MobStatusEffects, StatusEffect, StatusEffectEvent,
        },
        EnemyDeathEvent, HitEvent, ObjBreakEvent,
    },
    custom_commands::CommandsExt,
    enemy::{EliteMob, Mob},
    item::{
        projectile::{
            AnimVisualCategory, HomingEnergyBall, Projectile, ProjectileState, RangedAttackEvent,
        },
        ItemDrop, WorldObject,
    },
    player::{
        skills::{
            active_skill_scaling::{attack_damage_multiplier, BOMB},
            ActiveSkillUsedEvent, Heirloom, HeirloomTriggerCounts, ManaGainSource, PlayerSkills,
        },
        Player,
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, y_sort::YSort, TileMapPosition, TILE_SIZE},
    GameParam,
};

const ANT_FARM_COOLDOWN: f32 = 1.5;
const ANT_SPEED: f32 = 180.0;
const ANT_CONTACT_DISTANCE: f32 = 8.0;
const ANT_LIFETIME: f32 = 4.0;
const ANT_CHAIN_DELAY: f32 = 0.25;

/// Time for one full orbit (rotation speed); faster = snappier feel.
const STONE_TOOTH_ORBIT_PERIOD: f32 = 2.0;
/// Delay between spawning the next batch.
const STONE_TOOTH_SPAWN_INTERVAL: f32 = 6.0;
const STONE_TOOTH_ROCK_LIFETIME: f32 = 4.0;
/// Distance rocks travel outward from the player over their lifetime.
const STONE_TOOTH_TRAVEL_DISTANCE: f32 = 40.0;
const STONE_CONTACT_DISTANCE: f32 = 28.0;

const REAPER_SOUL_SPEED: f32 = 220.0;
const REAPER_SOUL_LIFETIME: f32 = 6.0;
const REAPER_DAMAGE_PERCENT: f32 = 1.0;
const REAPER_CONTACT_DISTANCE: f32 = 12.0;
const REAPER_SOUL_MAX_SPAWN_RANGE: f32 = 320.0;
const REAPER_SOUL_DRIFT_STRENGTH: f32 = 0.75;
const REAPER_SOUL_DRIFT_FREQ: f32 = 10.5;

/// Summon Ring: piercing ring that travels in a line and bounces off solid objects.
const SUMMON_RING_COOLDOWN: f32 = 3.5;
const SUMMON_RING_SPEED: f32 = 230.0;
const SUMMON_RING_LIFETIME: f32 = 3.0;
const SUMMON_RING_COLLIDER_RADIUS: f32 = 10.0;

#[derive(Clone)]
struct MobSnapshot {
    entity: Entity,
    position: Vec2,
    max_health: i32,
    kind: Mob,
}

/// Tracks the AntFarm heirloom's spawn cooldown on the player. Added when the
/// heirloom is granted and removed when it's cleared — stored `SparseSet` so
/// granting/clearing doesn't move the player between archetypes.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AntFarmState {
    pub timer: Timer,
}

impl Default for AntFarmState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(ANT_FARM_COOLDOWN, TimerMode::Repeating),
        }
    }
}

#[derive(Component)]
pub struct AntFarmAnt {
    pub target: Option<Entity>,
    pub damage_fraction: f32,
    pub speed: f32,
    pub lifetime: Timer,
    pub spawn_delay: Timer,
    /// Matches player projectile size (size stat + Gigantify, etc.).
    pub size_multiplier: f32,
}

/// Tracks the StoneTooth heirloom's spawn cooldown on the player. Same
/// (un)equip churn profile as other heirloom state components — stored
/// `SparseSet`.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct StoneToothState {
    pub elapsed: f32,
}

#[derive(Component)]
pub struct OrbitingStone {
    pub owner: Entity,
    pub base_angle: f32,
    /// If true, rock is visible and can deal damage. Always true for pierce rocks; lifespan controls despawn.
    pub active: bool,
    /// Matches player projectile size for hit radius and sprite.
    pub size_multiplier: f32,
    /// Staggered render depth so overlapping rocks don't z-fight.
    pub z_slot: u8,
}

const STONE_TOOTH_Z_DEPTH_LAYERS: u8 = 20;
const STONE_TOOTH_Z_DEPTH_STEP: f32 = 0.02;

/// Tracks lifetime and which enemies this rock has already hit (pierce: hit each once).
#[derive(Component)]
pub struct StoneToothRockLifetime {
    pub lifetime: Timer,
    pub hit_entities: HashSet<Entity>,
}

/// Fired when healing triggers "all summons once" (e.g. HealSummons heirloom).
#[derive(Message)]
pub struct TriggerSummonsEvent(pub Entity);

/// Marker on the player while the Reaper heirloom is equipped. Stored
/// `SparseSet` so (un)equipping doesn't move the player between archetypes.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ReaperState;

#[derive(Component)]
pub struct ReaperSoul {
    pub target: Option<Entity>,
    pub damage_fraction: f32,
    pub speed: f32,
    pub lifetime: Timer,
    pub drift_phase: f32,
}

/// SummonRing heirloom cooldown on the player. Stored `SparseSet` for the
/// same reasons as the other heirloom state components.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct SummonRingState {
    pub timer: Timer,
}

impl Default for SummonRingState {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(SUMMON_RING_COOLDOWN, TimerMode::Repeating),
        }
    }
}

/// Minimum time between bounces so direction doesn't flip every frame while overlapping.
const SUMMON_RING_BOUNCE_COOLDOWN: f32 = 0.15;

#[derive(Component)]
pub struct SummonRingProjectile {
    pub owner: Entity,
    pub direction: Vec2,
    pub lifetime: Timer,
    pub hit_entities: HashSet<Entity>,
    /// After the 2s outward duration, the ring returns to the player in a straight line.
    pub returning: bool,
    /// Cooldown before the ring can bounce again (starts at 0 so first bounce is immediate).
    pub bounce_cooldown: Timer,
    /// Scaled collider radius (matches Rapier ball + bounce nudge).
    pub collider_radius: f32,
}

/// World objects that projectiles pass through (no bounce). Ring bounces off everything else that has a collider.
fn summon_ring_pass_through(obj: WorldObject) -> bool {
    matches!(
        obj,
        WorldObject::Grass
            | WorldObject::Grass2
            | WorldObject::Grass3
            | WorldObject::RedFlower
            | WorldObject::PinkFlower
            | WorldObject::YellowFlower
            | WorldObject::RedMushroom
            | WorldObject::BrownMushroom
            | WorldObject::Stick
    )
}

fn gather_live_mobs(
    mobs: &Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
) -> Vec<MobSnapshot> {
    mobs.iter()
        .filter(|(_, _, health, _, _)| health.0 > 0)
        .map(|(entity, transform, _, max_health, mob)| MobSnapshot {
            entity,
            position: transform.translation().truncate(),
            max_health: max_health.0,
            kind: mob.clone(),
        })
        .collect()
}

fn pick_target(
    current_target: Option<Entity>,
    origin: Vec2,
    claimed_targets: &mut HashSet<Entity>,
    mobs: &[MobSnapshot],
) -> Option<MobSnapshot> {
    if let Some(target) = current_target {
        if let Some(snapshot) = mobs.iter().find(|mob| mob.entity == target) {
            if claimed_targets.insert(snapshot.entity) {
                return Some(snapshot.clone());
            }
        }
    }

    let mut best: Option<MobSnapshot> = None;
    let mut best_dist = f32::MAX;

    for snapshot in mobs
        .iter()
        .filter(|mob| !claimed_targets.contains(&mob.entity))
    {
        let dist_sq = snapshot.position.distance_squared(origin);
        if dist_sq < best_dist {
            best_dist = dist_sq;
            best = Some(snapshot.clone());
        }
    }

    if best.is_none() {
        for snapshot in mobs {
            let dist_sq = snapshot.position.distance_squared(origin);
            if dist_sq < best_dist {
                best_dist = dist_sq;
                best = Some(snapshot.clone());
            }
        }
    }

    if let Some(snapshot) = &best {
        claimed_targets.insert(snapshot.entity);
    }

    best
}

/// Summon damage = player damage (same calculation as skills: attack, crit, bonuses, frail, etc).
/// Returns (damage, was_crit, was_overcrit).
fn calculate_summon_damage(
    _commands: &mut Commands,
    game: &GameParam,
    _target: Entity,
    frail_stacks: u8,
) -> (i32, bool, bool) {
    let (damage, was_crit, was_overcrit) =
        game.calculate_player_damage(0, None, 0, None, frail_stacks, 0, false);
    (i32::max(1, damage as i32), was_crit, was_overcrit)
}

fn get_world_object_sprite(graphics: &Graphics, object: WorldObject) -> Option<Sprite> {
    graphics
        .spritesheet_map
        .as_ref()
        .and_then(|map| map.get(&object).cloned())
}

// ----- Summon helpers: shared spawn logic for timer-based and on-heal triggers -----

/// Spawns up to `count` Ant Farm ants. Deducts mana per ant if `mana_value` is `Some`.
/// Returns the number actually spawned.
pub fn spawn_ant_farm_ants(
    commands: &mut Commands,
    graphics: &Graphics,
    player_pos: Vec3,
    count: usize,
    mana_value: &mut Option<&mut i32>,
    mana_cost_per: i32,
    size_multiplier: f32,
) -> usize {
    let mut rng = rand::thread_rng();
    let mut spawned = 0;
    for i in 0..count {
        if let Some(mana) = mana_value.as_mut() {
            let current = **mana;
            if current < mana_cost_per {
                break;
            }
            **mana = current - mana_cost_per;
        }
        let angle = rng.gen_range(0.0..TAU);
        let distance = rng.gen_range(0.0..6.0);
        let offset = Vec2::from_angle(angle) * distance;
        let mut sprite = graphics.get_heirloom_icon(Heirloom::AntFarm);
        sprite.custom_size = Some(Vec2::splat(12.0 * size_multiplier));
        commands.spawn((
            sprite.clone(),
            Transform::from_translation(player_pos + Vec3::new(offset.x, offset.y, 0.2)),
            AntFarmAnt {
                target: None,
                damage_fraction: 2.0,
                speed: ANT_SPEED,
                lifetime: Timer::from_seconds(ANT_LIFETIME, TimerMode::Once),
                spawn_delay: Timer::from_seconds(i as f32 * ANT_CHAIN_DELAY, TimerMode::Once),
                size_multiplier,
            },
            AnimVisualCategory::Heirloom,
            YSort(-0.2),
            Name::new("AntFarmAnt"),
        ));
        spawned += 1;
    }
    spawned
}

/// Spawns up to `count` Stone Tooth rocks (orbit, pierce, 2s lifespan). Deducts mana per rock if `mana_value` is `Some`.
/// Returns the number actually spawned.
pub fn spawn_stone_tooth_rocks(
    commands: &mut Commands,
    graphics: &Graphics,
    player_e: Entity,
    player_pos: Vec3,
    count: usize,
    mana_value: &mut Option<&mut i32>,
    mana_cost_per: i32,
    size_multiplier: f32,
    z_slot_offset: usize,
) -> usize {
    let mut spawned = 0;
    for index in 0..count {
        if let Some(mana) = mana_value.as_mut() {
            let current = **mana;
            if current < mana_cost_per {
                break;
            }
            **mana = current - mana_cost_per;
        }
        let base_angle = if count > 0 {
            TAU * index as f32 / count as f32
        } else {
            0.0
        };
        let z_slot = ((z_slot_offset + index) % STONE_TOOTH_Z_DEPTH_LAYERS as usize) as u8;
        // Rocks start at player and expand outward (offset applied in update_stone_tooth).
        let offset = Vec2::ZERO;
        if let Some(sprite_sheet) = &graphics.spritesheet_map {
            if let Some(sprite) = sprite_sheet.get(&WorldObject::BoulderHeirloom) {
                let mut scaled_sprite = sprite.clone();
                scaled_sprite.custom_size = Some(Vec2::splat(32.0) * size_multiplier);
                let transform =
                    Transform::from_translation(player_pos + Vec3::new(offset.x, offset.y, 0.25));
                commands.spawn((
                    scaled_sprite.clone(),
                    transform,
                    OrbitingStone {
                        owner: player_e,
                        base_angle,
                        active: true,
                        size_multiplier,
                        z_slot,
                    },
                    StoneToothRockLifetime {
                        lifetime: Timer::from_seconds(STONE_TOOTH_ROCK_LIFETIME, TimerMode::Once),
                        hit_entities: HashSet::new(),
                    },
                    AnimVisualCategory::Heirloom,
                    YSort(-0.1),
                    Name::new("StoneToothRock"),
                ));
                spawned += 1;
            }
        }
    }
    spawned
}

/// Spawns up to `count` Summon Ring projectiles (piercing, 2s outward then returns to player). Deducts mana per ring if `mana_value` is `Some`.
pub fn spawn_summon_ring_rings(
    commands: &mut Commands,
    graphics: &Graphics,
    player_e: Entity,
    player_pos: Vec3,
    count: usize,
    mana_value: &mut Option<&mut i32>,
    mana_cost_per: i32,
    size_multiplier: f32,
) -> usize {
    let mut rng = rand::thread_rng();
    let mut spawned = 0;
    let count_float = count.max(1) as f32;
    let collider_radius = SUMMON_RING_COLLIDER_RADIUS * size_multiplier;
    for index in 0..count {
        if let Some(mana) = mana_value.as_mut() {
            let current = **mana;
            if current < mana_cost_per {
                break;
            }
            **mana = current - mana_cost_per;
        }
        // Spread rings evenly around the circle so multiple copies don't share the same direction.
        let segment = TAU / count_float;
        let angle = index as f32 * segment + rng.gen_range(0.0..segment);
        let direction = Vec2::from_angle(angle);
        let mut sprite = graphics.get_heirloom_icon(Heirloom::SummonRing);
        sprite.custom_size = Some(Vec2::splat(16.0 * size_multiplier));
        let rotation = Quat::from_rotation_z(angle);
        commands.spawn((
            sprite.clone(),
            Transform {
                translation: player_pos + Vec3::new(0., 0., 0.25),
                rotation,
                ..default()
            },
            SummonRingProjectile {
                owner: player_e,
                direction,
                lifetime: Timer::from_seconds(SUMMON_RING_LIFETIME, TimerMode::Once),
                hit_entities: HashSet::new(),
                returning: false,
                bounce_cooldown: Timer::from_seconds(0.0, TimerMode::Once),
                collider_radius,
            },
            AnimVisualCategory::Heirloom,
            RigidBody::KinematicPositionBased,
            Sensor,
            Collider::ball(collider_radius),
            YSort(-0.1),
            Name::new("SummonRing"),
        ));
        spawned += 1;
    }
    spawned
}

pub fn handle_ant_farm_state(
    mut commands: Commands,
    time: Res<Time>,
    mut player_query: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &ProjectileSize,
            Option<&mut AntFarmState>,
            &mut CurrentMana,
        ),
        With<Player>,
    >,
    graphics: Res<Graphics>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((player_e, player_txfm, skills, projectile_size, mut state_option, mut curr_mana)) =
        player_query.single_mut()
    else {
        return;
    };
    let size_mult = projectile_size.get_multiplier();
    let stacks = skills.get_count(Heirloom::AntFarm);
    let player_pos = player_txfm.translation();
    let had_state = state_option.is_some();
    let mut spawn_count = None;

    if stacks > 0 {
        if let Some(state) = state_option.as_mut() {
            state.timer.tick(time.delta());
            if state.timer.just_finished() {
                spawn_count = Some(stacks);
            }
        } else {
            spawn_count = Some(stacks);
        }
    }

    if stacks > 0 && !had_state {
        commands.entity(player_e).insert(AntFarmState::default());
    } else if stacks <= 0 && had_state {
        commands.entity(player_e).remove::<AntFarmState>();
    }

    let Some(count_i32) = spawn_count else {
        return;
    };

    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        return;
    }

    let count_usize = count_i32.max(0) as usize;
    let mut mana_opt = Some(&mut curr_mana.0);
    let mana_cost_per = Heirloom::AntFarm.get_mana_cost();
    let spawned = spawn_ant_farm_ants(
        &mut commands,
        &graphics,
        player_pos,
        count_usize,
        &mut mana_opt,
        mana_cost_per,
        size_mult,
    );
    if spawned > 0 {
        trigger_counts.record_mana(Heirloom::AntFarm, mana_cost_per * spawned as i32);
        trigger_counts.increment(Heirloom::AntFarm);
    }
}

pub fn handle_summon_ring_state(
    mut commands: Commands,
    time: Res<Time>,
    mut player_query: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &ProjectileSize,
            Option<&mut SummonRingState>,
            &mut CurrentMana,
        ),
        With<Player>,
    >,
    graphics: Res<Graphics>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((player_e, player_txfm, skills, projectile_size, mut state_option, mut curr_mana)) =
        player_query.single_mut()
    else {
        return;
    };
    let size_mult = projectile_size.get_multiplier();
    let stacks = skills.get_count(Heirloom::SummonRing);
    let player_pos = player_txfm.translation();
    let had_state = state_option.is_some();
    let mut spawn_count = None;

    if stacks > 0 {
        if let Some(state) = state_option.as_mut() {
            state.timer.tick(time.delta());
            if state.timer.just_finished() {
                spawn_count = Some(stacks);
            }
        } else {
            spawn_count = Some(stacks);
        }
    }

    if stacks > 0 && !had_state {
        commands.entity(player_e).insert(SummonRingState::default());
    } else if stacks <= 0 && had_state {
        commands.entity(player_e).remove::<SummonRingState>();
    }

    let Some(count_i32) = spawn_count else {
        return;
    };

    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        return;
    }

    let count_usize = count_i32.max(0) as usize;
    let mut mana_opt = Some(&mut curr_mana.0);
    let mana_cost_per = Heirloom::SummonRing.get_mana_cost();
    let spawned = spawn_summon_ring_rings(
        &mut commands,
        &graphics,
        player_e,
        player_pos,
        count_usize,
        &mut mana_opt,
        mana_cost_per,
        size_mult,
    );
    if spawned > 0 {
        trigger_counts.record_mana(Heirloom::SummonRing, mana_cost_per * spawned as i32);
        trigger_counts.increment(Heirloom::SummonRing);
    }
}

const SUMMON_RING_RETURN_REACH_DISTANCE: f32 = 12.0;

pub fn update_summon_ring(
    mut commands: Commands,
    time: Res<Time>,
    rapier_context: ReadRapierContext,
    mut rings: Query<(Entity, &mut Transform, &mut SummonRingProjectile)>,
    player_transforms: Query<&GlobalTransform, With<Player>>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    world_objects: Query<
        (Entity, &GlobalTransform, &WorldObject),
        (Without<Mob>, Without<ItemDrop>),
    >,
    player_entity: Query<Entity, With<Player>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: MessageWriter<HitEvent>,
    game: GameParam,
) {
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };
    let mob_snapshots = gather_live_mobs(&mobs);
    let delta = time.delta_secs();
    let move_step = SUMMON_RING_SPEED * delta;

    let mut to_despawn = Vec::new();
    for (ring_entity, mut transform, mut ring) in rings.iter_mut() {
        let ring_pos = transform.translation.truncate();

        if ring.returning {
            let Ok(player_txfm) = player_transforms.get(ring.owner) else {
                to_despawn.push(ring_entity);
                continue;
            };
            let player_pos = player_txfm.translation().truncate();
            let to_player = player_pos - ring_pos;
            let dist = to_player.length();
            if dist <= SUMMON_RING_RETURN_REACH_DISTANCE {
                to_despawn.push(ring_entity);
                continue;
            }
            let direction = to_player.normalize_or_zero();
            let step = move_step.min(dist);
            let new_pos = ring_pos + direction * step;
            transform.translation = new_pos.extend(transform.translation.z);
            transform.rotation = Quat::from_rotation_z(direction.y.atan2(direction.x));

            // Pierce and damage mobs on the way back too.
            for (e1, e2, _) in rapier_context.intersection_pairs_with(ring_entity) {
                let other = if e1 == ring_entity { e2 } else { e1 };
                if other == ring_entity {
                    continue;
                }
                if player_entity.get(other).is_ok() {
                    continue;
                }
                if let Some(snapshot) = mob_snapshots.iter().find(|s| s.entity == other) {
                    if ring.hit_entities.contains(&other) {
                        continue;
                    }
                    ring.hit_entities.insert(other);
                    let dir = (snapshot.position - new_pos).normalize_or_zero();
                    let frail_stacks = frail_query
                        .get(other)
                        .map(|s| s.frail_stacks())
                        .unwrap_or(0);
                    let (damage, was_crit, was_overcrit) =
                        calculate_summon_damage(&mut commands, &game, other, frail_stacks);
                    hit_events.write(HitEvent {
                        hit_entity: other,
                        damage,
                        dir,
                        hit_with_melee: None,
                        hit_with_projectile: None,
                        hit_by_mob: None,
                        hit_by_pet: None,
                        was_crit,
                        was_overcrit,
                        ignore_tool: true,
                        from_heirloom_effect: Some(Heirloom::SummonRing),
                        from_active_skill: false,
                    });
                }
            }
            continue;
        }

        ring.lifetime.tick(time.delta());
        ring.bounce_cooldown.tick(time.delta());
        if ring.lifetime.is_finished() {
            ring.returning = true;
            continue;
        }

        let new_pos = ring_pos + ring.direction * move_step;
        transform.translation = new_pos.extend(transform.translation.z);
        transform.rotation = Quat::from_rotation_z(ring.direction.y.atan2(ring.direction.x));

        for (e1, e2, _) in rapier_context.intersection_pairs_with(ring_entity) {
            let other = if e1 == ring_entity { e2 } else { e1 };
            if other == ring_entity {
                continue;
            }
            if player_entity.get(other).is_ok() {
                continue;
            }

            // Pierce mobs (never bounce off them).
            if let Some(snapshot) = mob_snapshots.iter().find(|s| s.entity == other) {
                if ring.hit_entities.contains(&other) {
                    continue;
                }
                ring.hit_entities.insert(other);
                let dir = (snapshot.position - new_pos).normalize_or_zero();
                let frail_stacks = frail_query
                    .get(other)
                    .map(|s| s.frail_stacks())
                    .unwrap_or(0);
                let (damage, was_crit, was_overcrit) =
                    calculate_summon_damage(&mut commands, &game, other, frail_stacks);
                hit_events.write(HitEvent {
                    hit_entity: other,
                    damage,
                    dir,
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    hit_by_mob: None,
                    hit_by_pet: None,
                    was_crit,
                    was_overcrit,
                    ignore_tool: true,
                    from_heirloom_effect: Some(Heirloom::SummonRing),
                    from_active_skill: false,
                });
                continue;
            }

            // Bounce off solid world objects only, and only when bounce cooldown has elapsed.
            if mob_snapshots.iter().any(|s| s.entity == other) {
                continue;
            }
            if let Ok((_, obj_txfm, obj)) = world_objects.get(other) {
                if summon_ring_pass_through(*obj) {
                    continue;
                }
                if !ring.bounce_cooldown.is_finished() {
                    continue;
                }
                let obj_pos = obj_txfm.translation().truncate();
                let delta = new_pos - obj_pos;

                // Determine which axis we're hitting based on the ring's approach direction
                // relative to the object. Use the axis where the ring is moving *into* the
                // object most strongly, then flip that component for a clean reflection.
                let dx = delta.x.abs();
                let dy = delta.y.abs();

                let mut bounced = false;
                if dx > dy {
                    // Approached from the side -> flip X
                    if ring.direction.x.abs() > 0.01 {
                        ring.direction.x = -ring.direction.x;
                        bounced = true;
                    }
                } else {
                    // Approached from top/bottom -> flip Y
                    if ring.direction.y.abs() > 0.01 {
                        ring.direction.y = -ring.direction.y;
                        bounced = true;
                    }
                }

                if bounced {
                    ring.direction = ring.direction.normalize_or_zero();
                    ring.bounce_cooldown =
                        Timer::from_seconds(SUMMON_RING_BOUNCE_COOLDOWN, TimerMode::Once);
                    // Push ring out of the object so it doesn't re-trigger
                    let push_dir = if dx > dy {
                        Vec2::new(delta.x.signum(), 0.0)
                    } else {
                        Vec2::new(0.0, delta.y.signum())
                    };
                    let nudge = (SUMMON_RING_COLLIDER_RADIUS - 4.0)
                        * (ring.collider_radius / SUMMON_RING_COLLIDER_RADIUS);
                    transform.translation += (push_dir * nudge).extend(0.0);
                    // Only one bounce per frame to avoid double-reflection from multiple
                    // intersections (e.g. same wall reported twice or corner with two colliders).
                    break;
                }
            }
        }
    }
    for e in to_despawn {
        commands.entity(e).despawn();
    }
}

pub fn update_ant_farm_ants(
    mut commands: Commands,
    time: Res<Time>,
    mut ants: Query<(Entity, &mut Transform, &mut AntFarmAnt)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: MessageWriter<HitEvent>,
    game: GameParam,
) {
    let mob_snapshots = gather_live_mobs(&mobs);

    if mob_snapshots.is_empty() {
        for (entity, _, mut ant) in ants.iter_mut() {
            ant.lifetime.tick(time.delta());
            if ant.lifetime.is_finished() {
                commands.entity(entity).despawn();
                continue;
            }
            ant.spawn_delay.tick(time.delta());
        }
        return;
    }

    let mut claimed_targets: HashSet<Entity> = HashSet::new();

    for (entity, mut transform, mut ant) in ants.iter_mut() {
        ant.lifetime.tick(time.delta());
        if ant.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        ant.spawn_delay.tick(time.delta());
        if !ant.spawn_delay.is_finished() {
            continue;
        }

        let ant_pos = transform.translation.truncate();

        let target_snapshot =
            pick_target(ant.target, ant_pos, &mut claimed_targets, &mob_snapshots);

        let Some(snapshot) = target_snapshot else {
            ant.target = None;
            continue;
        };

        ant.target = Some(snapshot.entity);

        let to_target = snapshot.position - ant_pos;
        let distance = to_target.length();
        let direction = to_target.normalize_or_zero();
        let step = (ant.speed * time.delta_secs()).min(distance);
        transform.translation += (direction * step).extend(0.0);

        let contact_dist = ANT_CONTACT_DISTANCE * ant.size_multiplier;
        if transform.translation.truncate().distance(snapshot.position) <= contact_dist {
            let frail_stacks = frail_query
                .get(snapshot.entity)
                .map(|s| s.frail_stacks())
                .unwrap_or(0);
            let (damage, was_crit, was_overcrit) =
                calculate_summon_damage(&mut commands, &game, snapshot.entity, frail_stacks);
            hit_events.write(HitEvent {
                hit_entity: snapshot.entity,
                damage,
                dir: direction,
                hit_with_melee: None,
                hit_with_projectile: None,
                hit_by_mob: None,
                hit_by_pet: None,
                was_crit,
                was_overcrit,
                ignore_tool: true,
                from_heirloom_effect: Some(Heirloom::AntFarm),
                from_active_skill: false,
            });
            commands.entity(entity).despawn();
        }
    }
}

pub fn update_stone_tooth(
    mut commands: Commands,
    time: Res<Time>,
    mut player_query: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &ProjectileSize,
            Option<&mut StoneToothState>,
            &mut CurrentMana,
        ),
        With<Player>,
    >,
    mut stones: Query<(
        Entity,
        &OrbitingStone,
        &mut Transform,
        Option<&mut StoneToothRockLifetime>,
    )>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: MessageWriter<HitEvent>,
    graphics: Res<Graphics>,
    mut game: GameParam,
) {
    let Ok((player_e, player_txfm, skills, projectile_size, mut state_option, mut curr_mana)) =
        player_query.single_mut()
    else {
        return;
    };
    let size_mult = projectile_size.get_multiplier();

    let stacks = skills.get_count(Heirloom::StoneTooth);
    let player_pos = player_txfm.translation();
    let player_xy = player_pos.truncate();
    let had_state = state_option.is_some();

    let mut should_spawn_stones = false;

    if stacks > 0 {
        if let Some(state) = state_option.as_mut() {
            state.elapsed += time.delta_secs();
            if state.elapsed >= STONE_TOOTH_SPAWN_INTERVAL {
                state.elapsed %= STONE_TOOTH_SPAWN_INTERVAL;
                should_spawn_stones = true;
            }
        } else {
            should_spawn_stones = true;
        }
    }

    if stacks > 0 && !had_state {
        commands.entity(player_e).insert(StoneToothState::default());
    } else if stacks <= 0 && had_state {
        commands.entity(player_e).remove::<StoneToothState>();
    }

    if stacks <= 0 {
        for (entity, stone, _, _) in stones.iter() {
            if stone.owner == player_e {
                commands.entity(entity).despawn();
            }
        }
        return;
    }

    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        return;
    }

    let mob_snapshots = gather_live_mobs(&mobs);

    // Tick lifetime, despawn expired rocks, update position and hit (pierce, hit each enemy once)
    let mut to_despawn = Vec::new();
    for (entity, stone, mut transform, lifetime_option) in stones.iter_mut() {
        if stone.owner != player_e {
            continue;
        }
        let Some(mut lifetime) = lifetime_option else {
            continue;
        };
        lifetime.lifetime.tick(time.delta());
        if lifetime.lifetime.is_finished() {
            to_despawn.push(entity);
            continue;
        }
        let elapsed = lifetime.lifetime.elapsed().as_secs_f32();
        let angle = stone.base_angle + (elapsed / STONE_TOOTH_ORBIT_PERIOD) * TAU;
        // Expand outward from player over lifetime (0 -> STONE_TOOTH_TRAVEL_DISTANCE).
        // Scale the orbit radius with the rock's size so larger rocks orbit
        // further out instead of clipping into the player.
        let radius = ((elapsed / STONE_TOOTH_ROCK_LIFETIME) * STONE_TOOTH_TRAVEL_DISTANCE + 24.)
            * stone.size_multiplier;
        let offset = Vec2::from_angle(angle) * radius;
        transform.translation = Vec3::new(
            player_xy.x + offset.x,
            player_xy.y + offset.y,
            player_pos.z + 0.25 + stone.z_slot as f32 * STONE_TOOTH_Z_DEPTH_STEP,
        );
        let stone_pos = transform.translation.truncate();
        for snapshot in mob_snapshots.iter() {
            if lifetime.hit_entities.contains(&snapshot.entity) {
                continue;
            }
            let mob_pos = snapshot.position;
            let contact = STONE_CONTACT_DISTANCE * stone.size_multiplier;
            if mob_pos.distance_squared(stone_pos) <= contact * contact {
                let dir = (mob_pos - stone_pos).normalize_or_zero();
                let frail_stacks = frail_query
                    .get(snapshot.entity)
                    .map(|s| s.frail_stacks())
                    .unwrap_or(0);
                let (damage, was_crit, was_overcrit) =
                    calculate_summon_damage(&mut commands, &game, snapshot.entity, frail_stacks);
                hit_events.write(HitEvent {
                    hit_entity: snapshot.entity,
                    damage,
                    dir,
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    hit_by_mob: None,
                    hit_by_pet: None,
                    was_crit,
                    was_overcrit,
                    ignore_tool: true,
                    from_heirloom_effect: Some(Heirloom::StoneTooth),
                    from_active_skill: false,
                });
                lifetime.hit_entities.insert(snapshot.entity);
            }
        }
    }
    for entity in to_despawn {
        commands.entity(entity).despawn();
    }

    // On timer: spawn a batch of rocks (orbit 1.5s, then ~3s delay before next batch)
    if !should_spawn_stones {
        return;
    }
    let active_rock_count = stones
        .iter()
        .filter(|(_, stone, _, lifetime_option)| {
            stone.owner == player_e
                && lifetime_option
                    .map(|lifetime| !lifetime.lifetime.is_finished())
                    .unwrap_or(false)
        })
        .count();
    let mut mana_opt = Some(&mut curr_mana.0);
    let mana_cost_per = Heirloom::StoneTooth.get_mana_cost();
    let spawned = spawn_stone_tooth_rocks(
        &mut commands,
        &graphics,
        player_e,
        player_pos,
        stacks as usize,
        &mut mana_opt,
        mana_cost_per,
        size_mult,
        active_rock_count,
    );
    if spawned > 0 {
        game.heirloom_trigger_counts
            .record_mana(Heirloom::StoneTooth, mana_cost_per * spawned as i32);
        game.heirloom_trigger_counts.increment(Heirloom::StoneTooth);
    }
}

pub fn handle_trigger_summons_on_heal(
    mut commands: Commands,
    mut trigger_events: MessageReader<TriggerSummonsEvent>,
    mut player_query: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &ProjectileSize,
            &mut CurrentMana,
            Option<&mut AntFarmState>,
            Option<&mut StoneToothState>,
            Option<&mut SummonRingState>,
        ),
        With<Player>,
    >,
    stones: Query<(&OrbitingStone, Option<&StoneToothRockLifetime>)>,
    graphics: Res<Graphics>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        return;
    }
    for event in trigger_events.read() {
        let Ok((
            player_e,
            player_txfm,
            skills,
            projectile_size,
            mut curr_mana,
            mut ant_state,
            mut stone_state,
            mut ring_state,
        )) = player_query.get_mut(event.0)
        else {
            continue;
        };
        let size_mult = projectile_size.get_multiplier();
        let player_pos = player_txfm.translation();
        trigger_counts.increment(Heirloom::HealSummons);

        // Finish the cooldown: trigger one "tick" of each summon (same count as timer would spawn), then reset timers.
        let ant_stacks = skills.get_count(Heirloom::AntFarm).max(0) as usize;
        if ant_stacks >= 1 {
            let mut mana_opt = Some(&mut curr_mana.0);
            spawn_ant_farm_ants(
                &mut commands,
                &graphics,
                player_pos,
                ant_stacks,
                &mut mana_opt,
                0,
                size_mult,
            );
            if let Some(ref mut state) = ant_state {
                state.timer.reset();
            }
        }

        let stone_stacks = skills.get_count(Heirloom::StoneTooth).max(0) as usize;
        if stone_stacks >= 1 {
            let active_rock_count = stones
                .iter()
                .filter(|(stone, lifetime_option)| {
                    stone.owner == player_e
                        && lifetime_option
                            .map(|lifetime| !lifetime.lifetime.is_finished())
                            .unwrap_or(false)
                })
                .count();
            let mut mana_opt = Some(&mut curr_mana.0);
            spawn_stone_tooth_rocks(
                &mut commands,
                &graphics,
                player_e,
                player_pos,
                stone_stacks,
                &mut mana_opt,
                0,
                size_mult,
                active_rock_count,
            );
            if let Some(ref mut state) = stone_state {
                state.elapsed = 0.0;
            }
        }

        let ring_stacks = skills.get_count(Heirloom::SummonRing).max(0) as usize;
        if ring_stacks >= 1 {
            let player_pos = player_txfm.translation();
            let mut mana_opt = Some(&mut curr_mana.0);
            spawn_summon_ring_rings(
                &mut commands,
                &graphics,
                player_e,
                player_pos,
                ring_stacks,
                &mut mana_opt,
                0,
                size_mult,
            );
            if let Some(ref mut state) = ring_state {
                state.timer.reset();
            }
        }
    }
}

pub fn handle_reaper_soul_spawns(
    mut commands: Commands,
    mut death_events: MessageReader<EnemyDeathEvent>,
    mut player_query: Query<(&PlayerSkills, &mut CurrentMana), With<Player>>,
    graphics: Res<Graphics>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, mut curr_mana)) = player_query.single_mut() else {
        return;
    };
    let stacks = skills.get_count(Heirloom::Reaper);
    let spawn_count = stacks.max(0) as usize;

    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        return;
    }
    let Some(sprite_template) = get_world_object_sprite(&graphics, WorldObject::ReaperSoul) else {
        return;
    };

    let mob_snapshots = gather_live_mobs(&mobs);
    if mob_snapshots.is_empty() || spawn_count == 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    for event in death_events.read() {
        let mut best_target: Option<&MobSnapshot> = None;
        let mut best_dist_sq = REAPER_SOUL_MAX_SPAWN_RANGE * REAPER_SOUL_MAX_SPAWN_RANGE;
        for snapshot in mob_snapshots.iter() {
            let dist_sq = snapshot.position.distance_squared(event.enemy_pos);
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_target = Some(snapshot);
            }
        }

        let Some(best_snapshot) = best_target else {
            continue;
        };
        trigger_counts.increment(Heirloom::Reaper);

        for i in 0..spawn_count {
            let mana_cost = Heirloom::Reaper.get_mana_cost();
            if curr_mana.0 >= mana_cost {
                curr_mana.0 -= mana_cost;
                trigger_counts.record_mana(Heirloom::Reaper, mana_cost);
            } else {
                break;
            }
            let offset = Vec2::from_angle(rng.gen_range(0.0..TAU)) * rng.gen_range(0.0..4.0);
            let mut sprite = sprite_template.clone();
            sprite.custom_size = Some(Vec2::splat(14.0));
            commands.spawn((
                sprite.clone(),
                Transform::from_translation(
                    (event.enemy_pos + offset).extend(0.3 + i as f32 * 0.01),
                ),
                ReaperSoul {
                    target: Some(best_snapshot.entity),
                    damage_fraction: REAPER_DAMAGE_PERCENT,
                    speed: REAPER_SOUL_SPEED,
                    lifetime: Timer::from_seconds(REAPER_SOUL_LIFETIME, TimerMode::Once),
                    drift_phase: rng.gen_range(0.0..TAU),
                },
                AnimVisualCategory::Heirloom,
                YSort(-0.15),
                Name::new("ReaperSoul"),
            ));
        }
    }
}

pub fn handle_mana_orb_drops(
    mut commands: Commands,
    proto: ProtoParam,
    mut death_events: MessageReader<EnemyDeathEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
    player_skills: Query<&PlayerSkills, With<Player>>,
) {
    let mut rng = rand::thread_rng();
    const MANA_ORB_DROP_CHANCE: f64 = 0.1;
    let drop_mult = 1.0
        + player_skills
            .single()
            .map(|s| s.get_count(Heirloom::ManaOrbDropMult) as f64)
            .unwrap_or(0.0);
    let roll_chance = (MANA_ORB_DROP_CHANCE * drop_mult).min(1.0);
    for event in death_events.read() {
        if !rng.gen_bool(roll_chance) {
            continue;
        }
        let offset = Vec2::new(rng.gen_range(-10.0..10.0), rng.gen_range(-10.0..10.0));
        commands.spawn_item_from_proto(
            WorldObject::ManaOrb,
            &proto,
            event.enemy_pos + offset,
            1,
            None,
        );
        trigger_counts.increment(Heirloom::ManaOrbs);
    }
}

/// Drop mana orbs on boss/elite hits at the same flat rate as on kills.
pub fn handle_boss_hit_mana_orb_drops(
    mut commands: Commands,
    proto: ProtoParam,
    mut hit_events: MessageReader<HitEvent>,
    mobs: Query<(&Mob, &GlobalTransform, Option<&EliteMob>)>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
    player_skills: Query<&PlayerSkills, With<Player>>,
) {
    let mut rng = rand::thread_rng();
    const MANA_ORB_DROP_CHANCE: f64 = 0.01;
    let drop_mult = 1.0
        + player_skills
            .single()
            .map(|s| s.get_count(Heirloom::ManaOrbDropMult) as f64)
            .unwrap_or(0.0);
    let roll_chance = (MANA_ORB_DROP_CHANCE * drop_mult).min(1.0);

    for hit in hit_events.read() {
        // Check if hit entity is a boss
        let Ok((mob, boss_transform, is_elite)) = mobs.get(hit.hit_entity) else {
            continue;
        };

        if !mob.is_boss() && is_elite.is_none() {
            continue;
        }

        if !rng.gen_bool(roll_chance) {
            continue;
        }

        // Spawn mana orb within 32px of boss location
        let boss_pos = boss_transform.translation().truncate();
        let angle = rng.gen_range(0.0..TAU);
        let distance = rng.gen_range(0.0..32.0);
        let offset = Vec2::new(angle.cos(), angle.sin()) * distance;

        commands.spawn_item_from_proto(WorldObject::ManaOrb, &proto, boss_pos + offset, 1, None);
        trigger_counts.increment(Heirloom::ManaOrbs);
    }
}

pub fn update_reaper_souls(
    mut commands: Commands,
    time: Res<Time>,
    mut souls: Query<(Entity, &mut Transform, &mut ReaperSoul)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: MessageWriter<HitEvent>,
    game: GameParam,
) {
    let mob_snapshots = gather_live_mobs(&mobs);

    // If no mobs exist, despawn all souls immediately
    if mob_snapshots.is_empty() {
        for (entity, _, _) in souls.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let mut claimed_targets: HashSet<Entity> = HashSet::new();

    for (entity, mut transform, mut soul) in souls.iter_mut() {
        soul.lifetime.tick(time.delta());
        if soul.lifetime.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }

        let soul_pos = transform.translation.truncate();
        let target_snapshot =
            pick_target(soul.target, soul_pos, &mut claimed_targets, &mob_snapshots);

        // If no target can be found, despawn the soul
        let Some(snapshot) = target_snapshot else {
            commands.entity(entity).despawn();
            continue;
        };

        soul.target = Some(snapshot.entity);

        let to_target = snapshot.position - soul_pos;
        let direction = to_target.normalize_or_zero();
        if direction.length_squared() == 0.0 {
            continue;
        }
        soul.drift_phase += REAPER_SOUL_DRIFT_FREQ * time.delta_secs();
        let drift_amount = soul.drift_phase.sin() * REAPER_SOUL_DRIFT_STRENGTH;
        let perp = Vec2::new(-direction.y, direction.x);
        let mut steering = direction + perp * drift_amount;
        steering = steering.normalize_or_zero();
        let step = soul.speed * time.delta_secs();
        transform.translation += (steering * step).extend(0.0);

        if transform.translation.truncate().distance(snapshot.position) <= REAPER_CONTACT_DISTANCE {
            let frail_stacks = frail_query
                .get(snapshot.entity)
                .map(|s| s.frail_stacks())
                .unwrap_or(0);
            let (damage, was_crit, was_overcrit) =
                calculate_summon_damage(&mut commands, &game, snapshot.entity, frail_stacks);
            hit_events.write(HitEvent {
                hit_entity: snapshot.entity,
                damage,
                dir: direction,
                hit_with_melee: None,
                hit_with_projectile: None,
                hit_by_mob: None,
                hit_by_pet: None,
                was_crit,
                was_overcrit,
                ignore_tool: true,
                from_heirloom_effect: Some(Heirloom::Reaper),
                from_active_skill: false,
            });
            commands.entity(entity).despawn();
        }
    }
}

pub fn break_crates_with_roll(
    game: GameParam,
    rapier_context: ReadRapierContext,
    crate_query: Query<(Entity, &GlobalTransform, &WorldObject)>,
    mut obj_break_events: MessageWriter<ObjBreakEvent>,
    mut broken_this_frame: Local<HashSet<Entity>>,
) {
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };
    broken_this_frame.clear();

    let player_entity = game.game.player;

    let player_state = game.player();
    if !player_state.is_dashing {
        return;
    }

    let mut to_break: Vec<(Entity, WorldObject, TileMapPosition)> = Vec::new();
    for (first, second, _) in rapier_context.intersection_pairs_with(player_entity) {
        let other = if first == player_entity {
            second
        } else {
            first
        };
        if broken_this_frame.contains(&other) {
            continue;
        }
        if let Ok((crate_entity, transform, obj)) = crate_query.get(other) {
            if !matches!(obj, WorldObject::Crate | WorldObject::Crate2) {
                continue;
            }

            let pos = world_pos_to_tile_pos(transform.translation().truncate());
            to_break.push((crate_entity, *obj, pos));
            broken_this_frame.insert(crate_entity);
        }
    }

    for (entity, obj, tile_pos) in to_break {
        obj_break_events.write(ObjBreakEvent {
            entity,
            obj,
            pos: tile_pos,
            give_drops_and_xp: true,
        });
    }
}

// ============================================================================
// MaxHPHunt - Every 3 kills grants +1 max hp per stack
// ============================================================================

/// Tracks kills for the MaxHPHunt heirloom. Present on the player only when
/// the heirloom is equipped — stored `SparseSet` so granting/clearing the
/// heirloom doesn't move the player between archetypes.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct MaxHPHuntTracker {
    pub kill_count: u32,
    pub total_hp_gained: i32, // Total max HP gained from this heirloom
}

/// Tracks bonus skill power for the SkillPowerHunt heirloom (3% on skill use to
/// gain +1). Stored `SparseSet` for the same reasons as the other per-heirloom
/// player state components.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct SkillPowerHuntTracker {
    pub bonus_skill_power: i32,
}

pub fn handle_max_hp_hunt(
    mut death_events: MessageReader<EnemyDeathEvent>,
    mut player_query: Query<(&mut MaxHPHuntTracker, &PlayerSkills), With<Player>>,
    mut attribute_events: MessageWriter<crate::attributes::AttributeChangeEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((mut tracker, skills)) = player_query.single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::MaxHPHunt);
    if stacks <= 0 {
        return;
    }

    let max_hp_cap = stacks * 250;

    let mut hp_was_gained = false;
    for _death_event in death_events.read() {
        if tracker.total_hp_gained >= max_hp_cap {
            continue;
        }
        tracker.kill_count += 1;

        if tracker.kill_count >= 25 {
            tracker.kill_count -= 25;
            let hp_gained = stacks;
            tracker.total_hp_gained = (tracker.total_hp_gained + hp_gained).min(max_hp_cap);
            hp_was_gained = true;
            trigger_counts.increment(Heirloom::MaxHPHunt);
        }
    }

    // Trigger attribute recalculation if HP was gained
    // This ensures the MaxHPHunt bonus is included in the max health calculation
    if hp_was_gained {
        attribute_events.write_default();
    }
}

// ============================================================================
// SkillPowerHunt - 3% chance when using a skill to gain +1 Skill Power
// ============================================================================

pub fn handle_skill_power_hunt(
    mut skill_events: MessageReader<ActiveSkillUsedEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut SkillPowerHuntTracker>), With<Player>>,
    mut attribute_events: MessageWriter<crate::attributes::AttributeChangeEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, state_option)) = player_query.single_mut() else {
        return;
    };
    let count = skills.get_count(Heirloom::SkillPowerHunt);
    if count <= 0 {
        return;
    }

    let Some(mut tracker) = state_option else {
        return;
    };

    let sp_cap = count * 1000;

    let mut rng = rand::thread_rng();
    for _ in skill_events.read() {
        if tracker.bonus_skill_power >= sp_cap {
            continue;
        }
        if rng.gen_ratio((count * 7).min(100) as u32, 100) {
            tracker.bonus_skill_power = (tracker.bonus_skill_power + 1).min(sp_cap);
            attribute_events.write_default();
            trigger_counts.increment(Heirloom::SkillPowerHunt);
        }
    }
}

// ============================================================================
// StandStill - Standing still increases damage
// ============================================================================

/// Tracks standing still time for the StandStill heirloom. Stored `SparseSet`
/// because it's only present when the heirloom is equipped.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct StandStillState {
    pub time_still: f32,
    pub was_moving: bool,
}

impl Default for StandStillState {
    fn default() -> Self {
        Self {
            time_still: 0.0,
            was_moving: false,
        }
    }
}

/// Maximum time to reach full damage bonus
const STAND_STILL_MAX_TIME: f32 = 3.0;
/// Time before buff starts ramping
const STAND_STILL_START_TIME: f32 = 0.3;

pub fn tick_stand_still_state(
    time: Res<Time>,
    mut player_query: Query<(&mut StandStillState, &crate::inputs::MovementVector), With<Player>>,
) {
    let Ok((mut state, movement)) = player_query.single_mut() else {
        return;
    };

    let is_moving = movement.0.length_squared() > 0.01;

    if is_moving {
        state.time_still = 0.0;
        state.was_moving = true;
    } else {
        state.time_still = (state.time_still + time.delta_secs()).min(STAND_STILL_MAX_TIME);
        state.was_moving = false;
    }
}

impl StandStillState {
    /// Get the damage multiplier based on time standing still
    /// Returns 1.0 if not standing still long enough, ramps to 1.5 at max time
    /// Additional stacks increase the max multiplier by 0.5 each
    pub fn get_damage_multiplier(&self, stacks: i32) -> f32 {
        if self.time_still < STAND_STILL_START_TIME {
            return 1.0;
        }

        // Calculate progress from start time to max time
        let progress = ((self.time_still - STAND_STILL_START_TIME)
            / (STAND_STILL_MAX_TIME - STAND_STILL_START_TIME))
            .clamp(0.0, 1.0);

        // Each stack adds 0.5 to the max multiplier (1 stack = 1.5x, 2 stacks = 2.0x, etc.)
        let max_bonus = 0.5 * stacks as f32;

        1.0 + (progress * max_bonus)
    }
}

// ============================================================================
// DeathDefiance - Survive death, freeze all enemies
// ============================================================================

/// Fired when the Cooked Cross ([`Heirloom::DeathDefiance`]) heirloom saves the player from death.
#[derive(Message)]
pub struct DeathDefianceSurvivedEvent;

/// Fired when an orb upgrade ranks a piece of equipment to Legendary rarity.
#[derive(Message)]
pub struct LegendaryEquipmentRankedEvent;

/// Marker component for mobs frozen by Death Defiance. Inserted briefly on
/// every mob in range when the heirloom procs, removed when the freeze timer
/// elapses. Stored `SparseSet` so the freeze doesn't move a bunch of mobs to
/// a new archetype and back each proc.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct DeathDefianceFrozen {
    pub timer: Timer,
}

pub fn handle_death_defiance_freeze(
    mut commands: Commands,
    time: Res<Time>,
    mut frozen_mobs: Query<(Entity, &mut DeathDefianceFrozen)>,
) {
    for (entity, mut frozen) in frozen_mobs.iter_mut() {
        frozen.timer.tick(time.delta());

        if frozen.timer.just_finished() {
            commands
                .entity(entity)
                .remove::<DeathDefianceFrozen>()
                .remove::<DeathDefianceTint>();
        }
    }
}

// ============================================================================
// CrateBreakDamage - Breaking crates gives permanent damage bonus
// ============================================================================

/// Tracks total damage bonus from breaking crates. Only present on the player
/// while the CrateBreakDamage heirloom is equipped — stored `SparseSet`.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct CrateBreakDamageTracker {
    pub bonus_damage_percent: f32,
}

/// Tracks thorns gained from taking damage (ThornsOnDamage heirloom). Stored
/// `SparseSet` for the same reason as the other heirloom trackers.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ThornsOnDamageTracker {
    pub thorns_gained: i32,
}

/// Max thorns Spiked Helmet may grant per equipped stack (mirrors MaxHPHunt's
/// `stacks * 250` cap pattern).
pub const THORNS_ON_DAMAGE_CAP_PER_STACK: i32 = 300;

impl ThornsOnDamageTracker {
    /// Gain thorns from taking damage. Returns `true` when any thorns were added.
    pub fn gain_from_damage(&mut self, stacks: i32) -> bool {
        if stacks <= 0 {
            return false;
        }
        let cap = stacks * THORNS_ON_DAMAGE_CAP_PER_STACK;
        if self.thorns_gained >= cap {
            return false;
        }
        self.thorns_gained = (self.thorns_gained + stacks).min(cap);
        true
    }
}

pub fn handle_crate_break_damage(
    mut obj_break_events: MessageReader<ObjBreakEvent>,
    mut player_query: Query<(&mut CrateBreakDamageTracker, &PlayerSkills), With<Player>>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((mut tracker, skills)) = player_query.single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::CrateBreakDamage);
    if stacks <= 0 {
        return;
    }

    for event in obj_break_events.read() {
        // Check if this was a crate
        if matches!(
            event.obj,
            WorldObject::Crate
                | WorldObject::Crate2
                | WorldObject::DesertCrate
                | WorldObject::DesertCrate2
                | WorldObject::SnowCrate1
                | WorldObject::SnowCrate2
                | WorldObject::SnowCrate3
                | WorldObject::SnowCrate4
        ) {
            // Each crate gives 1% damage per stack
            tracker.bonus_damage_percent += 1.5 * stacks as f32;
            trigger_counts.increment(Heirloom::CrateBreakDamage);
        }
    }
}

// ============================================================================
// DodgeCrit - Dodging gives speed/attack speed buff and next hit does 2x damage
// ============================================================================

/// Telescope (DodgeCrit) attack-speed burst while the dodge buff is active.
/// Applied through [`BonusAttackSpeed`] — the same mechanism used by Rapidfire
/// and attack-speed potions — where a `+1.0` multiplier means +100% == 2x
/// attack speed.
const DODGE_CRIT_ATTACK_SPEED_BONUS: f32 = 1.0;
const DODGE_CRIT_BUFF_SECS: f32 = 3.0;

/// State for the DodgeCrit heirloom buff. Stored `SparseSet` because it's
/// only present on the player while the heirloom is equipped.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct DodgeCritState {
    pub buff_active: bool,
    pub buff_timer: Timer,
    pub next_hit_bonus: bool,
}

impl Default for DodgeCritState {
    fn default() -> Self {
        Self {
            buff_active: false,
            buff_timer: Timer::from_seconds(DODGE_CRIT_BUFF_SECS, TimerMode::Once),
            next_hit_bonus: false,
        }
    }
}

pub fn handle_dodge_crit_activation(
    mut dodge_events: MessageReader<crate::ui::damage_numbers::DodgeEvent>,
    mut player_query: Query<
        (&mut DodgeCritState, &PlayerSkills, &mut BonusAttackSpeed),
        With<Player>,
    >,
    mut attribute_event: MessageWriter<crate::attributes::AttributeChangeEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((mut state, skills, mut bonus_attack_speed)) = player_query.single_mut() else {
        return;
    };

    if !skills.has(Heirloom::DodgeCrit) {
        return;
    }

    for _ in dodge_events.read() {
        // Refreshing an already-active buff only resets the timer; the attack-speed
        // multiplier is added once so repeated dodges don't stack past 2x.
        if !state.buff_active {
            bonus_attack_speed.add_multiplier(DODGE_CRIT_ATTACK_SPEED_BONUS);
        }
        state.buff_active = true;
        state.buff_timer.reset();
        state.next_hit_bonus = true;
        attribute_event.write(crate::attributes::AttributeChangeEvent);
        trigger_counts.increment(Heirloom::DodgeCrit);
    }
}

pub fn tick_dodge_crit_buff(
    time: Res<Time>,
    mut player_query: Query<(&mut DodgeCritState, &mut BonusAttackSpeed), With<Player>>,
    mut attribute_event: MessageWriter<crate::attributes::AttributeChangeEvent>,
) {
    let Ok((mut state, mut bonus_attack_speed)) = player_query.single_mut() else {
        return;
    };

    if state.buff_active {
        state.buff_timer.tick(time.delta());
        if state.buff_timer.just_finished() {
            state.buff_active = false;
            state.next_hit_bonus = false; // Also clear the unused next hit bonus
            bonus_attack_speed.remove_multiplier(DODGE_CRIT_ATTACK_SPEED_BONUS);
            attribute_event.write(crate::attributes::AttributeChangeEvent);
        }
    }
}

/// Consume the DodgeCrit "next hit does 2x damage" bonus once the player lands a
/// source of *weapon* damage (a melee weapon swing or a weapon projectile). Skill
/// and heirloom hits are ignored so the bonus is reserved for the next weapon hit,
/// matching where the 2x is applied in [`GameParam::calculate_player_damage`].
pub fn handle_dodge_crit_next_hit_reset(
    mut hit_events: MessageReader<crate::combat::HitEvent>,
    mut player_query: Query<&mut DodgeCritState, With<Player>>,
) {
    for hit in hit_events.read() {
        if hit.hit_by_mob.is_some() || !hit_is_weapon_damage(hit) {
            continue;
        }

        if let Ok(mut state) = player_query.single_mut() {
            if state.next_hit_bonus {
                state.next_hit_bonus = false;
            }
        }
        break; // Only need to consume once per frame
    }
}

/// A hit counts as weapon damage when it comes from a melee weapon swing or from a
/// weapon-fired projectile (i.e. not an active skill and not a heirloom proc).
pub fn hit_is_weapon_damage(hit: &crate::combat::HitEvent) -> bool {
    if hit.hit_with_melee.is_some() {
        return true;
    }
    let Some(proj) = hit.hit_with_projectile.as_ref() else {
        return false;
    };
    proj.animation_category() == AnimVisualCategory::Attack
        && !hit.from_active_skill
        && hit.from_heirloom_effect.is_none()
}

// ============================================================================
// LethalBlow Hallucination Stats - Tracks stat bonuses from execute procs
// ============================================================================

/// Tracks accumulated stat bonuses from LethalBlow hallucinations.
/// Wraps ItemAttributes so it can be combined with player attributes using
/// combine(). Stored `SparseSet` because this is only present while the
/// LethalBlow heirloom is equipped.
#[derive(Component, Default, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct HallucinationStats(pub crate::attributes::ItemAttributes);

/// List of stats that can be buffed by hallucinations
#[derive(Clone, Copy, Debug)]
pub enum HallucinationStatType {
    Attack,
    Health,
    Defence,
    CritChance,
    CritDamage,
    Speed,
    Lifesteal,
    Dodge,
    HealthRegen,
    Healing,
    Thorns,
    XPRate,
    Luck,
    Mana,
    Size,
    ManaRegen,
    AttackSpeed,
}

impl HallucinationStatType {
    pub fn random() -> Self {
        let mut rng = rand::thread_rng();
        match rng.gen_range(0..17) {
            0 => Self::Attack,
            1 => Self::Health,
            2 => Self::Defence,
            3 => Self::CritChance,
            4 => Self::CritDamage,
            5 => Self::Speed,
            6 => Self::Lifesteal,
            7 => Self::HealthRegen,
            8 => Self::Healing,
            9 => Self::Thorns,
            10 => Self::XPRate,
            11 => Self::Luck,
            12 => Self::Mana,
            13 => Self::Size,
            14 => Self::ManaRegen,
            15 => Self::AttackSpeed,
            16 => Self::Dodge,
            _ => Self::AttackSpeed,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Attack => "Attack",
            Self::Health => "Health",
            Self::Defence => "Defence",
            Self::CritChance => "Crit",
            Self::CritDamage => "Crit DMG",
            Self::Speed => "Speed",
            Self::Lifesteal => "Lifesteal",
            Self::Dodge => "Dodge",
            Self::HealthRegen => "Health Regen",
            Self::Healing => "Healing",
            Self::Thorns => "Thorns",
            Self::XPRate => "XP",
            Self::Luck => "Luck",
            Self::Mana => "Mana",
            Self::Size => "Size",
            Self::ManaRegen => "Mana Regen",
            Self::AttackSpeed => "Attack Speed",
        }
    }

    pub fn color(&self) -> bevy::prelude::Color {
        match self {
            Self::Attack => crate::colors::LIGHT_RED,
            Self::Health => crate::colors::LIGHT_RED,
            Self::Defence => crate::colors::GREY,
            Self::CritChance => crate::colors::YELLOW,
            Self::CritDamage => crate::colors::YELLOW,
            Self::Speed => crate::colors::LIGHT_GREEN,
            Self::Lifesteal => crate::colors::LIGHT_RED,
            Self::Dodge => crate::colors::YELLOW,
            Self::HealthRegen => crate::colors::LIGHT_RED,
            Self::Healing => crate::colors::LIGHT_GREEN,
            Self::Thorns => crate::colors::LIGHT_GREEN,
            Self::XPRate => crate::colors::YELLOW,
            Self::Luck => crate::colors::YELLOW,
            Self::Mana => crate::colors::LIGHT_BLUE,
            Self::Size => crate::colors::LIGHT_BLUE,
            Self::ManaRegen => crate::colors::LIGHT_BLUE,
            Self::AttackSpeed => crate::colors::LIGHT_GREEN,
        }
    }
}

impl HallucinationStats {
    pub fn add_stat(&mut self, stat_type: HallucinationStatType, amount: i32) {
        match stat_type {
            HallucinationStatType::Attack => self.0.attack.value += amount,
            HallucinationStatType::Health => self.0.health.value += amount,
            HallucinationStatType::Defence => self.0.defence.value += amount,
            HallucinationStatType::CritChance => self.0.crit_chance.value += amount,
            HallucinationStatType::CritDamage => self.0.crit_damage.value += amount,
            HallucinationStatType::Speed => self.0.speed.value += amount,
            HallucinationStatType::Lifesteal => self.0.lifesteal.value += amount,
            HallucinationStatType::Dodge => self.0.dodge.value += amount,
            HallucinationStatType::HealthRegen => self.0.health_regen.value += amount,
            HallucinationStatType::Healing => self.0.healing.value += amount,
            HallucinationStatType::Thorns => self.0.thorns.value += amount,
            HallucinationStatType::XPRate => self.0.xp_rate.value += amount,
            HallucinationStatType::Luck => self.0.loot_rate.value += amount,
            HallucinationStatType::Mana => self.0.mana.value += amount,
            HallucinationStatType::Size => self.0.size.value += amount,
            HallucinationStatType::ManaRegen => self.0.mana_regen.value += amount,
            HallucinationStatType::AttackSpeed => self.0.attack_speed.value += amount,
        }
    }

    /// Get the inner ItemAttributes for combining with player stats
    pub fn as_item_attributes(&self) -> &crate::attributes::ItemAttributes {
        &self.0
    }
}

// ============================================================================
// ManaChargeDamage (MPBarDMG) - Mana regen charges up bonus damage
// ============================================================================

/// Tracks accumulated mana regen for the MPBarDMG heirloom.
/// When mana is regenerated, the amount is stored here.
/// The next weapon attack consumes the stored mana as bonus flat damage.
/// Stored `SparseSet` because it's only on the player while the heirloom is
/// equipped.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ManaChargeDamageState {
    pub stored_mana_damage: f32,
}

impl ManaChargeDamageState {
    /// Get the bonus damage (non-mutating).
    /// Additional stacks increase the damage by 50% per stack.
    pub fn get_damage(&self, stacks: i32) -> i32 {
        if self.stored_mana_damage <= 0.0 {
            return 0;
        }

        // Base damage = stored mana, each additional stack adds 50% more
        // 1 stack = 1.0x, 2 stacks = 1.5x, 3 stacks = 2.0x, etc.
        let multiplier = 1.0 + (stacks - 1).max(0) as f32 * 0.5;
        (self.stored_mana_damage * multiplier).floor() as i32
    }

    /// Reset the stored mana after it was used in an attack
    pub fn reset(&mut self) {
        self.stored_mana_damage = 0.0;
    }

    /// Add mana regen to the stored damage
    pub fn add_mana(&mut self, amount: i32) {
        if amount > 0 {
            self.stored_mana_damage += amount as f32;
        }
    }

    /// Check if there's any stored mana to use
    pub fn has_stored_mana(&self) -> bool {
        self.stored_mana_damage > 0.0
    }
}

/// System to track mana regen and store it for the MPBarDMG heirloom
pub fn handle_mana_charge_damage(
    mut mana_events: MessageReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut ManaChargeDamageState>), With<Player>>,
) {
    let Ok((skills, state_option)) = player_query.single_mut() else {
        return;
    };

    // Only process if player has the heirloom
    if skills.get_count(Heirloom::MPBarDMG) <= 0 {
        return;
    }

    let Some(mut state) = state_option else {
        return;
    };

    for event in mana_events.read() {
        // Only track positive mana changes (regen, not consumption)
        if event.0 > 0 {
            state.add_mana(event.0);
        }
    }
}

/// System to reset stored mana after an attack is made.
/// Runs after HitEvents are processed.
pub fn handle_mana_charge_damage_reset(
    mut hit_events: MessageReader<HitEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut ManaChargeDamageState>), With<Player>>,
) {
    let Ok((skills, state_option)) = player_query.single_mut() else {
        return;
    };

    if skills.get_count(Heirloom::MPBarDMG) <= 0 {
        return;
    }
    // Only reset if there was a hit from the player (not from mobs, not from heirloom effects)
    let mut player_dealt_damage = false;
    for event in hit_events.read() {
        // Player weapon hits have hit_with_melee or hit_with_projectile set
        // Exclude heirloom effect damage (like echoes) to only consume on weapon attacks
        if event.from_heirloom_effect.is_none()
            && (event.hit_with_melee.is_some() || event.hit_with_projectile.is_some())
            && event.hit_by_mob.is_none()
        {
            player_dealt_damage = true;
            break;
        }
    }

    if !player_dealt_damage {
        return;
    }

    if let Some(mut state) = state_option {
        if state.has_stored_mana() {
            state.reset();
        }
    }
}

// ============================================================================
// ManaOrbAttack - Mana regen shoots mana orb projectiles at enemies
// ============================================================================

/// System to spawn mana orb projectiles when mana is regenerated
pub fn handle_mana_orb_attack(
    mut mana_events: MessageReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, &GlobalTransform, &mut CurrentMana), With<Player>>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth), With<Mob>>,
    mut ranged_attack_event: MessageWriter<RangedAttackEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, player_transform, mut current_mana)) = player_query.single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::ManaOrbAttack);
    if stacks <= 0 {
        return;
    }

    let mana_cost_per_orb = (Heirloom::ManaOrbAttack.get_mana_cost() as f32
        * if skills.has(Heirloom::DiscountMP) {
            0.75
        } else {
            1.
        }) as i32;
    if mana_cost_per_orb <= 0 {
        return;
    }

    let player_pos = player_transform.translation().truncate();
    let mut rng = rand::thread_rng();
    let mut remaining_mana = current_mana.0;

    for event in mana_events.read() {
        // Only trigger on positive mana changes (regen, not consumption)
        if event.0 <= 0 {
            continue;
        }

        // Find nearby enemies
        let nearby_mobs: Vec<_> = mobs
            .iter()
            .filter(|(_, mob_transform, health)| {
                health.0 > 0
                    && (mob_transform.translation().truncate() - player_pos).length() <= 400.0
            })
            .collect();

        if nearby_mobs.is_empty() {
            continue;
        }

        let orbs_to_fire =
            (stacks as usize).min(remaining_mana as usize / mana_cost_per_orb as usize);
        if orbs_to_fire == 0 {
            continue;
        }

        let total_mana_cost = mana_cost_per_orb * orbs_to_fire as i32;
        current_mana.0 -= total_mana_cost;
        remaining_mana = current_mana.0;
        trigger_counts.record_mana(Heirloom::ManaOrbAttack, total_mana_cost);
        trigger_counts.increment(Heirloom::ManaOrbAttack);

        for i in 0..orbs_to_fire {
            if let Some((_, target_transform, _)) = nearby_mobs.choose(&mut rng) {
                let target_pos = target_transform.translation().truncate();
                let direction = (target_pos - player_pos).normalize_or_zero();
                ranged_attack_event.write(RangedAttackEvent {
                    projectile: crate::item::projectile::Projectile::ManaOrbProjectile,
                    direction,
                    mana_cost: None,
                    mana_cost_heirloom: None,
                    from_enemy: false,
                    from_entity: None,
                    is_followup_proj: true,
                    dmg_override: Some(event.0),
                    pos_override: Some(player_pos),
                    spawn_delay: i as f32 * 0.1,
                });
            }
        }
    }
}

// ============================================================================
// ManaRegenPoison - Every 100 mana regen applies poison to all enemies
// ============================================================================

/// Tracks accumulated mana regen for the ManaRegenPoison heirloom.
/// When mana is regenerated, the amount is accumulated here.
/// When it reaches 100, poison is applied to all enemies and the tracker resets with the remainder.
/// Stored `SparseSet` because it's only on the player while the heirloom is
/// equipped.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct ManaRegenPoisonTracker {
    pub accumulated_mana: f32,
}

impl ManaRegenPoisonTracker {
    /// Add mana regen to the tracker and return how many times poison should be applied
    pub fn add_mana(&mut self, amount: i32) -> u32 {
        if amount <= 0 {
            return 0;
        }
        self.accumulated_mana += amount as f32;

        // Calculate how many times we've reached 100
        let poison_count = (self.accumulated_mana / 75.0).floor() as u32;

        // Keep the remainder
        self.accumulated_mana = self.accumulated_mana % 75.0;

        poison_count
    }
}

/// System to track mana regen and apply poison to all enemies when 100 is reached
pub fn handle_mana_regen_poison(
    mut mana_events: MessageReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut ManaRegenPoisonTracker>), With<Player>>,
    enemies: Query<Entity, (With<Mob>, Without<Player>)>,
    mut mob_status: Query<&mut MobStatusEffects, With<Mob>>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut status_event: MessageWriter<StatusEffectEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, state_option)) = player_query.single_mut() else {
        return;
    };

    // Only process if player has the heirloom
    let heirloom_count = skills.get_count(Heirloom::ManaRegenPoison);
    if heirloom_count <= 0 {
        return;
    }

    let poison_duration_bonus = player_skills
        .single()
        .map(|s| s.get_count(Heirloom::PoisonDuration) as f32 * 0.5 + 1.)
        .unwrap_or(1.0);

    let mut tracker = if let Some(state) = state_option {
        state
    } else {
        return;
    };

    for event in mana_events.read() {
        if event.0 > 0 {
            let poison_count = tracker.add_mana(event.0);

            for _ in 0..poison_count {
                trigger_counts.increment(Heirloom::ManaRegenPoison);
                for enemy_entity in enemies.iter() {
                    let Ok(mut status) = mob_status.get_mut(enemy_entity) else {
                        continue;
                    };
                    if let Some(burning) = status.burning.as_mut() {
                        burning.stacks = burning.stacks.saturating_add(heirloom_count as u128);
                        burning.duration_timer.reset();
                        let stacks = burning.stacks as i32;
                        status_event.write(StatusEffectEvent {
                            entity: enemy_entity,
                            effect: StatusEffect::Poison,
                            num_stacks: stacks,
                        });
                    } else {
                        status.burning = Some(Burning {
                            tick_timer: Timer::from_seconds(0.5, TimerMode::Repeating),
                            duration_timer: Timer::from_seconds(
                                3.0 * poison_duration_bonus,
                                TimerMode::Once,
                            ),
                            stacks: 1,
                        });
                        status_event.write(StatusEffectEvent {
                            entity: enemy_entity,
                            effect: StatusEffect::Poison,
                            num_stacks: 1,
                        });
                    }
                }
            }
        }
    }
}

// SkillManaRegen (Brown Card) - Using a skill has a 15% chance per stack to trigger mana regen
// ============================================================================

const SKILL_MANA_REGEN_PROC_PCT_PER_STACK: u32 = 15;

/// System to trigger mana regen when a skill is used
pub fn handle_skill_mana_regen(
    mut skill_events: MessageReader<ActiveSkillUsedEvent>,
    mut player_query: Query<(&PlayerSkills, &ManaRegen), With<Player>>,
    mut modify_mana_event: MessageWriter<ModifyManaEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, mana_regen)) = player_query.single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::SkillManaRegen);
    if stacks <= 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    let chance_pct = stacks as u32 * SKILL_MANA_REGEN_PROC_PCT_PER_STACK;

    for _event in skill_events.read() {
        let proc_count = roll_stacked_proc_count(chance_pct, &mut rng);
        if proc_count == 0 {
            continue;
        }

        for _ in 0..proc_count {
            modify_mana_event.write(ModifyManaEvent::gain(
                mana_regen.0,
                ManaGainSource::Heirloom(Heirloom::SkillManaRegen),
            ));
            trigger_counts.increment(Heirloom::SkillManaRegen);
        }
    }
}

// ManaRegenLightning - Mana regen has a 10% chance per stack to trigger lightning
// ============================================================================

/// System to spawn lightning strikes when mana is regenerated
pub fn handle_mana_regen_lightning(
    mut mana_events: MessageReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, &GlobalTransform, &Attack, &CurrentMana), With<Player>>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth), With<Mob>>,
    mut ranged_attack_event: MessageWriter<RangedAttackEvent>,
    mut commands: Commands,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, player_transform, attack, current_mana)) = player_query.single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::ManaRegenLightning);
    if stacks <= 0 {
        return;
    }

    let player_pos = player_transform.translation().truncate();
    let mut rng = rand::thread_rng();

    for event in mana_events.read() {
        // Only trigger on positive mana changes (regen, not consumption)
        if event.0 <= 0 {
            continue;
        }

        // 10% chance per stack
        let chance_per_stack = 20;
        let total_chance = (stacks * chance_per_stack).min(100);
        if !rng.gen_ratio(total_chance as u32, 100) {
            continue;
        }

        // Find nearby enemies (within 400 units)
        let nearby_mobs: Vec<_> = mobs
            .iter()
            .filter(|(_, mob_transform, health)| {
                health.0 > 0
                    && (mob_transform.translation().truncate() - player_pos).length() <= 200.0
            })
            .collect();

        if nearby_mobs.is_empty() {
            continue;
        }

        // Pick a random nearby enemy
        if let Some((_, target_transform, _)) = nearby_mobs.choose(&mut rng) {
            let target_pos = target_transform.translation().truncate();
            const MANA_COST: i32 = 5;

            // Check if player has enough mana
            if current_mana.0 >= MANA_COST {
                let lightning_damage = attack.0; // 100% damage
                ranged_attack_event.write(RangedAttackEvent {
                    projectile: Projectile::Lightning,
                    direction: Vec2::ZERO,
                    mana_cost: Some(MANA_COST),
                    mana_cost_heirloom: Some(Heirloom::ManaRegenLightning),
                    from_enemy: false,
                    from_entity: None,
                    is_followup_proj: false,
                    dmg_override: Some(lightning_damage),
                    pos_override: Some(target_pos + Vec2::new(0., 48.)),
                    spawn_delay: 0.0,
                });
                commands.spawn(SoundSpawner::new(AudioSoundEffect::LightningStaffCast, 0.2));
                trigger_counts.increment(Heirloom::ManaRegenLightning);
            }
        }
    }
}

// ============================================================================
// EnergyBallBarrage (Voltaic Core) - homing energy balls every 150 damage dealt
// ============================================================================

pub const ENERGY_BALL_DAMAGE_THRESHOLD: i32 = 150;
/// Maximum homing fire balls Underworld's Hat may spawn per second (end-game damage
/// can otherwise enqueue unbounded projectiles in a single frame).
pub const ENERGY_BALL_MAX_SUMMONS_PER_SEC: u32 = 100;
pub const ENERGY_BALL_INITIAL_SPEED: f32 = 90.0;
pub const ENERGY_BALL_LOCK_DELAY: f32 = 0.55;

const ENERGY_BALL_TARGET_RANGE: f32 = 420.0;
const ENERGY_BALL_MAX_SPEED: f32 = 520.0;
const ENERGY_BALL_ACCEL: f32 = 380.0;
const ENERGY_BALL_SWERVE_STRENGTH: f32 = 0.9;
const ENERGY_BALL_SWERVE_FREQ: f32 = 9.5;
const ENERGY_BALL_ARC_BLEND: f32 = 1.35;
const ENERGY_BALL_MUZZLE_DURATION: f32 = 0.35;
/// Distance from the player to spawn the muzzle flash, in the direction of fire.
const ENERGY_BALL_MUZZLE_OFFSET: f32 = 18.0;

/// Accumulates non-energy-ball player damage toward the next homing shot.
#[derive(Component, Default)]
#[component(storage = "SparseSet")]
pub struct EnergyBallBarrageTracker {
    pub accumulated: i32,
    spawns_this_sec: u32,
    rate_window_elapsed: f32,
}

fn is_energy_ball_damage_hit(hit: &HitEvent) -> bool {
    hit.hit_with_projectile.as_ref() == Some(&Projectile::EnergyBall)
        || hit.from_heirloom_effect == Some(Heirloom::EnergyBallBarrage)
}

/// Spawns the muzzle flash at the player's position. Purely visual — no
/// collider, no damage — so we spawn it as a bare `aseprite_bundle` +
/// `DespawnTimer`, the same pattern as other one-shot visual effects.
fn spawn_energy_ball_muzzle(
    commands: &mut Commands,
    asset_server: &AssetServer,
    player_pos: Vec3,
    direction: Vec2,
) {
    // Nudge the muzzle along the projectile's heading so it doesn't overlap
    // the player sprite, then rotate it to face that direction.
    let offset = direction.normalize_or_zero() * ENERGY_BALL_MUZZLE_OFFSET;
    let angle = direction.y.atan2(direction.x);
    commands.spawn((
        aseprite_bundle(
            asset_server.load(EnergyBallEffect::PATH),
            EnergyBallEffect::tags::MUZZLE,
            Transform::from_translation(player_pos + offset.extend(0.))
                .with_rotation(Quat::from_rotation_z(angle)),
            Visibility::default(),
            true,
        ),
        AnimVisualCategory::Heirloom,
        DespawnTimer(Timer::from_seconds(
            ENERGY_BALL_MUZZLE_DURATION,
            TimerMode::Once,
        )),
        DoneAnimation,
        Name::new("EnergyBallMuzzle"),
    ));
}

/// Every 150 damage dealt (any source except energy balls) fires a homing
/// energy ball.  The ball itself is spawned through the standard
/// `RangedAttackEvent` → proto pipeline so it gets proper physics, collision,
/// and the aseprite swap that happens in `spawn_projectile_from_proto`.
pub fn handle_energy_ball_barrage(
    time: Res<Time>,
    mut hit_events: MessageReader<HitEvent>,
    in_i_frame: Query<&crate::combat::InvincibilityTimer>,
    mut player_query: Query<
        (
            &PlayerSkills,
            &GlobalTransform,
            &Attack,
            &mut EnergyBallBarrageTracker,
        ),
        With<Player>,
    >,
    mobs: Query<Entity, With<Mob>>,
    mut ranged_attack_events: MessageWriter<RangedAttackEvent>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, player_transform, attack, mut tracker)) = player_query.single_mut() else {
        return;
    };
    if skills.get_count(Heirloom::EnergyBallBarrage) <= 0 {
        return;
    }

    tracker.rate_window_elapsed += time.delta_secs();
    if tracker.rate_window_elapsed >= 1.0 {
        tracker.rate_window_elapsed -= 1.0;
        tracker.spawns_this_sec = 0;
    }

    let player_pos = player_transform.translation();
    let stacks = skills.get_count(Heirloom::EnergyBallBarrage);
    let mut rng = rand::thread_rng();
    let mut fired = 0u32;

    for hit in hit_events.read() {
        if hit.hit_by_mob.is_some() {
            continue;
        }
        if is_energy_ball_damage_hit(hit) {
            continue;
        }
        if mobs.get(hit.hit_entity).is_err() {
            continue;
        }
        if in_i_frame.get(hit.hit_entity).is_ok() {
            continue;
        }

        let dmg = if hit.damage <= 0 { 1 } else { hit.damage };
        tracker.accumulated += dmg;

        while tracker.accumulated >= ENERGY_BALL_DAMAGE_THRESHOLD {
            if tracker.spawns_this_sec >= ENERGY_BALL_MAX_SUMMONS_PER_SEC {
                tracker.accumulated = 0;
                break;
            }
            tracker.accumulated -= ENERGY_BALL_DAMAGE_THRESHOLD;

            let mut spawned_any = false;
            let mut hit_cap = false;
            for _ in 0..stacks {
                if tracker.spawns_this_sec >= ENERGY_BALL_MAX_SUMMONS_PER_SEC {
                    hit_cap = true;
                    break;
                }
                spawned_any = true;
                tracker.spawns_this_sec += 1;

                // Random initial heading — the homing component steers it toward
                // the nearest enemy once the lock-on delay elapses.
                let initial_dir = Vec2::from_angle(rng.gen_range(0.0..TAU));

                // Muzzle flash: purely visual, spawned directly (no physics needed).
                spawn_energy_ball_muzzle(
                    &mut commands,
                    &asset_server,
                    player_pos + Vec3::new(0., 0., 0.15),
                    initial_dir,
                );

                // Actual projectile through the standard pipeline.
                ranged_attack_events.write(RangedAttackEvent {
                    projectile: Projectile::EnergyBall,
                    direction: initial_dir,
                    mana_cost: None,
                    mana_cost_heirloom: None,
                    from_enemy: false,
                    from_entity: None,
                    is_followup_proj: true,
                    dmg_override: Some(attack.0),
                    pos_override: Some(player_pos.truncate()),
                    spawn_delay: 0.0,
                });

                fired += 1;
            }

            if hit_cap {
                tracker.accumulated = 0;
                break;
            }

            if spawned_any {
                commands.spawn(SoundSpawner::new(
                    AudioSoundEffect::LightningStaffCast,
                    0.15,
                ));
            }
        }
    }

    if fired > 0 {
        for _ in 0..fired {
            trigger_counts.increment(Heirloom::EnergyBallBarrage);
        }
    }
}

/// Steers every `HomingEnergyBall` entity each frame: arcs outward, then
/// curves toward the nearest enemy with a swerving path.  Translation is
/// updated directly here; `handle_translate_projectiles` is excluded via
/// `Without<HomingEnergyBall>` so the two systems don't fight.
pub fn update_homing_energy_balls(
    time: Res<Time>,
    mut balls: Query<(&mut Transform, &mut HomingEnergyBall, &mut ProjectileState)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth), With<Mob>>,
) {
    for (mut transform, mut ball, mut proj_state) in balls.iter_mut() {
        let dt = time.delta_secs();

        ball.lock_elapsed = (ball.lock_elapsed + dt).min(ball.lock_duration);
        let lock_t = (ball.lock_elapsed / ball.lock_duration.max(0.001)).clamp(0., 1.);

        let pos = transform.translation.truncate();

        // Re-acquire target if we don't have one (or it died).
        let target_valid = ball.target.map_or(false, |e| {
            mobs.get(e).map(|(_, _, h)| h.0 > 0).unwrap_or(false)
        });
        if !target_valid {
            ball.target = None;
            let mut best_dist_sq = ENERGY_BALL_TARGET_RANGE * ENERGY_BALL_TARGET_RANGE;
            for (mob_e, mob_txfm, health) in mobs.iter() {
                if health.0 <= 0 {
                    continue;
                }
                let d = mob_txfm.translation().truncate().distance_squared(pos);
                if d < best_dist_sq {
                    best_dist_sq = d;
                    ball.target = Some(mob_e);
                }
            }
        }

        // Desired direction: straight ahead until lock-on delay passes, then home.
        let desired_dir = if let Some(target_e) = ball.target {
            if let Ok((_, target_txfm, _)) = mobs.get(target_e) {
                let to_target = target_txfm.translation().truncate() - pos;
                if to_target.length_squared() > 1.0 {
                    to_target.normalize()
                } else {
                    ball.initial_direction
                }
            } else {
                ball.initial_direction
            }
        } else {
            ball.initial_direction
        };

        // Blend from initial direction → homing direction as lock-on completes.
        let blended = ball
            .initial_direction
            .lerp(
                desired_dir,
                (lock_t * lock_t * ENERGY_BALL_ARC_BLEND).min(1.0),
            )
            .normalize_or_zero();

        // Add perpendicular swerve for the classic missile feel.
        ball.swerve_phase += ENERGY_BALL_SWERVE_FREQ * dt;
        let perp = Vec2::new(-blended.y, blended.x);
        // Swerve is stronger before lock-on (arcing phase), gentler after.
        let swerve_mag = ENERGY_BALL_SWERVE_STRENGTH * (1.0 + (1.0 - lock_t) * 0.6);
        let steering = (blended + perp * ball.swerve_phase.sin() * swerve_mag).normalize_or_zero();

        // Accelerate up to max speed.
        ball.speed = (ball.speed + ENERGY_BALL_ACCEL * dt).min(ENERGY_BALL_MAX_SPEED);

        let velocity = steering * ball.speed;
        transform.translation += velocity.extend(0.0) * dt;

        // Rotate sprite to face the direction of travel.
        if velocity.length_squared() > 0.01 {
            transform.rotation = Quat::from_rotation_z(velocity.y.atan2(velocity.x));
        }

        // Keep ProjectileState direction in sync so collision knockback is
        // calculated in the right direction.
        proj_state.direction = velocity.normalize_or_zero();
    }
}

// ============================================================================
// LobArc - shared arcing projectile flight used by Cherry Bomb heirloom and Bomb skill
// ============================================================================

const CHERRY_BOMB_PROC_PCT_PER_STACK: u32 = 25;
const CHERRY_BOMB_MIN_TILES: f32 = 3.0;
const CHERRY_BOMB_MAX_TILES: f32 = 10.0;
const LOB_ARC_FLIGHT_SECS: f32 = 0.72;
const LOB_ARC_HEIGHT: f32 = 28.0;
const CHERRY_BOMB_EXPLOSION_RADIUS: f32 = 20.0;
const CHERRY_BOMB_EXPLOSION_ANIM_SECS: f32 = 0.45;

#[derive(Clone, Copy)]
pub enum LobArcLanding {
    CherryBomb { explosion_damage: i32 },
    SkillBomb { explosion_damage: i32 },
}

#[derive(Component)]
pub struct LobArc {
    pub start_pos: Vec2,
    pub target_pos: Vec2,
    pub timer: Timer,
    pub arc_height: f32,
    pub landing: LobArcLanding,
}

/// Rolls how many times a stacked percentage proc fires.
/// e.g. 250% => 2 guaranteed + 50% chance for a 3rd.
pub fn roll_stacked_proc_count(chance_pct: u32, rng: &mut impl Rng) -> u32 {
    if chance_pct == 0 {
        return 0;
    }
    let guaranteed = chance_pct / 100;
    let remainder = chance_pct % 100;
    let extra = if remainder > 0 && rng.gen_ratio(remainder, 100) {
        1
    } else {
        0
    };
    guaranteed + extra
}

fn random_cherry_bomb_target(player_pos: Vec2, rng: &mut impl Rng) -> Vec2 {
    let min_dist = CHERRY_BOMB_MIN_TILES * TILE_SIZE.x;
    let max_dist = CHERRY_BOMB_MAX_TILES * TILE_SIZE.x;
    let angle = rng.gen_range(0.0..TAU);
    let dist = rng.gen_range(min_dist..max_dist);
    player_pos + Vec2::new(angle.cos(), angle.sin()) * dist
}

pub fn spawn_cherry_bomb_flight(
    commands: &mut Commands,
    graphics: &Graphics,
    start_pos: Vec2,
    target_pos: Vec2,
    explosion_damage: i32,
) {
    let Some(cherry_bomb_ase) = graphics.cherry_bomb_ase.as_ref() else {
        return;
    };

    commands.spawn((
        aseprite_bundle(
            cherry_bomb_ase.clone(),
            CherryBombSprite::tags::BOMB,
            Transform::from_translation(start_pos.extend(11.)),
            Visibility::default(),
            false,
        ),
        LobArc {
            start_pos,
            target_pos,
            timer: Timer::from_seconds(LOB_ARC_FLIGHT_SECS, TimerMode::Once),
            arc_height: LOB_ARC_HEIGHT,
            landing: LobArcLanding::CherryBomb { explosion_damage },
        },
        AnimVisualCategory::Heirloom,
        YSort(11.),
        Name::new("CHERRY_BOMB"),
    ));
}

pub fn spawn_skill_bomb_lob(
    commands: &mut Commands,
    graphics: &Graphics,
    start_pos: Vec2,
    target_pos: Vec2,
    explosion_damage: i32,
) {
    let Some(bomb_ase) = graphics.bomb_ase.as_ref() else {
        return;
    };

    commands.spawn((
        aseprite_bundle(
            bomb_ase.clone(),
            BombSprite::tags::BOMB,
            Transform::from_translation(start_pos.extend(11.)),
            Visibility::default(),
            false,
        ),
        LobArc {
            start_pos,
            target_pos,
            timer: Timer::from_seconds(LOB_ARC_FLIGHT_SECS, TimerMode::Once),
            arc_height: LOB_ARC_HEIGHT,
            landing: LobArcLanding::SkillBomb { explosion_damage },
        },
        AnimVisualCategory::Skill,
        YSort(11.),
        Name::new("SKILL_BOMB"),
    ));
}

fn spawn_cherry_bomb_explosion(
    commands: &mut Commands,
    graphics: &Graphics,
    pos: Vec2,
    dmg: i32,
    size_multiplier: f32,
) {
    let Some(explosion_ase) = graphics.cherry_bomb_explosion_ase.as_ref() else {
        return;
    };

    spawn_deferred_aseprite_collider(
        commands,
        Transform::from_translation(pos.extend(11.)).with_scale(Vec3::splat(size_multiplier)),
        CHERRY_BOMB_EXPLOSION_ANIM_SECS,
        dmg,
        // NOTE: do NOT pre-scale the collider radius by `size_multiplier`. The entity's
        // Transform scale above is applied to the collider by Rapier, so multiplying the
        // radius here as well would scale the hitbox twice (it would grow ~size^2).
        Collider::capsule(
            Vec2::new(0.0, -2.0),
            Vec2::new(0.0, -4.0),
            CHERRY_BOMB_EXPLOSION_RADIUS,
        ),
        explosion_ase.clone(),
        CherryBombExplosionSprite::tags::EXPLOSION,
        false,
        Projectile::CherryBombExplosion,
        vec![],
        None,
    );
}

pub fn handle_cherry_bomb_on_attack(
    mut attacks: MessageReader<AttackEvent>,
    mut player: Query<(&PlayerSkills, &Attack, &GlobalTransform, &mut CurrentMana), With<Player>>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, attack, player_transform, mut current_mana)) = player.single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::CherryBomb);
    if stacks <= 0 {
        return;
    }

    let mana_cost_per_bomb = (Heirloom::CherryBomb.get_mana_cost() as f32
        * if skills.has(Heirloom::DiscountMP) {
            0.75
        } else {
            1.
        }) as i32;
    if mana_cost_per_bomb <= 0 {
        return;
    }

    let player_pos = player_transform.translation().truncate();
    let explosion_damage = (attack.0 as f32 * attack_damage_multiplier(100.)).round() as i32;

    for _ in attacks.read() {
        let mut rng = rand::thread_rng();
        let chance_pct = stacks as u32 * CHERRY_BOMB_PROC_PCT_PER_STACK;
        let proc_count = roll_stacked_proc_count(chance_pct, &mut rng);
        if proc_count == 0 {
            continue;
        }

        let mut bombs_spawned = 0u32;
        for _ in 0..proc_count {
            if current_mana.0 < mana_cost_per_bomb {
                break;
            }
            current_mana.0 -= mana_cost_per_bomb;
            trigger_counts.record_mana(Heirloom::CherryBomb, mana_cost_per_bomb);

            let target_pos = random_cherry_bomb_target(player_pos, &mut rng);
            spawn_cherry_bomb_flight(
                &mut commands,
                &graphics,
                player_pos,
                target_pos,
                explosion_damage,
            );
            bombs_spawned += 1;
        }

        if bombs_spawned > 0 {
            trigger_counts.increment(Heirloom::CherryBomb);
        }
    }
}

pub fn update_lob_arcs(
    mut commands: Commands,
    time: Res<Time>,
    graphics: Res<Graphics>,
    player_size: Query<&ProjectileSize, With<Player>>,
    mut bombs: Query<(Entity, &mut LobArc, &mut Transform)>,
    mut ranged_attack_events: MessageWriter<RangedAttackEvent>,
    enemies: Query<(Entity, &GlobalTransform), With<Mob>>,
    mut mob_status: Query<&mut MobStatusEffects, With<Mob>>,
    mut status_event: MessageWriter<StatusEffectEvent>,
) {
    let size_multiplier = player_size
        .single()
        .map(|s| s.get_multiplier())
        .unwrap_or(1.0);

    for (entity, mut arc, mut transform) in bombs.iter_mut() {
        arc.timer.tick(time.delta());
        let duration = arc.timer.duration().as_secs_f32().max(f32::EPSILON);
        let t = (arc.timer.elapsed_secs() / duration).clamp(0.0, 1.0);
        let ground = arc.start_pos.lerp(arc.target_pos, t);
        let height = arc.arc_height * (std::f32::consts::PI * t).sin();
        transform.translation = Vec3::new(ground.x, ground.y + height, transform.translation.z);

        if arc.timer.just_finished() {
            match arc.landing {
                LobArcLanding::CherryBomb { explosion_damage } => {
                    spawn_cherry_bomb_explosion(
                        &mut commands,
                        &graphics,
                        arc.target_pos,
                        explosion_damage,
                        size_multiplier,
                    );
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.25));
                }
                LobArcLanding::SkillBomb { explosion_damage } => {
                    ranged_attack_events.write(RangedAttackEvent {
                        projectile: Projectile::BombExplosion,
                        direction: Vec2::ZERO,
                        mana_cost: None,
                        mana_cost_heirloom: None,
                        from_enemy: false,
                        from_entity: None,
                        is_followup_proj: false,
                        dmg_override: Some(explosion_damage),
                        pos_override: Some(arc.target_pos),
                        spawn_delay: 0.0,
                    });
                    apply_bomb_frail_at_position(
                        arc.target_pos,
                        &enemies,
                        &mut mob_status,
                        &mut status_event,
                    );
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

const BOMB_FRAIL_RADIUS: f32 = 50.0;

pub fn apply_bomb_frail_at_position(
    target_pos: Vec2,
    enemies: &Query<(Entity, &GlobalTransform), With<Mob>>,
    mob_status: &mut Query<&mut MobStatusEffects, With<Mob>>,
    status_event: &mut MessageWriter<StatusEffectEvent>,
) {
    for (enemy_entity, enemy_transform) in enemies.iter() {
        let enemy_pos = enemy_transform.translation().truncate();
        if target_pos.distance(enemy_pos) > BOMB_FRAIL_RADIUS {
            continue;
        }
        if let Ok(mut status) = mob_status.get_mut(enemy_entity) {
            status.frail = Some(crate::combat::status_effects::Frail {
                num_stacks: 3,
                timer: Timer::from_seconds(1.2, TimerMode::Repeating),
            });
        }
        status_event.write(StatusEffectEvent {
            entity: enemy_entity,
            effect: StatusEffect::Frail,
            num_stacks: 3,
        });
    }
}
