use super::combat_shrine::{CombatShrine, CombatShrineAnim};
use super::gamble_shrine::{GambleShrine, GambleShrineAnim};
use super::item_actions::ItemActionParam;
use super::{get_crafting_inventory_item_stacks, PlaceItemEvent, WorldObject};

use crate::container::Container;
use crate::inventory::Inventory;
use crate::juice::ShakeEffect;
use crate::player::ModifyTimeFragmentsEvent;
use crate::proto::proto_param::ProtoParam;
use crate::ui::crafting_ui::{CraftingContainer, CraftingContainerType};
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::world::dimension::DimensionSpawnEvent;
use crate::world::dungeon::spawn_new_dungeon_dimension;

use crate::world::TileMapPosition;
use crate::GameParam;
use crate::{
    attributes::modifiers::ModifyHealthEvent, player::MovePlayerEvent,
    world::world_helpers::world_pos_to_tile_pos,
};
use bevy::prelude::*;
use bevy_aseprite::anim::AsepriteAnimation;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use rand::Rng;

#[derive(Component, Reflect, FromReflect, Schematic, Clone, Default)]
#[reflect(Component, Schematic)]
pub enum ObjectAction {
    #[default]
    None,
    ModifyHealth(i32),
    Teleport(Vec2),
    DungeonTeleport,
    DungeonExit,
    Chest,
    Scrapper,
    Crafting(CraftingContainerType),
    Furnace,
    ChangeObject(WorldObject),
    SetHome,
    CombatShrine,
    GambleShrine,
}

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum ObjectActionCost {
    #[default]
    None,
    TimeFragment(i32),
    Item(WorldObject, usize),
}

impl ObjectAction {
    pub fn run_action(
        &self,
        e: Entity,
        obj_pos: TileMapPosition,
        obj: WorldObject,
        game: &mut GameParam,
        item_action_param: &mut ItemActionParam,
        commands: &mut Commands,
        proto_param: &mut ProtoParam,
        inv: &mut Inventory,
    ) {
        let maybe_cost: Option<&ObjectActionCost> =
            proto_param.get_component::<ObjectActionCost, _>(obj);
        if let Some(cost) = maybe_cost {
            match cost {
                ObjectActionCost::TimeFragment(cost) => {
                    if game.get_time_fragments() >= *cost {
                        item_action_param
                            .currency_event
                            .send(ModifyTimeFragmentsEvent { delta: -cost });
                    } else {
                        //TODO: SCREEN SHAKE
                        return;
                    }
                }
                ObjectActionCost::Item(obj, count) => {
                    if inv.items.get_item_count_in_container(*obj) >= *count {
                        if let Err(err) = inv.items.remove_from_inventory(*count, *obj) {
                            error!("Error removing item from inventory: {:?}", err);
                            return;
                        }
                    } else {
                        //TODO: SCREEN SHAKE
                        return;
                    }
                }
                _ => {}
            }
        }
        match self {
            ObjectAction::ModifyHealth(delta) => {
                item_action_param
                    .modify_health_event
                    .send(ModifyHealthEvent(*delta));
            }
            ObjectAction::Teleport(pos) => {
                let pos = world_pos_to_tile_pos(*pos);
                item_action_param
                    .move_player_event
                    .send(MovePlayerEvent { pos });
            }
            ObjectAction::DungeonTeleport => {
                spawn_new_dungeon_dimension(
                    game,
                    commands,
                    &mut proto_param.proto_commands,
                    &mut item_action_param.move_player_event,
                );
            }
            ObjectAction::DungeonExit => {
                item_action_param.dim_event.send(DimensionSpawnEvent {
                    swap_to_dim_now: true,
                    new_era: Some(game.era.current_era.clone()),
                });
            }
            ObjectAction::Chest => {
                let chest_inv = item_action_param.chest_query.get(e).unwrap();
                commands.insert_resource(chest_inv.clone());
            }
            ObjectAction::Scrapper => {
                let scrapper_inv = item_action_param.scrapper_query.get(e).unwrap();
                commands.insert_resource(scrapper_inv.clone());
            }
            ObjectAction::ChangeObject(new_obj) => {
                commands.entity(e).despawn_recursive();
                let pos = item_action_param.cursor_pos.world_coords.truncate();
                game.remove_object_from_chunk_cache(world_pos_to_tile_pos(pos));

                item_action_param.place_item_event.send(PlaceItemEvent {
                    obj: *new_obj,
                    pos,
                    placed_by_player: true,
                    override_existing_obj: false,
                });
            }
            ObjectAction::Crafting(crafting_type) => {
                if !item_action_param
                    .crafting_tracker
                    .discovered_crafting_types
                    .contains(crafting_type)
                {
                    for item in item_action_param
                        .crafting_tracker
                        .discovered_objects
                        .clone()
                    {
                        for (result, recipe) in item_action_param.recipes.crafting_list.iter() {
                            if recipe.1 != *crafting_type {
                                continue;
                            }
                            if item_action_param
                                .crafting_tracker
                                .discovered_recipes
                                .contains(result)
                            {
                                continue;
                            }
                            for ingredient in recipe.0.iter() {
                                if ingredient.item == item {
                                    item_action_param
                                        .crafting_tracker
                                        .discovered_recipes
                                        .push(*result);
                                    item_action_param
                                        .crafting_tracker
                                        .crafting_type_map
                                        .entry(recipe.1.clone())
                                        .or_insert(vec![])
                                        .push(*result);
                                }
                            }
                        }
                    }
                }

                let items = if let Some(crafting_items) = item_action_param
                    .crafting_tracker
                    .crafting_type_map
                    .get(crafting_type)
                {
                    get_crafting_inventory_item_stacks(
                        crafting_items.to_vec(),
                        &item_action_param.recipes,
                        proto_param,
                    )
                } else {
                    vec![]
                };
                let crafting_container_res = CraftingContainer {
                    items: Container { items },
                };
                commands.insert_resource(crafting_container_res.clone());

                item_action_param
                    .crafting_tracker
                    .discovered_crafting_types
                    .push(crafting_type.clone());
            }
            ObjectAction::Furnace => {
                let furnace_res = item_action_param.furnace_query.get(e).unwrap();
                commands.insert_resource(furnace_res.clone());
            }
            ObjectAction::SetHome => {
                let pos =
                    world_pos_to_tile_pos(item_action_param.cursor_pos.world_coords.truncate());
                game.game.home_pos = Some(pos);
            }
            ObjectAction::CombatShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(4., TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
                let mut rng = rand::thread_rng();
                let num_days = 2 + item_action_param.night_tracker.days;
                let num_spawns_left = rng.gen_range(num_days..=(num_days + 2)) as usize;
                commands
                    .entity(e)
                    .insert(CombatShrine {
                        num_mobs_left: num_spawns_left,
                    })
                    .insert(AsepriteAnimation::from(CombatShrineAnim::tags::ACTIVATE))
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();
            }
            ObjectAction::GambleShrine => {
                // Screen Shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 120.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in item_action_param.game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(1.5, TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
                let mut rng = rand::thread_rng();
                commands
                    .entity(e)
                    .remove::<InteractionGuideTrigger>()
                    .remove::<ObjectAction>();
                if rng.gen_bool(0.35) {
                    commands
                        .entity(e)
                        .insert(GambleShrine { success: true })
                        .insert(AsepriteAnimation::from(
                            GambleShrineAnim::tags::ACTIVATE_SUCCESS,
                        ));
                } else {
                    commands
                        .entity(e)
                        .insert(GambleShrine { success: false })
                        .insert(AsepriteAnimation::from(
                            GambleShrineAnim::tags::ACTIVATE_FAIL,
                        ));
                }
            }
            _ => {}
        }
    }
}
