use crate::attributes::{ItemAttributes, ItemRarity};
use crate::inventory::ItemStack;
use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use rand::Rng;
use serde::Deserialize;

use super::{ItemDisplayMetaData, WorldObject};
pub struct LootTablePlugin;
impl Plugin for LootTablePlugin {
    fn build(&self, _app: &mut App) {
        // app.add_event::<CraftingSlotUpdateEvent>().add_system_set(
        //     SystemSet::on_update(GameState::Main)
        //         .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
        // );
    }
}

#[derive(Default, Schematic, Reflect, FromReflect, Clone, Debug, Component, Deserialize)]
#[reflect(Schematic)]
pub struct LootTable {
    pub drops: Vec<Loot>,
}

#[derive(Default, Reflect, FromReflect, Clone, Debug, Deserialize)]
pub struct Loot {
    pub item: WorldObject,
    pub min: usize,
    pub max: usize,
    pub rate: f32,
}

impl LootTablePlugin {
    pub fn get_drops(loot_table: &LootTable, loot_bonus: i32) -> Vec<ItemStack> {
        let mut rng = rand::thread_rng();
        let mut loot = vec![];
        for drop in loot_table.drops.iter() {
            let r: f32 = rng.gen();
            if r <= drop.rate * (1.0 + loot_bonus as f32 / 100.0) {
                let attributes = ItemAttributes::default();
                loot.push(ItemStack {
                    obj_type: drop.item,
                    rarity: ItemRarity::Common,
                    count: if drop.min == drop.max {
                        drop.min
                    } else {
                        rng.gen_range(drop.min..drop.max)
                    },
                    attributes: attributes.clone(),
                    metadata: ItemDisplayMetaData {
                        name: drop.item.to_string(),
                        desc: "A cool piece of Equipment".to_string(),
                    },
                });
            }
        }
        loot
    }
}
