use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;

use crate::{item::WorldObject, proto::proto_param::ProtoParam, GameParam};

use super::{TileMapPosition, WallTextureData, CHUNK_SIZE, TILE_SIZE};

/// gets the chunk from pixel world coordinates
pub fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    // do this bc we want bottom left of the block to be 0,0 instead of centre
    let camera_pos = Vec2::new(
        camera_pos.x + (TILE_SIZE.x / 2.),
        camera_pos.y + (TILE_SIZE.y / 2.),
    );
    IVec2::new(
        (camera_pos.x / (CHUNK_SIZE as f32 * TILE_SIZE.x)).floor() as i32,
        (camera_pos.y / (CHUNK_SIZE as f32 * TILE_SIZE.y)).floor() as i32,
    )
}
/// gets the tile from pixel world coordinates
pub fn camera_pos_to_tile_pos(camera_pos: &Vec2) -> TilePos {
    let camera_pos = Vec2::new(
        camera_pos.x + (TILE_SIZE.x / 2.),
        camera_pos.y + (TILE_SIZE.y / 2.),
    );

    let mut block_pos = IVec2::new(
        ((camera_pos.x % (CHUNK_SIZE as f32 * TILE_SIZE.x)) / TILE_SIZE.x).floor() as i32,
        ((camera_pos.y % (CHUNK_SIZE as f32 * TILE_SIZE.y)) / TILE_SIZE.y).floor() as i32,
    );
    // do this bc bottom left is 0,0
    if block_pos.x < 0 {
        block_pos.x += CHUNK_SIZE as i32
    }
    if block_pos.y < 0 {
        block_pos.y += CHUNK_SIZE as i32;
    }

    TilePos {
        x: block_pos.x as u32,
        y: block_pos.y as u32,
    }
}
/// gets the [TileMapPosition] from pixel world coordinates
pub fn world_pos_to_tile_pos(pos: Vec2) -> TileMapPosition {
    let chunk_pos = camera_pos_to_chunk_pos(&pos);
    let tile_pos = camera_pos_to_tile_pos(&pos);

    TileMapPosition::new(chunk_pos, tile_pos)
}
pub fn world_pos_to_chunk_relative_world_pos(pos: Vec2) -> Vec2 {
    let chunk_pos = camera_pos_to_chunk_pos(&pos);
    pos - (Vec2::new(chunk_pos.x as f32, chunk_pos.y as f32) * CHUNK_SIZE as f32 * TILE_SIZE.x)
}

pub fn world_pos_to_chunk_relative_tile_pos(pos: Vec2) -> TileMapPosition {
    let chunk_relative_pos = world_pos_to_chunk_relative_world_pos(pos);
    world_pos_to_tile_pos(chunk_relative_pos)
}

//TODO: remove center, old code
pub fn tile_pos_to_world_pos(pos: TileMapPosition, _center: bool) -> Vec2 {
    Vec2::new(
        pos.tile_pos.x as f32 * TILE_SIZE.x
            + pos.chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
        pos.tile_pos.y as f32 * TILE_SIZE.x
            + pos.chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
    )
}

pub fn get_neighbour_tile(pos: TileMapPosition, offset: (i8, i8)) -> TileMapPosition {
    let dx = offset.0;
    let dy = offset.1;
    let x = pos.tile_pos.x as i8;
    let y = pos.tile_pos.y as i8;
    let chunk_pos = pos.chunk_pos;
    let mut neighbour_wall_pos = TilePos {
        x: (dx + x) as u32,
        y: (dy + y) as u32,
    };
    let mut adjusted_chunk_pos = pos.chunk_pos;
    if x + dx < 0 {
        adjusted_chunk_pos.x = chunk_pos.x - 1;
        neighbour_wall_pos.x = CHUNK_SIZE - 1;
    } else if x + dx >= CHUNK_SIZE.try_into().unwrap() {
        adjusted_chunk_pos.x = chunk_pos.x + 1;
        neighbour_wall_pos.x = 0;
    }
    if y + dy < 0 {
        adjusted_chunk_pos.y = chunk_pos.y - 1;
        neighbour_wall_pos.y = CHUNK_SIZE - 1;
    } else if y + dy >= CHUNK_SIZE.try_into().unwrap() {
        adjusted_chunk_pos.y = chunk_pos.y + 1;
        neighbour_wall_pos.y = 0;
    }
    TileMapPosition::new(adjusted_chunk_pos, neighbour_wall_pos)
}
pub fn get_neighbour_quadrant(pos: TileMapPosition, offset: (i8, i8)) -> TileMapPosition {
    let parent_world_pos = tile_pos_to_world_pos(pos, false);
    let dx = offset.0 as f32 * TILE_SIZE.x / 2.;
    let dy = offset.1 as f32 * TILE_SIZE.x / 2.;
    world_pos_to_tile_pos(parent_world_pos + Vec2::new(dx, dy))
}

pub fn get_neighbour_wall_data(
    pos: TileMapPosition,
    offset: (i8, i8),
    game: &mut GameParam,
    proto_param: &ProtoParam,
) -> Option<WallTextureData> {
    let pos = get_neighbour_quadrant(pos, offset);

    if game.get_chunk_entity(pos.chunk_pos).is_none() {
        return None;
    }
    game.get_wall_data_at_tile(pos, &proto_param)
}

pub fn can_object_be_placed_here(
    tile_pos: TileMapPosition,
    game: &mut GameParam,
    obj: WorldObject,
    proto_param: &ProtoParam,
) -> bool {
    if let Some(tile_data) = game.get_tile_data(tile_pos) {
        if tile_data.block_type.contains(&WorldObject::WaterTile) && obj != WorldObject::Bridge {
            debug!("water here {tile_pos:?}");
            return false;
        }
    }

    let is_medium = obj.is_medium_size(proto_param);
    if is_medium
        && (game
            .get_obj_entity_at_tile(tile_pos, &proto_param)
            .is_some()
            || game
                .get_obj_entity_at_tile(
                    tile_pos.get_neighbour_tiles_for_medium_objects()[0],
                    &proto_param,
                )
                .is_some()
            || game
                .get_obj_entity_at_tile(
                    tile_pos.get_neighbour_tiles_for_medium_objects()[1],
                    &proto_param,
                )
                .is_some()
            || game
                .get_obj_entity_at_tile(
                    tile_pos.get_neighbour_tiles_for_medium_objects()[2],
                    &proto_param,
                )
                .is_some())
    {
        debug!("obj exists here {tile_pos:?}");
        return false;
    } else if let Some(_existing_object) = game.get_obj_entity_at_tile(tile_pos, &proto_param) {
        debug!("obj exists here {tile_pos:?}");
        return false;
    }
    true
}
