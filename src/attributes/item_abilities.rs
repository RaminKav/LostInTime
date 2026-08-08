use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    animations::AttackEvent,
    attributes::CurrentMana,
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{
        HeirloomManaOverclock, MajorBlessing, OwnedMajorBlessings, overclock_mana_cost,
    },
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
    mut player: Query<
        (
            &PlayerSkills,
            &Attack,
            &mut CurrentMana,
            &OwnedMajorBlessings,
            Option<&mut HeirloomManaOverclock>,
        ),
        With<Player>,
    >,
    mut game: GameParam,
    mut commands: Commands,
) {
    let Ok((skills, dmg, mut current_mana, majors, mut overclock)) = player.single_mut() else {
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
            let base_mana_cost = (Heirloom::WaveAttack.get_mana_cost() as f32
                * if skills.has(Heirloom::DiscountMP) {
                    0.75
                } else {
                    1.
                }) as i32;
            let mana_cost = overclock_mana_cost(
                majors,
                overclock.as_deref_mut(),
                base_mana_cost,
                Some(&mut game.blessing_trigger_counts),
            );
            if mana_cost == 0 || current_mana.0 >= mana_cost {
                if mana_cost > 0 {
                    current_mana.0 -= mana_cost;
                    game.heirloom_trigger_counts
                        .record_mana(Heirloom::WaveAttack, mana_cost);
                }
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
                if majors.has(MajorBlessing::WeaponHeirloomDoubleTrigger) {
                    game.blessing_trigger_counts
                        .increment(MajorBlessing::WeaponHeirloomDoubleTrigger);
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
                        spawn_delay: 0.1
                            + crate::player::melee_skills::HEIRLOOM_EXTRA_CAST_DELAY,
                    });
                }
                commands.spawn(SoundSpawner::new(AudioSoundEffect::AirWaveAttack, 0.2));
            }
        }
    }
}
