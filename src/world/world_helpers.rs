use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;

use super::{CHUNK_SIZE, TILE_SIZE};

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
pub fn camera_pos_to_block_pos(camera_pos: &Vec2) -> TilePos {
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
