use bevy::prelude::*;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_inspector_egui::prelude::*;
use rand::rngs::ThreadRng;
use rand::Rng;

use crate::{
    item::{PlaceItemEvent, WorldObject},
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

pub const DUNGEON_GRID_SIZE: u32 = CHUNK_SIZE * 4 * 2;
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
            match which_dir {
                true => bias.bias,
                false => bias.bias.get_opposite(),
            }
        } else {
            match which_dir {
                true => bias.bias.get_neighbour(),
                false => bias.bias.get_opposite().get_neighbour(),
            }
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
    let grid_size = grid.len();
    let center_x = grid_size / 2;
    let center_y = grid_size / 2;

    // Spawn player at the south of the circular room (fixed position)
    let player_x = center_x;
    let player_y = center_y + 10; // South of center

    // Ensure the position is within bounds and walkable
    if player_x < grid_size && player_y < grid_size && grid[player_y][player_x] == 1 {
        let player_tile_pos = TileMapPosition::new(
            IVec2::new(
                f64::floor((player_x as f64 - 3. * CHUNK_SIZE as f64) / (CHUNK_SIZE) as f64) as i32,
                f64::floor(((3. * CHUNK_SIZE as f64) - player_y as f64 - 1.) / (CHUNK_SIZE) as f64)
                    as i32
                    + 1,
            ),
            TilePos {
                x: f64::floor(player_x as f64 % (CHUNK_SIZE) as f64) as u32,
                y: f64::ceil(CHUNK_SIZE as f64 - (player_y as f64 % (CHUNK_SIZE) as f64)) as u32
                    - 1,
            },
        );
        let temp_world_pos = tile_pos_to_world_pos(player_tile_pos, false) + Vec2::new(0., 9.);
        let player_pos = world_pos_to_tile_pos(temp_world_pos);
        return Some(player_pos);
    }
    None
}
//TODO: add seed to this rng
pub fn _gen_old_dungeon(steps: i32, grid_size: usize, bias: Bias) -> Vec<Vec<i8>> {
    let mut grid: Vec<Vec<i8>> = vec![vec![0; grid_size]; grid_size];
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

// Simplified dungeon generation - fixed centered room with predictable spawns
pub fn gen_new_room_dungeon(grid_size: usize) -> Vec<Vec<i8>> {
    let mut grid: Vec<Vec<i8>> = vec![vec![0; grid_size]; grid_size];
    let mut rng = rand::thread_rng();

    // Fixed room parameters - centered around (0,0) with fixed radius
    let room_radius = 16;
    let center_x = grid_size / 2;
    let center_y = grid_size / 2;

    // Create a circular room centered at the middle of the grid
    for x in 0..grid_size {
        for y in 0..grid_size {
            let dx = (x as i32 - center_x as i32) as f32;
            let dy = (y as i32 - center_y as i32) as f32;
            let distance = (dx * dx + dy * dy).sqrt();

            if distance <= room_radius as f32 {
                grid[y][x] = 1; // Walkable area
            }
        }
    }

    // Add some jagged wall texture by creating small cuts around the edges
    let num_wall_cuts = rng.gen_range(6..=10);
    for _ in 0..num_wall_cuts {
        let cut_width = rng.gen_range(2..=4);
        let cut_height = rng.gen_range(2..=4);

        // Random angle around the circle
        let angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
        let cut_distance = room_radius + rng.gen_range(2..=6);

        let cut_center_x = center_x as f32 + angle.cos() * cut_distance as f32;
        let cut_center_y = center_y as f32 + angle.sin() * cut_distance as f32;

        let start_x = (cut_center_x - cut_width as f32 / 2.0) as usize;
        let start_y = (cut_center_y - cut_height as f32 / 2.0) as usize;

        // Fill the cut
        for cut_x in start_x..(start_x + cut_width) {
            for cut_y in start_y..(start_y + cut_height) {
                if cut_x < grid_size && cut_y < grid_size {
                    grid[cut_y][cut_x] = 1;
                }
            }
        }
    }

    // Add a few internal pillars for visual interest
    let num_pillars = rng.gen_range(6..=10);
    for _ in 0..num_pillars {
        let pillar_angle = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
        let pillar_distance_x = rng.gen_range(3..=room_radius - 3);
        let pillar_distance_y = rng.gen_range(2..=10);
        let pillar_size = rng.gen_range(2..=4);

        let pillar_center_x = center_x as f32 + pillar_angle.cos() * pillar_distance_x as f32;
        let pillar_center_y = center_y as f32 + pillar_angle.sin() * pillar_distance_y as f32;

        let start_x = (pillar_center_x - pillar_size as f32 / 2.0) as usize;
        let start_y = usize::max((pillar_center_y - pillar_size as f32 / 2.0) as usize, 60);
        for px in start_x..(start_x + pillar_size) {
            for py in start_y..(start_y + pillar_size) {
                if px < grid_size && py < grid_size {
                    grid[py][px] = 0;
                }
            }
        }
    }

    grid
}

pub fn add_dungeon_shrines(
    new_dungeon: Query<&Dungeon, Added<ActiveDimension>>,
    mut place_item_event: EventWriter<PlaceItemEvent>,
) {
    let Ok(dungeon) = new_dungeon.get_single() else {
        return;
    };
    let grid_size = dungeon.grid.len();

    // Fixed shrine positions - north side of the circular room
    let center_x = grid_size / 2;
    let center_y = grid_size / 2;
    let shrine_y = center_y - 10; // North of center

    let shrine_types = [
        WorldObject::WeaponShrine,
        WorldObject::ArmorShrine,
        WorldObject::AccessoryShrine,
    ];

    // Place shrines in a horizontal line with fixed spacing
    let shrine_spacing = 4;
    for (i, shrine_type) in shrine_types.iter().enumerate() {
        let offset = (i as i32 - 1) * shrine_spacing;
        let shrine_x = (center_x as i32 + offset) as usize;

        // Ensure position is within bounds and walkable
        if shrine_x < grid_size && shrine_y < grid_size && dungeon.grid[shrine_y][shrine_x] == 1 {
            let pos = TileMapPosition::new(
                IVec2::new(
                    f64::floor((shrine_x as f64 - 3. * CHUNK_SIZE as f64) / (CHUNK_SIZE) as f64)
                        as i32,
                    f64::floor(
                        ((3. * CHUNK_SIZE as f64) - shrine_y as f64 - 1.) / (CHUNK_SIZE) as f64,
                    ) as i32
                        + 1,
                ),
                TilePos {
                    x: f64::floor(shrine_x as f64 % (CHUNK_SIZE) as f64) as u32,
                    y: f64::ceil(CHUNK_SIZE as f64 - (shrine_y as f64 % (CHUNK_SIZE) as f64))
                        as u32
                        - 1,
                },
            );

            place_item_event.send(PlaceItemEvent {
                obj: *shrine_type,
                pos: tile_pos_to_world_pos(pos, false),
                placed_by_player: false,
                override_existing_obj: false,
            });
        }
    }
}
