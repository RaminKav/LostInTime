use std::{collections::HashSet, f32::consts::TAU};

use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use rand::Rng;

use crate::{
    assets::Graphics,
    attributes::{CurrentHealth, MaxHealth},
    combat::{EnemyDeathEvent, HitEvent},
    custom_commands::CommandsExt,
    enemy::Mob,
    item::WorldObject,
    player::{
        skills::{Heirloom, PlayerSkills},
        Player,
    },
    proto::proto_param::ProtoParam,
    world::y_sort::YSort,
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
        let (damage, _) = game.calculate_player_damage(commands, target, 0, Some(percent), 0, None);
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
                ignore_tool: true,
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
                    ignore_tool: true,
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
        let Some(main_hand) = game.player().main_hand_slot.clone() else {
            continue;
        };
        if !main_hand.get_obj().is_magic_weapon() {
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

pub fn update_reaper_souls(
    mut commands: Commands,
    time: Res<Time>,
    mut souls: Query<(Entity, &mut Transform, &mut ReaperSoul)>,
    mobs: Query<(Entity, &GlobalTransform, &CurrentHealth, &MaxHealth, &Mob), With<Mob>>,
    mut hit_events: EventWriter<HitEvent>,
    game: GameParam,
) {
    let mob_snapshots = gather_live_mobs(&mobs);

    if mob_snapshots.is_empty() {
        for (entity, _, mut soul) in souls.iter_mut() {
            soul.lifetime.tick(time.delta());
            if soul.lifetime.finished() {
                commands.entity(entity).despawn_recursive();
            }
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

        let Some(snapshot) = target_snapshot else {
            soul.target = None;
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
                ignore_tool: true,
            });
            commands.entity(entity).despawn_recursive();
        }
    }
}
