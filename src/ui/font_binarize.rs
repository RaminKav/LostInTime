//! Binarize the alpha channel of every font atlas `Image` so glyphs render as crisp pixel art.
//!
//! ## Why
//! Bevy 0.10 rasterizes glyphs with `ab_glyph` from TTF outlines into RGBA8 atlas images and
//! produces **grayscale anti-aliased** coverage on glyph edges. For the pixel-style fonts in
//! `assets/fonts/` (`4x5`, `slkscr`, `alagard`, ...) those outlines don't always snap to integer
//! pixel boundaries at the design size, so edge texels come out with alpha `~200` instead of
//! `255`. The project enables [`bevy::prelude::ImagePlugin::default_nearest`] and the camera
//! upscales by an integer factor (e.g. 3× on 1080p), so each partial-alpha texel becomes an
//! `N×N` slightly-darker block next to fully-opaque blocks — the "some pixels of the text are
//! smaller/larger than they should be" symptom on low-DPI monitors.
//!
//! Verified on Windows 1920×1080 via the Phase 3 atlas dump in
//! [`crate::ui::fps_text::phase3_fps_atlas_dump`].
//!
//! ## What this does
//! After Bevy's text pipeline rasterizes / appends glyphs to a `FontAtlas` image, this system
//! discovers the atlas images via active `TextLayoutInfo` references, then snaps every pixel's
//! alpha to `0` or `255` using a `>= 128` threshold. The check is read-only when there is
//! nothing to flatten, so the steady-state cost is one `O(pixels)` scan per atlas per frame.
//!
//! ## Caveats
//! - All fonts in this project are pixel-style. If you ever add a smooth typeface (cursive
//!   menu title, etc.), binarizing it will alias hard — gate that font through a separate
//!   atlas or skip it here.

use bevy::asset::HandleId;
use bevy::prelude::*;
use bevy::sprite::TextureAtlas;
use bevy::text::TextLayoutInfo;
use bevy::utils::{HashMap, HashSet};

/// Alpha cutoff applied to every font atlas texel. `>=` keeps pixels with at least half coverage.
const ALPHA_THRESHOLD: u8 = 128;

/// Reads `TextLayoutInfo` of every text entity to find which images are currently used as font
/// atlases, then binarizes their alpha channel in-place when grayscale AA is present.
///
/// Runs every frame; reads cheaply when atlases are already binary (no `Assets<Image>::get_mut`
/// call ⇒ no `AssetEvent::Modified` ⇒ no GPU re-upload).
pub fn binarize_font_atlas_alpha(
    text_q: Query<&TextLayoutInfo>,
    atlases: Res<Assets<TextureAtlas>>,
    mut images: ResMut<Assets<Image>>,
    mut stats: Local<HashMap<HandleId, usize>>,
) {
    let mut font_image_handles: HashSet<Handle<Image>> = HashSet::new();
    for layout in text_q.iter() {
        for g in &layout.glyphs {
            let Some(atlas) = atlases.get(&g.atlas_info.texture_atlas) else {
                continue;
            };
            font_image_handles.insert(atlas.texture.clone_weak());
        }
    }

    for handle in font_image_handles.iter() {
        let needs_pass = match images.get(handle) {
            Some(img) => img
                .data
                .chunks_exact(4)
                .any(|p| p[3] != 0 && p[3] != 255),
            None => false,
        };
        if !needs_pass {
            continue;
        }

        let Some(image) = images.get_mut(handle) else {
            continue;
        };
        let mut converted = 0usize;
        for chunk in image.data.chunks_exact_mut(4) {
            let a = chunk[3];
            if a == 0 || a == 255 {
                continue;
            }
            chunk[3] = if a >= ALPHA_THRESHOLD { 255 } else { 0 };
            converted += 1;
        }
        let total = stats.entry(handle.id()).or_insert(0);
        *total += converted;
    }
}
