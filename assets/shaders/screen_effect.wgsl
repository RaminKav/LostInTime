#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

// NOTE: Bindings must come before functions that use them!
#import bevy_sprite::mesh2d_functions

@group(1) @binding(0)
var<uniform> opacity: f32; 

@group(1) @binding(1)
var _MainTex: texture_2d<f32>; //("Base (RGB) Trans (A)", 2D) = "white" {}
@group(1) @binding(2)
var _MainTexSampler: sampler; //("Base (RGB) Trans (A)", 2D) = "white" {}


@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
    ) -> @location(0) vec4<f32>{
    let c = textureSample(_MainTex, _MainTexSampler, uv) * vec4<f32>(1.,1.,1.,1.);
    return vec4<f32>(c.rgb, c.a * opacity);
}