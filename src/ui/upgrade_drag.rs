//! Drag-and-click upgrade flow for `UpgradeTome` / `OrbOfTransformation`.
//!
//! When the player is holding (dragging) a stack of upgrade materials and left-clicks on a
//! slot containing a piece of equipment — either in the main inventory grid / hotbar or in
//! one of the equipment / accessory / weapon / furnace slots — this system applies the
//! upgrade in place and consumes a single item from the dragged stack. This mirrors the
//! per-tick logic in [`crate::item::crafting::handle_furnace_slot_update`] but skips the
//! UI furnace flow entirely.
//!
//! Runs before [`handle_item_drop_clicks`] so the click is intercepted and the drop event
//! is not emitted on the same frame.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use rand::Rng;

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::{
            levelup_item_stats, reroll_item_bonus_attributes, spawn_rarity_animation,
        },
        ItemRarity, MAX_GEAR_LEVEL,
    },
    colors::{RED, WHITE},
    cursor::CursorPos,
    inventory::{Inventory, InventoryItemStack, ItemStack},
    item::WorldObject,
    juice::ShakeEffect,
    player::{skills::PlayerSkills, Player},
    proto::proto_param::ProtoParam,
    ui::{
        damage_numbers::spawn_floating_text_with_shadow_on_layer, game_fonts::FLOATING_TEXT,
        ui_helpers, DraggedItem, Interactable, Interaction, InventorySlotState, InventorySlotType,
        InventoryState, UIState,
    },
    TextureCamera,
};

/// Returns `true` when `obj` is a piece of gear that the furnace upgrade flow accepts
/// (weapons, armor, jewelry — anything listed in `FurnaceState::slot_map[1]`).
fn is_upgradeable_equipment(obj: WorldObject, inv_state: &InventoryState) -> bool {
    inv_state
        .furnace_state
        .slot_map
        .get(1)
        .map(|valid| valid.contains(&obj))
        .unwrap_or(false)
}

/// Slot types that can hold a piece of equipment we want to upgrade in place.
/// Trash / Crafting / CraftingInput / Chest / Scrapper are intentionally excluded.
fn slot_type_accepts_in_place_upgrade(slot_type: InventorySlotType) -> bool {
    matches!(
        slot_type,
        InventorySlotType::Normal
            | InventorySlotType::Hotbar
            | InventorySlotType::Equipment
            | InventorySlotType::Accessory
            | InventorySlotType::Weapon
            | InventorySlotType::Pet
            | InventorySlotType::Furnace
    )
}

#[derive(SystemParam)]
pub struct UpgradeDragAssets<'w> {
    pub asset_server: Res<'w, AssetServer>,
    pub graphics: Res<'w, Graphics>,
}

#[allow(clippy::too_many_arguments)]
pub fn handle_drag_upgrade_material_on_equipment(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    ui_state: Res<State<UIState>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut interactables: Query<(Entity, &mut Interactable)>,
    mut slot_states: Query<&mut InventorySlotState>,
    mut item_stacks: Query<&mut ItemStack>,
    dragged_query: Query<Entity, With<DraggedItem>>,
    mut inv: Query<&mut Inventory>,
    inv_state: Res<InventoryState>,
    proto: ProtoParam,
    assets: UpgradeDragAssets,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut legendary_rank_events: EventWriter<
        crate::player::combat_heirlooms::LegendaryEquipmentRankedEvent,
    >,
) {
    let asset_server = &assets.asset_server;
    if !ui_state.0.is_inv_open() {
        return;
    }
    if !mouse_input.just_pressed(MouseButton::Left) {
        return;
    }
    if dragged_query.iter().next().is_none() {
        return;
    }

    let Some(hit) = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None) else {
        return;
    };
    let hit_entity = hit.0;
    let Ok(target_slot_state) = slot_states.get(hit_entity).map(|s| s.clone()) else {
        return;
    };
    if !slot_type_accepts_in_place_upgrade(target_slot_state.r#type) {
        return;
    }
    // Furnace upgrade-material slot (index 0) is the original tome/orb slot — leave the
    // existing furnace flow alone there.
    if target_slot_state.r#type == InventorySlotType::Furnace && target_slot_state.slot_index == 0 {
        return;
    }

    // Find the dragging Interactable + its dragged item entity.
    let mut dragging: Option<(Entity, Entity)> = None;
    for (parent_e, interactable) in interactables.iter() {
        if let Interaction::Dragging { item, .. } = interactable.current() {
            dragging = Some((parent_e, *item));
            break;
        }
    }
    let Some((parent_e, item_e)) = dragging else {
        return;
    };

    let dragged_obj = match item_stacks.get(item_e) {
        Ok(stack) => stack.obj_type,
        Err(_) => return,
    };
    if !matches!(
        dragged_obj,
        WorldObject::UpgradeTome | WorldObject::OrbOfTransformation
    ) {
        return;
    }

    let mut inv = inv.single_mut();

    let target_stack_opt: Option<ItemStack> = {
        let container = inv.get_items_from_slot_type(target_slot_state.r#type);
        container
            .items
            .get(target_slot_state.slot_index)
            .and_then(|opt| opt.as_ref().map(|i| i.item_stack.clone()))
    };
    let Some(target_stack) = target_stack_opt else {
        return;
    };
    if !is_upgradeable_equipment(target_stack.obj_type, &inv_state) {
        return;
    }

    let gear_level = target_stack.metadata.level.unwrap_or(0);

    // Tome on max-level gear: floating warning, do not consume.
    if dragged_obj == WorldObject::UpgradeTome && gear_level >= MAX_GEAR_LEVEL {
        let text_pos = cursor_pos.ui_coords.truncate().extend(20.) + Vec3::new(50., 10., 0.);
        spawn_floating_text_with_shadow_on_layer(
            &mut commands,
            &asset_server,
            text_pos,
            WHITE,
            "Max Level Reached".to_string(),
            FLOATING_TEXT,
            bevy::render::view::RenderLayers::from_layers(&[3]),
        );
        mouse_input.clear();
        return;
    }

    let slot_type = target_slot_state.r#type;
    let slot_index = target_slot_state.slot_index;

    match dragged_obj {
        WorldObject::UpgradeTome => {
            let tome_double_count = player_skills
                .get_single()
                .map(|s| s.get_count(crate::player::skills::Heirloom::TomeDoubleUpgrade))
                .unwrap_or(0);
            let upgrade_count = 1 + tome_double_count;

            let container = inv.get_mut_items_from_slot_type(slot_type);
            let Some(existing) = container.items[slot_index].clone() else {
                return;
            };
            let mut new_stack = existing.item_stack.clone();
            for _ in 0..upgrade_count {
                new_stack = levelup_item_stats(&new_stack, 1, &proto, false);
            }
            container.items[slot_index] = Some(InventoryItemStack {
                item_stack: new_stack,
                slot: slot_index,
            });
            for _ in 0..upgrade_count {
                let leveled = container.items[slot_index].as_ref().unwrap().clone();
                leveled.modify_level(1, container);
            }
        }
        WorldObject::OrbOfTransformation => {
            let container = inv.get_mut_items_from_slot_type(slot_type);
            let Some(existing) = container.items[slot_index].clone() else {
                return;
            };
            let old_rarity = existing.item_stack.rarity.clone();
            let new_item = reroll_item_bonus_attributes(&existing.item_stack, &proto);
            let new_rarity = new_item.rarity.clone();
            let rarity_changed = new_rarity != old_rarity;
            container.items[slot_index] = Some(InventoryItemStack::new(new_item, existing.slot));

            if rarity_changed {
                let anim_pos = cursor_pos.ui_coords.truncate().extend(20.);
                spawn_rarity_animation(new_rarity.clone(), &mut commands, &asset_server, anim_pos);
                if new_rarity == ItemRarity::Legendary && old_rarity != ItemRarity::Legendary {
                    legendary_rank_events
                        .send(crate::player::combat_heirlooms::LegendaryEquipmentRankedEvent);
                    let mut rng = rand::thread_rng();
                    let seed = rng.gen_range(0..100000);
                    for e in game_camera.iter_mut() {
                        commands.entity(e).insert(ShakeEffect {
                            timer: Timer::from_seconds(2., TimerMode::Once),
                            speed: 10.,
                            seed,
                            max_mag: 90.,
                            noise: 0.5,
                            dir: Vec2::new(1., 1.),
                        });
                    }
                }
            }
        }
        _ => return,
    }

    // Mark all slot states matching this slot dirty so the UI refreshes the icon.
    for mut state in slot_states.iter_mut() {
        if state.slot_index == slot_index
            && (state.r#type == slot_type
                || (slot_type == InventorySlotType::Normal && state.r#type.is_hotbar())
                || (slot_type == InventorySlotType::Hotbar && state.r#type.is_inventory()))
        {
            state.dirty = true;
        }
    }

    // Consume one upgrade material from the dragged stack.
    let mut stack_empty = false;
    if let Ok(mut dragged_stack) = item_stacks.get_mut(item_e) {
        dragged_stack.modify_count(-1);
        stack_empty = dragged_stack.count == 0;
    }

    if stack_empty {
        commands.entity(item_e).despawn_recursive();
        if let Ok((_, mut parent_interactable)) = interactables.get_mut(parent_e) {
            parent_interactable.change(Interaction::None);
        }
    }
    // Otherwise leave the dragged icon alone — `update_dragged_item_stack_count_text`
    // will refresh the label from the mutated `ItemStack::count` next frame.

    // Consume the click so `handle_item_drop_clicks` doesn't also emit a drop event this frame.
    mouse_input.clear();
}
