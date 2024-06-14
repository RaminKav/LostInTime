use bevy::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    animations::AttackEvent,
    inventory::ItemStack,
    item::{
        projectile::{Projectile, RangedAttackEvent},
        MainHand,
    },
};

#[derive(Debug, PartialEq, Reflect, FromReflect, Clone, Serialize, Deserialize)]
#[reflect(Default)]
pub enum ItemAbility {
    Arc(i32),
    FireAttack(i32),
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
        let ability = match rng.gen_range(0..2) {
            0 => Some(ItemAbility::Arc(rng.gen_range(2..5))),
            1 => Some(ItemAbility::FireAttack(rng.gen_range(2..5))),
            _ => None,
        };
        stack.metadata.item_ability = ability;
    }
}

pub fn handle_item_abilitiy_on_attack(
    mut attacks: EventReader<AttackEvent>,
    weapon: Query<&ItemStack, With<MainHand>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
) {
    for attack in attacks.iter() {
        if let Ok(weapon_stack) = weapon.get_single() {
            match weapon_stack.metadata.item_ability {
                Some(ItemAbility::Arc(dmg)) => {
                    ranged_attack_event.send(RangedAttackEvent {
                        projectile: Projectile::Arc,
                        direction: attack.direction,
                        from_enemy: None,
                        is_followup_proj: true,
                        mana_cost: None,
                        dmg_override: Some(dmg),
                    });
                }
                Some(ItemAbility::FireAttack(dmg)) => ranged_attack_event.send(RangedAttackEvent {
                    projectile: Projectile::FireAttack,
                    direction: attack.direction,
                    from_enemy: None,
                    is_followup_proj: true,
                    mana_cost: None,
                    dmg_override: Some(dmg),
                }),
                None => {}
            }
        }
    }
}
