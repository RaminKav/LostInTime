// Passthrough shader for the UI render texture overlay.
// The render target already uses Bgra8UnormSrgb, so the GPU handles
// gamma conversion automatically — no manual sRGB transform needed.

@group(1) @binding(0)
var source_color_texture: texture_2d<f32>;
@group(1) @binding(1)
var source_texture_sampler: sampler;

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    return textureSample(source_color_texture, source_texture_sampler, uv);
}
