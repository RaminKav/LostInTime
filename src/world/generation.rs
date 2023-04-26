use super::{ChunkManager, ChunkObjectData};
use crate::item::{Foliage, WorldObject};
use crate::world::{noise_helpers, world_helpers, TileMapPositionData, CHUNK_SIZE, TILE_SIZE};
use crate::GameParam;
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::prelude::*;
use bevy_pkv::PkvStore;

pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(load_game_data)
            .add_system_to_stage(CoreStage::PostUpdate, exit_system);
    }
}

#[derive(Resource)]
pub struct GameData {
    pub data: HashMap<(i32, i32), ChunkObjectData>,
    pub name: String,
}

impl GenerationPlugin {
    fn get_perlin_block_at_tile(
        chunk_manager: &ChunkManager,
        chunk_pos: IVec2,
        tile_pos: TilePos,
        seed: u32,
    ) -> Option<WorldObject> {
        let x = tile_pos.x as f64;
        let y = tile_pos.y as f64;

        let nx = (x as i32 + chunk_pos.x * CHUNK_SIZE as i32) as f64;
        let ny = (y as i32 + chunk_pos.y * CHUNK_SIZE as i32) as f64;
        let e = noise_helpers::get_perlin_noise_for_tile(nx, ny, seed);
        if e <= chunk_manager.world_generation_params.stone_frequency {
            return Some(WorldObject::StoneFull);
        }
        None
    }
    fn generate_stone_for_chunk(
        chunk_manager: &ChunkManager,
        chunk_pos: IVec2,
        seed: u32,
    ) -> Vec<(f32, f32, WorldObject)> {
        let mut stone_blocks: Vec<(f32, f32, WorldObject)> = vec![];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                if let Some(block) =
                    Self::get_perlin_block_at_tile(chunk_manager, chunk_pos, TilePos { x, y }, seed)
                {
                    stone_blocks.push((x as f32, y as f32, block));
                }
            }
        }
        stone_blocks
    }

    // Use chunk manager as source of truth for index

    //TODO: update this to use new constants at top of file
    fn _smooth_terrain(
        k: i8,
        tile_storage: &mut TileStorage,
        tile_index_grid: [[u32; 16]; 16],
        commands: &mut Commands,
    ) {
        // Create a new grid to hold the smoothed terrain
        let mut smooth_grid = [[10000; 16_usize]; 16_usize];

        // Loop over each tile in the grid
        for y in 0..16 {
            for x in 0..16 {
                let current_tile = tile_index_grid[x as usize][y as usize];
                // Count the number of adjacent tiles that are the same type as the current tile
                let mut adjacent_count = 0;
                let mut previous_tile: u32 = 10000;
                let mut smooth_tile: u32 = 10000;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if x + dx >= 0 && x + dx < 16 && y + dy >= 0 && y + dy < 16 {
                            let adj_tile = tile_index_grid[i32::abs(x + dx) as usize]
                                [i32::abs(y + dy) as usize];
                            if adj_tile == current_tile {
                                continue;
                            }
                            if adj_tile == previous_tile {
                                adjacent_count += 1;
                                if adjacent_count >= k {
                                    smooth_tile = adj_tile;
                                }
                            } else {
                                previous_tile = adj_tile;
                            }
                        }
                    }
                }
                // If at least 5 adjacent tiles are the same type, set the smooth_grid value to 1
                // (indicating that this tile should be the same type as the current tile)
                if adjacent_count >= k {
                    smooth_grid[y as usize][x as usize] = smooth_tile;
                }
            }
        }

        // Use the smooth_grid to set the tile types in the tile_storage
        for y in 0..16 {
            for x in 0..16 {
                let tile_pos = TilePos {
                    x: x.try_into().unwrap(),
                    y: y.try_into().unwrap(),
                };
                if smooth_grid[y][x] < 1000 {
                    // tile_storage.get(&tile_pos, smoothed_tile);
                    commands
                        .entity(tile_storage.get(&tile_pos).unwrap())
                        .insert(TileTextureIndex(smooth_grid[y][x]));
                }
            }
        }
    }

    //TODO: do the same shit w graphcis resource loading, but w GameData and pkvStore
    pub fn spawn_objects(commands: &mut Commands, game: &mut GameParam, chunk_pos: IVec2) {
        let mut tree_children = Vec::new();
        let tree_points = game.game_data.data.get(&(chunk_pos.x, chunk_pos.y));
        if let Some(tree_points) = tree_points.to_owned() {
            println!("SPAWNING OBJECTS FOR {chunk_pos:?}");
            for tp in tree_points.0.clone().iter() {
                let tile_pos = IVec2::new(tp.0 as i32, tp.1 as i32);
                let tree;
                match tp.2 {
                    WorldObject::Foliage(_) => {
                        tree = tp.2.spawn_foliage(commands, game, tile_pos, chunk_pos);
                    }
                    _ => {
                        tree = tp.2.spawn(commands, game, tile_pos, chunk_pos);
                    }
                }
                if let Some(tree) = tree {
                    tree_children.push(tree);
                }
            }

            commands
                .spawn(SpatialBundle::default())
                .push_children(&tree_children);
        } else {
            warn!("No Object data found for chunk {:?}", chunk_pos);
        }
    }
    pub fn generate_and_cache_objects(
        game: &mut GameParam,
        pkv: &mut PkvStore,
        chunk_pos: IVec2,
        seed: u32,
    ) {
        let tree_points;

        if
        //false {
        let Ok(data) = pkv.get::<ChunkObjectData>(&format!("{} {}", chunk_pos.x, chunk_pos.y)) {
            tree_points = data.0;
            // info!(
            //     "LOADING OLD CHUNK OBJECT DATA FOR CHUNK {:?} TREES: {:?}",
            //     (chunk_pos.x, chunk_pos.y),
            //     tree_points.len()
            // );
        } else {
            println!("GENERATING AND STORING NEW CHUNK OBJECT DATA");
            let raw_tree_points = noise_helpers::poisson_disk_sampling(
                1.5 * TILE_SIZE.x as f64,
                30,
                rand::thread_rng(),
            );
            tree_points = raw_tree_points
                .iter()
                .map(|tp| {
                    let tp_vec = Vec2::new(
                        tp.0 + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                        tp.1 + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                    );
                    let relative_tp = world_helpers::camera_pos_to_block_pos(&tp_vec);
                    (
                        relative_tp.x as f32,
                        relative_tp.y as f32,
                        WorldObject::Foliage(Foliage::Tree),
                    )
                })
                .filter(|tp| {
                    let tile = game
                        .chunk_manager
                        .chunk_tile_entity_data
                        .get(&TileMapPositionData {
                            chunk_pos,
                            tile_pos: TilePos {
                                x: tp.0 as u32,
                                y: tp.1 as u32,
                            },
                        })
                        .unwrap()
                        .block_type;
                    if tile.contains(&WorldObject::Water)
                        || tile.contains(&WorldObject::Sand)
                        || tile.contains(&WorldObject::DungeonStone)
                    {
                        return false;
                    }
                    true
                })
                .collect::<Vec<(f32, f32, WorldObject)>>();
        }

        let stone_points = Self::generate_stone_for_chunk(&game.chunk_manager, chunk_pos, seed)
            .iter()
            .chain(tree_points.iter())
            .filter(|tp| {
                let tile = game
                    .chunk_manager
                    .chunk_tile_entity_data
                    .get(&TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tp.0 as u32,
                            y: tp.1 as u32,
                        },
                    })
                    .unwrap()
                    .block_type;
                if tile.contains(&WorldObject::Water)
                    || tile.contains(&WorldObject::Sand)
                    || tile.contains(&WorldObject::DungeonStone)
                {
                    return false;
                }
                true
            })
            .map(|tp| *tp)
            .collect::<Vec<(f32, f32, WorldObject)>>();

        game.game_data.data.insert(
            (chunk_pos.x, chunk_pos.y),
            ChunkObjectData(stone_points.to_vec()),
        );
    }
}

fn exit_system(
    mut pkv: ResMut<PkvStore>,
    mut events: EventReader<AppExit>,
    game_data: Res<GameData>,
) {
    if events.iter().count() > 0 {
        info!("SAVING GAME DATA...");

        for (chunk_pos, data) in game_data.data.iter() {
            pkv.set(&format!("{} {}", chunk_pos.0, chunk_pos.1), data)
                .expect("failed to store data");
        }
    }
}
fn load_game_data(mut commands: Commands) {
    //TODO: just instanciates GameData resource for now...
    commands.insert_resource(GameData {
        data: HashMap::new(),
        name: "".to_string(),
    })
}

//TODO: figure out why spawning chunks causes it to lag/glitch
