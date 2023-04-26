use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::TilemapTexture;

use crate::{
    enemy::Enemy,
    item::{Equipment, WorldObject},
    WorldGeneration,
};

use super::ChunkManager;

#[derive(Component, Debug)]
pub struct Dimension;
impl Dimension {}

#[derive(Component, Debug)]
pub struct GenerationSeed {
    pub seed: u32,
}

#[derive(Component, Debug)]
pub struct SpawnDimension;
pub struct DimensionSpawnEvent {
    pub generation_params: WorldGeneration,
    pub seed: Option<u32>,
    pub swap_to_dim_now: bool,
}
#[derive(Component)]

pub struct ActiveDimension;
pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkManager::new())
            .add_event::<DimensionSpawnEvent>()
            .add_system_to_stage(
                CoreStage::PreUpdate,
                Self::handle_dimension_swap_events.after(Self::new_dim_with_params),
            )
            .add_system_to_stage(CoreStage::PreUpdate, Self::new_dim_with_params);
    }
}
impl DimensionPlugin {
    ///spawns the initial world dimension entity
    pub fn new_dim_with_params(
        mut commands: Commands,
        mut spawn_event: EventReader<DimensionSpawnEvent>,
    ) {
        for new_dim in spawn_event.iter() {
            println!("SPAWNING NEW DIMENSION");
            let mut cm = ChunkManager::new();
            cm.world_generation_params = new_dim.generation_params.clone();
            let dim_e = commands
                .spawn((
                    Dimension,
                    GenerationSeed {
                        seed: new_dim.seed.unwrap_or(0),
                    },
                    cm,
                ))
                .id();
            if new_dim.swap_to_dim_now {
                commands.entity(dim_e).insert(SpawnDimension);
            }
        }
    }
    pub fn handle_dimension_swap_events(
        new_dim: Query<Entity, Added<SpawnDimension>>,
        mut commands: Commands,
        entity_query: Query<
            Entity,
            (
                Or<(With<WorldObject>, With<Enemy>, With<TilemapTexture>)>,
                Without<Equipment>,
            ),
        >,
        old_dim: Query<Entity, With<ActiveDimension>>,
        cm: Query<&ChunkManager>,
        old_cm: Res<ChunkManager>,
    ) {
        // event sent out when we enter a new dimension
        for d in new_dim.iter() {
            //despawn all entities with positions, except the player
            println!("DESPAWNING EVERYTHING!!! {:?}", entity_query.iter().len());
            for e in entity_query.iter() {
                commands.entity(e).despawn_recursive();
            }
            // clean up old dimension, remove active tag, and update its chunk manager
            if let Ok(old_dim) = old_dim.get_single() {
                commands
                    .entity(old_dim)
                    .remove::<ActiveDimension>()
                    .insert(old_cm.clone());
            }
            println!("inserting new chunk manager/dim {:?}", cm.iter().len());
            //give the new dimension active tag, and use its chunk manager as the game resource
            commands
                .entity(d)
                .insert(ActiveDimension)
                .remove::<SpawnDimension>();
            commands.insert_resource(cm.get(d).unwrap().clone());
        }
    }
}

// #[derive(Reflect, Resource, Clone, Debug, Default, InspectorOptions)]
// #[reflect(Resource, InspectorOptions)]
// pub enum Direction {
//     #[default]
//     Left,
//     Right,
//     Up,
//     Down,
// }
// pub struct Walker {
//     pos: Vec2,
// }

// #[derive(Reflect, Resource, Clone, InspectorOptions)]
// pub struct NumSteps(i32);
// impl Default for NumSteps {
//     fn default() -> Self {
//         Self(100)
//     }
// }
// #[derive(Reflect, Resource, Clone, InspectorOptions)]
// #[reflect(Resource, InspectorOptions)]
// pub struct GridSize(usize);
// impl Default for GridSize {
//     fn default() -> Self {
//         Self(32)
//     }
// }

// #[derive(Reflect, Resource, Clone, InspectorOptions)]
// #[reflect(Resource, InspectorOptions)]
// pub struct Bias {
//     bias: Direction,
//     #[inspector(min = 0, max = 50)]
//     strength: u32,
// }
// impl Default for Bias {
//     fn default() -> Self {
//         Self {
//             bias: Direction::Left,
//             strength: 0,
//         }
//     }
// }

// impl Direction {
//     pub fn get_next_dir(mut rng: ThreadRng, bias: Bias) -> Self {
//         // match bias.bias.clone() {
//         //     //0, 1, 2, 3, -> 0 0.05, 2 + 0.10, 3 - 0.05, 0.25 - 5
//         //     Some(v) => {
//         let is_biased = rng.gen_ratio(50 + bias.strength, 100);
//         println!(" {:?} {:?}", 50 + bias.strength, bias.bias);
//         let which_dir = rng.gen_ratio(1, 2);
//         if is_biased {
//             return match which_dir {
//                 true => bias.bias,
//                 false => bias.bias.get_opposite(),
//             };
//         } else {
//             return match which_dir {
//                 true => bias.bias.get_neighbour(),
//                 false => bias.bias.get_opposite().get_neighbour(),
//             };
//         }
//         //     }

//         //     None => {
//         //         let i = rng.gen_range(0..4);
//         //         return match i {
//         //             0 => Direction::Left,
//         //             1 => Direction::Right,
//         //             2 => Direction::Up,
//         //             3 => Direction::Down,
//         //             _ => Direction::Left,
//         //         };
//         //     }
//         // };
//     }
//     fn get_opposite(&self) -> Self {
//         match self {
//             Direction::Left => Direction::Right,
//             Direction::Right => Direction::Left,
//             Direction::Up => Direction::Down,
//             Direction::Down => Direction::Up,
//         }
//     }
//     fn get_neighbour(&self) -> Self {
//         match self {
//             Direction::Up => Direction::Right,
//             Direction::Right => Direction::Down,
//             Direction::Down => Direction::Left,
//             Direction::Left => Direction::Up,
//         }
//     }
// }
// fn key_event(
//     mut commands: Commands,
//     key_input: ResMut<Input<KeyCode>>,
//     old: Query<Entity, With<Sprite>>,
//     steps: Res<NumSteps>,
//     grid_size: Res<GridSize>,
//     bias: Res<Bias>,
// ) {
//     if key_input.just_pressed(KeyCode::R) {
//         gen_new_dungeon(&mut commands, old, steps, grid_size, bias);
//     }
// }

// fn gen_new_dungeon(
//     commands: &mut Commands,
//     old: Query<Entity, With<Sprite>>,
//     steps: Res<NumSteps>,
//     grid_size: Res<GridSize>,
//     bias: Res<Bias>,
// ) {
//     for old in old.iter() {
//         commands.entity(old).despawn();
//     }
//     let mut grid: Vec<Vec<i8>> = vec![vec![0; grid_size.0 as usize]; grid_size.0 as usize];
//     let mut walker = Walker {
//         pos: Vec2::new((grid_size.0 / 2) as f32, (grid_size.0 / 2) as f32),
//     };
//     let num_steps = steps.0;
//     let square_size = 10.;
//     let offset = (grid_size.0 * 6) as f32;
//     for _ in 0..num_steps {
//         let new_dir = Direction::get_next_dir(rand::thread_rng(), bias.clone());
//         println!("NEW DIR {new_dir:?}");
//         grid[walker.pos.x as usize][walker.pos.y as usize] = 1;
//         match new_dir {
//             Direction::Down => walker.pos.y -= 1.,
//             Direction::Up => walker.pos.y += 1.,
//             Direction::Left => walker.pos.x -= 1.,
//             Direction::Right => walker.pos.x += 1.,
//         }
//         if walker.pos.x > (grid_size.0 - 1) as f32 {
//             walker.pos.x = (grid_size.0 - 1) as f32
//         }
//         if walker.pos.y > (grid_size.0 - 1) as f32 {
//             walker.pos.y = (grid_size.0 - 1) as f32
//         }
//     }
//     for (x, sub_grid) in grid.iter().enumerate() {
//         for (y, v) in sub_grid.iter().enumerate() {
//             if v == &1 {
//                 commands.spawn(SpriteBundle {
//                     sprite: Sprite {
//                         color: Color::rgb(0.25, 0.25, 0.75),
//                         custom_size: Some(Vec2::new(square_size, square_size)),
//                         ..default()
//                     },
//                     transform: Transform::from_translation(Vec3::new(
//                         x as f32 * (square_size + 2.) - offset,
//                         y as f32 * (square_size + 2.) - offset,
//                         0.,
//                     )),
//                     ..default()
//                 });
//             } else {
//                 commands.spawn(SpriteBundle {
//                     sprite: Sprite {
//                         color: Color::rgb(1., 0.25, 0.75),
//                         custom_size: Some(Vec2::new(square_size, square_size)),
//                         ..default()
//                     },
//                     transform: Transform::from_translation(Vec3::new(
//                         x as f32 * (square_size + 2.) - offset,
//                         y as f32 * (square_size + 2.) - offset,
//                         0.,
//                     )),
//                     ..default()
//                 });
//             }
//         }
//     }
// }
