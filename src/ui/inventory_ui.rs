use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    assets::Graphics,
    attributes::AttributeChangeEvent,
    inventory::{Inventory, InventoryItemStack, InventoryPlugin, ItemStack},
    item::WorldObject,
    ui::{ChestInventory, CHEST_INVENTORY_UI_SIZE, INVENTORY_UI_SIZE},
    GAME_HEIGHT, GAME_WIDTH,
};

use super::{interactions::Interaction, DropInWorldEvent, Interactable, UIElement, UI_SLOT_SIZE};

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum InventoryUIState {
    #[default]
    Closed,
    Open,
    NPC,
    Chest,
}

#[derive(Component, Default, Clone)]
pub struct InventoryUI;
#[derive(Component, FromReflect, Reflect, Clone, Debug)]
pub struct InventorySlotState {
    pub slot_index: usize,
    pub item: Option<Entity>,
    pub count: Option<usize>,
    pub obj_type: Option<WorldObject>,
    pub r#type: InventorySlotType,
    pub dirty: bool,
}
#[derive(Resource, Default, Debug)]
pub struct InventoryState {
    pub open: bool,
    pub active_hotbar_slot: usize,
    pub inv_size: Vec2,
    pub hotbar_dirty: bool,
}
#[derive(FromReflect, PartialEq, Reflect, Debug, Clone, Copy)]
pub enum InventorySlotType {
    Normal,
    Hotbar,
    Crafting,
    CraftingResult,
    Equipment,
    Accessory,
    Chest,
}
impl InventorySlotType {
    pub fn is_crafting(self) -> bool {
        self == InventorySlotType::Crafting
    }
    pub fn is_hotbar(self) -> bool {
        self == InventorySlotType::Hotbar
    }
    pub fn is_crafting_result(self) -> bool {
        self == InventorySlotType::CraftingResult
    }
    pub fn is_equipment(self) -> bool {
        self == InventorySlotType::Equipment
    }
    pub fn is_accessory(self) -> bool {
        self == InventorySlotType::Accessory
    }
    pub fn is_normal(self) -> bool {
        self == InventorySlotType::Normal
    }
    pub fn is_chest(self) -> bool {
        self == InventorySlotType::Chest
    }
}
pub fn setup_inv_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut inv_state: ResMut<InventoryState>,
    cur_inv_state: Res<State<InventoryUIState>>,
) {
    let (size, texture, t_offset) = match cur_inv_state.0 {
        InventoryUIState::Open => (
            INVENTORY_UI_SIZE,
            graphics
                .ui_image_handles
                .as_ref()
                .unwrap()
                .get(&UIElement::Inventory)
                .unwrap()
                .clone(),
            Vec2::new(22., 0.5),
        ),
        InventoryUIState::Chest => (
            CHEST_INVENTORY_UI_SIZE,
            graphics
                .ui_image_handles
                .as_ref()
                .unwrap()
                .get(&UIElement::ChestInventory)
                .unwrap()
                .clone(),
            Vec2::new(22.5, 0.),
        ),
        _ => return,
    };
    println!("Spawning inventory UI {:?}", cur_inv_state.0);
    let overlay = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(GAME_WIDTH, GAME_HEIGHT)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-t_offset.x, 0., -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"))
        .id();
    let inv = commands
        .spawn(SpriteBundle {
            texture,
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(t_offset.x, t_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            // visibility: Visibility::Hidden,
            ..Default::default()
        })
        .insert(InventoryUI)
        .insert(Name::new("INVENTORY"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    inv_state.inv_size = size;
    commands.entity(inv).push_children(&[overlay]);
}

pub fn setup_inv_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<InventoryUIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    let (should_spawn_equipment, should_spawn_crafting) = match inv_state.0 {
        InventoryUIState::Open => (true, true),
        InventoryUIState::Chest => (false, false),
        _ => return,
    };
    for (slot_index, item) in inv.single_mut().items.items.iter().enumerate() {
        spawn_inv_slot(
            &mut commands,
            &inv_state,
            &graphics,
            slot_index,
            Interaction::None,
            &inv_state_res,
            &inv_query,
            &asset_server,
            InventorySlotType::Normal,
            item.clone(),
        );

        // crafting slots
        if slot_index < 4 && should_spawn_crafting {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Crafting,
                None,
            );
        }
        // crafting result slot
        if slot_index == 4 && should_spawn_crafting {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::CraftingResult,
                None,
            );
        }
        // equipment slots
        if slot_index < 4 && should_spawn_equipment {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Equipment,
                None,
            );
        }
        // accessoyr slots
        if slot_index < 4 && should_spawn_equipment {
            spawn_inv_slot(
                &mut commands,
                &inv_state,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_state_res,
                &inv_query,
                &asset_server,
                InventorySlotType::Accessory,
                None,
            );
        }
    }
}

pub fn toggle_inv_visibility(
    inv_state: Res<InventoryState>,
    mut hotbar_slots: Query<&mut Visibility, (Without<Interactable>, With<InventorySlotState>)>,
    crafting_slots: Query<(Entity, &InventorySlotState), With<Interactable>>,
    crafting_item_stacks: Query<&ItemStack>,
    mut inv: Query<&mut Inventory>,
    mut next_inv_state: ResMut<NextState<InventoryUIState>>,
    curr_inv_state: Res<State<InventoryUIState>>,
    mut world_drop_events: EventWriter<DropInWorldEvent>,
    inv_query: Query<Entity, With<InventoryUI>>,
    mut commands: Commands,
    chest_option: Option<Res<ChestInventory>>,
) {
    if !inv_state.open && curr_inv_state.0 != InventoryUIState::Closed {
        next_inv_state.set(InventoryUIState::Closed);
        if let Ok(e) = inv_query.get_single() {
            if let Some(chest) = chest_option {
                if let Some(mut chest_parent) = commands.get_entity(chest.parent) {
                    chest_parent.insert(chest.to_owned());
                }
                commands.remove_resource::<ChestInventory>();
            }
            commands.entity(e).despawn_recursive();
        }
    } else if inv_state.open
        && curr_inv_state.0 == InventoryUIState::Closed
        && !next_inv_state.is_changed()
    {
        next_inv_state.set(InventoryUIState::Open);
    }
    for mut hbv in hotbar_slots.iter_mut() {
        *hbv = if inv_state.open {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
    }
    if inv_state.open {
        return;
    }
    // if closing inv, drop all items in crafting slot
    for (e, state) in crafting_slots.iter() {
        if state.r#type.is_crafting() && state.item.is_some() {
            world_drop_events.send(DropInWorldEvent {
                dropped_entity: state.item.unwrap(),
                dropped_item_stack: crafting_item_stacks
                    .get(state.item.unwrap())
                    .unwrap()
                    .clone(),
                parent_interactable_entity: e,
                stack_empty: true,
            });
            inv.single_mut().crafting_items.items[state.slot_index] = None;
        }
    }
}
pub fn spawn_inv_slot(
    commands: &mut Commands,
    inv_ui_state: &Res<State<InventoryUIState>>,
    graphics: &Graphics,
    slot_index: usize,
    interactable_state: Interaction,
    inv_state: &InventoryState,
    inv_query: &Query<Entity, With<InventoryUI>>,
    asset_server: &AssetServer,
    slot_type: InventorySlotType,
    item_stack: Option<InventoryItemStack>,
) -> Entity {
    // spawns an inv slot, with an item icon as its child if an item exists in that inv slot.
    // the slot's parent is set to the inv ui entity.
    let inv_slot_offset = match inv_ui_state.0 {
        InventoryUIState::Chest => Vec2::new(0., 0.),
        _ => Vec2::new(0., 0.),
    };

    let mut x = ((slot_index % 6) as f32 * UI_SLOT_SIZE) - (inv_state.inv_size.x) / 2.
        + UI_SLOT_SIZE / 2.
        + 4.
        + inv_slot_offset.x;
    let mut y = ((slot_index / 6) as f32).trunc() * UI_SLOT_SIZE - (inv_state.inv_size.y) / 2.
        + 7.
        + UI_SLOT_SIZE / 2.
        + inv_slot_offset.y;

    if slot_type.is_hotbar() {
        y = -GAME_HEIGHT / 2. + 14.;
        x = ((slot_index % 6) as f32 * UI_SLOT_SIZE) - 2. * UI_SLOT_SIZE;
    } else if slot_type.is_crafting() {
        x = ((slot_index % 2) as f32 * UI_SLOT_SIZE) - (inv_state.inv_size.x) / 2.
            + UI_SLOT_SIZE / 2.
            + 4.
            + 1. * UI_SLOT_SIZE;
        y = ((slot_index / 2) as f32).trunc() * UI_SLOT_SIZE
            - (inv_state.inv_size.y + UI_SLOT_SIZE) / 2.
            + 5. * UI_SLOT_SIZE
            + 10.;
    } else if slot_type.is_crafting_result() {
        x = 6. + (4. * UI_SLOT_SIZE) - (inv_state.inv_size.x) / 2.;
        y = 5. * UI_SLOT_SIZE + 18. - (inv_state.inv_size.y + UI_SLOT_SIZE) / 2.;
    } else if slot_type.is_equipment() {
        x = UI_SLOT_SIZE - (inv_state.inv_size.x) / 2. + UI_SLOT_SIZE / 2. + 7. + 5. * UI_SLOT_SIZE;
        y = slot_index as f32 * UI_SLOT_SIZE - (inv_state.inv_size.y + UI_SLOT_SIZE) / 2.
            + UI_SLOT_SIZE
            + 1. * slot_index as f32
            + 4.;
    } else if slot_type.is_accessory() {
        x = UI_SLOT_SIZE - (inv_state.inv_size.x) / 2. + UI_SLOT_SIZE / 2. + 8. + 6. * UI_SLOT_SIZE;
        y = slot_index as f32 * UI_SLOT_SIZE - (inv_state.inv_size.y + UI_SLOT_SIZE) / 2.
            + UI_SLOT_SIZE
            + 1. * slot_index as f32
            + 4.;
    } else if slot_type.is_chest() {
        y += 4. * UI_SLOT_SIZE + 11.;
    } else if ((slot_index / 6) as f32).trunc() == 0. {
        y -= 3.;
    }
    let translation = Vec3::new(x, y, 1.);
    let mut item_icon_option = None;
    let mut item_type_option = None;
    let mut item_count_option = None;
    // check if we need to spawn an item icon for this slot
    if let Some(item) = item_stack {
        // player has item in this slot

        let obj_type = *item.get_obj();
        item_type_option = Some(obj_type);
        item_count_option = Some(item.item_stack.count);
        item_icon_option = Some(spawn_item_stack_icon(
            commands,
            graphics,
            &item.item_stack,
            asset_server,
        ));
    }

    let mut slot_entity = commands.spawn(SpriteBundle {
        texture: graphics
            .ui_image_handles
            .as_ref()
            .unwrap()
            .get(
                if slot_type.is_hotbar() && inv_state.active_hotbar_slot == slot_index {
                    &UIElement::InventorySlotHover
                } else {
                    &UIElement::InventorySlot
                },
            )
            .unwrap()
            .clone(),
        transform: Transform {
            translation,
            scale: Vec3::new(1., 1., 1.),
            ..Default::default()
        },
        sprite: Sprite {
            custom_size: Some(Vec2::new(UI_SLOT_SIZE, UI_SLOT_SIZE)),
            ..Default::default()
        },
        ..Default::default()
    });
    slot_entity
        .insert(RenderLayers::from_layers(&[3]))
        .insert(InventorySlotState {
            slot_index,
            item: item_icon_option,
            obj_type: item_type_option,
            count: item_count_option,
            dirty: false,
            r#type: slot_type,
        })
        .insert(UIElement::InventorySlot)
        .insert(Name::new(if slot_type.is_crafting() {
            "CRAFTING SLOT"
        } else if slot_type.is_crafting_result() {
            "CRAFT RESULT"
        } else {
            "SLOT"
        }));
    if let Some(i) = item_icon_option {
        slot_entity.push_children(&[i]);
    }
    if !slot_type.is_hotbar() {
        let inv_e = inv_query.single();
        slot_entity
            .set_parent(inv_e)
            .insert(Interactable::from_state(interactable_state));
    }
    slot_entity.id()
}
pub fn spawn_item_stack_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    item_stack: &ItemStack,
    asset_server: &AssetServer,
) -> Entity {
    let has_icon = graphics.icons.as_ref().unwrap().get(&item_stack.obj_type);
    let sprite = if let Some(icon) = has_icon {
        icon.clone()
    } else {
        graphics
            .spritesheet_map
            .as_ref()
            .unwrap()
            .get(&item_stack.obj_type)
            .unwrap_or_else(|| panic!("No graphic for object {:?}", item_stack.obj_type))
            .clone()
    };
    let item = commands
        .spawn(SpriteSheetBundle {
            sprite,
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(item_stack.clone())
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    if item_stack.count > 1 {
        let text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        item_stack.count.to_string(),
                        TextStyle {
                            font: asset_server.load("fonts/Kitchen Sink.ttf"),
                            font_size: 8.0,
                            color: Color::WHITE,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    transform: Transform {
                        translation: Vec3::new(7., -6., 1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(item).push_children(&[text]);
    }
    item
}
//TODO: make event?
pub fn change_hotbar_slot(
    slot: usize,
    inv_state: &mut InventoryState,
    inv_slots: &mut Query<&mut InventorySlotState>,
) {
    InventoryPlugin::mark_slot_dirty(
        inv_state.active_hotbar_slot,
        InventorySlotType::Hotbar,
        inv_slots,
    );
    inv_state.active_hotbar_slot = slot;
    InventoryPlugin::mark_slot_dirty(slot, InventorySlotType::Hotbar, inv_slots);
}
pub fn update_inventory_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut ui_elements: Query<(Entity, &InventorySlotState)>,
    interactables: Query<&Interactable>,
    inv_state: Res<InventoryState>,
    inv_ui_state: Res<State<InventoryUIState>>,
    inv_query: Query<Entity, With<InventoryUI>>,
    asset_server: Res<AssetServer>,
    inv: Query<&mut Inventory>,
    chest_option: Option<Res<ChestInventory>>,
) {
    for (e, slot_state) in ui_elements.iter_mut() {
        // check current inventory state against that slot's state
        // if they do not match, delete and respawn

        // hotbars are hidden when inventory is open, so defer update
        // untile inv is closed again.
        if inv_state.open && slot_state.r#type.is_hotbar() {
            continue;
        }

        let interactable_option = interactables.get(e);
        let item_option = if slot_state.r#type.is_chest() {
            chest_option.as_ref().unwrap().items.items[slot_state.slot_index].clone()
        } else {
            inv.single()
                .get_items_from_slot_type(slot_state.r#type)
                .items[slot_state.slot_index]
                .clone()
        };
        let real_count = if let Some(item) = item_option.clone() {
            Some(item.item_stack.count)
        } else {
            None
        };

        if slot_state.dirty || slot_state.count != real_count {
            commands.entity(e).despawn_recursive();
            spawn_inv_slot(
                &mut commands,
                &inv_ui_state,
                &graphics,
                slot_state.slot_index,
                if let Ok(i) = interactable_option {
                    i.current().clone()
                } else {
                    Interaction::None
                },
                &inv_state,
                &inv_query,
                &asset_server,
                slot_state.r#type,
                item_option.clone(),
            );
        }
    }
}
/// when items in the inventory state change, update the matching entities in the UI
pub fn handle_update_inv_item_entities(
    mut inv: Query<&mut Inventory, Changed<Inventory>>,
    mut inv_slot_state: Query<&mut InventorySlotState>,
    mut att_event: EventWriter<AttributeChangeEvent>,
    inv_state: Res<InventoryState>,
    mut commands: Commands,
) {
    if !inv_state.open {
        return;
    }
    if let Ok(inv) = inv.get_single_mut() {
        att_event.send(AttributeChangeEvent);
        for inv_item_option in inv.clone().items.items.iter() {
            if let Some(inv_item) = inv_item_option {
                let item = inv_item.item_stack.clone();
                for slot_state in inv_slot_state.iter_mut() {
                    if slot_state.slot_index == inv_item.slot
                        && (slot_state.r#type == InventorySlotType::Normal
                            || slot_state.r#type == InventorySlotType::Hotbar)
                    {
                        if let Some(item_e) = slot_state.item {
                            commands.entity(item_e).insert(item.clone());
                        }
                    }
                }
            }
        }
    }
}
