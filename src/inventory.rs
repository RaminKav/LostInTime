use core::panic;
use std::cmp::min;

use crate::{
    item::WorldObject,
    ui::{InventorySlotState, InventoryState},
    Game, GameState, TIME_STEP,
};
use bevy::{prelude::*, time::FixedTimestep};
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

pub const INVENTORY_SIZE: usize = 6 * 4;
pub const MAX_STACK_SIZE: usize = 16;

pub struct InventoryPlugin;

#[derive(Component, Inspectable, Debug, PartialEq, Copy, Clone)]
pub struct ItemStack {
    pub obj_type: WorldObject,
    pub count: usize,
}

#[derive(Component, Inspectable, Debug, PartialEq, Copy, Clone)]

pub struct InventoryItemStack {
    pub item_stack: ItemStack,
    pub slot: usize,
}

#[derive(Debug)]
pub enum InventoryError {
    FailedToMerge(String),
}
impl InventoryItemStack {
    pub fn add_to_inventory(self, game: &mut Game, inv_slots: &mut Query<&mut InventorySlotState>) {
        game.player.inventory[self.slot] = Some(self);
        InventoryPlugin::mark_slot_dirty(self.slot, inv_slots);
    }
    pub fn remove_from_inventory(self, game: &mut Game) {
        game.player.inventory[self.slot] = None
    }
    pub fn modify_count(&mut self, amount: i8) {
        self.item_stack.modify_count(amount);
    }
}
impl ItemStack {
    pub fn add_to_inventory(self, game: &mut Game, inv_slots: &mut Query<&mut InventorySlotState>) {
        // if stack of that item exists, add to it, otherwise push as new stack.
        // TODO: add max stack size, and create new stack if reached.
        // TODO: abstract direct access of .obj_type behind a getter
        if let Some(stack) = game.player.inventory.iter().find(|i| match i {
            Some(ii) if ii.item_stack.count < MAX_STACK_SIZE => {
                ii.item_stack.obj_type == self.obj_type
            }
            _ => false,
        }) {
            // safe to unwrap, we check for it above
            let slot = stack.unwrap().slot;
            let pre_stack_size = game.player.inventory[slot].unwrap().item_stack.count;

            game.player.inventory[slot] = Some(InventoryItemStack {
                item_stack: ItemStack {
                    obj_type: self.obj_type,
                    count: min(self.count + pre_stack_size, MAX_STACK_SIZE),
                },
                slot,
            });
            InventoryPlugin::mark_slot_dirty(slot, inv_slots);

            if pre_stack_size + self.count > MAX_STACK_SIZE {
                Self::add_to_empty_inventory_slot(
                    ItemStack {
                        obj_type: self.obj_type,
                        count: pre_stack_size + self.count - MAX_STACK_SIZE,
                    },
                    game,
                    inv_slots,
                );
            }
        } else {
            Self::add_to_empty_inventory_slot(self, game, inv_slots);
        }
    }
    pub fn add_to_empty_inventory_slot(
        self,
        game: &mut Game,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        let slot = InventoryPlugin::get_first_empty_slot(game);
        if let Some(slot) = slot {
            let item = InventoryItemStack {
                item_stack: self,
                slot,
            };
            item.add_to_inventory(game, inv_slots);
        }
    }
    pub fn try_add_to_target_inventory_slot(
        self,
        game: &mut Game,
        slot: usize,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) -> Result<(), InventoryError> {
        if let Some(mut existing_stack) = game.player.inventory[slot] {
            if existing_stack.item_stack.obj_type == self.obj_type {
                existing_stack.modify_count(self.count as i8);
                return Ok(());
            }
            Err(InventoryError::FailedToMerge(
                "Target item stack is not the same WorldObject type.".to_string(),
            ))
        } else {
            let item = InventoryItemStack {
                item_stack: self,
                slot,
            };
            item.add_to_inventory(game, inv_slots);
            Ok(())
        }
    }
    pub fn split(self) -> (usize, usize) {
        let split_count = self.count / 2;
        (self.count - split_count, split_count)
    }
    pub fn modify_count(&mut self, amount: i8) {
        if (self.count as i8) + amount <= 0 {
            self.count = 0;
        } else {
            self.count = ((self.count as i8) + amount) as usize;
        }
    }
}

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<ItemStack>().add_system_set(
            SystemSet::on_update(GameState::Main)
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64)), // .with_system(Self::update_inventory),
        );
    }
}

impl InventoryPlugin {
    // get the lowest slot number occupied

    pub fn get_first_empty_slot(game: &Game) -> Option<usize> {
        //TODO: add game param const for this
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..game.player.inventory.len()).find(|&i| game.player.inventory[i].is_none())
    }
    /// Attempt to merge item at slot a into b. Panics if
    /// either slot is empty, or not matching WorldObject types.
    /// Keeps remainder where it was, if overflow.
    pub fn merge_item_stacks(
        game: &mut Game,
        to_merge: ItemStack,
        merge_into: InventoryItemStack,
    ) -> Option<ItemStack> {
        let item_type = to_merge.obj_type;
        //TODO: should this return  None, or the original stack??
        if item_type != merge_into.item_stack.obj_type {
            return None;
        }
        let item_a_count = to_merge.count;
        let item_b_count = merge_into.item_stack.count;
        let combined_size = item_a_count + item_b_count;

        game.player.inventory[merge_into.slot] = Some(InventoryItemStack {
            item_stack: ItemStack {
                obj_type: item_type,
                count: min(combined_size, MAX_STACK_SIZE),
            },
            slot: merge_into.slot,
        });

        // if we overflow, keep remainder where it was
        if combined_size > MAX_STACK_SIZE {
            return Some(ItemStack {
                obj_type: item_type,
                count: combined_size - MAX_STACK_SIZE,
            });
        }

        None
    }
    fn swap_items(
        game: &mut Game,
        item: ItemStack,
        target_slot: usize,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) -> ItemStack {
        let target_item_option = game.player.inventory[target_slot];
        if let Some(target_item_stack) = target_item_option {
            game.player.inventory[target_slot] = Some(InventoryItemStack {
                item_stack: item,
                slot: target_item_stack.slot,
            });
            Self::mark_slot_dirty(target_slot, inv_slots);
            return target_item_stack.item_stack;
        }
        item
    }
    pub fn drop_item_on_slot(
        game: &mut Game,
        item: ItemStack,
        drop_slot: usize,
        inv_slot_state: &mut Query<&mut InventorySlotState>,
    ) -> Option<ItemStack> {
        let obj_type = item.obj_type;
        let target_item_option = game.player.inventory[drop_slot];
        if let Some(target_item) = target_item_option {
            if target_item.item_stack.obj_type == obj_type {
                Self::mark_slot_dirty(drop_slot, inv_slot_state);
                return Self::merge_item_stacks(game, item, target_item);
            } else {
                return Some(Self::swap_items(game, item, drop_slot, inv_slot_state));
            }
        } else if item
            .try_add_to_target_inventory_slot(game, drop_slot, inv_slot_state)
            .is_err()
        {
            panic!("Failed to drop item on stot");
        }

        None
    }

    // to split a stack, we right click on an existing stack.
    // we do not know where the target stack is, and since the current stack
    // is not moving, we are creating a new entity visual to drag
    // the drag
    pub fn split_stack(
        game: &mut Game,
        item_stack: ItemStack,
        item_slot: usize,
        item_slot_state: &mut InventorySlotState,
    ) -> ItemStack {
        let (amount_split, remainder_left) = item_stack.split();
        game.player.inventory[item_slot] = if remainder_left > 0 {
            Some(InventoryItemStack {
                item_stack: ItemStack {
                    obj_type: item_stack.obj_type,
                    count: remainder_left,
                },
                slot: item_slot,
            })
        } else {
            None
        };
        item_slot_state.dirty = true;
        ItemStack {
            obj_type: item_stack.obj_type,
            count: amount_split,
        }
    }
    //TODO: Maybe make a resource to instead store slot indexs, and then mark them all dirty in a system?
    // benefit: dont need to pass in the inv slot query anymore
    pub fn mark_slot_dirty(slot_index: usize, inv_slots: &mut Query<&mut InventorySlotState>) {
        for mut state in inv_slots.iter_mut() {
            if state.slot_index == slot_index {
                state.dirty = true;
            }
        }
    }
}
