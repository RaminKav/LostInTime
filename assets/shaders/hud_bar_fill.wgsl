// Fills a textured quad from the bottom up, like liquid in a potion.
// The texture's alpha defines the silhouette; the `fill` uniform (0..1)
// controls how far up the liquid has risen. The top row of liquid is
// brightened to read as a meniscus / highlight.

@group(1) @binding(0)
var<uniform> fill: f32;
@group(1) @binding(1)
var<uniform> highlight_thickness: f32; // in uv units; usually 1.0 / pixel_height
@group(1) @binding(2)
var<uniform> highlight_strength: f32;  // 0..1, how much to brighten the surface line
@group(1) @binding(3)
var fill_texture: texture_2d<f32>;
@group(1) @binding(4)
var fill_sampler: sampler;

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    let base = textureSample(fill_texture, fill_sampler, uv);

    // Bevy mesh2d UVs: (0,0) top-left, (1,1) bottom-right.
    // "Filled from bottom" -> visible where uv.y >= surface.
    let surface = 1.0 - clamp(fill, 0.0, 1.0);
    let in_fill = uv.y >= surface;

    // The top row of liquid: the band [surface, surface + highlight_thickness).
    // Skip when the bar is full — no meniscus line at max fill.
    let on_surface = in_fill && (fill < 1.0) && (uv.y < surface + highlight_thickness);

    var rgb = base.rgb;
    if (on_surface) {
        // Brighten without blowing past white.
        let lift = vec3<f32>(highlight_strength);
        rgb = clamp(rgb + (vec3<f32>(1.0) - rgb) * lift, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // Silhouette from texture alpha; level cutoff from fill.
    let alpha = base.a * select(0.0, 1.0, in_fill);
    return vec4<f32>(rgb, alpha);
}
