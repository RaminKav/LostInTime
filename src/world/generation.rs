use std::fs::File;

use super::chunk::GenerateObjectsEvent;
use super::dimension::{ActiveDimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::noise_helpers::get_object_points_for_chunk;
use super::wall_auto_tile::{handle_wall_break, handle_wall_placed, update_wall, ChunkWallCache};
use super::world_helpers::tile_pos_to_world_pos;
use super::{WorldGeneration, ISLAND_SIZE};
use crate::container::ContainerRegistry;
use crate::enemy::spawn_helpers::is_tile_water;
use crate::enemy::Mob;
use crate::item::{handle_break_object, WorldObject};
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;
use crate::schematic::loot_chests::get_random_loot_chest_type;
use crate::ui::minimap::UpdateMiniMapEvent;

use crate::world::world_helpers::get_neighbour_tile;
use crate::world::{noise_helpers, world_helpers, TileMapPosition, CHUNK_SIZE, TILE_SIZE};
use crate::NO_GEN;
use crate::{custom_commands::CommandsExt, CustomFlush, GameParam, GameState};

use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::prelude::*;
use bevy_proto::prelude::{ProtoCommands, Prototypes};
use bevy_rapier2d::prelude::Collider;
use rand::Rng;

#[derive(Debug, Clone)]
pub struct WallBreakEvent {
    pub pos: TileMapPosition,
}
pub struct DoneGeneratingEvent {
    pub chunk_pos: IVec2,
}

const UNIQUE_OBJECTS_DATA: [(WorldObject, Vec2); 2] = [
    (WorldObject::BossShrine, Vec2::new(8., 8.)),
    (WorldObject::DungeonEntrance, Vec2::new(2., 2.)),
];

#[derive(Resource, Debug, Default)]
pub struct WorldObjectCache {
    pub objects: HashMap<TileMapPosition, WorldObject>,
    pub unique_objs: HashMap<WorldObject, TileMapPosition>,
    pub dungeon_objects: HashMap<TileMapPosition, WorldObject>,
    pub generated_chunks: Vec<IVec2>,
    pub generated_dungeon_chunks: Vec<IVec2>,
}
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WallBreakEvent>()
            .add_event::<DoneGeneratingEvent>()
            .add_systems(
                (
                    handle_wall_break
                        .before(CustomFlush)
                        .before(handle_break_object),
                    handle_wall_placed.before(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                Self::generate_unique_objects_for_new_world.in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                Self::generate_and_cache_objects.before(CustomFlush).run_if(
                    resource_exists::<GenerationSeed>().and_then(in_state(GameState::Main)),
                ),
            )
            .add_system(
                update_wall
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
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
    pub fn generate_unique_objects_for_new_world(mut game: GameParam) {
        // if a save state exists, we assume unique objs have already been generated
        if let Ok(_) = File::open("save_state.json") {
            return;
        }
        let max_obj_spawn_radius = ((ISLAND_SIZE / CHUNK_SIZE as f32) - 1.) as i32;
        for (obj, _size) in UNIQUE_OBJECTS_DATA {
            if !game.world_obj_cache.unique_objs.contains_key(&obj) {
                let mut rng = rand::thread_rng();

                let pos = TileMapPosition::new(
                    IVec2::new(
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                    ),
                    TilePos::new(rng.gen_range(0..15), rng.gen_range(0..15)),
                );
                println!("set up a {obj:?} at {pos:?}");

                game.world_obj_cache.unique_objs.insert(obj, pos);
            }
        }
    }
    pub fn generate_and_cache_objects(
        mut commands: Commands,
        mut game: GameParam,
        mut chunk_spawn_event: EventReader<GenerateObjectsEvent>,
        dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
        seed: Res<GenerationSeed>,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
        mut chunk_wall_cache: Query<&mut ChunkWallCache>,
        mut proto_commands: ProtoCommands,
        prototypes: Prototypes,
        mut proto_param: ProtoParam,
        container_reg: Res<ContainerRegistry>,
        water_colliders: Query<
            (Entity, &Collider, &GlobalTransform),
            (Without<WorldObject>, Without<Mob>, Without<Player>),
        >,
        mut done_event: EventWriter<DoneGeneratingEvent>,
    ) {
        if *NO_GEN {
            return;
        }
        for chunk in chunk_spawn_event.iter() {
            let chunk_pos = chunk.chunk_pos;
            let chunk_e = game.get_chunk_entity(chunk_pos).unwrap().clone();
            let dungeon_check = dungeon_check.get_single();
            let is_chunk_generated = if dungeon_check.is_ok() {
                game.is_dungeon_chunk_generated(chunk_pos)
            } else {
                game.is_chunk_generated(chunk_pos)
            };
            if !is_chunk_generated {
                println!("Generating new objects for {chunk_pos:?}");
                // generate stone walls for dungeons
                let stone = Self::generate_stone_for_chunk(
                    &game.world_generation_params,
                    chunk_pos,
                    seed.seed,
                );

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

                let mut objs_to_spawn =
                    objs_to_spawn.collect::<Vec<(TileMapPosition, WorldObject)>>();
                if dungeon_check.is_err() {
                    let cached_objs = game.get_objects_from_chunk_cache(chunk_pos);
                    objs_to_spawn = objs_to_spawn
                        .into_iter()
                        .chain(cached_objs.to_owned().into_iter())
                        .collect::<Vec<(TileMapPosition, WorldObject)>>();
                } else {
                    let cached_objs = game.get_objects_from_dungeon_cache(chunk_pos);
                    objs_to_spawn = objs_to_spawn
                        .into_iter()
                        .chain(cached_objs.to_owned().into_iter())
                        .collect::<Vec<(TileMapPosition, WorldObject)>>();
                }
                let mut objs = objs_to_spawn
                    .iter()
                    .filter(|tp| {
                        // spawn walls in dungeon according to the generated grid layout
                        if let Ok(dungeon) = dungeon_check {
                            let mut wall_cache = chunk_wall_cache.get_mut(chunk_e).unwrap();
                            if chunk_pos.x < -3
                                || chunk_pos.x > 4
                                || chunk_pos.y < -4
                                || chunk_pos.y > 3
                            {
                                if tp.1.is_wall() {
                                    wall_cache.walls.insert(tp.0, true);
                                    return true;
                                } else {
                                    return false;
                                }
                            }

                            if dungeon.grid[(CHUNK_SIZE as i32 * (4 - chunk_pos.y)
                                - 1
                                - (tp.0.tile_pos.y as i32))
                                as usize][(3 * CHUNK_SIZE as i32
                                + (chunk_pos.x * CHUNK_SIZE as i32)
                                + tp.0.tile_pos.x as i32)
                                as usize]
                                == 1
                            {
                                if tp.1.is_wall() {
                                    wall_cache.walls.insert(tp.0, false);
                                    return false;
                                }
                            } else if !tp.1.is_wall() {
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
                            .expect(&format!("no allowed tiles for obj {:?}", &tp.1));
                        for allowed_tile in filter.iter() {
                            if tile.iter().filter(|t| *t == allowed_tile).count() == 4 {
                                return true;
                            }
                        }
                        false
                    })
                    .map(|tp| *tp)
                    .collect::<HashMap<_, _>>();

                // UNIQUE OBJECTS
                if dungeon_check.is_err() {
                    for (obj, pos) in game.world_obj_cache.unique_objs.clone() {
                        if pos.chunk_pos == chunk_pos {
                            //TODO: this will be funky if size is not even integers
                            let x_halfsize = (UNIQUE_OBJECTS_DATA
                                .iter()
                                .find(|(o, _)| o == &obj)
                                .map(|(_, s)| s)
                                .unwrap()
                                .x
                                / 2.) as i32;
                            let y_halfsize = (UNIQUE_OBJECTS_DATA
                                .iter()
                                .find(|(o, _)| o == &obj)
                                .map(|(_, s)| s)
                                .unwrap()
                                .y
                                / 2.) as i32;
                            println!("SPAWNING UNIQUE {obj:?} at {pos:?} {x_halfsize:?}");

                            let mut pos = pos;
                            let mut found_non_water_location = false;
                            'repeat: while !found_non_water_location {
                                for x in (-x_halfsize)..=x_halfsize {
                                    for y in (-y_halfsize)..=y_halfsize {
                                        let n_pos = tile_pos_to_world_pos(
                                            get_neighbour_tile(pos, (x as i8, y as i8)),
                                            false,
                                        );
                                        if is_tile_water(n_pos, &mut game).is_ok_and(|x| x) {
                                            let mut rng = rand::thread_rng();

                                            pos = TileMapPosition::new(
                                                chunk_pos,
                                                TilePos::new(
                                                    rng.gen_range(0..15),
                                                    rng.gen_range(0..15),
                                                ),
                                            );
                                            println!("relocating {obj:?} to {pos:?}");
                                            continue 'repeat;
                                        }
                                    }
                                }
                                found_non_water_location = true;
                            }
                            game.world_obj_cache.unique_objs.insert(obj, pos);

                            objs.insert(pos, obj);
                        }
                    }
                }

                // now spawn them, keeping track of duplicates on the same tile
                let mut tiles_to_spawn: HashMap<TileMapPosition, WorldObject> = HashMap::new();
                let mut occupied_tiles: HashMap<TileMapPosition, WorldObject> = HashMap::new();
                for obj_data in objs.clone().iter() {
                    let (pos, obj) = obj_data;
                    let is_medium = obj_data.1.is_medium_size(&proto_param);
                    if occupied_tiles.contains_key(pos)
                        || (is_medium
                            && (occupied_tiles.contains_key(&pos)
                                || occupied_tiles.contains_key(
                                    &pos.get_neighbour_tiles_for_medium_objects()[0],
                                )
                                || occupied_tiles.contains_key(
                                    &pos.get_neighbour_tiles_for_medium_objects()[1],
                                )
                                || occupied_tiles.contains_key(
                                    &pos.get_neighbour_tiles_for_medium_objects()[2],
                                )))
                    {
                        // override chests and dungeon exits, skip anything else
                        if obj == &WorldObject::DungeonExit
                            || obj == &WorldObject::Chest
                            || obj == &WorldObject::DungeonEntrance
                        {
                            occupied_tiles.remove(pos);
                            occupied_tiles.insert(*pos, *obj);
                        } else {
                            continue;
                        }
                    }
                    if is_medium {
                        occupied_tiles.insert(*pos, obj.clone());
                        occupied_tiles
                            .insert(pos.get_neighbour_tiles_for_medium_objects()[0], obj.clone());
                        occupied_tiles
                            .insert(pos.get_neighbour_tiles_for_medium_objects()[1], obj.clone());
                        occupied_tiles
                            .insert(pos.get_neighbour_tiles_for_medium_objects()[2], obj.clone());
                    } else {
                        occupied_tiles.insert(pos.clone(), obj.clone());
                    }
                    tiles_to_spawn.insert(*pos, *obj);
                }
                for (pos, obj) in tiles_to_spawn.iter() {
                    let mut is_touching_air = false;
                    if let Ok(dungeon) = dungeon_check {
                        for x in -1_i32..2 {
                            for y in -1_i32..2 {
                                let original_y = ((CHUNK_SIZE) as i32 * (4 - pos.chunk_pos.y)
                                    - 1
                                    - (pos.tile_pos.y as i32))
                                    as usize;
                                let original_x = ((3 * CHUNK_SIZE) as i32
                                    + (pos.chunk_pos.x * CHUNK_SIZE as i32)
                                    + pos.tile_pos.x as i32)
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

                    let obj_e = proto_commands.spawn_object_from_proto(
                        *obj,
                        tile_pos_to_world_pos(*pos, obj.is_medium_size(&proto_param)),
                        &prototypes,
                        &mut proto_param,
                        is_touching_air,
                    );

                    if let Some(spawned_obj) = obj_e {
                        if obj.is_medium_size(&proto_param) {
                            minimap_update.send(UpdateMiniMapEvent {
                                pos: Some(*pos),
                                new_tile: Some(*obj),
                            });
                            for q in 0..3 {
                                minimap_update.send(UpdateMiniMapEvent {
                                    pos: Some(pos.get_neighbour_tiles_for_medium_objects()[q]),
                                    new_tile: Some(*obj),
                                });
                            }
                        } else {
                            minimap_update.send(UpdateMiniMapEvent {
                                pos: Some(*pos),
                                new_tile: Some(*obj),
                            });
                        }

                        if obj == &WorldObject::Chest && container_reg.containers.get(pos).is_none()
                        {
                            println!("no registry at {pos:?}");
                            commands
                                .entity(spawned_obj)
                                .insert(get_random_loot_chest_type(rand::thread_rng()));
                        } else if obj == &WorldObject::Bridge {
                            for (e, _c, t) in water_colliders.iter() {
                                if t.translation()
                                    .truncate()
                                    .distance(tile_pos_to_world_pos(*pos, false))
                                    <= 6.
                                {
                                    commands.entity(e).despawn();
                                }
                            }
                        }
                        commands
                            .entity(spawned_obj)
                            .set_parent(game.get_chunk_entity(chunk_pos).unwrap());

                        if let Ok(_) = dungeon_check {
                            let mut wall_cache = chunk_wall_cache.get_mut(chunk_e).unwrap();
                            if obj.is_wall() {
                                wall_cache.walls.insert(*pos, true);
                            }
                            game.add_object_to_dungeon_cache(*pos, *obj);
                        } else {
                            game.add_object_to_chunk_cache(*pos, *obj);
                        }
                    }
                }
                if dungeon_check.is_err() {
                    game.set_chunk_generated(chunk_pos);
                } else {
                    game.set_dungeon_chunk_generated(chunk_pos);
                }
            } else {
                let objs = if dungeon_check.is_ok() {
                    game.get_objects_from_dungeon_cache(chunk_pos)
                } else {
                    game.get_objects_from_chunk_cache(chunk_pos)
                };
                for (pos, obj) in objs {
                    let spawned_obj = proto_commands.spawn_object_from_proto(
                        obj,
                        tile_pos_to_world_pos(pos, obj.is_medium_size(&proto_param)),
                        &prototypes,
                        &mut proto_param,
                        true,
                    );
                    if let Some(spawned_obj) = spawned_obj {
                        let mut wall_cache = chunk_wall_cache.get_mut(chunk_e).unwrap();
                        if obj.is_wall() {
                            wall_cache.walls.insert(pos, true);
                        } else if obj == WorldObject::Chest
                            && container_reg.containers.get(&pos).is_none()
                        {
                            println!("no registry at {pos:?}");
                            commands
                                .entity(spawned_obj)
                                .insert(get_random_loot_chest_type(rand::thread_rng()));
                        } else if obj == WorldObject::Bridge {
                            for (e, _c, t) in water_colliders.iter() {
                                if t.translation()
                                    .truncate()
                                    .distance(tile_pos_to_world_pos(pos, false))
                                    <= 6.
                                {
                                    commands.entity(e).despawn();
                                }
                            }
                        }
                        minimap_update.send(UpdateMiniMapEvent {
                            pos: Some(pos),
                            new_tile: Some(obj),
                        });

                        commands
                            .entity(spawned_obj)
                            .set_parent(game.get_chunk_entity(chunk_pos).unwrap());
                    }
                }
            }

            done_event.send(DoneGeneratingEvent { chunk_pos });
        }
    }
}
