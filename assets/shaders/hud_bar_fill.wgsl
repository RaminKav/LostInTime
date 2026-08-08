#import bevy_sprite::mesh2d_vertex_output::VertexOutput
// Fills a textured quad from the bottom up, like liquid in a potion.
// The texture's alpha defines the silhouette; the `fill` uniform (0..1)
// controls how far up the liquid has risen. The top row of liquid is
// brightened to read as a meniscus / highlight.
// Optional `shield_fill` draws a yellow shield layer (also bottom-up) that
// can extend above the HP fill when shield > HP.

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> fill: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var<uniform> highlight_thickness: f32; // in uv units; usually 1.0 / pixel_height
@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> highlight_strength: f32;  // 0..1, how much to brighten the surface line
@group(#{MATERIAL_BIND_GROUP}) @binding(3)
var<uniform> shield_fill: f32;
@group(#{MATERIAL_BIND_GROUP}) @binding(4)
var fill_texture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(5)
var fill_sampler: sampler;

@fragment
fn fragment(mesh: VertexOutput) -> @location(0) vec4<f32> {
    let uv = mesh.uv;
    let base = textureSample(fill_texture, fill_sampler, uv);

    // Bevy mesh2d UVs: (0,0) top-left, (1,1) bottom-right.
    // "Filled from bottom" -> visible where uv.y >= surface.
    let hp_surface = 1.0 - clamp(fill, 0.0, 1.0);
    let shield_surface = 1.0 - clamp(shield_fill, 0.0, 1.0);
    let in_hp = uv.y >= hp_surface;
    let in_shield = uv.y >= shield_surface;
    let visible = in_hp || in_shield;

    // The top row of liquid: the band [surface, surface + highlight_thickness).
    // Skip when the bar is full — no meniscus line at max fill.
    let surface = min(hp_surface, shield_surface);
    let max_fill = max(fill, shield_fill);
    let on_surface = visible && (max_fill < 1.0) && (uv.y < surface + highlight_thickness);

    var rgb = base.rgb;
    if (in_hp) {
        // Keep base HP tint.
    } else if (in_shield) {
        // Shield-only band above HP: yellow silhouette.
        rgb = vec3<f32>(0.93, 0.71, 0.21);
    }
    if (in_hp && in_shield) {
        // Overlay yellow on HP so shield reads on top of the red fill.
        let yellow = vec3<f32>(0.93, 0.71, 0.21);
        rgb = mix(rgb, yellow, 0.55);
    }
    if (on_surface) {
        // Brighten without blowing past white.
        let lift = vec3<f32>(highlight_strength);
        rgb = clamp(rgb + (vec3<f32>(1.0) - rgb) * lift, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // Silhouette from texture alpha; level cutoff from fill / shield.
    let alpha = base.a * select(0.0, 1.0, visible);
    return vec4<f32>(rgb, alpha);
}
