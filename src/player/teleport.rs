use std::{cmp::min, f32::consts::PI, time::Duration};

use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, Aseprite};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};

use crate::{
    animations::player_sprite::PlayerAnimation,
    attributes::Attack,
    combat_helpers::{spawn_one_time_aseprite_collider, spawn_temp_collider},
    inputs::MovementVector,
    item::WorldObject,
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::{melee_skills::OnHitAoe, MovePlayerEvent, Player, PlayerSkills, Skill};

#[derive(Component)]
pub struct JustTeleported;
#[derive(Component)]
pub struct TeleportState {
    pub just_teleported_timer: Timer,
    pub cooldown_timer: Timer,
    pub count: u32,
    pub max_count: u32,
    pub timer: Timer,
}

#[derive(Component)]
pub struct TeleportShockDmg;
// TODO
pub fn handle_teleport(
    mut move_player: EventWriter<MovePlayerEvent>,
    mut player: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &mut MovementVector,
            &Attack,
            &AsepriteAnimation,
            &mut KinematicCharacterController,
            &mut TeleportState,
        ),
        (With<Player>, With<TeleportState>),
    >,
    key_input: ResMut<Input<KeyCode>>,
    game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    mut ice_aoe_check: Local<bool>,
    time: Res<Time>,
    asset_server: Res<AssetServer>,
) {
    let Ok((e, player_pos, skills, mut move_direction, dmg, aseprite, mut kcc, mut teleport_state)) =
        player.get_single_mut()
    else {
        return;
    };

    if skills.has(Skill::Teleport)
        && teleport_state.count > 0
        && key_input.just_pressed(KeyCode::Space)
    {
        commands.entity(e).insert(PlayerAnimation::Teleport);
        teleport_state.count -= 1;
        teleport_state.timer.tick(time.delta());
    }

    if move_direction.0.length() != 0.
        && *ice_aoe_check == false
        && teleport_state.timer.percent() >= 0.5
    {
        *ice_aoe_check = true;
        let player_pos = player_pos.translation();
        let direction = move_direction.0.normalize();
        let distance = direction * 2.5 * TILE_SIZE.x;
        let pos = world_pos_to_tile_pos(player_pos.truncate() + distance);
        if let Some(tile_data) = game.get_tile_data(pos) {
            if tile_data.block_type.contains(&WorldObject::WaterTile) {
                return;
            }
        }
        if let Some((_, obj)) = game.get_obj_entity_at_tile(pos, &proto_param) {
            if obj.is_tree() || obj.is_wall() {
                return;
            }
        }

        if skills.has(Skill::TeleportShock) {
            let angle = f32::atan2(direction.y, direction.x) - PI / 2.;
            let shock_e = spawn_temp_collider(
                &mut commands,
                Transform::from_translation(Vec3::new(
                    player_pos.x + (distance.x / 2.),
                    player_pos.y + (distance.y / 2.),
                    0.,
                ))
                .with_rotation(Quat::from_rotation_z(angle)),
                0.5,
                dmg.0 / 5,
                Collider::cuboid(8., 1.5 * TILE_SIZE.x),
            );
            commands.entity(shock_e).insert(TeleportShockDmg);
        }
        if skills.has(Skill::TeleportIceAoe) {
            let hitbox_e = spawn_one_time_aseprite_collider(
                &mut commands,
                Transform::from_translation(Vec3::ZERO),
                10.5,
                dmg.0 / 4,
                Collider::capsule(Vec2::ZERO, Vec2::ZERO, 19.),
                asset_server.load::<Aseprite, _>(OnHitAoe::PATH),
                AsepriteAnimation::from(OnHitAoe::tags::AO_E),
                false,
            );
            commands.entity(hitbox_e).set_parent(e);
        }

        if skills.has(Skill::TeleportManaRegen) {
            commands.entity(e).insert(JustTeleported);
        }

        move_player.send(MovePlayerEvent { pos });
    }
    if teleport_state.timer.percent() != 0. {
        teleport_state.timer.tick(time.delta());
    }

    if aseprite.current_frame() >= 129 && aseprite.current_frame() <= 135 {
        move_direction.0 = Vec2::ZERO;
        kcc.translation = Some(Vec2::new(move_direction.0.x, move_direction.0.y));
    } else if aseprite.just_finished() {
        teleport_state.timer.reset();
        *ice_aoe_check = false;
    }
}

pub fn tick_just_teleported(
    mut teleported: Query<(Entity, &mut TeleportState)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut timer) in teleported.iter_mut() {
        timer.just_teleported_timer.tick(time.delta());
        if timer.just_teleported_timer.finished() {
            commands.entity(e).remove::<JustTeleported>();
        }
    }
}

pub fn tick_teleport_timer(
    time: Res<Time>,
    mut player_q: Query<(&PlayerSkills, &mut TeleportState)>,
) {
    if let Ok((skills, mut teleport_state)) = player_q.get_single_mut() {
        let d = time.delta();
        teleport_state
            .cooldown_timer
            .tick(if skills.has(Skill::TeleportCooldown) {
                Duration::new(
                    (d.as_secs() as f32 * 0.75) as u64,
                    (d.subsec_nanos() as f32 * 0.75) as u32,
                )
            } else {
                d
            });

        if teleport_state.cooldown_timer.finished()
            && teleport_state.count < teleport_state.max_count
        {
            teleport_state.count = min(teleport_state.max_count, teleport_state.count + 1);
            teleport_state.cooldown_timer.reset();
        }
    }
}
