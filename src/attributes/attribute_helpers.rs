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
        RawItemBonusAttributes, SkillPower,
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
    let eqp_type_option = proto.get_component::<EquipmentType, _>(stack.obj_type);
    // Tools (axe, pickaxe) are passive inventory items and behave like normal item drops,
    // so skip the equipment attribute pipeline even though they have an EquipmentType.
    let is_tool_item = eqp_type_option.map_or(false, |e| e.is_tool());
    let Some(eqp_type) = eqp_type_option.filter(|_| !is_tool_item) else {
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
    let Some(eqp_type) = proto.get_component::<EquipmentType, _>(stack.obj_type) else {
        return stack.clone();
    };
    // Tools are passive inventory items with no stats; nothing to reroll.
    if eqp_type.is_tool() {
        return stack.clone();
    }
    let raw_base_att = proto
        .get_component::<RawItemBaseAttributes, _>(stack.obj_type)
        .unwrap();
    let raw_bonus_att_option = proto.get_component::<RawItemBonusAttributes, _>(stack.obj_type);

    let mut rng = rand::thread_rng();
    let rarity_rng = rng.gen_range(0..=4);
    let rarity = if rarity_rng <= 0 {
        stack.rarity.get_next_rarity()
    } else {
        stack.rarity.clone()
    };
    // Regenerate bonus attributes
    let mut bonus_stat_lines = Vec::new();
    let parsed_bonus_att = if let Some(raw_bonus_att) = raw_bonus_att_option {
        raw_bonus_att.into_item_attributes(rarity.clone(), eqp_type, Some(&mut bonus_stat_lines))
    } else {
        ItemAttributes::default()
    };
    let mut final_att = parsed_bonus_att;

    final_att.max_durability = stack.attributes.max_durability;
    final_att.attack = stack.attributes.attack;
    final_att.attack_cooldown = stack.attributes.attack_cooldown;
    final_att.defence = stack.attributes.defence;
    final_att.health = stack.attributes.health;
    if raw_base_att.speed.is_some() {
        final_att.speed = stack.attributes.speed;
    }

    let mut new_stack = stack.copy_with_attributes(&final_att);
    new_stack.rarity = rarity;
    new_stack.metadata.bonus_stat_lines = bonus_stat_lines;

    new_stack = levelup_item_stats(&new_stack, level, proto, true);

    // Re-select inventory buff line after rerolling
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
    let legendary_threshold = (99. - loot_bonus_f * 0.08).max(0.0);
    // Rare: base 4 (12%), increases by 0.4% per loot = +0.1 threshold per loot
    let rare_threshold = (88.0 + loot_bonus_f * 0.12).min(99.0);
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
    let mut bonus_stat_lines = Vec::new();
    let parsed_bonus_att = if let Some(raw_bonus_att) = raw_bonus_att_option {
        raw_bonus_att.into_item_attributes(rarity.clone(), equip_type, Some(&mut bonus_stat_lines))
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
    // Store bonus stat lines in metadata
    new_stack.metadata.bonus_stat_lines = bonus_stat_lines;
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
        new_stack = levelup_item_stats(&new_stack, 1, &proto, true);
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
/// The index is into stack.metadata.bonus_stat_lines
pub fn select_random_inventory_buff_line(
    stack: &ItemStack,
    raw_base_att: &RawItemBaseAttributes,
    _raw_bonus_att_option: Option<&RawItemBonusAttributes>,
    _equip_type: &EquipmentType,
    _proto: &ProtoParam,
) -> Option<usize> {
    if stack.obj_type.is_accessory() {
        return None;
    }
    // Build a set of attribute names that are actually base attributes
    // This matches the logic in get_tooltips_from_stat_lines
    let mut base_attribute_names = std::collections::HashSet::new();

    if raw_base_att.health.is_some() {
        base_attribute_names.insert("health".to_string());
    }
    if raw_base_att.defence.is_some() {
        base_attribute_names.insert("defence".to_string());
    }
    if raw_base_att.attack.is_some() {
        base_attribute_names.insert("attack".to_string());
    }
    if raw_base_att.speed.is_some() {
        base_attribute_names.insert("speed".to_string());
    }

    // Get all bonus stat lines that are not base attributes
    let bonus_stat_lines: Vec<(usize, &crate::item::BonusStatLine)> = stack
        .metadata
        .bonus_stat_lines
        .iter()
        .enumerate()
        .filter(|(_, stat_line)| !base_attribute_names.contains(&stat_line.attribute_name))
        .collect();

    // Select a random bonus line index
    if bonus_stat_lines.is_empty() {
        None
    } else {
        let mut rng = rand::thread_rng();
        let selected = rng.gen_range(0..bonus_stat_lines.len());
        Some(bonus_stat_lines[selected].0)
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
        let mut modifiers: Vec<(String, i32, Option<usize>)> = vec![];

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
                    modifiers.push(("attack".to_owned(), 1, None));
                }
                filter.push("attack");
            } else if eqp_type.is_equipment() && !eqp_type.is_accessory() {
                if !skip_main_attributes {
                    modifiers.push(("health".to_owned(), 2, None));
                    modifiers.push(("defence".to_owned(), 1, None));
                }
                filter.push("health");
                filter.push("defence");
            }
            for _ in 0..num_upgrades {
                if let Some((index, bonus_mod)) =
                    stack.attributes.get_random_existing_bonus_attribute_string(
                        &stack.metadata.bonus_stat_lines,
                        &filter,
                    )
                {
                    modifiers.push((bonus_mod.attribute_name.clone(), 1, Some(index)));
                }
            }
        }
        for (modifier, delta, index) in modifiers {
            if stack
                .metadata
                .bonus_stat_lines
                .iter()
                .any(|line| line.attribute_name == modifier)
            {
                stack.metadata.bonus_stat_lines[index.unwrap()].value += delta;
            }
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

pub fn skill_power_multiplier(skill_power: &SkillPower, blessing_bonus: f32) -> f32 {
    (1.0 + (skill_power.0 as f32 / 100.0)) * blessing_bonus
}
