use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;

use crate::{
    player::MovePlayerEvent,
    world::dimension::{Dimension, SpawnDimension},
};

use super::{
    dimension::ActiveDimension,
    dungeon_generation::{add_dungeon_chests, gen_new_dungeon, get_player_spawn_tile, Bias},
    CHUNK_SIZE,
};

#[derive(Component)]
pub struct Dungeon {
    pub grid: Vec<Vec<i8>>,
}
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(Self::handle_move_player_after_dungeon_gen)
            .add_system(add_dungeon_chests);
    }
}

impl DungeonPlugin {
    pub fn spawn_new_dungeon_dimension(
        commands: &mut Commands,
        proto_commands: &mut ProtoCommands,
    ) {
        let grid = gen_new_dungeon(
            2000 * 2,
            // 250,
            (CHUNK_SIZE * 4 * 2) as usize,
            Bias {
                bias: super::dungeon_generation::Direction::Left,
                strength: 0,
            },
        );

        let dim_e = commands
            .spawn((Dimension, Dungeon { grid: grid.clone() }))
            .id();
        proto_commands.apply("DungeonWorldGenerationParams");
        commands.entity(dim_e).insert(SpawnDimension);
    }
    fn handle_move_player_after_dungeon_gen(
        new_dungeon: Query<&Dungeon, Added<ActiveDimension>>,
        mut move_player_event: EventWriter<MovePlayerEvent>,
    ) {
        if let Ok(dungeon) = new_dungeon.get_single() {
            let grid = &dungeon.grid;
            if let Some(pos) = get_player_spawn_tile(grid.clone()) {
                move_player_event.send(MovePlayerEvent { pos });
            }
        }
    }
}
