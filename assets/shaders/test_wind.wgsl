#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

// NOTE: Bindings must come before functions that use them!
#import bevy_sprite::mesh2d_functions
// struct View {
//     view_proj: mat4x4<f32>,
//     inverse_view_proj: mat4x4<f32>,
//     view: mat4x4<f32>,
//     inverse_view: mat4x4<f32>,
//     projection: mat4x4<f32>,
//     inverse_projection: mat4x4<f32>,
//     world_position: vec3<f32>,
//     // viewport(x_origin, y_origin, width, height)
//     viewport: vec4<f32>,
// };

// struct WindUniform{
//     _Color: vec4<f32>, // (1,1,1,1)
//     _ShakeDisplacement: f32,
//     _ShakeTime: f32,
//     _ShakeWindspeed: f32,
//     _ShakeBending: f32,
// };

@group(1) @binding(0)
var<uniform> speed: f32;
@group(1) @binding(1)
var<uniform> minStrength: f32;
@group(1) @binding(2)
var<uniform> maxStrength: f32;
@group(1) @binding(3)
var<uniform> strengthScale: f32;
@group(1) @binding(4)
var<uniform> interval: f32;
@group(1) @binding(5)
var<uniform> detail: f32;
@group(1) @binding(6)
var<uniform> distortion: f32;
@group(1) @binding(7)
var<uniform> heightOffset: f32;
@group(1) @binding(8)
var<uniform> offset: f32; 


// @group(1) @binding(1)
// var<uniform> _ShakeDisplacement: f32;
// @group(1) @binding(2)
// var<uniform> _ShakeTime: f32;
// @group(1) @binding(3)
// var<uniform> _ShakeWindspeed: f32;
// @group(1) @binding(4)
// var<uniform>  _ShakeBending: f32;
@group(1) @binding(9)
var _MainTex: texture_2d<f32>; //("Base (RGB) Trans (A)", 2D) = "white" {}
@group(1) @binding(10)
var _MainTexSampler: sampler; //("Base (RGB) Trans (A)", 2D) = "white" {}
// @group(0) @binding(0)
// var<uniform> view: View;

struct Vertex {
#ifdef VERTEX_POSITIONS
    @location(0) position: vec3<f32>,
#endif
#ifdef VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#ifdef VERTEX_UVS
    @location(2) uv: vec2<f32>,
#endif
#ifdef VERTEX_TANGENTS
    @location(3) tangent: vec4<f32>,
#endif
#ifdef VERTEX_COLORS
    @location(4) color: vec4<f32>,
#endif
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    #import bevy_sprite::mesh2d_vertex_output
}

fn fastSin(val: vec4<f32>) -> vec4<f32> {
    let new_val = val * 6.408849 - 3.1415927;
    let r5: vec4<f32> = new_val * new_val;
    let r6: vec4<f32> = r5 * r5;
    let r7: vec4<f32> = r6 * r5;
    let r8: vec4<f32> = r6 * r5;
    let r1: vec4<f32> = r5 * new_val;
    let r2: vec4<f32> = r1 * r5;
    let r3: vec4<f32> = r2 * r5;
    let sin7: vec4<f32> = vec4<f32>(1., -0.16161616, 0.0083333, -0.00019841) ;
    return vec4<f32>(new_val + r1 * sin7.y + r2 * sin7.z + r3 * sin7.w);
}
fn getWind(uv: vec2<f32>, time: f32) -> f32{
    let diff: f32 = pow(maxStrength - minStrength, 2.0);
    let strength: f32 = clamp(minStrength + diff + sin(time / interval) * diff, minStrength, maxStrength) * strengthScale;
    let wind: f32 = (sin(time) + cos(time * detail)) * strength * max(0.0, (1.0-uv.y) - heightOffset);
    
    return wind; 
}

@vertex
fn vertex(
    v: Vertex
) -> VertexOutput {
    var out: VertexOutput;
    #ifdef VERTEX_UVS
        out.uv = v.uv;
    #endif

    #ifdef VERTEX_POSITIONS
        out.world_position = mesh2d_position_local_to_world(mesh.model, vec4<f32>(v.position, 1.0));
        out.clip_position = mesh2d_position_world_to_clip(out.world_position);
    #endif

    #ifdef VERTEX_NORMALS
        out.world_normal = mesh2d_normal_local_to_world(v.normal);
    #endif

    #ifdef VERTEX_TANGENTS
        out.world_tangent = mesh2d_tangent_local_to_world(mesh.model, v.tangent);
    #endif

    #ifdef VERTEX_COLORS
        out.color = v.color;
    #endif
    
    // let factor: f32 = (1. - _ShakeDisplacement ) * 0.5;
       
    // let _WindSpeed: f32  = (_ShakeWindspeed);    
    // let _WaveScale: f32 = _ShakeDisplacement;
   
    // let  _waveXSize: vec4<f32> = vec4<f32>(0.048, 0.06, 0.24, 0.096);
    // let  _waveZSize: vec4<f32> = vec4<f32>(0.024, 0.08, 0.08, 0.2);
    // let  waveSpeed: vec4<f32> = vec4<f32>(1.2, 2., 1.6, 4.8);
 
    // let _waveXmove: vec4<f32> = vec4<f32>(0.024, 0.04, -0.12, 0.096);
    // let _waveZmove: vec4<f32> = vec4<f32>(0.006, 0.02, -0.02, 0.1);
   
    // var waves: vec4<f32>;
    // waves = v.position.x * _waveXSize;
    // waves += v.position.z * _waveZSize; //look up hlsl spec to see how this works
    // //todo: get time
    // waves += globals.time * (1. - _ShakeTime * 2. ) * waveSpeed * _WindSpeed;
 
    // waves = fract(waves);
    // var s: vec4<f32> = fastSin(waves);
 
    // let waveAmount: f32 = v.uv.y * (_ShakeBending);
    // s *= waveAmount;
 
    // s *= normalize(waveSpeed);
 
    // s = s * s;
    // let fade: f32 = dot(s, vec4<f32>(1.3));
    // s = s * s;
    // var waveMove: vec3<f32> = vec3<f32>(0.,0.,0.);
    // waveMove.x = dot(s, _waveXmove);
    // waveMove.z = dot(s, _waveZmove);

    // // find out wtf is going on w the _World2Object world matrix thing
    // var w = (view.inverse_view * vec4<f32>(waveMove, 0.));
    // out.clip_position.x -= waveMove.x;
    // out.clip_position.z -= waveMove.z;
    let pos: vec4<f32> = out.world_position;// * vec4(0.0, 0.0, 0.0, 1.0);
    // let time: f32 = globals.time * speed + offset;
    let time: f32 = globals.time * speed + pos.y +pos.z ;// not working when moving...
    out.clip_position.x += getWind(v.uv, time);
    let pix: f32 = 32.;
    out.uv *= vec2<f32>(pix,pix);
    out.uv = vec2<f32>(trunc(out.uv.x),trunc(out.uv.y));
    out.uv /= vec2<f32>(pix,pix);

    
    return out;
}

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
    ) -> @location(0) vec4<f32>{
    let c = textureSample(_MainTex, _MainTexSampler, uv) * vec4<f32>(1.,1.,1.,1.);
    return c;
}