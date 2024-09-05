use std::f32::consts::PI;

use bevy::prelude::*;
use bevy_aseprite::anim::AsepriteAnimation;
use bevy_rapier2d::prelude::KinematicCharacterController;
use serde::{Deserialize, Serialize};

use crate::{
    animations::{player_sprite::PlayerAnimation, AttackEvent},
    combat_helpers::spawn_temp_collider,
    inputs::MovementVector,
    item::{
        projectile::{Projectile, RangedAttackEvent},
        WorldObject,
    },
    player::{
        skills::{PlayerSkills, Skill},
        MovePlayerEvent, Player,
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

use super::Attack;

#[derive(Debug, PartialEq, Reflect, FromReflect, Clone, Serialize, Deserialize)]
#[reflect(Default)]
pub enum ItemAbility {
    Arc(i32),
    FireAttack(i32),
    Teleport(f32),
}
impl Default for ItemAbility {
    fn default() -> Self {
        ItemAbility::Arc(2)
    }
}

// items will have a chance to spawn with 1 ItemAbility.
// for now, it can be a fixed rate for all items, maybe 20%
// perhapse a 3rd item upgrade can add or override abilities on items
// when AttackEvent is fired, we match on enum and handle teh ability.

pub fn handle_item_abilitiy_on_attack(
    mut attacks: EventReader<AttackEvent>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
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
    time: Res<Time>,
    mut commands: Commands,
    mut teleported: Local<bool>,
) {
    let (e, player_pos, skills, mut move_direction, dmg, aseprite, mut kcc) = player.single_mut();

    let player = game.player_mut();
    if skills.has(Skill::Teleport)
        && player.player_dash_cooldown.tick(time.delta()).finished()
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
        info!(
            "Teleport {pos:?} {player_pos:?} | {:?} {:?}",
            player_pos.truncate() + distance,
            distance
        );

        move_player.send(MovePlayerEvent { pos });
    }
    if aseprite.current_frame() >= 129 && aseprite.current_frame() <= 135 {
        move_direction.0 = Vec2::ZERO;
        kcc.translation = Some(Vec2::new(move_direction.0.x, move_direction.0.y));
    } else if aseprite.current_frame() == 137 {
        *teleported = false;
    }
    let Some(main_hand) = game.player().main_hand_slot else {
        return;
    };
    for attack in attacks.iter() {
        if skills.has(Skill::WaveAttack) && Skill::WaveAttack.is_obj_valid(main_hand.get_obj()) {
            ranged_attack_event.send(RangedAttackEvent {
                projectile: Projectile::Arc,
                direction: attack.direction,
                from_enemy: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: Some(dmg.0 / 5),
                pos_override: None,
                spawn_delay: 0.1,
            });
        }
        if skills.has(Skill::FireDamage) && Skill::FireDamage.is_obj_valid(main_hand.get_obj()) {
            ranged_attack_event.send(RangedAttackEvent {
                projectile: Projectile::FireAttack,
                direction: attack.direction,
                from_enemy: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: Some(dmg.0 / 3),
                pos_override: None,
                spawn_delay: 0.1,
            });
        }
    }
}
