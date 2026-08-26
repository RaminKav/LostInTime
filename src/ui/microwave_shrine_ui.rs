use crate::{
    assets::Graphics,
    attributes::AttributeChangeEvent,
    colors::{WHITE, YELLOW},
    cursor::CursorPos,
    item::microwave_shrine::MicrowaveShrineState,
    item::WorldObject,
    juice::bounce::BounceOnHit,
    player::ModifyCurencyEvent,
    player::{
        skills::{Heirloom, HeirloomRarity, PlayerSkills},
        CoinCurrency,
    },
    ui::{
        damage_numbers::spawn_floating_text_with_shadow,
        game_fonts::{self as gf, FLOATING_TEXT},
        heirloom_tooltip::{
            heirloom_hud_hover_tooltip_position, HeirloomTooltipRequest, HeirloomTooltipShow,
        },
        interactions::{set_sprite_image, Interactable, Interaction},
        main_menu::{spawn_back_button, MAIN_MENU_WIDE_BUTTON_SIZE},
        Focusable, SkipFocusSelectedIndicator, UIElement, UIState,
    },
    ScreenResolution, GAME_HEIGHT,
};
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::text::Justify;

#[derive(Resource, Default)]
pub struct MicrowaveShrineUsages(pub u32);

#[derive(Component)]
pub struct MicrowaveShrineUI;

#[derive(Component)]
pub struct MicrowaveRarityButton {
    pub rarity: HeirloomRarity,
    pub cost: u32,
    pub can_afford: bool,
    pub has_enough_heirlooms: bool,
}

#[derive(Component)]
pub struct MicrowaveHeirloomButton {
    pub heirloom: Heirloom,
}

#[derive(Component)]
pub struct MicrowaveShrineEntityRef(pub Entity);

#[derive(Resource)]
pub struct MicrowaveShrineActive(pub Entity);

pub fn setup_microwave_shrine_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    shrines: Query<(Entity, &MicrowaveShrineState)>,
    usages: Res<MicrowaveShrineUsages>,
    player_skills: Query<&PlayerSkills>,
    coins: Res<CoinCurrency>,
) {
    let Ok(skills) = player_skills.single() else {
        return;
    };

    // Find the active shrine entity
    let mut shrine_entity = None;
    for (e, state) in shrines.iter() {
        if !state.is_used {
            shrine_entity = Some(e);
            break;
        }
    }

    let container = commands
        .spawn((
            Transform::from_translation(Vec3::new(0., 0., 50.)),
            Visibility::default(),
        ))
        .insert(MicrowaveShrineUI)
        .insert(UIState::MicrowaveShrine)
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    if let Some(e) = shrine_entity {
        commands
            .entity(container)
            .insert(MicrowaveShrineEntityRef(e));
        commands.insert_resource(MicrowaveShrineActive(e));
    }

    // BG
    let bg = commands
        .spawn((
            Sprite {
                color: Color::srgba(0.1, 0.1, 0.1, 0.95),
                custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&res)),
                ..default()
            },
            Transform::default(),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container))
        .id();

    // Title text (slkscrbold: 8.4 everywhere, see player_hud, class_selection, tips)
    commands
        .spawn(
            gf::MENU_TITLE
                .text(&asset_server, "Heirloom Swap Shrine", WHITE)
                .justify(Justify::Center)
                .with_transform(Transform {
                    translation: Vec3::new(0., GAME_HEIGHT / 2. - 60., 2.),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    commands
        .spawn(gf::BODY.text(&asset_server, "Select heirloom rarity, then select an heirloom to gain. \nCosts coins and consumes a random heirloom of the same rarity", WHITE).justify(Justify::Center).with_transform(Transform {
                translation: Vec3::new(0., GAME_HEIGHT / 2. - 100., 2.),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            }))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    // Gold count (slkscr: 8.4)
    commands
        .spawn(
            gf::BODY
                .text(&asset_server, format!("Gold: {}", coins.coins), YELLOW)
                .justify(Justify::Center)
                .with_transform(Transform {
                    translation: Vec3::new(0., GAME_HEIGHT / 2. - 80., 2.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(container));

    let rarities = [
        (HeirloomRarity::Common, 10),
        (HeirloomRarity::Uncommon, 20),
        (HeirloomRarity::Rare, 50),
    ];

    for (i, (rarity, base_cost)) in rarities.iter().enumerate() {
        let cost = base_cost * 2u32.pow(usages.0);
        let can_afford = coins.coins >= cost;

        let mut types_of_rarity = std::collections::HashSet::new();
        for h in &skills.heirlooms {
            if h.rarity == *rarity {
                types_of_rarity.insert(h.heirloom.clone());
            }
        }
        let has_enough_heirlooms = types_of_rarity.len() >= 2;

        let btn_size = MAIN_MENU_WIDE_BUTTON_SIZE;

        let btn = commands
            .spawn((
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::MainMenuStartButton),
                    custom_size: Some(btn_size),
                    ..default()
                },
                Transform::from_translation(Vec3::new(0., 30. - (i as f32 * 40.), 1.)),
            ))
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIElement::MainMenuStartButton)
            .insert(Interactable::default())
            .insert(Focusable {
                group: UIState::MicrowaveShrine,
                index: i as u32,
            })
            .insert(SkipFocusSelectedIndicator)
            .insert(MicrowaveRarityButton {
                rarity: rarity.clone(),
                cost,
                can_afford,
                has_enough_heirlooms,
            })
            .insert(ChildOf(container))
            .id();

        let rarity_str = match rarity {
            HeirloomRarity::Common => "Common",
            HeirloomRarity::Uncommon => "Uncommon",
            HeirloomRarity::Rare => "Rare",
            _ => "Unknown",
        };

        commands
            .spawn(
                gf::TITLE
                    .text(&asset_server, format!("{} ({}g)", rarity_str, cost), WHITE)
                    .justify(Justify::Center)
                    .with_transform(Transform {
                        translation: Vec3::new(0., -1., 1.),
                        scale: gf::TITLE.transform_scale(),
                        ..Default::default()
                    }),
            )
            .insert(RenderLayers::from_layers(&[3]))
            .insert(ChildOf(btn));
    }

    // Exit / Back button — child of container with high z so it draws above the overlay
    let back_button = spawn_back_button(
        Vec3::new(
            res.game_width / 2. - 55.,
            -res.game_height / 2. + 38.,
            60., // above overlay (container z=50, bg at 0 local)
        ),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands
        .entity(back_button)
        .insert(ChildOf(container))
        .insert(UIState::MicrowaveShrine)
        .insert(Focusable {
            group: UIState::MicrowaveShrine,
            index: 100,
        })
        .insert(SkipFocusSelectedIndicator);
}

fn point_in_sprite(cursor: &Vec3, size: Vec2, xform: &GlobalTransform) -> bool {
    let pos = xform.translation();
    let initial_x = pos.x - 0.5 * size.x;
    let initial_y = pos.y - 0.5 * size.y;
    (initial_x..=initial_x + size.x).contains(&cursor.x)
        && (initial_y..=initial_y + size.y).contains(&cursor.y)
}

pub fn handle_microwave_shrine_rarity_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut buttons: Query<(
        Entity,
        &Sprite,
        &GlobalTransform,
        &MicrowaveRarityButton,
        &mut Interactable,
    )>,
    ui_root: Query<Entity, With<MicrowaveShrineUI>>,
    rarity_buttons_to_remove: Query<(Entity, &ChildOf), With<MicrowaveRarityButton>>,
    player_skills: Query<&PlayerSkills>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let cursor = cursor_pos.ui_coords;

    for (e, sprite, global_transform, btn, mut interactable) in buttons.iter_mut() {
        let size = sprite.custom_size.unwrap_or(Vec2::ZERO);
        let hit =
            cursor_pos.ui_hover_hit_allowed() && point_in_sprite(&cursor, size, global_transform);

        let enabled = btn.can_afford && btn.has_enough_heirlooms;
        let is_focused = ui_focus.is_focused(e);
        let confirm_pressed =
            (hit && left_mouse_pressed) || (is_focused && ui_focus.confirm_just_pressed);

        if hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    if enabled {
                        interactable.change(Interaction::Hovering);
                        commands
                            .entity(e)
                            .insert(UIElement::MainMenuStartButtonHover)
                            .insert(Sprite {
                                image: graphics
                                    .get_ui_element_texture(UIElement::MainMenuStartButtonHover),
                                ..default()
                            });
                    }
                }
                Interaction::Hovering => {
                    if !enabled {
                        // Became disabled or cursor moved to a disabled button: clear hover
                        interactable.change(Interaction::None);
                        commands
                            .entity(e)
                            .insert(UIElement::MainMenuStartButton)
                            .insert(Sprite {
                                image: graphics
                                    .get_ui_element_texture(UIElement::MainMenuStartButton),
                                ..default()
                            });
                    } else if confirm_pressed {
                        if let Ok(root) = ui_root.single() {
                            // Remove only the three rarity buttons; keep overlay (bg, title, gold)
                            let to_despawn: Vec<Entity> = rarity_buttons_to_remove
                                .iter()
                                .filter(|(_, p)| p.parent() == root)
                                .map(|(e, _)| e)
                                .collect();
                            for entity in to_despawn {
                                commands.entity(entity).despawn();
                            }

                            let Ok(skills) = player_skills.single() else {
                                return;
                            };

                            let mut type_counts = std::collections::HashMap::new();
                            for h in &skills.heirlooms {
                                if h.rarity == btn.rarity {
                                    *type_counts.entry(h.heirloom.clone()).or_insert(0) += 1;
                                }
                            }

                            for (i, (heirloom, count)) in type_counts.iter().enumerate() {
                                let row = i / 6;
                                let col = i % 6;
                                let offset =
                                    Vec2::new(-75. + (col as f32 * 30.), 30. - (row as f32 * 40.));

                                let h_btn = commands
                                    .spawn((
                                        {
                                            let mut sprite =
                                                graphics.get_heirloom_icon(heirloom.clone());
                                            sprite.custom_size =
                                                Some(Vec2::splat(HEIRLOOM_BUTTON_HIT_SIZE));
                                            sprite
                                        },
                                        Transform::from_translation(offset.extend(1.)),
                                    ))
                                    .insert(RenderLayers::from_layers(&[3]))
                                    .insert(Interactable::default())
                                    .insert(Focusable {
                                        group: UIState::MicrowaveShrine,
                                        index: 10 + i as u32,
                                    })
                                    .insert(MicrowaveHeirloomButton {
                                        heirloom: heirloom.clone(),
                                    })
                                    .insert(ChildOf(root))
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
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            commands.entity(e).insert(UIElement::MainMenuStartButton);
            set_sprite_image(
                &mut commands,
                e,
                graphics.get_ui_element_texture(UIElement::MainMenuStartButton),
            );
        }
    }
}

/// Show the shared heirloom hover card for whichever swap icon is under the cursor.
pub fn handle_microwave_shrine_heirloom_tooltip(
    mut tooltip_requests: MessageWriter<HeirloomTooltipRequest>,
    buttons: Query<
        (&MicrowaveHeirloomButton, &GlobalTransform, &Interactable),
        With<MicrowaveHeirloomButton>,
    >,
    player_skills: Query<&PlayerSkills>,
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
            let Ok(skills) = player_skills.single() else {
                *last_hovered = hovered_heirloom;
                return;
            };

            let rarity = skills
                .heirlooms
                .iter()
                .find(|h| h.heirloom == *heirloom)
                .map(|h| h.rarity)
                .unwrap_or(HeirloomRarity::Common);

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
                ui_state: Some(UIState::MicrowaveShrine),
            }));
        }
    }

    *last_hovered = hovered_heirloom;
}

/// Hit size for heirloom icon buttons (SpriteSheetBundle has no Sprite with custom_size)
const HEIRLOOM_BUTTON_HIT_SIZE: f32 = 28.0;

pub fn handle_microwave_shrine_heirloom_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut buttons: Query<(
        Entity,
        &GlobalTransform,
        &MicrowaveHeirloomButton,
        &mut Interactable,
    )>,
    ui_root: Query<(Entity, &MicrowaveShrineEntityRef), With<MicrowaveShrineUI>>,
    mut player_skills: Query<&mut PlayerSkills>,
    mut usages: ResMut<MicrowaveShrineUsages>,
    mut shrines: Query<&mut MicrowaveShrineState>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    mut modify_currency: MessageWriter<ModifyCurencyEvent>,
    mut attribute_event: MessageWriter<AttributeChangeEvent>,
    player_query: Query<(Entity, &Transform), With<crate::player::Player>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let cursor = cursor_pos.ui_coords;
    let hit_size = Vec2::splat(HEIRLOOM_BUTTON_HIT_SIZE);

    for (e, global_transform, btn, mut interactable) in buttons.iter_mut() {
        let hit = cursor_pos.ui_hover_hit_allowed()
            && point_in_sprite(&cursor, hit_size, global_transform);
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
                        let Ok(mut skills) = player_skills.single_mut() else {
                            return;
                        };
                        let Ok((root, shrine_ref)) = ui_root.single() else {
                            return;
                        };

                        // Determine rarity
                        let rarity = skills
                            .heirlooms
                            .iter()
                            .find(|h| h.heirloom == btn.heirloom)
                            .unwrap()
                            .rarity
                            .clone();

                        // Calculate cost and pay
                        let base_cost = match rarity {
                            HeirloomRarity::Common => 10,
                            HeirloomRarity::Uncommon => 20,
                            HeirloomRarity::Rare => 50,
                            _ => 10,
                        };
                        let cost = base_cost * 2u32.pow(usages.0);

                        modify_currency.write(ModifyCurencyEvent {
                            delta: -(cost as i32),
                            obj: WorldObject::Coin,
                        });

                        // Find a different heirloom of the same rarity to consume
                        let mut consume_index = None;
                        for (i, h) in skills.heirlooms.iter().enumerate() {
                            if h.rarity == rarity && h.heirloom != btn.heirloom {
                                consume_index = Some(i);
                                break;
                            }
                        }

                        if let Some(idx) = consume_index {
                            let consumed = skills.heirlooms.remove(idx).heirloom;

                            // Add duplicate (same as essence_ui / skill_choice: components + attribute recalc)
                            skills
                                .heirlooms
                                .push(crate::player::skills::HeirloomWithRarity {
                                    heirloom: btn.heirloom.clone(),
                                    rarity: rarity.clone(),
                                });

                            usages.0 += 1;

                            if let Ok(mut shrine_state) = shrines.get_mut(shrine_ref.0) {
                                shrine_state.is_used = true;
                            }

                            // Add/update heirloom components, trigger attribute recalc, and show floating text (like essence_ui, item_chest)
                            if let Ok((player_entity, pt)) = player_query.single() {
                                btn.heirloom.add_heirloom_components(
                                    player_entity,
                                    &mut commands,
                                    skills.clone(),
                                );
                                attribute_event.write(AttributeChangeEvent);
                                // Format: "-1 " {icon} {name}. Use layer 0 so icon/name show (main text is layer 0; layer 3 is UI-only).
                                let color = rarity.get_color();
                                let floating_text = spawn_floating_text_with_shadow(
                                    &mut commands,
                                    &asset_server,
                                    pt.translation + Vec3::new(0., 40., 15.),
                                    color,
                                    "-1 ".to_string(),
                                    FLOATING_TEXT,
                                );
                                // Keep shadow + children on layer 0 so game camera renders them with the main "-1 " text
                                commands
                                    .entity(floating_text)
                                    .insert(RenderLayers::from_layers(&[0]));
                                // Icon after "-1 " (child of shadow entity)
                                let icon_entity = commands
                                    .spawn((
                                        graphics.get_heirloom_icon(consumed.clone()),
                                        Transform::from_translation(Vec3::new(4., 0., 4.)),
                                    ))
                                    .insert(RenderLayers::from_layers(&[0]))
                                    .id();
                                commands.entity(floating_text).add_child(icon_entity);
                            }
                        }

                        commands.remove_resource::<MicrowaveShrineActive>();
                        next_ui_state.set(UIState::Closed);
                        commands.entity(root).despawn();
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
