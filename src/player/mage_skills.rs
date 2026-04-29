use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController};

use crate::{
    animations::player_sprite::PlayerAnimation,
    assets::Graphics,
    attributes::{attribute_helpers::skill_power_multiplier, Attack, SkillPower},
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    combat_helpers::{spawn_deferred_aseprite_collider, spawn_temp_collider, DeferredComponent},
    inputs::MovementVector,
    item::{projectile::Projectile, WorldObject},
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::{
    skills::{
        active_skill_scaling::{attack_damage_multiplier, TELEPORT_SHOCK_ATTACK_PERCENT},
        ActiveSkill,
    },
    ActiveSkillUsedEvent, Heirloom, MovePlayerEvent, Player, PlayerSkills,
};

aseprite!(pub IceExplosion, "textures/effects/IceExplosion.aseprite");
aseprite!(pub Electricity, "textures/effects/Electricity.aseprite");
aseprite!(pub IceFloor, "textures/effects/IceFloor.aseprite");
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
pub struct TeleportShockDmg;

#[derive(Component)]
pub struct IceExplosionDmg;

pub fn handle_teleport(
    mut active_skill_events: EventReader<ActiveSkillUsedEvent>,
    mut move_player: EventWriter<MovePlayerEvent>,
    mut player: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &mut MovementVector,
            &Attack,
            &SkillPower,
            &AsepriteAnimation,
            &mut KinematicCharacterController,
            &mut TeleportState,
            &OwnedBlessings,
        ),
        (With<Player>, With<TeleportState>),
    >,
    game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    time: Res<Time>,
) {
    let Ok((
        e,
        player_pos,
        skills,
        mut move_direction,
        dmg,
        skill_power,
        aseprite,
        mut kcc,
        mut teleport_state,
        blessings,
    )) = player.get_single_mut()
    else {
        return;
    };

    // Check if we received a teleport activation event
    // The cooldown check is already done in dispatch_active_skill_events, so we just need to check animation timer
    let mut should_activate = false;
    if let Some(teleport_slot) = skills.has_active_skill(ActiveSkill::Teleport) {
        for ev in active_skill_events.iter() {
            if ev.slot == teleport_slot {
                // Only check if animation timer is ready (to prevent spamming)
                if teleport_state.timer.percent() == 0. || teleport_state.timer.percent() >= 1. {
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
    if move_direction.0.length() != 0. && teleport_state.timer.just_finished() {
        teleport_state.timer.reset();
        let direction = move_direction.0.normalize();
        let power_mult = skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());
        let base_distance = 4. * TILE_SIZE.x;
        let distance = direction * base_distance;
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

        let angle = f32::atan2(direction.y, direction.x) - PI / 2.;
        let shock_dmg = (dmg.0 as f32
            * power_mult
            * attack_damage_multiplier(TELEPORT_SHOCK_ATTACK_PERCENT))
            as i32;
        let shock_e = spawn_temp_collider(
            &mut commands,
            Transform::from_translation(Vec3::new(
                player_pos.x + (distance.x / 2.),
                player_pos.y + (distance.y / 2.),
                0.,
            ))
            .with_rotation(Quat::from_rotation_z(angle)),
            0.5,
            shock_dmg,
            Collider::cuboid(8., 1.5 * TILE_SIZE.x),
            Projectile::TeleportShock,
        );
        commands.entity(shock_e).insert(TeleportShockDmg);
        commands.spawn(SoundSpawner::new(AudioSoundEffect::TeleportShock, 0.2));

        if skills.has(Heirloom::TeleportManaRegen) {
            commands.entity(e).insert(JustTeleported);
        }

        move_player.send(MovePlayerEvent { pos });
    }

    if teleport_state.timer.percent() != 0. {
        teleport_state.timer.tick(time.delta());
    }

    if teleport_state.timer.percent() != 0. && teleport_state.timer.percent() < 1. {
        move_direction.0 = Vec2::ZERO;
        kcc.translation = Some(Vec2::new(move_direction.0.x, move_direction.0.y));
    } else if aseprite.just_finished() {
        teleport_state.timer.reset();
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


pub fn spawn_ice_explosion_hitbox(
    commands: &mut Commands,
    graphics: &Graphics,
    pos: Vec3,
    dmg: i32,
    size_multiplier: f32,
) {
    // Use default animation to ensure it starts at frame 0
    let anim = AsepriteAnimation::default();

    // Scale the collider radius by the size multiplier
    let base_radius = 26.0;
    let scaled_radius = base_radius * size_multiplier;

    // Queue deferred spawn - actual entity will be created in PreUpdate
    spawn_deferred_aseprite_collider(
        commands,
        Transform::from_translation(pos).with_scale(Vec3::splat(size_multiplier)),
        10.5,
        dmg,
        Collider::capsule(Vec2::ZERO, Vec2::ZERO, scaled_radius),
        graphics.ice_explosion_ase.as_ref().unwrap().clone(),
        anim,
        false,
        Projectile::IceExplosionAOE,
        vec![DeferredComponent::IceExplosionDmg],
        None, // No parent
    );
    // Sound is now handled by the caller to batch multiple explosions
}
