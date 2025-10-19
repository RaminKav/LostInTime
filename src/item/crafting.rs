use bevy::{prelude::*, reflect::TypeUuid, utils::HashMap};
use itertools::Itertools;
use rand::Rng;
use serde::{Deserialize, Serialize};

use super::EquipmentType;
use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::{
            levelup_item_stats, reroll_item_bonus_attributes, spawn_rarity_animation,
        },
        ItemRarity,
    },
    client::analytics::{AnalyticsTrigger, AnalyticsUpdateEvent},
    colors::YELLOW,
    container::Container,
    inventory::{Inventory, InventoryItemStack, ItemStack},
    item::WorldObject,
    juice::ShakeEffect,
    player::{levels::PlayerLevel, Player},
    proto::proto_param::ProtoParam,
    ui::{
        crafting_ui::CraftingContainerType,
        damage_numbers::{spawn_floating_text_with_shadow, NewRecipeTextTimer},
        handle_hovering, spawn_item_stack_icon, InventorySlotState, InventoryState,
        ToolTipUpdateEvent,
    },
    GameState, TextureCamera,
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

#[derive(Default, Clone, Debug, Deserialize, PartialEq, Eq, TypeUuid)]
#[uuid = "413bd529-bfeb-41b3-9db0-4b8b380a2c36"]
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
    pub discovered_crafting_types: Vec<CraftingContainerType>,
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
            if !craft_tracker.craftable.contains(&result) {
                craft_tracker.craftable.push(result);
            }
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
        while !remaining_cost.is_empty() {
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
                        item: item.item,
                        count: (item.count - count),
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
    player_level: u8,
) -> Vec<Option<InventoryItemStack>> {
    let mut list = vec![];
    for (slot, obj) in objs.iter().enumerate() {
        let recipe = rec.crafting_list.get(obj).expect("no recipe for item?");
        let mut default_stack = proto.get_item_data(*obj).unwrap().clone();

        let stack_count = recipe.2;
        let desc = recipe
            .0
            .iter()
            .map(|ingredient| format!("{}x", ingredient.count,))
            .collect();
        default_stack.metadata.desc = desc;
        if proto.get_component::<EquipmentType, _>(*obj).is_some() {
            default_stack.metadata.level = Some(player_level);
        }
        list.push(Some(InventoryItemStack::new(
            default_stack.copy_with_count(stack_count),
            slot,
        )));
    }
    list
}

pub fn handle_furnace_slot_update(
    mut commands: Commands,
    proto: ProtoParam,
    time: Res<Time>,
    mut inv: Query<&mut Inventory>,
    mut inv_state: ResMut<InventoryState>,
    mut inv_slots: Query<(Entity, &mut InventorySlotState)>,
    mut tooltip_update_events: EventWriter<ToolTipUpdateEvent>,
    asset_server: Res<AssetServer>,
    mut game_camera: Query<Entity, With<TextureCamera>>,
) {
    let mut inv = inv.single_mut();

    if inv_state.furnace_state.ready_to_upgrade {
        inv_state.furnace_state.upgrade_timer.tick(time.delta());

        if inv_state.furnace_state.upgrade_timer.just_finished() {
            match inv_state.furnace_state.current_fuel_type {
                WorldObject::UpgradeTome => {
                    // Upgrade Stats
                    let upgraded_stack = levelup_item_stats(
                        &inv.furnace_items.items[1]
                            .as_ref()
                            .unwrap()
                            .clone()
                            .item_stack,
                        1,
                        &proto,
                        false,
                    );

                    inv.furnace_items.items[1] = Some(InventoryItemStack {
                        item_stack: upgraded_stack.clone(),
                        slot: 1,
                    });

                    // Increase Level
                    inv.furnace_items.items[1]
                        .as_ref()
                        .unwrap()
                        .clone()
                        .modify_level(1, &mut inv.furnace_items);
                }
                WorldObject::OrbOfTransformation => {
                    // Reroll Attributes
                    let old_item = inv.furnace_items.items[1].as_ref().unwrap().clone();
                    let new_item = reroll_item_bonus_attributes(&old_item.item_stack, &proto);
                    inv.furnace_items.items[1] =
                        Some(InventoryItemStack::new(new_item.clone(), old_item.slot));

                    let new_rarity = new_item.rarity.clone();
                    let rarity_changed = new_rarity != old_item.clone().item_stack.rarity;

                    if rarity_changed {
                        spawn_rarity_animation(
                            new_rarity.clone(),
                            &mut commands,
                            &asset_server,
                            Vec3::new(99., 55., 20.),
                        );
                        if new_rarity == ItemRarity::Legendary {
                            let mut rng = rand::thread_rng();
                            let seed = rng.gen_range(0..100000);
                            let speed = 10.;
                            let max_mag = 90.;
                            let noise = 0.5;
                            let dir = Vec2::new(1., 1.);
                            for e in game_camera.iter_mut() {
                                commands.entity(e).insert(ShakeEffect {
                                    timer: Timer::from_seconds(2., TimerMode::Once),
                                    speed,
                                    seed,
                                    max_mag,
                                    noise,
                                    dir,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
            for (_, mut state) in inv_slots.iter_mut() {
                if state.slot_index == 1 && (state.r#type.is_furnace() || state.r#type.is_hotbar())
                {
                    state.dirty = true;
                }
            }
            inv_state.furnace_state.upgrade_timer.reset();
            inv_state.furnace_state.ready_to_upgrade = false;
            tooltip_update_events.send(ToolTipUpdateEvent {
                item_stack: inv.furnace_items.items[1].clone().unwrap().item_stack,
                is_recipe: false,
                show_range: false,
            });
        }
    }
}

pub fn handle_inv_changed_update_crafting_tracker(
    mut inv: Query<&mut Inventory, Changed<Inventory>>,
    mut craft_tracker: ResMut<CraftingTracker>,
    recipes: Res<Recipes>,
    proto: ProtoParam,
    player: Query<(&GlobalTransform, &PlayerLevel), With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    mut text_timer: ResMut<NewRecipeTextTimer>,
    time: Res<Time>,
) {
    let (player_t, player_level) = player.single();
    // queue processing
    if !text_timer.timer.finished() {
        text_timer.timer.tick(time.delta());
    } else if !text_timer.queue.is_empty() {
        let shadow = spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            player_t.translation() + Vec3::new(0., 15., 10.),
            YELLOW,
            "New Recipe!".to_string(),
        );
        let icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(text_timer.queue.pop().expect("queue is not empty")),
            &asset_server,
            Vec2::new(30., 2.),
            Vec2::new(0., 0.),
            0,
        );
        commands.entity(icon).set_parent(shadow);
        text_timer.timer.reset();
    }

    // detect new items in inventory and add to queue
    if inv.get_single().is_err() {
        return;
    }
    let mut inv = inv.single_mut();
    for slot in inv.items.items.iter() {
        if let Some(item) = slot {
            let new_obj = item.item_stack.obj_type;

            for (result, recipe) in recipes.crafting_list.iter() {
                if craft_tracker.discovered_recipes.contains(result)
                    || (!craft_tracker.discovered_crafting_types.contains(&recipe.1)
                        && recipe.1 != CraftingContainerType::Inventory)
                {
                    continue;
                }

                for ingredient in recipe.0.iter() {
                    if ingredient.item == new_obj {
                        craft_tracker.discovered_recipes.push(*result);
                        craft_tracker
                            .crafting_type_map
                            .entry(recipe.1.clone())
                            .or_insert(vec![])
                            .push(*result);
                        text_timer.queue.push(*result);
                        debug!("pushed to recipe queue {:?}", text_timer.queue);
                        continue;
                    }
                }
            }
            if !craft_tracker.discovered_objects.contains(&new_obj) {
                craft_tracker.discovered_objects.push(new_obj);
            }
        }
    }
    if let Some(inv_recipes) = craft_tracker
        .crafting_type_map
        .get(&CraftingContainerType::Inventory)
    {
        inv.crafting_items = Container {
            items: get_crafting_inventory_item_stacks(
                inv_recipes
                    .into_iter()
                    .sorted_by_key(|i| i.to_string())
                    .cloned()
                    .collect_vec(),
                &recipes,
                &proto,
                player_level.level,
            ),
            ..default()
        };
    }
}
