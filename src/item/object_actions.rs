use super::item_actions::ItemActionParam;

use crate::world::dungeon::DungeonPlugin;
use crate::{
    attributes::modifiers::ModifyHealthEvent, player::MovePlayerEvent,
    world::world_helpers::world_pos_to_tile_pos,
};
use bevy::prelude::*;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum ObjectAction {
    #[default]
    None,
    ModifyHealth(i32),
    Teleport(Vec2),
    DungeonTeleport,
    Chest,
    //MobRune - obj that if activated spawns a bunch of mobs, and when slain gives a chest reward?
}

impl ObjectAction {
    pub fn run_action(
        &self,
        e: Entity,
        item_action_param: &mut ItemActionParam,
        commands: &mut Commands,
        proto_commands: &mut ProtoCommands,
    ) {
        match self {
            ObjectAction::ModifyHealth(delta) => {
                item_action_param
                    .modify_health_event
                    .send(ModifyHealthEvent(*delta));
            }
            ObjectAction::Teleport(pos) => {
                let pos = world_pos_to_tile_pos(*pos);
                item_action_param.move_player_event.send(MovePlayerEvent {
                    chunk_pos: pos.chunk_pos,
                    tile_pos: pos.tile_pos,
                });
            }
            ObjectAction::DungeonTeleport => {
                DungeonPlugin::spawn_new_dungeon_dimension(commands, proto_commands);
            }
            ObjectAction::Chest => {
                let chest_inv = item_action_param.chest_query.get(e).unwrap();
                commands.insert_resource(chest_inv.clone());
            }
            _ => {}
        }
    }
}
