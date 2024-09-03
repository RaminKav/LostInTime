use core::panic;
use std::cmp::min;

use crate::{
    animations::{AnimationPosTracker, AnimationTimer},
    attributes::{add_item_glows, AttributeModifier, ItemAttributes, ItemRarity},
    container::Container,
    inputs::FacingDirection,
    item::{
        ActiveMainHandState, Equipment, EquipmentType, ItemDisplayMetaData, MainHand, WorldObject,
        PLAYER_EQUIPMENT_POSITIONS,
    },
    player::Limb,
    proto::proto_param::ProtoParam,
    ui::{mark_slot_dirty, InventorySlotState, InventorySlotType, UIContainersParam},
    world::y_sort::YSort,
    GameParam,
};
use rand::Rng;

use bevy::prelude::*;

use bevy_proto::prelude::*;
use bevy_rapier2d::prelude::{Collider, RigidBody, Sensor};
use serde::{Deserialize, Serialize};

pub const INVENTORY_SIZE: usize = 6 * 4;
pub const MAX_STACK_SIZE: usize = 64;

#[derive(Component, Debug, Default, Clone, Serialize, Deserialize)]
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

#[derive(
    Component,
    Debug,
    PartialEq,
    Reflect,
    FromReflect,
    Schematic,
    Default,
    Clone,
    Serialize,
    Deserialize,
)]
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
    NotEnoughItems(String),
}
#[derive(Component, Debug, PartialEq, Clone, Serialize, Deserialize)]
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
                mark_slot_dirty(self.slot, slot_type, inv_slots);
                return container.merge_item_stacks(self.item_stack.clone(), target_item);
            } else {
                return Some(container.swap_items(
                    self.item_stack.clone(),
                    self.slot,
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
            .insert(self.item_stack.clone())
            .set_parent(player_e)
            .id();

        let mut item_entity = commands.entity(item);

        item_entity
            .insert(MainHand)
            .insert(Sensor)
            .insert(RigidBody::Fixed)
            .insert(Collider::cuboid(16. / 1.5, 16. / 1.5));
        game.player_mut().main_hand_slot = Some(ActiveMainHandState {
            item_stack: self.item_stack.clone(),
            entity: item,
        });
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

    pub fn add_to_container(
        &self,
        container: &mut Container,
        slot_type: InventorySlotType,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        container.items[self.slot] = Some(self.clone());
        mark_slot_dirty(self.slot, slot_type, inv_slots);
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
    pub fn modify_level(&self, amount: i8, container: &mut Container) -> Self {
        let mut new_stack = self.clone();
        new_stack.item_stack.metadata.level =
            Some((new_stack.item_stack.metadata.level.unwrap() as i8 + amount) as u8);
        container.items[self.slot] = Some(new_stack.clone());

        new_stack
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
    /// creates a new item stack with count 1 with no attributes or metadata
    /// used for icons in UI
    pub fn crate_icon_stack(obj: WorldObject) -> Self {
        Self {
            obj_type: obj,
            count: 1,
            rarity: ItemRarity::Common,
            attributes: ItemAttributes::default(),
            metadata: ItemDisplayMetaData::default(),
        }
    }
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

        add_item_glows(commands, &game.graphics, item, self.rarity.clone());
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
    pub fn is_stackable(&self, other: &Self) -> bool {
        self.obj_type == other.obj_type
            && self.attributes == other.attributes
            && self.metadata == other.metadata
            && self.rarity == other.rarity
    }
    pub fn add_to_inventory(
        self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        // if stack of that item exists, add to it, otherwise push as new stack.
        if let Some(stack) = container.items.iter().find(|i| match i {
            Some(ii) if ii.item_stack.count < MAX_STACK_SIZE => self.is_stackable(&ii.item_stack),
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
            mark_slot_dirty(inv_item_stack.slot, InventorySlotType::Normal, inv_slots);

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
        let slot = container.get_first_empty_slot();
        if let Some(slot) = slot {
            let item = InventoryItemStack {
                item_stack: self,
                slot,
            };
            item.add_to_container(container, InventorySlotType::Normal, inv_slots);
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

            item.add_to_container(container, InventorySlotType::Normal, inv_slots);

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
