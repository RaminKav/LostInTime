use std::{collections::HashSet, f32::consts::TAU};

use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::{Collider, RapierContext, RigidBody, Sensor};
use rand::{seq::SliceRandom, Rng};

use crate::{
    assets::Graphics,
    attributes::{
        modifiers::ModifyManaEvent, Attack, CurrentHealth, CurrentMana, ManaRegen, MaxHealth,
        ProjectileSize,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    combat::{
        status_effects::{Burning, MobStatusEffects, StatusEffect, StatusEffectEvent},
        EnemyDeathEvent, HitEvent, ObjBreakEvent,
    },
    custom_commands::CommandsExt,
    enemy::{EliteMob, Mob},
    item::{
        projectile::{AnimVisualCategory, Projectile, RangedAttackEvent},
        ItemDrop, WorldObject,
    },
    player::{
        skills::{ActiveSkillUsedEvent, Heirloom, HeirloomTriggerCounts, PlayerSkills},
        Player,
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, y_sort::YSort, TileMapPosition},
    GameParam,
};

const ANT_FARM_COOLDOWN: f32 = 1.5;
const ANT_SPEED: f32 = 180.0;
const ANT_CONTACT_DISTANCE: f32 = 8.0;
const ANT_LIFETIME: f32 = 4.0;
const ANT_CHAIN_DELAY: f32 = 0.25;

/// Time for one full orbit (rotation speed); faster = snappier feel.
const STONE_TOOTH_ORBIT_PERIOD: f32 = 1.2;
/// Delay between spawning the next batch.
const STONE_TOOTH_SPAWN_INTERVAL: f32 = 2.8;
const STONE_TOOTH_ROCK_LIFETIME: f32 = 1.5;
/// Distance rocks travel outward from the player over their lifetime.
const STONE_TOOTH_TRAVEL_DISTANCE: f32 = 70.0;
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
const SUMMON_RING_LIFETIME: f32 = 2.2;
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
}

/// Tracks lifetime and which enemies this rock has already hit (pierce: hit each once).
#[derive(Component)]
pub struct StoneToothRockLifetime {
    pub lifetime: Timer,
    pub hit_entities: HashSet<Entity>,
}

/// Fired when healing triggers "all summons once" (e.g. HealSummons heirloom).
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
    commands: &mut Commands,
    game: &GameParam,
    target: Entity,
    frail_stacks: u8,
) -> (i32, bool, bool) {
    let (damage, was_crit, was_overcrit) =
        game.calculate_player_damage(commands, target, 0, None, 0, None, frail_stacks, 0);
    (i32::max(1, damage as i32), was_crit, was_overcrit)
}

fn get_world_object_sprite(graphics: &Graphics, object: WorldObject) -> Option<TextureAtlasSprite> {
    graphics
        .spritesheet_map
        .as_ref()
        .and_then(|map| map.get(&object))
        .cloned()
}

// ----- Summon helpers: shared spawn logic for timer-based and on-heal triggers -----

/// Spawns up to `count` Ant Farm ants. Deducts mana per ant if `mana_value` is `Some`.
/// Returns the number actually spawned.
pub fn spawn_ant_farm_ants(
    commands: &mut Commands,
    texture_atlas: &Handle<TextureAtlas>,
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
            SpriteSheetBundle {
                texture_atlas: texture_atlas.clone(),
                sprite,
                transform: Transform::from_translation(
                    player_pos + Vec3::new(offset.x, offset.y, 0.2),
                ),
                ..default()
            },
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
    texture_atlas: &Handle<TextureAtlas>,
    graphics: &Graphics,
    player_e: Entity,
    player_pos: Vec3,
    count: usize,
    mana_value: &mut Option<&mut i32>,
    mana_cost_per: i32,
    size_multiplier: f32,
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
        // Rocks start at player and expand outward (offset applied in update_stone_tooth).
        let offset = Vec2::ZERO;
        if let Some(sprite_sheet) = &graphics.spritesheet_map {
            if let Some(sprite) = sprite_sheet.get(&WorldObject::BoulderHeirloom) {
                let mut scaled_sprite = sprite.clone();
                scaled_sprite.custom_size = Some(Vec2::splat(32.0) * size_multiplier);
                let transform =
                    Transform::from_translation(player_pos + Vec3::new(offset.x, offset.y, 0.25));
                commands.spawn((
                    SpriteSheetBundle {
                        texture_atlas: texture_atlas.clone(),
                        sprite: scaled_sprite.clone(),
                        transform,
                        ..default()
                    },
                    OrbitingStone {
                        owner: player_e,
                        base_angle,
                        active: true,
                        size_multiplier,
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
    texture_atlas: &Handle<TextureAtlas>,
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
            SpriteSheetBundle {
                texture_atlas: texture_atlas.clone(),
                sprite,
                transform: Transform {
                    translation: player_pos + Vec3::new(0., 0., 0.25),
                    rotation,
                    ..default()
                },
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
        player_query.get_single_mut()
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

    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };

    let count_usize = count_i32.max(0) as usize;
    let mut mana_opt = Some(&mut curr_mana.0);
    let spawned = spawn_ant_farm_ants(
        &mut commands,
        texture_atlas,
        &graphics,
        player_pos,
        count_usize,
        &mut mana_opt,
        Heirloom::AntFarm.get_mana_cost(),
        size_mult,
    );
    if spawned > 0 {
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
        player_query.get_single_mut()
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

    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };

    let count_usize = count_i32.max(0) as usize;
    let mut mana_opt = Some(&mut curr_mana.0);
    let spawned = spawn_summon_ring_rings(
        &mut commands,
        texture_atlas,
        &graphics,
        player_e,
        player_pos,
        count_usize,
        &mut mana_opt,
        Heirloom::SummonRing.get_mana_cost(),
        size_mult,
    );
    if spawned > 0 {
        trigger_counts.increment(Heirloom::SummonRing);
    }
}

const SUMMON_RING_RETURN_REACH_DISTANCE: f32 = 12.0;

pub fn update_summon_ring(
    mut commands: Commands,
    time: Res<Time>,
    rapier_context: Res<RapierContext>,
    mut rings: Query<(Entity, &mut Transform, &mut SummonRingProjectile)>,
    player_transforms: Query<&GlobalTransform, With<Player>>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    world_objects: Query<
        (Entity, &GlobalTransform, &WorldObject),
        (Without<Mob>, Without<ItemDrop>),
    >,
    player_entity: Query<Entity, With<Player>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: EventWriter<HitEvent>,
    game: GameParam,
) {
    let mob_snapshots = gather_live_mobs(&mobs);
    let delta = time.delta_seconds();
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
            for (e1, e2, _) in rapier_context.intersections_with(ring_entity) {
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
                    hit_events.send(HitEvent {
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
                    });
                }
            }
            continue;
        }

        ring.lifetime.tick(time.delta());
        ring.bounce_cooldown.tick(time.delta());
        if ring.lifetime.finished() {
            ring.returning = true;
            continue;
        }

        let new_pos = ring_pos + ring.direction * move_step;
        transform.translation = new_pos.extend(transform.translation.z);
        transform.rotation = Quat::from_rotation_z(ring.direction.y.atan2(ring.direction.x));

        for (e1, e2, _) in rapier_context.intersections_with(ring_entity) {
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
                hit_events.send(HitEvent {
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
                if !ring.bounce_cooldown.finished() {
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
        commands.entity(e).despawn_recursive();
    }
}

pub fn update_ant_farm_ants(
    mut commands: Commands,
    time: Res<Time>,
    mut ants: Query<(Entity, &mut Transform, &mut AntFarmAnt)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: EventWriter<HitEvent>,
    game: GameParam,
) {
    let mob_snapshots = gather_live_mobs(&mobs);

    if mob_snapshots.is_empty() {
        for (entity, _, mut ant) in ants.iter_mut() {
            ant.lifetime.tick(time.delta());
            if ant.lifetime.finished() {
                commands.entity(entity).despawn_recursive();
                continue;
            }
            ant.spawn_delay.tick(time.delta());
        }
        return;
    }

    let mut claimed_targets: HashSet<Entity> = HashSet::new();

    for (entity, mut transform, mut ant) in ants.iter_mut() {
        ant.lifetime.tick(time.delta());
        if ant.lifetime.finished() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        ant.spawn_delay.tick(time.delta());
        if !ant.spawn_delay.finished() {
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
        let step = (ant.speed * time.delta_seconds()).min(distance);
        transform.translation += (direction * step).extend(0.0);

        let contact_dist = ANT_CONTACT_DISTANCE * ant.size_multiplier;
        if transform.translation.truncate().distance(snapshot.position) <= contact_dist {
            let frail_stacks = frail_query
                .get(snapshot.entity)
                .map(|s| s.frail_stacks())
                .unwrap_or(0);
            let (damage, was_crit, was_overcrit) =
                calculate_summon_damage(&mut commands, &game, snapshot.entity, frail_stacks);
            hit_events.send(HitEvent {
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
            });
            commands.entity(entity).despawn_recursive();
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
    mut hit_events: EventWriter<HitEvent>,
    graphics: Res<Graphics>,
    mut game: GameParam,
) {
    let Ok((player_e, player_txfm, skills, projectile_size, mut state_option, mut curr_mana)) =
        player_query.get_single_mut()
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
            state.elapsed += time.delta_seconds();
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
                commands.entity(entity).despawn_recursive();
            }
        }
        return;
    }

    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };

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
        if lifetime.lifetime.finished() {
            to_despawn.push(entity);
            continue;
        }
        let elapsed = lifetime.lifetime.elapsed().as_secs_f32();
        let angle = stone.base_angle + (elapsed / STONE_TOOTH_ORBIT_PERIOD) * TAU;
        // Expand outward from player over lifetime (0 -> STONE_TOOTH_TRAVEL_DISTANCE).
        let radius = (elapsed / STONE_TOOTH_ROCK_LIFETIME) * STONE_TOOTH_TRAVEL_DISTANCE + 24.;
        let offset = Vec2::from_angle(angle) * radius;
        transform.translation = Vec3::new(
            player_xy.x + offset.x,
            player_xy.y + offset.y,
            player_pos.z + 0.25,
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
                hit_events.send(HitEvent {
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
                });
                lifetime.hit_entities.insert(snapshot.entity);
            }
        }
    }
    for entity in to_despawn {
        commands.entity(entity).despawn_recursive();
    }

    // On timer: spawn a batch of rocks (orbit 1.5s, then ~3s delay before next batch)
    if !should_spawn_stones {
        return;
    }
    let mut mana_opt = Some(&mut curr_mana.0);
    let spawned = spawn_stone_tooth_rocks(
        &mut commands,
        texture_atlas,
        &graphics,
        player_e,
        player_pos,
        stacks as usize,
        &mut mana_opt,
        Heirloom::StoneTooth.get_mana_cost(),
        size_mult,
    );
    if spawned > 0 {
        game.heirloom_trigger_counts.increment(Heirloom::StoneTooth);
    }
}

pub fn handle_trigger_summons_on_heal(
    mut commands: Commands,
    mut trigger_events: EventReader<TriggerSummonsEvent>,
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
    graphics: Res<Graphics>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };
    for event in trigger_events.iter() {
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
                texture_atlas,
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
            let mut mana_opt = Some(&mut curr_mana.0);
            spawn_stone_tooth_rocks(
                &mut commands,
                texture_atlas,
                &graphics,
                player_e,
                player_pos,
                stone_stacks,
                &mut mana_opt,
                0,
                size_mult,
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
                texture_atlas,
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
    mut death_events: EventReader<EnemyDeathEvent>,
    mut player_query: Query<(&PlayerSkills, &mut CurrentMana), With<Player>>,
    graphics: Res<Graphics>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, mut curr_mana)) = player_query.get_single_mut() else {
        return;
    };
    let stacks = skills.get_count(Heirloom::Reaper);
    let spawn_count = stacks.max(0) as usize;

    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };
    let Some(sprite_template) = get_world_object_sprite(&graphics, WorldObject::ReaperSoul) else {
        return;
    };

    let mob_snapshots = gather_live_mobs(&mobs);
    if mob_snapshots.is_empty() || spawn_count == 0 {
        return;
    }

    let mut rng = rand::thread_rng();
    for event in death_events.iter() {
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
            } else {
                break;
            }
            let offset = Vec2::from_angle(rng.gen_range(0.0..TAU)) * rng.gen_range(0.0..4.0);
            let mut sprite = sprite_template.clone();
            sprite.custom_size = Some(Vec2::splat(14.0));
            commands.spawn((
                SpriteSheetBundle {
                    texture_atlas: texture_atlas.clone(),
                    sprite,
                    transform: Transform::from_translation(
                        (event.enemy_pos + offset).extend(0.3 + i as f32 * 0.01),
                    ),
                    ..default()
                },
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
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut death_events: EventReader<EnemyDeathEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let mut rng = rand::thread_rng();
    const MANA_ORB_DROP_CHANCE: f64 = 0.1;
    for event in death_events.iter() {
        if !rng.gen_bool(MANA_ORB_DROP_CHANCE) {
            continue;
        }
        let offset = Vec2::new(rng.gen_range(-10.0..10.0), rng.gen_range(-10.0..10.0));
        proto_commands.spawn_item_from_proto(
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
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut hit_events: EventReader<HitEvent>,
    mobs: Query<(&Mob, &GlobalTransform, Option<&EliteMob>)>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let mut rng = rand::thread_rng();
    const MANA_ORB_DROP_CHANCE: f64 = 0.01;

    for hit in hit_events.iter() {
        // Check if hit entity is a boss
        let Ok((mob, boss_transform, is_elite)) = mobs.get(hit.hit_entity) else {
            continue;
        };

        if !mob.is_boss() && is_elite.is_none() {
            continue;
        }

        if !rng.gen_bool(MANA_ORB_DROP_CHANCE) {
            continue;
        }

        // Spawn mana orb within 32px of boss location
        let boss_pos = boss_transform.translation().truncate();
        let angle = rng.gen_range(0.0..TAU);
        let distance = rng.gen_range(0.0..32.0);
        let offset = Vec2::new(angle.cos(), angle.sin()) * distance;

        proto_commands.spawn_item_from_proto(
            WorldObject::ManaOrb,
            &proto,
            boss_pos + offset,
            1,
            None,
        );
        trigger_counts.increment(Heirloom::ManaOrbs);
    }
}

pub fn update_reaper_souls(
    mut commands: Commands,
    time: Res<Time>,
    mut souls: Query<(Entity, &mut Transform, &mut ReaperSoul)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    frail_query: Query<&MobStatusEffects>,
    mut hit_events: EventWriter<HitEvent>,
    game: GameParam,
) {
    let mob_snapshots = gather_live_mobs(&mobs);

    // If no mobs exist, despawn all souls immediately
    if mob_snapshots.is_empty() {
        for (entity, _, _) in souls.iter() {
            commands.entity(entity).despawn_recursive();
        }
        return;
    }

    let mut claimed_targets: HashSet<Entity> = HashSet::new();

    for (entity, mut transform, mut soul) in souls.iter_mut() {
        soul.lifetime.tick(time.delta());
        if soul.lifetime.finished() {
            commands.entity(entity).despawn_recursive();
            continue;
        }

        let soul_pos = transform.translation.truncate();
        let target_snapshot =
            pick_target(soul.target, soul_pos, &mut claimed_targets, &mob_snapshots);

        // If no target can be found, despawn the soul
        let Some(snapshot) = target_snapshot else {
            commands.entity(entity).despawn_recursive();
            continue;
        };

        soul.target = Some(snapshot.entity);

        let to_target = snapshot.position - soul_pos;
        let direction = to_target.normalize_or_zero();
        if direction.length_squared() == 0.0 {
            continue;
        }
        soul.drift_phase += REAPER_SOUL_DRIFT_FREQ * time.delta_seconds();
        let drift_amount = soul.drift_phase.sin() * REAPER_SOUL_DRIFT_STRENGTH;
        let perp = Vec2::new(-direction.y, direction.x);
        let mut steering = direction + perp * drift_amount;
        steering = steering.normalize_or_zero();
        let step = soul.speed * time.delta_seconds();
        transform.translation += (steering * step).extend(0.0);

        if transform.translation.truncate().distance(snapshot.position) <= REAPER_CONTACT_DISTANCE {
            let frail_stacks = frail_query
                .get(snapshot.entity)
                .map(|s| s.frail_stacks())
                .unwrap_or(0);
            let (damage, was_crit, was_overcrit) =
                calculate_summon_damage(&mut commands, &game, snapshot.entity, frail_stacks);
            hit_events.send(HitEvent {
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
            });
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn break_crates_with_roll(
    game: GameParam,
    rapier_context: Res<RapierContext>,
    crate_query: Query<(Entity, &GlobalTransform, &WorldObject)>,
    mut obj_break_events: EventWriter<ObjBreakEvent>,
    mut broken_this_frame: Local<HashSet<Entity>>,
) {
    broken_this_frame.clear();

    let player_entity = game.game.player;

    let player_state = game.player();
    if !player_state.is_dashing {
        return;
    }

    let mut to_break: Vec<(Entity, WorldObject, TileMapPosition)> = Vec::new();
    for (first, second, _) in rapier_context.intersections_with(player_entity) {
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
        obj_break_events.send(ObjBreakEvent {
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
    mut death_events: EventReader<EnemyDeathEvent>,
    mut player_query: Query<(&mut MaxHPHuntTracker, &PlayerSkills), With<Player>>,
    mut attribute_events: EventWriter<crate::attributes::AttributeChangeEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((mut tracker, skills)) = player_query.get_single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::MaxHPHunt);
    if stacks <= 0 {
        return;
    }

    let max_hp_cap = stacks * 250;

    let mut hp_was_gained = false;
    for _death_event in death_events.iter() {
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
        attribute_events.send_default();
    }
}

// ============================================================================
// SkillPowerHunt - 3% chance when using a skill to gain +1 Skill Power
// ============================================================================

pub fn handle_skill_power_hunt(
    mut skill_events: EventReader<ActiveSkillUsedEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut SkillPowerHuntTracker>), With<Player>>,
    mut attribute_events: EventWriter<crate::attributes::AttributeChangeEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, state_option)) = player_query.get_single_mut() else {
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
    for _ in skill_events.iter() {
        if tracker.bonus_skill_power >= sp_cap {
            continue;
        }
        if rng.gen_ratio((count * 7).min(100) as u32, 100) {
            tracker.bonus_skill_power = (tracker.bonus_skill_power + 1).min(sp_cap);
            attribute_events.send_default();
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
    let Ok((mut state, movement)) = player_query.get_single_mut() else {
        return;
    };

    let is_moving = movement.0.length_squared() > 0.01;

    if is_moving {
        state.time_still = 0.0;
        state.was_moving = true;
    } else {
        state.time_still = (state.time_still + time.delta_seconds()).min(STAND_STILL_MAX_TIME);
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

/// Marker component for mobs frozen by Death Defiance. Inserted briefly on
/// every mob in range when the heirloom procs, removed when the freeze timer
/// elapses. Stored `SparseSet` so the freeze doesn't move a bunch of mobs to
/// a new archetype and back each proc.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct DeathDefianceFrozen {
    pub timer: Timer,
    pub original_color: Color,
}

pub fn handle_death_defiance_freeze(
    mut commands: Commands,
    time: Res<Time>,
    mut frozen_mobs: Query<(Entity, &mut DeathDefianceFrozen, &mut TextureAtlasSprite)>,
) {
    for (entity, mut frozen, mut sprite) in frozen_mobs.iter_mut() {
        frozen.timer.tick(time.delta());

        if frozen.timer.just_finished() {
            // Restore original color and remove freeze
            sprite.color = frozen.original_color;
            commands.entity(entity).remove::<DeathDefianceFrozen>();
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

pub fn handle_crate_break_damage(
    mut obj_break_events: EventReader<ObjBreakEvent>,
    mut player_query: Query<(&mut CrateBreakDamageTracker, &PlayerSkills), With<Player>>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((mut tracker, skills)) = player_query.get_single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::CrateBreakDamage);
    if stacks <= 0 {
        return;
    }

    for event in obj_break_events.iter() {
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
            buff_timer: Timer::from_seconds(3.0, TimerMode::Once),
            next_hit_bonus: false,
        }
    }
}

pub fn handle_dodge_crit_activation(
    mut dodge_events: EventReader<crate::ui::damage_numbers::DodgeEvent>,
    mut player_query: Query<(&mut DodgeCritState, &PlayerSkills), With<Player>>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((mut state, skills)) = player_query.get_single_mut() else {
        return;
    };

    if !skills.has(Heirloom::DodgeCrit) {
        return;
    }

    for _ in dodge_events.iter() {
        // Activate the buff
        state.buff_active = true;
        state.buff_timer.reset();
        state.next_hit_bonus = true;
        attribute_event.send(crate::attributes::AttributeChangeEvent);
        trigger_counts.increment(Heirloom::DodgeCrit);
    }
}

pub fn tick_dodge_crit_buff(
    time: Res<Time>,
    mut player_query: Query<&mut DodgeCritState, With<Player>>,
    mut attribute_event: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    let Ok(mut state) = player_query.get_single_mut() else {
        return;
    };

    if state.buff_active {
        state.buff_timer.tick(time.delta());
        if state.buff_timer.just_finished() {
            state.buff_active = false;
            state.next_hit_bonus = false; // Also clear the next hit bonus
            attribute_event.send(crate::attributes::AttributeChangeEvent);
        }
    }
}

/// Reset DodgeCrit next hit bonus after player deals damage
pub fn handle_dodge_crit_next_hit_reset(
    mut hit_events: EventReader<crate::combat::HitEvent>,
    mut player_query: Query<&mut DodgeCritState, With<Player>>,
) {
    // Only process hits that are from the player (not from mobs hitting player)
    for hit in hit_events.iter() {
        if hit.hit_by_mob.is_some() {
            continue;
        }

        // Reset the next hit bonus
        if let Ok(mut state) = player_query.get_single_mut() {
            if state.next_hit_bonus {
                state.next_hit_bonus = false;
            }
        }
        break; // Only need to process once per frame
    }
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
    mut mana_events: EventReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut ManaChargeDamageState>), With<Player>>,
) {
    let Ok((skills, state_option)) = player_query.get_single_mut() else {
        return;
    };

    // Only process if player has the heirloom
    if skills.get_count(Heirloom::MPBarDMG) <= 0 {
        return;
    }

    let Some(mut state) = state_option else {
        return;
    };

    for event in mana_events.iter() {
        // Only track positive mana changes (regen, not consumption)
        if event.0 > 0 {
            state.add_mana(event.0);
        }
    }
}

/// System to reset stored mana after an attack is made.
/// Runs after HitEvents are processed.
pub fn handle_mana_charge_damage_reset(
    mut hit_events: EventReader<HitEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut ManaChargeDamageState>), With<Player>>,
) {
    let Ok((skills, state_option)) = player_query.get_single_mut() else {
        return;
    };

    if skills.get_count(Heirloom::MPBarDMG) <= 0 {
        return;
    }
    // Only reset if there was a hit from the player (not from mobs, not from heirloom effects)
    let mut player_dealt_damage = false;
    for event in hit_events.iter() {
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
    mut mana_events: EventReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, &GlobalTransform), With<Player>>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth), With<Mob>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, player_transform)) = player_query.get_single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::ManaOrbAttack);
    if stacks <= 0 {
        return;
    }

    let player_pos = player_transform.translation().truncate();
    let mut rng = rand::thread_rng();

    for event in mana_events.iter() {
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
        trigger_counts.increment(Heirloom::ManaOrbAttack);

        // Spawn +1 orb per stack
        let orb_count = stacks as usize;
        for i in 0..orb_count {
            // Pick a random nearby enemy
            if let Some((_, target_transform, _)) = nearby_mobs.choose(&mut rng) {
                let target_pos = target_transform.translation().truncate();
                let direction = (target_pos - player_pos).normalize_or_zero();
                ranged_attack_event.send(RangedAttackEvent {
                    projectile: crate::item::projectile::Projectile::ManaOrbProjectile,
                    direction,
                    mana_cost: None,
                    from_enemy: false,
                    from_entity: None,
                    is_followup_proj: true,
                    dmg_override: Some(event.0), // Damage equals mana regen amount
                    pos_override: Some(player_pos),
                    spawn_delay: i as f32 * 0.1, // Slight delay between orbs
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
    mut mana_events: EventReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, Option<&mut ManaRegenPoisonTracker>), With<Player>>,
    enemies: Query<Entity, (With<Mob>, Without<Player>)>,
    mut mob_status: Query<&mut MobStatusEffects, With<Mob>>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut status_event: EventWriter<StatusEffectEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, state_option)) = player_query.get_single_mut() else {
        return;
    };

    // Only process if player has the heirloom
    let heirloom_count = skills.get_count(Heirloom::ManaRegenPoison);
    if heirloom_count <= 0 {
        return;
    }

    let poison_duration_bonus = player_skills
        .get_single()
        .map(|s| s.get_count(Heirloom::PoisonDuration) as f32 * 0.5 + 1.)
        .unwrap_or(1.0);

    let mut tracker = if let Some(state) = state_option {
        state
    } else {
        return;
    };

    for event in mana_events.iter() {
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
                        status_event.send(StatusEffectEvent {
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
                        status_event.send(StatusEffectEvent {
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

// SkillManaRegen - Using a skill has a 20% chance to trigger mana regen
// ============================================================================

/// System to trigger mana regen when a skill is used
pub fn handle_skill_mana_regen(
    mut skill_events: EventReader<ActiveSkillUsedEvent>,
    mut player_query: Query<(&PlayerSkills, &ManaRegen), With<Player>>,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, mana_regen)) = player_query.get_single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::SkillManaRegen);
    if stacks <= 0 {
        return;
    }

    let mut rng = rand::thread_rng();

    for _event in skill_events.iter() {
        // 20% chance per stack (capped at 100%)
        let chance_per_stack = 20;
        let total_chance = (stacks * chance_per_stack).min(100);
        if rng.gen_ratio(total_chance as u32, 100) {
            // Trigger mana regen (same amount as normal regen)
            modify_mana_event.send(ModifyManaEvent(mana_regen.0));
            trigger_counts.increment(Heirloom::SkillManaRegen);
        }
    }
}

// ManaRegenLightning - Mana regen has a 10% chance per stack to trigger lightning
// ============================================================================

/// System to spawn lightning strikes when mana is regenerated
pub fn handle_mana_regen_lightning(
    mut mana_events: EventReader<ModifyManaEvent>,
    mut player_query: Query<(&PlayerSkills, &GlobalTransform, &Attack, &CurrentMana), With<Player>>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth), With<Mob>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut commands: Commands,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    let Ok((skills, player_transform, attack, current_mana)) = player_query.get_single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::ManaRegenLightning);
    if stacks <= 0 {
        return;
    }

    let player_pos = player_transform.translation().truncate();
    let mut rng = rand::thread_rng();

    for event in mana_events.iter() {
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
                ranged_attack_event.send(RangedAttackEvent {
                    projectile: Projectile::Lightning,
                    direction: Vec2::ZERO,
                    mana_cost: Some(MANA_COST),
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
