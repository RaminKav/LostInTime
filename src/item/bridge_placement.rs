use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::{
    assets::Graphics,
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    inventory::{Inventory, ItemStack},
    item::item_actions::ActionSuccessEvent,
    ui::UIState,
    world::world_helpers::{can_object_be_placed_here, world_pos_to_tile_pos},
    GameParam,
};

use super::{item_actions::ItemActionParam, PlaceItemEvent, WorldObject};

const BRIDGE_MODE_IDLE_EXIT_SECS: f32 = 3.0;

/// Preview icon while placing bridges — not an inventory drag (`DraggedItem`) and has no
/// [`ItemStack`], so it cannot be world-dropped or picked up as loot.
#[derive(Component)]
pub struct BridgePlacementDragIcon;

#[derive(Resource, Default)]
pub struct BridgePlacementMode {
    pub active: bool,
    pub hotbar_slot: usize,
    pub drag_icon: Option<Entity>,
    idle_timer: Timer,
}

impl BridgePlacementMode {
    fn enter(&mut self, hotbar_slot: usize) {
        self.active = true;
        self.hotbar_slot = hotbar_slot;
        self.idle_timer = Timer::from_seconds(BRIDGE_MODE_IDLE_EXIT_SECS, TimerMode::Once);
    }

    fn exit(&mut self, commands: &mut Commands) {
        self.active = false;
        if let Some(icon) = self.drag_icon.take() {
            if let Some(ec) = commands.get_entity(icon) {
                ec.despawn_recursive();
            }
        }
    }

    fn reset_idle_timer(&mut self) {
        self.idle_timer = Timer::from_seconds(BRIDGE_MODE_IDLE_EXIT_SECS, TimerMode::Once);
    }
}

fn spawn_bridge_preview_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    item_stack: &ItemStack,
) -> Entity {
    let sprite = graphics
        .icons
        .as_ref()
        .and_then(|icons| icons.get(&item_stack.obj_type).cloned())
        .unwrap_or_else(|| {
            graphics
                .spritesheet_map
                .as_ref()
                .unwrap()
                .get(&item_stack.obj_type)
                .unwrap_or_else(|| panic!("No graphic for object {:?}", item_stack.obj_type))
                .clone()
        });

    commands
        .spawn((
            SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform::from_xyz(0., 0., 995.),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            BridgePlacementDragIcon,
            Name::new("Bridge Preview Icon"),
        ))
        .id()
}

pub fn try_toggle_bridge_placement_mode(
    hotbar_slot: usize,
    held_item: &ItemStack,
    bridge_mode: &mut BridgePlacementMode,
    commands: &mut Commands,
    graphics: &Graphics,
) -> bool {
    if held_item.obj_type != WorldObject::BridgeBlock {
        return false;
    }

    if bridge_mode.active && bridge_mode.hotbar_slot == hotbar_slot {
        bridge_mode.exit(commands);
        return true;
    }

    if bridge_mode.active {
        bridge_mode.exit(commands);
    }

    bridge_mode.enter(hotbar_slot);
    let icon = spawn_bridge_preview_icon(commands, graphics, held_item);
    bridge_mode.drag_icon = Some(icon);
    true
}

fn try_place_bridge_at_cursor(
    bridge_mode: &mut BridgePlacementMode,
    game: &mut GameParam,
    proto_param: &crate::proto::proto_param::ProtoParam,
    item_action_param: &mut ItemActionParam,
) -> bool {
    let pos = item_action_param.cursor_pos.world_coords.truncate();
    if game.player().position.truncate().distance(pos) > game.player().reach_distance * 32. {
        return false;
    }
    let tile_pos = world_pos_to_tile_pos(pos);
    if !can_object_be_placed_here(tile_pos, game, WorldObject::Bridge, proto_param) {
        return false;
    }

    item_action_param.place_item_event.send(PlaceItemEvent {
        obj: WorldObject::Bridge,
        pos,
        placed_by_player: true,
        override_existing_obj: false,
    });
    item_action_param
        .analytics_event
        .send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::ObjectPlaced(WorldObject::Bridge),
        });
    item_action_param
        .action_success_event
        .send(ActionSuccessEvent {
            obj: WorldObject::BridgeBlock,
            item_slot: bridge_mode.hotbar_slot,
        });
    bridge_mode.reset_idle_timer();
    true
}

pub fn handle_bridge_placement_mode(
    time: Res<Time>,
    mut bridge_mode: ResMut<BridgePlacementMode>,
    mut commands: Commands,
    mut mouse_input: ResMut<Input<MouseButton>>,
    ui_state: Res<State<UIState>>,
    inv: Query<&Inventory>,
    mut game: GameParam,
    proto_param: crate::proto::proto_param::ProtoParam,
    mut item_action_param: ItemActionParam,
    preview_icons: Query<(), With<BridgePlacementDragIcon>>,
) {
    if !bridge_mode.active {
        return;
    }

    if ui_state.0 != UIState::Closed {
        bridge_mode.exit(&mut commands);
        return;
    }

    // Preview entity was removed externally — exit cleanly instead of holding a stale id.
    if bridge_mode.drag_icon.is_some() && preview_icons.is_empty() {
        bridge_mode.active = false;
        bridge_mode.drag_icon = None;
        return;
    }

    bridge_mode.idle_timer.tick(time.delta());
    if bridge_mode.idle_timer.finished() {
        bridge_mode.exit(&mut commands);
        return;
    }

    let Ok(inventory) = inv.get_single() else {
        bridge_mode.exit(&mut commands);
        return;
    };
    let Some(held) = inventory.items.items[bridge_mode.hotbar_slot].clone() else {
        bridge_mode.exit(&mut commands);
        return;
    };
    if held.item_stack.obj_type != WorldObject::BridgeBlock || held.item_stack.count == 0 {
        bridge_mode.exit(&mut commands);
        return;
    }

    let wants_place =
        mouse_input.just_pressed(MouseButton::Left) || mouse_input.pressed(MouseButton::Left);
    if wants_place {
        if try_place_bridge_at_cursor(
            &mut bridge_mode,
            &mut game,
            &proto_param,
            &mut item_action_param,
        ) {
            mouse_input.clear_just_pressed(MouseButton::Left);
        }
    }
}

pub fn handle_bridge_preview_dragging(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mut preview_query: Query<&mut Transform, With<BridgePlacementDragIcon>>,
) {
    for mut transform in preview_query.iter_mut() {
        transform.translation = cursor_pos.ui_coords.truncate().extend(995.);
    }
}

pub fn bridge_placement_blocks_player_attack(bridge_mode: Res<BridgePlacementMode>) -> bool {
    bridge_mode.active
}
