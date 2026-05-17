use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle, Mesh2dHandle};

/// Material that fills a textured quad from the bottom up, with a brighter
/// "surface" line at the top of the fill. The bound texture's alpha defines
/// the bar's silhouette, so non-rectangular shapes (semicircles, potion
/// flasks, etc.) work without any shape math in the shader.
#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "f4d0a2d2-2c5e-4b27-9b8f-1e3a5b6c7d80"]
pub struct HudBarFillMaterial {
    /// Current fill level in [0, 1].
    #[uniform(0)]
    pub fill: f32,
    /// Thickness of the bright surface line, in UV units. Typically
    /// `1.0 / texture_height_px` so the highlight is exactly one pixel tall.
    #[uniform(1)]
    pub highlight_thickness: f32,
    /// How strongly to brighten the surface line, in [0, 1].
    #[uniform(2)]
    pub highlight_strength: f32,
    #[texture(3)]
    #[sampler(4)]
    pub texture: Option<Handle<Image>>,
}

impl Material2d for HudBarFillMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/hud_bar_fill.wgsl".into()
    }
}

impl HudBarFillMaterial {
    /// Convenience constructor. `pixel_height` should match the actual pixel
    /// height of `texture` (e.g. 34 for our 28x34 HP/mana fills) so the
    /// surface highlight is exactly one texel tall.
    pub fn new(texture: Handle<Image>, fill: f32, pixel_height: u32) -> Self {
        Self {
            fill,
            highlight_thickness: 1.0 / pixel_height.max(1) as f32,
            highlight_strength: 0.2,
            texture: Some(texture),
        }
    }
}

pub struct HudBarFillPlugin;

impl Plugin for HudBarFillPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(Material2dPlugin::<HudBarFillMaterial>::default());
    }
}

/// Spawns a single shader-driven fill quad. Returns the spawned entity so the
/// caller can parent it to the HUD frame and attach marker components
/// (`HealthBar`, `ManaBar`, etc.).
///
/// `size` should match the fill texture's pixel size (e.g. `Vec2::new(28., 34.)`).
pub fn spawn_hud_fill(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<HudBarFillMaterial>,
    texture: Handle<Image>,
    pixel_size: Vec2,
    translation: Vec3,
    initial_fill: f32,
) -> Entity {
    let mesh: Mesh2dHandle = meshes.add(Mesh::from(shape::Quad::new(pixel_size))).into();
    let material = materials.add(HudBarFillMaterial::new(
        texture,
        initial_fill,
        pixel_size.y as u32,
    ));
    commands
        .spawn(MaterialMesh2dBundle {
            mesh,
            material,
            transform: Transform::from_translation(translation),
            ..default()
        })
        .id()
}
