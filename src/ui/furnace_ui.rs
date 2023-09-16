pub use bevy::prelude::*;

use crate::{
    assets::Graphics,
    inventory::Container,
    item::{CraftingTracker, Recipes, WorldObject},
};

use super::{
    crafting_ui::CraftingContainerType, interactions::Interaction, spawn_inv_slot,
    InventorySlotType, InventoryState, InventoryUI, UIState,
};

#[derive(Component, Resource, Debug, Clone)]
pub struct FurnaceContainer {
    pub items: Container,
    pub parent: Entity,
    pub slot_map: Vec<Vec<WorldObject>>,
    pub timer: Timer,
}

pub fn setup_furnace_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<UIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    inv: Res<FurnaceContainer>,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    if inv_state.0 != UIState::Furnace {
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
            InventorySlotType::Furnace,
            item.clone(),
        );
    }
}
pub fn change_ui_state_to_furnace_when_resource_added(
    mut inv_ui_state: ResMut<NextState<UIState>>,
    mut inv_state: ResMut<InventoryState>,
) {
    inv_state.open = true;
    inv_ui_state.set(UIState::Furnace);
}

pub fn add_container_to_new_furnace_objs(
    mut commands: Commands,
    new_furnace: Query<(Entity, &WorldObject), Added<WorldObject>>,
    recipes: Res<Recipes>,
) {
    for e in new_furnace.iter() {
        match e.1 {
            WorldObject::Furnace => {
                let ing: Vec<_> = recipes
                    .furnace_list
                    .iter()
                    .map(|(k, _)| k.clone())
                    .collect();
                let results: Vec<_> = recipes
                    .furnace_list
                    .iter()
                    .map(|(_, v)| v.clone())
                    .collect();
                commands.entity(e.0).insert(FurnaceContainer {
                    items: Container::with_size(3),
                    parent: e.0,
                    slot_map: vec![vec![WorldObject::Coal], ing.clone(), results.clone()],
                    timer: Timer::from_seconds(3., TimerMode::Once),
                });
            }
            _ => {}
        }
    }
}
