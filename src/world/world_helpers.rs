use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;

use super::{TileMapPositionData, CHUNK_SIZE, TILE_SIZE};

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
