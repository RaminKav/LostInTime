use std::{collections::HashSet, f32::consts::TAU};

use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::RapierContext;
use rand::Rng;

use crate::{
    assets::Graphics,
    attributes::{CurrentHealth, MaxHealth},
    combat::{EnemyDeathEvent, HitEvent, ObjBreakEvent},
    custom_commands::CommandsExt,
    enemy::{EliteMob, Mob},
    item::WorldObject,
    player::{
        skills::{Heirloom, PlayerSkills},
        Player,
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, y_sort::YSort, TileMapPosition},
    GameParam,
};

const ANT_FARM_COOLDOWN: f32 = 3.0;
const ANT_SPEED: f32 = 180.0;
const ANT_CONTACT_DISTANCE: f32 = 8.0;
const ANT_LIFETIME: f32 = 6.0;
const ANT_CHAIN_DELAY: f32 = 0.25;

const STONE_TOOTH_PERIOD: f32 = 1.5;
const STONE_TOOTH_RADIUS: f32 = 24.0;
const STONE_CONTACT_DISTANCE: f32 = 12.0;

const REAPER_SOUL_SPEED: f32 = 220.0;
const REAPER_SOUL_LIFETIME: f32 = 6.0;
const REAPER_DAMAGE_PERCENT: f32 = 0.25;
const REAPER_CONTACT_DISTANCE: f32 = 12.0;
const REAPER_SOUL_MAX_SPAWN_RANGE: f32 = 320.0;
const REAPER_SOUL_DRIFT_STRENGTH: f32 = 0.75;
const REAPER_SOUL_DRIFT_FREQ: f32 = 10.5;

#[derive(Clone)]
struct MobSnapshot {
    entity: Entity,
    position: Vec2,
    max_health: i32,
    kind: Mob,
}

#[derive(Component)]
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
}

#[derive(Component, Default)]
pub struct StoneToothState {
    pub elapsed: f32,
}

#[derive(Component)]
pub struct OrbitingStone {
    pub owner: Entity,
    pub base_angle: f32,
    pub active: bool,
}

#[derive(Component, Default)]
pub struct ReaperState;

#[derive(Component)]
pub struct ReaperSoul {
    pub target: Option<Entity>,
    pub damage_fraction: f32,
    pub speed: f32,
    pub lifetime: Timer,
    pub drift_phase: f32,
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

fn calculate_percent_damage(
    commands: &mut Commands,
    game: &GameParam,
    target: Entity,
    max_health: i32,
    is_boss: bool,
    percent: f32,
) -> i32 {
    if is_boss {
        let (damage, _, _) =
            game.calculate_player_damage(commands, target, 0, Some(percent), 0, None, 0);
        i32::max(1, damage as i32)
    } else {
        ((max_health as f32) * percent).ceil().max(1.0) as i32
    }
}

fn get_world_object_sprite(graphics: &Graphics, object: WorldObject) -> Option<TextureAtlasSprite> {
    graphics
        .spritesheet_map
        .as_ref()
        .and_then(|map| map.get(&object))
        .cloned()
}

pub fn handle_ant_farm_state(
    mut commands: Commands,
    time: Res<Time>,
    mut player_query: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            Option<&mut AntFarmState>,
        ),
        With<Player>,
    >,
    graphics: Res<Graphics>,
) {
    let Ok((player_e, player_txfm, skills, mut state_option)) = player_query.get_single_mut()
    else {
        return;
    };
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

    let mut rng = rand::thread_rng();
    let count_usize = count_i32.max(0) as usize;
    for i in 0..count_usize {
        let angle = rng.gen_range(0.0..TAU);
        let distance = rng.gen_range(0.0..6.0);
        let offset = Vec2::from_angle(angle) * distance;

        let mut sprite = graphics.get_heirloom_icon(Heirloom::AntFarm);
        sprite.custom_size = Some(Vec2::splat(12.0));

        let transform =
            Transform::from_translation(player_pos + Vec3::new(offset.x, offset.y, 0.2));

        commands.spawn((
            SpriteSheetBundle {
                texture_atlas: texture_atlas.clone(),
                sprite,
                transform,
                ..default()
            },
            AntFarmAnt {
                target: None,
                damage_fraction: 1.0 / 3.0,
                speed: ANT_SPEED,
                lifetime: Timer::from_seconds(ANT_LIFETIME, TimerMode::Once),
                spawn_delay: Timer::from_seconds(i as f32 * ANT_CHAIN_DELAY, TimerMode::Once),
            },
            YSort(-0.2),
            Name::new("AntFarmAnt"),
        ));
    }
}

pub fn update_ant_farm_ants(
    mut commands: Commands,
    time: Res<Time>,
    mut ants: Query<(Entity, &mut Transform, &mut AntFarmAnt)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
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

        if transform.translation.truncate().distance(snapshot.position) <= ANT_CONTACT_DISTANCE {
            let damage = calculate_percent_damage(
                &mut commands,
                &game,
                snapshot.entity,
                snapshot.max_health,
                snapshot.kind.is_boss(),
                ant.damage_fraction,
            );
            hit_events.send(HitEvent {
                hit_entity: snapshot.entity,
                damage,
                dir: direction,
                hit_with_melee: None,
                hit_with_projectile: None,
                hit_by_mob: None,
                hit_by_pet: None,
                was_crit: false,
                was_overcrit: false,
                ignore_tool: true,
                from_heirloom_effect: true, // Ant heirloom effect shouldn't chain
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
            Option<&mut StoneToothState>,
        ),
        With<Player>,
    >,
    mut stones: Query<(Entity, &mut OrbitingStone, &mut Transform, &mut Visibility)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    mut hit_events: EventWriter<HitEvent>,
    graphics: Res<Graphics>,
    game: GameParam,
) {
    let Ok((player_e, player_txfm, skills, mut state_option)) = player_query.get_single_mut()
    else {
        return;
    };

    let stacks = skills.get_count(Heirloom::StoneTooth);
    let player_pos = player_txfm.translation();
    let had_state = state_option.is_some();

    let mut elapsed = 0.0;
    let mut respawn_stones = false;

    if stacks > 0 {
        if let Some(state) = state_option.as_mut() {
            state.elapsed += time.delta_seconds();
            if state.elapsed >= STONE_TOOTH_PERIOD {
                state.elapsed %= STONE_TOOTH_PERIOD;
                respawn_stones = true;
            }
            elapsed = state.elapsed;
        } else {
            respawn_stones = true;
            elapsed = 0.0;
        }
    }

    if stacks > 0 && !had_state {
        commands.entity(player_e).insert(StoneToothState::default());
    } else if stacks <= 0 && had_state {
        commands.entity(player_e).remove::<StoneToothState>();
    }

    if stacks <= 0 {
        for (entity, stone, _, _) in stones.iter_mut() {
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

    let mut owned_entities: Vec<Entity> = {
        let mut owned = Vec::new();
        for (entity, stone, _, _) in stones.iter_mut() {
            if stone.owner == player_e {
                owned.push(entity);
            }
        }
        owned
    };

    if owned_entities.len() < stacks as usize {
        for index in owned_entities.len()..stacks as usize {
            let base_angle = TAU * index as f32 / stacks as f32;
            let offset = Vec2::from_angle(base_angle) * STONE_TOOTH_RADIUS;
            let mut sprite = graphics.get_heirloom_icon(Heirloom::StoneTooth);
            if sprite.custom_size.is_none() {
                sprite.custom_size = Some(Vec2::splat(16.0));
            }

            let transform =
                Transform::from_translation(player_pos + Vec3::new(offset.x, offset.y, 0.25));

            let entity = commands
                .spawn((
                    SpriteSheetBundle {
                        texture_atlas: texture_atlas.clone(),
                        sprite,
                        transform,
                        ..default()
                    },
                    OrbitingStone {
                        owner: player_e,
                        base_angle,
                        active: true,
                    },
                    YSort(-0.1),
                    Name::new("StoneToothRock"),
                ))
                .id();
            owned_entities.push(entity);
        }
    } else if owned_entities.len() > stacks as usize {
        while owned_entities.len() > stacks as usize {
            if let Some(entity) = owned_entities.pop() {
                commands.entity(entity).despawn_recursive();
            }
        }
    }

    let rotation_progress = (elapsed / STONE_TOOTH_PERIOD).clamp(0.0, 1.0);
    let player_xy = player_pos.truncate();

    for (index, entity) in owned_entities.iter().enumerate() {
        let Ok((_entity_id, mut stone, mut transform, mut visibility)) = stones.get_mut(*entity)
        else {
            continue;
        };

        let base_angle = TAU * index as f32 / stacks as f32;
        stone.base_angle = base_angle;

        if respawn_stones {
            stone.active = true;
            *visibility = Visibility::Inherited;
        }

        let angle = base_angle + rotation_progress * TAU;
        let offset = Vec2::from_angle(angle) * STONE_TOOTH_RADIUS;
        transform.translation = Vec3::new(
            player_xy.x + offset.x,
            player_xy.y + offset.y,
            player_pos.z + 0.25,
        );

        if !stone.active {
            continue;
        }

        let stone_pos = transform.translation.truncate();
        for snapshot in mob_snapshots.iter() {
            let mob_pos = snapshot.position;
            if mob_pos.distance_squared(stone_pos)
                <= STONE_CONTACT_DISTANCE * STONE_CONTACT_DISTANCE
            {
                let dir = (mob_pos - stone_pos).normalize_or_zero();
                let damage = calculate_percent_damage(
                    &mut commands,
                    &game,
                    snapshot.entity,
                    snapshot.max_health,
                    snapshot.kind.is_boss(),
                    0.35,
                );
                hit_events.send(HitEvent {
                    hit_entity: snapshot.entity,
                    damage,
                    dir,
                    hit_with_melee: None,
                    hit_with_projectile: None,
                    hit_by_mob: None,
                    hit_by_pet: None,
                    was_crit: false,
                    was_overcrit: false,
                    ignore_tool: true,
                    from_heirloom_effect: true, // Stone heirloom effect shouldn't chain
                });
                stone.active = false;
                *visibility = Visibility::Hidden;
                break;
            }
        }
    }
}

pub fn handle_reaper_soul_spawns(
    mut commands: Commands,
    mut death_events: EventReader<EnemyDeathEvent>,
    player_query: Query<&PlayerSkills, With<Player>>,
    graphics: Res<Graphics>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
) {
    let Ok(skills) = player_query.get_single() else {
        for _ in death_events.iter() {}
        return;
    };
    let stacks = skills.get_count(Heirloom::Reaper);
    let spawn_count = stacks.max(0) as usize;

    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        for _ in death_events.iter() {}
        return;
    };
    let Some(sprite_template) = get_world_object_sprite(&graphics, WorldObject::ReaperSoul) else {
        for _ in death_events.iter() {}
        return;
    };

    let mob_snapshots = gather_live_mobs(&mobs);
    if mob_snapshots.is_empty() || spawn_count == 0 {
        for _ in death_events.iter() {}
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

        for i in 0..spawn_count {
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
    game: GameParam,
) {
    let mut rng = rand::thread_rng();
    for event in death_events.iter() {
        let has_mana_item = game.inv_slot_query.iter().any(|slot| {
            slot.obj_type
                .map(|obj| obj.is_magic_weapon())
                .unwrap_or(false)
        });
        if !has_mana_item {
            continue;
        }

        if !rng.gen_bool(0.30) {
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
    }
}

/// Drop mana orbs at 30% chance when player attacks a boss with a staff in hotbar
pub fn handle_boss_hit_mana_orb_drops(
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    mut hit_events: EventReader<HitEvent>,
    game: GameParam,
    mobs: Query<(&Mob, &GlobalTransform, Option<&EliteMob>)>,
) {
    let mut rng = rand::thread_rng();

    for hit in hit_events.iter() {
        // Check if hit entity is a boss
        let Ok((mob, boss_transform, is_elite)) = mobs.get(hit.hit_entity) else {
            continue;
        };

        if !mob.is_boss() && is_elite.is_none() {
            continue;
        }

        // Check if player has a staff in hotbar
        let has_staff = game.inv_slot_query.iter().any(|slot| {
            slot.obj_type
                .map(|obj| obj.is_magic_weapon())
                .unwrap_or(false)
        });

        if !has_staff {
            continue;
        }

        // 30% chance to drop mana orb
        if !rng.gen_bool(0.25) {
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
    }
}

pub fn update_reaper_souls(
    mut commands: Commands,
    time: Res<Time>,
    mut souls: Query<(Entity, &mut Transform, &mut ReaperSoul)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
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
            let damage = calculate_percent_damage(
                &mut commands,
                &game,
                snapshot.entity,
                snapshot.max_health,
                snapshot.kind.is_boss(),
                soul.damage_fraction,
            );
            hit_events.send(HitEvent {
                hit_entity: snapshot.entity,
                damage,
                dir: direction,
                hit_with_melee: None,
                hit_with_projectile: None,
                hit_by_mob: None,
                hit_by_pet: None,
                was_crit: false,
                was_overcrit: false,
                ignore_tool: true,
                from_heirloom_effect: true, // Reaper heirloom effect shouldn't chain
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

/// Tracks kills for the MaxHPHunt heirloom
#[derive(Component, Default)]
pub struct MaxHPHuntTracker {
    pub kill_count: u32,
    pub total_hp_gained: i32, // Total max HP gained from this heirloom
}

pub fn handle_max_hp_hunt(
    mut death_events: EventReader<EnemyDeathEvent>,
    mut player_query: Query<(&mut MaxHPHuntTracker, &PlayerSkills), With<Player>>,
    mut attribute_events: EventWriter<crate::attributes::AttributeChangeEvent>,
) {
    let Ok((mut tracker, skills)) = player_query.get_single_mut() else {
        return;
    };

    let stacks = skills.get_count(Heirloom::MaxHPHunt);
    if stacks <= 0 {
        return;
    }

    // Count kills from events (each event is one kill)
    let mut hp_was_gained = false;
    for _death_event in death_events.iter() {
        tracker.kill_count += 1;

        // Every 3 kills grants +1 max hp per stack
        if tracker.kill_count >= 3 {
            tracker.kill_count -= 3;
            let hp_gained = stacks; // Each stack gives +1 hp per trigger
            tracker.total_hp_gained += hp_gained; // Track total HP gained
            hp_was_gained = true;
        }
    }

    // Trigger attribute recalculation if HP was gained
    // This ensures the MaxHPHunt bonus is included in the max health calculation
    if hp_was_gained {
        attribute_events.send_default();
    }
}

// ============================================================================
// StandStill - Standing still increases damage
// ============================================================================

/// Tracks standing still time for the StandStill heirloom
#[derive(Component)]
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

/// Marker component for mobs frozen by Death Defiance
#[derive(Component)]
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

/// Tracks total damage bonus from breaking crates
#[derive(Component, Default)]
pub struct CrateBreakDamageTracker {
    pub bonus_damage_percent: f32,
}

pub fn handle_crate_break_damage(
    mut obj_break_events: EventReader<ObjBreakEvent>,
    mut player_query: Query<(&mut CrateBreakDamageTracker, &PlayerSkills), With<Player>>,
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
            crate::item::WorldObject::Crate | crate::item::WorldObject::Crate2
        ) {
            // Each crate gives 1% damage per stack
            tracker.bonus_damage_percent += 1.0 * stacks as f32;
        }
    }
}

// ============================================================================
// DodgeCrit - Dodging gives speed/attack speed buff and next hit does 2x damage
// ============================================================================

/// State for the DodgeCrit heirloom buff
#[derive(Component)]
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

/// Tracks accumulated stat bonuses from LethalBlow hallucinations
/// Tracks stat bonuses from LethalBlow hallucination executes.
/// Wraps ItemAttributes so it can be combined with player attributes using combine().
#[derive(Component, Default, Debug, Clone)]
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
