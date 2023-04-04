use bevy::{prelude::*, render::view::RenderLayers};

use strum_macros::{Display, EnumIter};

use crate::{
    assets::{GameAssetsPlugin, Graphics},
    inputs::CursorPos,
    inventory::{InventoryPlugin, ItemStack},
    item::WorldObject,
    Game, GameState,
};

use super::ui_helpers;

#[derive(Component, Debug, EnumIter, Display, Hash, PartialEq, Eq)]
pub enum UIElement {
    Inventory,
    InventorySlot,
    InventorySlotHover,
}

#[derive(Component, Clone, Debug)]
pub struct InventorySlotState {
    pub slot_index: usize,
    pub item: Option<Entity>,
    pub obj_type: Option<WorldObject>,
    pub dirty: bool,
}
#[derive(Component, Debug)]
pub struct InventoryState {
    open: bool,
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

pub struct DropEvent {
    dropped_entity: Entity,
    dropped_item_stack: ItemStack,
    drop_target_slot_state: InventorySlotState,
    parent_interactable_entity: Entity,
    stack_empty: bool,
}
#[derive(Resource)]
pub struct LastHoveredSlot {
    pub slot: Option<usize>,
}

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LastHoveredSlot { slot: None })
            .add_event::<DropEvent>()
            .add_system_set(
                SystemSet::on_exit(GameState::Loading)
                    .with_system(setup_inv_ui.after(GameAssetsPlugin::load_graphics)),
            )
            .add_system_set(SystemSet::on_enter(GameState::Main).with_system(setup_inv_slots_ui))
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_system(handle_item_drop_clicks)
                    .with_system(handle_dragging)
                    .with_system(handle_hovering)
                    .with_system(handle_drop_events.after(handle_item_drop_clicks))
                    .with_system(handle_cursor_update.before(handle_item_drop_clicks))
                    .with_system(sync_inventory_ui.after(handle_hovering)),
            );
    }
}
fn handle_drop_events(
    mut events: EventReader<DropEvent>,
    mut game: ResMut<Game>,
    mut commands: Commands,
    mut interactables: Query<(Entity, &UIElement, &mut Interactable)>,
    mut slot_states: Query<&mut InventorySlotState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    for drop_event in events.iter() {
        // all we need to do here is swap spots in the inventory
        let mut no_more_dragging = false;
        let return_item = InventoryPlugin::drop_item_on_slot(
            &mut game,
            drop_event.dropped_item_stack,
            drop_event.drop_target_slot_state.slot_index,
            &mut slot_states,
        );

        if let Some(return_item) = return_item {
            let new_drag_icon_entity = spawn_item_stack_icon(
                &mut commands,
                &graphics,
                &return_item.obj_type,
                &return_item,
                &asset_server,
            );
            if let Ok(mut parent_interactable) =
                interactables.get_mut(drop_event.parent_interactable_entity)
            {
                commands
                    .entity(drop_event.dropped_entity)
                    .despawn_recursive();
                commands.entity(new_drag_icon_entity).insert(DraggedItem);
                parent_interactable.2.change(Interaction::Dragging {
                    item: new_drag_icon_entity,
                })
            }
            no_more_dragging = false;
        } else {
            no_more_dragging = drop_event.stack_empty;
        }
        // set parant slot entity to dirty
        let parent_e = interactables.get_mut(drop_event.parent_interactable_entity);
        if let Ok(parent_e) = parent_e {
            slot_states.get_mut(parent_e.0).unwrap().dirty = true;
        }

        // if nothing left on cursor and dragging is done
        // despawn parent stack icon entity, set parent slot to no dragging
        if no_more_dragging {
            commands
                .entity(drop_event.dropped_entity)
                .despawn_recursive();
            if let Ok(mut parent_interactable) =
                interactables.get_mut(drop_event.parent_interactable_entity)
            {
                parent_interactable.2.change(Interaction::None);
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
    mut interactables: Query<(Entity, &UIElement, &mut Interactable, &mut Handle<Image>)>,
    graphics: Res<Graphics>,
    mut commands: Commands,
) {
    // iter all interactables, find ones in hover state.
    // match the UIElement type to swap to a new image
    for (e, ui, interactable, mut _texture) in interactables.iter_mut() {
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
            }
        }
        if let Interaction::Hovering = interactable.previous() {
            if ui == &UIElement::InventorySlotHover {
                // swap to base img

                commands.entity(e).insert(UIElement::InventorySlot);
                commands.entity(e).insert(
                    graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&UIElement::InventorySlot)
                        .unwrap()
                        .clone()
                        .to_owned(),
                );
            }
        }
    }
}

fn handle_item_drop_clicks(
    mouse_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_states: Query<&mut InventorySlotState>,

    mut events: EventWriter<DropEvent>,
    mut item_stack_query: Query<&mut ItemStack>,
    mut interactables: Query<(Entity, &mut Interactable)>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let right_mouse_pressed = mouse_input.just_pressed(MouseButton::Right);
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);

    for (e, interactable) in interactables.iter_mut() {
        // reset dragged interactables when mouse released
        if let Interaction::Dragging { item } = interactable.current() {
            if let Ok(mut item_stack) = item_stack_query.get_mut(*item) {
                if let Some(drop_target) = hit_test {
                    if let Ok(state) = slot_states.get(drop_target.0) {
                        if left_mouse_pressed {
                            events.send(DropEvent {
                                dropped_entity: *item,
                                dropped_item_stack: *item_stack,
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
                                    count: 1,
                                };
                                item_stack.modify_count(-1);
                                events.send(DropEvent {
                                    dropped_entity: *item,
                                    dropped_item_stack: lonely_item_stack,
                                    parent_interactable_entity: e,
                                    drop_target_slot_state: state.clone(),
                                    stack_empty: item_stack.count == 0,
                                });
                            }
                        }
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
    mut game: ResMut<Game>,
    cursor_pos: Res<CursorPos>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut inv_slots: Query<(Entity, &mut Interactable, &mut InventorySlotState)>,
    mut inv_item_icons: Query<(Entity, &mut Transform, &ItemStack)>,
    dragging_query: Query<&DraggedItem>,
    inv_state: Query<&InventoryState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
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
                                game.player.inventory[state.slot_index] = None;
                                state.dirty = true;
                                mouse_input.clear();
                            }
                        }
                    } else if right_mouse_pressed && !currently_dragging {
                        if let Some(item) = state.item {
                            if let Ok(item_icon) = inv_item_icons.get_mut(item) {
                                let obj_type = &item_icon.2.obj_type;
                                let split_stack = InventoryPlugin::split_stack(
                                    &mut game,
                                    *item_icon.2,
                                    state.slot_index,
                                    &mut state,
                                );
                                let e = spawn_item_stack_icon(
                                    &mut commands,
                                    &graphics,
                                    obj_type,
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
                translation: Vec3::new(0., 0., 9.),
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
        .insert(InventoryState { open: false })
        .insert(Name::new("INVENTORY"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    commands.entity(inv).push_children(&[overlay]);
}
pub fn setup_inv_slots_ui(
    mut commands: Commands,
    game: Res<Game>,
    graphics: Res<Graphics>,
    inv_query: Query<(Entity, &InventoryState, &Sprite)>,
    asset_server: Res<AssetServer>,
) {
    for (slot_index, _item) in game.player.inventory.iter().enumerate() {
        spawn_inv_slot(
            &mut commands,
            &graphics,
            &game,
            slot_index,
            Interaction::None,
            &inv_query,
            &asset_server,
        );
    }
}
pub fn toggle_inv_visibility(mut inv_query: Query<(&mut Visibility, &mut InventoryState)>) {
    let (mut v, mut state) = inv_query.single_mut();
    state.open = !state.open;
    v.is_visible = !v.is_visible;
}
pub fn spawn_inv_slot(
    commands: &mut Commands,
    graphics: &Graphics,
    game: &Game,
    slot_index: usize,
    interactable_state: Interaction,
    inv_query: &Query<(Entity, &InventoryState, &Sprite)>,
    asset_server: &AssetServer,
) -> Entity {
    // spawns an inv slot, with an item icon as its child if an item exists in that inv slot.
    // the slot's parent is set to the inv ui entity.
    let (inv_e, _inv_state, inv_sprite) = inv_query.single();

    let x =
        ((slot_index % 6) as f32 * 26.) - (inv_sprite.custom_size.unwrap().x) / 2. + 26. / 2. + 3.;
    let mut y = ((slot_index / 6) as f32).trunc() * 26. - (inv_sprite.custom_size.unwrap().y) / 2.
        + 6.
        + 26. / 2.;

    if ((slot_index / 6) as f32).trunc() == 0. {
        y -= 3.;
    }
    let translation = Vec3::new(x, y, 1.);

    let mut item_icon_option = None;
    let mut item_type_option = None;
    // check if we need to spawn an item icon for this slot
    if let Some(Some(item)) = game.player.inventory.get(slot_index) {
        // player has item in this slot
        let obj_type = item.item_stack.obj_type;
        item_type_option = Some(obj_type);
        item_icon_option = Some(spawn_item_stack_icon(
            commands,
            graphics,
            &obj_type,
            &item.item_stack,
            asset_server,
        ));
    }

    let mut slot_entity = commands.spawn(SpriteBundle {
        texture: graphics
            .ui_image_handles
            .as_ref()
            .unwrap()
            .get(&UIElement::InventorySlot)
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
        .set_parent(inv_e)
        .insert(Interactable::from_state(interactable_state))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(InventorySlotState {
            slot_index,
            item: item_icon_option,
            obj_type: item_type_option,
            dirty: false,
        })
        .insert(UIElement::InventorySlot);
    if let Some(i) = item_icon_option {
        slot_entity.push_children(&[i]);
    }
    slot_entity.id()
}
pub fn spawn_item_stack_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    obj_type: &WorldObject,
    item_stack: &ItemStack,
    asset_server: &AssetServer,
) -> Entity {
    let item = commands
        .spawn(SpriteBundle {
            texture: graphics
                .world_obj_image_handles
                .as_ref()
                .unwrap()
                .get(obj_type)
                .unwrap()
                .clone(),
            transform: Transform {
                translation: Vec3::new(0., 0., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(*item_stack)
        .insert(RenderLayers::from_layers(&[3]))
        .id();
    let text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    item_stack.count.to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/FiraMono-Medium.ttf"),
                        font_size: 10.0 * 20.,
                        color: Color::WHITE,
                    },
                )
                .with_alignment(TextAlignment::BOTTOM_RIGHT),
                transform: Transform {
                    translation: Vec3::new(11., -11., 1.),
                    scale: Vec3::new(1. / 20., 1. / 20., 1.),
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
pub fn sync_inventory_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    game: Res<Game>,
    mut ui_elements: Query<(Entity, &Interactable, &InventorySlotState)>,
    inv_query: Query<(Entity, &InventoryState, &Sprite)>,
    asset_server: Res<AssetServer>,
) {
    for (e, interactable, state) in ui_elements.iter_mut() {
        // check current inventory state against the slot's state
        // if they do not match, delete and respawn
        if state.dirty {
            commands.entity(e).despawn_recursive();
            spawn_inv_slot(
                &mut commands,
                &graphics,
                &game,
                state.slot_index,
                interactable.current().clone(),
                &inv_query,
                &asset_server,
            );
        }
    }
}
