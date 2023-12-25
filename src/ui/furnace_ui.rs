pub use bevy::prelude::*;
use bevy::{render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::YELLOW,
    container::{Container, ContainerRegistry},
    item::{Recipes, WorldObject},
    world::world_helpers::world_pos_to_tile_pos,
};

use super::{
    interactions::Interaction, spawn_inv_slot, InventorySlotType, InventoryState, InventoryUI,
    UIState,
};

#[derive(Component, Resource, Debug, Clone)]
pub struct FurnaceContainer {
    pub items: Container,
    pub parent: Entity,
    pub slot_map: Vec<Vec<WorldObject>>,
    pub timer: Timer,
    pub state: Option<FurnaceState>,
}
#[derive(Debug, Clone)]
pub struct FurnaceState {
    pub current_fuel_type: WorldObject,
    pub current_fuel_left: Timer,
}
impl FurnaceState {
    pub fn from_fuel(fuel: WorldObject) -> Self {
        Self {
            current_fuel_type: fuel,
            current_fuel_left: Timer::from_seconds(
                if fuel == WorldObject::Coal { 9. } else { 3. },
                TimerMode::Once,
            ),
        }
    }
}
#[derive(Component)]
pub struct FurnaceProgBar;

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
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: YELLOW,
                custom_size: Some(Vec2::new(18., 2.)),
                anchor: Anchor::CenterLeft,
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-8., 47., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(FurnaceProgBar)
        .insert(Name::new("inner xp bar"))
        .set_parent(inv_query.single());
}
pub fn update_furnace_bar(
    furnace_option: Option<ResMut<FurnaceContainer>>,
    mut furnace_bar_query: Query<&mut Sprite, With<FurnaceProgBar>>,
) {
    let Some(furnace) = furnace_option else {
        return;
    };
    if let Ok(mut furnace_bar) = furnace_bar_query.get_single_mut() {
        furnace_bar.custom_size = Some(Vec2 {
            x: 18. * furnace.timer.percent(),
            y: 2.,
        })
    };
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
    new_furnace: Query<(Entity, &GlobalTransform, &WorldObject), Added<WorldObject>>,
    recipes: Res<Recipes>,
    container_reg: Res<ContainerRegistry>,
) {
    for (e, t, obj) in new_furnace.iter() {
        let existing_cont_option = container_reg
            .containers
            .get(&world_pos_to_tile_pos(t.translation().truncate()));
        match obj {
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
                commands.entity(e).insert(FurnaceContainer {
                    items: existing_cont_option
                        .unwrap_or(&Container::with_size(3))
                        .clone(),
                    parent: e,
                    slot_map: vec![vec![WorldObject::Coal], ing.clone(), results.clone()],
                    timer: Timer::from_seconds(3., TimerMode::Once),
                    state: None,
                });
            }
            WorldObject::UpgradeStation => {
                commands.entity(e).insert(FurnaceContainer {
                    items: existing_cont_option
                        .unwrap_or(&Container::with_size(2))
                        .clone(),
                    parent: e,
                    slot_map: vec![
                        vec![WorldObject::UpgradeTome, WorldObject::OrbOfTransformation],
                        recipes.upgradeable_items.clone(),
                    ],
                    timer: Timer::from_seconds(3., TimerMode::Once),
                    state: None,
                });
            }
            _ => {}
        }
    }
}
