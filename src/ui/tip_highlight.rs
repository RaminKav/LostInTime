//! Full-screen dark overlay with a rectangular cutout + pulsing yellow border, spawned alongside
//! a [`crate::ui::tips::TipBox`] to draw the player's eye to a specific part of the HUD (see
//! `TipEvent::highlight` / `TipHighlightRect`). Lives on the same UI camera render layer (`3`)
//! as the rest of the HUD, which uses `ScalingMode::FixedVertical(game_height)` centered at the
//! origin — so quad-local units line up 1:1 with the coordinates used by HUD layout helpers
//! elsewhere in `src/ui/mod.rs` (e.g. `hud_heirloom_row_y`), no conversion needed.
use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::render::view::RenderLayers;
use bevy::sprite::{Material2d, Material2dPlugin, Mesh2dHandle};

use crate::ui::ui_helpers::full_screen_overlay_size;
use crate::ScreenResolution;

/// Rectangle to leave uncovered (and outline) in a tip's highlight overlay, in the same
/// world/HUD coordinate space as everything else drawn on UI render layer 3 (origin at screen
/// center).
#[derive(Debug, Clone, Copy)]
pub struct TipHighlightRect {
    pub center: Vec2,
    pub size: Vec2,
}

/// Dark tint covering everything outside the highlighted rect.
const OVERLAY_COLOR: Color = Color::rgba(0., 0., 0., 0.92);
/// Pulsing border color drawing attention to the highlighted rect.
const GLOW_COLOR: Color = Color::rgb(1.0, 0.85, 0.2);
/// Border ring thickness, in HUD pixels.
const BORDER_THICKNESS: f32 = 3.0;
/// How fast the border's brightness pulses.
const PULSE_SPEED: f32 = 4.5;
/// How often (seconds) an expanding "ping" ring fires from the border, like a radar blip.
const PING_PERIOD: f32 = 2.0;
/// How far outward the ping ring travels before fully fading, in HUD pixels.
const PING_MAX_DISTANCE: f32 = 14.0;

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "6f2c9a3e-1d4b-4a77-9b2d-7e6c4a2f8d31"]
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
}

/// Marks the spawned full-screen highlight overlay quad so it can be despawned when its tip closes.
#[derive(Component)]
pub struct TipHighlightOverlay;

pub struct TipHighlightPlugin;
impl Plugin for TipHighlightPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<TipHighlightMaterial>::default())
            .add_system(animate_tip_highlight_overlays);
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
        overlay_color: OVERLAY_COLOR.into(),
        glow_color: GLOW_COLOR.into(),
        rect: Vec4::new(rect_min.x, rect_min.y, rect_max.x, rect_max.y),
        params: Vec4::new(quad_size.x, quad_size.y, 0., BORDER_THICKNESS),
        params2: Vec4::new(PULSE_SPEED, PING_PERIOD, PING_MAX_DISTANCE, 0.),
    });
    let mesh: Mesh2dHandle = meshes.add(Mesh::from(shape::Quad::new(quad_size))).into();

    commands
        .spawn((
            mesh,
            material,
            SpatialBundle::from_transform(Transform::from_xyz(0., 0., depth)),
            RenderLayers::from_layers(&[3]),
            TipHighlightOverlay,
            Name::new("Tip Highlight Overlay"),
        ))
        .id()
}

fn animate_tip_highlight_overlays(
    time: Res<Time>,
    overlays: Query<&Handle<TipHighlightMaterial>, With<TipHighlightOverlay>>,
    mut materials: ResMut<Assets<TipHighlightMaterial>>,
) {
    if overlays.is_empty() {
        return;
    }
    let elapsed = time.elapsed_seconds();
    for handle in &overlays {
        if let Some(material) = materials.get_mut(handle) {
            material.params.z = elapsed;
        }
    }
}
