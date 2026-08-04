#import bevy_sprite::mesh2d_vertex_output::VertexOutput
// Outline / shadow for atlas sprites and standalone UI images. Samples
// neighboring texels; when the current pixel is transparent but a neighbor is
// opaque, draws `outline_color`. Neighbor samples are clamped to the sprite's
// own sub-rect (uv_bounds) so adjacent sprites in the sheet never bleed.
//
// `ring_params` = (ring_count, falloff, shadow_only, _):
//   ring_count : number of outline rings (1..=4). Ring k sits k texels from the
//                silhouette and draws at alpha = falloff^(k-1) of the base alpha.
//                falloff 0.5 => 100%, 50%, 25%, ... per successive ring.
//   falloff    : per-ring alpha multiplier (ignored when ring_count == 1).
//   shadow_only: 1.0 => interior (opaque) pixels are discarded so only the rings
//                render (used by non-destructive child shadows behind panels).
//                0.0 => interior passes through (floor drops, cursor, icons).
//
// All texture samples happen before branching (required by Naga uniformity rules).

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> uv_bounds: vec4<f32>; // (min_u, min_v, max_u, max_v)
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var source_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var source_texture_sampler: sampler;
@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var<uniform> outline_color: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4)
var<uniform> ring_params: vec4<f32>;

fn sample_alpha_in_bounds(p: vec2<f32>) -> f32 {
    let a = textureSample(source_color_texture, source_texture_sampler, p).a;
    let inside = p.x >= uv_bounds.x && p.x <= uv_bounds.z
        && p.y >= uv_bounds.y && p.y <= uv_bounds.w;
    return a * select(0.0, 1.0, inside);
}

// Max alpha among the 4 orthogonal neighbors at distance `k` texels.
fn ring_alpha(uv: vec2<f32>, texel: vec2<f32>, k: f32) -> f32 {
    let l = sample_alpha_in_bounds(uv + vec2(-k * texel.x, 0.0));
    let r = sample_alpha_in_bounds(uv + vec2(k * texel.x, 0.0));
    let u = sample_alpha_in_bounds(uv + vec2(0.0, -k * texel.y));
    let d = sample_alpha_in_bounds(uv + vec2(0.0, k * texel.y));
    return max(max(l, r), max(u, d));
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let dims = vec2<f32>(textureDimensions(source_color_texture));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);

    let ring_count = ring_params.x;
    let falloff = ring_params.y;
    let shadow_only = ring_params.z;

    let color = textureSample(source_color_texture, source_texture_sampler, uv);

    // Sample up to 4 rings unconditionally (uniform control flow).
    var hits = array<f32, 4>(
        ring_alpha(uv, texel, 1.0),
        ring_alpha(uv, texel, 2.0),
        ring_alpha(uv, texel, 3.0),
        ring_alpha(uv, texel, 4.0),
    );

    let center_inside = uv.x >= uv_bounds.x && uv.x <= uv_bounds.z
        && uv.y >= uv_bounds.y && uv.y <= uv_bounds.w;
    let is_opaque = color.a > 0.01 && center_inside;

    // Smallest ring index with an opaque neighbor wins (strongest, nearest silhouette).
    var factor = 0.0;
    for (var i = 4; i >= 1; i = i - 1) {
        let fi = f32(i);
        if (fi <= ring_count && hits[i - 1] > 0.01) {
            factor = pow(falloff, fi - 1.0);
        }
    }

    let outlined = vec4<f32>(outline_color.rgb, outline_color.a * factor);
    let interior = select(color, vec4<f32>(0.0, 0.0, 0.0, 0.0), shadow_only > 0.5);
    return select(outlined, interior, is_opaque);
}
