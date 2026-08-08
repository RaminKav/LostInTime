use crate::aseprite_assets::{Electricity, IceExplosion, IceFloor, SmallExplosion};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::Aseprite;
use bevy_rapier2d::{geometry::Collider, prelude::KinematicCharacterController};

use crate::{
    animations::player_sprite::PlayerAnimation,
    assets::Graphics,
    attributes::{attribute_helpers::skill_power_multiplier, Attack, SkillPower},
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    combat_helpers::{spawn_deferred_aseprite_collider, DeferredComponent},
    custom_commands::CommandsExt,
    inputs::{skill_aim_direction, FacingDirection, MovementVector},
    item::{
        projectile::{AnimVisualCategory, FromActiveSkill, Projectile},
        WorldObject,
    },
    proto::proto_param::ProtoParam,
    world::{
        world_helpers::{get_neighbour_tile, tile_pos_to_world_pos, world_pos_to_tile_pos},
        TileMapPosition, TILE_SIZE,
    },
    GameParam,
};

use super::{
    skills::{
        active_skill_scaling::{attack_damage_multiplier, TELEPORT_SHOCK_ATTACK_PERCENT},
        ActiveSkill,
    },
    ActiveSkillUsedEvent, Heirloom, MovePlayerEvent, Player, PlayerSkills,
};

/// Brief marker set on the player right after teleporting; removed once the
/// post-teleport shock timer expires. Stored `SparseSet` so the player entity
/// stays in a single archetype across teleports.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct JustTeleported;

/// Active teleport cooldown/shock timers held on the player while the
/// Teleport skill is equipped. Inserted on skill equip and removed when
/// unequipped — stored `SparseSet` to avoid archetype moves on (un)equip.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct TeleportState {
    pub just_teleported_timer: Timer,
    pub timer: Timer,
}

#[derive(Component)]
pub struct IceExplosionDmg;

/// Same blocking rules as the previous teleport check: water tile, or tree/wall object.
fn teleport_tile_blocked_by_collider(
    pos: TileMapPosition,
    game: &GameParam,
    proto_param: &ProtoParam,
) -> bool {
    if let Some(tile_data) = game.get_tile_data(pos) {
        if tile_data.block_type.contains(&WorldObject::WaterTile) {
            return true;
        }
    }
    if let Some((_, obj)) = game.get_obj_entity_at_tile(pos, proto_param) {
        if obj.is_tree() || obj.is_wall() {
            return true;
        }
    }
    false
}

/// Intended landing tile, or the nearest open neighbor (by world distance to the player) among the 8 adjacent tiles.
pub(crate) fn resolve_teleport_destination_tile(
    intended_tile: TileMapPosition,
    player_world: Vec2,
    game: &GameParam,
    proto_param: &ProtoParam,
) -> Option<TileMapPosition> {
    if !teleport_tile_blocked_by_collider(intended_tile, game, proto_param) {
        return Some(intended_tile);
    }

    const NEIGHBOUR_OFFSETS: [(i8, i8); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];

    let mut neighbours: Vec<TileMapPosition> = NEIGHBOUR_OFFSETS
        .map(|offset| get_neighbour_tile(intended_tile, offset))
        .into();

    let tile_center = |t: TileMapPosition| {
        tile_pos_to_world_pos(t, false) + Vec2::new(TILE_SIZE.x * 0.5, TILE_SIZE.y * 0.5)
    };

    neighbours.sort_by(|a, b| {
        let da = tile_center(*a).distance_squared(player_world);
        let db = tile_center(*b).distance_squared(player_world);
        da.total_cmp(&db)
    });

    neighbours
        .into_iter()
        .find(|&t| !teleport_tile_blocked_by_collider(t, game, proto_param))
}

pub fn handle_teleport(
    mut active_skill_events: MessageReader<ActiveSkillUsedEvent>,
    mut move_player: MessageWriter<MovePlayerEvent>,
    mut player: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &mut MovementVector,
            &FacingDirection,
            &Attack,
            &SkillPower,
            &mut KinematicCharacterController,
            &mut TeleportState,
            &OwnedBlessings,
            &crate::blessings::OwnedMajorBlessings,
        ),
        (With<Player>, With<TeleportState>),
    >,
    aim: Res<crate::aim::AimState>,
    game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    time: Res<Time>,
) {
    // Player visuals use native ultra `AseAnimation`.
    // Requiring the compat component made this query never match → cooldown-only no-op.
    let Ok((
        e,
        player_pos,
        skills,
        mut move_direction,
        facing,
        dmg,
        skill_power,
        mut kcc,
        mut teleport_state,
        blessings,
        majors,
    )) = player.single_mut()
    else {
        return;
    };

    // Check if we received a teleport activation event
    // The cooldown check is already done in dispatch_active_skill_events, so we just need to check animation timer
    let mut should_activate = false;
    if let Some(teleport_slot) = skills.has_active_skill(ActiveSkill::Teleport) {
        for ev in active_skill_events.read() {
            if ev.slot == teleport_slot {
                // Only check if animation timer is ready (to prevent spamming)
                if teleport_state.timer.fraction() == 0. || teleport_state.timer.fraction() >= 1. {
                    should_activate = true;
                    break;
                }
            }
        }
    }

    if should_activate {
        commands.entity(e).insert(PlayerAnimation::Teleport);
        commands.spawn(SoundSpawner::new(AudioSoundEffect::Teleport, 0.1));
        // Cooldown is managed by handle_active_skill_event, so we don't set it here
        teleport_state.timer.reset();
        teleport_state.timer.tick(time.delta());
    }

    let player_pos = player_pos.translation();
    if teleport_state.timer.just_finished() {
        teleport_state.timer.reset();
        let direction = skill_aim_direction(move_direction.0, aim.facing_dir, facing.get_dir_vec());
        if direction == Vec2::ZERO {
            return;
        }
        let power_mult = skill_power_multiplier(
            skill_power,
            blessings.get_skill_power_bonus_with_majors(majors),
        );
        let base_distance = 4.5 * TILE_SIZE.x;
        let distance = direction * base_distance;
        let intended_tile = world_pos_to_tile_pos(player_pos.truncate() + distance);
        let Some(dest_tile) = resolve_teleport_destination_tile(
            intended_tile,
            player_pos.truncate(),
            &game,
            &proto_param,
        ) else {
            return;
        };

        let dest_center = tile_pos_to_world_pos(dest_tile, false)
            + Vec2::new(TILE_SIZE.x * 0.5, TILE_SIZE.y * 0.5);
        let from_2d = player_pos.truncate();
        let to_dest = dest_center - from_2d;
        let shock_dmg =
            (dmg.0 as f32 * power_mult * attack_damage_multiplier(TELEPORT_SHOCK_ATTACK_PERCENT))
                as i32;
        let shock_midpoint = from_2d + to_dest * 0.5;
        if let Some(shock_e) = commands.spawn_projectile_from_proto(
            Projectile::TeleportLightning,
            &proto_param,
            shock_midpoint,
            direction,
            false,
            &proto_param.asset_server,
            1.0,
        ) {
            commands.entity(shock_e).insert((
                Transform {
                    translation: shock_midpoint.extend(0.),
                    rotation: Quat::from_rotation_z(direction.y.atan2(direction.x)),
                    scale: Vec3::ONE,
                    ..default()
                },
                Attack(shock_dmg),
                AnimVisualCategory::Skill,
                FromActiveSkill,
            ));
        }
        commands.spawn(SoundSpawner::new(AudioSoundEffect::TeleportShock, 0.2));

        if skills.has(Heirloom::TeleportManaRegen) {
            commands.entity(e).insert(JustTeleported);
        }

        // Keep Shadow Step trail: clearing would wipe the pre-teleport samples
        // and make immediate Recall only retrace the landing tile.
        move_player.write(MovePlayerEvent {
            pos: dest_tile,
            clear_recall_history: false,
        });
    }

    if teleport_state.timer.fraction() != 0. {
        teleport_state.timer.tick(time.delta());
    }

    if teleport_state.timer.fraction() != 0. && teleport_state.timer.fraction() < 1. {
        move_direction.0 = Vec2::ZERO;
        kcc.translation = Some(Vec2::new(move_direction.0.x, move_direction.0.y));
    }
}

pub fn tick_just_teleported(
    mut teleported: Query<(Entity, &mut TeleportState)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut timer) in teleported.iter_mut() {
        timer.just_teleported_timer.tick(time.delta());
        if timer.just_teleported_timer.is_finished() {
            commands.entity(e).remove::<JustTeleported>();
        }
    }
}

fn spawn_aseprite_explosion_hitbox(
    commands: &mut Commands,
    pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
    handle: Handle<Aseprite>,
    tag: &str,
    duration: f32,
    base_radius: f32,
    projectile: Projectile,
    extra_components: Vec<DeferredComponent>,
) {
    // NOTE: do NOT pre-scale the collider radius by `size_multiplier`. The entity's
    // Transform scale below is applied to the collider by Rapier, so multiplying the
    // radius here as well would scale the hitbox twice (it would grow ~size^2).
    spawn_deferred_aseprite_collider(
        commands,
        Transform::from_translation(pos).with_scale(Vec3::splat(size_multiplier)),
        duration,
        dmg,
        Collider::capsule(Vec2::ZERO, Vec2::ZERO, base_radius),
        handle,
        tag,
        false,
        projectile,
        extra_components,
        None,
    );
}

pub fn spawn_ice_explosion_hitbox(
    commands: &mut Commands,
    graphics: &Graphics,
    pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
) {
    spawn_aseprite_explosion_hitbox(
        commands,
        pos,
        dmg,
        size_multiplier,
        graphics.ice_explosion_ase.as_ref().unwrap().clone(),
        IceExplosion::tags::ICE_EXPLOSION,
        // Fallback lifetime; `DoneAnimation` despawns when the one-shot clip finishes.
        1.5,
        26.0,
        Projectile::IceExplosionAOE,
        vec![DeferredComponent::IceExplosionDmg],
    );
    // Sound is now handled by the caller to batch multiple explosions
}

pub fn spawn_small_explosion_hitbox(
    commands: &mut Commands,
    graphics: &Graphics,
    pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
) {
    spawn_aseprite_explosion_hitbox(
        commands,
        pos,
        dmg,
        size_multiplier,
        graphics.small_explosion_ase.as_ref().unwrap().clone(),
        SmallExplosion::tags::EXPLOSION,
        1.0,
        13.0,
        Projectile::SmallExplosionAOE,
        vec![],
    );
}
