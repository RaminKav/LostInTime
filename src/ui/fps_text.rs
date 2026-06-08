use bevy::{
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    render::view::RenderLayers,
};

use bevy::sprite::TextureAtlas;
use bevy::text::TextLayoutInfo;

use crate::{ui::game_fonts as gf, ScreenResolution, UICamera, DEBUG};
const VERSION: &str = "v0.21.1";
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

/// Phase 2 (`DEBUG=1`): one-shot dump of per-glyph layout for the FPS text after Bevy lays it out.
///
/// For each glyph (in font-pixel space), logs:
/// - `glyph_pos_world`      : glyph quad center in world units (entity-local, post layout)
/// - `glyph_pos_viewport`   : projected to viewport (logical) pixels
/// - `glyph_pos_phys`       : projected to physical pixels (the framebuffer grid we sample)
/// - `glyph_pos_phys.fract` : how far off the physical pixel grid each glyph quad center is
/// - `atlas_rect_size`      : the atlas rect for this glyph (rasterized pixel size)
///
/// If `fract` is non-zero for most glyphs, the "some pixels of the text are smaller or larger
/// than they should be" symptom is sub-pixel atlas sampling. Re-runs only when scale/render
/// size changes; emits at most once per resolution change to avoid spam.
pub fn phase2_fps_text_layout_diag(
    res: Res<ScreenResolution>,
    mut last_key: Local<Option<(u32, u32, u32)>>,
    fps_q: Query<(&GlobalTransform, &TextLayoutInfo, &Text), With<FPSText>>,
    ui_cam: Query<(&Camera, &GlobalTransform), (With<UICamera>, Without<FPSText>)>,
    windows: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
) {
    if !*DEBUG {
        return;
    }
    let key = (res.scale, res.render_width, res.render_height);
    if last_key.as_ref() == Some(&key) {
        return;
    }
    let Ok((fps_gt, layout, text)) = fps_q.get_single() else {
        return;
    };
    let Ok((cam, cam_gt)) = ui_cam.get_single() else {
        return;
    };
    let Ok(window) = windows.get_single() else {
        return;
    };
    if layout.glyphs.is_empty() {
        return;
    }

    let scale_factor = window.resolution.scale_factor() as f32;
    let phys_per_world = res.scale as f32;
    let entity_world = fps_gt.translation();
    info!(
        "Phase2 FPS layout: entity_world=({:.4},{:.4}) entity_world.fract*scale=({:.4},{:.4}) layout_size=({:.4},{:.4}) glyphs={} scale_factor={:.3} phys_per_world={}",
        entity_world.x,
        entity_world.y,
        (entity_world.x * phys_per_world).fract(),
        (entity_world.y * phys_per_world).fract(),
        layout.size.x,
        layout.size.y,
        layout.glyphs.len(),
        scale_factor,
        res.scale,
    );

    for (i, g) in layout.glyphs.iter().enumerate().take(8) {
        // PositionedGlyph.position is in *font pixel space* (i.e. pre `scale_factor.recip()`),
        // see bevy_text-0.10.1/src/text2d.rs extract step.
        // World-space glyph translation = entity_world + position * (1 / scale_factor).
        let pos_world = Vec3::new(
            entity_world.x + g.position.x / scale_factor,
            entity_world.y + g.position.y / scale_factor,
            entity_world.z,
        );
        let viewport = cam.world_to_viewport(cam_gt, pos_world);
        let phys = viewport.map(|v| v * scale_factor);
        info!(
            "Phase2 FPS glyph[{i}] section={} char_idx={} pos_font_px=({:.4},{:.4}) pos_world=({:.4},{:.4}) viewport=({:?}) phys=({:?}) phys.fract=({:?}) atlas_size=({:?})",
            g.section_index,
            i,
            g.position.x,
            g.position.y,
            pos_world.x,
            pos_world.y,
            viewport.map(|v| (format!("{:.4}", v.x), format!("{:.4}", v.y))),
            phys.map(|p| (format!("{:.4}", p.x), format!("{:.4}", p.y))),
            phys.map(|p| (format!("{:.4}", p.x.fract()), format!("{:.4}", p.y.fract()))),
            text.sections.get(g.section_index).map(|_| (g.size.x, g.size.y)),
        );
    }
    *last_key = Some(key);
}

/// Phase 3 (`DEBUG=1`): ASCII-dump the alpha channel of each glyph's atlas rect for the FPS text.
///
/// Each row is one atlas-pixel row. Glyphs are RGBA8 with alpha encoding glyph coverage.
/// Legend:
/// - `#`  = alpha 255 (fully covered, hard pixel)
/// - `o`  = alpha 192–254
/// - `=`  = alpha 128–191
/// - `:`  = alpha  64–127
/// - `.`  = alpha   1– 63
/// - ` `  = alpha 0
///
/// If most pixels are `#` or space, the rasterizer (`ab_glyph`) produced a clean binary glyph
/// and the pixel-wobble must be elsewhere. If you see lots of `o`/`=`/`:`/`.`, the artifact is
/// grayscale anti-aliased edges, and you should switch to a different font (or pre-baked atlas)
/// for that style.
pub fn phase3_fps_atlas_dump(
    mut last_key: Local<Option<(u32, u32, u32)>>,
    res: Res<ScreenResolution>,
    fps_q: Query<&TextLayoutInfo, With<FPSText>>,
    atlases: Res<Assets<TextureAtlas>>,
    images: Res<Assets<Image>>,
) {
    if !*DEBUG {
        return;
    }
    let key = (res.scale, res.render_width, res.render_height);
    if last_key.as_ref() == Some(&key) {
        return;
    }
    let Ok(layout) = fps_q.get_single() else {
        return;
    };
    if layout.glyphs.is_empty() {
        return;
    }

    for (i, g) in layout.glyphs.iter().enumerate().take(8) {
        let Some(atlas) = atlases.get(&g.atlas_info.texture_atlas) else {
            continue;
        };
        let Some(image) = images.get(&atlas.texture) else {
            continue;
        };
        let rect = atlas.textures[g.atlas_info.glyph_index];
        let img_w = image.texture_descriptor.size.width as usize;
        let img_h = image.texture_descriptor.size.height as usize;
        let bytes_per_pixel = 4;
        let x0 = rect.min.x as usize;
        let y0 = rect.min.y as usize;
        let w = (rect.max.x - rect.min.x) as usize;
        let h = (rect.max.y - rect.min.y) as usize;

        info!(
            "Phase3 FPS atlas[{i}]: atlas_image_size=({img_w}x{img_h}) rect_origin=({x0},{y0}) rect_size=({w}x{h})",
        );
        let mut rows = String::new();
        for row in 0..h {
            let y = y0 + row;
            let mut line = String::with_capacity(w);
            for col in 0..w {
                let x = x0 + col;
                if x >= img_w || y >= img_h {
                    line.push('?');
                    continue;
                }
                let pixel_idx = (y * img_w + x) * bytes_per_pixel;
                let alpha = image.data.get(pixel_idx + 3).copied().unwrap_or(0);
                let ch = match alpha {
                    0 => ' ',
                    1..=63 => '.',
                    64..=127 => ':',
                    128..=191 => '=',
                    192..=254 => 'o',
                    255 => '#',
                };
                line.push(ch);
            }
            rows.push('\n');
            rows.push('|');
            rows.push_str(&line);
            rows.push('|');
        }
        info!("Phase3 FPS atlas[{i}] alpha grid:{rows}");
    }
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
