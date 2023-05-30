use bevy::{prelude::*, utils::HashMap};

use crate::{
    player::MovePlayerEvent,
    world::dimension::{Dimension, GenerationSeed, SpawnDimension},
};

use super::{
    dimension::{ActiveDimension, ChunkCache},
    dungeon_generation::{gen_new_dungeon, get_player_spawn_tile, Bias},
    ChunkManager, CHUNK_SIZE,
};

#[derive(Component)]
pub struct Dungeon {
    pub grid: Vec<Vec<i8>>,
}
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(Self::handle_move_player_after_dungeon_gen);
    }
}

impl DungeonPlugin {
    pub fn spawn_new_dungeon_dimension(commands: &mut Commands) {
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
                ChunkCache {
                    snapshots: HashMap::new(),
                },
                cm,
            ))
            .id();
        commands.entity(dim_e).insert(SpawnDimension);
    }
    fn handle_move_player_after_dungeon_gen(
        new_dungeon: Query<&Dungeon, Added<ActiveDimension>>,
        mut move_player_event: EventWriter<MovePlayerEvent>,
    ) {
        if let Ok(dungeon) = new_dungeon.get_single() {
            let grid = &dungeon.grid;
            if let Some(pos) = get_player_spawn_tile(grid.clone()) {
                move_player_event.send(MovePlayerEvent {
                    chunk_pos: pos.chunk_pos,
                    tile_pos: pos.tile_pos,
                });
            }
        }
    }
}
