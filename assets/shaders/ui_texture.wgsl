#import bevy_sprite::mesh2d_vertex_output::VertexOutput
// Passthrough shader for the UI render texture overlay.
// The render target already uses Bgra8UnormSrgb, so the GPU handles
// gamma conversion automatically — no manual sRGB transform needed.

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var source_color_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var source_texture_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    return textureSample(source_color_texture, source_texture_sampler, uv);
}
