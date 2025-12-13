use std::cmp::max;

use bevy::{
    asset::AssetServer, math::Vec3, prelude::Commands, render::view::RenderLayers,
    transform::components::Transform,
};
use bevy_aseprite::{anim::AsepriteAnimation, AsepriteBundle};
use bevy_proto::custom::VisibilityBundle;
use rand::{rngs::ThreadRng, Rng};

use crate::{
    animations::DoneAnimation,
    attributes::{
        AttributeModifier, ItemAttributes, ItemRarity, RarityGlows, RawItemBaseAttributes,
        RawItemBonusAttributes,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    inventory::ItemStack,
    item::EquipmentType,
    proto::proto_param::ProtoParam,
};
pub fn create_new_random_item_stack_with_attributes(
    stack: &ItemStack,
    proto: &ProtoParam,
    commands: &mut Commands,
    loot_bonus: i32,
    play_audio: bool,
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

    let rarity = get_rarity_rng(rand::thread_rng(), loot_bonus);

    build_item_stack_with_parsed_attributes(
        stack,
        raw_base_att,
        raw_bonus_att_option,
        rarity,
        eqp_type,
        stack.metadata.level,
        commands,
        play_audio,
        proto,
    )
}

pub fn reroll_item_bonus_attributes(stack: &ItemStack, proto: &ProtoParam) -> ItemStack {
    let level = stack.metadata.level.unwrap_or(1);
    let raw_bonus_att_option = proto.get_component::<RawItemBonusAttributes, _>(stack.obj_type);
    let Some(eqp_type) = proto.get_component::<EquipmentType, _>(stack.obj_type) else {
        return stack.clone();
    };

    let mut rng = rand::thread_rng();
    let rarity_rng = rng.gen_range(0..=4);
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

    new_stack = levelup_item_stats(&new_stack, level, proto, true);

    // Re-select inventory buff line after rerolling
    let raw_base_att = proto
        .get_component::<RawItemBaseAttributes, _>(stack.obj_type)
        .unwrap();
    new_stack.metadata.inventory_buff_line_index = select_random_inventory_buff_line(
        &new_stack,
        raw_base_att,
        raw_bonus_att_option,
        &eqp_type,
        proto,
    );

    new_stack
}

pub fn get_rarity_rng(mut rng: ThreadRng, loot_bonus: i32) -> ItemRarity {
    // Base probabilities: Common 48%, Uncommon 36%, Rare 12%, Legendary 4%
    // Loot bonus increases higher rarity chances
    // Formula: each point of loot increases higher rarity chances by shifting thresholds
    // Each point of loot: +0.5% legendary, +0.4% rare, +0.3% uncommon, -1.2% common
    // Since we roll 0-24 (25 values), each 1% = 0.25 threshold points

    let loot_bonus_f = loot_bonus as f32;
    // Calculate adjusted thresholds (higher threshold = more chance for that rarity)
    // Legendary: base 1 (4%), increases by 0.5% per loot = +0.125 threshold per loot
    let legendary_threshold = (98.5 - loot_bonus_f * 0.1).max(0.0);
    // Rare: base 4 (12%), increases by 0.4% per loot = +0.1 threshold per loot
    let rare_threshold = (88.0 + loot_bonus_f * 0.2).min(99.0);
    // Uncommon: base 13 (36%), increases by 0.3% per loot = +0.075 threshold per loot
    let uncommon_threshold = (68.0 + loot_bonus_f * 0.1).min(99.0);

    let rarity_rng = rng.gen_range(0_f32..100_f32);
    if rarity_rng >= legendary_threshold {
        ItemRarity::Legendary
    } else if rarity_rng >= rare_threshold {
        ItemRarity::Rare
    } else if rarity_rng >= uncommon_threshold {
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
    play_audio: bool,
    proto: &ProtoParam,
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
    if play_audio && rarity == ItemRarity::Legendary {
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.15));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop2, 0.3));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.3).with_delay(0.55));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.15).with_delay(2.3));
    }
    if play_audio && rarity == ItemRarity::Rare {
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop1, 0.15));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop2, 0.3));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop1, 0.2).with_delay(0.4));
    }
    for _ in 0..(level - 1) {
        new_stack = levelup_item_stats(&new_stack, 1, &proto, false);
    }

    // Select a random bonus attribute line for inventory buff
    new_stack.metadata.inventory_buff_line_index = select_random_inventory_buff_line(
        &new_stack,
        raw_base_att,
        raw_bonus_att_option,
        &equip_type,
        proto,
    );

    new_stack
}

/// Selects a random bonus attribute line index for inventory buff
/// Returns None if there are no bonus attributes
pub fn select_random_inventory_buff_line(
    stack: &ItemStack,
    raw_base_att: &RawItemBaseAttributes,
    raw_bonus_att_option: Option<&RawItemBonusAttributes>,
    equip_type: &EquipmentType,
    _proto: &ProtoParam,
) -> Option<usize> {
    // Get tooltips to find which lines are bonus attributes
    let (tooltips, _, _) = stack.attributes.get_tooltips(
        stack.rarity.clone(),
        Some(raw_base_att),
        raw_bonus_att_option,
        stack.metadata.level.unwrap_or(1) as i32,
        stack.obj_type,
        equip_type,
    );

    // Identify base attributes based on equipment type
    // These should NOT be selectable as inventory buffs
    let base_attributes: Vec<&str> = if equip_type.is_weapon() || equip_type.is_tool() {
        // For weapons/tools: exclude Attack, Hits/s, and Attack Speed (base stats)
        vec!["Attack", "Hits/s", "% Attack Speed"]
    } else if equip_type.is_equipment() && !equip_type.is_accessory() {
        // For armor: exclude HP and Defence (base stats)
        vec!["HP", "Defence"]
    } else {
        // Accessories: no base attributes, everything is bonus (including Attack Speed)
        vec![]
    };

    // Find bonus attribute line indices (skip name line at index 0)
    let mut bonus_line_indices = Vec::new();
    for (i, (name, _, _)) in tooltips.iter().enumerate() {
        // Skip the name line (index 0)
        if i == 0 {
            continue;
        }
        // Check if this is a bonus attribute (not a base attribute)
        let is_base = base_attributes.iter().any(|base| name.contains(base));
        if !is_base && !name.is_empty() {
            bonus_line_indices.push(i);
        }
    }

    // Select a random bonus line index
    if bonus_line_indices.is_empty() {
        None
    } else {
        let mut rng = rand::thread_rng();
        Some(bonus_line_indices[rng.gen_range(0..bonus_line_indices.len())])
    }
}

pub fn levelup_item_stats(
    stack: &ItemStack,
    level: u8,
    proto: &ProtoParam,
    skip_main_attributes: bool,
) -> ItemStack {
    let mut stack = stack.clone();
    for _ in 0..level {
        let mut modifiers: Vec<(String, i32)> = vec![];

        let rarity = stack.rarity.clone();
        let num_upgrades = match rarity {
            ItemRarity::Legendary => 4,
            ItemRarity::Rare => 3,
            ItemRarity::Uncommon => 2,
            _ => 1,
        };
        if let Some(eqp_type) = stack.obj_type.get_equip_type(proto) {
            let mut filter: Vec<&str> = vec![];
            if eqp_type.is_weapon() || eqp_type.is_tool() {
                if !skip_main_attributes {
                    modifiers.push(("attack".to_owned(), 1));
                }
                filter.push("attack");
            } else if eqp_type.is_equipment() && !eqp_type.is_accessory() {
                if !skip_main_attributes {
                    modifiers.push(("health".to_owned(), 2));
                    modifiers.push(("defence".to_owned(), 1));
                }
                filter.push("health");
                filter.push("defence");
            }
            for _ in 0..num_upgrades {
                if let Some(bonus_mod) = stack
                    .attributes
                    .get_random_existing_bonus_attribute_string(&filter)
                {
                    modifiers.push((bonus_mod, 1));
                }
            }
        }
        for (modifier, delta) in modifiers {
            stack = stack.get_copy_with_modified_attributes(AttributeModifier { modifier, delta });
        }
    }
    stack.clone()
}

pub fn spawn_rarity_animation(
    new_rarity: ItemRarity,
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
) {
    if new_rarity == ItemRarity::Legendary {
        // Sound effect
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.15));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop2, 0.3));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.3).with_delay(0.55));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::LegendaryDrop1, 0.15).with_delay(2.3));

        // glow/shake effects
        commands
            .spawn(AsepriteBundle {
                aseprite: asset_server.load(RarityGlows::PATH),
                animation: AsepriteAnimation::from(RarityGlows::tags::RARER),
                transform: Transform::from_translation(pos),
                ..Default::default()
            })
            .insert(VisibilityBundle::default())
            .insert(DoneAnimation)
            .insert(RenderLayers::from_layers(&[3]));
    } else if new_rarity == ItemRarity::Rare {
        commands
            .spawn(AsepriteBundle {
                aseprite: asset_server.load(RarityGlows::PATH),
                animation: AsepriteAnimation::from(RarityGlows::tags::RARE),
                transform: Transform::from_translation(pos),
                ..Default::default()
            })
            .insert(VisibilityBundle::default())
            .insert(DoneAnimation)
            .insert(RenderLayers::from_layers(&[3]));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop1, 0.15));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop2, 0.3));
        commands.spawn(SoundSpawner::new(AudioSoundEffect::RareDrop1, 0.2).with_delay(0.4));
    }
}
