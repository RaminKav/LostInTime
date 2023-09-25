pub mod chest_ui;
pub mod crafting_ui;
pub mod damage_numbers;
pub mod ui_container_param;
pub use ui_container_param::*;
mod enemy_health_bar;
mod fps_text;
pub mod furnace_ui;
mod interactions;
mod inventory_ui;
pub mod minimap;
mod player_hud;
pub mod stats_ui;
mod tile_hover;
mod tooltips;
mod ui_helpers;
pub use chest_ui::*;
pub use enemy_health_bar::*;
use fps_text::*;
pub use furnace_ui::*;
pub use interactions::*;
pub use inventory_ui::*;
pub use player_hud::*;
pub use tooltips::*;
pub use ui_helpers::*;

use crate::{
    client::ClientPlugin, combat::handle_hits, item::item_actions::ActionSuccessEvent, CustomFlush,
    GameState,
};

use self::{
    crafting_ui::{change_ui_state_to_crafting_when_resource_added, CraftingContainer},
    // crafting_ui::setup_crafting_slots_ui,
    damage_numbers::{
        add_previous_health, handle_add_damage_numbers_after_hit, handle_add_dodge_text,
        tick_damage_numbers, DodgeEvent,
    },
    minimap::MinimapPlugin,
    stats_ui::{setup_stats_ui, toggle_stats_visibility, update_sp_text, update_stats_text},
    tile_hover::spawn_tile_hover_on_cursor_move,
};

pub const INVENTORY_UI_SIZE: Vec2 = Vec2::new(172., 135.);
pub const STATS_UI_SIZE: Vec2 = Vec2::new(79., 104.);
pub const TOOLTIP_UI_SIZE: Vec2 = Vec2::new(93., 120.5);
pub const CHEST_INVENTORY_UI_SIZE: Vec2 = Vec2::new(127., 142.);
pub const CRAFTING_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const FURNACE_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const UI_SLOT_SIZE: f32 = 20.0;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<UIState>()
            .insert_resource(LastHoveredSlot { slot: None })
            .insert_resource(InventoryState::default())
            .add_event::<ActionSuccessEvent>()
            .add_event::<DropOnSlotEvent>()
            .add_event::<DodgeEvent>()
            .add_event::<RemoveFromSlotEvent>()
            .add_event::<ToolTipUpdateEvent>()
            .add_event::<ShowInvPlayerStatsEvent>()
            .add_event::<DropInWorldEvent>()
            .register_type::<InventorySlotState>()
            .add_plugin(MinimapPlugin)
            .add_startup_system(spawn_fps_text)
            .add_systems((
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Inventory))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Chest))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Crafting))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Furnace))),
            ))
            .add_systems(
                (
                    setup_hotbar_hud,
                    setup_xp_bar_ui.after(ClientPlugin::load_on_start),
                    setup_bars_ui.after(ClientPlugin::load_on_start),
                )
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_systems(
                (
                    create_enemy_health_bar,
                    add_previous_health,
                    update_xp_bar,
                    handle_enemy_health_bar_change,
                    handle_enemy_health_visibility,
                    add_ui_icon_for_elite_mobs,
                    handle_add_damage_numbers_after_hit.after(handle_hits),
                    handle_add_dodge_text,
                    tick_damage_numbers,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_inv_slots_ui,
                    setup_chest_slots_ui.run_if(in_state(UIState::Chest)),
                    change_ui_state_to_chest_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ChestContainer>()),
                    text_update_system,
                    toggle_inv_visibility,
                    handle_item_drop_clicks,
                    handle_dragging,
                    handle_hovering.before(CustomFlush),
                    handle_drop_on_slot_events.after(handle_item_drop_clicks),
                    handle_drop_in_world_events.after(handle_item_drop_clicks),
                    handle_interaction_clicks
                        .before(handle_item_drop_clicks)
                        .run_if(not(in_state(UIState::Closed))),
                    handle_spawn_inv_item_tooltip,
                    update_inventory_ui.after(CustomFlush),
                    update_healthbar,
                    handle_update_inv_item_entities,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    add_inv_to_new_chest_objs.after(CustomFlush),
                    add_container_to_new_furnace_objs,
                    setup_furnace_slots_ui.run_if(in_state(UIState::Furnace)),
                    update_foodbar,
                    update_furnace_bar,
                    update_mana_bar,
                    handle_spawn_inv_player_stats.after(CustomFlush),
                    handle_cursor_stats_buttons.run_if(in_state(UIState::Stats)),
                    toggle_stats_visibility,
                    spawn_tile_hover_on_cursor_move,
                    setup_stats_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Stats))),
                    change_ui_state_to_crafting_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<CraftingContainer>()),
                    change_ui_state_to_furnace_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<FurnaceContainer>()),
                    update_stats_text.run_if(in_state(UIState::Stats)),
                    update_sp_text.run_if(in_state(UIState::Stats)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}
