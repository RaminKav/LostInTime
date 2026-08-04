use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    animations::AttackEvent,
    attributes::CurrentMana,
    audio::{AudioSoundEffect, SoundSpawner},
    item::projectile::{Projectile, RangedAttackEvent},
    player::{
        skills::{Heirloom, PlayerSkills},
        Player,
    },
    GameParam,
};

use super::Attack;

#[derive(Debug, PartialEq, Reflect, Clone, Serialize, Deserialize)]
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
    mut attacks: MessageReader<AttackEvent>,
    mut ranged_attack_event: MessageWriter<RangedAttackEvent>,
    mut player: Query<(&PlayerSkills, &Attack, &mut CurrentMana), With<Player>>,
    mut game: GameParam,
    mut commands: Commands,
) {
    let Ok((skills, dmg, mut current_mana)) = player.single_mut() else {
        return;
    };
    let Some(_) = game.player().main_hand_slot else {
        return;
    };
    for attack in attacks.read() {
        let mut rng = rand::thread_rng();
        if skills.has(Heirloom::WaveAttack)
            && rng.gen_bool((skills.get_count(Heirloom::WaveAttack) as f64 * 0.33).clamp(0., 1.))
        {
            let mana_cost = Heirloom::WaveAttack.get_mana_cost();
            if current_mana.0 >= mana_cost {
                current_mana.0 -= mana_cost;
                game.heirloom_trigger_counts
                    .record_mana(Heirloom::WaveAttack, mana_cost);
                game.heirloom_trigger_counts.increment(Heirloom::WaveAttack);
                ranged_attack_event.write(RangedAttackEvent {
                    projectile: Projectile::Arc,
                    direction: attack.direction,
                    from_entity: None,
                    from_enemy: false,
                    is_followup_proj: true,
                    mana_cost: None,
                    mana_cost_heirloom: None,
                    dmg_override: Some(dmg.0),
                    pos_override: None,
                    spawn_delay: 0.1,
                });
                commands.spawn(SoundSpawner::new(AudioSoundEffect::AirWaveAttack, 0.2));
            }
        }
    }
}
