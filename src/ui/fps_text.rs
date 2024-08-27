use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::RenderLayers,
};

use crate::{ScreenResolution, GAME_HEIGHT};
const VERSION: &str = "v0.1.3-alpha";
#[derive(Component)]
pub struct FPSText;

pub fn spawn_fps_text(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
) {
    // DEBUG FPS
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("FPS: \n\n{VERSION}"),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
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
                translation: Vec3::new(
                    resolution.game_width / 2. - 28.5,
                    -GAME_HEIGHT / 2. + 10.5,
                    1.,
                ),
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
                text.sections[0].value = format!("FPS: {value:.0}\n\n{VERSION}");
            }
        }
    }
}
