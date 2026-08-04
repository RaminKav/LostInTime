#import bevy_sprite::{
    mesh2d_functions as mesh_functions,
    mesh2d_vertex_output::VertexOutput,
    mesh2d_view_bindings::globals,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> speed: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> minStrength: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> maxStrength: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var<uniform> strengthScale: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(4)
var<uniform> interval: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(5)
var<uniform> detail: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(6)
var<uniform> distortion: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(7)
var<uniform> heightOffset: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(8)
var<uniform> offset: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(9)
var<uniform> opacity: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(10)
var _MainTex: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(11)
var _MainTexSampler: sampler;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
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

fn getWind(uv: vec2<f32>, time: f32) -> f32 {
    let diff: f32 = pow(maxStrength - minStrength, 2.0);
    let strength: f32 = clamp(
        minStrength + diff + sin(time / interval) * diff,
        minStrength,
        maxStrength
    ) * strengthScale;
    return (sin(time) + cos(time * detail)) * strength * max(0.0, (1.0 - uv.y) - heightOffset);
}

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
#ifdef VERTEX_UVS
    out.uv = vertex.uv;
#endif

#ifdef VERTEX_POSITIONS
    var world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    out.world_position = mesh_functions::mesh2d_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0)
    );
    out.position = mesh_functions::mesh2d_position_world_to_clip(out.world_position);
#endif

#ifdef VERTEX_NORMALS
    out.world_normal = mesh_functions::mesh2d_normal_local_to_world(
        vertex.normal,
        vertex.instance_index
    );
#endif

#ifdef VERTEX_TANGENTS
    out.world_tangent = mesh_functions::mesh2d_tangent_local_to_world(
        world_from_local,
        vertex.tangent
    );
#endif

#ifdef VERTEX_COLORS
    out.color = vertex.color;
#endif

    let time: f32 = globals.time * speed + out.world_position.y + out.world_position.z;
#ifdef VERTEX_UVS
    out.position.x += getWind(vertex.uv, time);

    let pix: f32 = 32.;
    out.uv *= vec2<f32>(pix, pix);
    out.uv = vec2<f32>(trunc(out.uv.x), trunc(out.uv.y));
    out.uv /= vec2<f32>(pix, pix);
#endif

    return out;
}

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let c = textureSample(_MainTex, _MainTexSampler, uv);
    return vec4<f32>(c.rgb, c.a * opacity);
}
