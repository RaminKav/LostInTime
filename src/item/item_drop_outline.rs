use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::sprite::{Material2d, Material2dPlugin, Mesh2dHandle};
use bevy::utils::HashMap;

use crate::assets::Graphics;
use crate::item::ItemDrop;
use crate::GameState;

/// Caches one outline material per atlas sprite index. Each material carries the
/// sprite's UV bounds so the shader only samples within that sub-rect and never
/// reads neighboring sprites in the sheet.
#[derive(Resource, Default)]
pub struct ItemDropOutlineState {
    pub materials: HashMap<usize, Handle<ItemDropOutlineMaterial>>,
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "c8e4f1a2-3b6d-4e9f-a1c2-d5e6f708192a"]
pub struct ItemDropOutlineMaterial {
    /// Sub-rect of the atlas this sprite occupies, in UV space:
    /// (min_u, min_v, max_u, max_v). Neighbor samples outside this are ignored.
    #[uniform(0)]
    pub uv_bounds: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

impl Material2d for ItemDropOutlineMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/item_drop_outline.wgsl".into()
    }
}

pub struct ItemDropOutlinePlugin;

impl Plugin for ItemDropOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ItemDropOutlineState>()
            .add_plugin(Material2dPlugin::<ItemDropOutlineMaterial>::default())
            .add_system(
                apply_item_drop_outline
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::Initializing))),
            );
    }
}

fn apply_item_drop_outline(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ItemDropOutlineMaterial>>,
    mut state: ResMut<ItemDropOutlineState>,
    atlases: Res<Assets<TextureAtlas>>,
    query: Query<
        (Entity, &TextureAtlasSprite, &Handle<TextureAtlas>),
        (With<ItemDrop>, Without<Mesh2dHandle>),
    >,
) {
    for (entity, sprite, atlas_handle) in &query {
        let Some(atlas) = atlases.get(atlas_handle) else {
            continue;
        };

        let rect = atlas.textures[sprite.index];
        let atlas_size = atlas.size;
        let uv_bounds = Vec4::new(
            rect.min.x / atlas_size.x,
            rect.min.y / atlas_size.y,
            rect.max.x / atlas_size.x,
            rect.max.y / atlas_size.y,
        );

        let material = state
            .materials
            .entry(sprite.index)
            .or_insert_with(|| {
                materials.add(ItemDropOutlineMaterial {
                    uv_bounds,
                    source_texture: Some(atlas.texture.clone()),
                })
            })
            .clone();

        let mesh = mesh_from_atlas_sprite(&mut meshes, atlas, sprite);

        // Insert only the mesh + material, NOT the full MaterialMesh2dBundle.
        // The bundle carries a default Transform/Visibility which would clobber
        // the drop's real spawn position and rotation.
        commands
            .entity(entity)
            .insert((mesh, material))
            .remove::<TextureAtlasSprite>()
            .remove::<Handle<TextureAtlas>>();
    }
}

/// World-space margin added around the sprite so the 1px outline has room to
/// draw even when the art touches its atlas cell edge. The matching UV margin
/// is one texel, so the quad stays at 1:1 pixel scale.
const OUTLINE_MARGIN_PX: f32 = 1.0;

fn mesh_from_atlas_sprite(
    meshes: &mut Assets<Mesh>,
    atlas: &TextureAtlas,
    sprite: &TextureAtlasSprite,
) -> Mesh2dHandle {
    let rect = atlas.textures[sprite.index];
    let atlas_size = atlas.size;

    let texel = Vec2::new(1.0 / atlas_size.x, 1.0 / atlas_size.y);

    // Unexpanded sprite rect in UV space.
    let u0 = rect.min.x / atlas_size.x;
    let u1 = rect.max.x / atlas_size.x;
    let v0 = rect.min.y / atlas_size.y;
    let v1 = rect.max.y / atlas_size.y;

    // Expand UVs outward by one texel so the quad has a 1px margin ring. The
    // shader clamps neighbor sampling to `uv_bounds`, so this margin reads as
    // transparent for the base sprite and only ever shows the outline.
    let (lu, ru) = if sprite.flip_x {
        (u1 + texel.x, u0 - texel.x)
    } else {
        (u0 - texel.x, u1 + texel.x)
    };
    let bottom_v = v0 - texel.y;
    let top_v = v1 + texel.y;

    let size = sprite
        .custom_size
        .unwrap_or_else(|| Vec2::new(rect.width(), rect.height()))
        + Vec2::splat(OUTLINE_MARGIN_PX * 2.0);

    // Match the vertex winding of `shape::Quad`, whose default UVs are
    // [0,0], [0,1], [1,1], [1,0] (top-left, bottom-left, bottom-right, top-right).
    // V is flipped relative to the atlas rect, so top vertices use the larger v.
    let mut mesh = Mesh::from(shape::Quad::new(size));
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![
            [lu, top_v],
            [lu, bottom_v],
            [ru, bottom_v],
            [ru, top_v],
        ],
    );

    meshes.add(mesh).into()
}
