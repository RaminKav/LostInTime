use crate::inventory::ItemStack;
use crate::proto::proto_param::ProtoParam;
use bevy::prelude::*;
use rand::Rng;
use serde::Deserialize;

use super::WorldObject;
pub struct LootTablePlugin;
impl Plugin for LootTablePlugin {
    fn build(&self, _app: &mut App) {
        // app.add_message::<CraftingSlotUpdateEvent>().add_system_set(
        //     SystemSet::on_update(GameState::Main)
        //         .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
        // );
    }
}

#[derive(Default, Reflect, Clone, Debug, Component, Deserialize)]
pub struct LootTable {
    pub drops: Vec<Loot>,
}

#[derive(Default, Reflect, Clone, Debug, Deserialize)]
pub struct Loot {
    pub item: WorldObject,
    pub min: usize,
    pub max: usize,
    pub rate: f32,
}
impl Loot {
    pub fn new(item: WorldObject, min: usize, max: usize, rate: f32) -> Self {
        Self {
            item,
            min,
            max,
            rate,
        }
    }
}

impl LootTablePlugin {
    pub fn get_drops(
        loot_table: &LootTable,
        proto: &ProtoParam,
        loot_bonus: i32,
        level: Option<u8>,
        is_infinite_mode: bool,
        is_boss: bool,
    ) -> Vec<ItemStack> {
        Self::get_drops_with_coin_rate(
            loot_table,
            proto,
            loot_bonus,
            level,
            is_infinite_mode,
            is_boss,
            1.0,
        )
    }

    /// Like [`get_drops`], but multiplies coin entry rates by `coin_rate_multiplier`
    /// (e.g. 1.2 for the major CoinDropRate blessing). Raising the original coin roll
    /// also increases Golden Tooth procs, which only fire after a successful coin drop.
    pub fn get_drops_with_coin_rate(
        loot_table: &LootTable,
        proto: &ProtoParam,
        loot_bonus: i32,
        level: Option<u8>,
        is_infinite_mode: bool,
        is_boss: bool,
        coin_rate_multiplier: f32,
    ) -> Vec<ItemStack> {
        let mut rng = rand::thread_rng();
        let mut loot = vec![];
        for drop in loot_table.drops.iter() {
            let r: f32 = rng.gen();
            let infinite_mode_drop_multiplier = if is_infinite_mode && !is_boss {
                0.25
            } else {
                1.0
            };
            let coin_mult = if drop.item == WorldObject::Coin {
                coin_rate_multiplier
            } else {
                1.0
            };
            if r <= (drop.rate * coin_mult * infinite_mode_drop_multiplier)
                * (1.0 + loot_bonus as f32 / 100.0)
            {
                let mut stack = proto.get_item_data(drop.item).unwrap().clone();
                stack.metadata.level = level;
                loot.push(stack.copy_with_count(if drop.min == drop.max {
                    drop.min
                } else {
                    rng.gen_range(drop.min..drop.max)
                }));
            }
        }
        loot
    }
}
