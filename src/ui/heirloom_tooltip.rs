//! Shared heirloom **hover** tooltip: one event channel and one processor so any UI can
//! show/dismiss the same card layout without duplicating spawn logic.

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::{LIGHT_GREY, SHIELD_BLUE, WHITE, YELLOW_2},
    item::item_drop_outline::{HeirloomIconOutline, HeirloomIconOutlineStyle},
    player::skills::{Heirloom, HeirloomRarity},
    ScreenResolution,
};

use super::{
    game_fonts as gf,
    tooltip_info_boxes::{
        build_tooltip_info_boxes, spawn_tooltip_info_boxes_with_resolution, HeirloomDescLineKind,
        TooltipInfoBoxAnchor,
    },
    tooltips, UIState,
};

/// Vertical offset from the hovered HUD heirloom icon to the tooltip card center.
pub const HEIRLOOM_HUD_HOVER_TOOLTIP_Y_OFFSET: f32 = -90.;
pub const HEIRLOOM_HUD_HOVER_TOOLTIP_Z_OFFSET: f32 = 100.;
/// Inset from the screen edge when clamping heirloom hover tooltips.
pub const HEIRLOOM_TOOLTIP_SCREEN_EDGE_PAD: f32 = 8.;
/// High layer-3 z for the hover card so it (and its child info boxes) render above all other
/// UI such as shop icons/hitboxes.
pub const HEIRLOOM_TOOLTIP_CARD_Z: f32 = 140.;

/// World position for a HUD heirloom hover card, nudged inward when near the left/right edge.
pub fn heirloom_hud_hover_tooltip_position(
    icon_pos: Vec3,
    tooltip_half_width: f32,
    game_width: f32,
) -> Vec3 {
    let x = tooltips::clamp_tooltip_center_x(
        icon_pos.x,
        tooltip_half_width,
        game_width,
        HEIRLOOM_TOOLTIP_SCREEN_EDGE_PAD,
    );
    Vec3::new(
        x,
        icon_pos.y + HEIRLOOM_HUD_HOVER_TOOLTIP_Y_OFFSET,
        icon_pos.z + HEIRLOOM_HUD_HOVER_TOOLTIP_Z_OFFSET,
    )
}

/// Marker on tooltip cards spawned through [`HeirloomTooltipRequest`] (and cleared by the processor).
#[derive(Component)]
pub struct HeirloomDynamicTooltip;

/// Payload for [`HeirloomTooltipRequest::Show`].
#[derive(Clone, Debug)]
pub struct HeirloomTooltipShow {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
    /// Absolute world position the hover handler computed (typically icon
    /// `GlobalTransform::translation()` + a per-UI offset). Used as-is by the processor.
    pub position: Vec3,
    pub scaling_text: Option<String>,
    pub trigger_count: u32,
    /// When set, tags the entity for `handle_new_ui_state` teardown.
    pub ui_state: Option<UIState>,
}

/// Request to clear or show the single global heirloom hover tooltip.
#[derive(Clone, Debug)]
pub enum HeirloomTooltipRequest {
    Clear,
    Show(HeirloomTooltipShow),
}

fn desc_line_color(kind: HeirloomDescLineKind) -> Color {
    match kind {
        HeirloomDescLineKind::Mana => SHIELD_BLUE,
        HeirloomDescLineKind::Stat => YELLOW_2,
        HeirloomDescLineKind::Effect | HeirloomDescLineKind::Blank => WHITE,
    }
}

/// Spawns a single heirloom card (icon, title, description, optional scaling / side info boxes).
/// Does **not** insert [`UIState`] — callers or [`process_heirloom_tooltip_requests`] add it.
/// Also used for non-hover cards (e.g. level-up choices) that attach their own components after.
pub fn spawn_heirloom_tooltip_card(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    heirloom: Heirloom,
    rarity: HeirloomRarity,
    position: Vec3,
    scaling_text: Option<String>,
    trigger_count: u32,
    show_info_boxes: bool,
) -> Entity {
    let (ui_element, size) = heirloom.get_ui_element(rarity);
    let card_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                // Force a high z so the card (and its children) render above all other
                // layer-3 UI like shop icons/hitboxes, regardless of the caller's z.
                translation: position + Vec3::new(0., 0., HEIRLOOM_TOOLTIP_CARD_Z),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ui_element)
        .insert(Name::new("HEIRLOOM TOOLTIP CARD"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    let skill_icon = commands
        .spawn(SpriteSheetBundle {
            sprite: graphics.get_heirloom_icon(heirloom.clone()),
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform {
                translation: Vec2::new(2., 52.).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(HeirloomIconOutline::new(
            rarity,
            HeirloomIconOutlineStyle::TooltipCard,
        ))
        .insert(Name::new("HEIRLOOM ICON"))
        .set_parent(card_e)
        .id();

    if let Some(glow) = rarity.get_item_glow() {
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_item_glow(glow),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(32., 32.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec2::new(0., 0.).extend(-1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(skill_icon);
    }

    let mut text_title = commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                heirloom.get_title(),
                gf::HEIRLOOM_CARD_TITLE.text_style(asset_server, WHITE),
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., 20., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("Heirloom Title"),
        RenderLayers::from_layers(&[3]),
    ));
    text_title.set_parent(card_e);

    let desc_lines = heirloom.desc_lines();
    let mut line_index = 0usize;
    let mut desc_slots: Vec<usize> = Vec::new();
    for line in &desc_lines {
        if line.kind == HeirloomDescLineKind::Blank {
            line_index += 1;
            continue;
        }
        desc_slots.push(line_index);
        line_index += 1;
    }

    let desc_y_offset = desc_slots
        .first()
        .zip(desc_slots.last())
        .map(|(first_slot, last_slot)| {
            let top = gf::heirloom_desc_first_line_y()
                - *first_slot as f32 * gf::HEIRLOOM_CARD_DESC_LINE_STEP;
            let bottom = gf::heirloom_desc_first_line_y()
                - *last_slot as f32 * gf::HEIRLOOM_CARD_DESC_LINE_STEP;
            gf::heirloom_desc_text_center_y() - (top + bottom) * 0.5
        })
        .unwrap_or(0.);

    line_index = 0;
    for line in &desc_lines {
        if line.kind == HeirloomDescLineKind::Blank {
            line_index += 1;
            continue;
        }
        let color = desc_line_color(line.kind);
        let y = gf::heirloom_desc_first_line_y()
            - line_index as f32 * gf::HEIRLOOM_CARD_DESC_LINE_STEP
            + desc_y_offset;
        let mut text_desc = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    line.text.as_str(),
                    gf::HEIRLOOM_CARD_BODY.text_style(asset_server, color),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(2., y, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Heirloom Desc"),
            RenderLayers::from_layers(&[3]),
        ));
        text_desc.set_parent(card_e);
        line_index += 1;
    }

    if let Some(scaling_text) = scaling_text {
        let scaling_y = gf::heirloom_desc_first_line_y()
            - line_index as f32 * gf::HEIRLOOM_CARD_DESC_LINE_STEP
            - 1.0
            + desc_y_offset;
        let mut text_scaling = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    scaling_text,
                    gf::HEIRLOOM_CARD_META.text_style(asset_server, LIGHT_GREY),
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, scaling_y, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Heirloom Scaling Text"),
            RenderLayers::from_layers(&[3]),
        ));
        text_scaling.set_parent(card_e);
    }

    if show_info_boxes {
        let info_boxes = build_tooltip_info_boxes(heirloom.clone(), trigger_count);
        if let Some(info_root) = spawn_tooltip_info_boxes_with_resolution(
            commands,
            graphics,
            asset_server,
            resolution,
            TooltipInfoBoxAnchor {
                center: position,
                half_width: size.x * 0.5,
                half_height: size.y * 0.5,
                game_width: resolution.game_width,
            },
            &info_boxes,
        ) {
            commands.entity(info_root).set_parent(card_e);
        }
    }

    card_e
}

/// Consumes [`HeirloomTooltipRequest`]. Hover systems compute world position (see [`ui_world_translation`]).
pub fn process_heirloom_tooltip_requests(
    mut commands: Commands,
    mut events: EventReader<HeirloomTooltipRequest>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    existing: Query<Entity, With<HeirloomDynamicTooltip>>,
) {
    for ev in events.iter() {
        let to_despawn: Vec<Entity> = existing.iter().collect();
        for e in to_despawn {
            commands.entity(e).despawn_recursive();
        }

        if let HeirloomTooltipRequest::Show(spec) = ev {
            let card = spawn_heirloom_tooltip_card(
                &graphics,
                &mut commands,
                &asset_server,
                &resolution,
                spec.heirloom.clone(),
                spec.rarity,
                spec.position,
                spec.scaling_text.clone(),
                spec.trigger_count,
                true,
            );
            let mut ec = commands.entity(card);
            ec.insert(HeirloomDynamicTooltip);
            if let Some(st) = spec.ui_state.clone() {
                ec.insert(st);
            }
        }
    }
}
