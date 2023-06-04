use crate::attributes::ItemAttributes;
use crate::enemy::Mob;
use crate::inventory::ItemStack;
use bevy::{prelude::*, utils::HashMap};
use rand::Rng;
use serde::Deserialize;

use super::{ItemDisplayMetaData, WorldObject};
pub struct LootTablePlugin;
impl Plugin for LootTablePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LootTableMap::default());
        // app.add_event::<CraftingSlotUpdateEvent>().add_system_set(
        //     SystemSet::on_update(GameState::Main)
        //         .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
        // );
    }
}

#[derive(Resource, Clone, Debug, Default, Deserialize)]
pub struct LootTableMap {
    pub table: HashMap<Mob, LootTable>,
}

#[derive(Default, Clone, Debug, Component, Deserialize)]
pub struct LootTable {
    pub drops: Vec<Loot>,
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Loot {
    pub item: WorldObject,
    pub min: usize,
    pub max: usize,
    pub rate: f32,
}

impl LootTablePlugin {
    pub fn get_drops(loot_table: &LootTable) -> Vec<ItemStack> {
        let mut rng = rand::thread_rng();
        let mut loot = vec![];
        for drop in loot_table.drops.iter() {
            let r: f32 = rng.gen();
            if r <= drop.rate {
                let attributes = ItemAttributes::default();
                loot.push(ItemStack {
                    obj_type: drop.item,
                    count: if drop.min == drop.max {
                        drop.min
                    } else {
                        rng.gen_range(drop.min..drop.max)
                    },
                    attributes: attributes.clone(),
                    metadata: ItemDisplayMetaData {
                        name: drop.item.to_string(),
                        desc: "A cool piece of Equipment".to_string(),
                        attributes: attributes.get_tooltips(),
                        durability: attributes.get_durability_tooltip(),
                    },
                });
            }
        }
        loot
    }
}
