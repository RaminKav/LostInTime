use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_inspector_egui::prelude::*;
use rand::rngs::ThreadRng;
use rand::seq::IteratorRandom;
use rand::Rng;

use crate::{
    item::{PlaceItemEvent, WorldObject},
    schematic::loot_chests::LootChestType,
    world::world_helpers::{tile_pos_to_world_pos, world_pos_to_tile_pos},
};

use super::{dimension::ActiveDimension, dungeon::Dungeon, TileMapPosition, CHUNK_SIZE};
///
///   grid is indexed as [y][x], where y = 0 is the top row, or chunk.y == 1 && tile.y == 15
///   and y = 127 is the bottom row, or chunk.y == -2 && tile.y == 0
///   and  x = 0 is the left col, or chunk.x == -1 && tile.x == 0
///   and  x = 127  is the right col, or chunk.x == 2 && tile.x == 15
///
///   grid has spots for quadrants, so each entry is one of 4 quadrants belonging
///   to one tile.
#[derive(Reflect, Resource, Clone, Debug, Default, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}
struct Walker {
    pos: Vec2,
}

#[derive(Reflect, Resource, Clone, InspectorOptions)]
pub struct NumSteps(i32);
impl Default for NumSteps {
    fn default() -> Self {
        Self(100)
    }
}
#[derive(Reflect, Resource, Clone, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct GridSize(usize);
impl Default for GridSize {
    fn default() -> Self {
        Self(32)
    }
}

#[derive(Reflect, Resource, Clone, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct Bias {
    pub bias: Direction,
    #[inspector(min = 0, max = 50)]
    pub strength: u32,
}
impl Default for Bias {
    fn default() -> Self {
        Self {
            bias: Direction::Left,
            strength: 0,
        }
    }
}

impl Direction {
    pub fn get_next_dir(mut rng: ThreadRng, bias: Bias) -> Self {
        let is_biased = rng.gen_ratio(50 + bias.strength, 100);
        let which_dir = rng.gen_ratio(1, 2);
        if is_biased {
            return match which_dir {
                true => bias.bias,
                false => bias.bias.get_opposite(),
            };
        } else {
            return match which_dir {
                true => bias.bias.get_neighbour(),
                false => bias.bias.get_opposite().get_neighbour(),
            };
        }
    }
    fn get_opposite(&self) -> Self {
        match self {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
        }
    }
    fn get_neighbour(&self) -> Self {
        match self {
            Direction::Up => Direction::Right,
            Direction::Right => Direction::Down,
            Direction::Down => Direction::Left,
            Direction::Left => Direction::Up,
        }
    }
}

pub fn get_player_spawn_tile(grid: Vec<Vec<i8>>) -> Option<TileMapPosition> {
    let grid_size = grid.len() as i32 - 1;
    for mut y in 0..grid_size {
        //start from bottom row, cy == -2, ty == 0
        y = grid_size - y;
        let g_y = &grid[y as usize];

        let picked_tile = g_y
            .iter()
            .enumerate()
            .filter(|(_, v)| *v == &1)
            .choose(&mut rand::thread_rng());

        if let Some((x, _)) = picked_tile {
            let player_tile_pos = TileMapPosition::new(
                IVec2::new(
                    f64::floor((x as f64 - 3. * CHUNK_SIZE as f64) as f64 / (CHUNK_SIZE) as f64)
                        as i32,
                    f64::floor(
                        ((3. * CHUNK_SIZE as f64) - y as f64 - 1.) as f64 / (CHUNK_SIZE) as f64,
                    ) as i32
                        + 1,
                ),
                TilePos {
                    x: f64::floor(x as f64 % (CHUNK_SIZE) as f64) as u32,
                    y: f64::ceil(CHUNK_SIZE as f64 - (y as f64 % (CHUNK_SIZE) as f64) as f64)
                        as u32
                        - 1,
                },
            );
            let temp_world_pos = tile_pos_to_world_pos(player_tile_pos, false) + Vec2::new(0., 9.);
            let player_pos = world_pos_to_tile_pos(temp_world_pos);
            return Some(player_pos);
        }
    }
    None
}
//TODO: add seed to this rng
pub fn gen_new_dungeon(steps: i32, grid_size: usize, bias: Bias) -> Vec<Vec<i8>> {
    let mut grid: Vec<Vec<i8>> = vec![vec![0; grid_size as usize]; grid_size as usize];
    let mut walker = Walker {
        pos: Vec2::new((grid_size / 2) as f32, (grid_size / 2) as f32),
    };
    let num_steps = steps;

    for _ in 0..num_steps {
        let new_dir = Direction::get_next_dir(rand::thread_rng(), bias.clone());
        grid[walker.pos.x as usize][walker.pos.y as usize] = 1;
        match new_dir {
            Direction::Down => walker.pos.y -= 1.,
            Direction::Up => walker.pos.y += 1.,
            Direction::Left => walker.pos.x -= 1.,
            Direction::Right => walker.pos.x += 1.,
        }
        if walker.pos.x > (grid_size - 1) as f32 {
            walker.pos.x = (grid_size - 1) as f32
        }
        if walker.pos.y > (grid_size - 1) as f32 {
            walker.pos.y = (grid_size - 1) as f32
        }
    }
    grid
}
pub fn add_dungeon_chests(
    new_dungeon: Query<&Dungeon, Added<ActiveDimension>>,
    mut place_item_event: EventWriter<PlaceItemEvent>,
) {
    let Ok(dungeon) = new_dungeon.get_single() else {
        return;
    };
    let mut rng = rand::thread_rng();
    let grid_size = dungeon.grid.len();
    let mut picked_x;
    let mut picked_y;
    let mut num_chests_left_to_spawn = if rng.gen_ratio(3, 4) { 2 } else { 1 };
    let mut chest_positions = vec![];

    while num_chests_left_to_spawn > 0 {
        picked_x = rng.gen_range(0..grid_size - 1);
        picked_y = rng.gen_range(0..grid_size - 1);
        if dungeon.grid[picked_y][picked_x] == 1 {
            if dungeon.grid[0.max(picked_y as i32 - 1) as usize][picked_x] == 0 {
                let pos = TileMapPosition::new(
                    IVec2::new(
                        f64::floor(
                            (picked_x as f64 - 3. * CHUNK_SIZE as f64) as f64 / (CHUNK_SIZE) as f64,
                        ) as i32,
                        f64::floor(
                            ((3. * CHUNK_SIZE as f64) - picked_y as f64 - 1.) as f64
                                / (CHUNK_SIZE) as f64,
                        ) as i32
                            + 1,
                    ),
                    TilePos {
                        x: f64::floor(picked_x as f64 % (CHUNK_SIZE) as f64) as u32,
                        y: f64::ceil(
                            CHUNK_SIZE as f64 - (picked_y as f64 % (CHUNK_SIZE) as f64) as f64,
                        ) as u32
                            - 1,
                    },
                );
                chest_positions.push(pos);
                num_chests_left_to_spawn -= 1;
            }
        }
    }
    for (i, pos) in chest_positions.iter().enumerate() {
        place_item_event.send(PlaceItemEvent {
            obj: WorldObject::Chest,
            pos: tile_pos_to_world_pos(*pos, false),
            placed_by_player: false,
        });
    }
}
pub fn add_dungeon_exit_block(
    new_dungeon: Query<&Dungeon, Added<ActiveDimension>>,
    mut place_item_event: EventWriter<PlaceItemEvent>,
) {
    let Ok(dungeon) = new_dungeon.get_single() else {
        return;
    };
    let mut rng = rand::thread_rng();
    let grid_size = dungeon.grid.len();
    let mut picked_x;
    let mut picked_y;
    let mut num_exits_left_to_spawn = 1;
    let mut chest_positions = vec![];

    while num_exits_left_to_spawn > 0 {
        picked_x = rng.gen_range(0..grid_size - 1);
        picked_y = rng.gen_range(0..grid_size - 1);
        if dungeon.grid[picked_y][picked_x] == 1 {
            if dungeon.grid[0.max(picked_y as i32 - 1) as usize][picked_x] == 0 {
                let pos = TileMapPosition::new(
                    IVec2::new(
                        f64::floor(
                            (picked_x as f64 - 3. * CHUNK_SIZE as f64) as f64 / (CHUNK_SIZE) as f64,
                        ) as i32,
                        f64::floor(
                            ((3. * CHUNK_SIZE as f64) - picked_y as f64 - 1.) as f64
                                / (CHUNK_SIZE) as f64,
                        ) as i32
                            + 1,
                    ),
                    TilePos {
                        x: f64::floor(picked_x as f64 % (CHUNK_SIZE) as f64) as u32,
                        y: f64::ceil(
                            CHUNK_SIZE as f64 - (picked_y as f64 % (CHUNK_SIZE) as f64) as f64,
                        ) as u32
                            - 1,
                    },
                );
                chest_positions.push(pos);
                num_exits_left_to_spawn -= 1;
            }
        }
    }
    for (i, pos) in chest_positions.iter().enumerate() {
        place_item_event.send(PlaceItemEvent {
            obj: WorldObject::DungeonExit,
            pos: tile_pos_to_world_pos(*pos, false),
            placed_by_player: false,
        });
    }
}
