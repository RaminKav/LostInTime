use bevy::prelude::*;
use bevy::reflect::TypeUuid;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};
use bevy::sprite::{Material2d, Material2dPlugin, Mesh2dHandle};
use bevy::utils::{HashMap, HashSet};
use bevy_aseprite::anim::AsepriteAnimation;

use crate::assets::Graphics;
use crate::blessings::BlessingChoiceUI;
use crate::colors::overwrite_alpha;
use crate::cursor::{CustomCursor, CursorColorSettings};
use crate::item::ItemDrop;
use crate::player::skills::{Heirloom, HeirloomChoiceQueue, HeirloomRarity, PlayerSkills};
use crate::ui::item_chest::{ItemChestFinalHeirloom, ItemChestState};
use crate::ui::player_hud::SkillHudIcon;
use crate::ui::{
    BanishTrackerIcon, CrystalUnlockIcon, MerchantHeirloomHover, MicrowaveHeirloomButton,
    SkillChoiceUI,
};
use crate::Player;

/// Alpha used by item-drop and cursor outlines (#cdceee at 0.22).
pub const DEFAULT_OUTLINE_ALPHA: f32 = 0.22;
/// Stronger heirloom outline for HUD row icons.
pub const HUD_HEIRLOOM_OUTLINE_ALPHA: f32 = 0.42;
/// Softer heirloom outline on tooltip cards (hover + level-up choice cards).
pub const TOOLTIP_CARD_HEIRLOOM_OUTLINE_ALPHA: f32 = 0.12;

/// Default outline for floor item drops and the in-game cursor (#cdceee).
pub const DEFAULT_OUTLINE_COLOR: Color =
    Color::rgba(0.804, 0.808, 0.933, DEFAULT_OUTLINE_ALPHA);

/// Controls outline strength for heirloom icons in different UI contexts.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeirloomIconOutlineStyle {
    #[default]
    Default,
    Hud,
    TooltipCard,
}

impl HeirloomIconOutlineStyle {
    pub fn alpha(self) -> f32 {
        match self {
            Self::Default => DEFAULT_OUTLINE_ALPHA,
            Self::Hud => HUD_HEIRLOOM_OUTLINE_ALPHA,
            Self::TooltipCard => TOOLTIP_CARD_HEIRLOOM_OUTLINE_ALPHA,
        }
    }
}

/// Marks a heirloom icon sprite for a rarity-colored outline.
#[derive(Component, Clone, Copy, Debug)]
pub struct HeirloomIconOutline {
    pub rarity: HeirloomRarity,
    pub style: HeirloomIconOutlineStyle,
}

impl HeirloomIconOutline {
    pub fn new(rarity: HeirloomRarity, style: HeirloomIconOutlineStyle) -> Self {
        Self { rarity, style }
    }

    pub fn outline_color(&self) -> Color {
        overwrite_alpha(heirloom_rarity_outline_base(self.rarity), self.style.alpha())
    }
}

/// Saturated outline tints derived from each rarity's text hue family. Text title
/// colors are intentionally pastel; at low outline alpha they wash out to white.
fn heirloom_rarity_outline_base(rarity: HeirloomRarity) -> Color {
    match rarity {
        HeirloomRarity::Common => Color::rgba(148. / 255., 152. / 255., 178. / 255., 1.),
        HeirloomRarity::Uncommon => Color::rgba(58. / 255., 188. / 255., 172. / 255., 1.),
        HeirloomRarity::Rare => Color::rgba(204. / 255., 88. / 255., 228. / 255., 1.),
        HeirloomRarity::Legendary => Color::rgba(245. / 255., 158. / 255., 24. / 255., 1.),
    }
}

/// Caches outline materials per atlas sprite index + outline color.
#[derive(Resource, Default)]
pub struct AtlasSpriteOutlineState {
    pub materials: HashMap<(usize, u32), Handle<AtlasSpriteOutlineMaterial>>,
}

/// Maps atlas sprite indices back to heirlooms for auto-tagging icon sprites.
#[derive(Resource, Default)]
pub struct HeirloomIconAtlasLookup {
    pub index_to_heirloom: HashMap<usize, Heirloom>,
    pub indices: HashSet<usize>,
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "c8e4f1a2-3b6d-4e9f-a1c2-d5e6f708192a"]
pub struct AtlasSpriteOutlineMaterial {
    /// Sub-rect of the atlas this sprite occupies, in UV space:
    /// (min_u, min_v, max_u, max_v). Neighbor samples outside this are ignored.
    #[uniform(0)]
    pub uv_bounds: Vec4,
    #[uniform(3)]
    pub outline_color: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

impl Material2d for AtlasSpriteOutlineMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/item_drop_outline.wgsl".into()
    }
}

pub struct ItemDropOutlinePlugin;

impl Plugin for ItemDropOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AtlasSpriteOutlineState>()
            .init_resource::<HeirloomIconAtlasLookup>()
            .add_plugin(Material2dPlugin::<AtlasSpriteOutlineMaterial>::default())
            .add_system(init_heirloom_icon_atlas_lookup)
            .add_system(tag_heirloom_icon_outlines.before(apply_atlas_sprite_outlines))
            .add_system(refresh_outlined_cursor_on_settings_change.before(apply_atlas_sprite_outlines))
            .add_system(apply_atlas_sprite_outlines);
    }
}

fn init_heirloom_icon_atlas_lookup(
    graphics: Res<Graphics>,
    mut lookup: ResMut<HeirloomIconAtlasLookup>,
) {
    if !lookup.indices.is_empty() {
        return;
    }
    let Some(heirloom_sprites) = graphics.heirloom_sprites.as_ref() else {
        return;
    };
    for (heirloom, sprite) in heirloom_sprites {
        lookup.indices.insert(sprite.index);
        lookup
            .index_to_heirloom
            .insert(sprite.index, heirloom.clone());
    }
}

fn tag_heirloom_icon_outlines(
    mut commands: Commands,
    graphics: Res<Graphics>,
    lookup: Res<HeirloomIconAtlasLookup>,
    player_skills: Query<&PlayerSkills>,
    choice_queue: Option<Res<HeirloomChoiceQueue>>,
    item_chest_state: Option<Res<ItemChestState>>,
    parents: Query<&Parent>,
    crystal_icons: Query<&CrystalUnlockIcon>,
    banish_icons: Query<&BanishTrackerIcon>,
    final_heirlooms: Query<&ItemChestFinalHeirloom>,
    merchant_hovers: Query<&MerchantHeirloomHover>,
    skill_hud_icons: Query<&SkillHudIcon>,
    microwave_buttons: Query<&MicrowaveHeirloomButton>,
    skill_choice_cards: Query<&SkillChoiceUI>,
    blessing_cards: Query<&BlessingChoiceUI>,
    candidates: Query<
        (Entity, &TextureAtlasSprite, &Handle<TextureAtlas>),
        (
            Without<HeirloomIconOutline>,
            Without<Mesh2dHandle>,
            Without<ItemDrop>,
            Without<CustomCursor>,
            Without<Player>,
            Without<AsepriteAnimation>,
        ),
    >,
) {
    if lookup.indices.is_empty() {
        return;
    }
    let Some(main_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };

    let player_skills = player_skills.get_single().ok();

    for (entity, sprite, atlas_handle) in &candidates {
        // Atlas indices are only meaningful within a specific sheet. Aseprite entities
        // (player, enemies, pets) reuse small indices in their own atlases and must
        // never be matched by heirloom icon indices from the main world/UI atlas.
        if atlas_handle != main_atlas {
            continue;
        }
        if !lookup.indices.contains(&sprite.index) {
            continue;
        }
        let Some(heirloom) = lookup.index_to_heirloom.get(&sprite.index) else {
            continue;
        };

        let rarity = resolve_heirloom_outline_rarity(
            entity,
            heirloom,
            player_skills,
            choice_queue.as_deref(),
            item_chest_state.as_deref(),
            &parents,
            &crystal_icons,
            &banish_icons,
            &final_heirlooms,
            &merchant_hovers,
            &skill_hud_icons,
            &microwave_buttons,
            &skill_choice_cards,
            &blessing_cards,
        );

        let style = if skill_hud_icons.get(entity).is_ok() {
            HeirloomIconOutlineStyle::Hud
        } else {
            HeirloomIconOutlineStyle::Default
        };

        commands
            .entity(entity)
            .insert(HeirloomIconOutline::new(rarity, style));
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_heirloom_outline_rarity(
    entity: Entity,
    heirloom: &Heirloom,
    player_skills: Option<&PlayerSkills>,
    choice_queue: Option<&HeirloomChoiceQueue>,
    item_chest_state: Option<&ItemChestState>,
    parents: &Query<&Parent>,
    crystal_icons: &Query<&CrystalUnlockIcon>,
    banish_icons: &Query<&BanishTrackerIcon>,
    final_heirlooms: &Query<&ItemChestFinalHeirloom>,
    merchant_hovers: &Query<&MerchantHeirloomHover>,
    skill_hud_icons: &Query<&SkillHudIcon>,
    microwave_buttons: &Query<&MicrowaveHeirloomButton>,
    skill_choice_cards: &Query<&SkillChoiceUI>,
    blessing_cards: &Query<&BlessingChoiceUI>,
) -> HeirloomRarity {
    if let Ok(icon) = crystal_icons.get(entity) {
        if icon.heirloom == *heirloom {
            return icon.rarity;
        }
    }
    if let Ok(icon) = banish_icons.get(entity) {
        if icon.heirloom == *heirloom {
            return icon.rarity;
        }
    }
    if let Ok(final_heirloom) = final_heirlooms.get(entity) {
        if final_heirloom.heirloom.heirloom == *heirloom {
            return final_heirloom.heirloom.rarity;
        }
    }
    if let Ok(hover) = merchant_hovers.get(entity) {
        if hover.heirloom == *heirloom {
            return hover.rarity;
        }
    }
    if let Ok(hud_icon) = skill_hud_icons.get(entity) {
        if hud_icon.0 == *heirloom {
            if let Some(skills) = player_skills {
                if let Some(rarity) = skills.get_heirloom_rarity(heirloom.clone()) {
                    return rarity;
                }
            }
        }
    }
    if let Ok(button) = microwave_buttons.get(entity) {
        if button.heirloom == *heirloom {
            if let Some(skills) = player_skills {
                if let Some(rarity) = skills.get_heirloom_rarity(heirloom.clone()) {
                    return rarity;
                }
            }
        }
    }

    if let Ok(parent) = parents.get(entity) {
        let parent_entity = parent.get();
        if let Ok(hover) = merchant_hovers.get(parent_entity) {
            if hover.heirloom == *heirloom {
                return hover.rarity;
            }
        }
        if let Ok(card) = skill_choice_cards.get(parent_entity) {
            if card.skill_choice.heirloom == *heirloom {
                return card.skill_choice.rarity;
            }
        }
        if let Ok(card) = blessing_cards.get(parent_entity) {
            if let Some(resolved) = card.choice.resolved_heirloom.as_ref() {
                if resolved.heirloom == *heirloom {
                    return resolved.rarity;
                }
            }
        }
    }

    if let Some(state) = item_chest_state {
        if state.current_heirloom.as_ref() == Some(heirloom) {
            if let Some(rarity) = state.target_heirloom_rarity {
                return rarity;
            }
        }
    }

    if let Some(queue) = choice_queue {
        for row in &queue.queue {
            for choice in row {
                if choice.heirloom == *heirloom {
                    return choice.rarity;
                }
            }
        }
        for choice in &queue.pool {
            if choice.heirloom == *heirloom {
                return choice.rarity;
            }
        }
    }

    if let Some(skills) = player_skills {
        if let Some(rarity) = skills.get_heirloom_rarity(heirloom.clone()) {
            return rarity;
        }
    }

    HeirloomRarity::Common
}

fn refresh_outlined_cursor_on_settings_change(
    cursor_color: Res<CursorColorSettings>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    cursors: Query<Entity, (With<CustomCursor>, With<Mesh2dHandle>)>,
) {
    if !cursor_color.is_changed() {
        return;
    }
    let Some(texture_atlas) = graphics.texture_atlas.as_ref() else {
        return;
    };
    let Some(base_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        return;
    };
    let mut sprite = base_sprite.clone();
    CursorColorSettings::apply_sprite_settings(
        &mut sprite,
        &base_sprite,
        cursor_color.double_size,
    );

    for entity in &cursors {
        commands
            .entity(entity)
            .remove::<Mesh2dHandle>()
            .remove::<Handle<AtlasSpriteOutlineMaterial>>()
            .insert((sprite.clone(), texture_atlas.clone()));
    }
}

fn apply_atlas_sprite_outlines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasSpriteOutlineMaterial>>,
    mut state: ResMut<AtlasSpriteOutlineState>,
    atlases: Res<Assets<TextureAtlas>>,
    item_drops: Query<
        (Entity, &TextureAtlasSprite, &Handle<TextureAtlas>),
        (With<ItemDrop>, Without<Mesh2dHandle>),
    >,
    heirloom_icons: Query<
        (
            Entity,
            &TextureAtlasSprite,
            &Handle<TextureAtlas>,
            &HeirloomIconOutline,
        ),
        Without<Mesh2dHandle>,
    >,
    cursors: Query<
        (Entity, &TextureAtlasSprite, &Handle<TextureAtlas>),
        (With<CustomCursor>, Without<Mesh2dHandle>),
    >,
) {
    for (entity, sprite, atlas_handle) in &item_drops {
        apply_outline_to_entity(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            &atlases,
            entity,
            sprite,
            atlas_handle,
            DEFAULT_OUTLINE_COLOR,
        );
    }

    for (entity, sprite, atlas_handle, outline) in &heirloom_icons {
        apply_outline_to_entity(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            &atlases,
            entity,
            sprite,
            atlas_handle,
            outline.outline_color(),
        );
    }

    for (entity, sprite, atlas_handle) in &cursors {
        apply_outline_to_entity(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            &atlases,
            entity,
            sprite,
            atlas_handle,
            DEFAULT_OUTLINE_COLOR,
        );
    }
}

fn apply_outline_to_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<AtlasSpriteOutlineMaterial>,
    state: &mut AtlasSpriteOutlineState,
    atlases: &Assets<TextureAtlas>,
    entity: Entity,
    sprite: &TextureAtlasSprite,
    atlas_handle: &Handle<TextureAtlas>,
    outline_color: Color,
) {
    let Some(atlas) = atlases.get(atlas_handle) else {
        return;
    };

    let rect = atlas.textures[sprite.index];
    let atlas_size = atlas.size;
    let uv_bounds = Vec4::new(
        rect.min.x / atlas_size.x,
        rect.min.y / atlas_size.y,
        rect.max.x / atlas_size.x,
        rect.max.y / atlas_size.y,
    );
    let outline_key = color_cache_key(outline_color);

    let material = state
        .materials
        .entry((sprite.index, outline_key))
        .or_insert_with(|| {
            materials.add(AtlasSpriteOutlineMaterial {
                uv_bounds,
                outline_color: outline_color.into(),
                source_texture: Some(atlas.texture.clone()),
            })
        })
        .clone();

    let mesh = mesh_from_atlas_sprite(meshes, atlas, sprite);

    // Insert only the mesh + material, NOT the full MaterialMesh2dBundle.
    // The bundle carries a default Transform/Visibility which would clobber
    // the entity's real spawn position and rotation.
    commands
        .entity(entity)
        .insert((mesh, material))
        .remove::<TextureAtlasSprite>()
        .remove::<Handle<TextureAtlas>>();
}

fn color_cache_key(color: Color) -> u32 {
    let [r, g, b, a]: [f32; 4] = color.into();
    let to_byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    to_byte(r) | (to_byte(g) << 8) | (to_byte(b) << 16) | (to_byte(a) << 24)
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
