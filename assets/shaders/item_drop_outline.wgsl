// 1px outline for atlas sprites. Samples neighboring atlas texels; when the
// current pixel is transparent but a neighbor is opaque, draws `outline_color`.
// Neighbor samples are clamped to the sprite's own sub-rect (uv_bounds) so
// adjacent sprites in the sheet never bleed into the outline.
// All texture samples happen before branching (required by Naga uniformity rules).

@group(1) @binding(0)
var<uniform> uv_bounds: vec4<f32>; // (min_u, min_v, max_u, max_v)
@group(1) @binding(1)
var source_color_texture: texture_2d<f32>;
@group(1) @binding(2)
var source_texture_sampler: sampler;
@group(1) @binding(3)
var<uniform> outline_color: vec4<f32>;

fn sample_alpha_in_bounds(p: vec2<f32>) -> f32 {
    let a = textureSample(source_color_texture, source_texture_sampler, p).a;
    let inside = p.x >= uv_bounds.x && p.x <= uv_bounds.z
        && p.y >= uv_bounds.y && p.y <= uv_bounds.w;
    return a * select(0.0, 1.0, inside);
}

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(source_color_texture));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);

    let color = textureSample(source_color_texture, source_texture_sampler, uv);
    let left = sample_alpha_in_bounds(uv + vec2(-texel.x, 0.0));
    let right = sample_alpha_in_bounds(uv + vec2(texel.x, 0.0));
    let up = sample_alpha_in_bounds(uv + vec2(0.0, -texel.y));
    let down = sample_alpha_in_bounds(uv + vec2(0.0, texel.y));

    // The quad has a 1px margin ring outside the sprite rect; treat any base
    // pixel outside the sprite's own bounds as transparent so only the outline
    // shows there.
    let center_inside = uv.x >= uv_bounds.x && uv.x <= uv_bounds.z
        && uv.y >= uv_bounds.y && uv.y <= uv_bounds.w;
    let is_opaque = color.a > 0.01 && center_inside;
    let has_outline = left > 0.01 || right > 0.01 || up > 0.01 || down > 0.01;
    let transparent = vec4<f32>(0.0, 0.0, 0.0, 0.0);

    let outlined = select(transparent, outline_color, has_outline);
    return select(outlined, color, is_opaque);
}
