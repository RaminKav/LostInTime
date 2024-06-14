use super::combat_shrine::CombatShrine;
use super::item_actions::ItemActionParam;
use super::{get_crafting_inventory_item_stacks, PlaceItemEvent, WorldObject};

use crate::container::Container;
use crate::custom_commands::CommandsExt;
use crate::enemy::{CombatAlignment, EliteMob, Mob};
use crate::item::combat_shrine::CombatShrineMob;
use crate::item::LootTable;
use crate::proto::proto_param::ProtoParam;
use crate::ui::crafting_ui::{CraftingContainer, CraftingContainerType};
use crate::world::dimension::DimensionSpawnEvent;
use crate::world::dungeon::spawn_new_dungeon_dimension;

use crate::world::world_helpers::tile_pos_to_world_pos;
use crate::world::{TileMapPosition, TILE_SIZE};
use crate::GameParam;
use crate::{
    attributes::modifiers::ModifyHealthEvent, player::MovePlayerEvent,
    world::world_helpers::world_pos_to_tile_pos,
};
use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use rand::Rng;

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum ObjectAction {
    #[default]
    None,
    ModifyHealth(i32),
    Teleport(Vec2),
    DungeonTeleport,
    DungeonExit,
    Chest,
    Crafting(CraftingContainerType), //MobRune - obj that if activated spawns a bunch of mobs, and when slain gives a chest reward?
    Furnace, //MobRune - obj that if activated spawns a bunch of mobs, and when slain gives a chest reward?
    ChangeObject(WorldObject),
    SetHome,
    CombatShrine,
}

impl ObjectAction {
    pub fn run_action(
        &self,
        e: Entity,
        obj_pos: TileMapPosition,
        game: &mut GameParam,
        item_action_param: &mut ItemActionParam,
        commands: &mut Commands,
        proto_param: &mut ProtoParam,
    ) {
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
                    generation_params: proto_param.get_world_gen().unwrap(),
                    swap_to_dim_now: true,
                });
            }
            ObjectAction::Chest => {
                let chest_inv = item_action_param.chest_query.get(e).unwrap();
                commands.insert_resource(chest_inv.clone());
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
                let crafting_items = item_action_param
                    .crafting_tracker
                    .crafting_type_map
                    .get(crafting_type)
                    .unwrap();
                let crafting_container_res = CraftingContainer {
                    items: Container {
                        items: get_crafting_inventory_item_stacks(
                            crafting_items.to_vec(),
                            &item_action_param.recipes,
                            proto_param,
                        ),
                    },
                };
                commands.insert_resource(crafting_container_res.clone());
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
                let mut rng = rand::thread_rng();
                let num_days = 4 + item_action_param.night_tracker.days;
                let mut num_spawns_left = rng.gen_range(num_days..=(num_days + 2)) as usize;
                commands.entity(e).insert(CombatShrine {
                    num_mobs_left: num_spawns_left,
                });
                let possible_spawns =
                    [Mob::FurDevil, Mob::Bushling, Mob::StingFly, Mob::SpikeSlime];
                while num_spawns_left > 0 {
                    let offset = Vec2::new(rng.gen_range(-3. ..=3.), rng.gen_range(-3. ..=3.))
                        * Vec2::splat(TILE_SIZE.x);
                    let choice_mob = rng.gen_range(0..possible_spawns.len());
                    num_spawns_left -= 1;

                    if let Some(mob) = proto_param.proto_commands.spawn_from_proto(
                        possible_spawns[choice_mob].clone(),
                        &proto_param.prototypes,
                        tile_pos_to_world_pos(obj_pos, true) + offset,
                    ) {
                        //last mob is elite
                        if num_spawns_left == 0 {
                            commands.entity(mob).insert(EliteMob);
                        }
                        proto_param
                            .proto_commands
                            .commands()
                            .entity(mob)
                            .insert(CombatAlignment::Hostile)
                            .insert(LootTable::default())
                            .insert(CombatShrineMob { parent_shrine: e });
                    }
                }
            }
            _ => {}
        }
    }
}
