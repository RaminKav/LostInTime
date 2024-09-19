use bevy::{prelude::*, sprite::MaterialMesh2dBundle, utils::HashMap};
use bevy_rapier2d::prelude::Collider;
use itertools::Itertools;
use std::fmt::Debug;

use crate::{
    assets::SpriteAnchor,
    inventory::ItemStack,
    item::WorldObject,
    world::{world_helpers::world_pos_to_tile_pos, y_sort::YSort, TileMapPosition},
    GameParam, DEBUG_AI,
};
use pathfinding::prelude::astar;

#[derive(Default, Resource)]
pub struct PathfindingCache {
    pub tile_valid_cache: HashMap<AIPos, bool>,
}

#[derive(Default, Eq, PartialEq, Hash, Clone, Reflect, FromReflect, Copy)]
/// Tile position split into quadrants, without chunking. Positions are relative to [0,0].
/// Used to give more fine-grained control over pathfinding, compared to [TileMapPosition]
pub struct AIPos {
    x: i32,
    y: i32,
}

impl AIPos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn distance(&self, other: &AIPos) -> f32 {
        let x = (self.x - other.x) as f32;
        let y = (self.y - other.y) as f32;
        (x * x + y * y).sqrt()
    }
}
impl Debug for AIPos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({:?}, {:?})", self.x, self.y)
    }
}
#[derive(Component)]
pub struct DebugPath;
#[derive(Component)]
pub struct DebugPathDelete;

pub struct DebugPathResetEvent {
    pub path: Vec<AIPos>,
}

pub fn cache_ai_path_on_new_obj_spawn(
    new_objs: Query<
        (&GlobalTransform, &SpriteAnchor, &WorldObject),
        (With<WorldObject>, Added<Collider>, Without<ItemStack>),
    >,
    mut game: GameParam,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    for (transform, anchor, obj) in new_objs.iter() {
        let pos = transform.translation().truncate();
        let anchor_offset = if obj.is_tree() {
            anchor.0
        } else {
            Vec2::new(0., 0.)
        };
        for quads in &[(-4., -4.), (-4., 4.), (4., 4.), (4., -4.)] {
            let offset_pos = pos + Vec2::new(quads.0, quads.1) - anchor_offset;
            let ai_pos = world_pos_to_AIPos(offset_pos);
            game.set_pos_validity_for_pathfinding(ai_pos, false);
            if *DEBUG_AI {
                commands
                    .spawn(MaterialMesh2dBundle {
                        mesh: meshes
                            .add(
                                shape::Quad {
                                    size: Vec2::new(7.0, 7.0),
                                    ..Default::default()
                                }
                                .into(),
                            )
                            .into(),
                        transform: Transform::from_translation(Vec3::new(
                            offset_pos.x,
                            offset_pos.y,
                            0.,
                        )),
                        material: materials.add(Color::RED.into()),
                        ..default()
                    })
                    .insert(YSort(-0.1))
                    .insert(Name::new("debug chunk border x"));
            }
        }
    }
}
pub fn spawn_new_debug_path(
    mut commands: Commands,
    mut events: EventReader<DebugPathResetEvent>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    old_paths: Query<Entity, With<DebugPath>>,
    old_paths_to_delete: Query<Entity, With<DebugPathDelete>>,
) {
    for path in events.iter() {
        for old_path in old_paths.iter() {
            commands.entity(old_path).insert(DebugPathDelete);
        }
        for old_path in old_paths_to_delete.iter() {
            commands.entity(old_path).despawn_recursive();
        }
        for quad in path.path.clone() {
            let pos = AIPos_to_world_pos(quad);
            let is_last_pos = quad == *path.path.last().unwrap();

            commands
                .spawn(MaterialMesh2dBundle {
                    mesh: meshes
                        .add(
                            shape::Quad {
                                size: Vec2::new(7.0, 7.0),
                                ..Default::default()
                            }
                            .into(),
                        )
                        .into(),
                    transform: Transform::from_translation(Vec3::new(pos.x + 4., pos.y - 4., 0.)),
                    material: materials.add(if is_last_pos {
                        Color::GREEN.into()
                    } else {
                        Color::RED.into()
                    }),
                    ..default()
                })
                .insert(YSort(0.1))
                .insert(DebugPath)
                .insert(Name::new("AI PATH"));
        }
    }
}
pub fn get_next_tile_A_star(target: &Vec2, start: &Vec2, game: &mut GameParam) -> Option<Vec2> {
    let target_tile: AIPos = world_pos_to_AIPos(*target);
    let start_tile: AIPos = world_pos_to_AIPos(*start);

    let mut max_iteration = 0;
    if let Some(result) = astar(
        &start_tile,
        |p| {
            get_valid_adjacent_tiles(p, &target_tile, game)
                .iter()
                .map(|p| (*p, 1))
                .collect_vec()
        },
        |p| p.distance(&target_tile) as i32,
        |p| {
            max_iteration += 1;
            (p == &target_tile) || max_iteration > 500
        },
    ) {
        if result.0.len() < 3 {
            warn!("Pathfinding too short {:?}", result);
            return None;
        }
        if max_iteration > 499 {
            warn!("Pathfinding took too long, aborting");
            return None;
        }
        if *DEBUG_AI {
            debug!("Result len {:?} {max_iteration:?}", result.0.len());
            // game.debug_ai_path_event.send(DebugPathResetEvent {
            //     path: result.0.clone(),
            // });
        }
        Some(AIPos_to_world_pos(result.0[1]))
    } else {
        warn!("No path found for ASTAR");
        None
    }
}

pub fn get_valid_adjacent_tiles(pos: &AIPos, _target: &AIPos, game: &GameParam) -> Vec<AIPos> {
    let mut valid_tiles = Vec::new();
    let mut valid_offsets = Vec::new();
    // println!("  -> pos: {pos:?} {target:?}");
    for offset in &[
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ] {
        let neighbour_tile = get_neighbour_AIPos_tile(*pos, *offset);

        //then search for tile in cache
        if let Some(is_valid) = game.get_pos_validity_for_pathfinding(neighbour_tile) {
            if is_valid {
                valid_tiles.push(neighbour_tile);
                valid_offsets.push(*offset);
            }
            continue;
        }

        valid_tiles.push(neighbour_tile);
        valid_offsets.push(*offset);
    }
    remove_invalid_corners(valid_tiles, valid_offsets, *pos)
}

fn remove_invalid_corners(
    valid_tiles: Vec<AIPos>,
    valid_offsets: Vec<(i8, i8)>,
    origin: AIPos,
) -> Vec<AIPos> {
    let mut valid_tiles = valid_tiles.clone();
    let mut tiles_to_remove = Vec::new();
    if !valid_offsets.contains(&(-1, 0)) && !valid_offsets.contains(&(0, 1)) {
        let neighbour_to_remove = get_neighbour_AIPos_tile(origin, (-1, 1));
        tiles_to_remove.push(neighbour_to_remove);
    }
    if !valid_offsets.contains(&(1, 0)) && !valid_offsets.contains(&(0, 1)) {
        let neighbour_to_remove = get_neighbour_AIPos_tile(origin, (1, 1));
        tiles_to_remove.push(neighbour_to_remove);
    }
    if !valid_offsets.contains(&(-1, 0)) && !valid_offsets.contains(&(0, -1)) {
        let neighbour_to_remove = get_neighbour_AIPos_tile(origin, (-1, -1));
        tiles_to_remove.push(neighbour_to_remove);
    }
    if !valid_offsets.contains(&(1, 0)) && !valid_offsets.contains(&(0, -1)) {
        let neighbour_to_remove = get_neighbour_AIPos_tile(origin, (1, -1));
        tiles_to_remove.push(neighbour_to_remove);
    }
    valid_tiles.retain(|t| !tiles_to_remove.contains(t));
    valid_tiles
}

/// offset should not be larger than +/- 15
pub fn get_neighbour_AIPos_tile(pos: AIPos, offset: (i8, i8)) -> AIPos {
    AIPos::new(pos.x + offset.0 as i32, pos.y + offset.1 as i32)
}

pub fn world_pos_to_AIPos(pos: Vec2) -> AIPos {
    AIPos::new((pos.x / 8.) as i32, (pos.y / 8.) as i32)
}

pub fn AIPos_to_world_pos(pos: AIPos) -> Vec2 {
    Vec2::new(pos.x as f32 * 8., pos.y as f32 * 8.)
}
pub fn _AIPos_to_tile_pos(pos: AIPos) -> TileMapPosition {
    let w = AIPos_to_world_pos(pos);
    world_pos_to_tile_pos(w)
}

pub fn _flood_fill(
    pos: AIPos,
    game: &mut GameParam,
    visited: &mut Vec<AIPos>,
    max_depth: i32,
    depth: i32,
) {
    if depth > max_depth {
        return;
    }
    visited.push(pos);
    for offset in &[
        (0, 1),
        (0, -1),
        (1, 0),
        (-1, 0),
        (1, 1),
        (1, -1),
        (-1, 1),
        (-1, -1),
    ] {
        let neighbour_tile = get_neighbour_AIPos_tile(pos, *offset);
        if visited.contains(&neighbour_tile) {
            continue;
        }
        if let Some(is_valid) = game.get_pos_validity_for_pathfinding(neighbour_tile) {
            if is_valid {
                _flood_fill(neighbour_tile, game, visited, max_depth, depth + 1);
            }
        } else {
            _flood_fill(neighbour_tile, game, visited, max_depth, depth + 1);
        }
    }
}
