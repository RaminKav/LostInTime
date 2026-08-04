#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

// NOTE: Bindings must come before functions that use them!
#import bevy_sprite::mesh2d_functions
#import bevy_sprite::mesh2d_vertex_output::VertexOutput

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> opacity: f32; 

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var _MainTex: texture_2d<f32>; //("Base (RGB) Trans (A)", 2D) = "white" {}
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var _MainTexSampler: sampler; //("Base (RGB) Trans (A)", 2D) = "white" {}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let sampled = textureSample(_MainTex, _MainTexSampler, uv);
    let alpha = (sampled.a * opacity);
    let color = sampled.rgb * alpha;
    return vec4<f32>(color, alpha);
}