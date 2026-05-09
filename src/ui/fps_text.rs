use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::RenderLayers,
};

use crate::{
    ui::game_fonts as gf,
    ScreenResolution, UICamera, DEBUG,
};
const VERSION: &str = "v0.18.0";
#[derive(Component)]
pub struct FPSText;

/// Snap UI world (`OrthographicProjection` + integer `ScreenResolution::scale`) to the physical
/// pixel grid: one world unit spans `scale` framebuffer pixels on both axes.
fn snap_world_xy_to_pixel_grid(xy: Vec2, scale: u32) -> Vec2 {
    let s = scale as f32;
    Vec2::new((xy.x * s).round() / s, (xy.y * s).round() / s)
}

pub fn spawn_fps_text(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
) {
    let raw = Vec2::new(
        resolution.game_width / 2. - 28.5,
        -resolution.game_height / 2. + 10.5,
    );
    let snapped = snap_world_xy_to_pixel_grid(raw, resolution.scale);

    // DEBUG FPS
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("FPS: \n\n{VERSION}"),
                TextStyle {
                    font: gf::HUD_FPS_DEBUG.load_font(asset_server.as_ref()),
                    font_size: gf::HUD_FPS_DEBUG.size,
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
                translation: Vec3::new(snapped.x, snapped.y, 1.),
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
/// Phase 1: logs FPS label world position and logical viewport coords (`DEBUG=1`).
/// Re-runs when `scale`/render size changes (window resize). Uses UI camera `world_to_viewport`
/// so fractional viewport values indicate subpixel placement (soft glyph hypothesis).
pub fn phase1_fps_text_viewport_diag(
    res: Res<ScreenResolution>,
    mut last_key: Local<Option<(u32, u32, u32)>>,
    fps_q: Query<&GlobalTransform, With<FPSText>>,
    ui_cam: Query<(&Camera, &GlobalTransform), With<UICamera>>,
) {
    if !*DEBUG {
        return;
    }
    let key = (res.scale, res.render_width, res.render_height);
    if last_key.as_ref() == Some(&key) {
        return;
    }
    let Ok(fps_gt) = fps_q.get_single() else {
        return;
    };
    let Ok((cam, cam_gt)) = ui_cam.get_single() else {
        return;
    };
    let world = fps_gt.translation();
    let Some(vp) = cam.world_to_viewport(cam_gt, world) else {
        return;
    };
    info!(
        "Phase1 HUD/FPS: world_xy=({:.4},{:.4}) viewport_logical_xy=({:.4},{:.4}) viewport_fract_xy=({:.4},{:.4}) scale={}",
        world.x,
        world.y,
        vp.x,
        vp.y,
        vp.x.fract(),
        vp.y.fract(),
        res.scale,
    );
    *last_key = Some(key);
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
