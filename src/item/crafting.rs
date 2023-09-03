use bevy::{prelude::*, utils::HashMap};
use serde::Deserialize;

use crate::{
    inventory::{Inventory, InventoryItemStack},
    item::WorldObject,
    proto::proto_param::ProtoParam,
    GameState,
};

pub struct CraftingPlugin;
impl Plugin for CraftingPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<CraftingSlotUpdateEvent>()
            .add_event::<CompleteRecipeEvent>()
            .insert_resource(Recipes::default())
            .add_systems(
                (
                    Self::handle_crafting_slot_update,
                    Self::handle_recipe_complete,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

#[derive(Resource, Default, Deserialize, Debug, PartialEq, Eq, Clone)]
pub enum RecipeType {
    #[default]
    Shaped,
    Shapeless,
}
#[derive(Resource, Default, Deserialize)]
pub struct Recipes {
    // map of recipie result and its recipe matrix
    pub recipes_list: RecipeList,
}
pub type RecipeList = HashMap<WorldObject, (CraftingGrid, RecipeType, usize)>;

impl CraftingPlugin {
    fn handle_crafting_slot_update(
        mut events: EventReader<CraftingSlotUpdateEvent>,
        mut inv: Query<&mut Inventory>,
        recipes_list: Res<Recipes>,
        proto: ProtoParam,
    ) {
        for _ in events.iter() {
            let crafting_slots = &inv.single().crafting_items.items;
            let mut recipe: CraftingGrid = [None; 4];
            for stack_option in crafting_slots.iter() {
                if let Some(stack) = stack_option {
                    let item = *stack.get_obj();
                    let i = match stack.slot {
                        0 => 2,
                        1 => 3,
                        2 => 0,
                        3 => 1,
                        4 => continue,
                        _ => unreachable!(),
                    };
                    recipe[i] = Some(item);
                }
            }

            let recipe = CraftingRecipe { recipe };
            let result_option = if let Some((result, count)) =
                recipe.get_potential_reward(bevy::prelude::Res::<'_, Recipes>::clone(&recipes_list))
            {
                let item_stack = proto.get_item_data(result).unwrap().copy_with_count(count);

                Some(InventoryItemStack {
                    item_stack: item_stack.clone(),
                    slot: 4,
                })
            } else {
                None
            };
            inv.single_mut().crafting_items.items[4] = result_option;
        }
    }
    fn handle_recipe_complete(
        mut events: EventReader<CompleteRecipeEvent>,
        mut crafting_slot_event: EventWriter<CraftingSlotUpdateEvent>,
        mut inv: Query<&mut Inventory>,
    ) {
        for _ in events.iter() {
            inv.single_mut().crafting_items.items[4] = None;
            for crafting_item_option in inv.single_mut().crafting_items.items.iter_mut() {
                if let Some(crafting_item) = crafting_item_option.as_mut() {
                    if let Some(remaining_item) = crafting_item.modify_count(-1) {
                        *crafting_item = remaining_item;
                    } else {
                        *crafting_item_option = None;
                    }
                }
            }
            crafting_slot_event.send(CraftingSlotUpdateEvent);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CraftingSlotUpdateEvent;
#[derive(Clone, Debug, Default)]
pub struct CompleteRecipeEvent;

#[derive(PartialEq)]
pub struct CraftingRecipe {
    recipe: CraftingGrid,
}
pub type CraftingGrid = [Option<WorldObject>; 4];

impl CraftingRecipe {
    fn get_potential_reward(self, recipes_list: Res<Recipes>) -> Option<(WorldObject, usize)> {
        for (result, (recipe, recipe_type, count)) in recipes_list.recipes_list.iter() {
            let mut grid = self.recipe.clone();

            let mut recipe = recipe.clone();
            if recipe_type == &RecipeType::Shapeless {
                recipe.sort_by_key(|v| v.is_some());
                grid.sort_by_key(|v| v.is_some());
                if recipe == grid {
                    return Some((*result, *count));
                }
                continue;
            }
            if self
                == (Self {
                    recipe: recipe.try_into().unwrap(),
                })
            {
                return Some((*result, *count));
            }
        }
        None
    }
}
