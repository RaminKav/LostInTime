use bevy::{ecs::query::Or, prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{LIGHT_GREEN, WHITE, YELLOW_2},
    cursor::CursorPos,
    player::{
        skills::{time_crystal_heirlooms, Heirloom, HeirloomRarity},
        time_crystals::LastRunCrystalProgress,
    },
    ui::{
        heirloom_tooltip::{HeirloomTooltipRequest, HeirloomTooltipShow},
        interactions::{Interactable, Interaction},
        ui_helpers, UIElement, UIState,
    },
    ScreenResolution,
};

/// Marker for everything spawned by the post-run Time Crystal progress popup.
#[derive(Component)]
pub struct TimeCrystalProgressUI;

/// Marker for the OK button inside the popup.
#[derive(Component)]
pub struct TimeCrystalProgressOKButton;

/// Marker for one of the heirloom unlock icons in the popup. The contained
/// heirloom + rarity drive the hover tooltip card.
#[derive(Component, Clone)]
pub struct CrystalUnlockIcon {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
}

/// Invisible hit target for a locked heirloom slot in the browser grid (`?` child text).
/// Hover still shows the heirloom tooltip card (same as [`CrystalUnlockIcon`]).
#[derive(Component, Clone)]
pub struct HeirloomGridLockedCell {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
}

const PANEL_Z: f32 = 101.;
const PANEL_CONTENT_Z: f32 = 102.;
const TOOLTIP_Z: f32 = 110.;

const PANEL_WIDTH: f32 = 280.;
const PANEL_BASE_HEIGHT: f32 = 110.;
const PANEL_HEIGHT_PER_UNLOCK_ROW: f32 = 50.;

const ICON_SIZE: f32 = 16.;
const ICON_SPACING: f32 = 24.;

/// Hit-test for [`CrystalUnlockIcon`] and [`HeirloomGridLockedCell`] sprites (main-menu tooltips).
pub(crate) fn pointcast_unlock_tooltip_sprite_hit(
    cursor_pos: &CursorPos,
    q: &Query<
        (Entity, &Sprite, &GlobalTransform),
        (
            With<Interactable>,
            Or<(With<CrystalUnlockIcon>, With<HeirloomGridLockedCell>)>,
        ),
    >,
) -> Option<Entity> {
    let mut ret: Option<(Entity, f32)> = None;
    for (ent, sprite, xform) in q.iter() {
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
            let z = xform.translation().z;
            if ret.map(|(_, z0)| z > z0).unwrap_or(true) {
                ret = Some((ent, z));
            }
        }
    }
    ret.map(|(e, _)| e)
}

/// On entering MainMenu, if the previous run produced any time-crystal progress, open
/// the popup so the player sees their gains before doing anything else.
///
/// Defers to any other popup (e.g. name entry) that already queued a UIState change
/// for this frame so we don't fight over the state transition.
pub fn check_show_time_crystal_progress_popup(
    progress: Option<Res<LastRunCrystalProgress>>,
    current_ui_state: Res<State<UIState>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    if progress.is_none() {
        return;
    }
    if current_ui_state.0 != UIState::Closed {
        return;
    }
    if next_ui_state.0.is_some() {
        return;
    }
    next_ui_state.set(UIState::TimeCrystalProgress);
}

pub fn setup_time_crystal_progress_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
    progress: Option<Res<LastRunCrystalProgress>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    let Some(progress) = progress else {
        // Resource isn't there for some reason - bail back to closed so the menu is
        // usable.
        next_ui_state.set(UIState::Closed);
        return;
    };

    let newly_completed = progress.newly_completed_indices();
    // Each newly-completed crystal adds one row (header + icon row) of vertical space.
    let panel_height =
        PANEL_BASE_HEIGHT + newly_completed.len() as f32 * PANEL_HEIGHT_PER_UNLOCK_ROW;

    // Background overlay (darken main menu)
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 0.7),
                custom_size: Some(Vec2::new(
                    resolution.game_width + 10.,
                    resolution.game_height + 20.,
                )),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., PANEL_Z - 1.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalProgressUI,
        UIState::TimeCrystalProgress,
        Name::new("Time Crystal Progress Overlay"),
    ));

    // Panel background
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.05, 0.05, 0.08, 0.95),
                custom_size: Some(Vec2::new(PANEL_WIDTH, panel_height)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., PANEL_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalProgressUI,
        UIState::TimeCrystalProgress,
        Name::new("Time Crystal Progress Panel"),
    ));

    let half_h = panel_height * 0.5;
    let mut cursor_y = half_h - 16.;

    // Title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Time Crystal Progress",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., cursor_y, PANEL_CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalProgressUI,
        UIState::TimeCrystalProgress,
        Name::new("Time Crystal Progress Title"),
    ));
    cursor_y -= 22.;

    // Crystal the player was filling *before* this run's shards (same index for before/after).
    // Do not use `after.current_focus_idx()` here: completing a crystal (e.g. 5/6 -> 6/6) moves
    // "focus" to the next slot, which would wrongly show 0/6 and +0 for that next crystal.
    let primary_idx = progress.before.current_focus_idx();
    let (focus_label, shards_line) = match primary_idx {
        Some(idx) => {
            if let Some(after_crystal) = progress.after.crystals.get(idx) {
                let before_shards = progress
                    .before
                    .crystals
                    .get(idx)
                    .map(|c| c.shards)
                    .unwrap_or(0);
                let gained_here = after_crystal.shards.saturating_sub(before_shards);
                let complete_note = if after_crystal.is_complete() {
                    " (complete!)"
                } else {
                    ""
                };
                (
                    format!("Time Crystal #{}", idx + 1),
                    format!(
                        "Shards: {}/{}{} +{} this run",
                        after_crystal.shards,
                        after_crystal.required_shards,
                        complete_note,
                        gained_here,
                    ),
                )
            } else {
                (
                    format!("Time Crystal #{}", idx + 1),
                    format!("+{} shards this run", progress.shards_earned),
                )
            }
        }
        None => (
            "All Time Crystals Complete!".to_string(),
            format!("+{} shards this run", progress.shards_earned),
        ),
    };

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                focus_label,
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., cursor_y, PANEL_CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalProgressUI,
        UIState::TimeCrystalProgress,
        Name::new("Time Crystal Focus"),
    ));
    cursor_y -= 14.;

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                shards_line,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., cursor_y, PANEL_CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalProgressUI,
        UIState::TimeCrystalProgress,
        Name::new("Time Crystal Shards Line"),
    ));
    cursor_y -= 20.;

    // Per-completed-crystal sections: header + heirloom icon row.
    for crystal_idx in newly_completed.iter().copied() {
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("Crystal #{} Completed!", crystal_idx + 1),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: LIGHT_GREEN,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., cursor_y, PANEL_CONTENT_Z)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            TimeCrystalProgressUI,
            UIState::TimeCrystalProgress,
            Name::new("Crystal Completed Header"),
        ));
        cursor_y -= 14.;

        let unlocks = time_crystal_heirlooms(crystal_idx);
        if unlocks.is_empty() {
            cursor_y -= 4.;
            continue;
        }

        let row_width = (unlocks.len() as f32 - 1.) * ICON_SPACING;
        let start_x = -row_width * 0.5;

        for (i, (heirloom, rarity)) in unlocks.into_iter().enumerate() {
            let icon_x = start_x + i as f32 * ICON_SPACING;
            commands.spawn((
                SpriteSheetBundle {
                    sprite: graphics.get_heirloom_icon(heirloom.clone()),
                    texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                    transform: Transform {
                        translation: Vec3::new(icon_x, cursor_y - ICON_SIZE * 0.5, PANEL_CONTENT_Z),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                Sprite {
                    custom_size: Some(Vec2::new(ICON_SIZE, ICON_SIZE)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                TimeCrystalProgressUI,
                UIState::TimeCrystalProgress,
                Interactable::default(),
                CrystalUnlockIcon {
                    heirloom: heirloom.clone(),
                    rarity: rarity.clone(),
                },
                Name::new("Crystal Unlock Icon"),
            ));
        }

        cursor_y -= ICON_SIZE + 12.;
    }

    // OK button
    let button_y = -half_h + 18.;
    let button_entity = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(50., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., button_y, PANEL_CONTENT_Z)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIElement::MenuButton,
            Interactable::default(),
            TimeCrystalProgressOKButton,
            TimeCrystalProgressUI,
            UIState::TimeCrystalProgress,
            Name::new("Time Crystal OK Button"),
        ))
        .id();

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "OK",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::TimeCrystalProgress)
        .insert(Name::new("Time Crystal OK Text"))
        .set_parent(button_entity);
}

/// Closes the popup when the OK button is clicked, and consumes the progress resource
/// so it doesn't pop up again on subsequent main-menu visits.
pub fn handle_time_crystal_progress_ok_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<TimeCrystalProgressOKButton>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable) in buttons.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        next_ui_state.set(UIState::Closed);
                        commands.remove_resource::<LastRunCrystalProgress>();
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            },
            _ => {
                interactable.change(Interaction::None);
            }
        }
    }
}

/// Spawn / despawn the heirloom tooltip card for whichever unlock icon is being hovered.
///
/// Main menu has no global hover driver for arbitrary [`Interactable`]s (unlike in-game HUD),
/// so we pointcast here and drive [`Interaction::Hovering`] ourselves, then send
/// [`HeirloomTooltipRequest`] for the shared tooltip processor.
pub fn handle_time_crystal_unlock_hover_tooltip(
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
    ui_state: Res<State<UIState>>,
    cursor_pos: Res<CursorPos>,
    icon_hit_targets: Query<
        (Entity, &Sprite, &GlobalTransform),
        (
            With<Interactable>,
            Or<(With<CrystalUnlockIcon>, With<HeirloomGridLockedCell>)>,
        ),
    >,
    mut icons: Query<
        (
            Entity,
            &mut Interactable,
            &GlobalTransform,
            Option<&CrystalUnlockIcon>,
            Option<&HeirloomGridLockedCell>,
        ),
        (
            With<Interactable>,
            Or<(With<CrystalUnlockIcon>, With<HeirloomGridLockedCell>)>,
        ),
    >,
    mut last_hovered: Local<Option<Entity>>,
) {
    let tooltip_ui_state = match ui_state.0 {
        UIState::TimeCrystalProgress => UIState::TimeCrystalProgress,
        UIState::TimeCrystalsBrowser => UIState::TimeCrystalsBrowser,
        _ => return,
    };

    let hit_entity = pointcast_unlock_tooltip_sprite_hit(&cursor_pos, &icon_hit_targets);

    for (entity, mut interactable, _, _, _) in icons.iter_mut() {
        let is_hit = hit_entity.map(|e| e == entity).unwrap_or(false);

        if is_hit && !matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::Hovering);
        } else if !is_hit && matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }

    let hovered_entity = icons
        .iter()
        .find(|(_, interactable, _, _, _)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(e, _, _, _, _)| e);

    if *last_hovered == hovered_entity {
        return;
    }

    match hovered_entity {
        None => tooltip_requests.send(HeirloomTooltipRequest::Clear),
        Some(entity) => {
            let Ok((_, _, transform, c_icon, l_cell)) = icons.get(entity) else {
                *last_hovered = hovered_entity;
                return;
            };
            let (heirloom, rarity) = if let Some(c) = c_icon {
                (c.heirloom.clone(), c.rarity)
            } else if let Some(l) = l_cell {
                (l.heirloom.clone(), l.rarity)
            } else {
                *last_hovered = hovered_entity;
                return;
            };
            let icon_pos = transform.translation();
            let tooltip_y = if icon_pos.y - 90. >= -100. {
                icon_pos.y - 90.
            } else {
                icon_pos.y + 100.
            };
            let tooltip_pos = Vec3::new(icon_pos.x, tooltip_y, TOOLTIP_Z);
            tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom,
                rarity,
                position: tooltip_pos,
                scaling_text: None,
                trigger_count_text: None,
                ui_state: Some(tooltip_ui_state),
            }));
        }
    }

    *last_hovered = hovered_entity;
}

pub fn cleanup_time_crystal_progress_ui(
    mut commands: Commands,
    query: Query<Entity, With<TimeCrystalProgressUI>>,
    tooltips: Query<Entity, With<super::heirloom_tooltip::HeirloomDynamicTooltip>>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    for entity in tooltips.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
