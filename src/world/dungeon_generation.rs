use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_inspector_egui::prelude::*;
use rand::rngs::ThreadRng;
use rand::seq::IteratorRandom;
use rand::Rng;

use super::{TileMapPositionData, CHUNK_SIZE};

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

pub fn get_player_spawn_tile(grid: Vec<Vec<i8>>) -> Option<TileMapPositionData> {
    let grid_size = grid.len() as i32 - 1;
    for y in 0..grid_size {
        let g_y = &grid[(grid_size - y) as usize];
        let picked_tile = g_y
            .iter()
            .enumerate()
            .filter(|(_, v)| *v == &1)
            .choose(&mut rand::thread_rng());
        if let Some((x, _)) = picked_tile {
            return Some(TileMapPositionData {
                chunk_pos: IVec2::new(
                    f64::floor((x as u32 - CHUNK_SIZE) as f64 / CHUNK_SIZE as f64) as i32,
                    -(f64::floor((grid_size - y - 1) as f64 / CHUNK_SIZE as f64) as i32 - 1),
                ),
                tile_pos: TilePos {
                    x: x as u32 % CHUNK_SIZE,
                    y: CHUNK_SIZE - ((grid_size - y) as u32 % CHUNK_SIZE) - 1,
                },
            });
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
