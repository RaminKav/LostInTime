use rand::Rng;

use crate::{
    attributes::{ItemAttributes, ItemRarity, RawItemBaseAttributes, RawItemBonusAttributes},
    inventory::ItemStack,
    item::EquipmentType,
    proto::proto_param::ProtoParam,
};
pub fn create_new_random_item_stack_with_attributes(
    stack: &ItemStack,
    proto: &ProtoParam,
) -> ItemStack {
    let Some(eqp_type) = proto.get_component::<EquipmentType, _>(stack.obj_type) else {
        return stack.clone();
    };
    let raw_bonus_att_option = proto.get_component::<RawItemBonusAttributes, _>(stack.obj_type);
    let raw_base_att = proto
        .get_component::<RawItemBaseAttributes, _>(stack.obj_type)
        .unwrap();
    let mut rng = rand::thread_rng();
    let rarity_rng = rng.gen_range(0..40);
    let rarity = if rarity_rng == 0 {
        ItemRarity::Legendary
    } else if rarity_rng < 4 {
        ItemRarity::Rare
    } else if rarity_rng < 13 {
        ItemRarity::Uncommon
    } else {
        ItemRarity::Common
    };
    let parsed_bonus_att = if let Some(raw_bonus_att) = raw_bonus_att_option {
        raw_bonus_att.into_item_attributes(rarity.clone(), eqp_type)
    } else {
        ItemAttributes::default()
    };
    let parsed_base_att = raw_base_att.into_item_attributes(stack.attributes.attack_cooldown);
    let mut final_att = parsed_bonus_att.combine(&parsed_base_att);
    final_att.max_durability = stack.attributes.max_durability;
    if final_att.max_durability > 0 {
        final_att.durability =
            rng.gen_range(final_att.max_durability / 10..final_att.max_durability);
    }
    let mut new_stack = stack.copy_with_attributes(&final_att);
    new_stack.rarity = rarity;
    new_stack
}

pub fn reroll_item_bonus_attributes(stack: &ItemStack, proto: &ProtoParam) -> ItemStack {
    let raw_bonus_att_option = proto.get_component::<RawItemBonusAttributes, _>(stack.obj_type);
    let Some(eqp_type) = proto.get_component::<EquipmentType, _>(stack.obj_type) else {
        return stack.clone();
    };

    let mut rng = rand::thread_rng();
    let rarity_rng = rng.gen_range(0..=10);
    let rarity = if rarity_rng <= 0 {
        stack.rarity.get_next_rarity()
    } else {
        stack.rarity.clone()
    };
    let parsed_bonus_att = if let Some(raw_bonus_att) = raw_bonus_att_option {
        raw_bonus_att.into_item_attributes(rarity.clone(), eqp_type)
    } else {
        ItemAttributes::default()
    };
    let mut final_att = parsed_bonus_att;

    final_att.max_durability = stack.attributes.max_durability;
    final_att.attack = stack.attributes.attack;
    final_att.attack_cooldown = stack.attributes.attack_cooldown;
    final_att.defence = stack.attributes.defence;
    final_att.health = stack.attributes.health;

    let mut new_stack = stack.copy_with_attributes(&final_att);
    new_stack.rarity = rarity;
    new_stack
}
