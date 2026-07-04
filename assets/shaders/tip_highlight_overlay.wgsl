// Full-screen dark overlay with a rectangular "cutout" (no darkening at all inside it) plus a
// thin pulsing yellow glow border traced around the cutout's edge. Used by tips (`src/ui/tips.rs`)
// to draw the player's eye to a specific part of the screen (e.g. the heirloom HUD row) while
// dimming everything else.
//
// The quad this material is applied to is a full-screen quad on the UI camera's render layer,
// centered at the origin — since that camera uses `ScalingMode::FixedVertical(game_height)` also
// centered at the origin, one quad-local unit == one screen/HUD-layout pixel. That means the
// cutout rect can be specified directly in the same coordinate space as the rest of the HUD
// layout helpers in `src/ui/mod.rs` (e.g. `hud_heirloom_row_y`), with no extra conversion.
#import bevy_sprite::mesh2d_view_bindings
#import bevy_sprite::mesh2d_bindings

#import bevy_sprite::mesh2d_functions

// Dark tint drawn everywhere outside the cutout rect (linear rgb + alpha).
@group(1) @binding(0)
var<uniform> overlay_color: vec4<f32>;
// Pulsing glow color traced around the cutout rect's border.
@group(1) @binding(1)
var<uniform> glow_color: vec4<f32>;
// rect: cutout rectangle in quad-local units, centered on the quad's own origin (min_x, min_y, max_x, max_y).
@group(1) @binding(2)
var<uniform> rect: vec4<f32>;
// params: x/y = this quad's own size (so uv can be mapped back to quad-local position), z = time
// (seconds), w = border thickness in quad-local units (pixels).
@group(1) @binding(3)
var<uniform> params: vec4<f32>;
// params2: x = pulse speed, y = ping period (seconds), z = max ping ring travel distance
// (quad-local units), w = unused.
@group(1) @binding(4)
var<uniform> params2: vec4<f32>;

@fragment
fn fragment(
    #import bevy_sprite::mesh2d_vertex_output
) -> @location(0) vec4<f32> {
    let quad_size = max(params.xy, vec2<f32>(1.0, 1.0));
    let time = params.z;
    let border = max(params.w, 0.5);
    let pulse_speed = params2.x;

    // Map this full-screen quad's own uv back to quad-local position (see `Quad`'s vertex
    // layout: v=0 is the top edge, so y needs to be flipped to match world-up-is-positive-y).
    let pos = vec2<f32>((uv.x - 0.5) * quad_size.x, (0.5 - uv.y) * quad_size.y);

    let rect_min = rect.xy;
    let rect_max = rect.zw;

    // Signed distance to the rect: negative inside, positive outside, 0 exactly on the edge.
    let d_outside = max(rect_min - pos, pos - rect_max);
    let outside_dist = length(max(d_outside, vec2<f32>(0.0, 0.0)));
    let inside_dist = min(max(d_outside.x, d_outside.y), 0.0);
    let dist = outside_dist + inside_dist;

    // Fully transparent (no darkening) inside the rect.
    let inside_cutout = dist < 0.0;

    // Thin ring around the rect edge, `border` quad-local units wide, centered on the edge.
    let half_border = border * 0.5;
    let ring = 1.0 - clamp(abs(dist) / half_border, 0.0, 1.0);

    // Brightness pulse "coursing through" the border rather than a flat glow.
    let pulse = 0.6 + 0.4 * sin(time * pulse_speed);
    let glow_alpha = ring * ring * pulse;

    // "Ping" — a copy of the border ring that expands outward from the rect and fades out,
    // repeating every `ping_period` seconds, like a radar blip drawing the eye to the box.
    let ping_period = max(params2.y, 0.1);
    let ping_max_dist = max(params2.z, 0.0);
    let ping_t = fract(time / ping_period);
    let ping_dist = ping_t * ping_max_dist;
    let ping_ring = 1.0 - clamp(abs(dist - ping_dist) / half_border, 0.0, 1.0);
    let ping_fade = 1.0 - ping_t;
    let ping_alpha = ping_ring * ping_ring * ping_fade * ping_fade;

    let combined_glow = max(glow_alpha, ping_alpha);
    let base_alpha = select(overlay_color.a, 0.0, inside_cutout);
    let final_color = mix(overlay_color.rgb, glow_color.rgb, combined_glow);
    let final_alpha = clamp(max(base_alpha, combined_glow), 0.0, 1.0);

    return vec4<f32>(final_color, final_alpha);
}
