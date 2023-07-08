use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;

use crate::GameParam;

use super::{TileMapPositionData, WorldObjectEntityData, CHUNK_SIZE, TILE_SIZE};

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
pub fn world_pos_to_tile_pos(pos: Vec2) -> TileMapPositionData {
    let chunk_pos = camera_pos_to_chunk_pos(&pos);
    let tile_pos = camera_pos_to_tile_pos(&pos);
    TileMapPositionData {
        chunk_pos,
        tile_pos,
    }
}
pub fn tile_pos_to_world_pos(pos: TileMapPositionData) -> Vec2 {
    Vec2::new(
        pos.tile_pos.x as f32 * TILE_SIZE.x
            + pos.chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
        pos.tile_pos.y as f32 * TILE_SIZE.x
            + pos.chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
    )
}

pub fn get_neighbours_tile(pos: TileMapPositionData, offset: (i8, i8)) -> TileMapPositionData {
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
    TileMapPositionData {
        chunk_pos: adjusted_chunk_pos,
        tile_pos: neighbour_wall_pos,
    }
}

pub fn get_neighbour_obj_data(
    pos: TileMapPositionData,
    offset: (i8, i8),
    game: &mut GameParam,
) -> Option<WorldObjectEntityData> {
    let TileMapPositionData {
        chunk_pos: adjusted_chunk_pos,
        tile_pos: neighbour_wall_pos,
    } = get_neighbours_tile(pos, offset);

    if game.get_chunk_entity(adjusted_chunk_pos).is_none() {
        return None;
    }

    if let Some(d) = game.get_tile_obj_data(TileMapPositionData {
        chunk_pos: adjusted_chunk_pos,
        tile_pos: neighbour_wall_pos,
    }) {
        return Some(d.clone());
    }
    None
}
