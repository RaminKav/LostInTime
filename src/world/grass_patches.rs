use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::utils::HashMap;
use serde::Deserialize;

use crate::cursor::CursorPos;
use crate::ecs_helpers::safe_set_parent;
use crate::world::y_sort::YSort;
use crate::{GameState, ImageAssets, DEBUG};

/// One entry in a ground-patch sprite descriptor RON. Coordinates are in
/// tile units (16px) so the format matches `sprites.desc.ron`.
#[derive(Default, Clone, Copy, Debug, Deserialize)]
pub struct GroundPatchData {
    pub texture_pos: Vec2,
    pub size: Vec2,
}

/// Back-compat alias for `grass_patches.patches.ron`.
pub type GrassPatchData = GroundPatchData;

impl GroundPatchData {
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

/// Logical identifiers for decorative ground patch sprites (grass and desert).
#[derive(
    Deserialize, Debug, Hash, PartialEq, Eq, Clone, Copy, Reflect, FromReflect, Component, Default,
)]
#[reflect(Component)]
pub enum GroundPatch {
    #[default]
    GrassPatch1,
    GrassPatch2,
    GrassPatch3,
    GrassPatch4,
    DesertPatch1,
}

/// RON asset describing each grass patch sprite location on the
/// `grass_patches.png` sheet.
#[derive(Deserialize, TypeUuid)]
#[uuid = "b8a3a5d2-7f7d-4a23-9e9f-1d7d6f3b2a91"]
pub struct GrassPatchesDesc {
    pub patches: HashMap<GroundPatch, GroundPatchData>,
}

/// Loaded atlas + per-patch sprite info populated after assets finish loading.
#[derive(Resource, Default)]
pub struct GroundPatchesGraphics {
    pub grass_atlas: Option<Handle<TextureAtlas>>,
    pub grass_sprites: Option<HashMap<GroundPatch, TextureAtlasSprite>>,
    pub desert_atlas: Option<Handle<TextureAtlas>>,
    pub desert_sprites: Option<HashMap<GroundPatch, TextureAtlasSprite>>,
}

/// Marker for spawned decorative ground patches.
#[derive(Component)]
pub struct GroundPatchSprite;

/// `YSort` offset for grass aligned with debug key 7 ([`GroundPatch::GrassPatch2`]).
pub const GROUND_PATCH_YSORT_KEY_7: f32 = -1.01;
/// `YSort` offset for grass aligned with debug key 8 ([`GroundPatch::GrassPatch3`]).
pub const GROUND_PATCH_YSORT_KEY_8: f32 = -1.0;
/// `YSort` offset for grass aligned with debug key 9 ([`GroundPatch::GrassPatch4`]) — standalone only.
pub const GROUND_PATCH_YSORT_KEY_9: f32 = -0.99;

/// Local **Y** offset when grass is parented to a tree so the patch sits at the trunk base.
pub const GROUND_PATCH_TREE_LOCAL_OFFSET_Y: f32 = -32.0;
/// Local **Y** offset when a desert patch is parented under a large cactus.
pub const DESERT_PATCH_CACTUS_LOCAL_OFFSET_Y: f32 = -16.0;
/// Extra local **Y** nudge for all era-2 desert patches (negative = downward).
pub const DESERT_PATCH_Y_NUDGE: f32 = -8.0;
/// Water exclusion radius (Chebyshev tiles) for era-1 grass ground patches.
pub const GROUND_PATCH_WATER_CHECK_RADIUS_TILES: i8 = 1;
/// Water exclusion radius for the larger era-2 desert ground patch.
pub const DESERT_PATCH_WATER_CHECK_RADIUS_TILES: i8 = 2;

pub fn desert_patch_parent_offset(base: Vec2) -> Vec2 {
    base + Vec2::new(0., DESERT_PATCH_Y_NUDGE)
}
/// Local **Z** for ground patches parented under an object. Negative places the patch behind the parent's
/// sprite (parent `YSort` sets world z; stacking adds local z). Not used with [`YSort`] on the patch.
pub const GROUND_PATCH_CHILD_LOCAL_Z: f32 = -12.0;
/// Extra local z for patches under medium objects (boulders, crates, etc.).
pub const GROUND_PATCH_CHILD_Z_OBJECT_BONUS: f32 = -1.0;

/// Local offset for grass parented under a shrine. Shrine entities are spawned at
/// `tile_pos + anchor` ([`crate::assets::SpriteAnchor`]); negating the anchor pulls the patch
/// back toward the tile / visual base (taller shrines usually have larger anchor **y**).
pub fn grass_patch_local_offset_for_shrine_anchor(anchor: Vec2) -> Vec2 {
    Vec2::new(-anchor.x, -anchor.y)
}

pub struct GrassPatchesPlugin;

impl Plugin for GrassPatchesPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<GroundPatch>()
            .init_resource::<GroundPatchesGraphics>()
            .add_system(load_ground_patches.in_schedule(OnExit(GameState::Loading)))
            .add_system(debug_spawn_ground_patches.in_set(OnUpdate(GameState::Main)));
    }
}

fn load_ground_patches(
    image_assets: Res<ImageAssets>,
    grass_descs: Res<Assets<GrassPatchesDesc>>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut ground_graphics: ResMut<GroundPatchesGraphics>,
) {
    if let Some(desc) = grass_descs.get(&image_assets.grass_patches_desc) {
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
        ground_graphics.grass_atlas = Some(texture_atlases.add(atlas));
        ground_graphics.grass_sprites = Some(sprites);
    } else {
        warn!("GrassPatchesDesc asset not loaded yet");
    }

    let desert_patch_data = GroundPatchData {
        texture_pos: Vec2::ZERO,
        size: Vec2::new(64., 64.),
    };
    let mut desert_atlas = TextureAtlas::new_empty(
        image_assets.desert_patch_sheet.clone(),
        Vec2::new(64., 64.),
    );
    let mut desert_sprite =
        TextureAtlasSprite::new(desert_atlas.add_texture(desert_patch_data.to_atlas_rect()));
    desert_sprite.custom_size = Some(desert_patch_data.size);
    ground_graphics.desert_atlas = Some(texture_atlases.add(desert_atlas));
    ground_graphics.desert_sprites = Some(HashMap::from([(
        GroundPatch::DesertPatch1,
        desert_sprite,
    )]));
}

/// Spawn a decorative ground patch (grass or desert).
///
/// **Standalone** (`parent == None`): uses `world_pos` and [`YSort`] for depth above tiles.
///
/// **Parented** (`parent` set): uses local `parent_local_offset` plus [`GROUND_PATCH_CHILD_LOCAL_Z`]
/// and optional `extra_child_local_z`. Does **not** attach [`YSort`] — the parent's sorted z stacks
/// with local z so the patch stays visually under the object sprite.
pub fn spawn_ground_patch(
    commands: &mut Commands,
    ground_graphics: &GroundPatchesGraphics,
    patch: GroundPatch,
    world_pos: Vec2,
    y_sort: f32,
    parent: Option<Entity>,
    parent_local_offset: Vec2,
    extra_child_local_z: f32,
) -> Option<Entity> {
    let (atlas, sprite) = match patch {
        GroundPatch::DesertPatch1 => (
            ground_graphics.desert_atlas.as_ref()?.clone(),
            ground_graphics
                .desert_sprites
                .as_ref()?
                .get(&patch)?
                .clone(),
        ),
        _ => (
            ground_graphics.grass_atlas.as_ref()?.clone(),
            ground_graphics
                .grass_sprites
                .as_ref()?
                .get(&patch)?
                .clone(),
        ),
    };

    let transform = if parent.is_some() {
        Transform::from_translation(Vec3::new(
            parent_local_offset.x,
            parent_local_offset.y,
            GROUND_PATCH_CHILD_LOCAL_Z + extra_child_local_z,
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
        .insert(GroundPatchSprite)
        .insert(Name::new("Ground Patch"));
    if parent.is_none() {
        ec.insert(YSort(y_sort));
    }
    let id = ec.id();
    if let Some(p) = parent {
        safe_set_parent(commands, id, p);
    }
    Some(id)
}

fn debug_spawn_ground_patches(
    mut commands: Commands,
    keys: Res<Input<KeyCode>>,
    cursor: Res<CursorPos>,
    ground_graphics: Res<GroundPatchesGraphics>,
) {
    if !*DEBUG {
        return;
    }
    let patch = if keys.just_pressed(KeyCode::Key6) {
        GroundPatch::GrassPatch1
    } else if keys.just_pressed(KeyCode::Key7) {
        GroundPatch::GrassPatch2
    } else if keys.just_pressed(KeyCode::Key8) {
        GroundPatch::GrassPatch3
    } else if keys.just_pressed(KeyCode::Key9) {
        GroundPatch::GrassPatch4
    } else if keys.just_pressed(KeyCode::Key0) {
        GroundPatch::DesertPatch1
    } else {
        return;
    };

    let y_sort = match patch {
        GroundPatch::GrassPatch1 => -1.02,
        GroundPatch::GrassPatch2 => GROUND_PATCH_YSORT_KEY_7,
        GroundPatch::GrassPatch3 => GROUND_PATCH_YSORT_KEY_8,
        GroundPatch::GrassPatch4 => GROUND_PATCH_YSORT_KEY_9,
        GroundPatch::DesertPatch1 => GROUND_PATCH_YSORT_KEY_7,
    };

    spawn_ground_patch(
        &mut commands,
        &ground_graphics,
        patch,
        cursor.world_coords.truncate(),
        y_sort,
        None,
        Vec2::ZERO,
        0.,
    );
}
