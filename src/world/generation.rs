use super::chunk::GenerateObjectsEvent;
use super::dimension::{ActiveDimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::noise_helpers::get_object_points_for_chunk;
use super::wall_auto_tile::{handle_wall_break, handle_wall_placed, update_wall, ChunkWallCache};
use super::world_helpers::tile_pos_to_world_pos;
use super::WorldGeneration;
use crate::item::{handle_break_object, WorldObject};
use crate::proto::proto_param::ProtoParam;
use crate::ui::minimap::UpdateMiniMapEvent;

use crate::world::{noise_helpers, world_helpers, TileMapPosition, CHUNK_SIZE, TILE_SIZE};
use crate::{custom_commands::CommandsExt, CustomFlush, GameParam, GameState};

use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use itertools::Itertools;

#[derive(Debug, Clone)]
pub struct WallBreakEvent {
    pub pos: TileMapPosition,
}
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WallBreakEvent>()
            .add_systems(
                (
                    handle_wall_break
                        .before(CustomFlush)
                        .before(handle_break_object),
                    handle_wall_placed.before(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(Self::generate_and_cache_objects.before(CustomFlush))
            .add_system(update_wall.in_base_set(CoreSet::PostUpdate))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl GenerationPlugin {
    fn get_perlin_block_at_tile(
        world_generation_params: &WorldGeneration,
        pos: TileMapPosition,
        seed: u64,
    ) -> Option<WorldObject> {
        let x = pos.tile_pos.x as f64;
        let y = pos.tile_pos.y as f64;
        // dont need to use expencive noise fn if it will always
        // result in the same tile
        if world_generation_params.stone_wall_frequency == 1. {
            return Some(WorldObject::StoneWall);
        }
        let nx = (x as i32 + pos.chunk_pos.x * CHUNK_SIZE as i32) as f64;
        let ny = (y as i32 + pos.chunk_pos.y * CHUNK_SIZE as i32) as f64;
        let e = noise_helpers::get_perlin_noise_for_tile(nx, ny, seed);
        if e <= world_generation_params.stone_wall_frequency {
            return Some(WorldObject::StoneWall);
        }
        None
    }
    fn generate_stone_for_chunk(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        seed: u64,
    ) -> Vec<(TileMapPosition, WorldObject)> {
        let mut stone_blocks: Vec<(TileMapPosition, WorldObject)> = vec![];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let pos = TileMapPosition::new(chunk_pos, TilePos { x, y });
                if let Some(block) =
                    Self::get_perlin_block_at_tile(world_generation_params, pos, seed)
                {
                    stone_blocks.push((pos, block));
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
        mut game: GameParam,
        mut chunk_spawn_event: EventReader<GenerateObjectsEvent>,
        seed: Query<(&GenerationSeed, Option<&Dungeon>), With<ActiveDimension>>,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
        mut chunk_wall_cache: Query<&mut ChunkWallCache>,
        mut proto_commands: ProtoCommands,
        prototypes: Prototypes,
        mut proto_param: ProtoParam,
    ) {
        for chunk in chunk_spawn_event.iter() {
            let chunk_pos = chunk.chunk_pos;
            let chunk_e = game.get_chunk_entity(chunk_pos).unwrap();
            let (seed, maybe_dungeon) = seed.single();

            // generate stone walls for dungeons
            let stone =
                Self::generate_stone_for_chunk(&game.world_generation_params, chunk_pos, seed.seed);

            // generate all objs
            let mut objs_to_spawn: Box<dyn Iterator<Item = (TileMapPosition, WorldObject)>> =
                Box::new(stone.into_iter());

            for (obj, frequency) in game
                .world_generation_params
                .object_generation_frequencies
                .iter()
            {
                let raw_points = get_object_points_for_chunk(seed.seed, *frequency);
                let points = raw_points
                    .iter()
                    .map(|tp| {
                        let tp_vec = Vec2::new(
                            tp.0 + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                            tp.1 + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                        );

                        let relative_tp = world_helpers::world_pos_to_tile_pos(tp_vec);
                        (relative_tp, *obj)
                    })
                    .collect::<Vec<(TileMapPosition, WorldObject)>>();
                objs_to_spawn = Box::new(objs_to_spawn.chain(points.into_iter()));
            }

            let mut objs_to_spawn = objs_to_spawn.collect::<Vec<(TileMapPosition, WorldObject)>>();

            if let Some(cached_objs) = game.get_objects_from_chunk_cache(chunk_pos) {
                objs_to_spawn = objs_to_spawn
                    .into_iter()
                    .chain(cached_objs.to_owned().into_iter())
                    .collect::<Vec<(TileMapPosition, WorldObject)>>();
            }
            let objs = objs_to_spawn
                .iter()
                .filter(|tp| {
                    // spawn walls in dungeon according to the generated grid layout
                    if let Some(dungeon) = maybe_dungeon {
                        let mut wall_cache = chunk_wall_cache.get_mut(*chunk_e).unwrap();
                        if chunk_pos.x < -1
                            || chunk_pos.x > 2
                            || chunk_pos.y < -2
                            || chunk_pos.y > 1
                        {
                            wall_cache.walls.insert(tp.0, true);
                            return true;
                        }

                        if dungeon.grid[(CHUNK_SIZE as i32 * 2 * (2 - chunk_pos.y)
                            - 1
                            - (2 * tp.0.tile_pos.y as i32))
                            as usize][(2 * CHUNK_SIZE as i32
                            + (chunk_pos.x * 2 * CHUNK_SIZE as i32)
                            + 2 * tp.0.tile_pos.x as i32)
                            as usize]
                            == 1
                        {
                            wall_cache.walls.insert(tp.0, false);
                            return false;
                        }
                    }
                    let tile = if let Some(tile_data) = game.get_tile_data(tp.0) {
                        tile_data.block_type
                    } else {
                        return false;
                    };
                    let filter = game
                        .world_generation_params
                        .obj_allowed_tiles_map
                        .get(&tp.1)
                        .unwrap();
                    for allowed_tile in filter.iter() {
                        if tile.iter().filter(|t| *t == allowed_tile).count() == 4 {
                            return true;
                        }
                    }
                    false
                })
                .map(|tp| *tp)
                .collect::<HashMap<_, _>>();

            // now spawn them, keeping track of duplicates on the same tile
            let mut spawned_vec: HashMap<TileMapPosition, WorldObject> = HashMap::new();
            for obj_data in objs.clone().iter() {
                let (pos, obj) = obj_data;
                let is_medium = obj_data.1.is_medium_size(&proto_param);
                if spawned_vec.contains_key(pos)
                    || (is_medium
                        && (spawned_vec.contains_key(&pos)
                            || spawned_vec
                                .contains_key(&pos.get_neighbour_tiles_for_medium_objects()[0])
                            || spawned_vec
                                .contains_key(&pos.get_neighbour_tiles_for_medium_objects()[1])
                            || spawned_vec
                                .contains_key(&pos.get_neighbour_tiles_for_medium_objects()[2])))
                {
                    continue;
                }
                if is_medium {
                    spawned_vec.insert(*pos, obj.clone());
                    spawned_vec
                        .insert(pos.get_neighbour_tiles_for_medium_objects()[0], obj.clone());
                    spawned_vec
                        .insert(pos.get_neighbour_tiles_for_medium_objects()[1], obj.clone());
                    spawned_vec
                        .insert(pos.get_neighbour_tiles_for_medium_objects()[2], obj.clone());
                } else {
                    spawned_vec.insert(pos.clone(), obj.clone());
                }

                let mut is_touching_air = false;
                if let Some(dungeon) = maybe_dungeon {
                    for x in -1..2 {
                        for y in -1..2 {
                            let original_y = (CHUNK_SIZE as i32 * 2 * (2 - obj_data.0.chunk_pos.y)
                                - 1
                                - (2 * obj_data.0.tile_pos.y as i32))
                                as usize;
                            let original_x = (2 * CHUNK_SIZE as i32
                                + (obj_data.0.chunk_pos.x * 2 * CHUNK_SIZE as i32)
                                + 2 * obj_data.0.tile_pos.x as i32)
                                as usize;
                            if dungeon.grid[(original_y + y as usize).clamp(0, 127)]
                                [(original_x + x as usize).clamp(0, 127)]
                                == 1
                            {
                                is_touching_air = true
                            }
                        }
                    }
                }

                let obj = proto_commands.spawn_object_from_proto(
                    *obj_data.1,
                    tile_pos_to_world_pos(*obj_data.0, obj_data.1.is_medium_size(&proto_param)),
                    &prototypes,
                    &mut proto_param,
                    is_touching_air,
                );

                if let Some(spawned_obj) = obj {
                    if is_medium {
                        minimap_update.send(UpdateMiniMapEvent {
                            pos: Some(*obj_data.0),
                            new_tile: Some(*obj_data.1),
                        });
                        for q in 0..3 {
                            minimap_update.send(UpdateMiniMapEvent {
                                pos: Some(obj_data.0.get_neighbour_tiles_for_medium_objects()[q]),
                                new_tile: Some(*obj_data.1),
                            });
                        }
                    } else {
                        minimap_update.send(UpdateMiniMapEvent {
                            pos: Some(*obj_data.0),
                            new_tile: Some(*obj_data.1),
                        });
                    }

                    commands
                        .entity(spawned_obj)
                        .set_parent(*game.get_chunk_entity(chunk_pos).unwrap());

                    if let Some(_) = maybe_dungeon {
                        let mut wall_cache = chunk_wall_cache.get_mut(*chunk_e).unwrap();
                        wall_cache.walls.insert(*obj_data.0, true);
                    }
                }
            }
            game.set_chunk_objects_cache(chunk_pos, spawned_vec);
        }
    }
}
