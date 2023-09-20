use bevy::{prelude::*, utils::HashMap};
use serde::Deserialize;

use crate::{
    attributes::{attribute_helpers::reroll_item_bonus_attributes, AttributeModifier},
    inventory::{Inventory, InventoryItemStack, InventoryPlugin},
    item::WorldObject,
    proto::proto_param::ProtoParam,
    ui::{
        crafting_ui::CraftingContainerType, handle_hovering, FurnaceContainer, FurnaceState,
        InventorySlotState, InventorySlotType,
    },
    GameState,
};

pub struct CraftingPlugin;
impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Recipes::default())
            .insert_resource(CraftingTracker::default())
            .add_event::<CraftedItemEvent>()
            .add_systems(
                (
                    handle_crafting_update_when_inv_changes,
                    handle_crafted_item,
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
);

#[derive(Resource, Default, Deserialize)]
pub struct CraftingTracker {
    // map of recipie result and its recipe matrix
    pub craftable: Vec<WorldObject>,
    pub discovered: Vec<WorldObject>,
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
            if InventoryPlugin::get_item_count_in_container(&inv.items, ingredient.item)
                < ingredient.count
            {
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
                let ingredient_slot =
                    InventoryPlugin::get_slot_for_item_in_container(&inv.items, &item.item)
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
            .map(|ingredient| {
                format!(
                    "{}x {}",
                    ingredient.count,
                    proto
                        .get_item_data(ingredient.item.clone())
                        .unwrap()
                        .metadata
                        .name
                )
            })
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
                        furnace.items.items[1]
                            .as_ref()
                            .unwrap()
                            .clone()
                            .modify_attributes(
                                AttributeModifier {
                                    modifier: "attack".to_owned(),
                                    delta: 1,
                                },
                                &mut furnace.items,
                            );
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
                InventoryPlugin::mark_slot_dirty(1, InventorySlotType::Furnace, &mut inv_slots);
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
