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
        if let Some(_existing_object) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
            return false;
        }
    }
    if game
        .get_tile_data(tile_pos)
        .expect("spawned mob but tile does not exist?")
        .block_type
        .contains(&WorldObject::WaterTile)
    {
        return false;
    }

    true
}
