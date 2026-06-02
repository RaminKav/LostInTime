#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

// NOTE: Bindings must come before functions that use them!
#import bevy_sprite::mesh2d_functions

// params.x = center alpha, params.y = edge alpha, params.z = falloff exponent
@group(1) @binding(0)
var<uniform> params: vec4<f32>;
// rgb in .xyz (linear 0..1); alpha comes from the radial ramp only
@group(1) @binding(1)
var<uniform> overlay_color: vec4<f32>;

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
    ) -> @location(0) vec4<f32> {
    // Per-axis normalised position: 0 at centre, 1.0 at the screen mid-edges.
    let p = (uv - vec2<f32>(0.5, 0.5)) * 2.0;
    let dist = clamp(length(p), 0.0, 1.0);
    // Exponent < 1 keeps edge alpha across most of the radius; smaller = tighter centre.
    let falloff = max(params.z, 0.01);
    let t = smoothstep(0.0, 1.0, pow(dist, falloff));
    let alpha = mix(params.x, params.y, t);
    return vec4<f32>(overlay_color.rgb, alpha);
}
