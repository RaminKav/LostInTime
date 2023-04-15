// #import bevy_sprite::mesh2d_view_bindings
// #import bevy_pbr::utils

@group(1) @binding(0)
var source_color_texture: texture_2d<f32>;
@group(1) @binding(1)
var source_texture_sampler: sampler;
@group(1) @binding(2)
var lookup_color_texture: texture_2d<f32>;
@group(1) @binding(3)
var lookup_texture_sampler: sampler;
@group(1) @binding(4)
var<uniform> flip: f32;
@group(1) @binding(5)
var<uniform> opacity: f32;


@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    let uv_map_dims = vec2<f32>(textureDimensions(source_color_texture));
    let palette_dims = vec2<f32>(textureDimensions(lookup_color_texture));
    var flipped_uv: vec2<f32>;
    if (flip > 0.5){
        flipped_uv = vec2<f32>(1. - uv.x, uv.y);
    } else {
        flipped_uv = uv;
    }
    let uv_map_uv = flipped_uv - (flipped_uv % (1f/32f)) + (1f / 64f);

    let uv_map = textureSample(
        source_color_texture,
        source_texture_sampler,
        uv_map_uv
    );

    // S = sRGB, L = Linear 
    // ((S+0.055)/1.055)^2.4
    // 1.055×L^1/2.4 − 0.055
    var u: f32;
    var v: f32;
    if (uv_map.r<= 0.00313)
    {
	     u = uv_map.r * 12.92;
    } else {
     u = (1.055 * pow(uv_map.r, 1./2.4)) - 0.055;
    }
    if (uv_map.g<= 0.00313)
    {
	     v = uv_map.g * 12.92;
    } else {
     v = (1.055 * pow(uv_map.g, 1./2.4)) - 0.055;
    }
    var a = uv_map.a;
    if uv_map.a > 0. {
        a = opacity;
    }
 
    let palette_uv = vec2<f32>(u*255f + 0.5, v*255f + 0.5) / palette_dims;


    let color = textureSample(
        lookup_color_texture,
        lookup_texture_sampler,
        palette_uv  
    );

    return vec4<f32>(color.rgb,a);
}
