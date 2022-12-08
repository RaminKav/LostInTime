use crate::assets::Graphics;
use crate::item::WorldObject;
use crate::{Game, GameState, ImageAssets, Player, WORLD_SIZE};
use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy::{math::Vec3Swizzles, utils::HashSet};
use bevy_ecs_tilemap::prelude::*;
use noise::{NoiseFn, OpenSimplex, Perlin, Seedable, Simplex};
use rand::rngs::ThreadRng;
use rand::Rng;
use serde::Deserialize;

pub struct WorldGenerationPlugin;

impl Plugin for WorldGenerationPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkManager::default()).add_system_set(
            SystemSet::on_update(GameState::Main)
                // .with_system(Self::spawn_chunk)
                .with_system(Self::spawn_chunks_around_camera)
                .with_system(Self::despawn_outofrange_chunks), // .with_system(Self::spawn_test_objects.after("graphics")),
        );
        //TODO: add updating code
        // .add_system_set(
        //     SystemSet::on_update(GameState::Main).with_system(Self::update_graphics),
        //     // .with_system(Self::world_object_growth),
        // );
    }
}
#[derive(Default, Debug, Resource)]
struct ChunkManager {
    pub spawned_chunks: HashSet<IVec2>,
}

impl WorldGenerationPlugin {
    fn spawn_chunk(
        commands: &mut Commands,
        sprite_sheet: &Res<ImageAssets>,
        graphics: &Res<Graphics>,
        game: &Res<Game>,
        chunk_pos: IVec2,
    ) {
        let tilemap_size = TilemapSize {
            x: 16 as u32,
            y: 16 as u32,
        };
        let tile_size = TilemapTileSize { x: 16., y: 16. };
        let grid_size = tile_size.into();
        let map_type = TilemapType::default();

        let tilemap_entity = commands.spawn_empty().id();
        let mut tile_storage = TileStorage::empty(tilemap_size);

        let mut value = [[0.; 16]; 16];
        let noise_e = Perlin::new(1);
        let noise_e2 = Perlin::new(2);
        let noise_e3 = Perlin::new(3);
        let noise_m = Simplex::new(4);
        let noise_m2 = Simplex::new(5);
        let noise_m3 = Simplex::new(6);

        for y in 0..16 {
            for x in 0..16 {
                let tile_pos = TilePos {
                    x: x.try_into().unwrap(),
                    y: y.try_into().unwrap(),
                };
                let nx = (x + chunk_pos.x * 16) as f64 / 16. as f64 - 0.5;
                let ny = (y + chunk_pos.y * 16) as f64 / 16. as f64 - 0.5;
                // let e = noise_e.get([nx, ny]) + 0.5;
                let base_oct = 1.;
                let e = (noise_e.get([nx * base_oct, ny * base_oct]) + 0.5)
                    + 0.5 * (noise_e2.get([nx * base_oct * 2., ny * base_oct * 2.]) + 0.5)
                    + 0.25 * (noise_e3.get([nx * base_oct * 3., ny * base_oct * 3.]) + 0.5);
                let m = (noise_m.get([nx * base_oct, ny * base_oct]) + 0.5)
                    + 0.5 * (noise_m2.get([nx * base_oct * 2., ny * base_oct * 2.]) + 0.5)
                    + 0.25 * (noise_m3.get([nx * base_oct * 3., ny * base_oct * 3.]) + 0.5);

                let e = f64::powf(e / (1. + 0.5 + 0.25), 1.);
                let m = f64::powf(m / (1. + 0.5 + 0.25), 1.);
                // print!("{:?}", e);
                let m = f64::powf(m, 1.);
                let block = if e <= game.world_generation_params.water_frequency {
                    WorldObject::Water
                } else if e <= game.world_generation_params.sand_frequency {
                    if m <= 0.35 {
                        WorldObject::RedSand
                    } else {
                        WorldObject::Sand
                    }
                } else if e <= game.world_generation_params.dirt_frequency {
                    if m < 0.6 {
                        WorldObject::Dirt
                    } else {
                        WorldObject::Mud
                    }
                } else if e <= game.world_generation_params.stone_frequency {
                    WorldObject::Stone
                } else {
                    if m < 0.35 {
                        WorldObject::DryGrass
                    } else {
                        WorldObject::Grass
                    }
                };
                let tile_entity = commands
                    .spawn(TileBundle {
                        position: tile_pos,
                        tilemap_id: TilemapId(tilemap_entity),
                        texture_index: TileTextureIndex(
                            (graphics.item_map.get(&block).unwrap().1) as u32,
                        ),
                        ..Default::default()
                    })
                    .id();
                commands.entity(tilemap_entity).add_child(tile_entity);

                tile_storage.set(&tile_pos, tile_entity);
            }
        }
        let transform = Transform::from_translation(Vec3::new(
            chunk_pos.x as f32 * 16. as f32 * 16.,
            chunk_pos.y as f32 * 16. as f32 * 16.,
            0.0,
        ));
        commands.entity(tilemap_entity).insert(TilemapBundle {
            grid_size,
            map_type,
            size: tilemap_size,
            storage: tile_storage,
            texture: TilemapTexture::Single(sprite_sheet.sprite_sheet.clone()),
            tile_size,
            transform,
            ..Default::default()
        });
    }

    fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
        let camera_pos = camera_pos.as_ivec2();
        let chunk_size: IVec2 = IVec2::new(16 as i32, 16 as i32);
        let tile_size: IVec2 = IVec2::new(16 as i32, 16 as i32);
        camera_pos / (chunk_size * tile_size)
    }

    fn spawn_chunks_around_camera(
        mut commands: Commands,
        sprite_sheet: Res<ImageAssets>,
        camera_query: Query<&Transform, With<Camera>>,
        mut chunk_manager: ResMut<ChunkManager>,
        graphics: Res<Graphics>,
        game: Res<Game>,
    ) {
        for transform in camera_query.iter() {
            let camera_chunk_pos = Self::camera_pos_to_chunk_pos(&transform.translation.xy());
            for y in (camera_chunk_pos.y - 2)..(camera_chunk_pos.y + 2) {
                for x in (camera_chunk_pos.x - 2)..(camera_chunk_pos.x + 2) {
                    if !chunk_manager.spawned_chunks.contains(&IVec2::new(x, y)) {
                        println!("spawning chunk at {:?} {:?}", x, y);
                        chunk_manager.spawned_chunks.insert(IVec2::new(x, y));
                        Self::spawn_chunk(
                            &mut commands,
                            &sprite_sheet,
                            &graphics,
                            &game,
                            IVec2::new(x, y),
                        );
                    }
                }
            }
        }
    }

    fn despawn_outofrange_chunks(
        mut commands: Commands,
        camera_query: Query<&Transform, With<Camera>>,
        chunks_query: Query<(Entity, &Transform)>,
        mut chunk_manager: ResMut<ChunkManager>,
    ) {
        for camera_transform in camera_query.iter() {
            for (entity, chunk_transform) in chunks_query.iter() {
                let chunk_pos = chunk_transform.translation.xy();
                let distance = camera_transform.translation.xy().distance(chunk_pos);
                //TODO: calculate maximum possible distance for 2x2 chunksa
                if distance > 1250.0 {
                    let x = (chunk_pos.x as f32 / (16 as f32 * 16.)).floor() as i32;
                    let y = (chunk_pos.y as f32 / (16 as f32 * 16.)).floor() as i32;
                    println!("despawning chunk at {:?} {:?} d === {:?}", x, y, distance);
                    chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
    }

    fn spawn_test_objects(mut commands: Commands, graphics: Res<Graphics>) {
        let mut tree_children = Vec::new();

        let tree_points = poisson_disk_sampling(4., 30, rand::thread_rng());
        for tp in tree_points {
            tree_children.push(WorldObject::Tree.spawn(
                &mut commands,
                &graphics,
                Vec3::new((tp.x as f32) * 16., (tp.y as f32) * 16., 0.1),
            ));
        }
        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            // .push_children(&children)
            .push_children(&tree_children);
    }
}

fn poisson_disk_sampling(r: f64, k: i8, mut rng: ThreadRng) -> Vec<Vec2> {
    // TODO: fix this to work w 4 quadrants -/+
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

        for i in i0..=i1 {
            for j in j0..=j1 {
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
        let i = rng.gen_range(0..=(active.len() - 1));
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
