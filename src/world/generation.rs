use super::chunk::GenerateObjectsEvent;
use super::dimension::{ActiveDimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::wall_auto_tile::{handle_wall_break, update_wall};
use super::world_helpers::tile_pos_to_world_pos;
use super::WorldGeneration;
use crate::item::WorldObject;
use crate::proto::proto_param::ProtoParam;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::world::{noise_helpers, world_helpers, TileMapPosition, CHUNK_SIZE, TILE_SIZE};
use crate::{custom_commands::CommandsExt, CustomFlush, GameParam, GameState};

use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};

#[derive(Debug, Clone)]
pub struct WallBreakEvent {
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WallBreakEvent>()
            .add_systems(
                (
                    Self::generate_and_cache_objects.before(CustomFlush),
                    handle_wall_break.after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(update_wall.in_base_set(CoreSet::PostUpdate))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl GenerationPlugin {
    fn get_perlin_block_at_tile(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        tile_pos: TilePos,
        seed: u32,
    ) -> Option<WorldObject> {
        let x = tile_pos.x as f64;
        let y = tile_pos.y as f64;
        // dont need to use expencive noise fn if it will always
        // result in the same tile
        if world_generation_params.stone_frequency == 1. {
            return Some(WorldObject::StoneWall);
        }
        let nx = (x as i32 + chunk_pos.x * CHUNK_SIZE as i32) as f64;
        let ny = (y as i32 + chunk_pos.y * CHUNK_SIZE as i32) as f64;
        let e = noise_helpers::get_perlin_noise_for_tile(nx, ny, seed);
        if e <= world_generation_params.stone_frequency {
            return Some(WorldObject::StoneWall);
        }
        None
    }
    fn generate_stone_for_chunk(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        seed: u32,
    ) -> Vec<(f32, f32, WorldObject)> {
        let mut stone_blocks: Vec<(f32, f32, WorldObject)> = vec![];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                if let Some(block) = Self::get_perlin_block_at_tile(
                    world_generation_params,
                    chunk_pos,
                    TilePos { x, y },
                    seed,
                ) {
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

    pub fn generate_and_cache_objects(
        mut commands: Commands,
        game: GameParam,
        mut chunk_spawn_event: EventReader<GenerateObjectsEvent>,
        seed: Query<&GenerationSeed, With<ActiveDimension>>,
        dungeon: Query<&Dungeon, With<ActiveDimension>>,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
        mut proto_commands: ProtoCommands,
        prototypes: Prototypes,
        mut proto_param: ProtoParam,
    ) {
        for chunk in chunk_spawn_event.iter() {
            let chunk_pos = chunk.chunk_pos;
            let maybe_dungeon = dungeon.get_single();

            let raw_tree_points = noise_helpers::poisson_disk_sampling(
                2.5 * TILE_SIZE.x as f64,
                30,
                game.world_generation_params.tree_frequency,
                rand::thread_rng(),
            );
            let raw_boulder_points = noise_helpers::poisson_disk_sampling(
                2.5 * TILE_SIZE.x as f64,
                30,
                game.world_generation_params.boulder_frequency,
                rand::thread_rng(),
            );
            let tree_points = raw_tree_points
                .iter()
                .map(|tp| {
                    let tp_vec = Vec2::new(
                        tp.0 + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                        tp.1 + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                    );
                    let relative_tp = world_helpers::camera_pos_to_tile_pos(&tp_vec);
                    (
                        relative_tp.x as f32,
                        relative_tp.y as f32,
                        WorldObject::Tree,
                    )
                })
                .collect::<Vec<(f32, f32, WorldObject)>>();
            let boulder_points = raw_boulder_points
                .iter()
                .map(|tp| {
                    let tp_vec = Vec2::new(
                        tp.0 + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                        tp.1 + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                    );
                    let relative_tp = world_helpers::camera_pos_to_tile_pos(&tp_vec);
                    (
                        relative_tp.x as f32,
                        relative_tp.y as f32,
                        WorldObject::Boulder,
                    )
                })
                .collect::<Vec<(f32, f32, WorldObject)>>();
            let stone_points = Self::generate_stone_for_chunk(
                &game.world_generation_params,
                chunk_pos,
                seed.single().seed,
            );
            let mut genned_objs = stone_points
                .into_iter()
                .chain(tree_points.into_iter())
                .chain(boulder_points.into_iter())
                .collect::<Vec<(f32, f32, WorldObject)>>();
            if let Some(cached_objs) = game.get_objects_from_chunk_cache(chunk_pos) {
                genned_objs = genned_objs
                    .into_iter()
                    .chain(
                        cached_objs
                            .into_iter()
                            .map(|o| (o.1.x as f32, o.1.y as f32, o.0)),
                    )
                    .collect::<Vec<(f32, f32, WorldObject)>>();
            }
            let objs = genned_objs
                .iter()
                .filter(|tp| {
                    // spawn walls in dungeon according to the generated grid layout
                    if let Ok(dungeon) = maybe_dungeon {
                        if chunk_pos.x < -1
                            || chunk_pos.x > 2
                            || chunk_pos.y < -2
                            || chunk_pos.y > 1
                        {
                            return true;
                        }
                        if dungeon.grid
                            [(CHUNK_SIZE as i32 * (2 - chunk_pos.y) - 1 - tp.1 as i32) as usize]
                            [(CHUNK_SIZE as i32 + (chunk_pos.x * CHUNK_SIZE as i32) + tp.0 as i32)
                                as usize]
                            == 1
                        {
                            return false;
                        }
                    }

                    let tile = game
                        .get_tile_data(TileMapPosition::new(
                            chunk_pos,
                            TilePos {
                                x: tp.0 as u32,
                                y: tp.1 as u32,
                            },
                            0,
                        ))
                        .unwrap()
                        .block_type;
                    let filter = game
                        .world_generation_params
                        .obj_allowed_tiles_map
                        .get(&tp.2)
                        .unwrap();
                    for allowed_tile in filter.iter() {
                        if tile.contains(allowed_tile) {
                            return true;
                        }
                    }
                    false
                })
                .map(|tp| *tp)
                .collect::<Vec<(f32, f32, WorldObject)>>();
            for obj_data in objs.clone().iter() {
                let tile_pos = TilePos {
                    x: obj_data.0 as u32,
                    y: obj_data.1 as u32,
                };
                if let Some(_existing_object) =
                    game.get_obj_entity_at_tile(TileMapPosition::new(chunk_pos, tile_pos, 0))
                {
                    warn!("obj exists here {chunk_pos}, {tile_pos:?}");
                    continue;
                }

                let obj = proto_commands.spawn_object_from_proto(
                    obj_data.2,
                    tile_pos_to_world_pos(
                        TileMapPosition::new(chunk_pos, tile_pos, 0),
                        obj_data.2.should_center_sprite(&proto_param),
                    ),
                    &prototypes,
                    &mut proto_param,
                );
                if let Some(spawned_obj) = obj {
                    minimap_update.send(UpdateMiniMapEvent {
                        pos: Some(TileMapPosition::new(chunk_pos, tile_pos, 0)),
                        new_tile: Some([obj_data.2; 4]),
                    });

                    commands
                        .entity(spawned_obj)
                        .set_parent(*game.get_chunk_entity(chunk_pos).unwrap());
                }
            }
        }
    }
}
