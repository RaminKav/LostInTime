use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::{MaterialMesh2dBundle, Mesh2dHandle};
use bevy::window::PrimaryWindow;

use crate::assets::Graphics;
use crate::inputs::{cursor_pos_in_ui, cursor_pos_in_world, player_move_inputs};
use crate::item::ammo::Ammo;
use crate::item::WorldObject;
use crate::GameState;
use crate::{Player, TextureCamera};

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

/// Marker component for the reload indicator circle
#[derive(Component)]
pub struct ReloadIndicator;

/// Marker component for the reload indicator background
#[derive(Component)]
pub struct ReloadIndicatorBackground;

pub struct CustomCursorPlugin;

impl Plugin for CustomCursorPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(update_cursor_pos.after(player_move_inputs))
            // Setup cursor once graphics are loaded (after Loading state)
            .add_system(setup_custom_cursor.in_schedule(OnEnter(GameState::MainMenu)))
            // Hide system cursor as soon as we leave loading
            .add_system(hide_system_cursor.in_schedule(OnEnter(GameState::MainMenu)))
            // Update cursor position in all game states (not just Main)
            .add_system(update_custom_cursor_position)
            // Reload indicator only relevant during gameplay
            .add_system(update_reload_indicator.in_set(OnUpdate(GameState::Main)));
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
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

    // Create circle meshes for reload indicator
    let circle_mesh: Mesh2dHandle = meshes
        .add(
            shape::Circle {
                radius: 4.0,
                ..default()
            }
            .into(),
        )
        .into();

    let inner_circle_mesh: Mesh2dHandle = meshes
        .add(
            shape::Circle {
                radius: 4.0,
                ..default()
            }
            .into(),
        )
        .into();

    // Spawn reload indicator background (dark circle)
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: circle_mesh.clone(),
            material: materials.add(ColorMaterial::from(Color::rgba(0.1, 0.1, 0.1, 0.7))),
            transform: Transform::from_xyz(0., 0., 998.),
            visibility: Visibility::Hidden,
            ..default()
        },
        ReloadIndicatorBackground,
        RenderLayers::from_layers(&[3]),
        Name::new("ReloadIndicatorBackground"),
    ));

    // Spawn reload indicator foreground (progress circle)
    commands.spawn((
        MaterialMesh2dBundle {
            mesh: inner_circle_mesh,
            material: materials.add(ColorMaterial::from(Color::rgba(0.8, 0.3, 0.2, 0.9))),
            transform: Transform::from_xyz(0., 0., 999.),
            visibility: Visibility::Hidden,
            ..default()
        },
        ReloadIndicator,
        RenderLayers::from_layers(&[3]),
        Name::new("ReloadIndicator"),
    ));
}

fn update_custom_cursor_position(
    cursor_pos: Res<CursorPos>,
    mut cursor_query: Query<
        &mut Transform,
        (
            With<CustomCursor>,
            Without<ReloadIndicator>,
            Without<ReloadIndicatorBackground>,
        ),
    >,
    mut indicator_query: Query<
        &mut Transform,
        (
            With<ReloadIndicator>,
            Without<CustomCursor>,
            Without<ReloadIndicatorBackground>,
        ),
    >,
    mut background_query: Query<
        &mut Transform,
        (
            With<ReloadIndicatorBackground>,
            Without<CustomCursor>,
            Without<ReloadIndicator>,
        ),
    >,
) {
    // Use UI coordinates since we're on render layer 3 (UI camera)
    // This ensures the cursor follows the mouse regardless of game camera position
    let ui_pos = cursor_pos.ui_coords;

    // Update cursor position
    for mut transform in cursor_query.iter_mut() {
        transform.translation.x = ui_pos.x;
        transform.translation.y = ui_pos.y;
    }

    // Update reload indicator position (above cursor)
    let indicator_offset = 10.;
    for mut transform in indicator_query.iter_mut() {
        transform.translation.x = ui_pos.x;
        transform.translation.y = ui_pos.y + indicator_offset;
    }

    for mut transform in background_query.iter_mut() {
        transform.translation.x = ui_pos.x;
        transform.translation.y = ui_pos.y + indicator_offset;
    }
}

fn update_reload_indicator(
    player_query: Query<Entity, With<Player>>,
    ammo_query: Query<&Ammo>,
    game: crate::GameParam,
    mut indicator_query: Query<
        (&mut Visibility, &mut Transform, &Handle<ColorMaterial>),
        (With<ReloadIndicator>, Without<ReloadIndicatorBackground>),
    >,
    mut background_query: Query<
        &mut Visibility,
        (With<ReloadIndicatorBackground>, Without<ReloadIndicator>),
    >,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    let Ok(_player_e) = player_query.get_single() else {
        return;
    };

    // Check if player has a weapon that's reloading
    let is_reloading = if let Some(main_hand) = game.player().main_hand_slot.as_ref() {
        if let Ok(ammo) = ammo_query.get(main_hand.entity) {
            ammo.reloading
        } else {
            false
        }
    } else {
        false
    };

    // Get reload progress
    let reload_progress = if let Some(main_hand) = game.player().main_hand_slot.as_ref() {
        if let Ok(ammo) = ammo_query.get(main_hand.entity) {
            if ammo.reloading {
                ammo.reload.percent()
            } else {
                0.0
            }
        } else {
            0.0
        }
    } else {
        0.0
    };

    // Update indicator visibility and scale based on progress
    for (mut visibility, mut transform, material_handle) in indicator_query.iter_mut() {
        if is_reloading {
            *visibility = Visibility::Visible;
            // Scale the indicator based on reload progress (grows as reload completes)
            let scale = reload_progress.max(0.1); // Minimum scale so it's visible
            transform.scale = Vec3::new(scale, scale, 1.0);

            // Change color based on progress
            let new_color = if reload_progress < 0.5 {
                Color::rgba(0.8, 0.3, 0.2, 0.9) // Red/orange at start
            } else if reload_progress < 0.9 {
                Color::rgba(0.8, 0.8, 0.2, 0.9) // Yellow in middle
            } else {
                Color::rgba(0.2, 0.8, 0.3, 0.9) // Green when almost done
            };

            // Update material color
            if let Some(material) = materials.get_mut(material_handle) {
                material.color = new_color;
            }
        } else {
            *visibility = Visibility::Hidden;
        }
    }

    // Update background visibility
    for mut visibility in background_query.iter_mut() {
        if is_reloading {
            *visibility = Visibility::Visible;
        } else {
            *visibility = Visibility::Hidden;
        }
    }
}

pub fn update_cursor_pos(
    windows: Query<&Window, With<PrimaryWindow>>,
    camera_q: Query<(&Transform, &Camera), With<TextureCamera>>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.iter() {
        // To get the mouse's world position, we have to transform its window position by
        // any transforms on the camera. This is done by projecting the cursor position into
        // camera space (world space).
        for (cam_t, cam) in camera_q.iter() {
            *cursor_pos = CursorPos {
                world_coords: cursor_pos_in_world(&windows, cursor_moved.position, cam_t, cam),
                ui_coords: cursor_pos_in_ui(&windows, cursor_moved.position, cam),
                screen_coords: cursor_moved.position.extend(0.),
            };
        }
    }
}
