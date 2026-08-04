//! Linear filtering for font atlases that are drawn at fractional physical texel sizes.
//!
//! The app uses [`bevy::prelude::ImagePlugin::default_nearest`] for pixel-art UI sprites.
//! Font atlases inherit that default. When `Transform.scale × UI_scale / window.scale_factor`
//! is non-integer (e.g. BODY `0.5` on Large UI `×3` → `1.5`), nearest sampling deforms
//! glyphs. Linear soft-scales those atlases instead.
//!
//! Scale-1.0 titles (main-menu "Enter", etc.) keep nearest: they use a separate atlas size
//! ([`crate::ui::game_fonts::ALAGARD_DISPLAY_SIZE`]) from scaled-down roles
//! ([`crate::ui::game_fonts::ALAGARD_SCALED_ATLAS_SIZE`]), and this system only switches an
//! atlas to linear when a text entity with a fractional mapping actually uses it.

use bevy::asset::AssetId;
use bevy::image::{ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::window::{PrimaryWindow, Window};

use crate::ScreenResolution;

fn sampler_is_linear(sampler: &ImageSampler) -> bool {
    match sampler {
        ImageSampler::Default => false,
        ImageSampler::Descriptor(desc) => {
            desc.mag_filter == ImageFilterMode::Linear && desc.min_filter == ImageFilterMode::Linear
        }
    }
}

fn sampler_is_nearest_like(sampler: &ImageSampler) -> bool {
    match sampler {
        ImageSampler::Default => true,
        ImageSampler::Descriptor(desc) => {
            desc.mag_filter == ImageFilterMode::Nearest
                && desc.min_filter == ImageFilterMode::Nearest
        }
    }
}

/// `phys_per_atlas_px = entity_uniform_scale × UI_scale / window.scale_factor`.
/// When this is (near) an integer, nearest sampling stays crisp.
fn needs_linear_sampling(entity_scale: f32, ui_scale: f32, scale_factor: f32) -> bool {
    if scale_factor <= 0.0 || ui_scale <= 0.0 || entity_scale <= 0.0 {
        return false;
    }
    let phys_per_atlas = entity_scale * ui_scale / scale_factor;
    let nearest = phys_per_atlas.round();
    (phys_per_atlas - nearest).abs() > 0.02
}

/// Discover font atlas images used by fractionally scaled text and set them to linear.
/// Atlases only used by integer-mapped (typically scale-1.0) text stay nearest.
pub fn ensure_font_atlas_linear_sampling(
    res: Res<ScreenResolution>,
    windows: Query<&Window, With<PrimaryWindow>>,
    text_q: Query<(&Transform, &TextLayoutInfo)>,
    mut images: ResMut<Assets<Image>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let scale_factor = window.resolution.scale_factor() as f32;
    let ui_scale = res.scale.max(1) as f32;

    let mut linear_handles: HashSet<AssetId<Image>> = HashSet::new();
    for (transform, layout) in text_q.iter() {
        // Text styles use uniform Transform.scale from FontStyle.
        let entity_scale = transform.scale.x;
        if !needs_linear_sampling(entity_scale, ui_scale, scale_factor) {
            continue;
        }
        for glyph in &layout.glyphs {
            linear_handles.insert(glyph.atlas_info.texture);
        }
    }

    for handle in linear_handles {
        let Some(image) = images.get(handle) else {
            continue;
        };
        if sampler_is_linear(&image.sampler) {
            continue;
        }
        // Only override default/nearest; never fight an explicit non-nearest custom sampler.
        if !sampler_is_nearest_like(&image.sampler) {
            continue;
        }
        let Some(mut image) = images.get_mut(handle) else {
            continue;
        };
        image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor::linear());
    }
}
