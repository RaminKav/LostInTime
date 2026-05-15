//! Glyph-atlas-at-physical-pixel-size helpers for `Text2dBundle`.
//!
//! **The problem.** In Bevy 0.10, `update_text2d_layout` rasterizes glyph atlases at exactly
//! `TextStyle::font_size` pixels (no window scale-factor multiplier). The resulting quads are
//! drawn in *world* units, then the orthographic camera scales 1 world unit → `scale` physical
//! pixels (here, `scale = render_height / game_height`, e.g. **3×** on a 1920×1080 monitor with
//! `target_height=360`). Because the project enables [`bevy::prelude::ImagePlugin::default_nearest`],
//! font glyph atlases inherit nearest-neighbor sampling — and each grayscale AA pixel produced by
//! `ab_glyph` becomes a `scale × scale` block on screen. On a low-DPI monitor this is the classic
//! "some pixels of the text are smaller/larger than they should be" look.
//!
//! **The fix.** Rasterize the glyph atlas at the on-screen pixel size, then shrink the entity's
//! `Transform::scale` by `1/scale` so the world footprint is unchanged. The AA shading is now
//! computed at display resolution, and pixel fonts (`4x5`, `slkscr`) line up with the integer
//! render scale.
//!
//! **Usage.**
//! ```ignore
//! let scale = resolution.scale;
//! commands.spawn((
//!     Text2dBundle {
//!         text: Text::from_section("hello", scaled_text_style(&asset_server, gf::HUD_PRIMARY, color, scale)),
//!         transform: Transform::from_translation(pos).with_scale(text_world_scale_vec(scale)),
//!         ..default()
//!     },
//!     ScaledText2d::from(gf::HUD_PRIMARY),
//! ));
//! ```
//! The [`sync_scaled_text2d_on_scale_change`] system keeps things consistent on window resize.

use bevy::prelude::*;

use crate::ui::game_fonts::FontStyle;
use crate::ScreenResolution;

/// Marker for `Text2dBundle` entities that have been pre-scaled by [`scaled_text_style`] +
/// [`text_world_scale_vec`]. Stores the **nominal world-space** font size so the sync system
/// can re-derive `font_size = nominal * scale` and `Transform::scale = 1/scale` when the
/// integer render scale changes (window resize, monitor swap).
#[derive(Component, Clone, Copy, Debug)]
pub struct ScaledText2d {
    pub nominal_size: f32,
}

impl ScaledText2d {
    #[inline]
    pub fn new(nominal_size: f32) -> Self {
        Self { nominal_size }
    }
}

impl From<FontStyle> for ScaledText2d {
    #[inline]
    fn from(style: FontStyle) -> Self {
        Self::new(style.size)
    }
}

/// Physical-pixel font size to request from the rasterizer for `nominal` world-space size at the
/// current integer render `scale`.
#[inline]
pub fn effective_font_size(nominal: f32, scale: u32) -> f32 {
    nominal * scale.max(1) as f32
}

/// Reciprocal of the integer render scale, applied to `Transform::scale.{x,y}` to keep the world
/// footprint identical after upscaling `font_size`.
#[inline]
pub fn text_world_xy_scale(scale: u32) -> f32 {
    1.0 / scale.max(1) as f32
}

#[inline]
pub fn text_world_scale_vec(scale: u32) -> Vec3 {
    let s = text_world_xy_scale(scale);
    Vec3::new(s, s, 1.0)
}

/// Build a `TextStyle` whose `font_size` is the on-screen physical pixel size at `scale`.
/// Pair with [`ScaledText2d`] and [`text_world_scale_vec`] on the same entity.
pub fn scaled_text_style(
    asset_server: &AssetServer,
    font_style: FontStyle,
    color: Color,
    scale: u32,
) -> TextStyle {
    TextStyle {
        font: font_style.load_font(asset_server),
        font_size: effective_font_size(font_style.size, scale),
        color,
    }
}

/// Re-apply `font_size` and `Transform::scale` to every [`ScaledText2d`] when the render scale
/// changes (window resize, monitor swap, projection rebuild in
/// [`crate::main::update_pixel_perfect_viewport`]).
pub fn sync_scaled_text2d_on_scale_change(
    resolution: Res<ScreenResolution>,
    mut last_scale: Local<u32>,
    mut q: Query<(&ScaledText2d, &mut Text, &mut Transform)>,
) {
    let scale = resolution.scale.max(1);
    if *last_scale == scale {
        return;
    }
    *last_scale = scale;

    let scale_f = scale as f32;
    let inv = 1.0 / scale_f;
    for (marker, mut text, mut transform) in q.iter_mut() {
        let effective = marker.nominal_size * scale_f;
        for section in text.sections.iter_mut() {
            section.style.font_size = effective;
        }
        transform.scale.x = inv;
        transform.scale.y = inv;
    }
}
