use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::text::Justify;
use rand::seq::SliceRandom;

use crate::{
    assets::Graphics,
    attributes::AttributeChangeEvent,
    blessings::{MajorHeirloomPickMode, PendingMajorHeirloomPick},
    colors::WHITE,
    cursor::CursorPos,
    juice::bounce::BounceOnHit,
    player::{
        skills::{Heirloom, HeirloomRarity, HeirloomWithRarity, PlayerSkills},
        Player,
    },
    ui::{
        game_fonts::{self as gf},
        heirloom_tooltip::{
            heirloom_hud_hover_tooltip_position, HeirloomTooltipRequest, HeirloomTooltipShow,
        },
        interactions::{Interactable, Interaction},
        Focusable, UIState,
    },
    ScreenResolution,
};

#[derive(Component)]
pub struct MajorHeirloomPickUI;

#[derive(Component)]
pub struct MajorHeirloomPickButton {
    pub heirloom: Heirloom,
}

const HEIRLOOM_BUTTON_HIT_SIZE: f32 = 28.0;

pub fn open_major_heirloom_pick_after_blessing(
    pending: Option<Res<PendingMajorHeirloomPick>>,
    ui_state: Res<State<UIState>>,
    mut next_ui: ResMut<NextState<UIState>>,
) {
    let Some(_) = pending else {
        return;
    };
    if *ui_state.get() == UIState::Closed {
        next_ui.set(UIState::MajorHeirloomPick);
    }
}

pub fn setup_major_heirloom_pick_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    player_skills: Query<&PlayerSkills>,
    pending: Option<Res<PendingMajorHeirloomPick>>,
    existing: Query<Entity, With<MajorHeirloomPickUI>>,
) {
    if !existing.is_empty() {
        return;
    }
    let Ok(skills) = player_skills.single() else {
        return;
    };
    let mode = pending
        .as_ref()
        .map(|p| p.mode)
        .unwrap_or(MajorHeirloomPickMode::TripleCopyLoseOne);
    let subtitle = match mode {
        MajorHeirloomPickMode::TripleCopyLoseOne => {
            "Gain 3 copies. Lose 1 random other uncommon."
        }
        MajorHeirloomPickMode::ConvertAllToChosen => {
            "Convert all other uncommons into copies of this one."
        }
    };

    let container = commands
        .spawn((
            Transform::from_translation(Vec3::new(0., 0., 50.)),
            Visibility::default(),
        ))
        .insert(MajorHeirloomPickUI)
        .insert(UIState::MajorHeirloomPick)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    commands
        .spawn((
            Sprite {
                color: Color::srgba(0.1, 0.1, 0.1, 0.85),
                custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&res)),
                ..default()
            },
            Transform::from_translation(Vec3::new(0., 0., 0.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    commands
        .spawn(
            gf::TITLE
                .text(&asset_server, "Choose an Uncommon Heirloom", WHITE)
                .justify(Justify::Center)
                .with_transform(Transform {
                    translation: Vec3::new(0., 70., 1.),
                    scale: gf::TITLE.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    commands
        .spawn(
            gf::BODY
                .text(&asset_server, subtitle, WHITE)
                .justify(Justify::Center)
                .with_transform(Transform {
                    translation: Vec3::new(0., 50., 1.),
                    scale: gf::BODY.transform_scale(),
                    ..default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    let mut type_counts = std::collections::HashMap::new();
    for h in &skills.heirlooms {
        if h.rarity == HeirloomRarity::Uncommon {
            *type_counts.entry(h.heirloom.clone()).or_insert(0) += 1;
        }
    }

    for (i, (heirloom, count)) in type_counts.iter().enumerate() {
        let row = i / 6;
        let col = i % 6;
        let offset = Vec2::new(-75. + (col as f32 * 30.), 10. - (row as f32 * 40.));

        let h_btn = commands
            .spawn((
                {
                    let mut sprite = graphics.get_heirloom_icon(heirloom.clone());
                    sprite.custom_size = Some(Vec2::splat(HEIRLOOM_BUTTON_HIT_SIZE));
                    sprite
                },
                Transform::from_translation(offset.extend(1.)),
            ))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Interactable::default())
            .insert(Focusable {
                group: UIState::MajorHeirloomPick,
                index: i as u32,
            })
            .insert(MajorHeirloomPickButton {
                heirloom: heirloom.clone(),
            })
            .insert(ChildOf(container))
            .id();

        commands
            .spawn(
                gf::HUD_MICRO
                    .text(&asset_server, format!("x{}", count), WHITE)
                    .justify(Justify::Center)
                    .with_transform(Transform {
                        translation: Vec3::new(0., -12., 1.),
                        scale: gf::HUD_MICRO.transform_scale(),
                        ..Default::default()
                    }),
            )
            .insert(RenderLayers::from_layers(&[3]))
            .insert(ChildOf(h_btn));
    }
}

pub fn handle_major_heirloom_pick_tooltip(
    mut tooltip_requests: MessageWriter<HeirloomTooltipRequest>,
    buttons: Query<
        (&MajorHeirloomPickButton, &GlobalTransform, &Interactable),
        With<MajorHeirloomPickButton>,
    >,
    res: Res<ScreenResolution>,
    mut last_hovered: Local<Option<Heirloom>>,
) {
    let currently_hovered = buttons
        .iter()
        .find(|(_, _, interactable)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(btn, transform, _)| (btn.heirloom.clone(), transform.translation()));

    let hovered_heirloom = currently_hovered.as_ref().map(|(h, _)| h.clone());
    if *last_hovered == hovered_heirloom {
        return;
    }

    match &currently_hovered {
        None => {
            let _ = tooltip_requests.write(HeirloomTooltipRequest::Clear);
        }
        Some((heirloom, icon_pos)) => {
            let rarity = HeirloomRarity::Uncommon;
            let (_, tooltip_size) = heirloom.get_ui_element(rarity);
            let mut tooltip_pos = heirloom_hud_hover_tooltip_position(
                *icon_pos,
                tooltip_size.x * 0.5,
                res.game_width,
            );
            tooltip_pos.y -= 10.;

            tooltip_requests.write(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom: heirloom.clone(),
                rarity,
                position: tooltip_pos,
                scaling_text: None,
                trigger_count: 0,
                ui_state: Some(UIState::MajorHeirloomPick),
            }));
        }
    }

    *last_hovered = hovered_heirloom;
}

fn point_in_sprite(cursor: &Vec2, hit_size: Vec2, global_transform: &GlobalTransform) -> bool {
    let pos = global_transform.translation().truncate();
    let half = hit_size * 0.5;
    cursor.x >= pos.x - half.x
        && cursor.x <= pos.x + half.x
        && cursor.y >= pos.y - half.y
        && cursor.y <= pos.y + half.y
}

pub fn handle_major_heirloom_pick_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut buttons: Query<(
        Entity,
        &GlobalTransform,
        &MajorHeirloomPickButton,
        &mut Interactable,
    )>,
    ui_root: Query<Entity, With<MajorHeirloomPickUI>>,
    mut player_skills: Query<(Entity, &mut PlayerSkills), With<Player>>,
    mut attribute_event: MessageWriter<AttributeChangeEvent>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    pending: Option<Res<PendingMajorHeirloomPick>>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let cursor = cursor_pos.ui_coords;
    let hit_size = Vec2::splat(HEIRLOOM_BUTTON_HIT_SIZE);

    for (e, global_transform, btn, mut interactable) in buttons.iter_mut() {
        let hit = cursor_pos.ui_hover_hit_allowed()
            && point_in_sprite(&cursor.truncate(), hit_size, global_transform);
        let is_focused = ui_focus.is_focused(e);
        let confirm_pressed =
            (hit && left_mouse_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        if hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.entity(e).insert(BounceOnHit::new());
                }
                Interaction::Hovering => {
                    if confirm_pressed {
                        let Ok((player_entity, mut skills)) = player_skills.single_mut() else {
                            return;
                        };

                        let chosen = btn.heirloom.clone();
                        let mode = pending
                            .as_ref()
                            .map(|p| p.mode)
                            .unwrap_or(MajorHeirloomPickMode::TripleCopyLoseOne);
                        match mode {
                            MajorHeirloomPickMode::TripleCopyLoseOne => {
                                // Remove one random *other* uncommon.
                                let other_indices: Vec<usize> = skills
                                    .heirlooms
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, h)| {
                                        h.rarity == HeirloomRarity::Uncommon
                                            && h.heirloom != chosen
                                    })
                                    .map(|(i, _)| i)
                                    .collect();
                                if let Some(&idx) =
                                    other_indices.choose(&mut rand::thread_rng())
                                {
                                    skills.heirlooms.remove(idx);
                                }
                                for _ in 0..3 {
                                    skills.heirlooms.push(HeirloomWithRarity {
                                        heirloom: chosen.clone(),
                                        rarity: HeirloomRarity::Uncommon,
                                    });
                                }
                            }
                            MajorHeirloomPickMode::ConvertAllToChosen => {
                                let other_count = skills
                                    .heirlooms
                                    .iter()
                                    .filter(|h| {
                                        h.rarity == HeirloomRarity::Uncommon
                                            && h.heirloom != chosen
                                    })
                                    .count();
                                skills.heirlooms.retain(|h| {
                                    !(h.rarity == HeirloomRarity::Uncommon
                                        && h.heirloom != chosen)
                                });
                                for _ in 0..other_count {
                                    skills.heirlooms.push(HeirloomWithRarity {
                                        heirloom: chosen.clone(),
                                        rarity: HeirloomRarity::Uncommon,
                                    });
                                }
                            }
                        }

                        chosen.add_heirloom_components(
                            player_entity,
                            &mut commands,
                            skills.clone(),
                        );
                        attribute_event.write(AttributeChangeEvent);

                        if let Ok(root) = ui_root.single() {
                            commands.entity(root).despawn();
                        }
                        commands.remove_resource::<PendingMajorHeirloomPick>();
                        next_ui_state.set(UIState::Closed);
                        return;
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
        }
    }
}

pub fn cleanup_major_heirloom_pick_ui(
    mut commands: Commands,
    ui_root: Query<Entity, With<MajorHeirloomPickUI>>,
) {
    for entity in ui_root.iter() {
        commands.entity(entity).despawn();
    }
}
