use bevy::text::Justify;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use rand::Rng;

use crate::{
    assets::Graphics,
    attributes::ItemRarity,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{BLACK, DARK_GREEN, GREY, WHITE, YELLOW},
    cursor::CursorPos,
    inventory::{try_add_item_stack_to_inventory, Inventory, ItemStack, INVENTORY_SIZE},
    item::WorldObject,
    player::Player,
    proto::proto_param::ProtoParam,
    ui::{
        focus::ModalFocusable,
        game_fonts as gf,
        interactions::{set_sprite_image, Interactable, Interaction},
        inventory_ui::{mark_slot_dirty, spawn_item_stack_icon},
        main_menu::{spawn_back_button, MAIN_MENU_WIDE_BUTTON_SIZE},
        tooltips::{ToolTipUpdateEvent, TooltipTeardownEvent},
        ui_helpers, Focusable, InventorySlotType, SkipFocusSelectedIndicator, UIElement, UIState,
        UI_SLOT_SIZE,
    },
    GameParam, ScreenResolution, GAME_HEIGHT,
};

const WELL_GRID_COLS: usize = 6;
const WELL_SLOT_GAP: f32 = 6.;
const WELL_SALVAGE_SLOT_SIZE: Vec2 = Vec2::new(36., 40.);
/// Panel-local Y of the top equipment row (row 0 centers).
const WELL_EQUIPMENT_ROW_Y: f32 = 70.;
/// Extra left nudge for the fixed well tooltip relative to left-of-first-slot placement.
const WELL_TOOLTIP_EXTRA_LEFT: f32 = 10.;
const WELL_TOOLTIP_GAP: f32 = 12.;

/// Fixed UI-space center for all well shrine item tooltips: left of the first (top-left)
/// equipment slot, plus [`WELL_TOOLTIP_EXTRA_LEFT`] further left.
pub fn well_shrine_fixed_tooltip_position() -> Vec2 {
    let slot_stride = UI_SLOT_SIZE.x + WELL_SLOT_GAP;
    let grid_width = WELL_GRID_COLS as f32 * slot_stride - WELL_SLOT_GAP;
    let start_x = -grid_width * 0.5 + UI_SLOT_SIZE.x * 0.5;
    let half_w = crate::ui::tooltips::ITEM_TOOLTIP_LARGE_CARD_SIZE.x * 0.5;
    let half_h = crate::ui::tooltips::ITEM_TOOLTIP_LARGE_CARD_SIZE.y * 0.5;
    let x = start_x - half_w - WELL_TOOLTIP_GAP - WELL_TOOLTIP_EXTRA_LEFT;
    let anchor_bias = crate::ui::tooltips::ITEM_TOOLTIP_LARGE_CARD_SIZE.y * 0.22 - half_h;
    Vec2::new(x, WELL_EQUIPMENT_ROW_Y + anchor_bias)
}

#[derive(Component)]
pub struct WellShrineUI;

#[derive(Component)]
pub struct WellEquipmentSlot {
    pub inv_slot: usize,
}

#[derive(Component)]
pub struct WellSalvageSlot;

#[derive(Component)]
pub struct WellSalvageButton {
    pub active: bool,
}

#[derive(Component)]
pub struct WellRewardModal;

#[derive(Component)]
pub struct WellRewardOkButton;

/// Inventory slot currently staged in the salvage slot (still in inventory until confirmed).
#[derive(Resource, Default)]
pub struct WellSalvageSelection(pub Option<usize>);

/// Rebuild equipment grid / salvage slot / button after selection or salvage changes.
#[derive(Resource)]
pub struct WellUiNeedsRefresh;

fn is_well_salvageable(obj: WorldObject, proto: &ProtoParam) -> bool {
    match obj.get_equip_type(proto) {
        Some(eq) if !eq.is_tool() => true,
        _ => false,
    }
}

fn collect_salvageable_slots(
    inventory: &Inventory,
    proto: &ProtoParam,
    selected: Option<usize>,
) -> Vec<(usize, ItemStack)> {
    let mut out = Vec::new();
    for slot in 0..INVENTORY_SIZE.min(inventory.items.items.len()) {
        if Some(slot) == selected {
            continue;
        }
        if let Some(inv_stack) = &inventory.items.items[slot] {
            if is_well_salvageable(inv_stack.item_stack.obj_type, proto) {
                out.push((slot, inv_stack.item_stack.clone()));
            }
        }
    }
    out
}

/// Rolls well salvage rewards. Returns a list so multiple reward types can be granted later.
pub fn roll_well_salvage_rewards(
    rarity: &ItemRarity,
    proto: &ProtoParam,
    rng: &mut impl Rng,
) -> Vec<ItemStack> {
    let gem = |count: usize| -> Option<ItemStack> {
        proto
            .get_item_data(WorldObject::MagicGem)
            .map(|s| s.copy_with_count(count))
    };
    let material = |obj: WorldObject, count: usize| -> Option<ItemStack> {
        proto.get_item_data(obj).map(|s| s.copy_with_count(count))
    };

    match rarity {
        ItemRarity::Common => {
            if rng.gen_bool(0.5) {
                gem(1).into_iter().collect()
            } else {
                let mats = [
                    WorldObject::StoneChunk,
                    WorldObject::Log,
                    WorldObject::Stick,
                ];
                let chosen = mats[rng.gen_range(0..mats.len())];
                material(chosen, 5).into_iter().collect()
            }
        }
        ItemRarity::Uncommon => gem(1).into_iter().collect(),
        ItemRarity::Rare => {
            let count = if rng.gen_bool(0.8) { 2 } else { 3 };
            gem(count).into_iter().collect()
        }
        ItemRarity::Legendary => gem(3).into_iter().collect(),
    }
}

pub fn setup_well_shrine_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    inventory: Query<&Inventory>,
    proto: ProtoParam,
    mut selection: ResMut<WellSalvageSelection>,
) {
    selection.0 = None;

    let container = commands
        .spawn((
            Transform::from_translation(Vec3::new(0., 0., 50.)),
            Visibility::default(),
        ))
        .insert(WellShrineUI)
        .insert(UIState::WellShrine)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    commands
        .spawn((
            Sprite {
                color: Color::srgba(0.1, 0.1, 0.1, 0.95),
                custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&res)),
                ..default()
            },
            Transform::default(),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, "Well Shrine", WHITE)
                .justify(Justify::Center)
                .with_transform(Transform {
                    translation: Vec3::new(0., GAME_HEIGHT / 2. - 60., 2.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    commands
        .spawn(
            gf::BODY
                .text(
                    &asset_server,
                    "Select spare equipment to throw into the well.",
                    WHITE,
                )
                .justify(Justify::Center)
                .with_transform(Transform {
                    translation: Vec3::new(0., GAME_HEIGHT / 2. - 88., 2.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    let Ok(inv) = inventory.single() else {
        return;
    };
    spawn_well_equipment_grid(
        &mut commands,
        &graphics,
        &asset_server,
        container,
        inv,
        &proto,
        None,
    );
    spawn_well_salvage_area(&mut commands, &graphics, &asset_server, container, None);
    spawn_well_salvage_button(&mut commands, &graphics, &asset_server, container, false);

    let back_button = spawn_back_button(
        Vec3::new(res.game_width / 2. - 55., -res.game_height / 2. + 38., 60.),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands
        .entity(back_button)
        .insert(ChildOf(container))
        .insert(UIState::WellShrine)
        .insert(Focusable {
            group: UIState::WellShrine,
            index: 200,
        })
        .insert(SkipFocusSelectedIndicator);
}

fn spawn_well_equipment_grid(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    container: Entity,
    inventory: &Inventory,
    proto: &ProtoParam,
    selected: Option<usize>,
) {
    let items = collect_salvageable_slots(inventory, proto, selected);
    let slot_stride = UI_SLOT_SIZE.x + WELL_SLOT_GAP;
    let grid_width = WELL_GRID_COLS as f32 * slot_stride - WELL_SLOT_GAP;
    let start_x = -grid_width * 0.5 + UI_SLOT_SIZE.x * 0.5;
    let start_y = WELL_EQUIPMENT_ROW_Y;

    for (i, (inv_slot, stack)) in items.iter().enumerate() {
        let col = i % WELL_GRID_COLS;
        let row = i / WELL_GRID_COLS;
        let pos = Vec3::new(
            start_x + col as f32 * slot_stride,
            start_y - row as f32 * slot_stride,
            5.,
        );

        let slot_e = commands
            .spawn((
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::InventorySlot),
                    custom_size: Some(UI_SLOT_SIZE),
                    ..default()
                },
                Transform::from_translation(pos),
            ))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Interactable::default())
            .insert(WellEquipmentSlot {
                inv_slot: *inv_slot,
            })
            .insert(Focusable {
                group: UIState::WellShrine,
                index: i as u32,
            })
            .insert(Name::new("Well Equipment Slot"))
            .insert(ChildOf(container))
            .id();

        let icon = spawn_item_stack_icon(
            commands,
            graphics,
            stack,
            asset_server,
            Vec2::ZERO,
            Vec2::ZERO,
            3,
        );
        commands.entity(slot_e).add_child(icon);
    }
}

fn spawn_well_salvage_area(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    container: Entity,
    selected_stack: Option<&ItemStack>,
) {
    let salvage_e = commands
        .spawn((
            Sprite {
                image: graphics.get_ui_element_texture(UIElement::PetSelectSlot),
                custom_size: Some(WELL_SALVAGE_SLOT_SIZE),
                ..default()
            },
            Transform::from_translation(Vec3::new(0., -40., 5.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIElement::PetSelectSlot)
        .insert(Interactable::default())
        .insert(WellSalvageSlot)
        .insert(Name::new("Well Salvage Slot"))
        .insert(ChildOf(container))
        .id();

    // Empty selected slot isn't a focus target — nothing to clear/confirm.
    if selected_stack.is_some() {
        commands.entity(salvage_e).insert(Focusable {
            group: UIState::WellShrine,
            index: 100,
        });
    }

    if let Some(stack) = selected_stack {
        let icon = spawn_item_stack_icon(
            commands,
            graphics,
            stack,
            asset_server,
            Vec2::new(0., -2.),
            Vec2::ZERO,
            3,
        );
        commands.entity(salvage_e).add_child(icon);
    }
}

fn spawn_well_salvage_button(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    container: Entity,
    active: bool,
) {
    let button_e = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::MainMenuStartButton),
                    custom_size: Some(MAIN_MENU_WIDE_BUTTON_SIZE),
                    color: Color::WHITE,
                    ..default()
                },
                Transform::from_translation(Vec3::new(0., -90., 5.)),
            ),
            Interactable::default(),
            UIElement::MainMenuStartButton,
            WellSalvageButton { active },
            SkipFocusSelectedIndicator,
            RenderLayers::from_layers(&[3]),
            Name::new("Well Salvage Button"),
        ))
        .insert(ChildOf(container))
        .id();

    // Disabled Salvage isn't focus-navigable — Confirm would do nothing.
    if active {
        commands.entity(button_e).insert(Focusable {
            group: UIState::WellShrine,
            index: 101,
        });
    }

    let label_color = if active { WHITE } else { GREY };
    commands
        .spawn(
            gf::MENU_TITLE
                .text(asset_server, "Salvage", label_color)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(button_e));
}

pub fn refresh_well_ui_displays(
    mut commands: Commands,
    needs_refresh: Option<Res<WellUiNeedsRefresh>>,
    selection: Res<WellSalvageSelection>,
    inventory: Query<&Inventory>,
    proto: ProtoParam,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    container: Query<Entity, With<WellShrineUI>>,
    equipment_slots: Query<Entity, With<WellEquipmentSlot>>,
    salvage_slots: Query<Entity, With<WellSalvageSlot>>,
    salvage_buttons: Query<Entity, With<WellSalvageButton>>,
) {
    if needs_refresh.is_none() {
        return;
    }
    commands.remove_resource::<WellUiNeedsRefresh>();

    let Ok(container_e) = container.single() else {
        return;
    };
    let Ok(inv) = inventory.single() else {
        return;
    };
    let selected = selection.0;

    for e in equipment_slots.iter() {
        commands.entity(e).despawn();
    }
    for e in salvage_slots.iter() {
        commands.entity(e).despawn();
    }
    for e in salvage_buttons.iter() {
        commands.entity(e).despawn();
    }

    spawn_well_equipment_grid(
        &mut commands,
        &graphics,
        &asset_server,
        container_e,
        inv,
        &proto,
        selected,
    );

    let selected_stack = selected.and_then(|slot| {
        inv.items
            .items
            .get(slot)
            .and_then(|s| s.as_ref().map(|inv| inv.item_stack.clone()))
    });
    spawn_well_salvage_area(
        &mut commands,
        &graphics,
        &asset_server,
        container_e,
        selected_stack.as_ref(),
    );
    spawn_well_salvage_button(
        &mut commands,
        &graphics,
        &asset_server,
        container_e,
        selected.is_some(),
    );
}

fn well_focus_driving(
    mouseless: &crate::inputs::MouselessModeState,
    cursor_pos: &CursorPos,
) -> bool {
    mouseless.0 || cursor_pos.suppress_ui_hover
}

pub fn handle_well_equipment_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slots: Query<(Entity, &WellEquipmentSlot)>,
    mut selection: ResMut<WellSalvageSelection>,
    reward_modal: Query<(), With<WellRewardModal>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
    mut tooltip_teardown: MessageWriter<TooltipTeardownEvent>,
) {
    if !reward_modal.is_empty() {
        return;
    }

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_pressed = mouse_input.just_pressed(MouseButton::Left);
    let focus_driving = well_focus_driving(&mouseless, &cursor_pos);

    for (e, slot) in slots.iter() {
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = focus_driving && ui_focus.is_focused(e);

        if (left_pressed && is_hit) || (is_focused && ui_focus.confirm_just_pressed) {
            selection.0 = Some(slot.inv_slot);
            tooltip_teardown.write_default();
            commands.insert_resource(WellUiNeedsRefresh);
            commands.spawn(crate::audio::SoundSpawner::new(
                crate::audio::AudioSoundEffect::ButtonClick,
                0.2,
            ));
            break;
        }
    }
}

pub fn handle_well_salvage_slot_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slots: Query<Entity, With<WellSalvageSlot>>,
    mut selection: ResMut<WellSalvageSelection>,
    reward_modal: Query<(), With<WellRewardModal>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
    mut tooltip_teardown: MessageWriter<TooltipTeardownEvent>,
) {
    if !reward_modal.is_empty() || selection.0.is_none() {
        return;
    }

    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_pressed = mouse_input.just_pressed(MouseButton::Left);
    let focus_driving = well_focus_driving(&mouseless, &cursor_pos);

    for e in slots.iter() {
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = focus_driving && ui_focus.is_focused(e);

        if (left_pressed && is_hit) || (is_focused && ui_focus.confirm_just_pressed) {
            selection.0 = None;
            tooltip_teardown.write_default();
            commands.insert_resource(WellUiNeedsRefresh);
            commands.spawn(crate::audio::SoundSpawner::new(
                crate::audio::AudioSoundEffect::ButtonClick,
                0.2,
            ));
            break;
        }
    }
}

pub fn handle_well_equipment_tooltip(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    key_input: Res<ButtonInput<KeyCode>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_transforms: Query<&GlobalTransform>,
    mut slots: Query<(Entity, &mut Interactable, &WellEquipmentSlot)>,
    inventory: Query<&Inventory>,
    graphics: Res<Graphics>,
    mut tooltip_update: MessageWriter<ToolTipUpdateEvent>,
    reward_modal: Query<(), With<WellRewardModal>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    if !reward_modal.is_empty() {
        return;
    }
    let Ok(inv) = inventory.single() else {
        return;
    };
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let shift = key_input.pressed(KeyCode::ShiftLeft);
    let focus_driving = well_focus_driving(&mouseless, &cursor_pos);

    // Teardown is deferred to [`finalize_well_tooltip_hover`] so equipment↔salvage
    // (and slot→slot) moves don't flash-despawn the replacement card.
    for (e, mut interactable, slot) in slots.iter_mut() {
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = focus_driving && ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;
        match (hovering, interactable.current()) {
            (true, Interaction::None) => {
                interactable.change(Interaction::Hovering);
                set_sprite_image(
                    &mut commands,
                    e,
                    graphics.get_ui_element_texture(UIElement::InventorySlotHover),
                );
                if let Some(Some(inv_stack)) = inv.items.items.get(slot.inv_slot) {
                    tooltip_update.write(ToolTipUpdateEvent {
                        item_stack: inv_stack.item_stack.clone(),
                        is_recipe: false,
                        show_range: shift,
                        anchor_ui: slot_transforms
                            .get(e)
                            .ok()
                            .map(|t| t.translation().truncate()),
                        ui_state_tag: Some(UIState::WellShrine),
                        ..Default::default()
                    });
                }
            }
            (false, Interaction::Hovering) => {
                interactable.change(Interaction::None);
                set_sprite_image(
                    &mut commands,
                    e,
                    graphics.get_ui_element_texture(UIElement::InventorySlot),
                );
            }
            _ => {}
        }
    }
}

pub fn handle_well_salvage_tooltip(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    key_input: Res<ButtonInput<KeyCode>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    slot_transforms: Query<&GlobalTransform>,
    mut slots: Query<(Entity, &mut Interactable), With<WellSalvageSlot>>,
    selection: Res<WellSalvageSelection>,
    inventory: Query<&Inventory>,
    graphics: Res<Graphics>,
    mut tooltip_update: MessageWriter<ToolTipUpdateEvent>,
    reward_modal: Query<(), With<WellRewardModal>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    if !reward_modal.is_empty() {
        return;
    }
    let Some(inv_slot) = selection.0 else {
        return;
    };
    let Ok(inv) = inventory.single() else {
        return;
    };
    let Some(Some(inv_stack)) = inv.items.items.get(inv_slot) else {
        return;
    };
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let shift = key_input.pressed(KeyCode::ShiftLeft);
    let focus_driving = well_focus_driving(&mouseless, &cursor_pos);

    for (e, mut interactable) in slots.iter_mut() {
        let is_hit = hit_test.map(|(ent, _, _)| ent == e).unwrap_or(false);
        let is_focused = focus_driving && ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;
        match (hovering, interactable.current()) {
            (true, Interaction::None) => {
                interactable.change(Interaction::Hovering);
                set_sprite_image(
                    &mut commands,
                    e,
                    graphics.get_ui_element_texture(UIElement::PetSelectSlotHover),
                );
                tooltip_update.write(ToolTipUpdateEvent {
                    item_stack: inv_stack.item_stack.clone(),
                    is_recipe: false,
                    show_range: shift,
                    anchor_ui: slot_transforms
                        .get(e)
                        .ok()
                        .map(|t| t.translation().truncate()),
                    ui_state_tag: Some(UIState::WellShrine),
                    ..Default::default()
                });
            }
            (false, Interaction::Hovering) => {
                interactable.change(Interaction::None);
                set_sprite_image(
                    &mut commands,
                    e,
                    graphics.get_ui_element_texture(UIElement::PetSelectSlot),
                );
            }
            _ => {}
        }
    }
}

/// Tears down the well item tooltip only when *no* equipment/salvage slot still wants one.
/// Runs after both hover handlers so cross-region focus moves (equipment ↔ salvage) keep a
/// continuous card instead of update+teardown racing in the same frame.
pub fn finalize_well_tooltip_hover(
    equipment: Query<(&Interactable, &WellEquipmentSlot)>,
    salvage: Query<&Interactable, With<WellSalvageSlot>>,
    inventory: Query<&Inventory>,
    selection: Res<WellSalvageSelection>,
    reward_modal: Query<(), With<WellRewardModal>>,
    mut tooltip_teardown: MessageWriter<TooltipTeardownEvent>,
    mut had_tooltip_hover: Local<bool>,
) {
    if !reward_modal.is_empty() {
        if *had_tooltip_hover {
            tooltip_teardown.write_default();
            *had_tooltip_hover = false;
        }
        return;
    }

    let Ok(inv) = inventory.single() else {
        return;
    };

    let equipment_item_hover = equipment.iter().any(|(interactable, slot)| {
        matches!(interactable.current(), Interaction::Hovering)
            && matches!(inv.items.items.get(slot.inv_slot), Some(Some(_)))
    });
    let salvage_item_hover = selection.0.is_some()
        && salvage
            .iter()
            .any(|interactable| matches!(interactable.current(), Interaction::Hovering));

    let any_tooltip_hover = equipment_item_hover || salvage_item_hover;
    if *had_tooltip_hover && !any_tooltip_hover {
        tooltip_teardown.write_default();
    }
    *had_tooltip_hover = any_tooltip_hover;
}

pub fn handle_well_salvage_button(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    // Bevy 0.19 B0001: hit-test `&Sprite` must not overlap `&mut Sprite` outside a ParamSet.
    mut button_queries: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<(Entity, &mut Interactable, &WellSalvageButton, &mut Sprite)>,
    )>,
    mut selection: ResMut<WellSalvageSelection>,
    mut inventory: Query<&mut Inventory>,
    mut params: ParamSet<(ProtoParam, GameParam)>,
    reward_modal: Query<(), With<WellRewardModal>>,
    player_tf: Query<&GlobalTransform, With<Player>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    if !reward_modal.is_empty() {
        return;
    }
    let Some(slot) = selection.0 else {
        return;
    };

    let hit_test = {
        let ui_sprites = button_queries.p0();
        ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None).map(|(e, _, _)| e)
    };
    let left_pressed = mouse_input.just_pressed(MouseButton::Left);
    let focus_driving = well_focus_driving(&mouseless, &cursor_pos);

    for (e, mut interactable, btn, mut sprite) in button_queries.p1().iter_mut() {
        if !btn.active {
            continue;
        }
        let is_hit = matches!(hit_test, Some(ent) if ent == e);
        let is_focused = focus_driving && ui_focus.is_focused(e);

        if is_hit || is_focused {
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
                sprite.image = params
                    .p0()
                    .graphics
                    .get_ui_element_texture(UIElement::MainMenuStartButtonHover);
                commands
                    .entity(e)
                    .insert(UIElement::MainMenuStartButtonHover);
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            sprite.image = params
                .p0()
                .graphics
                .get_ui_element_texture(UIElement::MainMenuStartButton);
            commands.entity(e).insert(UIElement::MainMenuStartButton);
        }

        if !((left_pressed && is_hit) || (is_focused && ui_focus.confirm_just_pressed)) {
            continue;
        }

        let Ok(mut inv) = inventory.single_mut() else {
            continue;
        };
        let Some(Some(inv_stack)) = inv.items.items.get(slot).cloned() else {
            continue;
        };
        let rarity = inv_stack.item_stack.rarity.clone();

        {
            let mut game = params.p1();
            inv.items.items[slot] = None;
            mark_slot_dirty(slot, InventorySlotType::Normal, &mut game.inv_slot_query);
        }

        let mut rng = rand::thread_rng();
        let rewards = {
            let proto = params.p0();
            roll_well_salvage_rewards(&rarity, &proto, &mut rng)
        };
        let player_pos = player_tf
            .single()
            .map(|t| t.translation().truncate())
            .unwrap_or(Vec2::ZERO);

        for reward in rewards.clone() {
            let allow_hotbar = {
                let proto = params.p0();
                proto
                    .get_component::<crate::item::item_actions::ConsumableItem, _>(reward.obj_type)
                    .is_some()
            };
            let add_result = {
                let mut game = params.p1();
                try_add_item_stack_to_inventory(
                    reward,
                    &mut inv,
                    &mut game.inv_slot_query,
                    allow_hotbar,
                )
            };
            if let Err(stack) = add_result {
                let mut game = params.p1();
                stack.spawn_as_drop(&mut commands, &mut game, player_pos);
            }
        }

        selection.0 = None;
        commands.insert_resource(WellUiNeedsRefresh);
        {
            let proto = params.p0();
            spawn_well_reward_modal(
                &mut commands,
                &proto.graphics,
                &proto.asset_server,
                &rewards,
            );
        }

        commands.spawn(crate::audio::SoundSpawner::new(
            crate::audio::AudioSoundEffect::ButtonClick,
            0.2,
        ));
        break;
    }
}

fn spawn_well_reward_modal(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    rewards: &[ItemStack],
) {
    let panel_size = Vec2::new(140., 80.);
    let panel = commands
        .spawn((
            (
                Sprite {
                    color: Color::srgba(0.15, 0.12, 0.10, 0.98),
                    custom_size: Some(panel_size),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0., 10., 80.)),
            ),
            RenderLayers::from_layers(&[3]),
            WellRewardModal,
            UIState::WellShrine,
            Name::new("Well Reward Modal"),
        ))
        .id();

    commands
        .spawn(
            gf::BODY
                .text(asset_server, "You gained:", WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(-26., 18., 1.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(panel));

    for (i, reward) in rewards.iter().enumerate() {
        let icon = spawn_item_stack_icon(
            commands,
            graphics,
            reward,
            asset_server,
            Vec2::ZERO,
            Vec2::ZERO,
            3,
        );
        commands
            .entity(icon)
            .insert(Transform::from_translation(Vec3::new(
                8. + i as f32 * 28.,
                18.,
                2.,
            )));
        commands.entity(panel).add_child(icon);
    }

    let ok = commands
        .spawn((
            (
                Sprite {
                    color: BLACK,
                    custom_size: Some(Vec2::new(54., 16.)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0., -18., 1.)),
            ),
            Interactable::default(),
            WellRewardOkButton,
            SkipFocusSelectedIndicator,
            Focusable {
                group: UIState::WellShrine,
                index: 250,
            },
            ModalFocusable { index: 0 },
            RenderLayers::from_layers(&[3]),
            Name::new("Well Reward OK"),
        ))
        .insert(ChildOf(panel))
        .id();

    commands
        .spawn(
            gf::BODY
                .text(asset_server, "OK", YELLOW)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(ok));
}

pub fn handle_well_reward_ok(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut button_queries: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<WellRewardOkButton>>,
        Query<(Entity, &mut Interactable, &mut Sprite), With<WellRewardOkButton>>,
    )>,
    modals: Query<Entity, With<WellRewardModal>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    if button_queries.p1().is_empty() {
        return;
    }

    let hit = {
        if !cursor_pos.ui_hover_hit_allowed() {
            None
        } else {
            let cursor = cursor_pos.ui_coords;
            let mut ret = None;
            for (ent, sprite, xform) in button_queries.p0().iter() {
                let Some(size) = sprite.custom_size else {
                    continue;
                };
                let pos = xform.translation();
                let min_x = pos.x - 0.5 * size.x;
                let min_y = pos.y - 0.5 * size.y;
                if (min_x..=min_x + size.x).contains(&cursor.x)
                    && (min_y..=min_y + size.y).contains(&cursor.y)
                {
                    ret = Some(ent);
                }
            }
            ret
        }
    };
    let left_pressed = mouse_input.just_pressed(MouseButton::Left);
    let focus_driving = well_focus_driving(&mouseless, &cursor_pos);

    for (e, mut interactable, mut sprite) in button_queries.p1().iter_mut() {
        let is_hit = matches!(hit, Some(ent) if ent == e);
        let is_focused = focus_driving && ui_focus.is_focused(e);
        let hovering = is_hit || is_focused;

        if hovering {
            sprite.color = DARK_GREEN;
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
            }
        } else {
            sprite.color = BLACK;
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }

        if (left_pressed && is_hit) || (is_focused && ui_focus.confirm_just_pressed) {
            for modal in modals.iter() {
                commands.entity(modal).despawn();
            }
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
            break;
        }
    }
}

pub fn cleanup_well_shrine_selection(
    mut selection: ResMut<WellSalvageSelection>,
    mut commands: Commands,
    curr_ui_state: Res<State<UIState>>,
) {
    if *curr_ui_state.get() != UIState::WellShrine {
        selection.0 = None;
        commands.remove_resource::<WellUiNeedsRefresh>();
    }
}
