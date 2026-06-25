use crate::{cursor::CursorPos, keybinds::InputBinding, world, Game, ScreenResolution};
use bevy::{
    prelude::*,
    reflect::TypeUuid,
    render::{
        mesh::MeshVertexBufferLayout,
        render_resource::{
            AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState,
            RenderPipelineDescriptor, ShaderRef, SpecializedMeshPipelineError,
        },
        view::RenderLayers,
    },
    sprite::{Material2d, Material2dKey, Mesh2dHandle},
};
use bevy_ecs_tilemap::tiles::TilePos;

use super::{Interactable, UIState};

/// How much more transparent the centre of a radial overlay is vs. its edge.
/// Kept small for a very subtle vignette-like effect.
pub const RADIAL_OVERLAY_CENTER_FALLOFF: f32 = 0.2;

/// Default overlay tint (black). Alpha is driven by centre/edge uniforms, not this color's alpha.
pub const RADIAL_OVERLAY_DEFAULT_COLOR: Color = Color::BLACK;

/// Falloff exponent passed to the shader (`params.z`). Lower = darker overall, smaller bright core.
pub const RADIAL_OVERLAY_DEFAULT_FALLOFF: f32 = 0.2;

/// Linear RGB components for the radial overlay material (alpha channel ignored).
#[inline]
pub fn radial_overlay_color_uniform(color: Color) -> Vec4 {
    Vec4::new(color.r(), color.g(), color.b(), 1.0)
}

const RADIAL_OVERLAY_BLEND: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
};

/// Full-screen radial overlay: tint color + centre/edge alpha ramp.
/// `params`: x = centre alpha, y = edge alpha, z = falloff exponent.
#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "b5d1f0c2-7a3e-4c1d-9f2a-1e6c8b4d7a90"]
pub struct RadialOverlayMaterial {
    #[uniform(0)]
    pub params: Vec4,
    #[uniform(1)]
    pub color: Vec4,
}

impl Material2d for RadialOverlayMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/radial_overlay.wgsl".into()
    }

    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayout,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = &mut descriptor.fragment {
            if let Some(target_state) = &mut fragment.targets[0] {
                target_state.blend = Some(RADIAL_OVERLAY_BLEND);
            }
        }
        Ok(())
    }
}

/// Typical full-screen UI overlays (inventory, shrines, class select, etc.) use z ≈ 9–15.
/// Active skill hotbar: above those modals, below the heirloom pick screen.
pub const Z_DEPTH_HUD_ACTIVE_SKILLS: f32 = 4.0;
/// “Choose an Heirloom” screen only: backdrop above active skills, below HUD heirloom row.
pub const Z_DEPTH_HEIRLOOM_SKILL_CHOICE_OVERLAY: f32 = 52.0;
/// Root depth for title bar, cards, reroll/banish buttons on that screen.
pub const Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT: f32 = 54.0;
/// Reroll/banish count labels (slightly in front of sibling UI on the same screen).
pub const Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND: f32 = 58.0;
/// HP/mana orb HUD + tracker tooltips (above heirloom pick overlays, z ≈ 52–58).
pub const Z_DEPTH_HUD_ORB_TRACKERS_FOREGROUND: f32 = 64.0;
/// HUD heirloom icons during normal play: above the XP/currency row, below inventory/shrine overlays (z ≈ 9+).
pub const Z_DEPTH_HUD_HEIRLOOM_ICONS: f32 = 8.0;
/// HUD heirloom icons when they must render above full-screen overlays (e.g. game over at z ≈ 58).
pub const Z_DEPTH_HUD_HEIRLOOM_ICONS_FOREGROUND: f32 = 62.0;
/// Options menu backdrop + content sit above gameplay HUD (including skills/heirlooms).
/// Kept below name-entry / loading overlays (z ≈ 100+).
pub const Z_DEPTH_OPTIONS_OVERLAY: f32 = 85.0;
pub const Z_DEPTH_OPTIONS_CONTENT: f32 = 86.0;

pub fn pointcast_2d<'a>(
    cursor_pos: &Res<CursorPos>,
    ui_sprites: &'a Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    excluded_entity: Option<Entity>,
) -> Option<(Entity, &'a Sprite, &'a GlobalTransform)> {
    let mut ret: Option<(Entity, &Sprite, &GlobalTransform)> = None;

    for (ent, sprite, xform) in ui_sprites.iter() {
        if let Some(excluded) = excluded_entity {
            if ent == excluded {
                continue;
            }
        }

        let Some(size) = sprite.custom_size else {
            continue;
        };

        let initial_x = xform.translation().x - (0.5 * size.x);
        let initial_y = xform.translation().y - (0.5 * size.y);

        let terminal_x = initial_x + size.x;
        let terminal_y = initial_y + size.y;
        if (initial_x..=terminal_x).contains(&cursor_pos.ui_coords.x)
            && (initial_y..=terminal_y).contains(&cursor_pos.ui_coords.y)
        {
            ret = Some((ent, sprite, xform));
        }
    }

    ret
}

pub fn _get_player_chunk_tile_coords(game: &mut Game) -> (IVec2, TilePos) {
    let player_pos = game.player_state.position;
    let chunk_pos =
        world::world_helpers::camera_pos_to_chunk_pos(&Vec2::new(player_pos.x, player_pos.y));
    let tile_pos =
        world::world_helpers::camera_pos_to_tile_pos(&Vec2::new(player_pos.x, player_pos.y));
    (chunk_pos, tile_pos)
}

/// Format a number to condensed display format:
/// 0-9999: no change
/// 10000-99999: 12.4k, 88.2k, etc
/// 100000-999999: 134k, 478k
/// 1000000-999999999: 1.43M, 20.5M, 999M, etc
/// 1000000000+: 1.43B, 20.5B, etc
pub fn format_number(value: i64) -> String {
    let abs = value.unsigned_abs();
    let sign = if value < 0 { "-" } else { "" };
    if abs < 10_000 {
        format!("{}{}", sign, abs)
    } else if abs < 100_000 {
        format!("{}{:.1}k", sign, abs as f64 / 1_000.0)
    } else if abs < 1_000_000 {
        format!("{}{}k", sign, abs / 1_000)
    } else if abs < 1_000_000_000 {
        let millions = abs as f64 / 1_000_000.0;
        let formatted = format!("{}{:.2}M", sign, millions);
        let trimmed = formatted.trim_end_matches('0');
        if trimmed.ends_with('.') {
            trimmed.trim_end_matches('.').to_string()
        } else {
            trimmed.to_string()
        }
    } else {
        let billions = abs as f64 / 1_000_000_000.0;
        let formatted = format!("{}{:.2}B", sign, billions);
        let trimmed = formatted.trim_end_matches('0');
        if trimmed.ends_with('.') {
            trimmed.trim_end_matches('.').to_string()
        } else {
            trimmed.to_string()
        }
    }
}

/// Spawn a keybind badge: 19×9 grey box + centered label.
///
/// When `parent` is `Some`, `transform` is local to that parent (e.g. corner HUD icons).
/// When `parent` is `None`, `transform` is world-space (e.g. bottom-anchored hotbar / skills).
pub fn spawn_keybind_badge(
    commands: &mut Commands,
    asset_server: &AssetServer,
    key: InputBinding,
    transform: Transform,
    parent: Option<Entity>,
    render_layer: u8,
) -> (Entity, Entity) {
    let mut key_bg = commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: crate::ui::KEYBIND_BADGE_COLOR,
            custom_size: Some(crate::ui::KEYBIND_BADGE_SIZE),
            ..default()
        },
        transform,
        ..default()
    });
    key_bg.insert(RenderLayers::from_layers(&[render_layer]));
    if let Some(parent) = parent {
        key_bg.set_parent(parent);
    }
    let key_bg = key_bg.id();

    let key_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                crate::keybinds::get_key_display_name(key),
                TextStyle {
                    font: asset_server.load("fonts/slkscr.ttf"),
                    font_size: 8.4,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[render_layer]))
        .set_parent(key_bg)
        .id();

    (key_bg, key_text)
}

/// Spawn a HUD label badge: same 19×9 grey box as keybind badges, with a static label.
pub fn spawn_hud_label_badge(
    commands: &mut Commands,
    asset_server: &AssetServer,
    label: &str,
    transform: Transform,
    parent: Option<Entity>,
    render_layer: u8,
) -> (Entity, Entity) {
    let mut key_bg = commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: crate::ui::KEYBIND_BADGE_COLOR,
            custom_size: Some(crate::ui::KEYBIND_BADGE_SIZE),
            ..default()
        },
        transform,
        ..default()
    });
    key_bg.insert(RenderLayers::from_layers(&[render_layer]));
    if let Some(parent) = parent {
        key_bg.set_parent(parent);
    }
    let key_bg = key_bg.id();

    let key_text = commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                label.to_string(),
                TextStyle {
                    font: asset_server.load("fonts/slkscr.ttf"),
                    font_size: 8.4,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[render_layer]))
        .set_parent(key_bg)
        .id();

    (key_bg, key_text)
}

/// Full-screen overlay dimensions with margin so edges stay covered at UI zoom-out.
pub fn full_screen_overlay_size(res: &ScreenResolution) -> Vec2 {
    // The UI camera uses `ScalingMode::FixedVertical(game_height)` centred at the origin,
    // so `game_width` x `game_height` covers the screen exactly. A couple of pixels of
    // overscan guarantees the (darkest) edge of the radial gradient sits just off-screen,
    // avoiding any hard seam from sub-pixel rounding.
    const OVERSCAN: f32 = 8.0;
    Vec2::new(res.game_width + OVERSCAN, res.game_height + OVERSCAN)
}

/// Night tint overlay — extra margin vs modal overlays; resized when the UI camera bucket changes.
pub fn night_overlay_size(res: &ScreenResolution) -> Vec2 {
    const WIDTH_PAD: f32 = 72.0;
    const HEIGHT_PAD: f32 = 200.0;
    Vec2::new(res.game_width + WIDTH_PAD, res.game_height + HEIGHT_PAD)
}

pub fn spawn_full_screen_ui_overlay(
    commands: &mut Commands,
    res: &ScreenResolution,
    alpha: f32,
    depth: f32,
) -> Entity {
    spawn_full_screen_ui_overlay_colored(
        commands,
        res,
        alpha,
        depth,
        RADIAL_OVERLAY_DEFAULT_COLOR,
    )
}

/// Full-screen radial overlay with explicit centre/edge alphas (default black tint).
pub fn spawn_full_screen_ui_overlay_tuned(
    commands: &mut Commands,
    res: &ScreenResolution,
    center_alpha: f32,
    edge_alpha: f32,
    depth: f32,
) -> Entity {
    spawn_full_screen_ui_overlay_tuned_colored(
        commands,
        res,
        center_alpha,
        edge_alpha,
        depth,
        RADIAL_OVERLAY_DEFAULT_COLOR,
        RADIAL_OVERLAY_DEFAULT_FALLOFF,
    )
}

/// Full-screen radial overlay with centre/edge alphas, tint color, and falloff exponent.
pub fn spawn_full_screen_ui_overlay_tuned_colored(
    commands: &mut Commands,
    res: &ScreenResolution,
    center_alpha: f32,
    edge_alpha: f32,
    depth: f32,
    color: Color,
    falloff: f32,
) -> Entity {
    spawn_ui_overlay_tuned_colored(
        commands,
        full_screen_overlay_size(res),
        center_alpha,
        edge_alpha,
        depth,
        color,
        falloff,
    )
}

pub fn spawn_ui_overlay(commands: &mut Commands, size: Vec2, alpha: f32, depth: f32) -> Entity {
    spawn_full_screen_ui_overlay_colored_inner(commands, size, alpha, depth, RADIAL_OVERLAY_DEFAULT_COLOR)
}

fn spawn_full_screen_ui_overlay_colored(
    commands: &mut Commands,
    res: &ScreenResolution,
    alpha: f32,
    depth: f32,
    color: Color,
) -> Entity {
    spawn_full_screen_ui_overlay_colored_inner(
        commands,
        full_screen_overlay_size(res),
        alpha,
        depth,
        color,
    )
}

fn spawn_full_screen_ui_overlay_colored_inner(
    commands: &mut Commands,
    size: Vec2,
    alpha: f32,
    depth: f32,
    color: Color,
) -> Entity {
    let edge_alpha = alpha;
    let center_alpha = alpha - RADIAL_OVERLAY_CENTER_FALLOFF;
    spawn_ui_overlay_tuned_colored(
        commands,
        size,
        center_alpha,
        edge_alpha,
        depth,
        color,
        RADIAL_OVERLAY_DEFAULT_FALLOFF,
    )
}

/// Caches one quad [`Mesh`] per overlay size so the asset id stays stable across opens.
#[derive(Resource, Default)]
pub struct RadialOverlayMeshCache(std::collections::HashMap<(i32, i32), Handle<Mesh>>);

/// Caches one [`RadialOverlayMaterial`] per (centre, edge, color, falloff) tuple.
#[derive(Resource, Default)]
pub struct RadialOverlayMaterialCache(
    std::collections::HashMap<(i32, i32, i32, i32, i32, i32), Handle<RadialOverlayMaterial>>,
);

/// Marker placed on a freshly-spawned overlay entity; [`attach_radial_overlay_visuals`]
/// fills in the mesh + material the next time it runs, then removes this component.
#[derive(Component, Clone, Copy)]
pub struct PendingRadialOverlay {
    size: Vec2,
    center_alpha: f32,
    edge_alpha: f32,
    color: Color,
    falloff: f32,
}

/// Spawns a radial overlay with centre/edge alphas, tint color, and falloff exponent.
pub fn spawn_ui_overlay_tuned_colored(
    commands: &mut Commands,
    size: Vec2,
    center_alpha: f32,
    edge_alpha: f32,
    depth: f32,
    color: Color,
    falloff: f32,
) -> Entity {
    commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_xyz(0., 0., depth)),
            RenderLayers::from_layers(&[3]),
            UIState::Inventory,
            PendingRadialOverlay {
                size,
                center_alpha: center_alpha.clamp(0.0, 1.0),
                edge_alpha: edge_alpha.clamp(0.0, 1.0),
                color,
                falloff,
            },
            Name::new("overlay"),
        ))
        .id()
}

/// Attaches the cached quad mesh + radial material to any overlay still awaiting them.
pub fn attach_radial_overlay_visuals(
    mut commands: Commands,
    pending: Query<(Entity, &PendingRadialOverlay)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<RadialOverlayMaterial>>,
    mut mesh_cache: ResMut<RadialOverlayMeshCache>,
    mut material_cache: ResMut<RadialOverlayMaterialCache>,
) {
    for (entity, overlay) in pending.iter() {
        let mesh_key = (overlay.size.x.round() as i32, overlay.size.y.round() as i32);
        let mesh = mesh_cache
            .0
            .entry(mesh_key)
            .or_insert_with(|| {
                meshes.add(Mesh::from(shape::Quad {
                    size: overlay.size,
                    ..default()
                }))
            })
            .clone();

        let color_u = radial_overlay_color_uniform(overlay.color);
        let mat_key = (
            (overlay.center_alpha * 1000.0).round() as i32,
            (overlay.edge_alpha * 1000.0).round() as i32,
            (color_u.x * 1000.0).round() as i32,
            (color_u.y * 1000.0).round() as i32,
            (color_u.z * 1000.0).round() as i32,
            (overlay.falloff * 1000.0).round() as i32,
        );
        let material = material_cache
            .0
            .entry(mat_key)
            .or_insert_with(|| {
                materials.add(RadialOverlayMaterial {
                    params: Vec4::new(
                        overlay.center_alpha,
                        overlay.edge_alpha,
                        overlay.falloff,
                        0.0,
                    ),
                    color: color_u,
                })
            })
            .clone();

        commands
            .entity(entity)
            .insert((Mesh2dHandle::from(mesh), material))
            .remove::<PendingRadialOverlay>();
    }
}
