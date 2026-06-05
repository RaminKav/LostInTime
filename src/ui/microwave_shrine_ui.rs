use crate::{
    assets::Graphics,
    attributes::AttributeChangeEvent,
    colors::{BLACK, WHITE, YELLOW},
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
        game_fonts::FLOATING_TEXT,
        heirloom_tooltip::{
            heirloom_hud_hover_tooltip_position, HeirloomTooltipRequest, HeirloomTooltipShow,
        },
    },
    ui::{
        interactions::{Interactable, Interaction},
        main_menu::spawn_back_button,
        UIElement, UIState,
    },
    ScreenResolution, GAME_HEIGHT,
};
use bevy::prelude::*;
use bevy::render::view::RenderLayers;

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
    let Ok(skills) = player_skills.get_single() else {
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
        .spawn(SpatialBundle {
            transform: Transform::from_translation(Vec3::new(0., 0., 50.)),
            ..Default::default()
        })
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
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.1, 0.1, 0.1, 0.95),
                custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&res)),
                ..default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(container)
        .id();

    // Title text (slkscrbold: 8.4 everywhere, see player_hud, class_selection, tips)
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Heirloom Swap Shrine",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            transform: Transform::from_translation(Vec3::new(0., GAME_HEIGHT / 2. - 20., 2.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(container);

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Select heirloom rarity, then select an heirloom to gain. \nCosts coins and consumes a random heirloom of the same rarity",
                TextStyle {
                    font: asset_server.load("fonts/slkscr.ttf"),
                    font_size: 8.4,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            transform: Transform::from_translation(Vec3::new(0., GAME_HEIGHT / 2. - 60., 2.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(container);

    // Gold count (slkscr: 8.4)
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                format!("Gold: {}", coins.coins),
                TextStyle {
                    font: asset_server.load("fonts/slkscr.ttf"),
                    font_size: 8.4,
                    color: YELLOW,
                },
            )
            .with_alignment(TextAlignment::Center),
            transform: Transform::from_translation(Vec3::new(0., GAME_HEIGHT / 2. - 40., 2.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(container);

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

        let btn_size = Vec2::new(100., 30.);

        let btn = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(btn_size),
                    ..default()
                },
                transform: Transform::from_translation(Vec3::new(0., 30. - (i as f32 * 40.), 1.)),
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIElement::MenuButton)
            .insert(Interactable::default())
            .insert(MicrowaveRarityButton {
                rarity: rarity.clone(),
                cost,
                can_afford,
                has_enough_heirlooms,
            })
            .set_parent(container)
            .id();

        let rarity_str = match rarity {
            HeirloomRarity::Common => "Common",
            HeirloomRarity::Uncommon => "Uncommon",
            HeirloomRarity::Rare => "Rare",
            _ => "Unknown",
        };

        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    format!("{} ({}g)", rarity_str, cost),
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: BLACK,
                    },
                )
                .with_alignment(TextAlignment::Center),
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(btn);
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
        .set_parent(container)
        .insert(UIState::MicrowaveShrine);
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
    mouse_input: Res<Input<MouseButton>>,
    mut buttons: Query<(
        Entity,
        &Sprite,
        &GlobalTransform,
        &MicrowaveRarityButton,
        &mut Interactable,
    )>,
    ui_root: Query<Entity, With<MicrowaveShrineUI>>,
    rarity_buttons_to_remove: Query<(Entity, &Parent), With<MicrowaveRarityButton>>,
    player_skills: Query<&PlayerSkills>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let cursor = cursor_pos.ui_coords;

    for (e, sprite, global_transform, btn, mut interactable) in buttons.iter_mut() {
        let size = sprite.custom_size.unwrap_or(Vec2::ZERO);
        let hit = point_in_sprite(&cursor, size, global_transform);

        let enabled = btn.can_afford && btn.has_enough_heirlooms;

        if hit {
            match interactable.current() {
                Interaction::None => {
                    if enabled {
                        interactable.change(Interaction::Hovering);
                        commands
                            .entity(e)
                            .insert(UIElement::MenuButtonHover)
                            .insert(graphics.get_ui_element_texture(UIElement::MenuButtonHover));
                    }
                }
                Interaction::Hovering => {
                    if !enabled {
                        // Became disabled or cursor moved to a disabled button: clear hover
                        interactable.change(Interaction::None);
                        commands
                            .entity(e)
                            .insert(UIElement::MenuButton)
                            .insert(graphics.get_ui_element_texture(UIElement::MenuButton));
                    } else if left_mouse_pressed {
                        if let Ok(root) = ui_root.get_single() {
                            // Remove only the three rarity buttons; keep overlay (bg, title, gold)
                            let to_despawn: Vec<Entity> = rarity_buttons_to_remove
                                .iter()
                                .filter(|(_, p)| p.get() == root)
                                .map(|(e, _)| e)
                                .collect();
                            for entity in to_despawn {
                                commands.entity(entity).despawn_recursive();
                            }

                            let Ok(skills) = player_skills.get_single() else {
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
                                    .spawn(SpriteSheetBundle {
                                        sprite: graphics.get_heirloom_icon(heirloom.clone()),
                                        texture_atlas: graphics
                                            .texture_atlas
                                            .as_ref()
                                            .unwrap()
                                            .clone(),
                                        transform: Transform::from_translation(offset.extend(1.)),
                                        ..default()
                                    })
                                    .insert(Sprite {
                                        custom_size: Some(Vec2::splat(HEIRLOOM_BUTTON_HIT_SIZE)),
                                        ..default()
                                    })
                                    .insert(RenderLayers::from_layers(&[3]))
                                    .insert(Interactable::default())
                                    .insert(MicrowaveHeirloomButton {
                                        heirloom: heirloom.clone(),
                                    })
                                    .set_parent(root)
                                    .id();

                                commands
                                    .spawn(Text2dBundle {
                                        text: Text::from_section(
                                            format!("x{}", count),
                                            TextStyle {
                                                font: asset_server.load("fonts/slkscr.ttf"),
                                                font_size: 8.4,
                                                color: WHITE,
                                            },
                                        )
                                        .with_alignment(TextAlignment::Center),
                                        transform: Transform::from_translation(Vec3::new(
                                            0., -12., 1.,
                                        )),
                                        ..Default::default()
                                    })
                                    .insert(RenderLayers::from_layers(&[3]))
                                    .set_parent(h_btn);
                            }
                        }
                    }
                }
                _ => {}
            }
        } else {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(UIElement::MenuButton)
                    .insert(graphics.get_ui_element_texture(UIElement::MenuButton));
            }
        }
    }
}

/// Show the shared heirloom hover card for whichever swap icon is under the cursor.
pub fn handle_microwave_shrine_heirloom_tooltip(
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
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
        None => tooltip_requests.send(HeirloomTooltipRequest::Clear),
        Some((heirloom, icon_pos)) => {
            let Ok(skills) = player_skills.get_single() else {
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
            let tooltip_pos = heirloom_hud_hover_tooltip_position(
                *icon_pos,
                tooltip_size.x * 0.5,
                res.game_width,
            );

            tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
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
    mouse_input: Res<Input<MouseButton>>,
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
    mut modify_currency: EventWriter<ModifyCurencyEvent>,
    mut attribute_event: EventWriter<AttributeChangeEvent>,
    player_query: Query<(Entity, &Transform), With<crate::player::Player>>,
) {
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let cursor = cursor_pos.ui_coords;
    let hit_size = Vec2::splat(HEIRLOOM_BUTTON_HIT_SIZE);

    for (e, global_transform, btn, mut interactable) in buttons.iter_mut() {
        let hit = point_in_sprite(&cursor, hit_size, global_transform);

        if hit {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.entity(e).insert(BounceOnHit::new());
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        let Ok(mut skills) = player_skills.get_single_mut() else {
                            return;
                        };
                        let Ok((root, shrine_ref)) = ui_root.get_single() else {
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

                        modify_currency.send(ModifyCurencyEvent {
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
                            if let Ok((player_entity, pt)) = player_query.get_single() {
                                btn.heirloom.add_heirloom_components(
                                    player_entity,
                                    &mut commands,
                                    skills.clone(),
                                );
                                attribute_event.send(AttributeChangeEvent);
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
                                    .spawn(SpriteSheetBundle {
                                        sprite: graphics.get_heirloom_icon(consumed.clone()),
                                        texture_atlas: graphics
                                            .texture_atlas
                                            .as_ref()
                                            .unwrap()
                                            .clone(),
                                        transform: Transform::from_translation(Vec3::new(
                                            4., 0., 4.,
                                        )),
                                        ..default()
                                    })
                                    .insert(RenderLayers::from_layers(&[0]))
                                    .id();
                                commands.entity(floating_text).add_child(icon_entity);
                                // Name after icon (same font/size as floating text)
                                // let name_entity = commands
                                //     .spawn(Text2dBundle {
                                //         text: Text::from_section(
                                //             consumed.get_title(),
                                //             TextStyle {
                                //                 font: asset_server.load("fonts/slkscr.ttf"),
                                //                 font_size: 8.4,
                                //                 color,
                                //             },
                                //         ),
                                //         text_anchor: bevy::sprite::Anchor::CenterLeft,
                                //         transform: Transform::from_translation(Vec3::new(
                                //             5., 0., 4.,
                                //         )),
                                //         ..default()
                                //     })
                                //     .insert(RenderLayers::from_layers(&[0]))
                                //     .id();
                                // commands.entity(floating_text).add_child(name_entity);
                            }
                        }

                        commands.remove_resource::<MicrowaveShrineActive>();
                        next_ui_state.set(UIState::Closed);
                        commands.entity(root).despawn_recursive();
                        return;
                    }
                }
                _ => {}
            }
        } else {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
    }
}
