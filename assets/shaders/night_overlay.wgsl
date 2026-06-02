#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

// NOTE: Bindings must come before functions that use them!
#import bevy_sprite::mesh2d_functions

// x = intensity (overall darkness 0..1), y = bubble radius (uv units),
// z = bubble softness (0..1), w = edge darkening boost (0..1)
@group(1) @binding(0)
var<uniform> params: vec4<f32>;
// Tint rgb in .xyz (linear 0..1); .w = saturation boost (1 = none, >1 = more vivid hue).
@group(1) @binding(1)
var<uniform> tint: vec4<f32>;
// Player position in quad-uv space in .xy, aspect ratio (w/h) in .z, .w unused.
@group(1) @binding(2)
var<uniform> player_uv: vec4<f32>;

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
    ) -> @location(0) vec4<f32> {
    let intensity = params.x;
    let radius = max(params.y, 0.001);
    let softness = clamp(params.z, 0.001, 1.0);
    let edge_boost = params.w;

    // Aspect-correct the horizontal axis so the player bubble stays circular.
    var delta = uv - player_uv.xy;
    delta.x = delta.x * player_uv.z;
    let d = length(delta) / radius;

    // 0 inside the bubble (clear), ramping to 1 outside it.
    let bubble = smoothstep(1.0 - softness, 1.0, d);

    // Subtle extra darkening toward the screen edges (vignette).
    let edge = smoothstep(0.35, 1.0, length((uv - vec2<f32>(0.5)) * 2.0));

    let alpha = intensity * mix(bubble, 1.0, edge * edge_boost);

    // Apply authorship saturation (< 1 desaturates, > 1 exaggerates hue).
    let lum = dot(tint.rgb, vec3<f32>(0.299, 0.587, 0.114));
    let sat = tint.w;
    let hue = mix(vec3<f32>(lum), tint.rgb, sat);

    return vec4<f32>(hue, clamp(alpha, 0.0, 1.0));
}
