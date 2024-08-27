pub use bevy::prelude::*;
use bevy::{render::view::RenderLayers, utils::HashMap};
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use rand::Rng;

use crate::{
    assets::Graphics,
    colors::WHITE,
    container::{Container, ContainerRegistry},
    inventory::{InventoryItemStack, ItemStack},
    item::WorldObject,
    proto::proto_param::ProtoParam,
    world::world_helpers::world_pos_to_tile_pos,
};

use super::{
    interactions::Interaction, spawn_inv_slot, Interactable, InventorySlotState, InventorySlotType,
    InventoryState, InventoryUI, MenuButton, UIElement, UIState,
};

pub const SCRAPPER_SIZE: usize = 6 * 2;

#[derive(Component, Resource, Debug, Clone)]
pub struct ScrapperContainer {
    pub items: Container,
    pub parent: Entity,
}
#[derive(Component, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct Scrap {
    pub obj: WorldObject,
    pub chance: f32,
}
impl Scrap {
    pub fn new(obj: WorldObject, chance: f32) -> Self {
        Self { obj, chance }
    }
}
#[derive(Component, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct ScrapsInto(pub Vec<Scrap>);

#[derive(Default)]
pub struct ScrapperEvent;

pub fn setup_scrapper_slots_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv_query: Query<Entity, With<InventoryUI>>,
    inv_state_res: Res<InventoryState>,
    inv_state: Res<State<UIState>>,
    inv_spawn_check: Query<Entity, Added<InventoryUI>>,

    asset_server: Res<AssetServer>,
    inv: Res<ScrapperContainer>,
) {
    if inv_spawn_check.get_single().is_err() {
        return;
    }
    if inv_state.0 != UIState::Scrapper {
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
            InventorySlotType::Scrapper,
            item.clone(),
        );
    }
    let parent = inv_spawn_check.single();
    // SCRAP BUTTON
    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Scrap",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(90., 34.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("SCRAPPER TEXT"),
            RenderLayers::from_layers(&[3]),
            Interactable::default(),
            UIElement::MenuButton,
            MenuButton::Scrapper,
            Sprite {
                custom_size: Some(Vec2::new(40., 12.)),
                ..default()
            },
        ))
        .set_parent(parent);
}
pub fn handle_scrap_items_in_scrapper(
    mut scrapper_inv: ResMut<ScrapperContainer>,
    mut inv_slots: Query<&mut InventorySlotState>,
    proto_param: ProtoParam,
    mut scrapper_event: EventReader<ScrapperEvent>,
) {
    if scrapper_event.iter().len() > 0 {
        scrapper_event.clear();
        let mut rng = rand::thread_rng();
        let mut new_items = HashMap::new();
        for item in scrapper_inv.items.items.iter().flatten() {
            if let Some(scrap) = proto_param.get_component::<ScrapsInto, _>(*item.get_obj()) {
                let rarity = item.item_stack.rarity.clone();
                for scrap in rarity.get_scrap().0.iter() {
                    if rng.gen::<f32>() <= scrap.chance {
                        *new_items.entry(scrap.obj.clone()).or_insert(0) += 1;
                    }
                }

                for scrap in scrap.0.iter() {
                    if rng.gen::<f32>() <= scrap.chance {
                        *new_items.entry(scrap.obj.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
        scrapper_inv.items = Container::with_size(SCRAPPER_SIZE);
        for (i, (item, count)) in new_items.iter().enumerate() {
            let stack = proto_param.get_component::<ItemStack, _>(*item).unwrap();
            InventoryItemStack::new(stack.copy_with_count(*count), i).add_to_container(
                &mut scrapper_inv.items,
                InventorySlotType::Scrapper,
                &mut inv_slots,
            );
        }
    }
}
pub fn change_ui_state_to_scrapper_when_resource_added(
    mut inv_ui_state: ResMut<NextState<UIState>>,
) {
    inv_ui_state.set(UIState::Scrapper);
}

pub fn add_inv_to_new_scrapper_objs(
    mut commands: Commands,
    new_chests: Query<(Entity, &GlobalTransform, &WorldObject), Without<ScrapperContainer>>,
    container_reg: Res<ContainerRegistry>,
) {
    for (e, t, obj) in new_chests.iter() {
        if obj == &WorldObject::Scrapper {
            let existing_cont_option = container_reg
                .containers
                .get(&world_pos_to_tile_pos(t.translation().truncate()));
            commands.entity(e).insert(ScrapperContainer {
                items: existing_cont_option
                    .unwrap_or(&Container::with_size(SCRAPPER_SIZE))
                    .clone(),
                parent: e,
            });
        }
    }
}
