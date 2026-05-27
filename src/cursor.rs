use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;

use crate::assets::Graphics;
use crate::inputs::{cursor_pos_in_ui, cursor_pos_in_world, player_move_inputs};
use crate::item::WorldObject;
use crate::{GameState, TextureCamera, UICamera, DEBUG};

#[derive(Reflect, Resource, Debug)]
#[reflect(Resource)]
pub struct CursorPos {
    pub world_coords: Vec3,
    pub screen_coords: Vec3,
    pub ui_coords: Vec3,
}
impl Default for CursorPos {
    fn default() -> Self {
        CursorPos {
            world_coords: Vec3::new(999., 0., 0.),
            screen_coords: Vec3::new(999., 0., 0.),
            ui_coords: Vec3::new(999., 0., 0.),
        }
    }
}
/// Marker component for the custom cursor sprite
#[derive(Component)]
pub struct CustomCursor;

pub struct CustomCursorPlugin;

/// Custom sprite cursor and hidden OS cursor are off when `DEBUG` is set (e.g. `DEBUG=1`).
fn use_custom_cursor() -> bool {
    !*DEBUG
}

impl Plugin for CustomCursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(update_cursor_pos.after(player_move_inputs))
            // Setup cursor once graphics are loaded (after Loading state)
            .add_system(
                setup_custom_cursor
                    .run_if(use_custom_cursor)
                    .in_schedule(OnEnter(GameState::MainMenu)),
            )
            // Hide system cursor as soon as we leave loading
            .add_system(
                hide_system_cursor
                    .run_if(use_custom_cursor)
                    .in_schedule(OnEnter(GameState::MainMenu)),
            )
            // Update cursor position in all game states (not just Main)
            .add_system(update_custom_cursor_position.run_if(use_custom_cursor));
    }
}

fn hide_system_cursor(mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.get_single_mut() {
        window.cursor.visible = false;
    }
}

fn setup_custom_cursor(
    mut commands: Commands,
    graphics: Res<Graphics>,
    existing_cursors: Query<Entity, With<CustomCursor>>,
) {
    // Don't spawn if already exists
    if !existing_cursors.is_empty() {
        return;
    }

    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        warn!("No texture atlas available for custom cursor");
        return;
    };

    let Some(spritesheet_map) = graphics.spritesheet_map.as_ref() else {
        warn!("No spritesheet map available for custom cursor");
        return;
    };

    // Use Flint as the cursor sprite (placeholder)
    let Some(cursor_sprite) = spritesheet_map.get(&WorldObject::Flint).cloned() else {
        warn!("Flint sprite not found for custom cursor");
        return;
    };

    // Spawn the custom cursor sprite on UI layer (render layer 3)
    // Uses ui_coords so it follows the cursor correctly regardless of camera position
    commands.spawn((
        SpriteSheetBundle {
            texture_atlas: texture_atlas.clone(),
            sprite: cursor_sprite,
            transform: Transform::from_translation(Vec3::new(999., 0., 999.)),
            ..default()
        },
        CustomCursor,
        RenderLayers::from_layers(&[3]), // UI camera layer
        Name::new("CustomCursor"),
    ));
}

fn update_custom_cursor_position(
    cursor_pos: Res<CursorPos>,
    mut cursor_query: Query<&mut Transform, With<CustomCursor>>,
) {
    // Use UI coordinates since we're on render layer 3 (UI camera)
    // This ensures the cursor follows the mouse regardless of game camera position
    let ui_pos = cursor_pos.ui_coords;

    for mut transform in cursor_query.iter_mut() {
        transform.translation.x = ui_pos.x;
        transform.translation.y = ui_pos.y;
    }
}

pub fn update_cursor_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    world_camera_q: Query<(&Transform, &Camera), With<TextureCamera>>,
    ui_camera_q: Query<&Camera, With<UICamera>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    let Ok((world_cam_t, world_cam)) = world_camera_q.get_single() else {
        return;
    };
    let Ok(ui_cam) = ui_camera_q.get_single() else {
        return;
    };

    for cursor_moved in cursor_moved_events.iter() {
        *cursor_pos = CursorPos {
            world_coords: cursor_pos_in_world(
                &windows,
                cursor_moved.position,
                world_cam_t,
                world_cam,
            ),
            ui_coords: cursor_pos_in_ui(&windows, cursor_moved.position, ui_cam),
            screen_coords: cursor_moved.position.extend(0.),
        };
    }
}
