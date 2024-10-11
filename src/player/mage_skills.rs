use std::{cmp::min, f32::consts::PI, time::Duration};

use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};

use crate::{
    animations::player_sprite::PlayerAnimation,
    attributes::Attack,
    audio::{AudioSoundEffect, SoundSpawner},
    combat_helpers::{spawn_one_time_aseprite_collider, spawn_temp_collider},
    get_active_skill_keybind,
    inputs::MovementVector,
    item::{projectile::Projectile, WorldObject},
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::{ActiveSkillUsedEvent, MovePlayerEvent, Player, PlayerSkills, Skill};

aseprite!(pub IceExplosion, "textures/effects/IceExplosion.aseprite");
aseprite!(pub Electricity, "textures/effects/Electricity.aseprite");
aseprite!(pub IceFloor, "textures/effects/IceFloor.aseprite");
#[derive(Component)]
pub struct JustTeleported;
#[derive(Component)]
pub struct TeleportState {
    pub just_teleported_timer: Timer,
    pub cooldown_timer: Timer,
    pub count: u32,
    pub max_count: u32,
    pub timer: Timer,
    pub second_explosion_timer: Timer,
}

#[derive(Component)]
pub struct TeleportShockDmg;

#[derive(Component)]
pub struct IceExplosionDmg;

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
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
) {
    let Ok((e, player_pos, skills, mut move_direction, dmg, aseprite, mut kcc, mut teleport_state)) =
        player.get_single_mut()
    else {
        return;
    };
    if let Some(teleport_slot) = skills.has_active_skill(Skill::Teleport) {
        if teleport_state.count > 0
            && key_input.just_pressed(get_active_skill_keybind(teleport_slot))
            && (teleport_state.timer.percent() == 0. || teleport_state.timer.percent() >= 1.)
        {
            active_skill_event.send(ActiveSkillUsedEvent {
                slot: teleport_slot,
                cooldown: teleport_state.cooldown_timer.duration().as_secs_f32(),
            });
            commands.entity(e).insert(PlayerAnimation::Teleport);
            commands.spawn(SoundSpawner::new(AudioSoundEffect::Teleport, 0.25));
            teleport_state.count -= 1;
            teleport_state.timer.reset();
            teleport_state.timer.tick(time.delta());
            teleport_state.second_explosion_timer.reset();
            teleport_state.second_explosion_timer.tick(time.delta());
        }
    }

    let player_pos = player_pos.translation();
    if skills.has(Skill::TeleportIceAoEEnd) && teleport_state.second_explosion_timer.just_finished()
    {
        teleport_state.second_explosion_timer.reset();
        spawn_ice_explosion_hitbox(&mut commands, &asset_server, player_pos, dmg.0 / 4);
    }
    if move_direction.0.length() != 0. && teleport_state.timer.just_finished() {
        teleport_state.timer.reset();
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
                Projectile::TeleportShock,
            );
            commands.entity(shock_e).insert(TeleportShockDmg);
            commands.spawn(SoundSpawner::new(AudioSoundEffect::TeleportShock, 0.3));
        }
        if skills.has(Skill::TeleportIceAoE) {
            spawn_ice_explosion_hitbox(&mut commands, &asset_server, player_pos, dmg.0 / 4);
        }

        if skills.has(Skill::TeleportManaRegen) {
            commands.entity(e).insert(JustTeleported);
        }

        move_player.send(MovePlayerEvent { pos });
    }

    if teleport_state.timer.percent() != 0. {
        teleport_state.timer.tick(time.delta());
    }
    if teleport_state.second_explosion_timer.percent() != 0. {
        teleport_state.second_explosion_timer.tick(time.delta());
    }

    if teleport_state.timer.percent() != 0. && teleport_state.timer.percent() < 1. {
        move_direction.0 = Vec2::ZERO;
        kcc.translation = Some(Vec2::new(move_direction.0.x, move_direction.0.y));
    } else if aseprite.just_finished() {
        teleport_state.timer.reset();
        teleport_state.second_explosion_timer.reset();
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
        if teleport_state.count < teleport_state.max_count {
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
        }

        if teleport_state.cooldown_timer.finished()
            && teleport_state.count < teleport_state.max_count
        {
            teleport_state.count = min(teleport_state.max_count, teleport_state.count + 1);
            teleport_state.cooldown_timer.reset();
        }
    }
}

pub fn spawn_ice_explosion_hitbox(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    dmg: i32,
) {
    let mut anim = AsepriteAnimation::from(IceExplosion::tags::ICE_EXPLOSION);
    anim.current_frame = 0;
    let c = spawn_one_time_aseprite_collider(
        commands,
        Transform::from_translation(pos),
        10.5,
        dmg,
        Collider::capsule(Vec2::ZERO, Vec2::ZERO, 28.),
        asset_server.load::<Aseprite, _>(IceExplosion::PATH),
        anim,
        false,
        Projectile::FireExplosionAOE,
    );

    commands.entity(c).insert(IceExplosionDmg);
    commands.spawn(SoundSpawner::new(AudioSoundEffect::IceExplosion, 0.4));
}
