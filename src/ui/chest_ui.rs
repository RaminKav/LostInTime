pub use bevy::prelude::*;

use crate::{
    assets::Graphics,
    inventory::{Container, InventoryItemStack, CHEST_SIZE},
};

use super::{
    interactions::Interaction, spawn_inv_slot, InventorySlotType, InventoryState, InventoryUI,
    InventoryUIState,
};
#[derive(Component, Resource, Debug, Clone)]
pub struct ChestInventory {
    pub items: Container,
    pub parent: Entity,
}

pub fn setup_chest_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<InventoryUIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    inv: Res<ChestInventory>,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    if inv_state.0 != InventoryUIState::Chest {
        return;
    };
    for (slot_index, item) in inv.items.items.iter().enumerate() {
        spawn_inv_slot(
            &mut commands,
            &inv_state,
            &graphics,
            slot_index,
            Interaction::None,
            &inv_state_res,
            &inv_query,
            &asset_server,
            InventorySlotType::Chest,
            item.clone(),
        );
    }
}
pub fn change_ui_state_to_chest_when_resource_added(
    mut inv_ui_state: ResMut<NextState<InventoryUIState>>,
    mut inv_state: ResMut<InventoryState>,
) {
    inv_state.open = true;
    inv_ui_state.set(InventoryUIState::Chest);
}
