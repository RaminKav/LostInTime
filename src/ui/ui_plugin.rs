use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::RenderLayers,
    sprite::Anchor,
};

use strum_macros::{Display, EnumIter};

use crate::{
    assets::{GameAssetsPlugin, Graphics},
    attributes::Health,
    inputs::CursorPos,
    inventory::{Inventory, InventoryItemStack, InventoryPlugin, ItemStack},
    item::WorldObject,
    GameParam, GameState, Player, GAME_HEIGHT, GAME_WIDTH,
};

use super::ui_helpers;

#[derive(Component, Debug, EnumIter, Display, Hash, PartialEq, Eq)]
pub enum UIElement {
    Inventory,
    InventorySlot,
    InventorySlotHover,
    HealthBarFrame,
    Tooltip,
    LargeTooltip,
}

#[derive(Component, Clone, Debug)]
pub struct InventorySlotState {
    pub slot_index: usize,
    pub item: Option<Entity>,
    pub count: Option<usize>,
    pub obj_type: Option<WorldObject>,
    pub is_hotbar: bool,
    pub dirty: bool,
}
#[derive(Component, Debug)]
pub struct InventoryState {
    pub open: bool,
    pub active_hotbar_slot: usize,
    pub hotbar_dirty: bool,
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

pub struct DropInWorldEvent {
    dropped_entity: Entity,
    dropped_item_stack: ItemStack,
    parent_interactable_entity: Entity,
    stack_empty: bool,
}
#[derive(Resource)]
pub struct LastHoveredSlot {
    pub slot: Option<usize>,
}
#[derive(Component)]
pub struct HealthBar;

#[derive(Component)]
pub struct FPSText;
pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LastHoveredSlot { slot: None })
            .add_event::<DropOnSlotEvent>()
            .add_event::<DropInWorldEvent>()
            .add_system_set(
                SystemSet::on_exit(GameState::Loading)
                    .with_system(setup_inv_ui.after(GameAssetsPlugin::load_graphics))
                    .with_system(setup_healthbar_ui.after(GameAssetsPlugin::load_graphics)),
            )
            .add_system_set(SystemSet::on_enter(GameState::Main).with_system(setup_inv_slots_ui))
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_system(text_update_system)
                    .with_system(toggle_inv_visibility)
                    .with_system(handle_item_drop_clicks)
                    .with_system(handle_dragging)
                    .with_system(handle_hovering)
                    .with_system(handle_drop_on_slot_events.after(handle_item_drop_clicks))
                    .with_system(handle_drop_in_world_events.after(handle_item_drop_clicks))
                    .with_system(handle_cursor_update.before(handle_item_drop_clicks))
                    .with_system(update_inventory_ui.after(handle_hovering))
                    .with_system(update_healthbar),
            );
    }
}

fn handle_drop_in_world_events(
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
fn handle_drop_on_slot_events(
    mut events: EventReader<DropOnSlotEvent>,
    mut game: GameParam,
    mut commands: Commands,
    mut interactables: Query<(Entity, &UIElement, &mut Interactable)>,
    item_stacks: Query<&ItemStack>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
) {
    for drop_event in events.iter() {
        // all we need to do here is swap spots in the inventory
        let no_more_dragging: bool;
        let return_item = InventoryPlugin::drop_item_on_slot(
            drop_event.dropped_item_stack.clone(),
            drop_event.drop_target_slot_state.slot_index,
            &mut inv,
            &mut game.inv_slot_query,
        );
        let updated_drag_item;
        if let Some(return_item) = return_item {
            updated_drag_item = return_item;
            no_more_dragging = false;
        } else {
            updated_drag_item = item_stacks.get(drop_event.dropped_entity).unwrap().clone();
            no_more_dragging = drop_event.stack_empty;
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
                println!("RESETTING DRAG ITEM");
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

fn handle_dragging(
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
fn handle_hovering(
    mut interactables: Query<(Entity, &UIElement, &mut Interactable, &InventorySlotState)>,
    tooltips: Query<(Entity, &UIElement, &Parent), Without<InventorySlotState>>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    inv: Query<&Inventory>,
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
                    let tooltip = self::spawn_inv_item_tooltip(
                        &mut commands,
                        &graphics,
                        &asset_server,
                        &inv.single().items[state.slot_index]
                            .clone()
                            .unwrap()
                            .item_stack,
                    );
                    commands.entity(e).add_child(tooltip);
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

fn handle_item_drop_clicks(
    mouse_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_states: Query<&mut InventorySlotState>,
    inv_state: Query<&InventoryState>,

    mut slot_drop_events: EventWriter<DropOnSlotEvent>,
    mut world_drop_events: EventWriter<DropInWorldEvent>,

    mut item_stack_query: Query<&mut ItemStack>,
    mut interactables: Query<(Entity, &mut Interactable)>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);
    let inv_open = inv_state.single().open;
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
fn handle_cursor_update(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut inv_slots: Query<(Entity, &mut Interactable, &mut InventorySlotState)>,
    mut inv_item_icons: Query<(Entity, &mut Transform, &ItemStack)>,
    dragging_query: Query<&DraggedItem>,
    inv_state: Query<&InventoryState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
) {
    // get cursor resource from inputs
    // do a ray cast and get results
    if !inv_state.single().open {
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
                                inv.single_mut().items[state.slot_index] = None;
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

    // check if left click just pressed
    // iter through all interactables and check if hit entity is one of them
    // if so, match on that entitis current interaction type, and update acordingly
}
pub fn setup_inv_ui(mut commands: Commands, graphics: Res<Graphics>) {
    let overlay = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(320.0, 180.0)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., -1.),
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
            texture: graphics
                .ui_image_handles
                .as_ref()
                .unwrap()
                .get(&UIElement::Inventory)
                .unwrap()
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(162., 128.)),

                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            visibility: Visibility { is_visible: false },
            ..Default::default()
        })
        .insert(InventoryState {
            open: false,
            active_hotbar_slot: 0,
            hotbar_dirty: true,
        })
        .insert(Name::new("INVENTORY"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    commands.entity(inv).push_children(&[overlay]);
}
pub fn setup_healthbar_ui(mut commands: Commands, graphics: Res<Graphics>) {
    let inner_health = commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(121. / 255., 68. / 255., 74. / 255., 1.),
                custom_size: Some(Vec2::new(62.0, 7.0)),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-62. / 2., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HealthBar)
        .insert(Name::new("inner health bar"))
        .id();
    let health_bar_frame = commands
        .spawn(SpriteBundle {
            texture: graphics
                .ui_image_handles
                .as_ref()
                .unwrap()
                .get(&UIElement::HealthBarFrame)
                .unwrap()
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(64., 9.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(
                    (-GAME_WIDTH + 68.) / 2.,
                    (GAME_HEIGHT - 11.) / 2. - 2.,
                    10.,
                ),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(Name::new("HEALTH BAR"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    commands
        .entity(health_bar_frame)
        .push_children(&[inner_health]);
}
pub fn setup_inv_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<(Entity, &InventoryState, &Sprite)>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
) {
    for (slot_index, item) in inv.single_mut().items.iter().enumerate() {
        spawn_inv_slot(
            &mut commands,
            &graphics,
            slot_index,
            Interaction::None,
            &inv_query,
            &asset_server,
            false,
            item.clone(),
        );
        if slot_index <= 5 {
            spawn_inv_slot(
                &mut commands,
                &graphics,
                slot_index,
                Interaction::None,
                &inv_query,
                &asset_server,
                true,
                item.clone(),
            );
        }
    }
}

pub fn toggle_inv_visibility(
    mut inv_query: Query<(&mut Visibility, &mut InventoryState)>,
    mut hotbar_slots: Query<
        &mut Visibility,
        (
            Without<Interactable>,
            Without<InventoryState>,
            With<InventorySlotState>,
        ),
    >,
) {
    let (mut v, state) = inv_query.single_mut();
    if v.is_visible == state.open {
        return;
    }
    v.is_visible = state.open;
    for mut hbv in hotbar_slots.iter_mut() {
        hbv.is_visible = !v.is_visible;
    }
}
fn spawn_inv_item_tooltip(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    item_stack: &ItemStack,
) -> Entity {
    let has_attributes = item_stack.metadata.attributes.len() > 0;
    let size = if has_attributes {
        Vec2::new(80., 80.)
    } else {
        Vec2::new(64., 24.)
    };
    let tooltip = commands
        .spawn((
            SpriteBundle {
                texture: graphics
                    .ui_image_handles
                    .as_ref()
                    .unwrap()
                    .get(if has_attributes {
                        &UIElement::LargeTooltip
                    } else {
                        &UIElement::Tooltip
                    })
                    .unwrap()
                    .clone(),
                transform: Transform {
                    translation: Vec3::new(0., if has_attributes { 48. } else { 20. }, 2.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                sprite: Sprite {
                    custom_size: Some(size),
                    ..Default::default()
                },
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIElement::LargeTooltip,
            Name::new("TOOLTIP"),
        ))
        .id();

    let mut tooltip_text: Vec<(String, f32)> = vec![];
    tooltip_text.push((item_stack.metadata.name.clone(), 0.));
    // tooltip_text.push(item_stack.metadata.desc.clone());
    for (i, a) in item_stack.metadata.attributes.iter().enumerate().clone() {
        let d = if i == 0 { 2. } else { 0. };
        tooltip_text.push((a.to_string(), d));
    }
    if has_attributes {
        tooltip_text.push((item_stack.metadata.durability.clone(), 28.));
    }

    // let item_stack = ItemStack {
    //     obj_type: item_stack.obj_type,
    //     metadata: item_stack.metadata.clone(),
    //     attributes: item_stack.attributes.clone(),
    //     count: item_stack.count,
    // };
    // let item_icon = spawn_item_stack_icon(commands, graphics, &item_stack, asset_server);
    // commands.entity(tooltip).add_child(item_icon);
    for (i, (text, d)) in tooltip_text.iter().enumerate() {
        let text_pos = if has_attributes {
            Vec3::new(-32., 28. - (i as f32 * 10.) - d, 1.)
        } else {
            Vec3::new(-24., 0., 1.)
        };
        let text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        text,
                        TextStyle {
                            font: asset_server.load("fonts/Kitchen Sink.ttf"),
                            font_size: 8.0,
                            color: Color::Rgba {
                                red: 75. / 255.,
                                green: 61. / 255.,
                                blue: 68. / 255.,
                                alpha: 1.,
                            },
                        },
                    )
                    .with_alignment(TextAlignment::CENTER_LEFT),
                    transform: Transform {
                        translation: text_pos,
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("TOOLTIP TEXT"),
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(tooltip).add_child(text);
    }
    tooltip
}

pub fn spawn_inv_slot(
    commands: &mut Commands,
    graphics: &Graphics,
    slot_index: usize,
    interactable_state: Interaction,
    inv_query: &Query<(Entity, &InventoryState, &Sprite)>,
    asset_server: &AssetServer,
    is_hotbar_slot: bool,
    item_stack: Option<InventoryItemStack>,
) -> Entity {
    // spawns an inv slot, with an item icon as its child if an item exists in that inv slot.
    // the slot's parent is set to the inv ui entity.
    let (inv_e, inv_state, inv_sprite) = inv_query.single();

    let x =
        ((slot_index % 6) as f32 * 26.) - (inv_sprite.custom_size.unwrap().x) / 2. + 26. / 2. + 3.;
    let mut y = ((slot_index / 6) as f32).trunc() * 26. - (inv_sprite.custom_size.unwrap().y) / 2.
        + 6.
        + 26. / 2.;

    if is_hotbar_slot {
        y = -GAME_HEIGHT / 2. + 22.;
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
        let obj_type = item.item_stack.obj_type;
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
                if is_hotbar_slot && inv_state.active_hotbar_slot == slot_index {
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
            custom_size: Some(Vec2::new(26., 26.)),
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
            is_hotbar: is_hotbar_slot,
        })
        .insert(UIElement::InventorySlot)
        .insert(Name::new("SLOT"));
    if let Some(i) = item_icon_option {
        slot_entity.push_children(&[i]);
    }
    if !is_hotbar_slot {
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
    let item = commands
        .spawn(SpriteBundle {
            texture: graphics
                .world_obj_image_handles
                .as_ref()
                .unwrap()
                .get(&item_stack.obj_type)
                .unwrap()
                .clone(),
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(item_stack.clone())
        .insert(RenderLayers::from_layers(&[3]))
        .id();
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
                .with_alignment(TextAlignment::BOTTOM_RIGHT),
                transform: Transform {
                    translation: Vec3::new(10., -9., 1.),
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
    item
}

pub fn change_hotbar_slot(
    slot: usize,
    inv_state: &mut Query<&mut InventoryState>,
    inv_slots: &mut Query<&mut InventorySlotState>,
) {
    let mut inv_state = inv_state.single_mut();

    InventoryPlugin::mark_slot_dirty(inv_state.active_hotbar_slot, inv_slots);
    inv_state.active_hotbar_slot = slot;
    InventoryPlugin::mark_slot_dirty(slot, inv_slots);
}
pub fn update_inventory_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut ui_elements: Query<(Entity, &InventorySlotState)>,
    interactables: Query<&Interactable>,
    inv_query: Query<(Entity, &InventoryState, &Sprite)>,
    asset_server: Res<AssetServer>,
    mut inv: Query<&mut Inventory>,
) {
    for (e, state) in ui_elements.iter_mut() {
        // check current inventory state against that slot's state
        // if they do not match, delete and respawn

        // hotbars are hidden when inventory is open, so defer update
        // untile inv is closed again.
        let inv_state = inv_query.get_single().unwrap().1;
        if inv_state.open && state.is_hotbar {
            continue;
        }

        let interactable_option = interactables.get(e);
        let item_option = inv.single_mut().items[state.slot_index].clone();
        let real_count = if let Some(item) = item_option.clone() {
            Some(item.item_stack.count)
        } else {
            None
        };

        if state.dirty || state.count != real_count {
            commands.entity(e).despawn_recursive();
            spawn_inv_slot(
                &mut commands,
                &graphics,
                state.slot_index,
                if let Ok(i) = interactable_option {
                    i.current().clone()
                } else {
                    Interaction::None
                },
                &inv_query,
                &asset_server,
                state.is_hotbar,
                item_option.clone(),
            );
        }
    }
}
fn update_healthbar(
    player_health_query: Query<&Health, With<Player>>,
    mut health_bar_query: Query<&mut Sprite, With<HealthBar>>,
) {
    let player_health = player_health_query.single();
    health_bar_query.single_mut().custom_size = Some(Vec2 {
        x: 62. * player_health.0 as f32 / 100.,
        y: 7.,
    });
}
fn text_update_system(diagnostics: Res<Diagnostics>, mut query: Query<&mut Text, With<FPSText>>) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                // Update the value of the second section
                text.sections[0].value = format!("{value:.2}");
            }
        }
    }
}
