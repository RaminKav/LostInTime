use std::cmp::min;

use crate::{
    inventory::{InventoryItemStack, ItemStack, MAX_STACK_SIZE},
    item::{CraftedItemEvent, WorldObject},
    ui::{mark_slot_dirty, InventorySlotState, InventorySlotType, UIContainersParam},
    world::TileMapPosition,
};

use bevy::{prelude::*, utils::HashMap};

pub const CONTAINER_UNIT: Option<InventoryItemStack> = None;

#[derive(Default, Debug, Clone)]
pub struct Container {
    pub items: Vec<Option<InventoryItemStack>>,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct ContainerRegistry {
    pub containers: HashMap<TileMapPosition, Container>,
}

impl Container {
    pub fn with_size(size: usize) -> Self {
        Self {
            items: vec![CONTAINER_UNIT; size],
        }
    }
    pub fn get_first_empty_slot(&self) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..self.items.len()).find(|&i| self.items[i].is_none())
    }
    pub fn get_first_empty_hotbar_slot(&self) -> Option<usize> {
        (0..6).find(|&i| self.items[i].is_none())
    }
    pub fn get_first_empty_non_hotbar_slot(&self) -> Option<usize> {
        (6..self.items.len()).find(|&i| self.items[i].is_none())
    }

    pub fn get_slot_for_item_in_container(&self, obj: &WorldObject) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..self.items.len()).find(|&i| {
            self.items[i].is_some() && self.items[i].as_ref().unwrap().item_stack.obj_type == *obj
        })
    }
    pub fn get_slot_for_item_in_container_with_space(
        &self,
        obj: &WorldObject,
        exclude_slot: Option<usize>,
    ) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..self.items.len()).find(|&i| {
            self.items[i].is_some()
                && self.items[i].as_ref().unwrap().item_stack.obj_type == *obj
                && self.items[i].as_ref().unwrap().item_stack.count < MAX_STACK_SIZE
                && exclude_slot != Some(i)
        })
    }
    pub fn get_item_count_in_container(&self, obj: WorldObject) -> usize {
        let mut count = 0;
        for item in self.items.clone() {
            if let Some(item_stack) = item {
                if item_stack.item_stack.obj_type == obj {
                    count += item_stack.item_stack.count;
                }
            }
        }
        count
    }
    pub fn move_item_to_target_container(&mut self, target_container: &mut Container, slot: usize) {
        let container_item = self.items[slot].clone();
        if let Some(mut container_a_item) = container_item {
            let container_a_item_count = container_a_item.item_stack.count;
            if let Some(existing_item_slot) = Self::get_slot_for_item_in_container_with_space(
                target_container,
                &container_a_item.item_stack.obj_type,
                None,
            ) {
                let mut existing_item = target_container.items[existing_item_slot]
                    .as_ref()
                    .unwrap()
                    .clone();
                let space_left = MAX_STACK_SIZE - existing_item.item_stack.count;
                if space_left < container_a_item_count {
                    target_container.items[existing_item.slot] =
                        existing_item.modify_count(space_left as i8);
                    self.items[container_a_item.slot] =
                        container_a_item.modify_count(-(space_left as i8));
                    if let Some(next_avail_slot) = Self::get_first_empty_slot(target_container) {
                        target_container.items[next_avail_slot] = container_a_item
                            .modify_slot(next_avail_slot)
                            .modify_count(-(space_left as i8));
                        self.items[container_a_item.slot] = None;
                    }
                } else {
                    target_container.items[existing_item.slot] =
                        existing_item.modify_count(container_a_item_count as i8);
                    self.items[slot] = None;
                }
            } else {
                if let Some(next_avail_slot) = Self::get_first_empty_slot(target_container) {
                    target_container.items[next_avail_slot] =
                        Some(container_a_item.modify_slot(next_avail_slot));
                    self.items[slot] = None;
                }
            }
        }
    }
    //TODO: there has to be a nice way to merge the two move_item_between_containers fn
    /// use only on inventory container
    pub fn move_item_from_hotbar_to_inv_or_vice_versa(&mut self, slot: usize) {
        let inv_item = self.items[slot].clone();
        let is_from_hotbar = slot < 6;
        if let Some(mut inv_item_stack) = inv_item {
            let stack_count = inv_item_stack.item_stack.count;
            if let Some(existing_item_slot) = Self::get_slot_for_item_in_container_with_space(
                self,
                &inv_item_stack.item_stack.obj_type,
                Some(slot),
            ) {
                if is_from_hotbar && existing_item_slot < 6 {
                    if let Some(next_avail_inv_slot) = Self::get_first_empty_non_hotbar_slot(self) {
                        self.items[next_avail_inv_slot] =
                            Some(inv_item_stack.modify_slot(next_avail_inv_slot));
                        self.items[slot] = None;
                    }
                    return;
                } else if !is_from_hotbar && existing_item_slot >= 6 {
                    if let Some(next_avail_hotbar_slot) = Self::get_first_empty_hotbar_slot(self) {
                        self.items[next_avail_hotbar_slot] =
                            Some(inv_item_stack.modify_slot(next_avail_hotbar_slot));
                        self.items[slot] = None;
                    }
                    return;
                }
                let mut existing_item = self.items[existing_item_slot].as_ref().unwrap().clone();
                let space_left = MAX_STACK_SIZE - existing_item.item_stack.count;
                if space_left < stack_count {
                    self.items[existing_item.slot] = existing_item.modify_count(space_left as i8);
                    self.items[inv_item_stack.slot] =
                        inv_item_stack.modify_count(-(space_left as i8));
                    if let Some(next_avail_slot) = Self::get_first_empty_slot(self) {
                        self.items[next_avail_slot] = inv_item_stack
                            .modify_slot(next_avail_slot)
                            .modify_count(-(space_left as i8));
                        self.items[inv_item_stack.slot] = None;
                    }
                } else {
                    self.items[existing_item.slot] = existing_item.modify_count(stack_count as i8);
                    self.items[slot] = None;
                }
            } else {
                if !is_from_hotbar {
                    if let Some(next_avail_slot) = Self::get_first_empty_hotbar_slot(self) {
                        self.items[next_avail_slot] =
                            Some(inv_item_stack.modify_slot(next_avail_slot));
                        self.items[slot] = None;
                    }
                } else {
                    if let Some(next_avail_slot) = Self::get_first_empty_non_hotbar_slot(self) {
                        self.items[next_avail_slot] =
                            Some(inv_item_stack.modify_slot(next_avail_slot));
                        self.items[slot] = None;
                    }
                }
            }
        }
    }
    /// Attempt to merge item at slot a into b. Panics if
    /// either slot is empty, or not matching WorldObject types.
    /// Keeps remainder where it was, if overflow.
    pub fn merge_item_stacks(
        &mut self,
        to_merge: ItemStack,
        merge_into: InventoryItemStack,
    ) -> Option<ItemStack> {
        let item_type = to_merge.obj_type;
        //TODO: should this return  None, or the original stack??
        if item_type != *merge_into.get_obj()
            || merge_into.item_stack.metadata != to_merge.metadata
            || merge_into.item_stack.attributes != to_merge.attributes
        {
            return Some(to_merge);
        }
        let item_a_count = to_merge.count;
        let item_b_count = merge_into.item_stack.count;
        let combined_size = item_a_count + item_b_count;
        let new_item = Some(InventoryItemStack {
            item_stack: to_merge.copy_with_count(min(combined_size, MAX_STACK_SIZE)),
            slot: merge_into.slot,
        });

        self.items[merge_into.slot] = new_item;

        // if we overflow, keep remainder where it was
        if combined_size > MAX_STACK_SIZE {
            return Some(to_merge.copy_with_count(combined_size - MAX_STACK_SIZE));
        }

        None
    }
    pub fn pick_up_and_merge_crafting_result_stack(
        &self,
        dragging_item: ItemStack,
        dropped_slot: usize,
        cont_param: &mut UIContainersParam,
    ) -> Option<ItemStack> {
        let inv = self.clone();
        let container_option = cont_param.get_active_ui_container();

        let pickup_item_option = if let Some(container) = container_option {
            container.items[dropped_slot].clone()
        } else {
            inv.items[dropped_slot].clone()
        };
        if let Some(pickup_item) = pickup_item_option {
            let dragging_item_type = dragging_item.obj_type;
            if dragging_item_type != *pickup_item.get_obj() {
                return Some(dragging_item);
            }
            let item_a_count = dragging_item.count;
            let item_b_count = pickup_item.item_stack.count;
            let combined_size = item_a_count + item_b_count;
            let new_item = Some(dragging_item.copy_with_count(min(combined_size, MAX_STACK_SIZE)));
            cont_param.crafted_event.send(CraftedItemEvent {
                obj: dragging_item_type,
            });
            return new_item;
        } else {
            Some(dragging_item)
        }
    }
    pub fn swap_items(
        &mut self,
        item: ItemStack,
        target_slot: usize,
        inv_slots: &mut Query<&mut InventorySlotState>,
        slot_type: InventorySlotType,
    ) -> ItemStack {
        let target_item_option = self.items[target_slot].clone();
        if let Some(target_item_stack) = target_item_option {
            let swapped_item = Some(InventoryItemStack {
                item_stack: item,
                slot: target_item_stack.slot,
            });

            self.items[target_slot] = swapped_item;
            mark_slot_dirty(target_item_stack.slot, slot_type, inv_slots);
            return target_item_stack.item_stack;
        }
        item
    }
}
