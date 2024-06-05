pub use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::container::Container;

use super::UIState;

#[derive(Resource, Default, Debug, Clone)]
pub struct CraftingContainer {
    pub items: Container,
}

#[derive(
    Default, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Reflect, FromReflect,
)]
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
) {
    inv_ui_state.set(UIState::Crafting);
}
