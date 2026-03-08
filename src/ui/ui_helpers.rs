use crate::{cursor::CursorPos, world, Game};
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

/// Format a number to condensed display format:
/// 0-9999: no change
/// 10000-99999: 12.4k, 88.2k, etc
/// 100000-999999: 134k, 478k
/// 1000000-999999999: 1.43M, 20.5M, 999M, etc
/// 1000000000+: 1.43B, 20.5B, etc
pub fn format_number(value: i64) -> String {
    let abs = value.unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    if abs < 10_000 {
        format!("{}{}", sign, abs)
    } else if abs < 100_000 {
        format!("{}{:.1}k", sign, abs as f64 / 1_000.0)
    } else if abs < 1_000_000 {
        format!("{}{}k", sign, abs / 1_000)
    } else if abs < 1_000_000_000 {
        let millions = abs as f64 / 1_000_000.0;
        let formatted = format!("{}{:.2}M", sign, millions);
        let trimmed = formatted.trim_end_matches('0');
        if trimmed.ends_with('.') {
            trimmed.trim_end_matches('.').to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        let billions = abs as f64 / 1_000_000_000.0;
        let formatted = format!("{}{:.2}B", sign, billions);
        let trimmed = formatted.trim_end_matches('0');
        if trimmed.ends_with('.') {
            trimmed.trim_end_matches('.').to_string()
        } else {
            trimmed.to_string()
        }
    }
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
