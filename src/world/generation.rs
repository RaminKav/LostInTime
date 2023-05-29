use super::chunk::GenerateObjectsEvent;
use super::dimension::{ActiveDimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::{ChunkManager, WorldObjectEntityData};
use crate::item::{Foliage, Wall, WorldObject};
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::world::{noise_helpers, world_helpers, TileMapPositionData, CHUNK_SIZE, TILE_SIZE};
use crate::{CustomFlush, GameParam, GameState};
use bevy::app::AppExit;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;

#[derive(Debug, Clone)]
pub struct WallBreakEvent {
    pub chunk_pos: IVec2,
    pub tile_pos: TilePos,
}
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<WallBreakEvent>()
            .add_system(exit_system.in_base_set(CoreSet::PostUpdate))
            .add_systems(
                (
                    Self::handle_new_wall_spawn_update.after(CustomFlush),
                    Self::generate_and_cache_objects.before(CustomFlush),
                    Self::handle_wall_break.after(CustomFlush),
                    Self::handle_update_this_wall
                        .after(CustomFlush)
                        .after(Self::handle_new_wall_spawn_update),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
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
        // dont need to use expencive noise fn if it will always
        // result in the same tile
        if chunk_manager.world_generation_params.stone_frequency == 1. {
            return Some(WorldObject::Wall(Wall::Stone));
        }
        let nx = (x as i32 + chunk_pos.x * CHUNK_SIZE as i32) as f64;
        let ny = (y as i32 + chunk_pos.y * CHUNK_SIZE as i32) as f64;
        let e = noise_helpers::get_perlin_noise_for_tile(nx, ny, seed);
        if e <= chunk_manager.world_generation_params.stone_frequency {
            return Some(WorldObject::Wall(Wall::Stone));
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
    pub fn handle_new_wall_spawn_update(
        mut game: GameParam,
        mut new_wall_query: Query<Entity, Added<Wall>>,
        mut wall_data: Query<(Entity, &mut TextureAtlasSprite, &TileMapPositionData)>,
    ) {
        for new_wall in new_wall_query.iter_mut() {
            let new_wall = wall_data.get(new_wall).unwrap();
            let new_wall_pos = new_wall.2.clone();
            for dy in -1i8..=1 {
                for dx in -1i8..=1 {
                    //skip corner block updates for walls
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let TileMapPositionData {
                        chunk_pos: adjusted_chunk_pos,
                        tile_pos: neighbour_tile_pos,
                    } = get_adjusted_tile(new_wall_pos.clone(), (dx, dy));

                    if game.get_chunk_entity(adjusted_chunk_pos).is_none() {
                        continue;
                    }

                    if let Some(neighbour_wall_data) =
                        game.get_tile_obj_data_mut(TileMapPositionData {
                            chunk_pos: adjusted_chunk_pos,
                            tile_pos: neighbour_tile_pos,
                        })
                    {
                        if !matches!(neighbour_wall_data.object, WorldObject::Wall(_)) {
                            continue;
                        } else if neighbour_wall_data.obj_bit_index != 0b1111 {
                            Self::update_wall(
                                neighbour_tile_pos,
                                adjusted_chunk_pos,
                                &mut game,
                                &mut wall_data,
                            );
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
            }
        }
    }

    pub fn update_wall(
        tile_pos: TilePos,
        chunk_pos: IVec2,
        game: &mut GameParam,
        wall_data: &mut Query<(Entity, &mut TextureAtlasSprite, &TileMapPositionData)>,
    ) {
        let new_wall_pos = TileMapPositionData {
            chunk_pos,
            tile_pos,
        };
        let new_wall_entity = game.get_obj_entity_at_tile(new_wall_pos.clone());
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                //skip corner block updates for walls
                if (dx != 0 && dy != 0) || (dx == 0 && dy == 0) {
                    continue;
                }
                // only use neighbours that have at least one water bitt
                let mut neighbour_is_wall = false;
                if let Some(neighbour_block_entity_data) =
                    get_neighbour_obj_data(new_wall_pos.clone(), (dx, dy), game)
                {
                    if matches!(neighbour_block_entity_data.object, WorldObject::Wall(_)) {
                        neighbour_is_wall = true;
                    }
                }
                let mut new_wall_data = game.get_tile_obj_data_mut(new_wall_pos.clone()).unwrap();

                let updated_bit_index = Self::compute_wall_index(
                    new_wall_data.obj_bit_index,
                    (dx, dy),
                    !neighbour_is_wall,
                );

                new_wall_data.texture_offset = 0;

                let mut new_sprite = wall_data.get_mut(new_wall_entity.unwrap()).unwrap().1;
                new_wall_data.obj_bit_index = updated_bit_index;
                new_sprite.index = (updated_bit_index + new_wall_data.texture_offset) as usize;
            }
        }
        let mut first_corner_is_wall = false;
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                //only bottom corner block updates for walls
                if dx == 0 || dy != -1 {
                    continue;
                }
                // only use neighbours that have at least one water bitt
                let mut corner_neighbour_is_wall = false;
                if let Some(neighbour_block_entity_data) =
                    get_neighbour_obj_data(new_wall_pos.clone(), (dx, dy), game)
                {
                    corner_neighbour_is_wall =
                        matches!(neighbour_block_entity_data.object, WorldObject::Wall(_));
                }
                let mut new_wall_data = game.get_tile_obj_data_mut(new_wall_pos.clone()).unwrap();

                let has_wall_below = (new_wall_data.obj_bit_index & 0b0100) == 0b0100;

                let is_0b1111 = new_wall_data.obj_bit_index == 0b1111;
                let is_0b1101 = new_wall_data.obj_bit_index == 0b1101;
                let is_0b1110 = new_wall_data.obj_bit_index == 0b1110;
                let has_wall_on_side = if dx == -1 {
                    (new_wall_data.obj_bit_index & 0b0001) == 0b0001
                } else {
                    (new_wall_data.obj_bit_index & 0b1000) == 0b1000
                };
                if !(corner_neighbour_is_wall || !has_wall_on_side || !has_wall_below) {
                    let updated_bit_index = if is_0b1111 {
                        if first_corner_is_wall {
                            10
                        } else if dx == -1 {
                            14
                        } else {
                            15
                        }
                    } else if is_0b1101 {
                        if first_corner_is_wall {
                            4
                        } else if dx == -1 {
                            13
                        } else {
                            11
                        }
                    } else if is_0b1110 {
                        if dx == -1 {
                            7
                        } else {
                            6
                        }
                    } else {
                        new_wall_data.obj_bit_index
                    };
                    new_wall_data.texture_offset = 16;
                    let mut new_sprite = wall_data.get_mut(new_wall_entity.unwrap()).unwrap().1;
                    new_sprite.index = (updated_bit_index + new_wall_data.texture_offset) as usize;

                    if dx == -1 {
                        first_corner_is_wall = true;
                    }
                }
            }
        }
    }
    pub fn handle_update_this_wall(
        mut game: GameParam,
        mut new_wall_query: Query<Entity, Added<Wall>>,
        mut wall_data: Query<(Entity, &mut TextureAtlasSprite, &TileMapPositionData)>,
    ) {
        for new_wall in new_wall_query.iter_mut() {
            let new_wall = wall_data.get(new_wall).unwrap().clone();
            if game.get_tile_obj_data(*new_wall.2).unwrap().texture_offset == 0 {
                Self::update_wall(
                    new_wall.2.tile_pos,
                    new_wall.2.chunk_pos,
                    &mut game,
                    &mut wall_data,
                );
            }
        }
    }
    pub fn handle_wall_break(
        mut game: GameParam,
        mut obj_break_events: EventReader<WallBreakEvent>,

        mut wall_data: Query<(Entity, &mut TextureAtlasSprite, &TileMapPositionData)>,
    ) {
        for broken_wall in obj_break_events.iter() {
            let chunk_pos = broken_wall.chunk_pos;

            for dy in -1i8..=1 {
                for dx in -1i8..=1 {
                    //skip corner block updates for walls
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let wall_pos = TileMapPositionData {
                        chunk_pos,
                        tile_pos: broken_wall.tile_pos,
                    };
                    let TileMapPositionData {
                        chunk_pos: adjusted_chunk_pos,
                        tile_pos: neighbour_wall_pos,
                    } = get_adjusted_tile(wall_pos.clone(), (dx, dy));

                    if let Some(neighbour_block_entity_data) =
                        get_neighbour_obj_data(wall_pos, (dx, dy), &mut game)
                    {
                        if matches!(neighbour_block_entity_data.object, WorldObject::Wall(_)) {
                            Self::update_wall(
                                neighbour_wall_pos,
                                adjusted_chunk_pos,
                                &mut game,
                                &mut wall_data,
                            );
                        }
                    }
                }
            }
        }
    }
    pub fn compute_wall_index(neighbour_bits: u8, edge: (i8, i8), remove: bool) -> u8 {
        let mut index = 0;
        // new tile will be 0b1111 i think
        if edge == (0, 1) {
            //above me...
            // Top edge needs b0 b1
            if !remove {
                index = 0b0010;
            }
            index |= neighbour_bits & 0b1101;
        } else if edge == (1, 0) {
            // Right edge
            if !remove {
                index = 0b1000;
            }
            index |= neighbour_bits & 0b0111;
        } else if edge == (0, -1) {
            // Bottom edge
            if !remove {
                index = 0b0100;
            }
            index |= neighbour_bits & 0b1011;
        } else if edge == (-1, 0) {
            // Left edge
            if !remove {
                index = 0b0001;
            }
            index |= neighbour_bits & 0b1110;
        } else if edge == (-1, -1) {
            // Bottom Left edge, remove left bit
            // index |= new_tile_bits & 0b0001;
            index |= neighbour_bits & 0b1110;
        } else if edge == (1, -1) {
            // Bottom Right edge, remove Right bit
            // index |= new_tile_bits & 0b0001;
            index |= neighbour_bits & 0b0111;
        }
        index
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
        seed: Query<&GenerationSeed, With<ActiveDimension>>,
        dungeon: Query<&Dungeon, With<ActiveDimension>>,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
        mut meshes: ResMut<Assets<Mesh>>,
    ) {
        for chunk in chunk_spawn_event.iter() {
            let chunk_pos = chunk.chunk_pos;
            let tree_points;
            let maybe_dungeon = dungeon.get_single();

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
                        .get_tile_data(TileMapPositionData {
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
            let stone_points =
                Self::generate_stone_for_chunk(&game.chunk_manager, chunk_pos, seed.single().seed);
            let objs = stone_points
                .iter()
                .chain(tree_points.iter())
                .filter(|tp| {
                    let tile = game
                        .get_tile_data(TileMapPositionData {
                            chunk_pos,
                            tile_pos: TilePos {
                                x: tp.0 as u32,
                                y: tp.1 as u32,
                            },
                        })
                        .unwrap()
                        .block_type;
                    if tile.contains(&WorldObject::Water) || tile.contains(&WorldObject::Sand) {
                        return false;
                    }
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
                    true
                })
                .map(|tp| *tp)
                .collect::<Vec<(f32, f32, WorldObject)>>();

            println!("SPAWNING OBJECTS FOR {chunk_pos:?} {:?}", objs.len(),);
            for tp in objs.clone().iter() {
                let tile_pos = TilePos {
                    x: tp.0 as u32,
                    y: tp.1 as u32,
                };
                match tp.2 {
                    WorldObject::Foliage(_) => {
                        tp.2.spawn_foliage(
                            &mut commands,
                            &mut game,
                            &mut meshes,
                            tile_pos,
                            chunk_pos,
                        );
                    }
                    WorldObject::Wall(_) => {
                        tp.2.spawn_wall(
                            &mut commands,
                            &mut game,
                            tile_pos,
                            chunk_pos,
                            Some(0b1111),
                        );
                    }
                    _ => {}
                }
            }
            minimap_update.send(UpdateMiniMapEvent);
        }
    }
}

fn exit_system(
    // mut pkv: ResMut<PkvStore>,
    mut events: EventReader<AppExit>,
) {
    if events.iter().count() > 0 {
        info!("SAVING GAME DATA...");

        // for (chunk_pos, data) in game_data.data.iter() {
        //     // pkv.set(&format!("{} {}", chunk_pos.0, chunk_pos.1), data)
        //     //     .expect("failed to store data");
        // }
    }
}

//TODO: move to own helpers file
pub fn get_adjusted_tile(pos: TileMapPositionData, offset: (i8, i8)) -> TileMapPositionData {
    let dx = offset.0;
    let dy = offset.1;
    let x = pos.tile_pos.x as i8;
    let y = pos.tile_pos.y as i8;
    let chunk_pos = pos.chunk_pos;
    let mut neighbour_wall_pos = TilePos {
        x: (dx + x) as u32,
        y: (dy + y) as u32,
    };
    let mut adjusted_chunk_pos = pos.chunk_pos;
    if x + dx < 0 {
        adjusted_chunk_pos.x = chunk_pos.x - 1;
        neighbour_wall_pos.x = CHUNK_SIZE - 1;
    } else if x + dx >= CHUNK_SIZE.try_into().unwrap() {
        adjusted_chunk_pos.x = chunk_pos.x + 1;
        neighbour_wall_pos.x = 0;
    }
    if y + dy < 0 {
        adjusted_chunk_pos.y = chunk_pos.y - 1;
        neighbour_wall_pos.y = CHUNK_SIZE - 1;
    } else if y + dy >= CHUNK_SIZE.try_into().unwrap() {
        adjusted_chunk_pos.y = chunk_pos.y + 1;
        neighbour_wall_pos.y = 0;
    }
    TileMapPositionData {
        chunk_pos: adjusted_chunk_pos,
        tile_pos: neighbour_wall_pos,
    }
}

fn get_neighbour_obj_data(
    pos: TileMapPositionData,
    offset: (i8, i8),
    game: &mut GameParam,
) -> Option<WorldObjectEntityData> {
    let TileMapPositionData {
        chunk_pos: adjusted_chunk_pos,
        tile_pos: neighbour_wall_pos,
    } = get_adjusted_tile(pos, offset);

    if game.get_chunk_entity(adjusted_chunk_pos).is_none() {
        return None;
    }

    if let Some(d) = game.get_tile_obj_data(TileMapPositionData {
        chunk_pos: adjusted_chunk_pos,
        tile_pos: neighbour_wall_pos,
    }) {
        return Some(d.clone());
    }
    None
}
