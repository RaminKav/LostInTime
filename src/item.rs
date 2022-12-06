use std::ops::Mul;

use crate::assets::Graphics;
use crate::{Game, GameState, WORLD_SIZE};
use bevy::prelude::*;
use noise::{NoiseFn, Seedable, Simplex};
use noisy_bevy::simplex_noise_2d;
use rand::rngs::ThreadRng;
use rand::Rng;
use serde::Deserialize;

#[derive(Component)]
pub struct Breakable {
    object: WorldObject,
    pub turnsInto: Option<WorldObject>,
}

/// The core enum of the game, lists everything that can be held or placed in the game
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, Deserialize, Component)]
pub enum WorldObject {
    None,
    Grass,
    Dirt,
    Stone,
    Water,
    Sand,
    Coal,
    Tree,
}

impl WorldObject {
    pub fn spawn(self, commands: &mut Commands, graphics: &Graphics, position: Vec3) -> Entity {
        let sprite = graphics
            .item_map
            .get(&self)
            .expect(&format!("No graphic for object {:?}", self))
            .clone();

        let item = commands
            .spawn(SpriteSheetBundle {
                sprite,
                texture_atlas: graphics.texture_atlas.clone(),
                transform: Transform {
                    translation: position,
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(Name::new("GroundItem"))
            .insert(self)
            .id();

        if let Some(breakable) = self.as_breakable() {
            commands.entity(item).insert(breakable);
        }

        // if let Some(pickup) = self.as_pickup() {
        //     commands.entity(item).insert(pickup);
        // }

        // if self.grows_into().is_some() {
        //     commands.entity(item).insert(GrowthTimer {
        //         timer: Timer::from_seconds(3.0, false),
        //     });
        // }

        item
    }
    pub fn as_breakable(&self) -> Option<Breakable> {
        match self {
            WorldObject::Grass => Some(Breakable {
                object: WorldObject::Grass,
                turnsInto: Some(WorldObject::Dirt),
            }),
            WorldObject::Stone => Some(Breakable {
                object: WorldObject::Stone,
                turnsInto: Some(WorldObject::Coal),
            }),
            _ => None,
        }
    }
}

impl Default for WorldObject {
    fn default() -> Self {
        WorldObject::None
    }
}

pub struct ItemsPlugin;

impl Plugin for ItemsPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_enter(GameState::Main)
                .with_system(Self::spawn_test_objects.after("graphics")),
        )
        .add_system_set(
            SystemSet::on_update(GameState::Main).with_system(Self::update_graphics),
            // .with_system(Self::world_object_growth),
        );
    }
}

impl ItemsPlugin {
    /// Keeps the graphics up to date for things that are harvested or grown
    fn update_graphics(
        mut to_update_query: Query<(&mut TextureAtlasSprite, &WorldObject), Changed<WorldObject>>,
        graphics: Res<Graphics>,
    ) {
        for (mut sprite, world_object) in to_update_query.iter_mut() {
            sprite.clone_from(
                graphics
                    .item_map
                    .get(world_object)
                    .expect(&format!("No graphic for object {:?}", world_object)),
            );
        }
    }

    // Creates our testing map
    #[allow(clippy::vec_init_then_push)]
    fn spawn_test_objects(mut commands: Commands, graphics: Res<Graphics>, game: Res<Game>) {
        let mut children = Vec::new();
        let mut tree_children = Vec::new();
        let mut value = [[0.; WORLD_SIZE]; WORLD_SIZE];
        let noise = Simplex::new(182);

        for y in 0..WORLD_SIZE {
            for x in 0..WORLD_SIZE {
                value[y][x] = noise.get([
                    x as f64 / WORLD_SIZE as f64 - 0.5,
                    y as f64 / WORLD_SIZE as f64 - 0.5,
                ]) + 0.5;
                let block = if value[y][x] <= game.world_generation_params.water_frequency {
                    WorldObject::Water
                } else if value[y][x] <= game.world_generation_params.sand_frequency {
                    WorldObject::Sand
                } else if value[y][x] <= game.world_generation_params.dirt_frequency {
                    WorldObject::Dirt
                } else if value[y][x] <= game.world_generation_params.stone_frequency {
                    WorldObject::Stone
                } else {
                    WorldObject::Grass
                };

                children.push(block.spawn(
                    &mut commands,
                    &graphics,
                    Vec3::new((y as f32) * 0.5, (x as f32) * 0.5, 0.),
                ));
            }
        }
        let tree_points = poisson_disk_sampling(12., 30, rand::thread_rng());
        for tp in tree_points {
            tree_children.push(WorldObject::Tree.spawn(
                &mut commands,
                &graphics,
                Vec3::new((tp.x as f32) * 0.5, (tp.y as f32) * 0.5, 0.),
            ));
        }
        // println!("{:?}", value);
        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            .push_children(&children)
            .push_children(&tree_children);
    }
}

fn poisson_disk_sampling(r: f64, k: i8, mut rng: ThreadRng) -> Vec<Vec2> {
    let n = 2.;
    // the final set of points to return
    let mut points: Vec<Vec2> = vec![];
    // the currently "Active" set of points
    let mut active: Vec<Vec2> = vec![];

    let p0 = Vec2::new(
        rng.gen_range(0..WORLD_SIZE) as f32,
        rng.gen_range(0..WORLD_SIZE) as f32,
    );

    let cell_size = f64::floor(r / f64::sqrt(n));
    let num_cell: usize = (f64::ceil(WORLD_SIZE as f64 / cell_size) + 1.) as usize;
    let mut grid: Vec<Vec<Option<Vec2>>> = vec![vec![None; num_cell]; num_cell];

    let insert_point = |g: &mut Vec<Vec<Option<Vec2>>>, p: Vec2| {
        let xi: usize = f64::floor(p.x as f64 / cell_size) as usize;
        let yi: usize = f64::floor(p.y as f64 / cell_size) as usize;
        g[xi][yi] = Some(p);
    };

    let is_valid_point = move |g: &Vec<Vec<Option<Vec2>>>, p: Vec2| -> bool {
        // make sure p is on screen
        if p.x < 0. || p.x > WORLD_SIZE as f32 || p.y < 0. || p.y > WORLD_SIZE as f32 {
            return false;
        }

        // check neighboring eight cells
        let xi: f64 = f64::floor(p.x as f64 / cell_size);
        let yi: f64 = f64::floor(p.y as f64 / cell_size);
        let i0 = usize::max((xi - 1.) as usize, 0);
        let i1 = usize::min((xi + 1.) as usize, num_cell - 1. as usize);
        let j0 = usize::max((yi - 1.) as usize, 0);
        let j1 = usize::min((yi + 1.) as usize, num_cell - 1. as usize);

        for i in i0..i1 {
            for j in j0..j1 {
                if let Some(sample_point) = g[i][j] {
                    if sample_point.distance(p) < r as f32 {
                        return false;
                    }
                }
            }
        }
        true
    };

    insert_point(&mut grid, p0);
    points.push(p0);
    active.push(p0);
    while active.len() > 0 {
        let i = if active.len() == 1 {
            0
        } else {
            rng.gen_range(0..active.len() - 1)
        };
        let p = active.get(i).unwrap();
        let mut found = false;

        for _ in 0..k {
            // get a random angle
            let theta: f64 = rng.gen_range(0. ..360.);
            let new_r = rng.gen_range(r..(2. * r));

            // create new point from randodm angle r distance away from p
            let new_px = p.x as f64 + new_r * theta.to_radians().cos();
            let new_py = p.y as f64 + new_r * theta.to_radians().sin();
            let new_p = Vec2::new(new_px as f32, new_py as f32);

            if !is_valid_point(&grid, new_p) {
                continue;
            }

            //add the new point to our lists and break
            points.push(new_p);
            insert_point(&mut grid, new_p);
            active.push(new_p);
            found = true;
            break;
        }

        if !found {
            active.remove(i);
        }
    }

    points
}
