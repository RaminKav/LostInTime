pub use bevy::prelude::*;

use crate::{assets::Graphics, inventory::Container, item::WorldObject};

use super::{
    interactions::Interaction, spawn_inv_slot, InventorySlotType, InventoryState, InventoryUI,
    UIState,
};

pub const CHEST_SIZE: usize = 6 * 2;

#[derive(Component, Resource, Debug, Clone)]
pub struct ChestContainer {
    pub items: Container,
    pub parent: Entity,
}

pub fn setup_chest_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<UIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    inv: Res<ChestContainer>,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    if inv_state.0 != UIState::Chest {
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
    mut inv_ui_state: ResMut<NextState<UIState>>,
    mut inv_state: ResMut<InventoryState>,
) {
    inv_state.open = true;
    inv_ui_state.set(UIState::Chest);
}

pub fn add_inv_to_new_chest_objs(
    mut commands: Commands,
    new_chests: Query<(Entity, &WorldObject), Without<ChestContainer>>,
) {
    for e in new_chests.iter() {
        if e.1 == &WorldObject::Chest {
            commands.entity(e.0).insert(ChestContainer {
                items: Container::with_size(CHEST_SIZE),
                parent: e.0,
            });
        }
    }
}
