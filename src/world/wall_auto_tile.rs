use bevy::{platform::collections::HashMap, prelude::*};

use crate::{
    item::{PlaceItemEvent, Wall},
    proto::proto_param::ProtoParam,
    GameParam,
};

use super::{
    chunk::GenerateObjectsEvent,
    dungeon::Dungeon,
    generation::WallBreakEvent,
    world_helpers::{get_neighbour_tile, get_neighbour_wall_data, world_pos_to_tile_pos},
    TileMapPosition,
};
/// Marker set on a wall entity whenever its auto-tile neighbors change so it
/// gets re-evaluated in the next auto-tile pass. Very high churn while the
/// player places/breaks walls — stored `SparseSet` so re-tiling a wall
/// doesn't move it through an extra archetype every neighbor update.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Dirty;

/// Marker set on a wall entity after it has finished its auto-tile pass, so
/// neighbouring updates can short-circuit the work. Also `SparseSet` for the
/// same reason as `Dirty`.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct AutoTileComplete;

#[derive(Component)]
pub struct ChunkWallCache {
    pub walls: HashMap<TileMapPosition, bool>,
}
pub fn handle_wall_break(
    mut game: GameParam,
    mut commands: Commands,
    proto_param: ProtoParam,
    mut obj_break_events: MessageReader<WallBreakEvent>,
    mut chunk_wall_cache: Query<&mut ChunkWallCache>,
) {
    let mut removed_wall_pos = Vec::new();
    for broken_wall in obj_break_events.read() {
        let chunk_e = game.get_chunk_entity(broken_wall.pos.chunk_pos).unwrap();
        if let Ok(mut cache) = chunk_wall_cache.get_mut(chunk_e) {
            cache.walls.insert(broken_wall.pos, false);
            removed_wall_pos.push(broken_wall.pos);
        }
    }
    for pos in removed_wall_pos.iter() {
        mark_neighbour_walls_dirty(
            *pos,
            &mut game,
            &proto_param,
            &mut commands,
            &chunk_wall_cache,
        );
    }
}
pub fn handle_wall_placed(
    mut game: GameParam,
    mut commands: Commands,
    proto_param: ProtoParam,
    mut events: MessageReader<PlaceItemEvent>,

    mut chunk_wall_cache: Query<&mut ChunkWallCache>,
    dungeon_check: Query<&Dungeon>,
) {
    let mut new_walls_pos = Vec::new();
    for PlaceItemEvent {
        obj,
        pos,
        placed_by_player,
        override_existing_obj: _,
    } in events.read()
    {
        if !placed_by_player && dungeon_check.single().is_ok() {
            continue;
        }
        if proto_param.get_component::<Wall, _>(*obj).is_none() {
            continue;
        }
        let new_wall_pos = world_pos_to_tile_pos(*pos);
        let Some(chunk_e) = game.get_chunk_entity(new_wall_pos.chunk_pos) else {
            continue;
        };
        if let Ok(mut cache) = chunk_wall_cache.get_mut(chunk_e) {
            cache.walls.insert(world_pos_to_tile_pos(*pos), true);
            new_walls_pos.push(new_wall_pos);
        }
    }
    for new_wall_pos in new_walls_pos.iter() {
        mark_neighbour_walls_dirty(
            *new_wall_pos,
            &mut game,
            &proto_param,
            &mut commands,
            &chunk_wall_cache,
        );
    }
}

pub fn mark_neighbour_walls_dirty(
    target_pos: TileMapPosition,
    game: &mut GameParam,
    proto_param: &ProtoParam,
    commands: &mut Commands,
    chunk_wall_cache: &Query<&mut ChunkWallCache>,
) {
    for dy in -1i8..=1 {
        for dx in -1i8..=1 {
            //skip corner block updates
            if dx == 0 && dy == 0 {
                continue;
            }
            let wall_pos = target_pos;
            let neighbour_pos = get_neighbour_tile(wall_pos, (dx, dy));
            let Some(neighbour_chunk_e) = game.get_chunk_entity(neighbour_pos.chunk_pos) else {
                continue;
            };
            if let Ok(cache) = chunk_wall_cache.get(neighbour_chunk_e) {
                if let Some(true) = cache.walls.get(&neighbour_pos).cloned() {
                    let Some((new_wall_entity, _)) =
                        game.get_obj_entity_at_tile(neighbour_pos, proto_param)
                    else {
                        continue;
                    };
                    commands.entity(new_wall_entity).insert(Dirty);
                }
            } else if get_neighbour_wall_data(wall_pos, (dx, dy), game, proto_param).is_some() {
                let new_wall_entity = game
                    .get_obj_entity_at_tile(neighbour_pos, proto_param)
                    .unwrap();

                commands.entity(new_wall_entity.0).insert(Dirty);
            }
        }
    }
}
pub fn compute_wall_index(neighbour_bits: u8, edge: (i8, i8), remove: bool) -> u8 {
    let mut index = 0;
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
