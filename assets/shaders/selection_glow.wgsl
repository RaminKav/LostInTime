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
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// Base gold sampled from the Selected slot art (linear 0..1).
@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> glow_color: vec4<f32>;
// Brighter yellow mixed in on shimmer highlights.
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> hot_color: vec4<f32>;
// params: x = time (seconds), y = shimmer speed, z = border thickness in texels (matches the
// slot art's own pixel resolution), w = base intensity (0..1)
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> params: vec4<f32>;
// shape_params: x/y = the slot's own art size in texels (1 texel == 1 game pixel here), z = how
// much larger this padded quad is than the slot (`GLOW_SCALE`), w = shimmer frame duration in
// seconds (low frame rate so it flickers like sprite art instead of smoothly interpolating).
@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var<uniform> shape_params: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(4)
var source_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5)
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

// Same lookup without implicit derivatives, so it can run inside the distance search below.
fn sample_alpha_level(uv: vec2<f32>) -> f32 {
    let inside = uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0;
    let c = textureSampleLevel(source_color_texture, source_texture_sampler, clamp(uv, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0)), 0.0);
    return select(0.0, c.a, inside);
}

// Distance from `uv` to the nearest opaque texel of the art, measured to the texel's square
// (not just 8 rays). Ray samples turn every stair-step on a rounded pixel corner into a spike
// once the border is more than a few texels thick; a disk search rounds those corners off.
fn nearest_edge_distance(uv: vec2<f32>, slot_size: vec2<f32>, max_r: f32) -> f32 {
    let frag = uv * slot_size;
    let base = vec2<i32>(floor(frag));
    var best = max_r + 1.0;
    // Fixed bound (the border never exceeds 8 texels) so the loop stays valid WGSL.
    for (var y = -8; y <= 8; y = y + 1) {
        for (var x = -8; x <= 8; x = x + 1) {
            let texel_index = base + vec2<i32>(x, y);
            let center = vec2<f32>(f32(texel_index.x), f32(texel_index.y)) + vec2<f32>(0.5, 0.5);
            let dist = length(max(abs(frag - center) - vec2<f32>(0.5, 0.5), vec2<f32>(0.0, 0.0)));
            if (dist > max_r || dist >= best) {
                continue;
            }
            if (sample_alpha_level(center / slot_size) > 0.01) {
                best = dist;
            }
        }
    }
    return best;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let slot_size = max(shape_params.xy, vec2<f32>(1.0, 1.0));
    let glow_scale = max(shape_params.z, 1.0);
    let frame_duration = max(shape_params.w, 0.001);

    // Map this padded quad's own uv (0..1 across the larger glow mesh) back into the slot
    // art's own uv space (0..1 across just the slot), so the alpha lookups below read the
    // slot's actual silhouette instead of the padded quad's bounding rectangle.
    let slot_uv = (uv - vec2<f32>(0.5, 0.5)) * glow_scale + vec2<f32>(0.5, 0.5);

    let center_alpha = sample_alpha(slot_uv);

    let num_layers = max(params.z, 1.0);
    let intensity = params.w;
    let shimmer_speed = params.y;
    // Snap time to a low frame rate so the shimmer plays like a hand-drawn sprite flicker
    // instead of continuously interpolating.
    let time = floor(params.x / frame_duration) * frame_duration;

    // Distance to the art's silhouette. Cubing the linear falloff makes the outer texels
    // drop off sharply instead of a constant per-texel decrease. Skip the search on the
    // opaque interior; that quad sits behind the sprite and would be hidden anyway.
    var layer_fade = 0.0;
    if (center_alpha <= 0.5) {
        let dist = nearest_edge_distance(slot_uv, slot_size, num_layers);
        if (dist <= num_layers) {
            let linear_fade = clamp(1.0 - dist / num_layers, 0.0, 1.0);
            layer_fade = linear_fade * linear_fade * linear_fade;
        }
    }

    // Smooth, low-frequency shimmer so neighboring border pixels' brightness correlates into
    // gentle waves instead of independent per-pixel static.
    let shimmer_uv = slot_uv * 5.0 + vec2<f32>(0.0, -time * shimmer_speed);
    let shimmer = fbm(shimmer_uv);

    let alpha = clamp(layer_fade * intensity * (0.5 + shimmer * 0.6), 0.0, 1.0);
    let color = mix(glow_color.rgb, hot_color.rgb, clamp(shimmer * 1.3 - 0.3, 0.0, 1.0));

    return vec4<f32>(color, alpha);
}
