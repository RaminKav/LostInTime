use bevy::math::Vec2;

use crate::{
    item::WorldObject, proto::proto_param::ProtoParam, world::world_helpers::world_pos_to_tile_pos,
    GameParam,
};

pub fn can_spawn_mob_here(
    pos: Vec2,
    game: &GameParam,
    proto_param: &ProtoParam,
    ignore_objs: bool,
) -> bool {
    let tile_pos = world_pos_to_tile_pos(pos);
    if !ignore_objs {
        if let Some(_existing_object) = game.get_obj_entity_at_tile(tile_pos, proto_param) {
            return false;
        }
    }
    if is_tile_water(pos, game).is_ok_and(|x| x) {
        return false;
    }
    true
}

pub fn is_tile_water(pos: Vec2, game: &GameParam) -> Result<bool, ()> {
    let tile_pos = world_pos_to_tile_pos(pos);
    if let Some(tile_data) = game.get_tile_data(tile_pos) {
        return Ok(tile_data.block_type.contains(&WorldObject::WaterTile));
    }
    Err(())
}
