use bevy::prelude::*;

use crate::{item::Wall, proto::proto_param::ProtoParam, GameParam};

use super::{
    generation::WallBreakEvent,
    world_helpers::{get_neighbour_obj_data, get_neighbours_tile, world_pos_to_tile_pos},
    TileMapPosition,
};
#[derive(Component)]
pub struct Dirty;
pub fn update_wall(
    mut commands: Commands,
    proto_param: ProtoParam,
    mut walls_to_update: Query<(Entity, &mut TextureAtlasSprite), (With<Wall>, With<Dirty>)>,
    mut game: GameParam,
    txns: Query<&GlobalTransform>,
) {
    for (wall_entity, mut wall_sprite) in walls_to_update.iter_mut() {
        let mut has_wall_above = false;
        let mut has_wall_below = false;
        let mut has_wall_on_left_side = false;
        let mut has_wall_on_right_side = false;
        let new_wall_pos =
            world_pos_to_tile_pos(txns.get(wall_entity).unwrap().translation().truncate());
        commands.entity(wall_entity).remove::<Dirty>();
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                //skip corner block updates for walls
                if (dx != 0 && dy != 0) || (dx == 0 && dy == 0) {
                    continue;
                }
                // only use neighbours that are a wall
                let mut neighbour_is_wall = false;
                if let Some(neighbour_block_entity_data) =
                    get_neighbour_obj_data(new_wall_pos.clone(), (dx, dy), &mut game)
                {
                    if let Some(_wall) =
                        proto_param.get_component::<Wall, _>(neighbour_block_entity_data.object)
                    {
                        if dy == 1 {
                            has_wall_above = true;
                        } else if dy == -1 {
                            has_wall_below = true;
                        } else if dx == -1 {
                            has_wall_on_left_side = true;
                        } else if dx == 1 {
                            has_wall_on_right_side = true;
                        }
                        neighbour_is_wall = true;
                    }
                }
                let mut new_wall_data = game.get_tile_obj_data_mut(new_wall_pos.clone()).unwrap();

                let updated_bit_index =
                    compute_wall_index(new_wall_data.obj_bit_index, (dx, dy), !neighbour_is_wall);
                new_wall_data.texture_offset = 0;
                if new_wall_data.obj_bit_index == updated_bit_index {
                    continue;
                }

                new_wall_data.obj_bit_index = updated_bit_index;
                (*wall_sprite).index = (updated_bit_index + new_wall_data.texture_offset) as usize;
                if neighbour_is_wall {
                    let neighbour_pos = get_neighbours_tile(new_wall_pos.clone(), (dx, dy));
                    let neighbour_entity = game.get_obj_entity_at_tile(neighbour_pos);
                    commands.entity(neighbour_entity.unwrap()).insert(Dirty);
                    // mark corners as dirty too
                    if let Some(top_left_corner_entity) = game
                        .get_obj_entity_at_tile(get_neighbours_tile(new_wall_pos.clone(), (-1, 1)))
                    {
                        commands.entity(top_left_corner_entity).insert(Dirty);
                    }
                    if let Some(top_right_corner_entity) = game
                        .get_obj_entity_at_tile(get_neighbours_tile(new_wall_pos.clone(), (1, 1)))
                    {
                        commands.entity(top_right_corner_entity).insert(Dirty);
                    }
                }
            }
        }
        let mut first_corner_neighbour_is_not_wall = false;
        let mut is_weird_edge_case_corner = false;
        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                //only bottom corner block updates now
                if dx == 0 || dy != -1 {
                    continue;
                }
                // only use neighbours that are walls
                let mut this_corner_neighbour_is_wall = false;
                if let Some(neighbour_block_entity_data) =
                    get_neighbour_obj_data(new_wall_pos.clone(), (dx, dy), &mut game)
                {
                    this_corner_neighbour_is_wall = proto_param
                        .get_component::<Wall, _>(neighbour_block_entity_data.object)
                        .is_some();
                }
                let mut new_wall_data = game.get_tile_obj_data_mut(new_wall_pos.clone()).unwrap();

                let has_wall_on_this_side = if dx == -1 {
                    has_wall_on_left_side
                } else {
                    has_wall_on_right_side
                };
                if !(this_corner_neighbour_is_wall || !has_wall_on_this_side || !has_wall_below) {
                    let updated_bit_index = if has_wall_above
                        && has_wall_on_left_side
                        && has_wall_on_right_side
                    {
                        if first_corner_neighbour_is_not_wall && !this_corner_neighbour_is_wall {
                            10
                        } else if dx == -1 {
                            14
                        } else {
                            15
                        }
                    } else if has_wall_above {
                        if dx == -1 {
                            7
                        } else {
                            6
                        }
                    } else if !has_wall_above && has_wall_on_left_side && has_wall_on_right_side {
                        if first_corner_neighbour_is_not_wall && !this_corner_neighbour_is_wall {
                            4
                        } else if dx == -1 {
                            13
                        } else {
                            11
                        }
                    } else {
                        new_wall_data.obj_bit_index
                    };
                    new_wall_data.texture_offset = 16;
                    is_weird_edge_case_corner = true;
                    if wall_sprite.index
                        == (updated_bit_index + new_wall_data.texture_offset) as usize
                    {
                        continue;
                    }
                    new_wall_data.obj_bit_index = updated_bit_index;
                    (*wall_sprite).index =
                        (updated_bit_index + new_wall_data.texture_offset) as usize;
                    first_corner_neighbour_is_not_wall = true;
                }
                // just trust me on this one
                if !is_weird_edge_case_corner {
                    new_wall_data.texture_offset = 0;
                    (*wall_sprite).index =
                        (new_wall_data.obj_bit_index + new_wall_data.texture_offset) as usize;
                }
            }
        }
    }
}
pub fn handle_wall_break(
    mut game: GameParam,
    proto_param: ProtoParam,
    mut obj_break_events: EventReader<WallBreakEvent>,
    mut commands: Commands,
) {
    for broken_wall in obj_break_events.iter() {
        let chunk_pos = broken_wall.chunk_pos;

        for dy in -1i8..=1 {
            for dx in -1i8..=1 {
                //skip corner block updates
                if dx == 0 && dy == 0 {
                    continue;
                }
                let wall_pos = TileMapPosition::new(chunk_pos, broken_wall.tile_pos, 0);
                let pos = get_neighbours_tile(wall_pos.clone(), (dx, dy));

                if let Some(neighbour_block_entity_data) =
                    get_neighbour_obj_data(wall_pos, (dx, dy), &mut game)
                {
                    if let Some(_wall) =
                        proto_param.get_component::<Wall, _>(neighbour_block_entity_data.object)
                    {
                        let new_wall_entity = game.get_obj_entity_at_tile(pos.clone()).unwrap();

                        commands.entity(new_wall_entity).insert(Dirty);
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
