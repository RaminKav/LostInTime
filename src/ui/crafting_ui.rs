pub use bevy::prelude::*;
use serde::Deserialize;

use crate::container::Container;

use super::{InventoryState, UIState};

#[derive(Resource, Default, Debug, Clone)]
pub struct CraftingContainer {
    pub items: Container,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Deserialize, Reflect, FromReflect)]
pub enum CraftingContainerType {
    #[default]
    Inventory,
    CraftingTable,
    Anvil,
    Cauldron,
    AlchemyTable,
}

pub fn change_ui_state_to_crafting_when_resource_added(
    mut inv_ui_state: ResMut<NextState<UIState>>,
    mut inv_state: ResMut<InventoryState>,
) {
    inv_state.open = true;
    inv_ui_state.set(UIState::Crafting);
}
