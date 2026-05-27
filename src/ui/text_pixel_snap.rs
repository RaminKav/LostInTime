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
//! in every `TextLayoutInfo` on render layer 3:
//!
//! 1. **Per-text bulk X snap.** Compute one `alignment_offset.x` rounded to the physical pixel
//!    grid (the offset that [`bevy::sprite::Anchor`] applies to center / right-anchor a
//!    `Text2dBundle`). Apply the same delta to every glyph in the layout. Because every glyph
//!    receives the *same* horizontal shift, the per-glyph spacing produced by `glyph_brush_layout`
//!    is preserved exactly — only the rigid origin of the text moves onto an integer column.
//! 2. **Per-glyph Y snap.** Round each glyph quad's bottom edge to an integer physical pixel
//!    (glyphs of different heights / baselines need their own Y rounding).
//!
//! ### Why per-glyph X rounding is still avoided
//! Rounding each glyph's bottom-left **X** independently distorts horizontal spacing from
//! `glyph_brush_layout` — different glyphs pick up different nudges, so letter gaps shrink or
//! grow. On Windows (`scale_factor == 1`) that showed up as merged pairs in longer strings such
//! as "View heirlooms". This pass only snaps the *bulk* X (a single value applied uniformly).
//!
//! ### Why bulk X snap matters
//! With [`bevy::sprite::Anchor::Center`] (used by every heirloom card title and the
//! `Rerolls:` / `Banishes:` counters in [`crate::ui::skill_choice_ui`]) the alignment offset is
//! `layout.size * -0.5`. When the rendered text width is **odd in font pixels**, that offset is a
//! literal `n + 0.5` — a half-pixel in font units. At scale 3 (`1080p` Windows) that's
//! `0.5 * 3 / 1 = 1.5 physical pixels`, which is enough for `nearest` sampling to slice or
//! duplicate a single column of a glyph's atlas rect (the "U with a chipped top-right corner",
//! "E with a bump on the middle bar", "Tempered Soul" looking stretched). Sibling description
//! lines escape the artifact only because their widths happen to be even. Snapping the bulk X
//! once per text removes the content-dependent half-pixel without touching glyph spacing.
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

        // Bulk X snap: round the text-level alignment offset to the physical pixel grid and
        // apply the same delta to every glyph. Because every glyph receives the identical
        // horizontal shift, `glyph_brush_layout` spacing is preserved exactly — only the rigid
        // origin of the text moves onto an integer column. Fixes content-dependent half-pixel
        // artifacts (chipped `U`, bumpy `E` middle bar, "Tempered Soul" stretchy look) on every
        // `Anchor::Center` text. See module docs.
        let alignment_offset_phys_x = alignment_offset.x / scale_factor * phys_per_world;
        let bulk_dx_phys_x = alignment_offset_phys_x.round() - alignment_offset_phys_x;
        let bulk_dx_font_x = bulk_dx_phys_x * font_per_phys;

        for g in layout.glyphs.iter_mut() {
            let center_offset_font = alignment_offset + g.position;
            let half_size_font = g.size * 0.5;
            let corner_offset_font = center_offset_font - half_size_font;
            let corner_offset_phys = corner_offset_font / scale_factor * phys_per_world;

            // Per-glyph Y snap (each glyph has its own baseline-relative Y). See module docs
            // for why per-glyph X is *not* rounded.
            let delta_phys_y = corner_offset_phys.y.round() - corner_offset_phys.y;
            let delta_font_y = delta_phys_y * font_per_phys;

            if bulk_dx_font_x.abs() < 1e-4 && delta_font_y.abs() < 1e-4 {
                continue;
            }
            g.position.x += bulk_dx_font_x;
            g.position.y += delta_font_y;
        }
    }
}
