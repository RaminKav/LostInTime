use bevy::prelude::*;

use crate::{
    player::MovePlayerEvent,
    world::dimension::{Dimension, GenerationSeed, SpawnDimension},
};

use super::{
    dungeon_generation::{gen_new_dungeon, get_player_spawn_tile, Bias},
    ChunkManager, CHUNK_SIZE,
};

#[derive(Component)]
pub struct Dungeon {
    pub grid: Vec<Vec<i8>>,
}
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, _app: &mut App) {}
}

impl DungeonPlugin {
    pub fn spawn_new_dungeon_dimension(
        commands: &mut Commands,
        mut move_player_event: EventWriter<MovePlayerEvent>,
    ) {
        let mut cm = ChunkManager::new();
        let grid = gen_new_dungeon(
            1500,
            (CHUNK_SIZE * 4) as usize,
            Bias {
                bias: super::dungeon_generation::Direction::Left,
                strength: 0,
            },
        );
        cm.world_generation_params.stone_frequency = 1.;
        cm.world_generation_params.dungeon_stone_frequency = 1.;

        let dim_e = commands
            .spawn((
                Dimension,
                Dungeon { grid: grid.clone() },
                GenerationSeed { seed: 123 },
                cm,
            ))
            .id();
        commands.entity(dim_e).insert(SpawnDimension);
        if let Some(pos) = get_player_spawn_tile(grid.clone()) {
            move_player_event.send(MovePlayerEvent {
                chunk_pos: pos.chunk_pos,
                tile_pos: pos.tile_pos,
            });
        }
    }
}
