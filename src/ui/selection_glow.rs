//! Pixelated purple glow border behind the class-select / pet-select slot backgrounds when
//! they're the current selection (see `PlayerSelectSlot`/`PetSelectSlot` in `class_selection.rs`).
//!
//! The Selected slot art already swaps in on its own (`update_slot_visuals`), but Hover takes
//! priority over Selected there — so if you hover a slot you already had selected, the art
//! flips to the Hover texture and the "this is still selected" signal would otherwise vanish.
//! This glow is a separate, non-destructive child quad spawned *behind* the slot sprite (same
//! trick as `crate::item::item_drop_outline::UiShadow`), so it keeps showing regardless of
//! which texture is currently on top. The shader samples the Selected art's own alpha channel
//! to hug its actual silhouette (rounded corners, top notch, etc.) rather than a bounding-box
//! shape, so it never "borders" the sprite's fully transparent corners.
use bevy::camera::visibility::RenderLayers;
use bevy::math::primitives::Rectangle;
use bevy::mesh::Mesh2d;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};

use crate::assets::Graphics;
use crate::ui::class_selection::{PetSelectSlot, PlayerSelectSlot};
use crate::ui::UIElement;

/// Base purple sampled from `PlayerSelectSlotSelected.png` / `PetSelectSlotSelected.png`'s
/// border pixel (#5D2375-ish) so the glow reads as "the same purple as the selected frame".
pub const SELECTION_GLOW_COLOR: Color = Color::srgb(93. / 255., 35. / 255., 117. / 255.);
/// Brighter purple mixed in on shimmer highlights.
pub const SELECTION_GLOW_HOT_COLOR: Color = Color::srgb(189. / 255., 88. / 255., 214. / 255.);

const GLOW_Z: f32 = -0.5;
/// How much larger than the slot's own size the glow quad is — only the part that pokes out
/// past the slot's edges ends up visible, since the slot art itself is opaque and sits in front.
/// Just needs to comfortably fit the border layers below plus a little slack.
const GLOW_SCALE: f32 = 1.3;
/// Border thickness, in texels (the slot art's own pixel resolution is 1:1 with game pixels) —
/// a 3-4 pixel border as requested, with opacity decreasing outward one texel at a time.
const GLOW_BORDER_LAYERS: f32 = 3.5;
/// How quickly the shimmer noise drifts to new brightness values.
const GLOW_SHIMMER_SPEED: f32 = 0.6;
const GLOW_INTENSITY: f32 = 0.8;
/// Shimmer animation frame duration — low frame rate so it flickers like hand-drawn sprite art
/// instead of smoothly interpolating.
const GLOW_FRAME_DURATION: f32 = 0.09;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct SelectionGlowMaterial {
    #[uniform(0)]
    pub glow_color: Vec4,
    #[uniform(1)]
    pub hot_color: Vec4,
    /// (time, shimmer_speed, border_layers, intensity) — see `selection_glow.wgsl`.
    #[uniform(2)]
    pub params: Vec4,
    /// (slot_width_px, slot_height_px, glow_scale, frame_duration) — see `selection_glow.wgsl`.
    #[uniform(3)]
    pub shape_params: Vec4,
    /// The slot's own "Selected" art, sampled for its alpha silhouette so the border hugs the
    /// actual drawn shape instead of the sprite's bounding rectangle.
    #[texture(4)]
    #[sampler(5)]
    pub source_texture: Option<Handle<Image>>,
}

impl Material2d for SelectionGlowMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/selection_glow.wgsl".into()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

/// Marks the spawned glow child quad itself.
#[derive(Component)]
pub struct SelectionGlow;

/// Marks a `PlayerSelectSlot`/`PetSelectSlot` background that already has its glow child
/// spawned, so `spawn_selection_glows` doesn't duplicate it every frame.
#[derive(Component)]
pub struct SelectionGlowApplied;

/// Glow materials keyed by the "Selected" art texture they sample (which also implies the
/// slot's size/aspect ratio) — class slots and pet slots use distinct art, but every slot of a
/// given kind shares the exact same asset, so this stays a small, fixed-size cache.
#[derive(Resource, Default)]
pub struct SelectionGlowState {
    pub materials: HashMap<Handle<Image>, Handle<SelectionGlowMaterial>>,
}

pub struct SelectionGlowPlugin;

impl Plugin for SelectionGlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SelectionGlowState>()
            .add_plugins(Material2dPlugin::<SelectionGlowMaterial>::default())
            .add_systems(
                Update,
                (
                    spawn_selection_glows,
                    update_selection_glow_visibility,
                    animate_selection_glow_material,
                ),
            );
    }
}

fn spawn_selection_glows(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<SelectionGlowMaterial>>,
    mut state: ResMut<SelectionGlowState>,
    graphics: Res<Graphics>,
    player_slots: Query<
        (Entity, &Sprite, &PlayerSelectSlot, Option<&RenderLayers>),
        Without<SelectionGlowApplied>,
    >,
    pet_slots: Query<
        (Entity, &Sprite, &PetSelectSlot, Option<&RenderLayers>),
        Without<SelectionGlowApplied>,
    >,
) {
    if player_slots.is_empty() && pet_slots.is_empty() {
        return;
    }

    let player_art = graphics.get_ui_element_texture(UIElement::PlayerSelectSlotSelected);
    let pet_art = graphics.get_ui_element_texture(UIElement::PetSelectSlotSelected);

    for (entity, sprite, slot, layers) in &player_slots {
        spawn_glow_child(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            entity,
            sprite,
            &player_art,
            slot.is_selected,
            layers,
        );
    }
    for (entity, sprite, slot, layers) in &pet_slots {
        spawn_glow_child(
            &mut commands,
            &mut meshes,
            &mut materials,
            &mut state,
            entity,
            sprite,
            &pet_art,
            slot.is_selected,
            layers,
        );
    }
}

fn spawn_glow_child(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<SelectionGlowMaterial>,
    state: &mut SelectionGlowState,
    parent: Entity,
    sprite: &Sprite,
    art_texture: &Handle<Image>,
    is_selected: bool,
    layers: Option<&RenderLayers>,
) {
    let base_size = sprite.custom_size.unwrap_or(Vec2::new(44., 60.));
    let glow_size = base_size * GLOW_SCALE;
    let material = state
        .materials
        .entry(art_texture.clone())
        .or_insert_with(|| {
            materials.add(SelectionGlowMaterial {
                glow_color: SELECTION_GLOW_COLOR.to_linear().to_vec4(),
                hot_color: SELECTION_GLOW_HOT_COLOR.to_linear().to_vec4(),
                params: Vec4::new(0., GLOW_SHIMMER_SPEED, GLOW_BORDER_LAYERS, GLOW_INTENSITY),
                shape_params: Vec4::new(base_size.x, base_size.y, GLOW_SCALE, GLOW_FRAME_DURATION),
                source_texture: Some(art_texture.clone()),
            })
        })
        .clone();
    let mesh: Mesh2d = meshes
        .add(Mesh::from(Rectangle::new(glow_size.x, glow_size.y)))
        .into();
    let layers = layers.cloned().unwrap_or_else(|| RenderLayers::layer(3));

    let child = commands
        .spawn((
            mesh,
            MeshMaterial2d(material.clone()),
            (
                Transform::from_xyz(0., 0., GLOW_Z),
                if is_selected {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ),
            layers,
            SelectionGlow,
            Name::new("Selection Glow"),
        ))
        .id();
    crate::ecs_helpers::safe_add_child(commands, parent, child);
    commands.entity(parent).insert(SelectionGlowApplied);
}

fn update_selection_glow_visibility(
    player_slots: Query<(&PlayerSelectSlot, &Children), Changed<PlayerSelectSlot>>,
    pet_slots: Query<(&PetSelectSlot, &Children), Changed<PetSelectSlot>>,
    mut glow_visibility: Query<&mut Visibility, With<SelectionGlow>>,
) {
    for (slot, children) in &player_slots {
        set_children_glow_visibility(children, slot.is_selected, &mut glow_visibility);
    }
    for (slot, children) in &pet_slots {
        set_children_glow_visibility(children, slot.is_selected, &mut glow_visibility);
    }
}

fn set_children_glow_visibility(
    children: &Children,
    is_selected: bool,
    glow_visibility: &mut Query<&mut Visibility, With<SelectionGlow>>,
) {
    let target = if is_selected {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for child in children.iter() {
        if let Ok(mut visibility) = glow_visibility.get_mut(child) {
            *visibility = target;
        }
    }
}

fn animate_selection_glow_material(
    time: Res<Time>,
    state: Res<SelectionGlowState>,
    mut materials: ResMut<Assets<SelectionGlowMaterial>>,
) {
    let elapsed = time.elapsed_secs();
    for handle in state.materials.values() {
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.x = elapsed;
        }
    }
}
