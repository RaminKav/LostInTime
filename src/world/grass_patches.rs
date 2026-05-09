use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::HashMap;
use serde::Deserialize;

use crate::cursor::CursorPos;
use crate::world::y_sort::YSort;
use crate::{GameState, ImageAssets, DEBUG};

/// One entry in the grass patches sprite descriptor RON. Coordinates are in
/// tile units (16px) so the format matches `sprites.desc.ron`.
#[derive(Default, Clone, Copy, Debug, Deserialize)]
pub struct GrassPatchData {
    pub texture_pos: Vec2,
    pub size: Vec2,
}

impl GrassPatchData {
    pub fn to_atlas_rect(self) -> bevy::math::Rect {
        bevy::math::Rect {
            min: Vec2::new(
                self.texture_pos.x * 16. + 0.15,
                self.texture_pos.y * 16. + 0.15,
            ),
            max: Vec2::new(
                self.texture_pos.x * 16. + self.size.x - 0.15,
                self.texture_pos.y * 16. + self.size.y - 0.15,
            ),
        }
    }
}

/// Logical identifiers for the decorative grass patch sprites.
#[derive(
    Deserialize, Debug, Hash, PartialEq, Eq, Clone, Copy, Reflect, FromReflect, Component, Default,
)]
#[reflect(Component)]
pub enum GrassPatch {
    #[default]
    GrassPatch1,
    GrassPatch2,
    GrassPatch3,
    GrassPatch4,
}

/// RON asset describing each grass patch sprite location on the
/// `grass_patches.png` sheet.
#[derive(Deserialize, TypeUuid)]
#[uuid = "b8a3a5d2-7f7d-4a23-9e9f-1d7d6f3b2a91"]
pub struct GrassPatchesDesc {
    pub patches: HashMap<GrassPatch, GrassPatchData>,
}

/// Loaded atlas + per-patch sprite info populated after assets finish loading.
#[derive(Resource, Default)]
pub struct GrassPatchesGraphics {
    pub atlas: Option<Handle<TextureAtlas>>,
    pub sprites: Option<HashMap<GrassPatch, TextureAtlasSprite>>,
}

/// Marker for spawned decorative grass patches (debug-spawned for now).
#[derive(Component)]
pub struct GrassPatchSprite;

/// `YSort` offset for grass aligned with debug key 7 ([`GrassPatch::GrassPatch2`]).
pub const GRASS_PATCH_YSORT_KEY_7: f32 = -1.01;
/// `YSort` offset for grass aligned with debug key 8 ([`GrassPatch::GrassPatch3`]).
pub const GRASS_PATCH_YSORT_KEY_8: f32 = -1.0;
/// `YSort` offset for grass aligned with debug key 9 ([`GrassPatch::GrassPatch4`]) — standalone only.
pub const GRASS_PATCH_YSORT_KEY_9: f32 = -0.99;

/// Local **Y** offset when grass is parented to a tree so the patch sits at the trunk base.
pub const GRASS_PATCH_TREE_LOCAL_OFFSET_Y: f32 = -32.0;
/// Local **Z** for grass parented under an object. Negative places the patch behind the parent's
/// sprite (parent `YSort` sets world z; stacking adds local z). Not used with [`YSort`] on the patch.
pub const GRASS_PATCH_CHILD_LOCAL_Z: f32 = -12.0;

/// Local offset for grass parented under a shrine. Shrine entities are spawned at
/// `tile_pos + anchor` ([`crate::assets::SpriteAnchor`]); negating the anchor pulls the patch
/// back toward the tile / visual base (taller shrines usually have larger anchor **y**).
pub fn grass_patch_local_offset_for_shrine_anchor(anchor: Vec2) -> Vec2 {
    Vec2::new(-anchor.x, -anchor.y)
}

pub struct GrassPatchesPlugin;

impl Plugin for GrassPatchesPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GrassPatch>()
            .init_resource::<GrassPatchesGraphics>()
            .add_system(load_grass_patches.in_schedule(OnExit(GameState::Loading)))
            .add_system(debug_spawn_grass_patches.in_set(OnUpdate(GameState::Main)));
    }
}

fn load_grass_patches(
    image_assets: Res<ImageAssets>,
    descs: Res<Assets<GrassPatchesDesc>>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut grass_graphics: ResMut<GrassPatchesGraphics>,
) {
    let Some(desc) = descs.get(&image_assets.grass_patches_desc) else {
        warn!("GrassPatchesDesc asset not loaded yet");
        return;
    };

    let mut atlas = TextureAtlas::new_empty(
        image_assets.grass_patches_sheet.clone(),
        Vec2::new(176., 320.),
    );
    let mut sprites = HashMap::default();
    for (patch, data) in desc.patches.iter() {
        let mut sprite = TextureAtlasSprite::new(atlas.add_texture(data.to_atlas_rect()));
        sprite.custom_size = Some(data.size);
        sprites.insert(*patch, sprite);
    }
    grass_graphics.atlas = Some(texture_atlases.add(atlas));
    grass_graphics.sprites = Some(sprites);
}

/// Spawn a grass patch.
///
/// **Standalone** (`parent == None`): uses `world_pos` and [`YSort`] for depth above tiles.
///
/// **Parented** (`parent` set): uses local `parent_local_offset` plus [`GRASS_PATCH_CHILD_LOCAL_Z`].
/// Does **not** attach [`YSort`] — the parent's sorted z stacks with local z so the patch stays
/// visually under the object sprite.
pub fn spawn_grass_patch(
    commands: &mut Commands,
    grass_graphics: &GrassPatchesGraphics,
    patch: GrassPatch,
    world_pos: Vec2,
    y_sort: f32,
    parent: Option<Entity>,
    parent_local_offset: Vec2,
) -> Option<Entity> {
    let atlas = grass_graphics.atlas.as_ref()?.clone();
    let sprite = grass_graphics.sprites.as_ref()?.get(&patch)?.clone();

    let transform = if parent.is_some() {
        Transform::from_translation(Vec3::new(
            parent_local_offset.x,
            parent_local_offset.y,
            GRASS_PATCH_CHILD_LOCAL_Z
                + if patch == GrassPatch::GrassPatch2 {
                    -1.
                } else {
                    0.
                },
        ))
    } else {
        Transform::from_translation(Vec3::new(world_pos.x, world_pos.y, 0.))
    };

    let mut ec = commands.spawn(SpriteSheetBundle {
        sprite,
        texture_atlas: atlas,
        transform,
        ..Default::default()
    });
    ec.insert(patch)
        .insert(GrassPatchSprite)
        .insert(Name::new("Grass Patch"));
    if parent.is_none() {
        ec.insert(YSort(y_sort));
    }
    let id = ec.id();
    if let Some(p) = parent {
        commands.entity(id).set_parent(p);
    }
    Some(id)
}

fn debug_spawn_grass_patches(
    mut commands: Commands,
    keys: Res<Input<KeyCode>>,
    cursor: Res<CursorPos>,
    grass_graphics: Res<GrassPatchesGraphics>,
) {
    if !*DEBUG {
        return;
    }
    let patch = if keys.just_pressed(KeyCode::Key6) {
        GrassPatch::GrassPatch1
    } else if keys.just_pressed(KeyCode::Key7) {
        GrassPatch::GrassPatch2
    } else if keys.just_pressed(KeyCode::Key8) {
        GrassPatch::GrassPatch3
    } else if keys.just_pressed(KeyCode::Key9) {
        GrassPatch::GrassPatch4
    } else {
        return;
    };

    let y_sort = match patch {
        GrassPatch::GrassPatch1 => -1.02,
        GrassPatch::GrassPatch2 => GRASS_PATCH_YSORT_KEY_7,
        GrassPatch::GrassPatch3 => GRASS_PATCH_YSORT_KEY_8,
        GrassPatch::GrassPatch4 => GRASS_PATCH_YSORT_KEY_9,
    };

    spawn_grass_patch(
        &mut commands,
        &grass_graphics,
        patch,
        cursor.world_coords.truncate(),
        y_sort,
        None,
        Vec2::ZERO,
    );
}
