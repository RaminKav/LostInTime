use bevy::prelude::*;

use strum_macros::{Display, EnumIter};

use crate::{
    assets::Graphics,
    inputs::CursorPos,
    inventory::{Inventory, InventoryPlugin, ItemStack},
    item::{CompleteRecipeEvent, CraftingSlotUpdateEvent},
    GameParam,
};

use super::{spawn_item_stack_icon, ui_helpers, InventorySlotState, InventoryState};

#[derive(Component, Debug, EnumIter, Display, Hash, PartialEq, Eq)]
pub enum UIElement {
    Inventory,
    ChestInventory,
    InventorySlot,
    InventorySlotHover,
    HealthBarFrame,
    Tooltip,
    LargeTooltip,
    Minimap,
    LargeMinimap,
}

#[derive(Component, Debug, Clone)]
pub struct Interactable {
    state: Interaction,
    previous_state: Interaction,
}
impl Default for Interactable {
    fn default() -> Self {
        Self {
            state: Interaction::None,
            previous_state: Interaction::None,
        }
    }
}
impl Interactable {
    pub fn from_state(state: Interaction) -> Self {
        Self {
            state,
            previous_state: Interaction::None,
        }
    }
    pub fn current(&self) -> &Interaction {
        &self.state
    }
    pub fn previous(&self) -> &Interaction {
        &self.previous_state
    }
    fn change(&mut self, new_state: Interaction) {
        std::mem::swap(&mut self.previous_state, &mut self.state);
        self.state = new_state;
    }
}
#[derive(Component, Default, Debug, Clone)]
pub enum Interaction {
    #[default]
    None,
    Hovering,
    Dragging {
        item: Entity,
    },
}

#[derive(Component)]
pub struct DraggedItem;

#[derive(Debug, Clone)]

pub struct DropOnSlotEvent {
    dropped_entity: Entity,
    dropped_item_stack: ItemStack,
    drop_target_slot_state: InventorySlotState,
    parent_interactable_entity: Entity,
    stack_empty: bool,
}
#[derive(Debug, Clone)]

pub struct ToolTipUpdateEvent {
    pub item_stack: ItemStack,
    pub parent_slot_entity: Entity,
}
#[derive(Debug, Clone)]

pub struct DropInWorldEvent {
    pub dropped_entity: Entity,
    pub dropped_item_stack: ItemStack,
    pub parent_interactable_entity: Entity,
    pub stack_empty: bool,
}
#[derive(Resource)]
pub struct LastHoveredSlot {
    pub slot: Option<usize>,
}
#[derive(Component)]
pub struct HealthBar;

#[derive(Component)]
pub struct FPSText;

pub fn handle_drop_in_world_events(
    mut events: EventReader<DropInWorldEvent>,
    mut game_param: GameParam,
    mut commands: Commands,
    mut interactables: Query<(Entity, &UIElement, &mut Interactable)>,
    item_stacks: Query<&ItemStack>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    for drop_event in events.iter() {
        let p = ui_helpers::get_player_chunk_tile_coords(&mut game_param.game);
        drop_event.dropped_item_stack.obj_type.spawn_item_drop(
            &mut commands,
            &mut game_param,
            p.1,
            p.0,
            drop_event.dropped_item_stack.count,
            Some(drop_event.dropped_item_stack.attributes.clone()),
        );
        commands
            .entity(drop_event.dropped_entity)
            .despawn_recursive();

        if let Ok(mut parent_interactable) =
            interactables.get_mut(drop_event.parent_interactable_entity)
        {
            if drop_event.stack_empty {
                commands
                    .entity(drop_event.dropped_entity)
                    .despawn_recursive();

                parent_interactable.2.change(Interaction::None);
            } else {
                let new_drag_icon_entity = spawn_item_stack_icon(
                    &mut commands,
                    &graphics,
                    item_stacks.get(drop_event.dropped_entity).unwrap(),
                    &asset_server,
                );

                commands.entity(new_drag_icon_entity).insert(DraggedItem);
                parent_interactable.2.change(Interaction::Dragging {
                    item: new_drag_icon_entity,
                });
            }
        }
    }
}
pub fn handle_drop_on_slot_events(
    mut events: EventReader<DropOnSlotEvent>,
    mut game: GameParam,
    mut commands: Commands,
    mut interactables: Query<(Entity, &UIElement, &mut Interactable)>,
    item_stacks: Query<&ItemStack>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    mut crafting_slot_event: EventWriter<CraftingSlotUpdateEvent>,
    mut complete_recipe_event: EventWriter<CompleteRecipeEvent>,
) {
    for drop_event in events.iter() {
        // all we need to do here is swap spots in the inventory
        let no_more_dragging: bool;
        let return_item = if drop_event
            .drop_target_slot_state
            .r#type
            .is_crafting_result()
        {
            InventoryPlugin::pick_up_and_merge_crafting_result_stack(
                drop_event.dropped_item_stack.clone(),
                &mut inv,
                &mut complete_recipe_event,
            )
        } else {
            InventoryPlugin::drop_item_on_slot(
                drop_event.dropped_item_stack.clone(),
                drop_event.drop_target_slot_state.slot_index,
                &mut inv,
                &mut game.inv_slot_query,
                drop_event.drop_target_slot_state.r#type,
            )
        };

        let updated_drag_item;
        if let Some(return_item) = return_item {
            updated_drag_item = return_item;
            no_more_dragging = false;
        } else {
            updated_drag_item = item_stacks.get(drop_event.dropped_entity).unwrap().clone();
            no_more_dragging = drop_event.stack_empty;
        }

        if drop_event.drop_target_slot_state.r#type.is_crafting() {
            crafting_slot_event.send(CraftingSlotUpdateEvent);
        }

        // if nothing left on cursor and dragging is done
        // despawn parent stack icon entity, set parent slot to no dragging

        commands
            .entity(drop_event.dropped_entity)
            .despawn_recursive();
        if let Ok(mut parent_interactable) =
            interactables.get_mut(drop_event.parent_interactable_entity)
        {
            if no_more_dragging {
                parent_interactable.2.change(Interaction::None);
            } else {
                let new_drag_icon_entity = spawn_item_stack_icon(
                    &mut commands,
                    &graphics,
                    &updated_drag_item,
                    &asset_server,
                );

                commands.entity(new_drag_icon_entity).insert(DraggedItem);
                parent_interactable.2.change(Interaction::Dragging {
                    item: new_drag_icon_entity,
                });
            }
        }
    }
}

pub fn handle_dragging(
    cursor_pos: Res<CursorPos>,
    mut interactables: Query<(Entity, &mut Interactable)>,
    mut drag_query: Query<(Entity, &mut Transform)>,
) {
    // iter all interactables, find ones in dragging.
    // set translation to cursor, and bring them to the top z layer so they render on top of everything

    // check things that were just dropped (.previous == dragging)
    // check cursor pos to see if it dropped on top of another item
    // if so, swap their places in the inventory
    // if dropped outside inventory space, remove item and spawn dropped entity item stack
    // else, it is an invalid drag, reset the its original location
    for (_e, interactable) in interactables.iter_mut() {
        match interactable.current() {
            Interaction::Dragging { item, .. } => {
                if let Ok((_e, mut t)) = drag_query.get_mut(*item) {
                    t.translation = cursor_pos.ui_coords.truncate().extend(998.);
                }
            }
            _ => {}
        }
    }
}
pub fn handle_hovering(
    mut interactables: Query<(Entity, &UIElement, &mut Interactable, &InventorySlotState)>,
    tooltips: Query<(Entity, &UIElement, &Parent), Without<InventorySlotState>>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    inv: Query<&Inventory>,
    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
) {
    // iter all interactables, find ones in hover state.
    // match the UIElement type to swap to a new image
    for (e, ui, interactable, state) in interactables.iter_mut() {
        if let Interaction::Hovering = interactable.current() {
            if ui == &UIElement::InventorySlot {
                // swap to hover img
                commands.entity(e).insert(UIElement::InventorySlotHover);
                commands.entity(e).insert(
                    graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&UIElement::InventorySlotHover)
                        .unwrap()
                        .clone()
                        .to_owned(),
                );

                if let Some(_item_e) = state.item {
                    let item = if state.r#type.is_crafting() {
                        inv.single().crafting_items[state.slot_index]
                            .clone()
                            .unwrap()
                            .item_stack
                    } else if state.r#type.is_crafting_result() {
                        inv.single()
                            .crafting_result_item
                            .clone()
                            .unwrap()
                            .item_stack
                    } else {
                        inv.single().items[state.slot_index]
                            .clone()
                            .unwrap()
                            .item_stack
                    };
                    tooltip_update_events.send(ToolTipUpdateEvent {
                        item_stack: item,
                        parent_slot_entity: e,
                    });
                }
            }
        }
        if let Interaction::Hovering = interactable.previous() {
            if ui == &UIElement::InventorySlotHover {
                // swap to base img

                commands.entity(e).insert(UIElement::InventorySlot).insert(
                    graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&UIElement::InventorySlot)
                        .unwrap()
                        .clone()
                        .to_owned(),
                );
                for tooltip in tooltips.iter() {
                    commands.entity(tooltip.0).despawn_recursive();
                }
            }
        }
    }
}

pub fn handle_item_drop_clicks(
    mouse_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_states: Query<&mut InventorySlotState>,
    inv_state: Res<InventoryState>,

    mut slot_drop_events: EventWriter<DropOnSlotEvent>,
    mut world_drop_events: EventWriter<DropInWorldEvent>,

    mut item_stack_query: Query<&mut ItemStack>,
    mut interactables: Query<(Entity, &mut Interactable)>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);
    let inv_open = inv_state.open;
    let hit_test = if inv_open {
        ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None)
    } else {
        None
    };
    for (e, interactable) in interactables.iter_mut() {
        // reset dragged interactables when mouse released
        if let Interaction::Dragging { item } = interactable.current() {
            if let Ok(mut item_stack) = item_stack_query.get_mut(*item) {
                if let Some(drop_target) = hit_test {
                    if let Ok(state) = slot_states.get(drop_target.0) {
                        if left_mouse_pressed {
                            slot_drop_events.send(DropOnSlotEvent {
                                dropped_entity: *item,
                                dropped_item_stack: item_stack.clone(),
                                drop_target_slot_state: state.clone(),
                                parent_interactable_entity: e,
                                stack_empty: true,
                            });
                        } else if right_mouse_pressed {
                            let mut valid_drop = true;
                            if let Some(target_obj_type) = state.obj_type {
                                if item_stack.obj_type != target_obj_type {
                                    valid_drop = false;
                                }
                            }
                            if valid_drop {
                                let lonely_item_stack: ItemStack = ItemStack {
                                    obj_type: item_stack.obj_type,
                                    metadata: item_stack.metadata.clone(),
                                    attributes: item_stack.attributes.clone(),
                                    count: 1,
                                };
                                item_stack.modify_count(-1);
                                slot_drop_events.send(DropOnSlotEvent {
                                    dropped_entity: *item,
                                    dropped_item_stack: lonely_item_stack,
                                    parent_interactable_entity: e,
                                    drop_target_slot_state: state.clone(),
                                    stack_empty: item_stack.count == 0,
                                });
                            }
                        }
                    }
                } else {
                    // we did not click on a slot, so send drop out of inv event
                    if left_mouse_pressed {
                        world_drop_events.send(DropInWorldEvent {
                            dropped_entity: *item,
                            dropped_item_stack: item_stack.clone(),
                            parent_interactable_entity: e,
                            stack_empty: true,
                        });
                    } else if right_mouse_pressed {
                        let lonely_item_stack: ItemStack = ItemStack {
                            obj_type: item_stack.obj_type,
                            metadata: item_stack.metadata.clone(),
                            attributes: item_stack.attributes.clone(),
                            count: 1,
                        };
                        item_stack.modify_count(-1);
                        world_drop_events.send(DropInWorldEvent {
                            dropped_entity: *item,
                            dropped_item_stack: lonely_item_stack,
                            parent_interactable_entity: e,
                            stack_empty: item_stack.count == 0,
                        });
                    }
                }
            }
        } else {
            continue;
        };
    }
}
pub fn handle_cursor_update(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut inv_slots: Query<(Entity, &mut Interactable, &mut InventorySlotState)>,
    mut inv_item_icons: Query<(Entity, &mut Transform, &ItemStack)>,
    dragging_query: Query<&DraggedItem>,
    inv_state: Res<InventoryState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
    mut crafting_slot_event: EventWriter<CraftingSlotUpdateEvent>,
    mut complete_recipe_event: EventWriter<CompleteRecipeEvent>,
) {
    // get cursor resource from inputs
    // do a ray cast and get results
    if !inv_state.open {
        return;
    }

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);
    let currently_dragging = dragging_query.iter().len() > 0;
    for (e, mut interactable, mut state) in inv_slots.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && !currently_dragging {
                        //send drag event
                        if let Some(item) = state.item {
                            if let Ok(item_icon) = inv_item_icons.get_mut(item) {
                                commands
                                    .entity(item_icon.0)
                                    .remove_parent()
                                    .insert(DraggedItem);

                                interactable.change(Interaction::Dragging { item: item_icon.0 });
                                if state.r#type.is_crafting() {
                                    inv.single_mut().crafting_items[state.slot_index] = None;
                                    crafting_slot_event.send(CraftingSlotUpdateEvent);
                                } else if state.r#type.is_crafting_result() {
                                    complete_recipe_event.send(CompleteRecipeEvent);
                                } else {
                                    inv.single_mut().items[state.slot_index] = None;
                                }
                                state.dirty = true;
                                mouse_input.clear();
                            }
                        }
                    } else if right_mouse_pressed && !currently_dragging {
                        if let Some(item) = state.item {
                            if let Ok(item_icon) = inv_item_icons.get_mut(item) {
                                let split_stack = InventoryPlugin::split_stack(
                                    item_icon.2.clone(),
                                    state.slot_index,
                                    &mut state,
                                    &mut inv,
                                );
                                let e = spawn_item_stack_icon(
                                    &mut commands,
                                    &graphics,
                                    &split_stack,
                                    &asset_server,
                                );

                                commands.entity(e).insert(DraggedItem);
                                interactable.change(Interaction::Dragging { item: e });
                                mouse_input.clear();
                            }
                        }
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering ?
                let Interaction::Hovering = interactable.current() else {continue};

                interactable.change(Interaction::None);
            }
        }
    }
}
