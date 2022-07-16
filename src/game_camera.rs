use crate::assets::TILE_SIZE;
use crate::{GameState, HEIGHT, RESOLUTION};
use bevy::prelude::*;
use bevy::render::camera::{Camera2d, ScalingMode};
// use bevy_inspector_egui::{Inspectable, RegisterInspectable};

pub struct GameCameraPlugin;

/// Marks something that should always be in a constant place on screen,
/// Currently only used for the campfire overlay but there are probably better
/// ways of handling this
#[derive(Component, Default)]
pub struct CameraFollower {
    //TODO find a better way to force ordering
    pub offset: f32,
}

impl Plugin for GameCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_system_set(
            SystemSet::on_enter(GameState::Main).with_system(Self::spawn_camera.label("camera")),
        )
        .add_system_set(
            SystemSet::on_update(GameState::Main)
                .with_system(Self::camera_follow)
                //  .with_system(Self::camera_follows_player),
        );
        // .register_inspectable::<CameraFollower>();
    }
}

impl GameCameraPlugin {
    fn spawn_camera(mut commands: Commands) {
        //commands.spawn_bundle(UiCameraBundle::default());

        let mut camera = OrthographicCameraBundle::new_2d();

        // One unit in world space is one tile
        camera.orthographic_projection.left = -HEIGHT / TILE_SIZE / 2.0 * RESOLUTION;
        camera.orthographic_projection.right = HEIGHT / TILE_SIZE / 2.0 * RESOLUTION;
        camera.orthographic_projection.top = HEIGHT / TILE_SIZE / 2.0;
        camera.orthographic_projection.bottom = -HEIGHT / TILE_SIZE / 2.0;
        camera.orthographic_projection.scaling_mode = ScalingMode::None;

        commands.spawn_bundle(camera);
        info!("camera spawned")
    }

    fn camera_follow(
        mut follower_query: Query<(&mut Transform, &CameraFollower)>,
        camera_query: Query<&Transform, (With<Camera2d>, Without<CameraFollower>)>,
    ) {
        let camera_translation = camera_query.single().translation;
        for (mut transform, follow) in follower_query.iter_mut() {
            transform.translation.x = camera_translation.x;
            transform.translation.y = camera_translation.y;
            transform.translation.z = 800. + follow.offset;
        }
    }

    // fn camera_follows_player(
    //     player_query: Query<&Transform, With<Player>>,
    //     mut camera_query: Query<&mut Transform, (With<Camera2d>, Without<Player>)>,
    // ) {
    //     let player_transform = player_query.single().translation;
    //     let mut camera_transform = camera_query.single_mut();

    //     camera_transform.translation.x = player_transform.x;
    //     camera_transform.translation.y = player_transform.y;
    // }
}
