use bevy::camera::visibility::RenderLayers;
use bevy::mesh::Mesh2d;
use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d};
use bevy_aseprite_ultra::prelude::AseAnimation;

use crate::assets::Graphics;
use crate::assets::SpriteAnchor;
use crate::blessings::BlessingChoiceUI;
use crate::colors::{overwrite_alpha, RED, YELLOW_2};
use crate::cursor::{CursorColorSettings, CustomCursor};
use crate::ecs_helpers::safe_add_child;
use crate::item::shrine_visuals::{
    shrine_is_consumed, uses_standalone_shrine_texture, ShrineEyeDoneVisual, ShrineNeedsRepair,
};
use crate::item::{ItemDrop, WorldObject};
use crate::player::skills::{Heirloom, HeirloomChoiceQueue, HeirloomRarity, PlayerSkills};
use crate::player::Player;
use crate::ui::item_chest::{ItemChestFinalHeirloom, ItemChestState};
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::ui::player_hud::SkillHudIcon;
use crate::ui::{
    BanishTrackerIcon, CrystalUnlockIcon, MerchantHeirloomHover, MicrowaveHeirloomButton,
    SkillChoiceUI,
};

/// Alpha used by item-drop and cursor outlines (#cdceee at 0.22).
pub const DEFAULT_OUTLINE_ALPHA: f32 = 0.22;
/// Stronger heirloom outline for HUD row icons.
pub const HUD_HEIRLOOM_OUTLINE_ALPHA: f32 = 0.42;
/// Softer heirloom outline on tooltip cards (hover + level-up choice cards).
pub const TOOLTIP_CARD_HEIRLOOM_OUTLINE_ALPHA: f32 = 0.12;

/// Default outline for floor item drops and the in-game cursor (#cdceee).
pub const DEFAULT_OUTLINE_COLOR: Color = Color::srgba(0.804, 0.808, 0.933, DEFAULT_OUTLINE_ALPHA);

/// Near-black shadow tint for UI container / tooltip outlines.
pub const UI_SHADOW_OUTLINE_COLOR_RGB: (f32, f32, f32) = (0.0, 0.0, 0.0);
/// Inner-ring alpha for container panel shadows.
pub const UI_CONTAINER_SHADOW_ALPHA: f32 = 0.88;
/// Inner-ring alpha for tooltip card shadows (a touch softer than containers).
pub const UI_TOOLTIP_SHADOW_ALPHA: f32 = 0.98;
/// Each successive shadow ring draws at this fraction of the previous ring's alpha
/// (0.5 => 100%, 50%, 25%, ...).
pub const UI_SHADOW_RING_FALLOFF: f32 = 0.65;
/// Ring counts: containers get 2, tooltip hovers get 3.
pub const UI_CONTAINER_SHADOW_RINGS: u32 = 3;
pub const UI_TOOLTIP_SHADOW_RINGS: u32 = 7;

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

/// Parent already has a non-destructive outline child (keeps [`Sprite`] for hit-testing).
#[derive(Component)]
pub struct HeirloomIconOutlineApplied;

/// Child mesh spawned behind a [`HeirloomIconOutline`] sprite.
#[derive(Component)]
pub struct HeirloomIconOutlineChild;

/// Floor [`ItemDrop`] already has a non-destructive outline child.
///
/// Drops must keep their [`Sprite`]: converting them to `Mesh2d` alone put them in a
/// different 2D pass than world sprites (shrines, trees), so tall art painted over
/// pickable rewards even when XY cleared the silhouette.
#[derive(Component)]
pub struct ItemDropOutlineApplied;

/// Outline mesh child behind an [`ItemDrop`] sprite.
#[derive(Component)]
pub struct ItemDropOutlineChild;

/// Local Z of the item-drop outline child — behind the parent sprite so only rings show.
const ITEM_DROP_OUTLINE_CHILD_Z: f32 = -0.1;

impl HeirloomIconOutline {
    pub fn new(rarity: HeirloomRarity, style: HeirloomIconOutlineStyle) -> Self {
        Self { rarity, style }
    }

    pub fn outline_color(&self) -> Color {
        overwrite_alpha(
            heirloom_rarity_outline_base(self.rarity),
            self.style.alpha(),
        )
    }
}

/// Saturated outline tints derived from each rarity's text hue family. Text title
/// colors are intentionally pastel; at low outline alpha they wash out to white.
fn heirloom_rarity_outline_base(rarity: HeirloomRarity) -> Color {
    match rarity {
        HeirloomRarity::Common => Color::srgba(148. / 255., 152. / 255., 178. / 255., 1.),
        HeirloomRarity::Uncommon => Color::srgba(58. / 255., 188. / 255., 172. / 255., 1.),
        HeirloomRarity::Rare => Color::srgba(204. / 255., 88. / 255., 228. / 255., 1.),
        HeirloomRarity::Legendary => Color::srgba(245. / 255., 158. / 255., 24. / 255., 1.),
    }
}

/// Attach to any textured UI sprite (`Sprite` + `Handle<Image>`) to give it a soft
/// drop-shadow outline. This is **non-destructive**: a separate child mesh entity is
/// spawned *behind* the sprite that renders only the shadow rings, so the original
/// sprite keeps its `Sprite`/`Handle<Image>` (hover tinting, texture swaps, and
/// interaction hit-detection all keep working).
#[derive(Component, Clone, Copy, Debug)]
pub struct UiShadow {
    pub color: Color,
    /// Number of shadow rings (1..=4).
    pub ring_count: u32,
    /// Per-ring alpha multiplier.
    pub falloff: f32,
}

impl UiShadow {
    /// Standard 2-ring shadow for panels / buttons / cards.
    pub fn container() -> Self {
        let (r, g, b) = UI_SHADOW_OUTLINE_COLOR_RGB;
        Self {
            color: Color::srgba(r, g, b, UI_CONTAINER_SHADOW_ALPHA),
            ring_count: UI_CONTAINER_SHADOW_RINGS,
            falloff: UI_SHADOW_RING_FALLOFF,
        }
    }
    pub fn hud() -> Self {
        let (r, g, b) = UI_SHADOW_OUTLINE_COLOR_RGB;
        Self {
            color: Color::srgba(r, g, b, UI_CONTAINER_SHADOW_ALPHA),
            ring_count: UI_CONTAINER_SHADOW_RINGS,
            falloff: UI_SHADOW_RING_FALLOFF - 0.2,
        }
    }

    /// Softer 3-ring shadow for tooltip / hover cards.
    pub fn tooltip_card() -> Self {
        let (r, g, b) = UI_SHADOW_OUTLINE_COLOR_RGB;
        Self {
            color: Color::srgba(r, g, b, UI_TOOLTIP_SHADOW_ALPHA),
            ring_count: UI_TOOLTIP_SHADOW_RINGS,
            falloff: UI_SHADOW_RING_FALLOFF,
        }
    }
}

/// Yellow "valid upgrade target" highlight (Track 4 controller carry): while carrying an
/// `UpgradeTome` / `OrbOfTransformation` via focus navigation, every inventory item icon that
/// the material can be applied to gets a bright 1px outline so gamepad players can see where the
/// upgrade will land (the mouse relies on hovering, which the controller can't do).
pub const UPGRADE_TARGET_OUTLINE_COLOR: Color = Color::srgba(1.0, 0.85, 0.15, 1.0);

/// Marker on an item-icon entity that currently has a spawned upgrade-target highlight child.
#[derive(Component)]
pub struct UpgradeTargetHighlighted;

/// Marker on the spawned child mesh that draws the yellow upgrade-target outline.
#[derive(Component)]
pub struct UpgradeTargetHighlightChild;

/// Caches yellow upgrade-highlight materials per atlas sprite index.
#[derive(Resource, Default)]
pub struct UpgradeHighlightState {
    pub materials: HashMap<usize, Handle<AtlasSpriteOutlineMaterial>>,
}

/// Marks a sprite whose child shadow has already been spawned (so we don't duplicate it).
#[derive(Component)]
pub struct UiShadowApplied;

/// Marks the spawned child shadow mesh entity.
#[derive(Component)]
pub struct UiShadowChild;

/// Caches outline materials per atlas sprite index + outline color.
#[derive(Resource, Default)]
pub struct AtlasSpriteOutlineState {
    pub materials: HashMap<(usize, u32), Handle<AtlasSpriteOutlineMaterial>>,
}

/// Caches shadow materials per (image, color, ring_count) so identical panels share one.
#[derive(Resource, Default)]
pub struct UiShadowState {
    pub materials: HashMap<(Handle<Image>, u32, u32), Handle<AtlasSpriteOutlineMaterial>>,
}

/// Maps atlas sprite indices back to heirlooms for auto-tagging icon sprites.
#[derive(Resource, Default)]
pub struct HeirloomIconAtlasLookup {
    pub index_to_heirloom: HashMap<usize, Heirloom>,
    pub indices: HashSet<usize>,
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct AtlasSpriteOutlineMaterial {
    /// Sub-rect of the atlas this sprite occupies, in UV space:
    /// (min_u, min_v, max_u, max_v). Neighbor samples outside this are ignored.
    #[uniform(0)]
    pub uv_bounds: Vec4,
    #[uniform(3)]
    pub outline_color: Vec4,
    /// `(ring_count, falloff, shadow_only, _)` — see the shader header.
    #[uniform(4)]
    pub ring_params: Vec4,
    #[texture(1)]
    #[sampler(2)]
    pub source_texture: Option<Handle<Image>>,
}

impl Material2d for AtlasSpriteOutlineMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/item_drop_outline.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

pub struct ItemDropOutlinePlugin;

impl Plugin for ItemDropOutlinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AtlasSpriteOutlineState>()
            .init_resource::<UiShadowState>()
            .init_resource::<UpgradeHighlightState>()
            .init_resource::<ShrineProximityOutlineState>()
            .init_resource::<HeirloomIconAtlasLookup>()
            .add_plugins(Material2dPlugin::<AtlasSpriteOutlineMaterial>::default())
            .add_systems(Update, init_heirloom_icon_atlas_lookup)
            .add_systems(
                Update,
                tag_heirloom_icon_outlines.before(apply_atlas_sprite_outlines),
            )
            .add_systems(
                Update,
                refresh_outlined_cursor_on_settings_change.before(apply_atlas_sprite_outlines),
            )
            .add_systems(Update, apply_atlas_sprite_outlines)
            .add_systems(Update, update_upgrade_target_highlights)
            .add_systems(Update, update_shrine_proximity_outlines)
            .add_systems(Update, spawn_ui_shadows);
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
        let Some(atlas) = sprite.texture_atlas.as_ref() else {
            continue;
        };
        lookup.indices.insert(atlas.index);
        lookup
            .index_to_heirloom
            .insert(atlas.index, heirloom.clone());
    }
}

fn tag_heirloom_icon_outlines(
    mut commands: Commands,
    graphics: Res<Graphics>,
    lookup: Res<HeirloomIconAtlasLookup>,
    player_skills: Query<&PlayerSkills>,
    choice_queue: Option<Res<HeirloomChoiceQueue>>,
    item_chest_state: Option<Res<ItemChestState>>,
    parents: Query<&ChildOf>,
    crystal_icons: Query<&CrystalUnlockIcon>,
    banish_icons: Query<&BanishTrackerIcon>,
    final_heirlooms: Query<&ItemChestFinalHeirloom>,
    merchant_hovers: Query<&MerchantHeirloomHover>,
    skill_hud_icons: Query<&SkillHudIcon>,
    microwave_buttons: Query<&MicrowaveHeirloomButton>,
    skill_choice_cards: Query<&SkillChoiceUI>,
    blessing_cards: Query<&BlessingChoiceUI>,
    candidates: Query<
        (Entity, &Sprite),
        (
            Without<HeirloomIconOutline>,
            Without<Mesh2d>,
            Without<ItemDrop>,
            Without<CustomCursor>,
            Without<Player>,
            Without<AseAnimation>,
        ),
    >,
) {
    if lookup.indices.is_empty() {
        return;
    }
    let Some(main_atlas) = graphics.texture_atlas_layout.as_ref() else {
        return;
    };

    let player_skills = player_skills.single().ok();

    for (entity, sprite) in &candidates {
        // Atlas indices are only meaningful within a specific sheet. Aseprite entities
        // (player, enemies, pets) reuse small indices in their own atlases and must
        // never be matched by heirloom icon indices from the main world/UI atlas.
        let Some(atlas) = sprite.texture_atlas.as_ref() else {
            continue;
        };
        if &atlas.layout != main_atlas {
            continue;
        }
        if !lookup.indices.contains(&atlas.index) {
            continue;
        }
        let Some(heirloom) = lookup.index_to_heirloom.get(&atlas.index) else {
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

        // Chest take/banish (and other UI teardown) may despawn this icon in the
        // same frame after we queued the insert — Bevy 0.19 panics on apply.
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.try_insert(HeirloomIconOutline::new(rarity, style));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_heirloom_outline_rarity(
    entity: Entity,
    heirloom: &Heirloom,
    player_skills: Option<&PlayerSkills>,
    choice_queue: Option<&HeirloomChoiceQueue>,
    item_chest_state: Option<&ItemChestState>,
    parents: &Query<&ChildOf>,
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
        let parent_entity = parent.parent();
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
            if let Some(minor) = card.choice.as_minor() {
                if let Some(resolved) = minor.resolved_heirloom.as_ref() {
                    if resolved.heirloom == *heirloom {
                        return resolved.rarity;
                    }
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
    cursors: Query<Entity, (With<CustomCursor>, With<Mesh2d>)>,
) {
    if !cursor_color.is_changed() {
        return;
    }
    if graphics.texture_atlas_layout.is_none() || graphics.texture_atlas_image.is_none() {
        return;
    }
    let Some(base_sprite) = graphics.get_cursor_color_sprite(cursor_color.index) else {
        return;
    };
    let mut sprite = base_sprite.clone();
    CursorColorSettings::apply_sprite_settings(&mut sprite, &base_sprite, cursor_color.double_size);

    for entity in &cursors {
        commands
            .entity(entity)
            .remove::<Mesh2d>()
            .remove::<MeshMaterial2d<AtlasSpriteOutlineMaterial>>()
            .insert(sprite.clone());
    }
}

/// Local Z of the shadow child relative to its parent sprite. Negative so the
/// shadow renders *behind* the panel and only its rings peek out around the edges.
const UI_SHADOW_CHILD_Z: f32 = -0.5;

/// Spawns a non-destructive child shadow mesh behind every `UiShadow`-tagged sprite.
fn spawn_ui_shadows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasSpriteOutlineMaterial>>,
    mut state: ResMut<UiShadowState>,
    images: Res<Assets<Image>>,
    tagged: Query<(Entity, &Sprite, &UiShadow, Option<&RenderLayers>), Without<UiShadowApplied>>,
) {
    for (entity, sprite, shadow, render_layers) in tagged.iter() {
        let image_handle = &sprite.image;
        let Some(image) = images.get(image_handle) else {
            // Texture not loaded yet; retry next frame (no `UiShadowApplied` inserted).
            continue;
        };

        let ring_count = shadow.ring_count.clamp(1, 4);
        let color_key = color_cache_key(shadow.color);
        let falloff_key = (shadow.falloff.clamp(0.0, 1.0) * 255.0).round() as u32;
        let variant_key = ring_count | (falloff_key << 8);

        let material = state
            .materials
            .entry((image_handle.clone(), color_key, variant_key))
            .or_insert_with(|| {
                materials.add(AtlasSpriteOutlineMaterial {
                    uv_bounds: Vec4::new(0., 0., 1., 1.),
                    outline_color: outline_color_uniform(shadow.color),
                    ring_params: Vec4::new(ring_count as f32, shadow.falloff, 1.0, 0.0),
                    source_texture: Some(image_handle.clone()),
                })
            })
            .clone();

        let mesh = mesh_from_standalone_image(&mut meshes, image, sprite, ring_count as f32);
        let layers = render_layers
            .cloned()
            .unwrap_or_else(|| RenderLayers::layer(3));

        let child = commands
            .spawn((
                mesh,
                MeshMaterial2d(material),
                (
                    Transform::from_xyz(0., 0., UI_SHADOW_CHILD_Z),
                    Visibility::default(),
                ),
                layers,
                UiShadowChild,
                Name::new("UI Shadow"),
            ))
            .id();
        safe_add_child(&mut commands, entity, child);
        // Parent may have been despawned this frame (e.g. inventory UI teardown).
        // `get_entity` only checks at queue time — use try_insert so apply is silenced.
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.try_insert(UiShadowApplied);
        }
    }
}

fn apply_atlas_sprite_outlines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasSpriteOutlineMaterial>>,
    mut state: ResMut<AtlasSpriteOutlineState>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    item_drops: Query<(Entity, &Sprite), (With<ItemDrop>, Without<ItemDropOutlineApplied>)>,
    heirloom_icons: Query<
        (Entity, &Sprite, &HeirloomIconOutline, Option<&RenderLayers>),
        (Without<Mesh2d>, Without<HeirloomIconOutlineApplied>),
    >,
    cursors: Query<(Entity, &Sprite), (With<CustomCursor>, Without<Mesh2d>)>,
) {
    for (entity, sprite) in &item_drops {
        apply_item_drop_outline_child(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            &layouts,
            entity,
            sprite,
            DEFAULT_OUTLINE_COLOR,
        );
    }

    for (entity, sprite, outline, render_layers) in &heirloom_icons {
        // Keep `Sprite` on the icon — hover/tooltips (time-crystal tracker, HUD, etc.)
        // hit-test against `Sprite::custom_size`. Destructive Mesh2d conversion broke that.
        apply_heirloom_icon_outline_child(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            &layouts,
            entity,
            sprite,
            outline.outline_color(),
            render_layers.cloned(),
        );
    }

    for (entity, sprite) in &cursors {
        apply_outline_to_entity(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            &layouts,
            entity,
            sprite,
            DEFAULT_OUTLINE_COLOR,
        );
    }
}

/// Destructive outline for the cursor only: replace [`Sprite`] with `Mesh2d`.
/// Floor drops must use [`apply_item_drop_outline_child`] instead.
fn apply_outline_to_entity(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<AtlasSpriteOutlineMaterial>,
    state: &mut AtlasSpriteOutlineState,
    layouts: &Assets<TextureAtlasLayout>,
    entity: Entity,
    sprite: &Sprite,
    outline_color: Color,
) {
    let Some(atlas) = sprite.texture_atlas.as_ref() else {
        return;
    };
    let Some(layout) = layouts.get(&atlas.layout) else {
        return;
    };

    let Some(rect) = layout.textures.get(atlas.index).copied() else {
        return;
    };
    let atlas_size = layout.size.as_vec2();
    let uv_bounds = Vec4::new(
        rect.min.x as f32 / atlas_size.x,
        rect.min.y as f32 / atlas_size.y,
        rect.max.x as f32 / atlas_size.x,
        rect.max.y as f32 / atlas_size.y,
    );
    let outline_key = color_cache_key(outline_color);

    let material = state
        .materials
        .entry((atlas.index, outline_key))
        .or_insert_with(|| {
            materials.add(AtlasSpriteOutlineMaterial {
                uv_bounds,
                outline_color: outline_color_uniform(outline_color),
                ring_params: Vec4::new(1.0, 0.0, 0.0, 0.0),
                source_texture: Some(sprite.image.clone()),
            })
        })
        .clone();

    let mesh = mesh_from_atlas_sprite(meshes, layout, sprite);

    // Insert only the mesh + material, NOT the full MaterialMesh2dBundle.
    // The bundle carries a default Transform/Visibility which would clobber
    // the entity's real spawn position and rotation.
    commands
        .entity(entity)
        .insert((mesh, MeshMaterial2d(material)))
        .remove::<Sprite>();
}

/// Non-destructive floor-drop outline: parent keeps [`Sprite`] (same pass as shrines / YSort).
fn apply_item_drop_outline_child(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<AtlasSpriteOutlineMaterial>,
    state: &mut AtlasSpriteOutlineState,
    layouts: &Assets<TextureAtlasLayout>,
    entity: Entity,
    sprite: &Sprite,
    outline_color: Color,
) {
    let Some(atlas) = sprite.texture_atlas.as_ref() else {
        return;
    };
    let Some(layout) = layouts.get(&atlas.layout) else {
        return;
    };

    let Some(rect) = layout.textures.get(atlas.index).copied() else {
        return;
    };
    let atlas_size = layout.size.as_vec2();
    let uv_bounds = Vec4::new(
        rect.min.x as f32 / atlas_size.x,
        rect.min.y as f32 / atlas_size.y,
        rect.max.x as f32 / atlas_size.x,
        rect.max.y as f32 / atlas_size.y,
    );
    let outline_key = color_cache_key(outline_color);

    let material = state
        .materials
        .entry((atlas.index, outline_key))
        .or_insert_with(|| {
            materials.add(AtlasSpriteOutlineMaterial {
                uv_bounds,
                outline_color: outline_color_uniform(outline_color),
                // Same as cursor/heirloom cache entries (shadow_only = 0). Parent Sprite
                // covers the child interior; only the outline rings peek out.
                ring_params: Vec4::new(1.0, 0.0, 0.0, 0.0),
                source_texture: Some(sprite.image.clone()),
            })
        })
        .clone();

    let mesh = mesh_from_atlas_sprite(meshes, layout, sprite);
    let child = commands
        .spawn((
            mesh,
            MeshMaterial2d(material),
            (
                Transform::from_xyz(0., 0., ITEM_DROP_OUTLINE_CHILD_Z),
                Visibility::default(),
            ),
            ItemDropOutlineChild,
            Name::new("Item Drop Outline"),
        ))
        .id();
    safe_add_child(commands, entity, child);
    if let Ok(mut entity_commands) = commands.get_entity(entity) {
        entity_commands.try_insert(ItemDropOutlineApplied);
    }
}

/// Non-destructive outline for UI heirloom icons: parent keeps [`Sprite`] for pointcasts.
fn apply_heirloom_icon_outline_child(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<AtlasSpriteOutlineMaterial>,
    state: &mut AtlasSpriteOutlineState,
    layouts: &Assets<TextureAtlasLayout>,
    entity: Entity,
    sprite: &Sprite,
    outline_color: Color,
    render_layers: Option<RenderLayers>,
) {
    let Some(atlas) = sprite.texture_atlas.as_ref() else {
        return;
    };
    let Some(layout) = layouts.get(&atlas.layout) else {
        return;
    };

    let Some(rect) = layout.textures.get(atlas.index).copied() else {
        return;
    };
    let atlas_size = layout.size.as_vec2();
    let uv_bounds = Vec4::new(
        rect.min.x as f32 / atlas_size.x,
        rect.min.y as f32 / atlas_size.y,
        rect.max.x as f32 / atlas_size.x,
        rect.max.y as f32 / atlas_size.y,
    );
    let outline_key = color_cache_key(outline_color);

    let material = state
        .materials
        .entry((atlas.index, outline_key))
        .or_insert_with(|| {
            materials.add(AtlasSpriteOutlineMaterial {
                uv_bounds,
                outline_color: outline_color_uniform(outline_color),
                ring_params: Vec4::new(1.0, 0.0, 0.0, 0.0),
                source_texture: Some(sprite.image.clone()),
            })
        })
        .clone();

    let mesh = mesh_from_atlas_sprite(meshes, layout, sprite);
    let layers = render_layers.unwrap_or_else(|| RenderLayers::layer(3));
    let child = commands
        .spawn((
            mesh,
            MeshMaterial2d(material),
            (Transform::from_xyz(0., 0., -0.1), Visibility::default()),
            layers,
            HeirloomIconOutlineChild,
            Name::new("Heirloom Icon Outline"),
        ))
        .id();
    safe_add_child(commands, entity, child);
    // Same deferred-despawn race as UI shadows — never panic on apply.
    if let Ok(mut entity_commands) = commands.get_entity(entity) {
        entity_commands.try_insert(HeirloomIconOutlineApplied);
    }
}

/// Proximity outline for overworld shrine body art (not eye / ring children).
/// Yellow when healthy; font [`RED`] when the shrine needs repair.
pub const SHRINE_PROXIMITY_OUTLINE_ALPHA: f32 = 0.85;
/// Inner ring at full outline alpha; outer ring at `falloff` of that (0.5 => 50%).
pub const SHRINE_PROXIMITY_OUTLINE_FALLOFF: f32 = 0.4;
pub const SHRINE_PROXIMITY_OUTLINE_RINGS: u32 = 3;
const SHRINE_PROXIMITY_OUTLINE_CHILD_Z: f32 = -0.0;
const DEFAULT_SHRINE_INTERACT_DISTANCE: f32 = 32.;

fn shrine_proximity_outline_color(needs_repair: bool) -> Color {
    let base = if needs_repair { RED } else { YELLOW_2 };
    let a = if needs_repair {
        1.0
    } else {
        SHRINE_PROXIMITY_OUTLINE_ALPHA
    };
    overwrite_alpha(base, a)
}

/// Marker on a shrine that currently has a proximity outline child.
#[derive(Component)]
pub struct ShrineProximityOutlined {
    pub needs_repair: bool,
}

/// Marker on the outline mesh child (silhouette rings only).
#[derive(Component)]
pub struct ShrineProximityOutlineChild;

/// Caches outline materials per shrine body texture.
#[derive(Resource, Default)]
pub struct ShrineProximityOutlineState {
    pub materials: HashMap<(Handle<Image>, u32), Handle<AtlasSpriteOutlineMaterial>>,
}

fn update_shrine_proximity_outlines(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasSpriteOutlineMaterial>>,
    mut state: ResMut<ShrineProximityOutlineState>,
    images: Res<Assets<Image>>,
    player_query: Query<&GlobalTransform, With<Player>>,
    shrines: Query<(
        Entity,
        &WorldObject,
        &GlobalTransform,
        &Sprite,
        Option<&SpriteAnchor>,
        Option<&InteractionGuideTrigger>,
        Option<&ShrineNeedsRepair>,
        Option<&Children>,
    )>,
    outlined: Query<&ShrineProximityOutlined>,
    outline_children: Query<(), With<ShrineProximityOutlineChild>>,
    changed_images: Query<Entity, (With<ShrineProximityOutlined>, Changed<Sprite>)>,
    done_eyes: Query<(), With<ShrineEyeDoneVisual>>,
) {
    let Ok(player_t) = player_query.single() else {
        return;
    };
    let player_pos = player_t.translation().truncate();

    for (entity, obj, gtf, sprite, anchor, guide, needs_repair, children) in shrines.iter() {
        if !uses_standalone_shrine_texture(obj) {
            continue;
        }

        let eye_done = children.map_or(false, |c| {
            c.iter().any(|child| done_eyes.get(child).is_ok())
        });
        let shrine_done = shrine_is_consumed(obj) || eye_done;
        let needs_repair = needs_repair.is_some();
        let outline_color = shrine_proximity_outline_color(needs_repair);
        let color_key = color_cache_key(outline_color);

        let feet = gtf.translation().truncate() - anchor.map(|a| a.0).unwrap_or(Vec2::ZERO);
        let range = guide
            .map(|g| g.activation_distance)
            .unwrap_or(DEFAULT_SHRINE_INTERACT_DISTANCE);
        let in_range = feet.distance(player_pos) < range;
        let already = outlined.get(entity).ok();
        let texture_changed = changed_images.get(entity).is_ok();
        let repair_state_changed = already.map_or(false, |o| o.needs_repair != needs_repair);

        if already.is_some()
            && (!in_range || texture_changed || shrine_done || repair_state_changed)
        {
            despawn_shrine_proximity_outline_children(
                &mut commands,
                entity,
                children,
                &outline_children,
            );
            commands.entity(entity).remove::<ShrineProximityOutlined>();
            if !in_range || shrine_done {
                continue;
            }
            // Fall through to respawn (new texture and/or repair color).
        } else if !in_range || shrine_done {
            continue;
        } else if already.is_some() {
            continue;
        }

        let image_handle = &sprite.image;
        let Some(image) = images.get(image_handle) else {
            continue;
        };

        let material = state
            .materials
            .entry((image_handle.clone(), color_key))
            .or_insert_with(|| {
                materials.add(AtlasSpriteOutlineMaterial {
                    uv_bounds: Vec4::new(0., 0., 1., 1.),
                    outline_color: outline_color_uniform(outline_color),
                    ring_params: Vec4::new(
                        SHRINE_PROXIMITY_OUTLINE_RINGS as f32,
                        SHRINE_PROXIMITY_OUTLINE_FALLOFF,
                        1.0, // shadow_only — rings only; shrine Sprite stays on the parent
                        0.0,
                    ),
                    source_texture: Some(image_handle.clone()),
                })
            })
            .clone();

        let mesh = mesh_from_standalone_image(
            &mut meshes,
            image,
            sprite,
            SHRINE_PROXIMITY_OUTLINE_RINGS as f32,
        );
        let child = commands
            .spawn((
                mesh,
                MeshMaterial2d(material),
                (
                    Transform::from_xyz(0., 0., SHRINE_PROXIMITY_OUTLINE_CHILD_Z),
                    Visibility::default(),
                ),
                ShrineProximityOutlineChild,
                Name::new("Shrine Proximity Outline"),
            ))
            .id();
        safe_add_child(&mut commands, entity, child);
        commands
            .entity(entity)
            .insert(ShrineProximityOutlined { needs_repair });
    }
}

fn despawn_shrine_proximity_outline_children(
    commands: &mut Commands,
    shrine: Entity,
    children: Option<&Children>,
    outline_children: &Query<(), With<ShrineProximityOutlineChild>>,
) {
    let Some(children) = children else {
        return;
    };
    for child in children.iter() {
        if outline_children.get(child).is_ok() {
            commands.entity(child).despawn();
        }
    }
}

/// Local Z of the highlight child relative to the item icon — just behind the icon so the
/// yellow rings peek out around the art without tinting the icon itself.
const UPGRADE_HIGHLIGHT_CHILD_Z: f32 = -0.2;

/// Spawns/removes the yellow "valid upgrade target" outline on inventory item icons while an
/// upgrade material is being carried via focus navigation.
#[allow(clippy::too_many_arguments)]
fn update_upgrade_target_highlights(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AtlasSpriteOutlineMaterial>>,
    mut state: ResMut<UpgradeHighlightState>,
    layouts: Res<Assets<TextureAtlasLayout>>,
    ui_state: Res<State<crate::ui::UIState>>,
    inv_state: Res<crate::ui::InventoryState>,
    dragged: Query<&crate::inventory::ItemStack, With<crate::ui::DraggedItem>>,
    slots: Query<&crate::ui::InventorySlotState>,
    icon_sprites: Query<(&Sprite, Option<&RenderLayers>)>,
    highlighted: Query<Entity, With<UpgradeTargetHighlighted>>,
    children_q: Query<&Children>,
    highlight_children: Query<(), With<UpgradeTargetHighlightChild>>,
) {
    let carrying_material = ui_state.is_inv_open()
        && dragged.iter().any(|s| {
            matches!(
                s.obj_type,
                crate::item::WorldObject::UpgradeTome
                    | crate::item::WorldObject::OrbOfTransformation
            )
        });

    let clear_all = |commands: &mut Commands| {
        for icon in highlighted.iter() {
            despawn_upgrade_highlight_children(commands, icon, &children_q, &highlight_children);
            commands.entity(icon).remove::<UpgradeTargetHighlighted>();
        }
    };

    if !carrying_material {
        clear_all(&mut commands);
        return;
    }

    // Collect icon entities of every slot that can currently receive the material.
    let mut valid_icons: bevy::platform::collections::HashSet<Entity> =
        bevy::platform::collections::HashSet::new();
    for slot in slots.iter() {
        let Some(icon) = slot.item else {
            continue;
        };
        if !crate::ui::upgrade_drag::slot_type_accepts_in_place_upgrade(slot.r#type) {
            continue;
        }
        if slot.r#type == crate::ui::InventorySlotType::Furnace && slot.slot_index == 0 {
            continue;
        }
        let Some(obj) = slot.obj_type else {
            continue;
        };
        if !crate::ui::upgrade_drag::is_upgradeable_equipment(obj, &inv_state) {
            continue;
        }
        valid_icons.insert(icon);
    }

    // Remove highlights that are no longer valid.
    for icon in highlighted.iter() {
        if !valid_icons.contains(&icon) {
            despawn_upgrade_highlight_children(
                &mut commands,
                icon,
                &children_q,
                &highlight_children,
            );
            commands.entity(icon).remove::<UpgradeTargetHighlighted>();
        }
    }

    // Add highlights to newly valid icons.
    for icon in valid_icons {
        if highlighted.get(icon).is_ok() {
            continue;
        }
        let Ok((sprite, render_layers)) = icon_sprites.get(icon) else {
            continue;
        };
        let Some(atlas) = sprite.texture_atlas.as_ref() else {
            continue;
        };
        let Some(layout) = layouts.get(&atlas.layout) else {
            continue;
        };

        let Some(rect) = layout.textures.get(atlas.index).copied() else {
            continue;
        };
        let atlas_size = layout.size.as_vec2();
        let uv_bounds = Vec4::new(
            rect.min.x as f32 / atlas_size.x,
            rect.min.y as f32 / atlas_size.y,
            rect.max.x as f32 / atlas_size.x,
            rect.max.y as f32 / atlas_size.y,
        );

        let material = state
            .materials
            .entry(atlas.index)
            .or_insert_with(|| {
                materials.add(AtlasSpriteOutlineMaterial {
                    uv_bounds,
                    outline_color: outline_color_uniform(UPGRADE_TARGET_OUTLINE_COLOR),
                    // shadow_only = 1.0 → draw only the outline rings, not the sprite itself.
                    ring_params: Vec4::new(1.0, 0.0, 1.0, 0.0),
                    source_texture: Some(sprite.image.clone()),
                })
            })
            .clone();

        let mesh = mesh_from_atlas_sprite(&mut meshes, layout, sprite);
        let layers = render_layers
            .cloned()
            .unwrap_or_else(|| RenderLayers::layer(3));
        let child = commands
            .spawn((
                mesh,
                MeshMaterial2d(material),
                (
                    Transform::from_xyz(0., 0., UPGRADE_HIGHLIGHT_CHILD_Z),
                    Visibility::default(),
                ),
                layers,
                UpgradeTargetHighlightChild,
                Name::new("Upgrade Target Highlight"),
            ))
            .id();
        safe_add_child(&mut commands, icon, child);
        commands.entity(icon).insert(UpgradeTargetHighlighted);
    }
}

fn despawn_upgrade_highlight_children(
    commands: &mut Commands,
    icon: Entity,
    children_q: &Query<&Children>,
    highlight_children: &Query<(), With<UpgradeTargetHighlightChild>>,
) {
    if let Ok(children) = children_q.get(icon) {
        for child in children.iter() {
            if highlight_children.get(child).is_ok() {
                commands.entity(child).despawn();
            }
        }
    }
}

/// World-space margin added around the sprite so the 1px outline has room to
/// draw even when the art touches its atlas cell edge. The matching UV margin
/// is one texel, so the quad stays at 1:1 pixel scale.
const OUTLINE_MARGIN_PX: f32 = 1.0;

fn outline_color_uniform(color: Color) -> Vec4 {
    let c = color.to_srgba();
    Vec4::new(c.red, c.green, c.blue, c.alpha)
}

fn color_cache_key(color: Color) -> u32 {
    let c = color.to_srgba();
    let to_byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    to_byte(c.red) | (to_byte(c.green) << 8) | (to_byte(c.blue) << 16) | (to_byte(c.alpha) << 24)
}

/// UV corners for Bevy 0.19 `Rectangle` meshes: vertices are TR, TL, BL, BR
/// with V=0 at +Y (top) and V=1 at -Y (bottom).
fn rectangle_uvs(lu: f32, ru: f32, v_at_top: f32, v_at_bottom: f32) -> Vec<[f32; 2]> {
    vec![
        [ru, v_at_top],    // TR
        [lu, v_at_top],    // TL
        [lu, v_at_bottom], // BL
        [ru, v_at_bottom], // BR
    ]
}

fn mesh_from_standalone_image(
    meshes: &mut Assets<Mesh>,
    image: &Image,
    sprite: &Sprite,
    ring_count: f32,
) -> Mesh2d {
    let dims = Vec2::new(
        image.texture_descriptor.size.width as f32,
        image.texture_descriptor.size.height as f32,
    );
    let texel = Vec2::new(1.0 / dims.x, 1.0 / dims.y);

    // The shadow needs `ring_count` px of margin so the outermost ring has room.
    let margin = ring_count.max(1.0);
    let size = sprite.custom_size.unwrap_or(dims) + Vec2::splat(margin * 2.0);

    let mut lu = 0.0 - margin * texel.x;
    let mut ru = 1.0 + margin * texel.x;
    let mut v_at_top = 0.0 - margin * texel.y;
    let mut v_at_bottom = 1.0 + margin * texel.y;

    if sprite.flip_x {
        std::mem::swap(&mut lu, &mut ru);
    }
    if sprite.flip_y {
        std::mem::swap(&mut v_at_top, &mut v_at_bottom);
    }

    let mut mesh = Mesh::from(Rectangle::new(size.x, size.y));
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        rectangle_uvs(lu, ru, v_at_top, v_at_bottom),
    );

    meshes.add(mesh).into()
}

fn mesh_from_atlas_sprite(
    meshes: &mut Assets<Mesh>,
    layout: &TextureAtlasLayout,
    sprite: &Sprite,
) -> Mesh2d {
    let Some(atlas) = sprite.texture_atlas.as_ref() else {
        return meshes.add(Mesh::from(Rectangle::new(1.0, 1.0))).into();
    };
    let Some(rect) = layout.textures.get(atlas.index).copied() else {
        return meshes.add(Mesh::from(Rectangle::new(1.0, 1.0))).into();
    };
    let atlas_size = layout.size.as_vec2();

    let texel = Vec2::new(1.0 / atlas_size.x, 1.0 / atlas_size.y);

    // Unexpanded sprite rect in UV space.
    let u0 = rect.min.x as f32 / atlas_size.x;
    let u1 = rect.max.x as f32 / atlas_size.x;
    let v0 = rect.min.y as f32 / atlas_size.y;
    let v1 = rect.max.y as f32 / atlas_size.y;

    // Expand UVs outward by one texel so the quad has a 1px margin ring. The
    // shader clamps neighbor sampling to `uv_bounds`, so this margin reads as
    // transparent for the base sprite and only ever shows the outline.
    let mut lu = u0 - texel.x;
    let mut ru = u1 + texel.x;
    let mut v_at_top = v0 - texel.y;
    let mut v_at_bottom = v1 + texel.y;
    if sprite.flip_x {
        std::mem::swap(&mut lu, &mut ru);
    }
    if sprite.flip_y {
        std::mem::swap(&mut v_at_top, &mut v_at_bottom);
    }

    let size = sprite
        .custom_size
        .unwrap_or_else(|| Vec2::new(rect.width() as f32, rect.height() as f32))
        + Vec2::splat(OUTLINE_MARGIN_PX * 2.0);

    let mut mesh = Mesh::from(Rectangle::new(size.x, size.y));
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        rectangle_uvs(lu, ru, v_at_top, v_at_bottom),
    );

    meshes.add(mesh).into()
}
