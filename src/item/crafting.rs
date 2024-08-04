use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};

use crate::{
    attributes::{attribute_helpers::reroll_item_bonus_attributes, AttributeModifier},
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    colors::YELLOW,
    container::Container,
    inventory::{Inventory, InventoryItemStack},
    item::WorldObject,
    player::Player,
    proto::proto_param::ProtoParam,
    ui::{
        crafting_ui::CraftingContainerType, damage_numbers::spawn_floating_text_with_shadow,
        handle_hovering, mark_slot_dirty, FurnaceContainer, FurnaceState, InventorySlotState,
        InventorySlotType,
    },
    GameState,
};

pub struct CraftingPlugin;
impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Recipes::default())
            .add_event::<CraftedItemEvent>()
            .add_systems(
                (
                    handle_crafting_update_when_inv_changes,
                    handle_crafted_item,
                    handle_inv_changed_update_crafting_tracker,
                    handle_furnace_slot_update.after(handle_hovering),
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

#[derive(Resource, Default, Deserialize)]
pub struct Recipes {
    // map of recipie result and its recipe matrix
    pub crafting_list: RecipeList,
    pub furnace_list: FurnaceRecipeList,
    pub upgradeable_items: Vec<WorldObject>,
}

#[derive(Default, Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct RecipeItem {
    pub item: WorldObject,
    pub count: usize,
}

pub type RecipeList = HashMap<WorldObject, (Vec<RecipeItem>, CraftingContainerType, usize)>;
pub type FurnaceRecipeList = HashMap<WorldObject, WorldObject>;
pub type RecipeListProto = (
    Vec<(WorldObject, (Vec<RecipeItem>, CraftingContainerType, usize))>,
    Vec<(WorldObject, WorldObject)>,
    Vec<WorldObject>,
);

#[derive(Resource, Default, Clone, Serialize, Deserialize)]
pub struct CraftingTracker {
    pub craftable: Vec<WorldObject>,
    pub discovered_objects: Vec<WorldObject>,
    pub discovered_recipes: Vec<WorldObject>,
    pub crafting_type_map: HashMap<CraftingContainerType, Vec<WorldObject>>,
}

pub struct CraftedItemEvent {
    pub obj: WorldObject,
}

// there will be N craftable items in a given Crafting UI.
// each has a required list of items to craft it.
// each time the inventory changes, we re-calculate if any of the craftable items can be crafted.
// if so, we update the UI to show the item as craftable.
pub fn handle_crafting_update_when_inv_changes(
    inv: Query<&Inventory, Changed<Inventory>>,
    recipes: Res<Recipes>,
    mut craft_tracker: ResMut<CraftingTracker>,
) {
    if inv.get_single().is_err() {
        return;
    }

    for (result, recipe) in recipes.crafting_list.clone() {
        let mut can_craft = true;
        let inv = inv.single();
        for ingredient in recipe.0.clone() {
            if inv.items.get_item_count_in_container(ingredient.item) < ingredient.count {
                can_craft = false;
                break;
            }
        }
        if can_craft {
            craft_tracker.craftable.push(result);
        } else {
            craft_tracker.craftable.retain(|x| x != &result);
        }
    }
}
pub fn handle_crafted_item(
    mut inv: Query<&mut Inventory>,
    mut events: EventReader<CraftedItemEvent>,
    recipes: Res<Recipes>,
    mut analytics: EventWriter<AnalyticsUpdateEvent>,
) {
    for event in events.iter() {
        let mut inv = inv.single_mut();
        let mut remaining_cost = recipes
            .crafting_list
            .get(&event.obj)
            .expect("crafted item does not have recipe?")
            .0
            .clone();
        while remaining_cost.len() > 0 {
            for item in remaining_cost.clone().iter() {
                let ingredient_slot = inv
                    .items
                    .get_slot_for_item_in_container(&item.item)
                    .expect("player crafted item but does not have the required ingredients?");
                let stack = inv.items.items[ingredient_slot].as_mut().unwrap();
                if stack.item_stack.count >= item.count {
                    inv.items.items[ingredient_slot] = stack.modify_count(-(item.count as i8));
                    remaining_cost.retain(|x| x != item);
                } else {
                    let count = stack.item_stack.count;
                    inv.items.items[ingredient_slot] = None;
                    remaining_cost.retain(|x| x != item);
                    remaining_cost.push(RecipeItem {
                        item: item.item.clone(),
                        count: (item.count - count) as usize,
                    });
                }
            }
        }
        analytics.send(AnalyticsUpdateEvent {
            update_type: AnalyticsTrigger::RecipeCrafted(event.obj),
        });
    }
}

pub fn get_crafting_inventory_item_stacks(
    objs: Vec<WorldObject>,
    rec: &Recipes,
    proto: &ProtoParam,
) -> Vec<Option<InventoryItemStack>> {
    let mut list = vec![];
    for (slot, obj) in objs.iter().enumerate() {
        let recipe = rec.crafting_list.get(&obj).expect("no recipe for item?");
        let mut default_stack = proto.get_item_data(*obj).unwrap().clone();
        let stack_count = recipe.2;
        let desc = recipe
            .0
            .iter()
            .map(|ingredient| format!("{}x", ingredient.count,))
            .collect();
        default_stack.metadata.desc = desc;
        list.push(Some(InventoryItemStack::new(
            default_stack.copy_with_count(stack_count),
            slot,
        )));
    }
    list
}

pub fn handle_furnace_slot_update(
    furnace_option: Option<ResMut<FurnaceContainer>>,
    mut furnace_objects: Query<&mut FurnaceContainer>,
    proto: ProtoParam,
    time: Res<Time>,
    recipes: Res<Recipes>,
    mut inv_slots: Query<&mut InventorySlotState>,
) {
    let mut process_furnace = |furnace: &mut FurnaceContainer| {
        let is_upgrade_furnace = furnace.items.items.len() == 2;
        let mut needs_fuel = false;
        if let Some(fuel_state) = furnace.state.as_mut() {
            fuel_state.current_fuel_left.tick(time.delta());
        } else {
            needs_fuel = true;
        }
        if furnace.items.items[1].is_none() || (furnace.items.items[0].is_none() && needs_fuel) {
            furnace.timer.reset();
            return;
        }
        let ingredient = furnace.items.items[1].as_ref().unwrap();
        let curr_result_obj = if is_upgrade_furnace {
            None
        } else {
            furnace.items.items[2].clone()
        };

        let expected_result = if is_upgrade_furnace {
            &WorldObject::None
        } else {
            recipes
                .furnace_list
                .get(&ingredient.item_stack.obj_type)
                .expect("incorrect furnace recipe?")
        };
        if let Some(curr_result) = curr_result_obj {
            if curr_result.item_stack.obj_type != expected_result.clone() {
                // the ingredient in slot 1 does not match the current output in slot 2
                furnace.timer.reset();
                return;
            }
        }

        if needs_fuel {
            let fuel = furnace.items.items[0].as_mut().unwrap();
            let mut fuel_state = FurnaceState::from_fuel(*fuel.get_obj());
            fuel_state.current_fuel_left.tick(time.delta());
            furnace.state = Some(fuel_state);
            let updated_fuel = fuel.modify_count(-1);
            furnace.items.items[0] = updated_fuel;
        }

        furnace.timer.tick(time.delta());

        if furnace.timer.just_finished() {
            if !is_upgrade_furnace {
                let updated_result =
                    if let Some(mut existing_result) = furnace.items.items[2].clone() {
                        existing_result.modify_count(1).unwrap()
                    } else {
                        InventoryItemStack::new(
                            proto.get_item_data(*expected_result).unwrap().clone(),
                            0,
                        )
                    };
                furnace.items.items[2] = Some(updated_result);
                let updated_resource = furnace.items.items[1].as_mut().unwrap().modify_count(-1);
                furnace.items.items[1] = updated_resource;
            } else {
                match furnace
                    .state
                    .as_ref()
                    .expect("no furnace state")
                    .current_fuel_type
                {
                    WorldObject::UpgradeTome => {
                        let mut modifiers: Vec<(String, i32)> = vec![];
                        let furnace_item = furnace.items.items[1].as_ref().unwrap();
                        if let Some(eqp_type) = furnace_item.get_obj().get_equip_type(&proto) {
                            if eqp_type.is_weapon() || eqp_type.is_tool() {
                                modifiers.push(("attack".to_owned(), 1));
                            } else if eqp_type.is_equipment() && !eqp_type.is_accessory() {
                                modifiers.push(("health".to_owned(), 2));
                                modifiers.push(("armor".to_owned(), 1));
                            }
                        }
                        for (modifier, delta) in modifiers {
                            furnace.items.items[1]
                                .as_ref()
                                .unwrap()
                                .clone()
                                .modify_attributes(
                                    AttributeModifier { modifier, delta },
                                    &mut furnace.items,
                                );
                            furnace.items.items[1]
                                .as_ref()
                                .unwrap()
                                .clone()
                                .modify_level(1, &mut furnace.items);
                        }
                    }
                    WorldObject::OrbOfTransformation => {
                        let old_item = furnace.items.items[1].as_ref().unwrap();
                        furnace.items.items[1] = Some(InventoryItemStack::new(
                            reroll_item_bonus_attributes(&old_item.item_stack, &proto),
                            old_item.slot,
                        ));
                    }
                    _ => {}
                }
                mark_slot_dirty(1, InventorySlotType::Furnace, &mut inv_slots);
            }

            if let Some(state) = furnace.state.as_ref() {
                if state.current_fuel_left.finished() {
                    furnace.state = None;
                }
            }
            furnace.timer.reset();
        }
    };

    if let Some(mut furnace) = furnace_option {
        process_furnace(&mut furnace);
    }
    for mut furnace in furnace_objects.iter_mut() {
        process_furnace(&mut furnace);
    }
}

pub fn handle_inv_changed_update_crafting_tracker(
    mut inv: Query<&mut Inventory, Changed<Inventory>>,
    mut craft_tracker: ResMut<CraftingTracker>,
    recipes: Res<Recipes>,
    proto: ProtoParam,
    player_t: Query<&GlobalTransform, With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    if inv.get_single().is_err() {
        return;
    }

    let mut inv = inv.single_mut();
    for slot in inv.items.items.iter() {
        if let Some(item) = slot {
            let new_obj = item.item_stack.obj_type;
            if craft_tracker.discovered_objects.contains(&new_obj) {
                continue;
            }

            for (result, recipe) in recipes.crafting_list.iter() {
                if craft_tracker.discovered_recipes.contains(&result) {
                    continue;
                }
                for ingredient in recipe.0.iter() {
                    if ingredient.item == new_obj {
                        craft_tracker.discovered_recipes.push(result.clone());
                        craft_tracker
                            .crafting_type_map
                            .entry(recipe.1.clone())
                            .or_insert(vec![])
                            .push(result.clone());
                        spawn_floating_text_with_shadow(
                            &mut commands,
                            &asset_server,
                            player_t.single().translation() + Vec3::new(0., 10., 0.),
                            YELLOW,
                            "New Recipe!".to_string(),
                        );
                        continue;
                    }
                }
            }
            craft_tracker.discovered_objects.push(new_obj);
        }
    }
    if let Some(inv_recipes) = craft_tracker
        .crafting_type_map
        .get(&CraftingContainerType::Inventory)
    {
        inv.crafting_items = Container {
            items: get_crafting_inventory_item_stacks(inv_recipes.clone(), &recipes, &proto),
            ..default()
        };
    }
}
