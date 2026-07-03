// Pixelated glow border for "Selected" UI slots (class/pet select screens). Renders as a
// non-destructive child quad *behind* the slot art (see `SelectionGlow` in
// `src/ui/selection_glow.rs`), sized only a little larger than the slot.
//
// Unlike a shape-based border, this samples the slot's own "Selected" art alpha channel to find
// how far each padded-quad pixel is from the nearest *opaque* pixel of that art — so the border
// hugs the actual drawn silhouette (including its rounded corners and top notch) and never
// wraps around the fully-transparent corners of the sprite's bounding rectangle.
#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

#import bevy_sprite::mesh2d_functions

// Base purple sampled from the Selected slot art (linear 0..1).
@group(1) @binding(0)
var<uniform> glow_color: vec4<f32>;
// Brighter purple mixed in on shimmer highlights.
@group(1) @binding(1)
var<uniform> hot_color: vec4<f32>;
// params: x = time (seconds), y = shimmer speed, z = border thickness in texels (matches the
// slot art's own pixel resolution), w = base intensity (0..1)
@group(1) @binding(2)
var<uniform> params: vec4<f32>;
// shape_params: x/y = the slot's own art size in texels (1 texel == 1 game pixel here), z = how
// much larger this padded quad is than the slot (`GLOW_SCALE`), w = shimmer frame duration in
// seconds (low frame rate so it flickers like sprite art instead of smoothly interpolating).
@group(1) @binding(3)
var<uniform> shape_params: vec4<f32>;
@group(1) @binding(4)
var source_color_texture: texture_2d<f32>;
@group(1) @binding(5)
var source_texture_sampler: sampler;

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Two octaves of value noise for a smooth, slowly drifting shimmer rather than independent
// per-pixel randomness (which reads as TV static instead of a glow).
fn fbm(p: vec2<f32>) -> f32 {
    var total = 0.0;
    var amp = 0.5;
    var q = p;
    for (var i = 0; i < 2; i = i + 1) {
        total = total + value_noise(q) * amp;
        q = q * 2.0;
        amp = amp * 0.5;
    }
    return total;
}

// Alpha of the slot's own art at a given uv, treating anywhere outside the [0,1] art bounds as
// fully transparent (so the border can extend past the art's edges without wrapping/repeating).
fn sample_alpha(uv: vec2<f32>) -> f32 {
    let inside = uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0;
    let c = textureSample(source_color_texture, source_texture_sampler, clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0)));
    return select(0.0, c.a, inside);
}

// Brightest alpha among 8 neighbors at `k` texels away. Sampling all 8 unconditionally (rather
// than branching out early) keeps texture-sample derivatives uniform across the quad.
fn ring_alpha(uv: vec2<f32>, texel: vec2<f32>, k: f32) -> f32 {
    let diag = 0.7071 * k;
    var best = 0.0;
    best = max(best, sample_alpha(uv + vec2<f32>(k, 0.0) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(-k, 0.0) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(0.0, k) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(0.0, -k) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(diag, diag) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(-diag, diag) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(diag, -diag) * texel));
    best = max(best, sample_alpha(uv + vec2<f32>(-diag, -diag) * texel));
    return best;
}

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    let slot_size = max(shape_params.xy, vec2<f32>(1.0, 1.0));
    let glow_scale = max(shape_params.z, 1.0);
    let frame_duration = max(shape_params.w, 0.001);

    // Map this padded quad's own uv (0..1 across the larger glow mesh) back into the slot
    // art's own uv space (0..1 across just the slot), so the alpha lookups below read the
    // slot's actual silhouette instead of the padded quad's bounding rectangle.
    let slot_uv = (uv - vec2<f32>(0.5, 0.5)) * glow_scale + vec2<f32>(0.5, 0.5);
    let texel = 1.0 / slot_size;

    let center_alpha = sample_alpha(slot_uv);

    let num_layers = max(params.z, 1.0);
    let intensity = params.w;
    let shimmer_speed = params.y;
    // Snap time to a low frame rate so the shimmer plays like a hand-drawn sprite flicker
    // instead of continuously interpolating.
    let time = floor(params.x / frame_duration) * frame_duration;

    // Sample up to 4 rings unconditionally (uniform control flow), same pattern as the item
    // drop-shadow shader. Ring `k` sits `k` texels from the art's silhouette.
    var hits = array<f32, 4>(
        ring_alpha(slot_uv, texel, 1.0),
        ring_alpha(slot_uv, texel, 2.0),
        ring_alpha(slot_uv, texel, 3.0),
        ring_alpha(slot_uv, texel, 4.0),
    );

    // Smallest ring index with an opaque neighbor wins (nearest silhouette pixel), fading
    // out to 0 by `num_layers` texels away. Cubing the linear falloff makes the outer layers
    // drop off sharply instead of a constant per-layer decrease.
    var layer_fade = 0.0;
    for (var i = 4; i >= 1; i = i - 1) {
        let fi = f32(i);
        if (fi <= num_layers && hits[i - 1] > 0.01) {
            let linear_fade = clamp(1.0 - (fi - 1.0) / num_layers, 0.0, 1.0);
            layer_fade = linear_fade * linear_fade * linear_fade;
        }
    }

    // Smooth, low-frequency shimmer so neighboring border pixels' brightness correlates into
    // gentle waves instead of independent per-pixel static.
    let shimmer_uv = slot_uv * 5.0 + vec2<f32>(0.0, -time * shimmer_speed);
    let shimmer = fbm(shimmer_uv);

    // Never draw over the art's own opaque interior (it's hidden behind the real sprite anyway
    // since this quad sits behind it, but skip it so nothing leaks past rounding/size mismatches).
    let shape_alpha = select(layer_fade, 0.0, center_alpha > 0.5);

    let alpha = clamp(shape_alpha * intensity * (0.5 + shimmer * 0.6), 0.0, 1.0);
    let color = mix(glow_color.rgb, hot_color.rgb, clamp(shimmer * 1.3 - 0.3, 0.0, 1.0));

    return vec4<f32>(color, alpha);
}
