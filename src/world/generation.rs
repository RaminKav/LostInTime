use super::chunk::{ChunkPlugin, GenerateObjectsEvent, TileSpriteData};
use super::dimension::{ActiveDimension, Era, GenerationSeed};
use super::dungeon::Dungeon;
use super::noise_helpers::{_poisson_disk_sampling, get_object_points_for_chunk};
use super::portal::TimePortal;
use super::wall_auto_tile::ChunkWallCache;
use super::world_helpers::tile_pos_to_world_pos;
use super::y_sort::YSort;
use super::{WorldGeneration, ISLAND_SIZE};
use crate::aseprite_assets::Portal;
use crate::aseprite_helpers::aseprite_bundle;
use crate::assets::{Graphics, SpriteAnchor};
use crate::enemy::spawn_helpers::is_tile_water;
use crate::item::{object_actions::ObjectAction, PlaceItemEvent, WorldObject};
use crate::pets::state::{Pet, PetSpawner};
use crate::player::skills::ActiveSkill;
use crate::proto::proto_param::ProtoParam;
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::world::chunk::DoneCreateChunkEvent;
use bevy_aseprite_ultra::prelude::Aseprite;
use itertools::Itertools;

use crate::world::world_helpers::{get_neighbour_tile, world_pos_to_tile_pos};
use crate::world::{noise_helpers, world_helpers, TileMapPosition, CHUNK_SIZE, TILE_SIZE};
use crate::{run_once_per_run, CustomFlush, GameParam, GameState, DEBUG_AI};
use crate::{DEBUG, NO_GEN, TEST_SHRINES};

use bevy::platform::collections::{HashMap, HashSet};
use bevy::{
    math::primitives::Rectangle,
    prelude::*,
    sprite_render::{ColorMaterial, MeshMaterial2d},
};
use bevy_ecs_tilemap::prelude::*;
use bevy_rapier2d::prelude::Collider;

use rand::{
    seq::{IteratorRandom, SliceRandom},
    Rng,
};

#[derive(Debug, Clone, Message)]
pub struct WallBreakEvent {
    pub pos: TileMapPosition,
}
#[derive(Message)]
pub struct DoneGeneratingEvent {
    pub chunk_pos: IVec2,
}

const UNIQUE_OBJECTS_DATA: [(WorldObject, Vec2, i32); 2] = [
    (WorldObject::BossShrine, Vec2::new(8., 8.), 10),
    (WorldObject::DungeonEntrance, Vec2::new(2., 2.), 7),
    // (WorldObject::TimeGate, Vec2::new(2., 2.), 3),
];

/// Tiles cleared of large foliage objects (trees / medium-size props) around every
/// pre-rolled shrine so the shrine isn't buried in the forest.
const SHRINE_CLEAR_RADIUS: i8 = 3;

/// Preferred minimum Chebyshev distance between shrine tiles (a 3-tile gap).
/// Placement falls back to ignoring this if no spaced spot can be found.
const SHRINE_MIN_GAP_TILES: i8 = 3;

/// Shrines stay inside this radius (world pixels from `(0, 0)`) so they avoid the outer ring
/// of island chunks where tiles are often water.
const SHRINE_MAX_DIST_FROM_WORLD_ORIGIN: f32 = 5. * CHUNK_SIZE as f32 * TILE_SIZE.x;

#[derive(Resource, Debug, Default, Clone)]
pub struct WorldObjectCache {
    pub objects: HashMap<TileMapPosition, WorldObject>,
    pub unique_objs: HashMap<WorldObject, TileMapPosition>,
    pub shrines: HashMap<TileMapPosition, WorldObject>,
    pub shrines_rolled: bool,
    pub dungeon_objects: HashMap<TileMapPosition, WorldObject>,
    pub generated_chunks: Vec<IVec2>,
    pub generated_dungeon_chunks: Vec<IVec2>,
    pub tile_data_cache: HashMap<TileMapPosition, TileSpriteData>,
    /// Rolled once per active skill shrine tile (world-unique spawn or first interact). Survives chunk despawn.
    pub active_skill_shrine_offers: HashMap<TileMapPosition, Vec<ActiveSkill>>,
    /// Per-tile shrine repair rolls. Empty vec = healthy; non-empty = broken with rolled material costs.
    /// Survives chunk despawn so broken state and costs stay stable.
    pub broken_shrine_costs: HashMap<TileMapPosition, Vec<(crate::item::WorldObject, u32)>>,
}
pub struct GenerationPlugin;

impl Plugin for GenerationPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<WallBreakEvent>()
            .add_message::<DoneGeneratingEvent>()
            .add_systems(
                Update,
                Self::generate_unique_objects_for_new_world
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_systems(
                Update,
                Self::generate_and_cache_objects
                    .before(ChunkPlugin::mark_outofrange_chunks_for_despawn)
                    .before(CustomFlush)
                    .run_if(
                        resource_exists::<GenerationSeed>.and_then(
                            in_state(GameState::Main).or_else(in_state(GameState::Initializing)),
                        ),
                    ),
            )
            .add_systems(
                Update,
                Self::spawn_test_shrine_grid
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            )
            .add_systems(
                OnEnter(GameState::Main),
                Self::spawn_debug_chunk_borders.run_if(run_once_per_run()),
            )
            .add_systems(Update, ApplyDeferred.in_set(CustomFlush));
    }
}

impl GenerationPlugin {
    fn spawn_pet_spawner(
        commands: &mut Commands,
        asset_server: &AssetServer,
        pet_type: Pet,
        world_pos: Vec2,
    ) {
        let aseprite_path = pet_type.get_aseprite_path().to_owned();
        let idle_anim = pet_type.get_idle_anim();

        commands.spawn((
            PetSpawner {
                pet_type: pet_type.clone(),
            },
            aseprite_bundle(
                asset_server.load(aseprite_path),
                idle_anim,
                Transform::from_translation(world_pos.extend(1.0)),
                Visibility::Inherited,
                false,
            ),
            YSort(0.001),
            Collider::capsule(Vec2::new(0., -6.), Vec2::new(0., -6.), 5.0),
            InteractionGuideTrigger {
                text: Some("Interact".to_string()),
                activation_distance: 32.,
                icon_stack: None,
            },
            Name::new(format!("{:?} Pet Spawner", pet_type)),
        ));
    }

    fn _get_perlin_block_at_tile(
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
    fn generate_forest_for_chunk(
        world_generation_params: &WorldGeneration,
        chunk_pos: IVec2,
        _seed: u64,
    ) -> Vec<(TileMapPosition, WorldObject)> {
        let mut rng = rand::thread_rng();

        //TODO: make these come from proto, use frequencies?
        let TREES = world_generation_params.forest_params.tree_weights.clone();

        // Safety check: ensure we have valid tree weights
        if TREES.is_empty() {
            error!("No tree weights defined for forest generation!");
            return vec![];
        }

        // Log tree weights for debugging
        let total_weight: f32 = TREES.values().sum();
        if total_weight <= 0.0 || !total_weight.is_finite() {
            error!(
                "Invalid tree weights! Total: {}, Weights: {:?}",
                total_weight, TREES
            );
            return vec![];
        }

        let num_clusters = if rng.gen_ratio(1, 2) { 3 } else { 2 };
        let mut trees: Vec<(TileMapPosition, WorldObject)> = vec![];
        for _ in 0..num_clusters {
            let trees_vec = TREES.iter().collect_vec();

            // Choose trees (min of 2 or available count)
            let num_to_pick = usize::min(2, trees_vec.len());

            let picked_trees_result =
                trees_vec.choose_multiple_weighted(&mut rng.clone(), num_to_pick, |item| {
                    let weight = *item.1 as f64;
                    if !weight.is_finite() || weight < 0.0 {
                        error!("Invalid tree weight for {:?}: {}", item.0, weight);
                        return 0.0; // Return 0 for invalid weights
                    }
                    weight
                });

            let mut picked_trees = match picked_trees_result {
                Ok(iter) => iter.map(|x| x.0).collect_vec(),
                Err(e) => {
                    error!("Failed to pick trees: {:?}, TREES: {:?}", e, TREES);
                    // Fallback: just pick the first tree
                    if let Some(first) = trees_vec.first() {
                        vec![first.0]
                    } else {
                        continue;
                    }
                }
            };
            if picked_trees.contains(&&WorldObject::RedTree) {
                picked_trees = vec![&WorldObject::RedTree];
            }
            // for now, every chunk will get 1 forest startt point
            let rand_x = rng.gen_range(0..CHUNK_SIZE) as f32;
            let rand_y = rng.gen_range(0..CHUNK_SIZE) as f32;
            let forest_nucleous = Vec2::new(rand_x, rand_y);
            let noise_points = _poisson_disk_sampling(
                world_generation_params.forest_params.tree_spacing_radius,
                30,
                f32::min(
                    world_generation_params.forest_params.tree_density * 100.,
                    1.,
                ),
                world_generation_params.forest_params.forest_radius * TILE_SIZE.x,
                world_generation_params.forest_params.max_trees_per_forest,
                forest_nucleous,
                rng.clone(),
            );
            for point in noise_points {
                let x = point.0;
                let y = point.1;
                let updated_pos = Vec2::new(
                    x + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                    y + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y),
                );
                let pos = world_pos_to_tile_pos(updated_pos);
                trees.push((pos, **picked_trees.iter().choose(&mut rng.clone()).unwrap()));
            }
        }

        trees
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

    /// Chebyshev distance in tiles between two map positions (chunk-aware).
    fn chebyshev_tile_dist(a: TileMapPosition, b: TileMapPosition) -> i32 {
        let aw = tile_pos_to_world_pos(a, false);
        let bw = tile_pos_to_world_pos(b, false);
        let dx = ((aw.x - bw.x) / TILE_SIZE.x).round().abs() as i32;
        let dy = ((aw.y - bw.y) / TILE_SIZE.y).round().abs() as i32;
        dx.max(dy)
    }

    /// True when `pos` is within the shrine origin distance limit, has no water
    /// on itself or any adjacent tile (Chebyshev radius 1), and optionally keeps
    /// a gap from already-placed shrines.
    ///
    /// Uses [`WorldObjectCache::tile_data_cache`] (noise-baked for the whole island),
    /// not live chunk entities — `pre_roll_shrines` runs before most chunks are loaded,
    /// so `GameParam::get_tile_data` would reject every candidate.
    fn is_valid_shrine_tile(
        game: &GameParam,
        pos: TileMapPosition,
        max_dist_sq: f32,
        placed: &[TileMapPosition],
        enforce_gap: bool,
    ) -> bool {
        let world_pos = tile_pos_to_world_pos(pos, false);
        if !world_pos.length_squared().is_finite() || world_pos.length_squared() > max_dist_sq {
            return false;
        }
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                let tp = get_neighbour_tile(pos, (dx, dy));
                let Some(tile_data) = game.world_obj_cache.tile_data_cache.get(&tp) else {
                    return false;
                };
                if tile_data.block_type.contains(&WorldObject::WaterTile) {
                    return false;
                }
            }
        }
        if enforce_gap
            && placed
                .iter()
                .any(|other| Self::chebyshev_tile_dist(pos, *other) <= SHRINE_MIN_GAP_TILES as i32)
        {
            return false;
        }
        true
    }

    /// Spiral-search outward from `seed` within the same chunk for a valid shrine tile.
    /// Returns `None` if nothing works (never panics).
    fn find_nearest_valid_shrine_tile(
        game: &GameParam,
        seed: TileMapPosition,
        max_dist_sq: f32,
        placed: &[TileMapPosition],
        enforce_gap: bool,
    ) -> Option<TileMapPosition> {
        if Self::is_valid_shrine_tile(game, seed, max_dist_sq, placed, enforce_gap) {
            return Some(seed);
        }
        // Prefer the usual inner placement band, then fall back to the full chunk.
        for &(min_t, max_t) in &[(4u32, 13u32), (0u32, 16u32)] {
            for radius in 1..=CHUNK_SIZE as i8 {
                for dy in -radius..=radius {
                    for dx in -radius..=radius {
                        if dx.abs() != radius && dy.abs() != radius {
                            continue;
                        }
                        let candidate = get_neighbour_tile(seed, (dx, dy));
                        if candidate.chunk_pos != seed.chunk_pos {
                            continue;
                        }
                        let tx = candidate.tile_pos.x;
                        let ty = candidate.tile_pos.y;
                        if tx < min_t || tx >= max_t || ty < min_t || ty >= max_t {
                            continue;
                        }
                        if Self::is_valid_shrine_tile(
                            game,
                            candidate,
                            max_dist_sq,
                            placed,
                            enforce_gap,
                        ) {
                            return Some(candidate);
                        }
                    }
                }
            }
        }
        None
    }

    /// Try to pick a tile for one shrine across the chunk pool.
    fn pick_shrine_tile(
        game: &GameParam,
        chunk_pool: &[IVec2],
        max_dist_sq: f32,
        placed: &[TileMapPosition],
        enforce_gap: bool,
        rng: &mut impl Rng,
    ) -> Option<(usize, TileMapPosition)> {
        const TILE_RETRIES: u32 = 8;
        let max_chunk_pick_attempts = (chunk_pool.len().saturating_mul(32)).max(64);

        for _ in 0..max_chunk_pick_attempts {
            if chunk_pool.is_empty() {
                break;
            }
            let chunk_idx = rng.gen_range(0..chunk_pool.len());
            let chunk_pos = chunk_pool[chunk_idx];

            let mut chosen = None;
            for _ in 0..TILE_RETRIES {
                let tx = rng.gen_range(4..13);
                let ty = rng.gen_range(4..13);
                let candidate = TileMapPosition::new(chunk_pos, TilePos::new(tx, ty));
                if !Self::is_valid_shrine_tile(game, candidate, max_dist_sq, placed, enforce_gap) {
                    continue;
                }
                chosen = Some((chunk_idx, candidate));
                break;
            }

            if chosen.is_none() {
                let seed = TileMapPosition::new(
                    chunk_pos,
                    TilePos::new(rng.gen_range(4..13), rng.gen_range(4..13)),
                );
                if let Some(pos) = Self::find_nearest_valid_shrine_tile(
                    game,
                    seed,
                    max_dist_sq,
                    placed,
                    enforce_gap,
                ) {
                    chosen = Some((chunk_idx, pos));
                }
            }

            if chosen.is_some() {
                return chosen;
            }
        }
        None
    }

    fn pre_roll_shrines(game: &mut GameParam) {
        let mut rng = rand::thread_rng();

        // Build a flat list of every shrine instance we want in the world by
        // rolling each shrine type's count independently.
        let mut shrines_to_place: Vec<WorldObject> = Vec::new();
        let mut log_lines: Vec<String> = Vec::new();
        for (shrine_obj, count_range) in game.world_generation_params.shrine_counts.clone().iter() {
            let min = count_range.min;
            let max = count_range.max.max(min);
            let rolled = if min == max {
                min
            } else {
                rng.gen_range(min..=max)
            };
            log_lines.push(format!(
                "  {:?} -> {} (range {}..={})",
                shrine_obj, rolled, min, max
            ));
            for _ in 0..rolled {
                shrines_to_place.push(*shrine_obj);
            }
        }

        let max_chunk = ((ISLAND_SIZE / CHUNK_SIZE as f32) as i32) - 1;
        let center_excluded = [
            IVec2::ZERO,
            IVec2::new(-1, 0),
            IVec2::new(0, -1),
            IVec2::new(-1, -1),
        ];
        let mut chunk_pool: Vec<IVec2> = Vec::new();
        for cx in -max_chunk..=max_chunk {
            for cy in -max_chunk..=max_chunk {
                let c = IVec2::new(cx, cy);
                if center_excluded.contains(&c) {
                    continue;
                }
                chunk_pool.push(c);
            }
        }

        // Shuffle so the placement order isn't biased by iteration order of the
        // shrine map or the chunk grid scan.
        shrines_to_place.shuffle(&mut rng);
        chunk_pool.shuffle(&mut rng);

        let max_dist_sq = SHRINE_MAX_DIST_FROM_WORLD_ORIGIN * SHRINE_MAX_DIST_FROM_WORLD_ORIGIN;

        // Seed with unique landmarks already placed (BossShrine, DungeonEntrance, …)
        // and any shrines already in the cache so the 3-tile gap covers them too.
        let mut placed_positions: Vec<TileMapPosition> =
            game.world_obj_cache.shrines.keys().copied().collect();
        for (obj, _, _) in UNIQUE_OBJECTS_DATA {
            if let Some(pos) = game.world_obj_cache.unique_objs.get(&obj) {
                placed_positions.push(*pos);
            }
        }

        let mut placed = 0_usize;
        for shrine_obj in shrines_to_place.iter() {
            // Prefer a 3-tile gap from existing shrines; fall back without the gap
            // rather than skipping the shrine entirely.
            let chosen = Self::pick_shrine_tile(
                game,
                &chunk_pool,
                max_dist_sq,
                &placed_positions,
                true,
                &mut rng,
            )
            .or_else(|| {
                Self::pick_shrine_tile(
                    game,
                    &chunk_pool,
                    max_dist_sq,
                    &placed_positions,
                    false,
                    &mut rng,
                )
            });

            let Some((chunk_idx, pos)) = chosen else {
                warn!(
                    "Could not place shrine {:?} within {:.0}px of world origin with a 1-tile water buffer; skipping",
                    shrine_obj, SHRINE_MAX_DIST_FROM_WORLD_ORIGIN,
                );
                continue;
            };

            chunk_pool.swap_remove(chunk_idx);
            game.world_obj_cache.shrines.insert(pos, *shrine_obj);
            placed_positions.push(pos);
            crate::item::shrine_repair::maybe_mark_shrine_broken(
                &mut game.world_obj_cache,
                pos,
                *shrine_obj,
                &game.era.current_era,
                &mut rng,
            );

            // Pre-roll and cache the skill offer for active skill shrines so the
            // selection is stable from world-gen. The offer is still re-validated
            // and topped up against the player's current skills at interaction time.
            if *shrine_obj == WorldObject::ActiveSkillShrine {
                if let Ok((_, player_skills, _)) = game.player_query.single() {
                    let skills =
                        crate::item::active_skill_shrine::roll_active_skill_shrine_offer_skills(
                            Some(player_skills),
                        );
                    if !skills.is_empty() {
                        game.world_obj_cache
                            .active_skill_shrine_offers
                            .insert(pos, skills);
                    }
                }
            }

            placed += 1;
        }

        game.world_obj_cache.shrines_rolled = true;
        info!("=== Pre-rolled Shrine Placements ===");
        for line in log_lines {
            info!("{}", line);
        }
        info!("  Placed {} shrines across the world", placed);
        info!("====================================");
    }

    //TODO: do the same shit w graphcis resource loading, but w GameData and pkvStore
    pub fn generate_unique_objects_for_new_world(
        mut game: GameParam,
        done_chunks_event: MessageReader<DoneCreateChunkEvent>,
        mut commands: Commands,
        dungeon_check: Query<&Dungeon>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        graphics: Res<Graphics>,
        achievements: Option<Res<crate::player::achievements::Achievements>>,
        asset_server: Res<AssetServer>,
    ) {
        if done_chunks_event.len() == 0 {
            return;
        }
        // Debug shrine grid owns placement — skip unique objs / portal / shrine pre-roll.
        if *TEST_SHRINES {
            return;
        }
        let max_obj_spawn_radius = ((ISLAND_SIZE / CHUNK_SIZE as f32) - 3.) as i32;

        // Spawn pet spawners if conditions are met
        // Spawn Slime Pet in Era1
        if game.era.current_era == Era::Main {
            let achievement_ok = achievements
                .as_ref()
                .map(|a| !a.has(crate::player::achievements::Achievement::SlimePet))
                .unwrap_or(true);
            if achievement_ok {
                let mut rng = rand::thread_rng();
                let pos = TileMapPosition::new(
                    IVec2::new(
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                    ),
                    TilePos::new(rng.gen_range(0..15), rng.gen_range(0..15)),
                );

                let world_pos = tile_pos_to_world_pos(pos, false);
                info!("spawning pet slime at {world_pos:?}");
                Self::spawn_pet_spawner(&mut commands, &asset_server, Pet::Slime, world_pos);
            }
        }

        // Spawn Fairy Pet in Era2
        if game.era.current_era == Era::Second {
            let achievement_ok = achievements
                .as_ref()
                .map(|a| !a.has(crate::player::achievements::Achievement::FairyPet))
                .unwrap_or(true);
            if achievement_ok {
                let mut rng = rand::thread_rng();
                let pos = TileMapPosition::new(
                    IVec2::new(
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                        rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                    ),
                    TilePos::new(rng.gen_range(0..15), rng.gen_range(0..15)),
                );
                let world_pos = tile_pos_to_world_pos(pos, false);
                info!("spawning pet fairy at {world_pos:?}");

                Self::spawn_pet_spawner(&mut commands, &asset_server, Pet::Fairy, world_pos);
            }
        }

        for (obj_to_spawn, size, _) in UNIQUE_OBJECTS_DATA {
            if !game.world_obj_cache.unique_objs.contains_key(&obj_to_spawn) {
                let mut rng = rand::thread_rng();
                let gen_new_pos = |rng: &mut rand::rngs::ThreadRng| {
                    let mut pos = TileMapPosition::new(
                        IVec2::new(
                            rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                            rng.gen_range(-max_obj_spawn_radius..max_obj_spawn_radius),
                        ),
                        TilePos::new(rng.gen_range(0..15), rng.gen_range(0..15)),
                    );
                    if pos.chunk_pos == IVec2::ZERO {
                        pos.chunk_pos = IVec2::new(2, 2);
                    }
                    if pos.chunk_pos.x == 1 {
                        pos.chunk_pos.x = 2;
                    }
                    if pos.chunk_pos.y == 1 {
                        pos.chunk_pos.y = 2;
                    }
                    if pos.chunk_pos.x == -1 {
                        pos.chunk_pos.x = -2;
                    }
                    if pos.chunk_pos.y == -1 {
                        pos.chunk_pos.y = -2;
                    }
                    pos
                };
                let mut pos = gen_new_pos(&mut rng);
                info!("NEW UNIQUE OBJ: {obj_to_spawn:?} {pos:?}");

                //TODO: this will be funky if size is not even integers
                let x_halfsize = (size.x / 2.) as i32;
                let y_halfsize = (size.y / 2.) as i32;

                let mut found_non_water_location = false;
                let mut max_chunk_retries = 16;
                'repeat: while !found_non_water_location {
                    for x in (-x_halfsize)..=x_halfsize {
                        for y in (-y_halfsize)..=y_halfsize {
                            let n_pos = tile_pos_to_world_pos(
                                get_neighbour_tile(pos, (x as i8, y as i8)),
                                false,
                            );
                            if is_tile_water(n_pos, &game).is_ok_and(|x| x) {
                                let mut rng = rand::thread_rng();

                                pos = TileMapPosition::new(
                                    pos.chunk_pos,
                                    TilePos::new(rng.gen_range(0..15), rng.gen_range(0..15)),
                                );

                                max_chunk_retries -= 1;
                                if max_chunk_retries == 0 {
                                    max_chunk_retries = 16;
                                    pos = gen_new_pos(&mut rng);
                                }
                                continue 'repeat;
                            }
                        }
                    }
                    found_non_water_location = true;
                }
                debug!("set up a {obj_to_spawn:?} at {pos:?}");
                game.world_obj_cache.unique_objs.insert(obj_to_spawn, pos);
            }
        }

        if !game.world_obj_cache.shrines_rolled {
            Self::pre_roll_shrines(&mut game);
        }

        if dungeon_check.single().is_err() && !*NO_GEN {
            info!("SPAWN PORTAL");
            // summon portal
            commands
                .spawn(Visibility::default())
                .insert(YSort(0.))
                .insert(TimePortal)
                .insert(WorldObject::TimePortal)
                .insert(SpriteAnchor(Vec2::new(0., 10.)))
                .insert(InteractionGuideTrigger {
                    text: Some("???".to_string()),
                    activation_distance: 32.,
                    icon_stack: None,
                })
                .insert(ObjectAction::TimePortal)
                .insert(Collider::capsule(
                    Vec2::new(0., 10.),
                    Vec2::new(0., -18.),
                    11.,
                ))
                .insert(aseprite_bundle(
                    graphics.portal_ase.as_ref().unwrap().clone(),
                    Portal::tags::IDLE,
                    Transform::from_translation(Vec3::new(0., 50., 0.)),
                    Visibility::Inherited,
                    false,
                ))
                .insert(Name::new("Time Portal"));
            for y in 3..9 {
                for x in 0..2 {
                    if *DEBUG_AI {
                        for pos in vec![
                            (8. * x as f32 + 4., y as f32 * 8. + 4.),
                            (8. * x as f32 + 4., y as f32 * 8. - 4.),
                            (8. * x as f32 + -4., y as f32 * 8. + 4.),
                            (8. * x as f32 + -4., y as f32 * 8. + -4.),
                        ] {
                            let pos = Vec2::new(pos.0, pos.1);
                            commands
                                .spawn((
                                    Mesh2d(meshes.add(Mesh::from(Rectangle::new(7.0, 7.0)))),
                                    MeshMaterial2d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                                    Transform::from_translation(Vec3::new(pos.x, pos.y, 0.)),
                                ))
                                .insert(YSort(-0.1))
                                .insert(Name::new("debug chunk border x"));
                        }
                    }
                }
            }
        }
    }

    pub fn generate_and_cache_objects(
        mut commands: Commands,
        mut game: GameParam,
        mut chunk_spawn_event: MessageReader<GenerateObjectsEvent>,
        dungeon_check: Query<&Dungeon, With<ActiveDimension>>,
        seed: Res<GenerationSeed>,
        _chunk_wall_cache: Query<&mut ChunkWallCache>,
        proto_param: ProtoParam,
        mut done_event: MessageWriter<DoneGeneratingEvent>,
        mut place_item_event: MessageWriter<PlaceItemEvent>,
    ) {
        if *NO_GEN || *TEST_SHRINES {
            return;
        }
        let mut total_coal = 0;
        let mut total_metal = 0;
        // Get dungeon check result once for all chunks (it's the same query result)
        let in_dungeon = dungeon_check.single().is_ok();
        for chunk in chunk_spawn_event.read() {
            let chunk_pos = chunk.chunk_pos;
            let is_chunk_generated = game.is_chunk_generated(chunk_pos);
            if !is_chunk_generated {
                debug!(
                    "Generating new objects for {chunk_pos:?} {:?}",
                    game.get_chunk_entity(chunk_pos).is_some()
                );

                // generate forest trees for chunk
                let mut trees = Self::generate_forest_for_chunk(
                    &game.world_generation_params,
                    chunk_pos,
                    seed.seed,
                );

                // random size forest clearings
                if chunk_pos.x.abs() > 1 || chunk_pos.y.abs() > 1 {
                    let mut rng = rand::thread_rng();
                    let rng_x = rng.gen_range(0..CHUNK_SIZE);
                    let rng_y = rng.gen_range(0..CHUNK_SIZE);
                    let clear_tiles = get_radial_tile_positions(
                        TileMapPosition::new(chunk_pos, TilePos::new(rng_x, rng_y)),
                        rng.gen_range(4..8),
                    );
                    trees = trees
                        .into_iter()
                        .filter(|tp| !clear_tiles.contains(&tp.0))
                        .collect_vec();
                }

                // generate all objs
                let mut objs_to_spawn: Box<dyn Iterator<Item = (TileMapPosition, WorldObject)>> =
                    Box::new(trees.clone().into_iter());
                let mut occupied_tiles: HashMap<TileMapPosition, WorldObject> =
                    trees.into_iter().collect();

                for (obj_to_spawn, frequency) in game
                    .world_generation_params
                    .object_generation_frequencies
                    .iter()
                {
                    let mut validated_objs: Vec<(TileMapPosition, WorldObject)> = vec![];
                    let raw_points = get_object_points_for_chunk(seed.seed, *frequency);
                    let points = raw_points
                        .iter()
                        .map(|tp| {
                            let tp_vec = Vec2::new(
                                tp.0 + (chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                                tp.1 + (chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x),
                            );

                            let relative_tp = world_helpers::world_pos_to_tile_pos(tp_vec);
                            (relative_tp, *obj_to_spawn)
                        })
                        .collect::<Vec<(TileMapPosition, WorldObject)>>();
                    for (pos, obj) in points.iter() {
                        // check if tile(s) already occupied by another object waiting to spawn
                        let is_medium = obj.is_medium_size(&proto_param);
                        let tiles_obj_wants_to_take_up = if is_medium {
                            pos.get_neighbour_tiles_for_medium_objects()
                                .into_iter()
                                .chain(vec![*pos])
                                .collect_vec()
                        } else {
                            vec![*pos]
                        };
                        if tiles_obj_wants_to_take_up
                            .iter()
                            .any(|p| occupied_tiles.contains_key(p))
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

                        // mark tiles as occupied for future objects
                        tiles_obj_wants_to_take_up.iter().for_each(|p| {
                            occupied_tiles.insert(*p, *obj);
                        });
                        validated_objs.push((*pos, *obj));
                    }
                    objs_to_spawn = Box::new(objs_to_spawn.chain(validated_objs.into_iter()));
                }
                let mut objs_to_spawn =
                    objs_to_spawn.collect::<Vec<(TileMapPosition, WorldObject)>>();
                let cached_objs = game.get_objects_from_chunk_cache(chunk_pos);
                objs_to_spawn = objs_to_spawn
                    .into_iter()
                    .chain(cached_objs.to_owned().into_iter())
                    .collect::<Vec<(TileMapPosition, WorldObject)>>();

                let mut objs = objs_to_spawn
                    .iter()
                    .filter(|tp| {
                        if tp.1 == WorldObject::None {
                            return false;
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
                            .unwrap_or(&vec![WorldObject::GrassTile])
                            .clone();
                        for allowed_tile in filter.iter() {
                            if tile.iter().filter(|t| *t == allowed_tile).count() == 4 {
                                return true;
                            }
                        }
                        false
                    })
                    .copied()
                    .collect::<HashMap<_, _>>();

                if !in_dungeon {
                    // clear out spawn area
                    let clear_tiles = get_radial_tile_positions(
                        TileMapPosition::new(IVec2::ZERO, TilePos::new(0, 0)),
                        10,
                    );
                    for pos in clear_tiles {
                        if let Some(obj_to_clear) = objs.get(&pos) {
                            if obj_to_clear.is_tree() {
                                objs.remove(&pos);
                            }
                        }
                    }
                    // clear out portal area
                    let clear_tiles = get_radial_tile_positions(
                        TileMapPosition::new(IVec2::ZERO, TilePos::new(0, 2)),
                        2,
                    );
                    for pos in clear_tiles {
                        if let Some(obj) = objs.get(&pos) {
                            if obj.is_tree()
                                || obj.is_medium_size(&proto_param)
                                || obj == &WorldObject::Grass
                                || obj == &WorldObject::Grass2
                                || obj == &WorldObject::Grass3
                            {
                                objs.remove(&pos);
                            }
                        }
                    }

                    // UNIQUE OBJECTS
                    for (unique_obj, pos) in game.world_obj_cache.unique_objs.clone() {
                        if pos.chunk_pos == chunk_pos {
                            objs.insert(pos, unique_obj);
                        };
                        // clear out area
                        let clear_tiles = get_radial_tile_positions(
                            pos,
                            *UNIQUE_OBJECTS_DATA
                                .iter()
                                .find(|(o, _, _)| o == &unique_obj)
                                .map(|(_, _, r)| r)
                                .unwrap() as i8,
                        );
                        for pos_to_clear in clear_tiles {
                            if let Some(obj_to_clear) = objs.get(&pos_to_clear) {
                                if (obj_to_clear.is_tree()
                                    || obj_to_clear.is_medium_size(&proto_param))
                                    && !obj_to_clear.is_unique_object()
                                {
                                    objs.remove(&pos_to_clear);
                                    if let Some((entity_to_despawn, obj_to_despawn)) =
                                        game.get_obj_entity_at_tile(pos_to_clear, &proto_param)
                                    {
                                        if (obj_to_despawn.is_tree()
                                            || obj_to_despawn.is_medium_size(&proto_param))
                                            && !obj_to_despawn.is_unique_object()
                                        {
                                            commands.entity(entity_to_despawn).despawn();
                                        }
                                    }
                                    game.remove_object_from_chunk_cache(pos_to_clear);
                                }
                            }
                        }
                    }

                    for (pos, shrine_obj) in game.world_obj_cache.shrines.clone() {
                        if pos.chunk_pos == chunk_pos {
                            objs.insert(pos, shrine_obj);
                        }
                        // Clear a radius of large foliage objects (trees / medium props)
                        // around every shrine so it doesn't spawn buried in the forest.
                        let clear_tiles = get_radial_tile_positions(pos, SHRINE_CLEAR_RADIUS);
                        for pos_to_clear in clear_tiles {
                            if let Some(obj_to_clear) = objs.get(&pos_to_clear) {
                                if (obj_to_clear.is_tree()
                                    || obj_to_clear.is_medium_size(&proto_param))
                                    && !obj_to_clear.is_unique_object()
                                {
                                    objs.remove(&pos_to_clear);
                                    if let Some((entity_to_despawn, obj_to_despawn)) =
                                        game.get_obj_entity_at_tile(pos_to_clear, &proto_param)
                                    {
                                        if (obj_to_despawn.is_tree()
                                            || obj_to_despawn.is_medium_size(&proto_param))
                                            && !obj_to_despawn.is_unique_object()
                                        {
                                            commands.entity(entity_to_despawn).despawn();
                                        }
                                    }
                                    game.remove_object_from_chunk_cache(pos_to_clear);
                                }
                            }
                        }
                    }
                }
                if !in_dungeon && game.era.current_era == Era::Third {
                    extend_ice_patches_from_seeds(&mut objs, &game);
                }
                // For non-dungeon chunks, we don't need distance checks since chunks are generated dynamically
                // and we already check if chunk_entity exists and chunk is generated
                // Distance check is only needed for dungeon chunks

                for (pos, mut obj_to_spawn) in objs.iter() {
                    // only spawn if generated obj is in our chunk or a previously genereated chunk,
                    // otherwise cache it for the correct chunk to spawn

                    if total_metal > total_coal + 5 && obj_to_spawn == &WorldObject::MetalBoulder {
                        obj_to_spawn = &WorldObject::CoalBoulder;
                    }
                    if total_coal > total_metal + 5 && obj_to_spawn == &WorldObject::CoalBoulder {
                        obj_to_spawn = &WorldObject::MetalBoulder;
                    }
                    if obj_to_spawn == &WorldObject::CoalBoulder {
                        total_coal += 1;
                    }
                    if obj_to_spawn == &WorldObject::MetalBoulder {
                        total_metal += 1;
                    }
                    // Fixed: Remove distance restriction for non-dungeon chunks - objects should spawn
                    // wherever chunks are generated. Distance check only applies to dungeons.
                    // The chunk existence and generation checks are sufficient for normal world gen.
                    if (in_dungeon || game.get_chunk_entity(chunk_pos).is_some())
                        && (pos.chunk_pos == chunk_pos || game.is_chunk_generated(pos.chunk_pos))
                    {
                        place_item_event.write(PlaceItemEvent {
                            obj: *obj_to_spawn,
                            pos: tile_pos_to_world_pos(
                                *pos,
                                obj_to_spawn.is_medium_size(&proto_param),
                            ),
                            placed_by_player: false,
                            override_existing_obj: false,
                        });
                    } else {
                        game.add_object_to_chunk_cache(*pos, *obj_to_spawn);
                    };
                }

                game.set_chunk_generated(chunk_pos);
            } else {
                let objs = game.get_objects_from_chunk_cache(chunk_pos);
                for (pos, obj_to_spawn) in objs {
                    place_item_event.write(PlaceItemEvent {
                        obj: obj_to_spawn,
                        pos: tile_pos_to_world_pos(pos, obj_to_spawn.is_medium_size(&proto_param)),
                        placed_by_player: false,
                        override_existing_obj: false,
                    });
                }
            }

            done_event.write(DoneGeneratingEvent { chunk_pos });
        }
    }

    /// Done-state counterpart for a shrine listed in `shrine_counts`, if one exists.
    fn shrine_done_variant(obj: WorldObject) -> Option<WorldObject> {
        match obj {
            WorldObject::CombatShrine => Some(WorldObject::CombatShrineDone),
            WorldObject::GambleShrine => Some(WorldObject::GambleShrineDone),
            WorldObject::MicrowaveShrine => Some(WorldObject::MicrowaveShrineDone),
            WorldObject::HeirloomShrine => Some(WorldObject::HeirloomShrineDone),
            WorldObject::BlacksmithMerchant => Some(WorldObject::BlacksmithMerchantDone),
            WorldObject::ActiveSkillShrine => Some(WorldObject::ActiveSkillShrineDone),
            WorldObject::CauldronShrine => Some(WorldObject::CauldronShrineDone),
            WorldObject::ChaosTotem => Some(WorldObject::ChaosTotemDone),
            WorldObject::WellShrine => Some(WorldObject::WellShrineDone),
            WorldObject::WeaponShrine => Some(WorldObject::WeaponShrineDone),
            WorldObject::ArmorShrine => Some(WorldObject::ArmorShrineDone),
            WorldObject::AccessoryShrine => Some(WorldObject::AccessoryShrineDone),
            _ => None,
        }
    }

    /// `TEST_SHRINES=1`: once chunks around origin exist, place every Era1 `shrine_counts`
    /// shrine (active + done) in a 5-row grid for visual / interaction debugging.
    pub fn spawn_test_shrine_grid(
        mut place_item_event: MessageWriter<PlaceItemEvent>,
        game: GameParam,
        mut spawned: Local<bool>,
    ) {
        if !*TEST_SHRINES || *spawned {
            return;
        }
        if game.get_chunk_entity(IVec2::ZERO).is_none() {
            return;
        }
        if game.world_generation_params.shrine_counts.is_empty() {
            return;
        }

        let mut shrines: Vec<WorldObject> = game
            .world_generation_params
            .shrine_counts
            .keys()
            .copied()
            .collect();
        // Stable left-to-right order (HashMap iteration order is otherwise arbitrary).
        shrines.sort_by(|a, b| format!("{a:?}").cmp(&format!("{b:?}")));

        const ROWS: usize = 5;
        const SPACING_X: f32 = 64.;
        const SPACING_Y: f32 = 96.;
        // Start about one chunk west of origin so the row reads near the player spawn.
        let start_x = -(CHUNK_SIZE as f32) * TILE_SIZE.x;
        let start_y = ((ROWS - 1) as f32) * SPACING_Y * 0.5;

        *spawned = true;
        info!(
            "[TEST_SHRINES] Spawning {} shrine types × {} rows (active+done pairs)",
            shrines.len(),
            ROWS
        );

        for row in 0..ROWS {
            let y = start_y - row as f32 * SPACING_Y;
            let mut col: usize = 0;
            for shrine in &shrines {
                let active_x = start_x + col as f32 * SPACING_X;
                place_item_event.write(PlaceItemEvent {
                    obj: *shrine,
                    pos: Vec2::new(active_x, y),
                    placed_by_player: false,
                    override_existing_obj: true,
                });
                col += 1;

                if let Some(done) = Self::shrine_done_variant(*shrine) {
                    let done_x = start_x + col as f32 * SPACING_X;
                    place_item_event.write(PlaceItemEvent {
                        obj: done,
                        pos: Vec2::new(done_x, y),
                        placed_by_player: false,
                        override_existing_obj: true,
                    });
                    col += 1;
                }
            }
        }
    }

    fn spawn_debug_chunk_borders(
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
    ) {
        if !*DEBUG {
            return;
        }
        let offset = Vec2::new(-8., -8.);
        //vertical
        for i in -10..10 {
            commands
                .spawn((
                    Mesh2d(meshes.add(Mesh::from(Rectangle::new(1.0, 100000.0)))),
                    MeshMaterial2d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                    Transform::from_translation(Vec3::new(
                        i as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x + offset.x,
                        0. + offset.y,
                        900.,
                    )),
                ))
                .insert(Name::new("debug chunk border y"));
        }

        //horizontal
        for i in -10..10 {
            commands
                .spawn((
                    Mesh2d(meshes.add(Mesh::from(Rectangle::new(100000.0, 1.0)))),
                    MeshMaterial2d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                    Transform::from_translation(Vec3::new(
                        offset.x,
                        i as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y + offset.y,
                        900.,
                    )),
                ))
                .insert(Name::new("debug chunk border x"));
        }
    }
}

/// After normal object placement, grow each seed `IcePatch` by 5–14 extra tiles (cardinal-only flood),
/// replacing snow grass where needed. Era 3 overworld only.
fn extend_ice_patches_from_seeds(
    objs: &mut HashMap<TileMapPosition, WorldObject>,
    game: &GameParam,
) {
    let seeds: Vec<TileMapPosition> = objs
        .iter()
        .filter(|(_, o)| **o == WorldObject::IcePatch)
        .map(|(p, _)| *p)
        .collect();
    if seeds.is_empty() {
        return;
    }

    const NEIGHBORS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

    let mut rng = rand::thread_rng();

    for seed in seeds {
        let extra = rng.gen_range(5..=14);
        let mut patch: HashSet<TileMapPosition> = HashSet::new();
        patch.insert(seed);
        for _ in 0..extra {
            let mut candidate_set: HashSet<TileMapPosition> = HashSet::default();
            for p in &patch {
                for &(dx, dy) in &NEIGHBORS {
                    let n = get_neighbour_tile(*p, (dx, dy));
                    if patch.contains(&n) {
                        continue;
                    }
                    if !tile_allows_ice_patch_placement(game, n) {
                        continue;
                    }
                    match objs.get(&n) {
                        None => {
                            candidate_set.insert(n);
                        }
                        Some(
                            WorldObject::SnowGrass1
                            | WorldObject::SnowGrass2
                            | WorldObject::SnowGrass3
                            | WorldObject::SnowGrass4,
                        ) => {
                            candidate_set.insert(n);
                        }
                        Some(WorldObject::IcePatch) => {}
                        _ => {}
                    }
                }
            }
            if candidate_set.is_empty() {
                break;
            }
            let candidates: Vec<TileMapPosition> = candidate_set.into_iter().collect();
            let choice = *candidates.choose(&mut rng).unwrap();
            objs.insert(choice, WorldObject::IcePatch);
            patch.insert(choice);
        }
    }
}

fn tile_allows_ice_patch_placement(game: &GameParam, pos: TileMapPosition) -> bool {
    let tile = if let Some(tile_data) = game.get_tile_data(pos) {
        tile_data.block_type
    } else {
        return false;
    };
    if tile.iter().any(|t| *t == WorldObject::WaterTile) {
        return false;
    }
    let filter = game
        .world_generation_params
        .obj_allowed_tiles_map
        .get(&WorldObject::IcePatch)
        .unwrap_or(&vec![WorldObject::GrassTile])
        .clone();
    for allowed_tile in filter.iter() {
        if tile.iter().filter(|t| *t == allowed_tile).count() == 4 {
            return true;
        }
    }
    false
}

pub fn get_radial_tile_positions(origin: TileMapPosition, radius: i8) -> Vec<TileMapPosition> {
    //TODO: add rng padding around edges
    let mut positions = vec![];
    let origin_pos = tile_pos_to_world_pos(origin, false);
    let max_dist = radius as i32 * (TILE_SIZE.x as i32);
    for x in -radius..=radius {
        for y in -radius..=radius {
            let pos = get_neighbour_tile(origin, (x, y));
            let dist = tile_pos_to_world_pos(pos, false).distance(origin_pos);
            if dist <= max_dist as f32 {
                positions.push(pos);
            }
        }
    }

    positions
}
