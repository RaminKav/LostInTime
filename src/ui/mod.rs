// mod inventory_slot_widget;
// mod inventory_widget;
mod enemy_health_bar;
pub mod minimap;
mod ui_helpers;
mod ui_plugin;
use bevy::prelude::*;
// pub use inventory_slot_widget::*;
// pub use inventory_widget::*;
pub use enemy_health_bar::*;
pub use ui_helpers::*;
pub use ui_plugin::*;

use crate::{client::ClientPlugin, combat::CombatPlugin, GameState};

use self::minimap::MinimapPlugin;

pub struct UIPlugin;

impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(LastHoveredSlot { slot: None })
            .add_event::<DropOnSlotEvent>()
            .add_event::<ToolTipUpdateEvent>()
            .add_event::<DropInWorldEvent>()
            .register_type::<InventorySlotState>()
            .add_plugin(MinimapPlugin)
            .add_systems(
                (
                    setup_inv_ui.after(ClientPlugin::load_on_start),
                    setup_healthbar_ui.after(ClientPlugin::load_on_start),
                )
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_systems(
                (
                    create_enemy_health_bar,
                    handle_enemy_health_bar_change,
                    handle_add_damage_numbers_after_hit.before(CombatPlugin::handle_hits),
                    tick_damage_numbers,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_inv_slots_ui,
                    text_update_system,
                    toggle_inv_visibility,
                    handle_item_drop_clicks,
                    handle_dragging,
                    handle_hovering,
                    handle_drop_on_slot_events.after(handle_item_drop_clicks),
                    handle_drop_in_world_events.after(handle_item_drop_clicks),
                    handle_cursor_update.before(handle_item_drop_clicks),
                    handle_spawn_inv_item_tooltip,
                    update_inventory_ui.after(handle_hovering),
                    update_healthbar,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}
