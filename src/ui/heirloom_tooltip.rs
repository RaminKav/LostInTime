//! Shared heirloom **hover** tooltip: one event channel and one processor so any UI can
//! show/dismiss the same card layout without duplicating spawn logic.

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::WHITE,
    player::skills::{Heirloom, HeirloomRarity},
};

use super::UIState;

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
    pub trigger_count_text: Option<String>,
    /// When set, tags the entity for `handle_new_ui_state` teardown.
    pub ui_state: Option<UIState>,
}

/// Request to clear or show the single global heirloom hover tooltip.
#[derive(Clone, Debug)]
pub enum HeirloomTooltipRequest {
    Clear,
    Show(HeirloomTooltipShow),
}

/// Spawns a single heirloom card (icon, title, description, optional scaling / trigger lines).
/// Does **not** insert [`UIState`] — callers or [`process_heirloom_tooltip_requests`] add it.
/// Also used for non-hover cards (e.g. level-up choices) that attach their own components after.
pub fn spawn_heirloom_tooltip_card(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    heirloom: Heirloom,
    rarity: HeirloomRarity,
    position: Vec3,
    scaling_text: Option<String>,
    trigger_count_text: Option<String>,
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
                translation: position,
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
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
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

    for (j, desc) in heirloom.get_desc().iter().enumerate() {
        let mut text_desc = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    desc,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., -(j as f32 * 9.) - 4., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Heirloom Desc"),
            RenderLayers::from_layers(&[3]),
        ));
        text_desc.set_parent(card_e);
    }

    let desc_count = heirloom.get_desc().len();
    let mut extra_lines = 0;

    if let Some(scaling_text) = scaling_text {
        let mut text_scaling = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    scaling_text,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::LIGHT_GREY,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, -(desc_count as f32 * 9.) - 5.0, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Heirloom Scaling Text"),
            RenderLayers::from_layers(&[3]),
        ));
        text_scaling.set_parent(card_e);
        extra_lines += 1;
    }

    if let Some(trigger_text) = trigger_count_text {
        let y_offset = -(desc_count as f32 * 9.) - 5.0 - (extra_lines as f32 * 9.);
        let mut text_trigger = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    trigger_text,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::YELLOW_2,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, y_offset, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Heirloom Trigger Count"),
            RenderLayers::from_layers(&[3]),
        ));
        text_trigger.set_parent(card_e);
    }

    card_e
}

/// Consumes [`HeirloomTooltipRequest`]. Hover systems compute world position (see [`ui_world_translation`]).
pub fn process_heirloom_tooltip_requests(
    mut commands: Commands,
    mut events: EventReader<HeirloomTooltipRequest>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
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
                spec.heirloom.clone(),
                spec.rarity,
                spec.position,
                spec.scaling_text.clone(),
                spec.trigger_count_text.clone(),
            );
            let mut ec = commands.entity(card);
            ec.insert(HeirloomDynamicTooltip);
            if let Some(st) = spec.ui_state.clone() {
                ec.insert(st);
            }
        }
    }
}
