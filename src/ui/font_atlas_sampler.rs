//! Force linear filtering on Bevy font atlas images only.
//!
//! The app uses [`bevy::prelude::ImagePlugin::default_nearest`] for pixel-art UI sprites.
//! Font atlases are created with [`ImageSampler::Default`], so they inherit nearest and
//! deform when drawn at fractional physical sizes (e.g. Alagard `Transform.scale` 0.5 on
//! odd UI scales). Overriding atlas samplers to linear soft-scales text without touching
//! sprite assets.

use bevy::prelude::*;
use bevy::render::texture::ImageSampler;
use bevy::sprite::TextureAtlas;
use bevy::text::TextLayoutInfo;
use bevy::utils::HashSet;
use bevy::render::render_resource::FilterMode;

fn sampler_is_linear(sampler: &ImageSampler) -> bool {
    match sampler {
        ImageSampler::Default => false,
        ImageSampler::Descriptor(desc) => {
            desc.mag_filter == FilterMode::Linear && desc.min_filter == FilterMode::Linear
        }
    }
}

/// Discover font atlas images from active text layouts and set their samplers to linear.
pub fn ensure_font_atlas_linear_sampling(
    text_q: Query<&TextLayoutInfo>,
    atlases: Res<Assets<TextureAtlas>>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut font_image_handles: HashSet<Handle<Image>> = HashSet::new();
    for layout in text_q.iter() {
        for glyph in &layout.glyphs {
            let Some(atlas) = atlases.get(&glyph.atlas_info.texture_atlas) else {
                continue;
            };
            font_image_handles.insert(atlas.texture.clone_weak());
        }
    }

    for handle in font_image_handles {
        let Some(image) = images.get(&handle) else {
            continue;
        };
        if sampler_is_linear(&image.sampler_descriptor) {
            continue;
        }
        let Some(image) = images.get_mut(&handle) else {
            continue;
        };
        image.sampler_descriptor = ImageSampler::linear();
    }
}
