pub mod chest_ui;
mod damage_numbers;
mod enemy_health_bar;
mod fps_text;
mod interactions;
mod inventory_ui;
pub mod minimap;
mod player_hud;
mod tooltips;
mod ui_helpers;
pub use chest_ui::*;
pub use enemy_health_bar::*;
use fps_text::*;
pub use interactions::*;
pub use inventory_ui::*;
pub use player_hud::*;
pub use tooltips::*;
pub use ui_helpers::*;

use crate::{
    client::ClientPlugin, combat::CombatPlugin, item::item_actions::ActionSuccessEvent,
    CustomFlush, GameState,
};

use self::{
    damage_numbers::{
        add_previous_health, handle_add_damage_numbers_after_hit, tick_damage_numbers,
    },
    minimap::MinimapPlugin,
};

pub const INVENTORY_UI_SIZE: Vec2 = Vec2::new(172., 135.);
pub const CHEST_INVENTORY_UI_SIZE: Vec2 = Vec2::new(127., 142.);
pub const UI_SLOT_SIZE: f32 = 20.0;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<InventoryUIState>()
            .insert_resource(LastHoveredSlot { slot: None })
            .insert_resource(InventoryState::default())
            .add_event::<ActionSuccessEvent>()
            .add_event::<DropOnSlotEvent>()
            .add_event::<RemoveFromSlotEvent>()
            .add_event::<ToolTipUpdateEvent>()
            .add_event::<ShowInvPlayerStatsEvent>()
            .add_event::<DropInWorldEvent>()
            .register_type::<InventorySlotState>()
            .add_plugin(MinimapPlugin)
            .add_startup_system(spawn_fps_text)
            .add_systems((setup_inv_ui.before(CustomFlush).run_if(
                state_changed::<InventoryUIState>()
                    .and_then(not(in_state(InventoryUIState::Closed))),
            ),))
            .add_systems(
                (
                    setup_hotbar_hud,
                    setup_foodbar_ui.after(ClientPlugin::load_on_start),
                    setup_healthbar_ui.after(ClientPlugin::load_on_start),
                )
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_systems(
                (
                    create_enemy_health_bar,
                    add_previous_health,
                    handle_enemy_health_bar_change,
                    handle_enemy_health_visibility,
                    handle_add_damage_numbers_after_hit.after(CombatPlugin::handle_hits),
                    tick_damage_numbers,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_inv_slots_ui,
                    setup_chest_slots_ui.run_if(in_state(InventoryUIState::Chest)),
                    change_ui_state_to_chest_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ChestInventory>()),
                    text_update_system,
                    toggle_inv_visibility,
                    handle_item_drop_clicks,
                    handle_dragging,
                    handle_hovering.before(CustomFlush),
                    handle_drop_on_slot_events.after(handle_item_drop_clicks),
                    handle_drop_in_world_events.after(handle_item_drop_clicks),
                    handle_cursor_update
                        .before(handle_item_drop_clicks)
                        .run_if(not(in_state(InventoryUIState::Closed))),
                    handle_spawn_inv_item_tooltip,
                    update_inventory_ui.after(CustomFlush),
                    update_healthbar,
                    handle_update_inv_item_entities,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    add_inv_to_new_chest_objs,
                    update_foodbar,
                    handle_spawn_inv_player_stats
                        .run_if(in_state(InventoryUIState::Open))
                        .after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}
