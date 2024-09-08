use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_aseprite::anim::AsepriteAnimation;
use bevy_rapier2d::prelude::KinematicCharacterController;

use crate::{
    animations::player_sprite::PlayerAnimation,
    attributes::Attack,
    combat_helpers::spawn_temp_collider,
    inputs::MovementVector,
    item::WorldObject,
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::{MovePlayerEvent, Player, PlayerSkills, Skill};

#[derive(Component)]
pub struct JustTeleported(pub Timer);
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
        ),
        With<Player>,
    >,
    key_input: ResMut<Input<KeyCode>>,
    mut game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    mut teleported: Local<bool>,
) {
    let (e, player_pos, skills, mut move_direction, dmg, aseprite, mut kcc) = player.single_mut();

    let player = game.player_mut();
    if skills.has(Skill::Teleport)
        && player.player_dash_cooldown.finished()
        && key_input.just_pressed(KeyCode::Space)
    {
        commands.entity(e).insert(PlayerAnimation::Teleport);
        player.player_dash_cooldown.reset();
    }

    if move_direction.0.length() != 0. && aseprite.current_frame() == 136 && !*teleported {
        *teleported = true;
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
            spawn_temp_collider(
                &mut commands,
                Transform::from_translation(Vec3::new(
                    player_pos.x + (distance.x / 2.),
                    player_pos.y + (distance.y / 2.),
                    0.,
                ))
                .with_rotation(Quat::from_rotation_z(angle)),
                Vec2::new(16., 3. * TILE_SIZE.x),
                0.5,
                dmg.0 / 5,
            )
        }

        if skills.has(Skill::TeleportManaRegen) {
            commands
                .entity(e)
                .insert(JustTeleported(Timer::from_seconds(0.7, TimerMode::Once)));
        }

        move_player.send(MovePlayerEvent { pos });
    }
    if aseprite.current_frame() >= 129 && aseprite.current_frame() <= 135 {
        move_direction.0 = Vec2::ZERO;
        kcc.translation = Some(Vec2::new(move_direction.0.x, move_direction.0.y));
    } else if aseprite.current_frame() == 137 {
        *teleported = false;
    }
}

pub fn tick_just_teleported(
    mut teleported: Query<(Entity, &mut JustTeleported)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (e, mut timer) in teleported.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.entity(e).remove::<JustTeleported>();
        }
    }
}
