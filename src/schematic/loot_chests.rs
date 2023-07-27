use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use crate::{
    inventory::{InventoryItemStack, ItemStack},
    item::WorldObject,
    proto::proto_param::{self, ProtoParam},
    ui::ChestInventory,
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
    mut loot_chests: Query<(&LootChestType, &mut ChestInventory), Added<LootChestType>>,
    proto_param: ProtoParam,
) {
    for (chest_type, mut inventory) in loot_chests.iter_mut() {
        match chest_type {
            LootChestType::Common => {
                inventory.items.items[0] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::Stick)
                        .unwrap()
                        .clone(),
                    0,
                ));
            }
            LootChestType::Uncommon => {
                inventory.items.items[3] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::SlimeGoo)
                        .unwrap()
                        .clone(),
                    3,
                ));
                inventory.items.items[11] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::SmallPotion)
                        .unwrap()
                        .clone(),
                    11,
                ));
            }
            LootChestType::Rare => {
                inventory.items.items[5] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::Chestplate)
                        .unwrap()
                        .clone(),
                    5,
                ));
                inventory.items.items[7] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::LargePotion)
                        .unwrap()
                        .clone(),
                    7,
                ));
                inventory.items.items[1] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::BasicStaff)
                        .unwrap()
                        .clone(),
                    1,
                ));
            }
            LootChestType::Food => {
                inventory.items.items[7] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::Apple)
                        .unwrap()
                        .clone(),
                    7,
                ));
                inventory.items.items[10] = Some(InventoryItemStack::new(
                    proto_param
                        .get_item_data(WorldObject::Apple)
                        .unwrap()
                        .clone(),
                    10,
                ));
            }
        }
    }
}
