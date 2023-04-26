use bevy::prelude::*;
use bevy_inspector_egui::prelude::*;
use rand::rngs::ThreadRng;
use rand::Rng;

#[derive(Reflect, Resource, Clone, Debug, Default, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub enum Direction {
    #[default]
    Left,
    Right,
    Up,
    Down,
}
pub struct Walker {
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
    bias: Direction,
    #[inspector(min = 0, max = 50)]
    strength: u32,
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
fn key_event(
    mut commands: Commands,
    key_input: ResMut<Input<KeyCode>>,
    old: Query<Entity, With<Sprite>>,
    steps: Res<NumSteps>,
    grid_size: Res<GridSize>,
    bias: Res<Bias>,
) {
    if key_input.just_pressed(KeyCode::R) {
        // gen_new_dungeon(steps, grid_size, bias);
    }
}

pub fn gen_new_dungeon(
    steps: Res<NumSteps>,
    grid_size: Res<GridSize>,
    bias: Res<Bias>,
) -> Vec<Vec<i8>> {
    let mut grid: Vec<Vec<i8>> = vec![vec![0; grid_size.0 as usize]; grid_size.0 as usize];
    let mut walker = Walker {
        pos: Vec2::new((grid_size.0 / 2) as f32, (grid_size.0 / 2) as f32),
    };
    let num_steps = steps.0;
    let square_size = 10.;
    let offset = (grid_size.0 * 6) as f32;
    for _ in 0..num_steps {
        let new_dir = Direction::get_next_dir(rand::thread_rng(), bias.clone());
        grid[walker.pos.x as usize][walker.pos.y as usize] = 1;
        match new_dir {
            Direction::Down => walker.pos.y -= 1.,
            Direction::Up => walker.pos.y += 1.,
            Direction::Left => walker.pos.x -= 1.,
            Direction::Right => walker.pos.x += 1.,
        }
        if walker.pos.x > (grid_size.0 - 1) as f32 {
            walker.pos.x = (grid_size.0 - 1) as f32
        }
        if walker.pos.y > (grid_size.0 - 1) as f32 {
            walker.pos.y = (grid_size.0 - 1) as f32
        }
    }
    grid

    // for (x, sub_grid) in grid.iter().enumerate() {
    //     for (y, v) in sub_grid.iter().enumerate() {
    //         if v == &1 {
    //             commands.spawn(SpriteBundle {
    //                 sprite: Sprite {
    //                     color: Color::rgb(0.25, 0.25, 0.75),
    //                     custom_size: Some(Vec2::new(square_size, square_size)),
    //                     ..default()
    //                 },
    //                 transform: Transform::from_translation(Vec3::new(
    //                     x as f32 * (square_size + 2.) - offset,
    //                     y as f32 * (square_size + 2.) - offset,
    //                     0.,
    //                 )),
    //                 ..default()
    //             });
    //         } else {
    //             commands.spawn(SpriteBundle {
    //                 sprite: Sprite {
    //                     color: Color::rgb(1., 0.25, 0.75),
    //                     custom_size: Some(Vec2::new(square_size, square_size)),
    //                     ..default()
    //                 },
    //                 transform: Transform::from_translation(Vec3::new(
    //                     x as f32 * (square_size + 2.) - offset,
    //                     y as f32 * (square_size + 2.) - offset,
    //                     0.,
    //                 )),
    //                 ..default()
    //             });
    //         }
    //     }
    // }
}
