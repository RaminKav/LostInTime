use std::cmp::max;

use bevy::prelude::Commands;
use rand::{rngs::ThreadRng, Rng};

use crate::{
    attributes::{ItemAttributes, ItemRarity, RawItemBaseAttributes, RawItemBonusAttributes},
    audio::{AudioSoundEffect, SoundSpawner},
    inventory::ItemStack,
    item::EquipmentType,
    proto::proto_param::ProtoParam,
};
pub fn create_new_random_item_stack_with_attributes(
    stack: &ItemStack,
    proto: &ProtoParam,
    commands: &mut Commands,
) -> ItemStack {
    let Some(eqp_type) = proto.get_component::<EquipmentType, _>(stack.obj_type) else {
        let mut stack = stack.clone();
        stack.metadata = proto
            .get_item_data(stack.obj_type)
            .unwrap()
            .metadata
            .clone();
        return stack.clone();
    };

    let raw_bonus_att_option = proto.get_component::<RawItemBonusAttributes, _>(stack.obj_type);
    let raw_base_att = proto
        .get_component::<RawItemBaseAttributes, _>(stack.obj_type)
        .unwrap();

    let rarity = get_rarity_rng(rand::thread_rng());

    build_item_stack_with_parsed_attributes(
        stack,
        raw_base_att,
        raw_bonus_att_option,
        rarity,
        eqp_type,
        stack.metadata.level,
        commands,
    )
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

pub fn get_rarity_rng(mut rng: ThreadRng) -> ItemRarity {
    let rarity_rng = rng.gen_range(0..35);
    if rarity_rng == 0 {
        ItemRarity::Legendary
    } else if rarity_rng < 4 {
        ItemRarity::Rare
    } else if rarity_rng < 13 {
        ItemRarity::Uncommon
    } else {
        ItemRarity::Common
    }
}

pub fn build_item_stack_with_parsed_attributes(
    stack: &ItemStack,
    raw_base_att: &RawItemBaseAttributes,
    raw_bonus_att_option: Option<&RawItemBonusAttributes>,
    rarity: ItemRarity,
    equip_type: &EquipmentType,
    level_option: Option<u8>,
    commands: &mut Commands,
) -> ItemStack {
    let parsed_bonus_att = if let Some(raw_bonus_att) = raw_bonus_att_option {
        raw_bonus_att.into_item_attributes(rarity.clone(), equip_type)
    } else {
        ItemAttributes::default()
    };
    let parsed_base_att =
        raw_base_att.into_item_attributes(rarity.clone(), stack.attributes.attack_cooldown);
    let mut final_att = parsed_bonus_att.combine(&parsed_base_att);
    let mut level = 1;
    if let Some(item_level) = level_option {
        if item_level > 1 {
            if equip_type.is_weapon() || equip_type.is_tool() {
                let att_modifier = stack.obj_type.get_weapon_levelup_upgrade();
                final_att.attack = final_att.attack + (item_level - 1) as i32 * att_modifier;
            } else if equip_type.is_equipment() && !equip_type.is_accessory() {
                final_att.health = final_att.health + max(0, ((item_level * 2) - 1) as i32);
                final_att.defence = final_att.defence + max(0, (item_level - 1) as i32);
            }
        }
        level = item_level;
    }
    let mut new_stack = stack.copy_with_attributes(&final_att);
    new_stack.metadata.level = Some(level);
    new_stack.rarity = rarity.clone();
    if rarity == ItemRarity::Legendary {
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.3));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop2, 1.));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.5).with_delay(0.55));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.2).with_delay(2.3));
    }
    if rarity == ItemRarity::Rare {
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop1, 0.2));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop2, 0.7));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop1, 0.5).with_delay(0.4));
    }

    new_stack
}
