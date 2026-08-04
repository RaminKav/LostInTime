//! Full-screen dark overlay with a rectangular cutout + pulsing yellow border, spawned alongside
//! a [`crate::ui::tips::TipBox`] to draw the player's eye to a specific part of the HUD (see
//! `TipEvent::highlight` / `TipHighlightRect`). Lives on the same UI camera render layer (`3`)
//! as the rest of the HUD, which uses `ScalingMode::FixedVertical(game_height)` centered at the
//! origin — so quad-local units line up 1:1 with the coordinates used by HUD layout helpers
//! elsewhere in `src/ui/mod.rs` (e.g. `hud_heirloom_row_y`), no conversion needed.
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::Mesh2d;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d};

use crate::ui::ui_helpers::full_screen_overlay_size;
use crate::ScreenResolution;

/// Linear sRGB + alpha for tip-highlight uniforms.
/// Do not use [`crate::ui::ui_helpers::radial_overlay_color_uniform`] here — that helper
/// intentionally forces alpha to 1.0 because radial overlays drive opacity via other params.
#[inline]
fn tip_highlight_color_uniform(color: Color) -> Vec4 {
    let c = color.to_srgba();
    Vec4::new(c.red, c.green, c.blue, c.alpha)
}

/// Rectangle to leave uncovered (and outline) in a tip's highlight overlay, in the same
/// world/HUD coordinate space as everything else drawn on UI render layer 3 (origin at screen
/// center).
#[derive(Debug, Clone, Copy)]
pub struct TipHighlightRect {
    pub center: Vec2,
    pub size: Vec2,
}

/// Dark tint covering everything outside the highlighted rect.
const OVERLAY_COLOR: Color = Color::srgba(0., 0., 0., 0.92);
/// Pulsing border color drawing attention to the highlighted rect.
const GLOW_COLOR: Color = Color::srgb(1.0, 0.85, 0.2);
/// Border ring thickness, in HUD pixels.
const BORDER_THICKNESS: f32 = 3.0;
/// How fast the border's brightness pulses.
const PULSE_SPEED: f32 = 4.5;
/// How often (seconds) an expanding "ping" ring fires from the border, like a radar blip.
const PING_PERIOD: f32 = 2.0;
/// How far outward the ping ring travels before fully fading, in HUD pixels.
const PING_MAX_DISTANCE: f32 = 14.0;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TipHighlightMaterial {
    #[uniform(0)]
    pub overlay_color: Vec4,
    #[uniform(1)]
    pub glow_color: Vec4,
    /// (rect_min_x, rect_min_y, rect_max_x, rect_max_y), quad-local units.
    #[uniform(2)]
    pub rect: Vec4,
    /// (quad_width, quad_height, time, border_thickness).
    #[uniform(3)]
    pub params: Vec4,
    /// (pulse_speed, ping_period, ping_max_distance, unused).
    #[uniform(4)]
    pub params2: Vec4,
}

impl Material2d for TipHighlightMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/tip_highlight_overlay.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Marks the spawned full-screen highlight overlay quad so it can be despawned when its tip closes.
#[derive(Component)]
pub struct TipHighlightOverlay;

pub struct TipHighlightPlugin;
impl Plugin for TipHighlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Material2dPlugin::<TipHighlightMaterial>::default())
            .add_systems(Update, animate_tip_highlight_overlays);
    }
}

/// Spawns a full-screen highlight overlay quad for the given rect at `depth` (UI camera z).
/// Returns the spawned entity so callers can tie its lifetime to whatever UI it belongs to.
pub fn spawn_tip_highlight_overlay(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<TipHighlightMaterial>,
    res: &ScreenResolution,
    rect: TipHighlightRect,
    depth: f32,
) -> Entity {
    let quad_size = full_screen_overlay_size(res);
    let rect_min = rect.center - rect.size * 0.5;
    let rect_max = rect.center + rect.size * 0.5;

    let material = materials.add(TipHighlightMaterial {
        overlay_color: tip_highlight_color_uniform(OVERLAY_COLOR),
        glow_color: tip_highlight_color_uniform(GLOW_COLOR),
        rect: Vec4::new(rect_min.x, rect_min.y, rect_max.x, rect_max.y),
        params: Vec4::new(quad_size.x, quad_size.y, 0., BORDER_THICKNESS),
        params2: Vec4::new(PULSE_SPEED, PING_PERIOD, PING_MAX_DISTANCE, 0.),
    });
    let mesh: Mesh2d = meshes
        .add(Mesh::from(Rectangle::new(quad_size.x, quad_size.y)))
        .into();

    commands
        .spawn((
            mesh,
            MeshMaterial2d(material),
            (Transform::from_xyz(0., 0., depth), Visibility::default()),
            RenderLayers::from_layers(&[3]),
            TipHighlightOverlay,
            Name::new("Tip Highlight Overlay"),
        ))
        .id()
}

fn animate_tip_highlight_overlays(
    time: Res<Time>,
    overlays: Query<&MeshMaterial2d<TipHighlightMaterial>, With<TipHighlightOverlay>>,
    mut materials: ResMut<Assets<TipHighlightMaterial>>,
) {
    if overlays.is_empty() {
        return;
    }
    let elapsed = time.elapsed_secs();
    for material_handle in &overlays {
        if let Some(mut material) = materials.get_mut(&material_handle.0) {
            material.params.z = elapsed;
        }
    }
}
