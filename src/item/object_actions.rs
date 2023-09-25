use super::get_crafting_inventory_item_stacks;
use super::item_actions::ItemActionParam;

use crate::inventory::Container;
use crate::proto::proto_param::ProtoParam;
use crate::ui::crafting_ui::{CraftingContainer, CraftingContainerType};
use crate::world::dimension::DimensionSpawnEvent;
use crate::world::dungeon::{spawn_new_dungeon_dimension, DungeonPlugin};
use crate::GameParam;
use crate::{
    attributes::modifiers::ModifyHealthEvent, player::MovePlayerEvent,
    world::world_helpers::world_pos_to_tile_pos,
};
use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

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
}

impl ObjectAction {
    pub fn run_action(
        &self,
        e: Entity,
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
                spawn_new_dungeon_dimension(game, commands, &mut proto_param.proto_commands);
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
            _ => {}
        }
    }
}
