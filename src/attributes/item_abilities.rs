use std::f32::consts::PI;

use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    animations::AttackEvent,
    combat_helpers::spawn_temp_collider,
    inputs::MovementVector,
    inventory::ItemStack,
    item::projectile::{Projectile, RangedAttackEvent},
    player::{
        skills::{PlayerSkills, Skill},
        MovePlayerEvent, Player,
    },
    proto::proto_param::ProtoParam,
    world::{world_helpers::world_pos_to_tile_pos, TILE_SIZE},
    GameParam,
};

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
const ITEM_ABILITY_CHANCE: u32 = 25;
pub fn add_ability_to_item_drops(stack: &mut ItemStack) {
    let mut rng = rand::thread_rng();
    let chance = rng.gen_range(0..100);
    if chance <= ITEM_ABILITY_CHANCE {
        let ability = match rng.gen_range(0..3) {
            0 => Some(ItemAbility::Arc(rng.gen_range(2..5))),
            1 => Some(ItemAbility::FireAttack(rng.gen_range(2..5))),
            2 => Some(ItemAbility::Teleport(
                rng.gen_range(2..5) as f32 * TILE_SIZE.x,
            )),
            _ => None,
        };
        stack.metadata.item_ability = ability;
    }
}

pub fn handle_item_abilitiy_on_attack(
    mut attacks: EventReader<AttackEvent>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut move_player: EventWriter<MovePlayerEvent>,
    player: Query<(&GlobalTransform, &PlayerSkills, &MovementVector), With<Player>>,
    key_input: ResMut<Input<KeyCode>>,
    mut game: GameParam,
    proto_param: ProtoParam,
    time: Res<Time>,
    mut commands: Commands,
) {
    let (player_pos, skills, move_direction) = player.single();
    let Some(main_hand) = game.player().main_hand_slot else {
        return;
    };
    for attack in attacks.iter() {
        if skills.get(Skill::WaveAttack) && Skill::WaveAttack.is_obj_valid(main_hand.get_obj()) {
            ranged_attack_event.send(RangedAttackEvent {
                projectile: Projectile::Arc,
                direction: attack.direction,
                from_enemy: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: Some(3),
                pos_override: None,
            });
        }
        if skills.get(Skill::FireDamage) && Skill::FireDamage.is_obj_valid(main_hand.get_obj()) {
            ranged_attack_event.send(RangedAttackEvent {
                projectile: Projectile::FireAttack,
                direction: attack.direction,
                from_enemy: None,
                is_followup_proj: true,
                mana_cost: None,
                dmg_override: Some(4),
                pos_override: None,
            });
        }
    }

    let player = game.player_mut();
    if skills.get(Skill::Teleport)
        && player.player_dash_cooldown.tick(time.delta()).finished()
        && key_input.just_pressed(KeyCode::Space)
    {
        player.player_dash_cooldown.reset();
        let player_pos = player_pos.translation();
        let direction = move_direction.0.normalize();
        let distance = direction * 3. * TILE_SIZE.x;
        let pos = world_pos_to_tile_pos(player_pos.truncate() + distance);
        if let Some((_, obj)) = game.get_obj_entity_at_tile(pos, &proto_param) {
            if obj.is_tree() || obj.is_wall() {
                return;
            }
        }

        if skills.get(Skill::TeleportShock) {
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
                3,
            )
        }

        move_player.send(MovePlayerEvent { pos });
    }
}
