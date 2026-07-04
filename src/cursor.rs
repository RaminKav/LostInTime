use std::fs::File;
use std::io::BufReader;

use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::window::PrimaryWindow;
use serde::{Deserialize, Serialize};

use crate::assets::Graphics;
use crate::datafiles;
use crate::inputs::{cursor_pos_in_ui, cursor_pos_in_world};
use crate::{GameState, TextureCamera, UICamera, DEBUG};

/// Number of selectable custom-cursor colors (sheet positions (4,1)..(11,1)).
pub const NUM_CURSOR_COLORS: u8 = 8;

/// Persisted player choice of custom cursor color, as an index into the cursor color sprites.
#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorColorSettings {
    pub index: u8,
    /// When true, the in-game custom cursor renders at 2× its normal size.
    #[serde(default)]
    pub double_size: bool,
}

impl Default for CursorColorSettings {
    fn default() -> Self {
        Self {
            index: 0,
            double_size: false,
        }
    }
}

impl CursorColorSettings {
    pub fn load() -> Self {
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.cursor_color.unwrap_or_default().sanitized();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = datafiles::game_data();
        let mut game_data = if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.cursor_color = Some(self.sanitized());

        if let Ok(file) = File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            index: self.index % NUM_CURSOR_COLORS,
            double_size: self.double_size,
        }
    }

    pub fn apply_sprite_settings(sprite: &mut TextureAtlasSprite, base: &TextureAtlasSprite, double_size: bool) {
        sprite.index = base.index;
        sprite.custom_size = base.custom_size.map(|size| {
            if double_size {
                size * 2.0
            } else {
                size
            }
        });
    }

    /// Cycle to the next/previous color, wrapping around.
    pub fn nudge(&mut self, forward: bool) {
        self.index = if forward {
            (self.index + 1) % NUM_CURSOR_COLORS
        } else {
            (self.index + NUM_CURSOR_COLORS - 1) % NUM_CURSOR_COLORS
        };
    }
}

#[derive(Reflect, Resource, Debug)]
#[reflect(Resource)]
pub struct CursorPos {
    pub world_coords: Vec3,
    pub screen_coords: Vec3,
    pub ui_coords: Vec3,
    /// When true, UI hit-tests act as if the cursor is off-screen. Set while mouseless mode is
    /// on or a gamepad is connected whenever a UI screen/overlay opens, until the player moves
    /// the mouse (see `update_cursor_ui_hover_suppression` in `ui/focus.rs`).
    pub suppress_ui_hover: bool,
}
impl Default for CursorPos {
    fn default() -> Self {
        CursorPos {
            world_coords: Vec3::new(999., 0., 0.),
            screen_coords: Vec3::new(999., 0., 0.),
            ui_coords: Vec3::new(999., 0., 0.),
            suppress_ui_hover: false,
        }
    }
}

impl CursorPos {
    /// Whether UI sprites under the cursor should register mouse hover this frame.
    pub fn ui_hover_hit_allowed(&self) -> bool {
        !self.suppress_ui_hover
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
        app.add_system(
            update_cursor_pos
                .after(crate::inputs::move_camera_with_player)
                .in_base_set(CoreSet::PostUpdate),
        )
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
            .add_system(update_custom_cursor_position.run_if(use_custom_cursor))
            // Re-skin the cursor whenever the chosen color or size changes
            .add_system(update_custom_cursor_appearance.run_if(use_custom_cursor));
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
    cursor_color: Res<CursorColorSettings>,
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

    let Some(base_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        warn!("No cursor color sprite available for custom cursor");
        return;
    };
    let mut cursor_sprite = base_sprite.clone();
    CursorColorSettings::apply_sprite_settings(
        &mut cursor_sprite,
        &base_sprite,
        cursor_color.double_size,
    );

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

fn update_custom_cursor_appearance(
    cursor_color: Res<CursorColorSettings>,
    graphics: Res<Graphics>,
    mut cursor_query: Query<&mut TextureAtlasSprite, With<CustomCursor>>,
) {
    if !cursor_color.is_changed() {
        return;
    }
    let Some(base_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        return;
    };
    for mut sprite in cursor_query.iter_mut() {
        CursorColorSettings::apply_sprite_settings(
            &mut sprite,
            &base_sprite,
            cursor_color.double_size,
        );
    }
}

pub fn update_cursor_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    world_camera_q: Query<(&Transform, &Camera), With<TextureCamera>>,
    ui_camera_q: Query<&Camera, With<UICamera>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.iter() {
        cursor_pos.screen_coords = cursor_moved.position.extend(0.);
    }

    let Ok(window) = windows.get_single() else {
        return;
    };
    if let Some(pos) = window.cursor_position() {
        cursor_pos.screen_coords = pos.extend(0.);
    }

    let Ok((world_cam_t, world_cam)) = world_camera_q.get_single() else {
        return;
    };
    let Ok(ui_cam) = ui_camera_q.get_single() else {
        return;
    };

    let screen = cursor_pos.screen_coords.truncate();
    cursor_pos.world_coords =
        cursor_pos_in_world(&windows, screen, world_cam_t, world_cam);
    cursor_pos.ui_coords = cursor_pos_in_ui(&windows, screen, ui_cam);
}
