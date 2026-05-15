//! Snap each `Text2dBundle` glyph quad to the physical pixel grid.
//!
//! ## Why
//! Bevy 0.10's `Text2dBundle` rasterizes glyphs into a `FontAtlas` and draws each glyph as its
//! own centered quad whose **size in world units equals the atlas rect size in font pixels**
//! (divided by `window.scale_factor`). On this project's pixel-perfect setup the orthographic
//! camera then upscales 1 world unit → `ScreenResolution::scale` physical pixels (typically
//! `3×` on 1920×1080, `4–6×` on retina at large window sizes).
//!
//! When an atlas dimension is **odd** (5×5 letters in `4x5.ttf`, `1×4` colon, `3×5` numerals,
//! …) and the integer render scale is **odd** (`3`), the quad's center lands on an integer
//! physical pixel but its edges fall on **half-pixel boundaries**. With nearest sampling that
//! makes one **row** of atlas texels span one fewer physical pixel than its neighbors — the
//! canonical "some pixels of the text are smaller/larger than they should be" symptom observed
//! on a 1920×1080 27" monitor (horizontal odd-width edge effects are rarer in practice).
//!
//! Confirmed by [`crate::ui::fps_text::phase2_fps_text_layout_diag`]: glyph[0] of `F` at world
//! Y=−164.6667 maps to phys Y=1034.0 (integer center) but the 5-pixel-tall atlas places its
//! edges at phys Y=1026.5 / 1041.5 (half-integer).
//!
//! ## What this does
//! After Bevy lays out the text (in `PostUpdate`, after `update_text2d_layout`), for every glyph
//! in every `TextLayoutInfo` on render layer 3, shift `position` so the glyph quad's bottom edge
//! lands on an integer physical **Y** relative to the parent (same coordinate space as
//! [`crate::ui::snap_layer3_visuals_to_pixel_grid`]).
//!
//! **Only Y is rounded.** Rounding each glyph's bottom-left **X** independently distorts
//! horizontal spacing from `glyph_brush_layout` — different glyphs pick up different nudges, so
//! letter gaps shrink or grow. On Windows (`scale_factor == 1`) that showed up as merged pairs in
//! longer strings such as "View heirlooms"; macOS (`scale_factor == 2`) masked it.
//!
//! ## Scope
//! Restricted to `RenderLayers::layer(3)` (the UI camera), which is also what
//! `snap_layer3_visuals_to_pixel_grid` snaps. Other render layers (world-space damage numbers,
//! etc.) are left alone — animating their glyphs onto a coarse pixel grid would make them
//! jitter during motion.

use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy::text::TextLayoutInfo;
use bevy::window::{PrimaryWindow, Window};

use crate::ScreenResolution;

/// Per-glyph pixel snap. See module docs.
pub fn pixel_snap_text_glyphs(
    res: Res<ScreenResolution>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut text_q: Query<(&Anchor, &mut TextLayoutInfo, &RenderLayers)>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };
    let scale_factor = window.resolution.scale_factor() as f32;
    let phys_per_world = res.scale.max(1) as f32;
    if scale_factor <= 0.0 || phys_per_world <= 0.0 {
        return;
    }
    let font_per_phys = scale_factor / phys_per_world;

    let layer3 = RenderLayers::layer(3);
    for (anchor, mut layout, layers) in text_q.iter_mut() {
        if !layers.intersects(&layer3) {
            continue;
        }
        if layout.glyphs.is_empty() {
            continue;
        }

        let text_anchor = -(anchor.as_vec() + 0.5);
        let alignment_offset = layout.size * text_anchor;

        for g in layout.glyphs.iter_mut() {
            let center_offset_font = alignment_offset + g.position;
            let half_size_font = g.size * 0.5;
            let corner_offset_font = center_offset_font - half_size_font;
            let corner_offset_phys = corner_offset_font / scale_factor * phys_per_world;

            // Y only — see module docs (preserve horizontal letter spacing).
            let target_phys = Vec2::new(corner_offset_phys.x, corner_offset_phys.y.round());
            let delta_phys = target_phys - corner_offset_phys;
            if delta_phys.y.abs() < 1e-4 {
                continue;
            }
            g.position += delta_phys * font_per_phys;
        }
    }
}
