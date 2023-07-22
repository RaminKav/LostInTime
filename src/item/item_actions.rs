use std::marker::PhantomData;

use crate::{
    attributes::modifiers::ModifyHealthEvent, inputs::CursorPos, inventory::Inventory,
    player::MovePlayerEvent, proto::proto_param::ProtoParam, ui::InventoryState,
    world::world_helpers::world_pos_to_tile_pos,
};
use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use super::{PlaceItemEvent, WorldObject};

#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub enum ItemAction {
    #[default]
    None,
    ModifyHealth(i32),
    Teleport(Vec2),
    PlacesInto(WorldObject),
}
#[derive(Component, Reflect, FromReflect, Schematic, Default)]
#[reflect(Component, Schematic)]
pub struct ConsumableItem;

#[derive(SystemParam)]
pub struct ItemActionParam<'w, 's> {
    pub move_player_event: EventWriter<'w, MovePlayerEvent>,
    pub modify_health_event: EventWriter<'w, ModifyHealthEvent>,
    pub place_item_event: EventWriter<'w, PlaceItemEvent>,
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
                item_action_param.place_item_event.send(PlaceItemEvent {
                    obj: *obj,
                    pos: item_action_param.cursor_pos.world_coords.truncate(),
                });
            }
            _ => {}
        }
    }
}
