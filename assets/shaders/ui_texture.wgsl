// #import bevy_sprite::mesh2d_view_bindings
// #import bevy_pbr::utils

@group(1) @binding(0)
var source_color_texture: texture_2d<f32>;
@group(1) @binding(1)
var source_texture_sampler: sampler;


@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {

    let color = textureSample(
        source_color_texture,
        source_texture_sampler,
        uv
    );
    var u: f32;
    var v: f32;
    if (color.r<= 0.00313)
    {
	     u = color.r * 12.92;
    } else {
     u = (1.055 * pow(color.r, 1./2.4)) - 0.055;
    }
    if (color.g<= 0.00313)
    {
	     v = color.g * 12.92;
    } else {
     v = (1.055 * pow(color.g, 1./2.4)) - 0.055;
    }
    if (u > 0.39 && u < 0.41) {
        return vec4<f32>(color.rgb,1.);
    }
    
    return vec4<f32>(color.rgba);
}
