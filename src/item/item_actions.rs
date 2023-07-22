use std::marker::PhantomData;

use crate::{
    attributes::modifiers::ModifyHealthEvent, inputs::CursorPos, player::MovePlayerEvent,
    world::world_helpers::world_pos_to_tile_pos,
};
use bevy::{ecs::system::SystemParam, prelude::*};

use super::WorldObject;
pub enum ItemAction {
    ModifyHealth(i32),
    Teleport(Vec2),
    PlacesInto(WorldObject),
}

#[derive(SystemParam)]
pub struct ItemActionParam<'w, 's> {
    pub move_player_event: EventWriter<'w, MovePlayerEvent>,
    pub modify_health_event: EventWriter<'w, ModifyHealthEvent>,
    pub cursor_pos: Res<'w, CursorPos>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}

impl ItemAction {
    pub fn run_action(&self, item_action_param: &mut ItemActionParam) {
        match self {
            ItemAction::ModifyHealth(delta) => {
                item_action_param
                    .modify_health_event
                    .send(ModifyHealthEvent(*delta));
            }
            ItemAction::Teleport(pos) => {
                let pos = world_pos_to_tile_pos(*pos);
                item_action_param.move_player_event.send(MovePlayerEvent {
                    chunk_pos: pos.chunk_pos,
                    tile_pos: pos.tile_pos,
                });
            }
            ItemAction::PlacesInto(obj) => {
                //TODO: add place into item event and system that listens to it
            }
        }
    }
}
