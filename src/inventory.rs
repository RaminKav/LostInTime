use core::panic;
use std::cmp::min;

use crate::{
    animations::{AnimationPosTracker, AnimationTimer, AttackAnimationTimer},
    attributes::{AttributeModifier, ItemAttributes, ItemRarity},
    inputs::FacingDirection,
    item::{
        Equipment, EquipmentData, EquipmentType, ItemDisplayMetaData, MainHand, WorldObject,
        PLAYER_EQUIPMENT_POSITIONS,
    },
    player::Limb,
    proto::proto_param::ProtoParam,
    ui::{InventorySlotState, InventorySlotType, UIContainersParam},
    world::y_sort::YSort,
    GameParam,
};
use rand::Rng;

use bevy::prelude::*;

use bevy_proto::prelude::*;
use bevy_rapier2d::prelude::{Collider, RigidBody, Sensor};

pub const INVENTORY_SIZE: usize = 6 * 4;
pub const INVENTORY_INIT: Option<InventoryItemStack> = None;
pub const MAX_STACK_SIZE: usize = 64;

#[derive(Component, Debug, Default, Clone)]
pub struct Inventory {
    pub items: Container,
    pub equipment_items: Container,
    pub accessory_items: Container,
    pub crafting_items: Container,
    // pub crafting_result_item: Container,
}
impl Inventory {
    pub fn get_items_from_slot_type(&self, slot_type: InventorySlotType) -> &Container {
        match slot_type {
            InventorySlotType::Equipment => &self.equipment_items,
            InventorySlotType::Accessory => &self.accessory_items,
            InventorySlotType::Crafting => &self.crafting_items,
            _ => &self.items,
        }
    }
    pub fn get_mut_items_from_slot_type(&mut self, slot_type: InventorySlotType) -> &mut Container {
        match slot_type {
            InventorySlotType::Equipment => &mut self.equipment_items,
            InventorySlotType::Accessory => &mut self.accessory_items,
            InventorySlotType::Crafting => &mut self.crafting_items,
            _ => &mut self.items,
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct Container {
    pub items: Vec<Option<InventoryItemStack>>,
}
impl Container {
    pub fn with_size(size: usize) -> Self {
        Self {
            items: vec![INVENTORY_INIT; size],
        }
    }
}
pub struct InventoryPlugin;

#[derive(Component, Debug, PartialEq, Reflect, FromReflect, Schematic, Default, Clone)]
#[reflect(Schematic, Default)]
pub struct ItemStack {
    pub obj_type: WorldObject,
    pub count: usize,
    pub rarity: ItemRarity,
    pub attributes: ItemAttributes,
    pub metadata: ItemDisplayMetaData,
}

#[derive(Debug)]
pub enum InventoryError {
    FailedToMerge(String),
}
#[derive(Component, Debug, PartialEq, Clone)]
pub struct InventoryItemStack {
    pub item_stack: ItemStack,
    pub slot: usize,
}

impl InventoryItemStack {
    pub fn new(item_stack: ItemStack, slot: usize) -> Self {
        Self { item_stack, slot }
    }
    pub fn get_obj(&self) -> &WorldObject {
        &self.item_stack.obj_type
    }
    pub fn drop_item_on_slot(
        &self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
        slot_type: InventorySlotType,
    ) -> Option<ItemStack> {
        let obj_type = self.item_stack.obj_type;
        let target_item_option = container.items[self.slot].clone();
        if let Some(target_item) = target_item_option {
            if target_item.get_obj() == &obj_type
                && target_item.item_stack.metadata == self.item_stack.metadata
                && target_item.item_stack.attributes == self.item_stack.attributes
                && !(slot_type.is_equipment() || slot_type.is_accessory())
            {
                InventoryPlugin::mark_slot_dirty(self.slot, slot_type, inv_slots);
                return InventoryPlugin::merge_item_stacks(
                    self.item_stack.clone(),
                    target_item,
                    container,
                );
            } else {
                return Some(InventoryPlugin::swap_items(
                    self.item_stack.clone(),
                    self.slot,
                    container,
                    inv_slots,
                    slot_type,
                ));
            }
        } else if self
            .item_stack
            .clone()
            .try_add_to_target_inventory_slot(self.slot, container, inv_slots)
            .is_err()
        {
            panic!("Failed to drop item on stot");
        }

        None
    }
    /// spawns the item entity in this item stack in players hand
    pub fn spawn_item_on_hand(
        &self,
        commands: &mut Commands,
        game: &mut GameParam,
        proto: &ProtoParam,
    ) -> Entity {
        let obj = *self.get_obj();
        let limb = &Limb::Hands;
        let item_map = &game.graphics.spritesheet_map;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        //TODO: extract this out to helper fn vvvv
        let is_block = obj.is_block();
        let has_icon = if is_block {
            game.graphics.icons.as_ref().unwrap().get(&obj)
        } else {
            None
        };
        let _sprite = if let Some(icon) = has_icon {
            icon.clone()
        } else {
            game.graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&obj)
                .unwrap_or_else(|| panic!("No graphic for object {self:?}"))
                .clone()
        };

        let player_state = game.player();
        let player_e = game.player_query.single().0;
        let obj_data = game.world_obj_data.properties.get(&obj).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let is_facing_left = player_state.direction == FacingDirection::Left;

        let position = Vec3::new(
            PLAYER_EQUIPMENT_POSITIONS[limb].x
                + anchor.x * obj_data.size.x
                + if is_facing_left { 0. } else { 11. },
            PLAYER_EQUIPMENT_POSITIONS[limb].y + anchor.y * obj_data.size.y,
            0.01, //500. - (PLAYER_EQUIPMENT_POSITIONS[&limb].y + anchor.y * obj_data.size.y) * 0.1,
        );
        // despawn old held item if it exists
        if let Some(main_hand_data) = &player_state.main_hand_slot {
            commands.entity(main_hand_data.entity).despawn();
        }

        //spawn new item entity
        let item = commands
            .spawn(SpatialBundle {
                transform: Transform {
                    translation: position,
                    scale: Vec3::new(1., 1., 1.),
                    // rotation: Quat::from_rotation_z(0.8),
                    ..Default::default()
                },
                visibility: Visibility::Visible,
                ..Default::default()
            })
            .insert(Equipment(*limb))
            .insert(Name::new("EquippedItem"))
            .insert(self.item_stack.attributes.clone())
            .insert(obj)
            .set_parent(player_e)
            .id();

        let mut item_entity = commands.entity(item);

        item_entity
            .insert(MainHand)
            .insert(Sensor)
            .insert(RigidBody::Fixed)
            .insert(Collider::cuboid(
                obj_data.size.x / 1.5,
                obj_data.size.y / 1.5,
            ))
            .insert(AttackAnimationTimer(
                Timer::from_seconds(0.18, TimerMode::Once),
                0.,
            ));
        game.player_mut().main_hand_slot = Some(EquipmentData { obj, entity: item });
        if let Some(melee) = proto.is_item_melee_weapon(obj) {
            item_entity.insert(melee.clone());
        }
        if let Some(ranged) = proto.is_item_ranged_weapon(obj) {
            item_entity.insert(ranged.clone());
        }

        item
    }
    // to split a stack, we right click on an existing stack.
    // we do not know where the target stack is, and since the current stack
    // is not moving, we are creating a new entity visual to drag
    // the drag
    pub fn split_stack(
        &self,
        item_slot_state: &mut InventorySlotState,
        container: &mut Container,
    ) -> ItemStack {
        let (amount_split, remainder_left) = self.item_stack.clone().split();
        let remainder_stack = if remainder_left > 0 {
            Some(InventoryItemStack {
                item_stack: self.item_stack.copy_with_count(remainder_left),
                slot: self.slot,
            })
        } else {
            None
        };
        container.items[self.slot] = remainder_stack;
        item_slot_state.dirty = true;
        self.item_stack.copy_with_count(amount_split)
    }

    pub fn add_to_inventory(
        &self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        container.items[self.slot] = Some(self.clone());
        InventoryPlugin::mark_slot_dirty(self.slot, InventorySlotType::Normal, inv_slots);
    }
    pub fn remove_from_inventory(self, container: &mut Container) {
        container.items[self.slot] = None
    }
    pub fn modify_attributes(
        &self,
        modifier: AttributeModifier,
        container: &mut Container,
    ) -> Self {
        let new_item_stack = self
            .item_stack
            .clone()
            .get_copy_with_modified_attributes(modifier);

        let inv_stack = Self {
            item_stack: new_item_stack,
            slot: self.slot,
        };
        container.items[self.slot] = Some(inv_stack.clone());
        inv_stack
    }
    pub fn modify_count(&mut self, amount: i8) -> Option<Self> {
        self.item_stack.modify_count(amount);
        if self.item_stack.count == 0 {
            return None;
        }
        Some(self.clone())
    }
    pub fn modify_slot(&self, slot: usize) -> Self {
        let item_stack = self.item_stack.clone();
        Self { item_stack, slot }
    }
    pub fn validate(
        &self,
        slot_type: InventorySlotType,
        proto_param: &ProtoParam,
        ui_cont_param: &UIContainersParam,
    ) -> bool {
        if slot_type.is_furnace() {
            return ui_cont_param.furnace_option.as_ref().unwrap().slot_map[self.slot]
                .contains(&self.item_stack.obj_type);
        }
        if !(slot_type.is_accessory() || slot_type.is_equipment()) {
            return true;
        }
        let equipment_type =
            proto_param.get_component::<EquipmentType, _>(self.item_stack.obj_type);
        if let Some(equipment_type) = equipment_type {
            return equipment_type.get_valid_slots().contains(&self.slot)
                && equipment_type.get_valid_slot_type() == slot_type;
        }
        false
    }
}
//TODO: abstract all these behind a AddItemToInventoryEvent ? let event drive info needed for sub-fns
impl ItemStack {
    //TODO: fix for later, remove and use proto
    pub fn spawn_as_drop(
        &self,
        commands: &mut Commands,
        game: &mut GameParam,
        pos: Vec2,
    ) -> Entity {
        let item_map = &game.graphics.spritesheet_map;
        let obj = self.obj_type;
        if item_map.is_none() {
            panic!("graphics not loaded");
        }
        let sprite = game
            .graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&obj)
            .unwrap_or_else(|| panic!("No graphic for object {self:?}"))
            .clone();
        let obj_data = game.world_obj_data.properties.get(&obj).unwrap();
        let anchor = obj_data.anchor.unwrap_or(Vec2::ZERO);
        let mut rng = rand::thread_rng();
        let drop_spread = 10.;

        let position = Vec3::new(
            pos.x + anchor.x * obj_data.size.x + rng.gen_range(-drop_spread..drop_spread),
            pos.y + anchor.y * obj_data.size.y + rng.gen_range(-drop_spread..drop_spread),
            0.,
        );

        let transform = Transform {
            translation: position,
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        };

        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: game.graphics.texture_atlas.as_ref().unwrap().clone(),
                transform,
                ..Default::default()
            })
            .insert(Name::new("DropItem"))
            .insert(self.clone())
            //TODO: double colliders??
            .insert(Collider::cuboid(8., 8.))
            .insert(Sensor)
            .insert(AnimationTimer(Timer::from_seconds(
                0.1,
                TimerMode::Repeating,
            )))
            .insert(AnimationPosTracker(0., 0., 0.3))
            .insert(YSort(0.))
            .insert(obj)
            .id();

        commands.entity(item).insert(Collider::cuboid(
            obj_data.size.x / 3.5,
            obj_data.size.y / 4.5,
        ));
        game.world_obj_data
            .drop_entities
            .insert(item, (self.clone(), transform));
        item
    }
    pub fn copy_with_attributes(&self, attributes: &ItemAttributes) -> Self {
        Self {
            obj_type: self.obj_type,
            count: self.count,
            rarity: self.rarity.clone(),
            attributes: attributes.clone(),
            metadata: self.metadata.clone(),
        }
    }
    pub fn copy_with_count(&self, count: usize) -> Self {
        Self {
            obj_type: self.obj_type,
            count,
            rarity: self.rarity.clone(),
            attributes: self.attributes.clone(),
            metadata: self.metadata.clone(),
        }
    }
    pub fn add_to_inventory(
        self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        // if stack of that item exists, add to it, otherwise push as new stack.
        // TODO: add max stack size, and create new stack if reached.
        // TODO: abstract direct access of .obj_type behind a getter
        if let Some(stack) = container.items.iter().find(|i| match i {
            Some(ii) if ii.item_stack.count < MAX_STACK_SIZE => {
                *ii.get_obj() == self.obj_type && ii.item_stack.attributes == self.attributes
            }
            _ => false,
        }) {
            // safe to unwrap, we check for it above
            let slot = stack.clone().unwrap().slot;
            let inv_item_stack = container.items[slot].clone().unwrap();
            let pre_stack_size = inv_item_stack.item_stack.count;

            container.items[slot] = Some(InventoryItemStack {
                item_stack: self.copy_with_count(min(self.count + pre_stack_size, MAX_STACK_SIZE)),
                slot,
            });
            InventoryPlugin::mark_slot_dirty(
                inv_item_stack.slot,
                InventorySlotType::Normal,
                inv_slots,
            );

            if pre_stack_size + self.count > MAX_STACK_SIZE {
                Self::add_to_empty_inventory_slot(
                    self.copy_with_count(pre_stack_size + self.count - MAX_STACK_SIZE),
                    container,
                    inv_slots,
                );
            }
        } else {
            Self::add_to_empty_inventory_slot(self, container, inv_slots);
        }
    }
    pub fn add_to_empty_inventory_slot(
        self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        let slot = InventoryPlugin::get_first_empty_slot(container);
        if let Some(slot) = slot {
            let item = InventoryItemStack {
                item_stack: self,
                slot,
            };
            item.add_to_inventory(container, inv_slots);
        }
    }
    pub fn try_add_to_target_inventory_slot(
        self,
        slot: usize,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) -> Result<(), InventoryError> {
        let inv_or_crafting = container.items[slot].clone();
        if let Some(mut existing_stack) = inv_or_crafting {
            if existing_stack.get_obj() == &self.obj_type {
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

            item.add_to_inventory(container, inv_slots);

            Ok(())
        }
    }
    pub fn split(self) -> (usize, usize) {
        let split_count = self.count / 2;
        (self.count - split_count, split_count)
    }
    pub fn get_copy_with_modified_attributes(&mut self, modifier: AttributeModifier) -> Self {
        self.clone()
            .copy_with_attributes(self.attributes.change_attribute(modifier))
    }
    pub fn modify_count(&mut self, amount: i8) -> Self {
        if (self.count as i8) + amount <= 0 {
            self.count = 0;
        } else {
            self.count = ((self.count as i8) + amount) as usize;
        }
        self.clone()
    }
}

impl Plugin for InventoryPlugin {
    fn build(&self, _app: &mut App) {
        //
    }
}

impl InventoryPlugin {
    // get the lowest slot number occupied

    pub fn get_first_empty_slot(container: &Container) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..container.items.len()).find(|&i| container.items[i].is_none())
    }
    pub fn get_slot_for_item_in_container(
        container: &Container,
        obj: &WorldObject,
    ) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..container.items.len()).find(|&i| {
            container.items[i].is_some()
                && container.items[i].as_ref().unwrap().item_stack.obj_type == *obj
        })
    }
    pub fn get_slot_for_item_in_container_with_space(
        container: &Container,
        obj: &WorldObject,
    ) -> Option<usize> {
        //TODO: maybe move the actual inv to a type in this file, and move this fn into that struct
        (0..container.items.len()).find(|&i| {
            container.items[i].is_some()
                && container.items[i].as_ref().unwrap().item_stack.obj_type == *obj
                && container.items[i].as_ref().unwrap().item_stack.count < MAX_STACK_SIZE
        })
    }
    pub fn get_item_count_in_container(container: &Container, obj: WorldObject) -> usize {
        let mut count = 0;
        for item in container.items.clone() {
            if let Some(item_stack) = item {
                if item_stack.item_stack.obj_type == obj {
                    count += item_stack.item_stack.count;
                }
            }
        }
        count
    }
    pub fn move_item_between_containers(
        container_a: &mut Container,
        container_b: &mut Container,
        slot: usize,
    ) {
        let container_item = container_a.items[slot].clone();
        if let Some(mut container_a_item) = container_item {
            let container_a_item_count = container_a_item.item_stack.count;
            if let Some(existing_item_slot) = Self::get_slot_for_item_in_container_with_space(
                container_b,
                &container_a_item.item_stack.obj_type,
            ) {
                let mut existing_item = container_b.items[existing_item_slot]
                    .as_ref()
                    .unwrap()
                    .clone();
                let space_left = MAX_STACK_SIZE - existing_item.item_stack.count;
                if space_left < container_a_item_count {
                    container_b.items[existing_item.slot] =
                        existing_item.modify_count(space_left as i8);
                    container_a.items[container_a_item.slot] =
                        container_a_item.modify_count(-(space_left as i8));
                    if let Some(next_avail_slot) = Self::get_first_empty_slot(container_b) {
                        container_b.items[next_avail_slot] = container_a_item
                            .modify_slot(next_avail_slot)
                            .modify_count(-(space_left as i8));
                        container_a.items[container_a_item.slot] = None;
                    }
                } else {
                    container_b.items[existing_item.slot] =
                        existing_item.modify_count(container_a_item_count as i8);
                    container_a.items[slot] = None;
                }
            } else {
                if let Some(next_avail_slot) = Self::get_first_empty_slot(container_b) {
                    container_b.items[next_avail_slot] =
                        Some(container_a_item.modify_slot(next_avail_slot));
                    container_a.items[slot] = None;
                }
            }
        }
    }
    /// Attempt to merge item at slot a into b. Panics if
    /// either slot is empty, or not matching WorldObject types.
    /// Keeps remainder where it was, if overflow.
    pub fn merge_item_stacks(
        to_merge: ItemStack,
        merge_into: InventoryItemStack,
        container: &mut Container,
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

        container.items[merge_into.slot] = new_item;

        // if we overflow, keep remainder where it was
        if combined_size > MAX_STACK_SIZE {
            return Some(to_merge.copy_with_count(combined_size - MAX_STACK_SIZE));
        }

        None
    }
    pub fn pick_up_and_merge_crafting_result_stack(
        dragging_item: ItemStack,
        dropped_slot: usize,
        container: &mut Container,
    ) -> Option<ItemStack> {
        let pickup_item_option = container.items[dropped_slot].clone();
        if let Some(pickup_item) = pickup_item_option {
            let item_type = dragging_item.obj_type;
            //TODO: should this return  None, or the original stack??
            if item_type != *pickup_item.get_obj()
                || pickup_item.item_stack.metadata != dragging_item.metadata
                || pickup_item.item_stack.attributes != dragging_item.attributes
            {
                return Some(dragging_item);
            }
            let item_a_count = dragging_item.count;
            let item_b_count = pickup_item.item_stack.count;
            let combined_size = item_a_count + item_b_count;
            let new_item = Some(dragging_item.copy_with_count(min(combined_size, MAX_STACK_SIZE)));

            return new_item;
        } else {
            Some(dragging_item)
        }
    }
    fn swap_items(
        item: ItemStack,
        target_slot: usize,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
        slot_type: InventorySlotType,
    ) -> ItemStack {
        let target_item_option = container.items[target_slot].clone();
        if let Some(target_item_stack) = target_item_option {
            let swapped_item = Some(InventoryItemStack {
                item_stack: item,
                slot: target_item_stack.slot,
            });

            container.items[target_slot] = swapped_item;
            InventoryPlugin::mark_slot_dirty(target_item_stack.slot, slot_type, inv_slots);
            return target_item_stack.item_stack;
        }
        item
    }
    //TODO: Maybe make a resource to instead store slot indexs, and then mark them all dirty in a system?
    // benefit: dont need to pass in the inv slot query anymore
    pub fn mark_slot_dirty(
        slot_index: usize,
        slot_type: InventorySlotType,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        for mut state in inv_slots.iter_mut() {
            if state.slot_index == slot_index
                && (state.r#type == slot_type || state.r#type.is_hotbar())
            {
                state.dirty = true;
            }
        }
    }
}
