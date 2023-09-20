use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use rand::Rng;

use crate::{
    attributes::attribute_helpers::create_new_random_item_stack_with_attributes,
    inventory::InventoryItemStack,
    item::{Loot, LootTable, LootTablePlugin, WorldObject},
    proto::proto_param::ProtoParam,
    ui::ChestContainer,
};

#[derive(Component, Reflect, FromReflect, Schematic, Default, Debug, Clone)]
#[reflect(Component, Schematic)]
pub enum LootChestType {
    #[default]
    Common,
    Uncommon,
    Rare,
    Food,
}

pub fn handle_new_loot_chest_spawn(
    mut loot_chests: Query<(&LootChestType, &mut ChestContainer), Added<LootChestType>>,
    proto_param: ProtoParam,
) {
    let mut rng = rand::thread_rng();

    for (chest_type, mut inventory) in loot_chests.iter_mut() {
        let loot_table = match chest_type {
            LootChestType::Common => LootTable {
                drops: vec![
                    Loot::new(WorldObject::Flint, 1, 2, 0.15),
                    Loot::new(WorldObject::Log, 1, 2, 0.15),
                    Loot::new(WorldObject::SmallPotion, 1, 1, 0.09),
                    Loot::new(WorldObject::GrassBlock, 1, 4, 0.25),
                    Loot::new(WorldObject::SlimeGoo, 1, 4, 0.25),
                    Loot::new(WorldObject::Stick, 1, 4, 0.25),
                    Loot::new(WorldObject::PlantFibre, 1, 4, 0.25),
                    Loot::new(WorldObject::String, 1, 2, 0.15),
                    Loot::new(WorldObject::Apple, 1, 4, 0.15),
                    Loot::new(WorldObject::Arrow, 11, 48, 0.85),
                    Loot::new(WorldObject::ThrowingStar, 11, 48, 0.85),
                    Loot::new(WorldObject::WoodPlank, 1, 4, 0.25),
                    Loot::new(WorldObject::PebbleBlock, 1, 4, 0.25),
                    Loot::new(WorldObject::Bandage, 1, 1, 0.09),
                ],
            },
            LootChestType::Uncommon => LootTable {
                drops: vec![
                    Loot::new(WorldObject::Flint, 1, 2, 0.2),
                    Loot::new(WorldObject::Log, 1, 2, 0.2),
                    Loot::new(WorldObject::SmallPotion, 1, 2, 0.2),
                    Loot::new(WorldObject::String, 1, 2, 0.2),
                    Loot::new(WorldObject::Apple, 1, 4, 0.2),
                    Loot::new(WorldObject::Arrow, 11, 48, 0.85),
                    Loot::new(WorldObject::ThrowingStar, 11, 48, 0.85),
                    Loot::new(WorldObject::WoodPlank, 1, 4, 0.25),
                    Loot::new(WorldObject::Bandage, 1, 2, 0.2),
                    Loot::new(WorldObject::Sword, 1, 1, 0.05),
                    Loot::new(WorldObject::BasicStaff, 1, 1, 0.05),
                    Loot::new(WorldObject::FireStaff, 1, 1, 0.05),
                    Loot::new(WorldObject::Chestplate, 1, 1, 0.05),
                    Loot::new(WorldObject::MetalPants, 1, 1, 0.05),
                    Loot::new(WorldObject::Dagger, 1, 1, 0.05),
                    Loot::new(WorldObject::Ring, 1, 1, 0.05),
                    Loot::new(WorldObject::Pendant, 1, 1, 0.05),
                    Loot::new(WorldObject::LargePotion, 1, 1, 0.15),
                    Loot::new(WorldObject::WoodBow, 1, 1, 0.05),
                    Loot::new(WorldObject::MagicWhip, 1, 1, 0.05),
                    Loot::new(WorldObject::Claw, 1, 1, 0.05),
                ],
            },
            LootChestType::Rare => LootTable {
                drops: vec![
                    Loot::new(WorldObject::Flint, 1, 2, 0.2),
                    Loot::new(WorldObject::Log, 1, 2, 0.2),
                    Loot::new(WorldObject::SmallPotion, 1, 4, 0.35),
                    Loot::new(WorldObject::String, 1, 2, 0.2),
                    Loot::new(WorldObject::Apple, 1, 4, 0.2),
                    Loot::new(WorldObject::Arrow, 32, 64, 0.85),
                    Loot::new(WorldObject::ThrowingStar, 32, 64, 0.85),
                    Loot::new(WorldObject::WoodPlank, 1, 4, 0.25),
                    Loot::new(WorldObject::Bandage, 1, 4, 0.35),
                    Loot::new(WorldObject::Sword, 1, 1, 0.15),
                    Loot::new(WorldObject::BasicStaff, 1, 1, 0.15),
                    Loot::new(WorldObject::FireStaff, 1, 1, 0.15),
                    Loot::new(WorldObject::Chestplate, 1, 1, 0.15),
                    Loot::new(WorldObject::MetalPants, 1, 1, 0.15),
                    Loot::new(WorldObject::Dagger, 1, 1, 0.15),
                    Loot::new(WorldObject::Ring, 1, 1, 0.15),
                    Loot::new(WorldObject::Pendant, 1, 1, 0.15),
                    Loot::new(WorldObject::LargePotion, 1, 3, 0.35),
                    Loot::new(WorldObject::WoodBow, 1, 1, 0.15),
                    Loot::new(WorldObject::MagicWhip, 1, 1, 0.15),
                    Loot::new(WorldObject::Claw, 1, 1, 0.15),
                ],
            },
            LootChestType::Food => LootTable {
                drops: vec![
                    Loot::new(WorldObject::Apple, 1, 4, 0.85),
                    Loot::new(WorldObject::Apple, 1, 4, 0.85),
                    Loot::new(WorldObject::Apple, 1, 4, 0.85),
                ],
            },
        };
        for loot in LootTablePlugin::get_drops(&loot_table, &proto_param, 0).iter() {
            let mut found_slot = false;
            while !found_slot {
                let picked_slot = rng.gen_range(0..inventory.items.items.len());
                if inventory.items.items[picked_slot].is_none() {
                    inventory.items.items[picked_slot] = Some(InventoryItemStack::new(
                        create_new_random_item_stack_with_attributes(loot, &proto_param),
                        picked_slot,
                    ));
                    found_slot = true;
                }
            }
        }
    }
}
