use std::fs::File;
use std::io::BufReader;

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use serde::{Deserialize, Serialize};

use crate::assets::Graphics;
use crate::datafiles;
use crate::inputs::{cursor_pos_in_ui, cursor_pos_in_world};
use crate::{TextureCamera, UICamera, DEBUG};

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

    pub fn apply_sprite_settings(sprite: &mut Sprite, base: &Sprite, double_size: bool) {
        *sprite = base.clone();
        if double_size {
            if let Some(size) = sprite.custom_size.as_mut() {
                *size *= 2.0;
            }
        }
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
        // Must run every frame. Pre-0.19 this lived in CoreSet::PostUpdate; a mistaken
        // OnEnter(MainMenu) migration left CursorPos stuck at the default off-screen
        // coords, so menu hover/clicks and the custom cursor all died.
        // Stay in Update (not PostUpdate) so systems like aim that `.after` this
        // don't create an Update ↔ PostUpdate schedule cycle on 0.19.
        app.add_systems(
            Update,
            update_cursor_pos.after(crate::inputs::move_camera_with_player),
        )
        // Retry until the sheet atlas exists (loading may finish after MainMenu enter).
        // Only hide the OS cursor once the custom sprite actually spawns.
        .add_systems(Update, setup_custom_cursor.run_if(use_custom_cursor))
        .add_systems(
            Update,
            update_custom_cursor_position.run_if(use_custom_cursor),
        )
        .add_systems(
            Update,
            update_custom_cursor_appearance.run_if(use_custom_cursor),
        );
    }
}

fn setup_custom_cursor(
    mut commands: Commands,
    graphics: Res<Graphics>,
    cursor_color: Res<CursorColorSettings>,
    existing_cursors: Query<Entity, With<CustomCursor>>,
    mut cursor_options: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut warned_missing_atlas: Local<bool>,
) {
    // Don't spawn if already exists
    if !existing_cursors.is_empty() {
        return;
    }

    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        if !*warned_missing_atlas {
            warn!("No texture atlas available for custom cursor yet; retrying…");
            *warned_missing_atlas = true;
        }
        return;
    }

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
        cursor_sprite,
        Transform::from_translation(Vec3::new(999., 0., 999.)),
        CustomCursor,
        RenderLayers::from_layers(&[3]), // UI camera layer
        Name::new("CustomCursor"),
    ));

    if let Ok(mut cursor) = cursor_options.single_mut() {
        cursor.visible = false;
    }
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
    mut cursor_query: Query<&mut Sprite, With<CustomCursor>>,
) {
    if !cursor_color.is_changed() {
        return;
    }
    let Some(base_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        return;
    };
    let mut updated = base_sprite.clone();
    CursorColorSettings::apply_sprite_settings(
        &mut updated,
        &base_sprite,
        cursor_color.double_size,
    );
    for mut sprite in cursor_query.iter_mut() {
        *sprite = updated.clone();
    }
}

pub fn update_cursor_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    world_camera_q: Query<(&GlobalTransform, &Camera), With<TextureCamera>>,
    ui_camera_q: Query<&Camera, With<UICamera>>,
    mut cursor_moved_events: MessageReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.read() {
        cursor_pos.screen_coords = cursor_moved.position.extend(0.);
    }

    let Ok(window) = windows.single() else {
        return;
    };
    if let Some(pos) = window.cursor_position() {
        cursor_pos.screen_coords = pos.extend(0.);
    }

    let Ok((world_cam_t, world_cam)) = world_camera_q.single() else {
        return;
    };
    let Ok(ui_cam) = ui_camera_q.single() else {
        return;
    };

    let screen = cursor_pos.screen_coords.truncate();
    cursor_pos.world_coords = cursor_pos_in_world(&windows, screen, world_cam_t, world_cam);
    cursor_pos.ui_coords = cursor_pos_in_ui(&windows, screen, ui_cam);
}
