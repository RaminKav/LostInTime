use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::{
        COMMON_TOOLTIP_TITLE, LEGENDARY_TOOLTIP_TITLE, RARE_TOOLTIP_TITLE, UNCOMMON_TOOLTIP_TITLE,
        WHITE,
    },
    player::{
        skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState, HeirloomRarity},
        time_crystals::TimeCrystals,
    },
    ui::{game_fonts as gf, ui_helpers::Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND, Interactable, UIState},
    ScreenResolution,
};

use super::{
    heirloom_tooltip::{HeirloomTooltipRequest, HeirloomTooltipShow},
    interactions::Interaction,
};

/// Root container for the per-rarity banish tracker (skill choice + heirloom chest screens).
#[derive(Component)]
pub struct BanishTrackerRoot;

/// Marker for an individual heirloom icon inside the banish tracker. Carries the heirloom +
/// rarity so the hover tooltip system can mirror the player HUD's heirloom hover behavior.
#[derive(Component, Clone)]
pub struct BanishTrackerIcon {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
}

const BANISH_TRACKER_ICON_SIZE: f32 = 14.;
const BANISH_TRACKER_ICON_SPACING: f32 = 16.;
/// Vertical spacing between rows of icons inside the same rarity (when wrapping).
const BANISH_TRACKER_ICON_ROW_SPACING: f32 = 16.;
/// Vertical gap between the bottom of one rarity row and the heading of the next.
const BANISH_TRACKER_RARITY_GAP: f32 = 18.;
const BANISH_TRACKER_HEADING_TO_ICONS: f32 = 12.;
const BANISH_TRACKER_ICONS_PER_ROW: usize = 4;

pub fn banish_tracker_root_transform(game_width: f32) -> Transform {
    Transform::from_translation(Vec3::new(
        -game_width * 0.5 + 6.,
        58.,
        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
    ))
}

/// Spawns the banish tracker root and its children at the shared left-side position.
pub fn spawn_banish_tracker(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    queue: &HeirloomChoiceQueue,
    time_crystals: &TimeCrystals,
    resolution: &ScreenResolution,
    ui_state: UIState,
) -> Entity {
    let root = commands
        .spawn((
            SpatialBundle::from_transform(banish_tracker_root_transform(resolution.game_width)),
            RenderLayers::from_layers(&[3]),
            ui_state.clone(),
            BanishTrackerRoot,
            Name::new("Banish Tracker"),
        ))
        .id();
    build_banish_tracker_children(
        commands,
        asset_server,
        graphics,
        queue,
        time_crystals,
        root,
        ui_state,
    );
    root
}

/// Build the banishes tracker (title + per-rarity heading rows + heirloom icon rows) under `root`.
/// Icon entities carry [`BanishTrackerIcon`] + [`Interactable`] so [`handle_banish_tracker_tooltip`]
/// spawns the same heirloom hover card used by the player HUD.
pub fn build_banish_tracker_children(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    queue: &HeirloomChoiceQueue,
    time_crystals: &TimeCrystals,
    root: Entity,
    ui_state: UIState,
) {
    let title_style = gf::SKILL_CHOICE_TRACKER_TITLE.text_style(asset_server, WHITE);

    let mut y: f32 = 0.;
    let title = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section("banishes", title_style.clone())
                    .with_alignment(TextAlignment::Left),
                text_anchor: Anchor::TopLeft,
                transform: Transform::from_translation(Vec3::new(0., y, 1.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            ui_state.clone(),
            Name::new("Banish Tracker Title"),
        ))
        .id();
    commands.entity(title).set_parent(root);
    y -= 10.;

    for rarity in [
        HeirloomRarity::Common,
        HeirloomRarity::Uncommon,
        HeirloomRarity::Rare,
        HeirloomRarity::Legendary,
    ] {
        let (heading, heading_color) = match rarity {
            HeirloomRarity::Common => ("Common", COMMON_TOOLTIP_TITLE),
            HeirloomRarity::Uncommon => ("Uncommon", UNCOMMON_TOOLTIP_TITLE),
            HeirloomRarity::Rare => ("Rare", RARE_TOOLTIP_TITLE),
            HeirloomRarity::Legendary => ("Legendary", LEGENDARY_TOOLTIP_TITLE),
        };
        let allowed = queue.allowed_banishes_for_rarity(time_crystals, rarity);
        let heading_style = gf::SKILL_CHOICE_MICRO.text_style(asset_server, heading_color);
        let heading_e = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(format!("{} ({})", heading, allowed), heading_style)
                        .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::TopLeft,
                    transform: Transform::from_translation(Vec3::new(0., y, 1.)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                ui_state.clone(),
                Name::new("Banish Tracker Heading"),
            ))
            .id();
        commands.entity(heading_e).set_parent(root);
        y -= BANISH_TRACKER_HEADING_TO_ICONS;

        let icon_y = y - BANISH_TRACKER_ICON_SIZE * 0.5;
        let icons: Vec<HeirloomChoiceState> = queue
            .banished_heirlooms
            .iter()
            .filter(|h| h.rarity == rarity && h.heirloom != Heirloom::None)
            .cloned()
            .collect();

        if icons.is_empty() {
            y -= BANISH_TRACKER_ICON_SIZE;
        } else {
            let row_count =
                (icons.len() + BANISH_TRACKER_ICONS_PER_ROW - 1) / BANISH_TRACKER_ICONS_PER_ROW;
            for (i, choice) in icons.iter().enumerate() {
                let row = i / BANISH_TRACKER_ICONS_PER_ROW;
                let col = i % BANISH_TRACKER_ICONS_PER_ROW;
                let icon_x =
                    BANISH_TRACKER_ICON_SIZE * 0.5 + col as f32 * BANISH_TRACKER_ICON_SPACING;
                let row_icon_y = icon_y - row as f32 * BANISH_TRACKER_ICON_ROW_SPACING;
                let icon_e = commands
                    .spawn((
                        SpriteSheetBundle {
                            sprite: graphics.get_heirloom_icon(choice.heirloom.clone()),
                            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                            transform: Transform::from_translation(Vec3::new(
                                icon_x, row_icon_y, 1.,
                            )),
                            ..Default::default()
                        },
                        Sprite {
                            custom_size: Some(Vec2::new(
                                BANISH_TRACKER_ICON_SIZE,
                                BANISH_TRACKER_ICON_SIZE,
                            )),
                            ..Default::default()
                        },
                        RenderLayers::from_layers(&[3]),
                        ui_state.clone(),
                        Interactable::default(),
                        BanishTrackerIcon {
                            heirloom: choice.heirloom.clone(),
                            rarity: choice.rarity,
                        },
                        Name::new("Banish Tracker Icon"),
                    ))
                    .id();
                commands.entity(icon_e).set_parent(root);
            }
            y -= BANISH_TRACKER_ICON_SIZE
                + (row_count.saturating_sub(1)) as f32 * BANISH_TRACKER_ICON_ROW_SPACING;
        }

        y -= BANISH_TRACKER_RARITY_GAP;
    }
}

/// Hover for banish tracker icons: drives [`Interactable`] like the player HUD, then sends
/// [`HeirloomTooltipRequest`] for the shared post-update processor.
pub fn handle_banish_tracker_tooltip(
    cursor_pos: Res<crate::cursor::CursorPos>,
    hit_detection_sprites: Query<
        (Entity, &Sprite, &GlobalTransform),
        With<super::interactions::Interactable>,
    >,
    mut tracker_icons: Query<(
        Entity,
        &GlobalTransform,
        &mut super::interactions::Interactable,
        &BanishTrackerIcon,
        &UIState,
    )>,
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
    mut last_hovered: Local<Option<Heirloom>>,
) {
    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None, None);

    for (entity, _, mut interactable, _, _) in tracker_icons.iter_mut() {
        let is_hit = hit_entity
            .as_ref()
            .map(|(e, _, _)| *e == entity)
            .unwrap_or(false);
        if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::Hovering);
        } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }

    let currently_hovered = tracker_icons
        .iter()
        .find(|(_, _, interactable, _, _)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(_, transform, _, icon, ui_state)| (icon.clone(), transform.translation(), ui_state.clone()));

    let hovered_heirloom = currently_hovered.as_ref().map(|(i, _, _)| i.heirloom.clone());
    if *last_hovered == hovered_heirloom {
        return;
    }

    match &currently_hovered {
        None => {
            tooltip_requests.send(HeirloomTooltipRequest::Clear);
        }
        Some((icon, icon_pos, ui_state)) => {
            let tooltip_pos = Vec3::new(icon_pos.x + 90., icon_pos.y, icon_pos.z + 10.);
            tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom: icon.heirloom.clone(),
                rarity: icon.rarity,
                position: tooltip_pos,
                scaling_text: None,
                trigger_count: 0,
                ui_state: Some(ui_state.clone()),
            }));
        }
    }

    *last_hovered = hovered_heirloom;
}

pub fn update_banish_tracker_ui(
    mut commands: Commands,
    skill_queue: Res<HeirloomChoiceQueue>,
    time_crystals: Res<TimeCrystals>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    tracker_roots: Query<(Entity, &UIState), With<BanishTrackerRoot>>,
    children: Query<&Children>,
) {
    if !skill_queue.is_changed() {
        return;
    }
    for (root, ui_state) in tracker_roots.iter() {
        if let Ok(kids) = children.get(root) {
            for child in kids.iter() {
                commands.entity(*child).despawn_recursive();
            }
        }
        build_banish_tracker_children(
            &mut commands,
            &asset_server,
            &graphics,
            &skill_queue,
            &time_crystals,
            root,
            ui_state.clone(),
        );
    }
}
