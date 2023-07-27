use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::RenderLayers,
};

use crate::{GAME_HEIGHT, GAME_WIDTH};

#[derive(Component)]
pub struct FPSText;

pub fn spawn_fps_text(mut commands: Commands, asset_server: Res<AssetServer>) {
    // DEBUG FPS
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "FPS: ",
                TextStyle {
                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                    font_size: 8.0,
                    color: Color::Rgba {
                        red: 75. / 255.,
                        green: 61. / 255.,
                        blue: 68. / 255.,
                        alpha: 1.,
                    },
                },
            )
            .with_alignment(TextAlignment::Right),
            transform: Transform {
                translation: Vec3::new(GAME_WIDTH / 2. - 10., -GAME_HEIGHT / 2. + 10., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("FPS TEXT"),
        FPSText,
        RenderLayers::from_layers(&[3]),
    ));
}
pub fn text_update_system(
    diagnostics: Res<Diagnostics>,
    mut query: Query<&mut Text, With<FPSText>>,
) {
    for mut text in &mut query {
        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(value) = fps.smoothed() {
                // Update the value of the second section
                text.sections[0].value = format!("{value:.2}");
            }
        }
    }
}
