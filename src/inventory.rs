use core::panic;
use std::cmp::min;

use crate::{
    animations::{AnimationPosTracker, AnimationTimer},
    ecs_helpers::SafeHierarchyExt,
    attributes::{add_item_glows, AttributeModifier, ItemAttributes, ItemRarity},
    container::{main_inv_bag_slot_indices_top_to_bottom, Container},
    inputs::FacingDirection,
    item::{
        item_actions::{proto_item_allows_hotbar_band, ItemActions},
        ActiveMainHandState, Equipment, EquipmentType, ItemDisplayMetaData, ItemDrop, MainHand,
        WorldObject, PLAYER_EQUIPMENT_POSITIONS,
    },
    player::Limb,
    proto::proto_param::ProtoParam,
    ui::{mark_slot_dirty, InventorySlotState, InventorySlotType, UIContainersParam},
    world::y_sort::YSort,
    GameParam,
};
use rand::Rng;

use bevy::prelude::*;

use crate::item::ammo::Ammo;
use bevy_proto::prelude::*;
use serde::{Deserialize, Serialize};

pub const INVENTORY_SIZE: usize = 8 * 4;
/// First this many `Inventory::items` indices are the quickbar (keys 1–4 + two extra storage slots).
pub const INVENTORY_HOTBAR_SLOTS: usize = 4;
/// Keyed row (`0..INVENTORY_HOTBAR_SLOTS`) plus two passive cells (`4` and `5`); consumables only.
pub const INVENTORY_HOTBAR_BAND_SLOTS: usize = 4;
pub const MAX_STACK_SIZE: usize = 9999;

#[derive(Component, Debug, Default, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub items: Container,
    pub equipment_items: Container,
    pub accessory_items: Container,
    /// Single-slot container for the player's currently-equipped weapon.
    /// Drives `PlayerState::main_hand_slot` (see `update_attributes_with_held_item_change`).
    #[serde(default = "default_single_slot_container")]
    pub weapon_items: Container,
    /// Single-slot container for the pet's equipped weapon.
    /// Drives `PetState::weapon_slot` (see `update_pet_weapon_on_inv_change`).
    #[serde(default = "default_single_slot_container")]
    pub pet_items: Container,
    pub crafting_items: Container,
    pub furnace_items: Container,
    pub trash_items: Container,
    /// Three input slots rendered on the "CRAFTING" side panel (inventory → crafting mode toggle).
    /// Kept separate from `crafting_items` (the recipe strip) and `furnace_items` (tome/orb upgrade).
    #[serde(default = "default_crafting_inputs_container")]
    pub crafting_inputs_items: Container,
    // pub crafting_result_item: Container,
}

/// Serde default for single-slot containers (used by migration of older saves
/// that predate the `weapon_items` / `pet_items` fields).
fn default_single_slot_container() -> Container {
    Container::with_size(1)
}
/// Serde default for `crafting_inputs_items` so saves predating this container deserialize cleanly.
fn default_crafting_inputs_container() -> Container {
    Container::with_size(3)
}
impl Inventory {
    pub fn is_empty(&self) -> bool {
        self.items
            .items
            .iter()
            .flatten()
            .collect::<Vec<_>>()
            .is_empty()
    }
    pub fn get_items_from_slot_type(&self, slot_type: InventorySlotType) -> &Container {
        match slot_type {
            InventorySlotType::Equipment => &self.equipment_items,
            InventorySlotType::Accessory => &self.accessory_items,
            InventorySlotType::Weapon => &self.weapon_items,
            InventorySlotType::Pet => &self.pet_items,
            InventorySlotType::Crafting => &self.crafting_items,
            InventorySlotType::Furnace => &self.furnace_items,
            InventorySlotType::Trash => &self.trash_items,
            InventorySlotType::CraftingInput => &self.crafting_inputs_items,
            _ => &self.items,
        }
    }
    pub fn get_mut_items_from_slot_type(&mut self, slot_type: InventorySlotType) -> &mut Container {
        match slot_type {
            InventorySlotType::Equipment => &mut self.equipment_items,
            InventorySlotType::Accessory => &mut self.accessory_items,
            InventorySlotType::Weapon => &mut self.weapon_items,
            InventorySlotType::Pet => &mut self.pet_items,
            InventorySlotType::Crafting => &mut self.crafting_items,
            InventorySlotType::Furnace => &mut self.furnace_items,
            InventorySlotType::Trash => &mut self.trash_items,
            InventorySlotType::CraftingInput => &mut self.crafting_inputs_items,
            _ => &mut self.items,
        }
    }

    /// True if any slot in the main grid, equipment, accessory, weapon, or pet containers holds
    /// an item with this [`EquipmentType`] (e.g. [`EquipmentType::Axe`] for trees).
    pub fn has_equipment_type(&self, required: &EquipmentType, proto: &ProtoParam) -> bool {
        let containers = [
            &self.items,
            &self.equipment_items,
            &self.accessory_items,
            &self.weapon_items,
            &self.pet_items,
        ];
        for container in containers {
            for slot in container.items.iter().flatten() {
                let obj = slot.item_stack.obj_type;
                if let Some(eq_type) = proto.get_component::<EquipmentType, _>(obj) {
                    if eq_type == required {
                        return true;
                    }
                }
            }
        }
        false
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
        // Trash slots always overwrites
        if slot_type.is_trash() {
            container.items[self.slot] = Some(self.clone());
            mark_slot_dirty(self.slot, slot_type, inv_slots);
            return None;
        }

        let obj_type = self.item_stack.obj_type;
        let target_item_option = container.items[self.slot].clone();
        if let Some(target_item) = target_item_option {
            if target_item.get_obj() == &obj_type
                && target_item.item_stack.metadata == self.item_stack.metadata
                && target_item.item_stack.attributes == self.item_stack.attributes
                && !(slot_type.is_equipment()
                    || slot_type.is_accessory()
                    || slot_type.is_weapon_slot()
                    || slot_type.is_pet_slot())
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
            if let Some(mut entity_commands) = commands.get_entity(main_hand_data.entity) {
                entity_commands.despawn();
            }
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
            .safe_set_parent(player_e)
            .id();

        let mut item_entity = commands.entity(item);

        item_entity.insert(MainHand);
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

        // Initialize Ammo for non-magic ranged weapons at equip time
        let (max, reload_s) = obj.get_ammo();
        if max > 0 {
            let mut new = Ammo::new(max, reload_s);
            new.start_reload();
            item_entity.insert(new);
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

    pub fn write_to_container(&self, container: &mut Container) {
        container.items[self.slot] = Some(self.clone());
    }

    pub fn add_to_container(
        &self,
        container: &mut Container,
        slot_type: InventorySlotType,
        inv_slots: &mut Query<&mut InventorySlotState>,
    ) {
        self.write_to_container(container);
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
    pub fn modify_count(&mut self, amount: i32) -> Option<Self> {
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
            return ui_cont_param.inv_state.furnace_state.slot_map[self.slot]
                .contains(&self.item_stack.obj_type);
        }
        // Weapon / Pet slots: accept any item whose `WorldObject::is_weapon()` is true.
        // Both slots are size 1, so `self.slot` must be 0.
        if slot_type.is_weapon_slot() || slot_type.is_pet_slot() {
            return self.slot == 0 && self.item_stack.obj_type.is_weapon();
        }
        if slot_type.is_inventory() || slot_type.is_hotbar() {
            if self.slot < INVENTORY_HOTBAR_BAND_SLOTS {
                return proto_item_allows_hotbar_band(self.item_stack.obj_type, proto_param);
            }
            return true;
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
            .insert(ItemDrop)
            .insert(Name::new("DropItem"))
            .insert(self.clone())
            .insert(AnimationTimer(Timer::from_seconds(
                0.1,
                TimerMode::Repeating,
            )))
            .insert(AnimationPosTracker(0., 0., 0.3))
            .insert(YSort(0.))
            .insert(obj)
            .insert(crate::item::ItemDropDespawnTimer(Timer::from_seconds(
                20.0,
                TimerMode::Once,
            )))
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

    /// Get the item attributes, reconstructing from stat lines if they exist
    /// This ensures attributes are always in sync with stat lines
    pub fn get_attributes(&self) -> ItemAttributes {
        if !self.metadata.bonus_stat_lines.is_empty() {
            // Reconstruct from stat lines to ensure consistency
            ItemAttributes::from_stat_lines(&self.metadata.bonus_stat_lines)
        } else {
            // Fall back to stored attributes
            self.attributes.clone()
        }
    }
    pub fn add_to_inventory(
        self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
        proto: &ProtoParam,
    ) {
        // if stack of that item exists, add to it, otherwise push as new stack.
        if let Some((_, stack_cell)) = container.items.iter().enumerate().find(|(slot_idx, i)| {
            let skip_band = container.items.len() == INVENTORY_SIZE
                && *slot_idx < INVENTORY_HOTBAR_BAND_SLOTS
                && !proto_item_allows_hotbar_band(self.obj_type, proto);
            if skip_band {
                return false;
            }
            match i {
                Some(ii) if ii.item_stack.count < MAX_STACK_SIZE => {
                    self.is_stackable(&ii.item_stack)
                }
                _ => false,
            }
        }) {
            // safe to unwrap, we check for it above
            let slot = stack_cell.as_ref().unwrap().slot;
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
                    proto,
                );
            }
        } else {
            Self::add_to_empty_inventory_slot(self, container, inv_slots, proto);
        }
    }
    pub fn add_to_empty_inventory_slot(
        self,
        container: &mut Container,
        inv_slots: &mut Query<&mut InventorySlotState>,
        proto: &ProtoParam,
    ) {
        let slot = container.get_first_empty_player_slot_for_pickup(&self, proto);
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
                existing_stack.modify_count(self.count as i32);
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
    pub fn get_copy_with_modified_attributes(&self, modifier: AttributeModifier) -> Self {
        self.clone()
            .copy_with_attributes(self.attributes.clone().change_attribute(modifier))
    }
    pub fn modify_count(&mut self, amount: i32) -> Self {
        // NOTE: Do all math in i64 to avoid silently truncating `self.count`.
        // The previous implementation cast `self.count` (usize, up to MAX_STACK_SIZE = 9999)
        // down to `i8`, which wiped large stacks during crafting (e.g. 256 logs → 0 left
        // when crafting a bridge that should only consume 4).
        let new_count = (self.count as i64) + (amount as i64);
        self.count = new_count.max(0) as usize;
        self.clone()
    }
}

/// Marker tag for the "Sort" button rendered under the trash slot in the inventory UI.
/// Click handler lives in `ui::interactions::handle_sort_inventory_button_click`.
#[derive(Component, Default, Clone, Debug)]
pub struct SortInventoryButton;

/// Toggles whether breakable world entities without [`crate::enemy::Mob`] spawn loot (chest
/// contents and loot-table item drops). XP from [`crate::player::levels::ExperienceReward`]
/// is unchanged. Read in `item::handle_break_object`; toggled from the inventory UI.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct SuppressNonMobBreakDrops(pub bool);

/// Marker for the material-drops toggle slot under the sort button (`handle_material_drops_toggle_button_click`).
#[derive(Component, Clone, Debug)]
pub struct MaterialDropsToggleButton;

/// Red "X" overlay child; visible when [`SuppressNonMobBreakDrops`] is true.
#[derive(Component, Clone, Debug)]
pub struct MaterialDropsToggleXOverlay;

/// Sort priority bucket for the inventory-sort button. Lower values sort first.
/// Sub-field ordering is alphabetical on the item's display name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SortBucket {
    Weapon,
    Chestplate,
    Pants,
    Shoes,
    Head,
    Ring,
    Pendant,
    Trinket,
    Cape,
    Consumable,
    Material,
}

/// Returns true if an item's [`ItemActions`] list represents a consumable-style usable
/// (food / potion / magic gem / key / etc.) — i.e. an item whose primary purpose is to be
/// *used* from the inventory rather than placed in the world. Items whose only actions
/// place a block (`PlacesInto`) fall through to the `Material` bucket.
fn item_actions_are_consumable(actions: &ItemActions) -> bool {
    actions.actions.iter().len() > 0
}

fn sort_bucket_for_item(stack: &ItemStack, proto: &ProtoParam) -> SortBucket {
    let obj = stack.obj_type;
    let equip_type = proto.get_component::<EquipmentType, _>(obj).cloned();
    match equip_type {
        Some(EquipmentType::Weapon) => return SortBucket::Weapon,
        Some(EquipmentType::Chest) => return SortBucket::Chestplate,
        Some(EquipmentType::Legs) => return SortBucket::Pants,
        Some(EquipmentType::Feet) => return SortBucket::Shoes,
        Some(EquipmentType::Head) => return SortBucket::Head,
        Some(EquipmentType::Ring) => return SortBucket::Ring,
        Some(EquipmentType::Pendant) => return SortBucket::Pendant,
        Some(EquipmentType::Trinket) => return SortBucket::Trinket,
        Some(EquipmentType::Cape) => return SortBucket::Cape,
        _ => {}
    }
    // Fallback on hardcoded weapon list (items that predate EquipmentType metadata).
    if obj.is_weapon() {
        return SortBucket::Weapon;
    }
    if let Some(actions) = proto.get_component::<ItemActions, _>(obj) {
        if item_actions_are_consumable(actions) {
            return SortBucket::Consumable;
        }
    }
    SortBucket::Material
}

/// Alphabetical sub-key for items within the same [`SortBucket`]. Uses the display name when
/// present, falling back to the `WorldObject` debug name so sorts are stable across saves even
/// before an item's prototype has been inspected (and metadata.name populated).
fn sort_sub_key(stack: &ItemStack) -> String {
    let name = stack.metadata.name.trim();
    if name.is_empty() {
        format!("{:?}", stack.obj_type)
    } else {
        name.to_ascii_lowercase()
    }
}

/// Sorts the player's main inventory grid in place. Hotbar slots
/// (`0..INVENTORY_HOTBAR_SLOTS`) are left untouched so the player's quickbar layout is
/// preserved. Bag slots are re-packed in **visual** top-to-bottom order (row-major indices are
/// bottom-up in memory; see [`crate::container::main_inv_bag_slot_indices_top_to_bottom`]) using
/// the ordering defined by [`SortBucket`] and then by display name. Stackable duplicates are
/// merged (up to [`MAX_STACK_SIZE`]) so the bag shrinks as much as possible. All affected
/// inventory + hotbar slots are marked dirty so the UI respawns them.
pub fn sort_main_inventory(
    inv: &mut Inventory,
    proto: &ProtoParam,
    inv_slots: &mut Query<&mut InventorySlotState>,
) {
    let len = inv.items.items.len();
    if len <= INVENTORY_HOTBAR_SLOTS {
        return;
    }

    // Collect and clear the bag slots (keep hotbar indices intact).
    let mut stacks: Vec<ItemStack> = inv.items.items[INVENTORY_HOTBAR_SLOTS..len]
        .iter_mut()
        .filter_map(|s| s.take().map(|is| is.item_stack))
        .collect();

    // Merge stackable duplicates before sorting so repeated bucket/name pairs collapse.
    let mut merged: Vec<ItemStack> = Vec::with_capacity(stacks.len());
    'outer: for stack in stacks.drain(..) {
        for existing in merged.iter_mut() {
            if existing.is_stackable(&stack) && existing.count < MAX_STACK_SIZE {
                let space = MAX_STACK_SIZE - existing.count;
                let take = stack.count.min(space);
                existing.count += take;
                let leftover = stack.count - take;
                if leftover > 0 {
                    merged.push(stack.copy_with_count(leftover));
                }
                continue 'outer;
            }
        }
        merged.push(stack);
    }

    merged.sort_by(|a, b| {
        let ba = sort_bucket_for_item(a, proto);
        let bb = sort_bucket_for_item(b, proto);
        ba.cmp(&bb)
            .then_with(|| sort_sub_key(a).cmp(&sort_sub_key(b)))
    });

    let fill_order = main_inv_bag_slot_indices_top_to_bottom(len);
    for (i, stack) in merged.into_iter().enumerate() {
        let Some(&slot) = fill_order.get(i) else {
            break;
        };
        inv.items.items[slot] = Some(InventoryItemStack {
            item_stack: stack,
            slot,
        });
    }

    // Evict gear/materials from quick-access band slots (keys 1–4 row + two passive cells).
    for slot in 0..INVENTORY_HOTBAR_BAND_SLOTS.min(len) {
        let Some(cell) = inv.items.items[slot].clone() else {
            continue;
        };
        if proto_item_allows_hotbar_band(cell.item_stack.obj_type, proto) {
            continue;
        }
        let stack_ref = &cell.item_stack;
        if inv
            .items
            .get_first_empty_player_slot_for_pickup(stack_ref, proto)
            .is_none()
            && inv
                .items
                .get_slot_for_item_in_container_with_space_for_pickup(stack_ref, None, proto)
                .is_none()
        {
            continue;
        }
        inv.items.items[slot] = None;
        cell.item_stack
            .add_to_inventory(&mut inv.items, inv_slots, proto);
    }

    // Mark every visible inventory slot (Normal + Hotbar) dirty so the UI respawns icons.
    for mut state in inv_slots.iter_mut() {
        if state.r#type.is_inventory() || state.r#type.is_hotbar() {
            state.dirty = true;
        }
    }
}

/// True if a weapon pickup could go into an empty main-weapon or pet slot (so we should not
/// reject the pickup when the main grid is full). See [`try_auto_equip_weapon_on_pickup`].
pub fn can_auto_equip_weapon_on_pickup(
    item_stack: &ItemStack,
    inventory: &Inventory,
    player_has_pet: bool,
) -> bool {
    if !item_stack.obj_type.is_weapon() {
        return false;
    }
    if inventory
        .weapon_items
        .items
        .get(0)
        .map_or(true, |s| s.is_none())
    {
        return true;
    }
    player_has_pet
        && inventory
            .pet_items
            .items
            .get(0)
            .map_or(true, |s| s.is_none())
}

/// Whether [`check_item_drop_collisions`] would accept this stack (currency / XP / mana bypass
/// inventory; otherwise same empty-slot / merge / weapon-auto-equip rules).
pub fn player_can_accept_ground_item_pickup(
    item_stack: &ItemStack,
    inventory: &Inventory,
    player_has_pet: bool,
    proto: &ProtoParam,
) -> bool {
    let obj = item_stack.obj_type;
    if obj == WorldObject::TimeFragment
        || obj == WorldObject::Coin
        || obj == WorldObject::ManaOrb
        || obj == WorldObject::XPShard
        || obj == WorldObject::XPShardMedium
        || obj == WorldObject::XPShardLarge
    {
        return true;
    }
    if can_auto_equip_weapon_on_pickup(item_stack, inventory, player_has_pet) {
        return true;
    }
    let inv_container = &inventory.items;
    inv_container
        .get_first_empty_player_slot_for_pickup(item_stack, proto)
        .is_some()
        || inv_container
            .get_slot_for_item_in_container_with_space_for_pickup(item_stack, None, proto)
            .is_some()
}

/// Places a weapon stack into the main weapon slot if empty, else into the pet slot if the
/// player has a pet and that slot is empty. Returns `true` if the stack was stored (caller
/// should not add it to the main grid).
pub fn try_auto_equip_weapon_on_pickup(
    item_stack: ItemStack,
    inventory: &mut Inventory,
    inv_slots: &mut Query<&mut InventorySlotState>,
    player_has_pet: bool,
) -> bool {
    if !item_stack.obj_type.is_weapon() {
        return false;
    }
    if inventory
        .weapon_items
        .items
        .get(0)
        .map_or(true, |s| s.is_none())
    {
        InventoryItemStack {
            item_stack,
            slot: 0,
        }
        .add_to_container(
            &mut inventory.weapon_items,
            InventorySlotType::Weapon,
            inv_slots,
        );
        return true;
    }
    if player_has_pet
        && inventory
            .pet_items
            .items
            .get(0)
            .map_or(true, |s| s.is_none())
    {
        InventoryItemStack {
            item_stack,
            slot: 0,
        }
        .add_to_container(&mut inventory.pet_items, InventorySlotType::Pet, inv_slots);
        return true;
    }
    false
}

/// Slot index on [`Inventory::furnace_items`] that accepts the equipment piece being upgraded
/// (tomes / orbs live at index 0). Kept here so UI glue and auto-equip logic stay in sync.
pub const UPGRADE_EQUIPMENT_SLOT_INDEX: usize = 1;

/// Cape occupies equipment grid index 3 (see [`crate::ui::inventory_ui::equipment_grid_cell`]);
/// [`EquipmentType::get_valid_slots`] does not list it, so auto-equip handles it explicitly.
const CAPE_EQUIPMENT_SLOT_INDEX: usize = 3;

fn resolve_equipment_type_for_auto_equip(
    obj: WorldObject,
    proto: &ProtoParam,
) -> Option<EquipmentType> {
    match proto.get_component::<EquipmentType, _>(obj).cloned() {
        Some(EquipmentType::None) | None => {
            if obj.is_weapon() {
                Some(EquipmentType::Weapon)
            } else if obj.is_cape() {
                Some(EquipmentType::Cape)
            } else {
                match obj {
                    WorldObject::Ring => Some(EquipmentType::Ring),
                    WorldObject::Pendant => Some(EquipmentType::Pendant),
                    _ => None,
                }
            }
        }
        Some(t) => Some(t),
    }
}

/// If the player left gear in the upgrade slot (`furnace_items[1]`) and then toggles to the
/// crafting panel (which hides the upgrade slots), move that item into the first empty weapon /
/// armor / accessory slot it fits. If no appropriate slot is empty the item stays so the player
/// still sees it when they toggle back.
///
/// Returns `true` if the item was moved; both the source and destination slots are marked dirty
/// so the UI respawns their icons on the next `update_inventory_ui` tick.
pub fn try_auto_equip_from_upgrade_slot(
    inv: &mut Inventory,
    proto: &ProtoParam,
    inv_slots: &mut Query<&mut InventorySlotState>,
) -> bool {
    let Some(stack_entry) = inv
        .furnace_items
        .items
        .get(UPGRADE_EQUIPMENT_SLOT_INDEX)
        .cloned()
        .flatten()
    else {
        return false;
    };

    let item_stack = stack_entry.item_stack;
    let obj = item_stack.obj_type;
    let Some(eq_type) = resolve_equipment_type_for_auto_equip(obj, proto) else {
        return false;
    };

    // [`EquipmentType::is_equipment`] only means armor (Head/Chest/Legs/Feet); weapons and
    // accessories must still auto-equip here.

    let target_slot_type = eq_type.get_valid_slot_type();
    if target_slot_type == InventorySlotType::Normal {
        return false;
    }
    let valid_slots = eq_type.get_valid_slots();
    if valid_slots.is_empty() {
        return false;
    }

    let Some(dest_slot) = valid_slots.into_iter().find(|&i| {
        inv.get_items_from_slot_type(target_slot_type)
            .items
            .get(i)
            .is_some_and(|s| s.is_none())
    }) else {
        return false;
    };

    inv.furnace_items.items[UPGRADE_EQUIPMENT_SLOT_INDEX] = None;
    mark_slot_dirty(
        UPGRADE_EQUIPMENT_SLOT_INDEX,
        InventorySlotType::Furnace,
        inv_slots,
    );

    InventoryItemStack {
        item_stack,
        slot: dest_slot,
    }
    .add_to_container(
        inv.get_mut_items_from_slot_type(target_slot_type),
        target_slot_type,
        inv_slots,
    );

    true
}

/// Result of attempting a shift-click quick-equip from a container slot into weapon / armor /
/// accessory slots (mirrors [`try_auto_equip_from_upgrade_slot`] destination rules).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShiftQuickEquipResult {
    /// Not a weapon or equippable piece (caller may run hotbar / container moves).
    NotEquippable,
    /// Moved into an equip slot; source slot was cleared and marked dirty.
    Equipped,
    /// Weapon or gear, but every valid equip slot is full — do nothing else (no hotbar move).
    NoEmptyEquipSlot,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InventoryShiftClickSource {
    MainGrid,
    CraftingInputs,
}

fn take_item_from_shift_source(
    inv: &mut Inventory,
    source: InventoryShiftClickSource,
    src_slot: usize,
) -> Option<InventoryItemStack> {
    match source {
        InventoryShiftClickSource::MainGrid => {
            inv.items.items.get_mut(src_slot).and_then(|s| s.take())
        }
        InventoryShiftClickSource::CraftingInputs => inv
            .crafting_inputs_items
            .items
            .get_mut(src_slot)
            .and_then(|s| s.take()),
    }
}

/// Shift-click: move gear from the main grid or crafting input slots into the first empty
/// matching equip slot. Weapons prefer the main hand, then the pet slot when applicable.
///
/// Caller should set the source slot’s `InventorySlotState::dirty` when the result is not
/// [`ShiftQuickEquipResult::NotEquippable`]; equip destinations refresh via `update_inventory_ui`
/// when stored counts diverge from UI state.
pub fn try_shift_quick_equip_from_inventory_source(
    inv: &mut Inventory,
    source: InventoryShiftClickSource,
    src_slot: usize,
    proto: &ProtoParam,
    player_has_pet: bool,
) -> ShiftQuickEquipResult {
    let stack_entry = match source {
        InventoryShiftClickSource::MainGrid => inv.items.items.get(src_slot).cloned().flatten(),
        InventoryShiftClickSource::CraftingInputs => inv
            .crafting_inputs_items
            .items
            .get(src_slot)
            .cloned()
            .flatten(),
    };
    let Some(stack_entry) = stack_entry else {
        return ShiftQuickEquipResult::NotEquippable;
    };
    let item_stack = stack_entry.item_stack;

    if item_stack.obj_type.is_weapon() {
        if inv.weapon_items.items.get(0).map_or(true, |s| s.is_none()) {
            InventoryItemStack {
                item_stack: item_stack.clone(),
                slot: 0,
            }
            .write_to_container(&mut inv.weapon_items);
            take_item_from_shift_source(inv, source, src_slot);
            return ShiftQuickEquipResult::Equipped;
        }
        if player_has_pet && inv.pet_items.items.get(0).map_or(true, |s| s.is_none()) {
            InventoryItemStack {
                item_stack: item_stack.clone(),
                slot: 0,
            }
            .write_to_container(&mut inv.pet_items);
            take_item_from_shift_source(inv, source, src_slot);
            return ShiftQuickEquipResult::Equipped;
        }
        return ShiftQuickEquipResult::NoEmptyEquipSlot;
    }

    let Some(eq_type) = resolve_equipment_type_for_auto_equip(item_stack.obj_type, proto) else {
        return ShiftQuickEquipResult::NotEquippable;
    };

    if eq_type.is_cape() {
        if inv
            .equipment_items
            .items
            .get(CAPE_EQUIPMENT_SLOT_INDEX)
            .is_some_and(|s| s.is_none())
        {
            let taken = take_item_from_shift_source(inv, source, src_slot).unwrap();
            InventoryItemStack {
                item_stack: taken.item_stack,
                slot: CAPE_EQUIPMENT_SLOT_INDEX,
            }
            .write_to_container(&mut inv.equipment_items);
            return ShiftQuickEquipResult::Equipped;
        }
        return ShiftQuickEquipResult::NoEmptyEquipSlot;
    }

    let target_slot_type = eq_type.get_valid_slot_type();
    if target_slot_type == InventorySlotType::Normal {
        return ShiftQuickEquipResult::NotEquippable;
    }

    let valid_slots = eq_type.get_valid_slots();
    if valid_slots.is_empty() {
        return ShiftQuickEquipResult::NoEmptyEquipSlot;
    }

    let Some(dest_slot) = valid_slots.into_iter().find(|&i| {
        inv.get_items_from_slot_type(target_slot_type)
            .items
            .get(i)
            .is_some_and(|s| s.is_none())
    }) else {
        return ShiftQuickEquipResult::NoEmptyEquipSlot;
    };

    let taken = take_item_from_shift_source(inv, source, src_slot).unwrap();
    InventoryItemStack {
        item_stack: taken.item_stack,
        slot: dest_slot,
    }
    .write_to_container(inv.get_mut_items_from_slot_type(target_slot_type));
    ShiftQuickEquipResult::Equipped
}

/// Shift-click from an equipment / accessory / weapon / pet slot into the main grid (`items`),
/// merging with partial stacks or using the first empty slot when possible.
pub fn shift_move_equipped_slot_to_main_items(
    inv: &mut Inventory,
    slot_type: InventorySlotType,
    slot_index: usize,
    proto: &ProtoParam,
) {
    match slot_type {
        InventorySlotType::Equipment => {
            inv.equipment_items.move_item_to_target_container(
                &mut inv.items,
                slot_index,
                Some(proto),
            );
        }
        InventorySlotType::Accessory => {
            inv.accessory_items.move_item_to_target_container(
                &mut inv.items,
                slot_index,
                Some(proto),
            );
        }
        InventorySlotType::Weapon => {
            inv.weapon_items
                .move_item_to_target_container(&mut inv.items, slot_index, Some(proto));
        }
        InventorySlotType::Pet => {
            inv.pet_items
                .move_item_to_target_container(&mut inv.items, slot_index, Some(proto));
        }
        _ => {}
    }
}
