use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;

use crate::{world::CHUNK_SIZE, CoreGameSet, Player, RawPosition};

pub struct MovePlayerEvent {
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}
pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MovePlayerEvent>()
            .add_system(handle_move_player.in_base_set(CoreGameSet::Main));
    }
}
pub fn handle_move_player(
    mut player: Query<&mut RawPosition, With<Player>>,
    mut move_events: EventReader<MovePlayerEvent>,
) {
    for m in move_events.iter() {
        //TODO: Add world helper to get chunk -> world pos, lots of copy code in item.rs
        let new_pos = Vec3::new(
            (m.tile_pos.x as i32 * 32 + m.chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            (m.tile_pos.y as i32 * 32 + m.chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
            0.,
        );
        player.single_mut().0 = new_pos.truncate();
    }
}
