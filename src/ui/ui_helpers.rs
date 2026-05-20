use crate::{cursor::CursorPos, keybinds::InputBinding, world, Game};
use bevy::{prelude::*, render::view::RenderLayers};
use bevy_ecs_tilemap::tiles::TilePos;

use super::{Interactable, UIState};

/// Typical full-screen UI overlays (inventory, shrines, class select, etc.) use z ≈ 9–15.
/// Active skill hotbar: above those modals, below the heirloom pick screen.
pub const Z_DEPTH_HUD_ACTIVE_SKILLS: f32 = 4.0;
/// “Choose an Heirloom” screen only: backdrop above active skills, below HUD heirloom row.
pub const Z_DEPTH_HEIRLOOM_SKILL_CHOICE_OVERLAY: f32 = 52.0;
/// Root depth for title bar, cards, reroll/banish buttons on that screen.
pub const Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT: f32 = 54.0;
/// Reroll/banish count labels (slightly in front of sibling UI on the same screen).
pub const Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND: f32 = 58.0;
/// HUD heirloom icons (top-left): above heirloom selection UI, still below options menu.
pub const Z_DEPTH_HUD_HEIRLOOM_ICONS: f32 = 62.0;
/// Options menu backdrop + content sit above gameplay HUD (including skills/heirlooms).
/// Kept below name-entry / loading overlays (z ≈ 100+).
pub const Z_DEPTH_OPTIONS_OVERLAY: f32 = 85.0;
pub const Z_DEPTH_OPTIONS_CONTENT: f32 = 86.0;

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

/// Spawn a keybind badge: 19×9 grey box + centered label.
///
/// When `parent` is `Some`, `transform` is local to that parent (e.g. corner HUD icons).
/// When `parent` is `None`, `transform` is world-space (e.g. bottom-anchored hotbar / skills).
pub fn spawn_keybind_badge(
    commands: &mut Commands,
    asset_server: &AssetServer,
    key: InputBinding,
    transform: Transform,
    parent: Option<Entity>,
    render_layer: u8,
) -> (Entity, Entity) {
    let mut key_bg = commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: crate::ui::KEYBIND_BADGE_COLOR,
            custom_size: Some(crate::ui::KEYBIND_BADGE_SIZE),
            ..default()
        },
        transform,
        ..default()
    });
    key_bg.insert(RenderLayers::from_layers(&[render_layer]));
    if let Some(parent) = parent {
        key_bg.set_parent(parent);
    }
    let key_bg = key_bg.id();

    let key_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                crate::keybinds::get_key_display_name(key),
                TextStyle {
                    font: asset_server.load("fonts/slkscr.ttf"),
                    font_size: 8.4,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[render_layer]))
        .set_parent(key_bg)
        .id();

    (key_bg, key_text)
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
