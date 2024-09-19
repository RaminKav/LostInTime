use crate::{inputs::CursorPos, world, Game};
use bevy::{prelude::*, render::view::RenderLayers};
use bevy_ecs_tilemap::tiles::TilePos;

use super::{Interactable, UIState};

pub fn pointcast_2d<'a>(
    cursor_pos: &Res<CursorPos>,
    ui_sprites: &'a Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    excluded_entity: Option<Entity>,
) -> Option<(Entity, &'a Sprite, &'a GlobalTransform)> {
    let mut ret: Option<(Entity, &Sprite, &GlobalTransform)> = None;

    for (ent, sprite, xform) in ui_sprites.iter() {
        if let Some(excluded) = excluded_entity {
            if ent == excluded {
                continue;
            }
        }

        let Some(size) = sprite.custom_size else {
            continue;
        };

        let initial_x = xform.translation().x - (0.5 * size.x);
        let initial_y = xform.translation().y - (0.5 * size.y);

        let terminal_x = initial_x + size.x;
        let terminal_y = initial_y + size.y;
        if (initial_x..=terminal_x).contains(&cursor_pos.ui_coords.x)
            && (initial_y..=terminal_y).contains(&cursor_pos.ui_coords.y)
        {
            ret = Some((ent, sprite, xform));
        }
    }

    ret
}

pub fn _get_player_chunk_tile_coords(game: &mut Game) -> (IVec2, TilePos) {
    let player_pos = game.player_state.position;
    let chunk_pos =
        world::world_helpers::camera_pos_to_chunk_pos(&Vec2::new(player_pos.x, player_pos.y));
    let tile_pos =
        world::world_helpers::camera_pos_to_tile_pos(&Vec2::new(player_pos.x, player_pos.y));
    (chunk_pos, tile_pos)
}

pub fn spawn_ui_overlay(commands: &mut Commands, size: Vec2, alpha: f32, depth: f32) -> Entity {
    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., alpha),
                custom_size: Some(size),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., depth),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(UIState::Inventory)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"))
        .id()
}
