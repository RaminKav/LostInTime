use bevy::{prelude::*, transform::commands};
use serde::{Deserialize, Serialize};

use crate::{
    animations::AttackEvent,
    audio::{AudioSoundEffect, SoundSpawner},
    item::projectile::{Projectile, RangedAttackEvent},
    player::{
        skills::{PlayerSkills, Skill},
        Player,
    },
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
    mut player: Query<(&PlayerSkills, &Attack), With<Player>>,
    game: GameParam,
    mut commands: Commands,
) {
    let (skills, dmg) = player.single_mut();

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
            commands.spawn(SoundSpawner::new(AudioSoundEffect::AirWaveAttack, 0.4));
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
