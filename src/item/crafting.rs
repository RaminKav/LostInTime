use bevy::{prelude::*, utils::HashMap};
use serde::Deserialize;

use crate::{
    inventory::{Inventory, InventoryItemStack, InventoryPlugin, ItemStack},
    item::WorldObject,
    proto::proto_param::ProtoParam,
    schematic::loot_chests::create_new_random_item_stack_with_attributes,
    ui::{
        crafting_ui::CraftingContainerType,
        ui_container_param::{self, UIContainersParam},
        FurnaceContainer,
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
                    handle_furnace_slot_update,
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
) {
    let process_furnace = |furnace: &mut FurnaceContainer| {
        if !(furnace.items.items[1].is_some()
            && (furnace.items.items[0].is_some() || furnace.timer.percent() != 0.))
        {
            furnace.timer.reset();
            return;
        }
        let ingredient = furnace.items.items[1].as_ref().unwrap();
        let curr_result_obj = furnace.items.items[2].clone();

        let expected_result = recipes
            .furnace_list
            .get(&ingredient.item_stack.obj_type)
            .expect("incorrect furnace recipe?");
        if let Some(curr_result) = curr_result_obj {
            if curr_result.item_stack.obj_type != expected_result.clone() {
                // the ingredient in slot 1 does not match the current output in slot 2
                furnace.timer.reset();
                return;
            }
        }

        if furnace.timer.percent() == 0. {
            let updated_fuel = furnace.items.items[0].as_mut().unwrap().modify_count(-1);
            furnace.items.items[0] = updated_fuel;
            furnace.timer.tick(time.delta());
        } else {
            furnace.timer.tick(time.delta());
        }

        if furnace.timer.just_finished() {
            let updated_result = if let Some(mut existing_result) = furnace.items.items[2].clone() {
                existing_result.modify_count(1).unwrap()
            } else {
                InventoryItemStack::new(proto.get_item_data(*expected_result).unwrap().clone(), 0)
            };
            furnace.items.items[2] = Some(updated_result);

            let updated_resource = furnace.items.items[1].as_mut().unwrap().modify_count(-1);
            furnace.items.items[1] = updated_resource;
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
