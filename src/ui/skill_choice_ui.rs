use crate::aseprite_assets::SkillChoiceFlash;
use crate::aseprite_helpers::aseprite_bundle;
use bevy::text::Justify;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};
use bevy_aseprite_ultra::prelude::{AnimationState, Aseprite};

use crate::{
    animations::DoneAnimation,
    assets::Graphics,
    attributes::LootRateBonus,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{WHITE, YELLOW_2},
    cursor::CursorPos,
    item::item_drop_outline::UiShadow,
    juice::bounce::BounceOnHit,
    player::{
        levels::PlayerLevel,
        skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState},
        time_crystals::TimeCrystals,
        unlocks::RunUnlockState,
        Player,
    },
    ui::{
        essence_ui::{MERCHANT_REROLL_ICON_PATH, MERCHANT_REROLL_ICON_SIZE},
        game_fonts as gf,
        ui_helpers::{self, spawn_full_screen_ui_overlay_tuned},
        CheatSettings, KEYBIND_BADGE_COLOR,
    },
    ScreenResolution, DEBUG,
};

use super::{
    banish_tracker_ui::spawn_banish_tracker,
    heirloom_tooltip::spawn_heirloom_tooltip_card,
    interactions::Interaction,
    tooltip_info_boxes::{
        build_tooltip_info_boxes, spawn_tooltip_info_boxes_with_resolution, TooltipInfoBoxAnchor,
    },
    ui_helpers::{
        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT, Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_OVERLAY,
    },
    Focusable, Interactable, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
};

/// Bounce strength for heirloom choice cards on hover (fraction of default mob bounce).
const HEIRLOOM_CARD_BOUNCE_STRENGTH: f32 = 0.4;
/// Raised into the old per-slot dice row so banishes sit closer under the cards.
const SKILL_CHOICE_BANISH_Y: f32 = -120.;
/// Centered shrine-style reroll badge beneath the banish row.
const SKILL_CHOICE_REROLL_BUTTON_Y: f32 = -142.;
const SKILL_CHOICE_REROLL_BADGE_SIZE: Vec2 = Vec2::new(14., 12.);
const SKILL_CHOICE_COUNT_TEXT_Y: f32 = -142.;

/// Side info boxes spawned on hover for a level-up choice card (despawned on unhover).
#[derive(Component)]
pub struct SkillChoiceInfoBoxRoot;

#[derive(Component)]
pub struct SkillChoiceUI {
    pub index: usize,
    pub skill_choice: HeirloomChoiceState,
    pub interaction_lock_timer: Timer,
}

/// Centered badge button that rerolls the whole heirloom offer.
#[derive(Component)]
pub struct SkillChoiceRerollButton;

#[derive(Component)]
pub struct SkillChoiceRerollIcon;

/// Flash overlay spawned over each card while a global reroll plays out.
#[derive(Component)]
pub struct SkillChoiceRerollFlash;

#[derive(Component)]
pub struct BanishButton(pub usize);

#[derive(Component)]
pub struct BanishButtonLabel(pub usize);

#[derive(Component)]
pub struct RerollCountText;

#[derive(Component)]
pub struct BanishCountText;

pub fn handle_skill_choice_info_box_hover(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    cards: Query<(Entity, &SkillChoiceUI, &Interactable, &GlobalTransform)>,
    existing_roots: Query<Entity, With<SkillChoiceInfoBoxRoot>>,
    mut last_card: Local<Option<Entity>>,
) {
    let hovered = cards
        .iter()
        .find(|(_, _, interactable, _)| matches!(interactable.current(), Interaction::Hovering));

    let hovered_card = hovered.map(|(e, _, _, _)| e);

    if *last_card == hovered_card {
        return;
    }

    for root in existing_roots.iter() {
        commands.entity(root).despawn();
    }

    *last_card = hovered_card;

    let Some((card_e, choice, _, transform)) = hovered else {
        return;
    };

    let (_, size) = choice
        .skill_choice
        .heirloom
        .get_ui_element(choice.skill_choice.rarity.clone());
    let specs = build_tooltip_info_boxes(choice.skill_choice.heirloom.clone(), 0);
    if specs.is_empty() {
        return;
    }

    if let Some(root) = spawn_tooltip_info_boxes_with_resolution(
        &mut commands,
        &graphics,
        &asset_server,
        &resolution,
        TooltipInfoBoxAnchor {
            center: transform.translation(),
            half_width: size.x * 0.5,
            half_height: size.y * 0.5,
            game_width: resolution.game_width,
            prefer_left: false,
        },
        &specs,
    ) {
        commands
            .entity(root)
            .insert(SkillChoiceInfoBoxRoot)
            .insert(ChildOf(card_e));
    }
}

pub fn setup_skill_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    choices_queue: Res<HeirloomChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    res: Res<ScreenResolution>,
    run_unlocks: Res<RunUnlockState>,
    time_crystals: Res<TimeCrystals>,
) {
    if choices_queue.queue.is_empty() {
        next_ui_state.set(UIState::Closed);
        return;
    }
    let asset_server = asset_server.as_ref();
    let choices = &choices_queue.queue[0];
    let t_offset = Vec2::new(4., 4.);

    let title_text = commands
        .spawn((
            gf::MENU_TITLE
                .text(&asset_server, "Choose an Heirloom".to_string(), WHITE)
                .with_transform(Transform {
                    translation: Vec3::new(0., 115., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT),
                    scale: gf::MENU_TITLE.transform_scale(),
                    ..Default::default()
                }),
            RenderLayers::from_layers(&[3]),
        ))
        .id();

    commands.entity(title_text).insert(UIState::Skills);

    let heirloom_choice_overlay = spawn_full_screen_ui_overlay_tuned(
        &mut commands,
        &res,
        0.0,
        0.95,
        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_OVERLAY,
    );
    commands
        .entity(heirloom_choice_overlay)
        .insert(UIState::Skills);

    spawn_skill_choice_entities(
        &graphics,
        &mut commands,
        &asset_server,
        &res,
        choices.clone().to_vec(),
        t_offset,
    );

    spawn_skill_choice_reroll_button(
        &mut commands,
        &asset_server,
        run_unlocks.rerolls_remaining > 0,
    );

    for i in -1i32..2 {
        let slot_index = (i + 1) as usize;
        let slot_ok =
            choices_queue.banish_allowed_for_choice_slot(time_crystals.as_ref(), slot_index);
        let banish_enabled = run_unlocks.banishes_remaining > 0 && slot_ok;
        let translation = Vec3::new(
            i as f32 * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.,
            SKILL_CHOICE_BANISH_Y,
            Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT,
        );
        let mut banish_button = commands.spawn((
            Sprite {
                image: graphics
                    .get_ui_element_texture(UIElement::BackButton)
                    .clone(),
                custom_size: Some(Vec2::new(48., 18.)),
                color: if banish_enabled {
                    Color::WHITE
                } else {
                    Color::srgb(0.5, 0.5, 0.5)
                },
                ..Default::default()
            },
            Transform {
                translation,
                ..Default::default()
            },
        ));
        let banish_entity = banish_button.id();
        banish_button
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::Skills)
            .insert(UIElement::BackButton)
            .insert(BanishButton(slot_index))
            .insert(Interactable::default())
            .insert(Focusable {
                group: UIState::Skills,
                index: 20 + slot_index as u32,
            })
            .insert(UiShadow::container())
            .insert(Name::new(format!("BANISH BUTTON {slot_index}")));

        commands
            .spawn((
                gf::SKILL_CHOICE_MICRO
                    .text(
                        &asset_server,
                        "Banish ",
                        if banish_enabled {
                            WHITE
                        } else {
                            Color::srgb(0.7, 0.7, 0.7)
                        },
                    )
                    .justify(Justify::Center)
                    .anchor(Anchor::CENTER)
                    .with_transform(Transform {
                        translation: Vec3::new(2., 0., 1.),
                        scale: gf::SKILL_CHOICE_MICRO.transform_scale(),
                        ..Default::default()
                    }),
                RenderLayers::from_layers(&[3]),
                UIState::Skills,
                BanishButtonLabel(slot_index),
                Name::new(format!("BANISH BUTTON TEXT {slot_index}")),
            ))
            .insert(ChildOf(banish_entity));
    }

    spawn_banish_tracker(
        &mut commands,
        &asset_server,
        &graphics,
        choices_queue.as_ref(),
        time_crystals.as_ref(),
        &res,
        UIState::Skills,
    );

    commands.spawn((
        gf::SKILL_CHOICE_MICRO
            .text(
                &asset_server,
                format!("Rerolls: {}", run_unlocks.rerolls_remaining),
                WHITE,
            )
            .justify(Justify::Center)
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(
                    -80.5,
                    SKILL_CHOICE_COUNT_TEXT_Y,
                    Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                ),
                scale: gf::SKILL_CHOICE_MICRO.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        UIState::Skills,
        RerollCountText,
        Name::new("Reroll Count Text2d"),
    ));

    commands.spawn((
        gf::SKILL_CHOICE_MICRO
            .text(
                &asset_server,
                format!("Banishes: {}", run_unlocks.banishes_remaining),
                WHITE,
            )
            .justify(Justify::Center)
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(
                    80.,
                    SKILL_CHOICE_COUNT_TEXT_Y,
                    Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
                ),
                scale: gf::SKILL_CHOICE_MICRO.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        UIState::Skills,
        BanishCountText,
        Name::new("Banish Count Text2d"),
    ));
}

fn spawn_skill_choice_reroll_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    enabled: bool,
) {
    let icon_color = if enabled {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };

    let mut btn = commands.spawn((
        Sprite {
            color: KEYBIND_BADGE_COLOR,
            custom_size: Some(SKILL_CHOICE_REROLL_BADGE_SIZE),
            ..default()
        },
        Transform::from_translation(Vec3::new(
            0.,
            SKILL_CHOICE_REROLL_BUTTON_Y,
            Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT,
        )),
    ));
    btn.insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::Skills)
        .insert(SkillChoiceRerollButton)
        .insert(UiShadow::container())
        .insert(Name::new("Skill Choice Reroll"));

    if enabled {
        btn.insert(Interactable::default()).insert(Focusable {
            group: UIState::Skills,
            index: 10,
        });
    }

    let btn_e = btn.id();

    commands
        .spawn((
            Sprite {
                image: asset_server.load(MERCHANT_REROLL_ICON_PATH),
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                color: icon_color,
                ..default()
            },
            Transform::from_translation(Vec3::new(0., 0., 1.)),
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(SkillChoiceRerollIcon)
        .insert(ChildOf(btn_e));
}

pub fn tick_skill_choice_interaction_lock_timers(
    time: Res<Time>,
    mut query: Query<&mut SkillChoiceUI>,
) {
    for mut skill_ui in query.iter_mut() {
        if skill_ui.interaction_lock_timer.is_finished() {
            continue;
        }
        skill_ui.interaction_lock_timer.tick(time.delta());
    }
}

pub fn spawn_skill_choice_entities(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    resolution: &ScreenResolution,
    choices: Vec<HeirloomChoiceState>,
    t_offset: Vec2,
) {
    let COUNT: usize = 3;
    for i in -1i32..(COUNT as i32 - 1) {
        let choice = choices[(i + 1) as usize].clone();
        let (_, size) = choice.heirloom.get_ui_element(choice.rarity.clone());
        let translation = Vec2::new(
            i as f32 * (size.x + 8.) + if COUNT == 2 { size.x / 2. } else { 0. } + 0.1,
            0.,
        );
        if choice.heirloom == crate::player::skills::Heirloom::None {
            continue;
        }
        let index = (i + 1) as usize;
        let position = Vec3::new(
            (translation.x + t_offset.x).round(),
            (translation.y + t_offset.y).round(),
            Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT,
        );
        let card_e = spawn_heirloom_tooltip_card(
            graphics,
            commands,
            asset_server,
            resolution,
            choice.heirloom.clone(),
            choice.rarity.clone(),
            position,
            None,
            0,
            false,
            UiShadow::container(),
        );
        commands
            .entity(card_e)
            .insert(UIState::Skills)
            .insert(SkillChoiceUI {
                index,
                skill_choice: choice,
                interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
            })
            .insert(Interactable::default())
            .insert(Focusable {
                group: UIState::Skills,
                index: index as u32,
            })
            .insert(BounceOnHit::with_strength_fraction(
                HEIRLOOM_CARD_BOUNCE_STRENGTH,
            ))
            .insert(Name::new("SKILLS UI"));
    }
}

pub fn toggle_skills_visibility(
    curr_ui_state: Res<State<UIState>>,
    key_input: ResMut<ButtonInput<KeyCode>>,
    mut queue: ResMut<HeirloomChoiceQueue>,
    old_skill_entities: Query<Entity, With<SkillChoiceUI>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    player_atts: Query<(&LootRateBonus, &PlayerLevel), With<Player>>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    if *curr_ui_state.get() == UIState::ActiveSkills {
        return;
    }
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);

    if (*DEBUG || dev_mode) && key_input.just_pressed(KeyCode::KeyN) {
        if queue.queue.is_empty() {
            return;
        }
        let remaining_choices = queue.queue.remove(0).to_vec();
        for choice in remaining_choices.iter() {
            queue.pool.push(choice.clone());
        }
        for e in old_skill_entities.iter() {
            commands.entity(e).despawn();
        }

        let mut rng = rand::thread_rng();
        let (loot_bonus, player_level) = player_atts
            .single()
            .map(|a| (a.0 .0, a.1.level))
            .unwrap_or((0, 1));
        queue.add_new_skills_after_levelup(&mut rng, loot_bonus, player_level);
        spawn_skill_choice_entities(
            &graphics,
            &mut commands,
            &asset_server,
            &res,
            queue.queue[0].clone().to_vec(),
            Vec2::new(4., 4.),
        );
    }
}
pub fn handle_skill_choice_reroll_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    mut sprites: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<&mut Sprite, With<SkillChoiceRerollIcon>>,
    )>,
    mut reroll_buttons: Query<(
        Entity,
        &mut Interactable,
        &SkillChoiceRerollButton,
        &Children,
    )>,
    skill_cards: Query<&SkillChoiceUI>,
    pending_flash: Query<Entity, With<SkillChoiceRerollFlash>>,
    mut reroll_text: Query<(&mut Text2d, &mut TextColor), With<RerollCountText>>,
    mut commands: Commands,
    mut run_unlocks: ResMut<RunUnlockState>,
    asset_server: Res<AssetServer>,
    focus_input: crate::ui::focus::FocusInput,
) {
    let hit_entity = {
        let ui_sprites = sprites.p0();
        ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None).map(|(e, _, _)| e)
    };
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let mut any_hovered = false;
    let reroll_in_progress = !pending_flash.is_empty();

    for (e, mut interactable, _btn, btn_children) in reroll_buttons.iter_mut() {
        let enabled = run_unlocks.rerolls_remaining > 0 && !reroll_in_progress;
        let icon_color = if enabled {
            Color::WHITE
        } else {
            Color::srgb(0.45, 0.45, 0.45)
        };
        let is_hit = hit_entity == Some(e);
        let is_focused = focus_input.is_focused(e);
        let confirm_pressed =
            (is_hit && left_mouse_pressed) || (is_focused && focus_input.confirm_just_pressed());

        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None if enabled => {
                    interactable.change(Interaction::Hovering);
                    for child in btn_children.iter() {
                        if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                            sprite.color = YELLOW_2;
                        }
                    }
                }
                Interaction::Hovering => {
                    any_hovered = true;
                    if confirm_pressed && enabled {
                        run_unlocks.rerolls_remaining =
                            run_unlocks.rerolls_remaining.saturating_sub(1);

                        interactable.change(Interaction::None);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillReRoll, 0.4));

                        for card in skill_cards.iter() {
                            if card.skill_choice.heirloom == Heirloom::default() {
                                continue;
                            }
                            spawn_skill_choice_flash(
                                &mut commands,
                                &asset_server,
                                Vec3::new(
                                    (card.index as f32 - 1.) * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.,
                                    4.,
                                    15.,
                                ),
                            );
                        }

                        for child in btn_children.iter() {
                            if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                                sprite.color = icon_color;
                            }
                        }
                    }
                }
                _ => (),
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            for child in btn_children.iter() {
                if let Ok(mut sprite) = sprites.p1().get_mut(child) {
                    sprite.color = icon_color;
                }
            }
        }
    }

    for (mut text, mut text_color) in reroll_text.iter_mut() {
        text.0 = format!("Rerolls: {}", run_unlocks.rerolls_remaining);
        text_color.0 = if any_hovered { YELLOW_2 } else { WHITE };
    }
}

pub fn handle_skill_reroll_after_flash(
    flashes: Query<(Entity, &AnimationState), (With<DoneAnimation>, With<SkillChoiceRerollFlash>)>,
    mut skill_queue: ResMut<HeirloomChoiceQueue>,
    old_skill_entities: Query<Entity, With<SkillChoiceUI>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    player_atts: Query<(&LootRateBonus, &PlayerLevel), With<Player>>,
) {
    let mut should_reroll = false;
    for (e, state) in flashes.iter() {
        if usize::from(state.current_frame()) == 3 {
            should_reroll = true;
            // Clear markers on every flash so a 3-card reroll only applies once.
            commands.entity(e).remove::<SkillChoiceRerollFlash>();
        }
    }
    if !should_reroll {
        return;
    }
    for (e, _) in flashes.iter() {
        commands.entity(e).remove::<SkillChoiceRerollFlash>();
    }

    let (loot_bonus, player_level) = player_atts
        .single()
        .map(|a| (a.0 .0, a.1.level))
        .unwrap_or((0, 1));
    skill_queue.handle_reroll_all(&mut rand::thread_rng(), loot_bonus, player_level);
    for e in old_skill_entities.iter() {
        commands.entity(e).despawn();
    }
    spawn_skill_choice_entities(
        &graphics,
        &mut commands,
        &asset_server,
        &res,
        skill_queue.queue[0].clone().to_vec(),
        Vec2::new(4., 4.),
    );
}

pub fn spawn_skill_choice_flash(commands: &mut Commands, asset_server: &AssetServer, pos: Vec3) {
    commands
        .spawn(aseprite_bundle(
            asset_server.load(SkillChoiceFlash::PATH),
            SkillChoiceFlash::tags::FLASH,
            Transform {
                translation: pos,
                ..Default::default()
            },
            Visibility::Inherited,
            true,
        ))
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Visibility::default())
        .insert(SkillChoiceRerollFlash)
        .insert(DoneAnimation);
}

pub fn update_skill_choice_button_states(
    run_unlocks: Res<RunUnlockState>,
    skill_queue: Res<HeirloomChoiceQueue>,
    time_crystals: Res<TimeCrystals>,
    mut reroll_buttons: Query<
        (Entity, &mut Sprite, &Children),
        (With<SkillChoiceRerollButton>, With<Interactable>),
    >,
    mut reroll_icons: Query<
        &mut Sprite,
        (
            With<SkillChoiceRerollIcon>,
            Without<SkillChoiceRerollButton>,
            Without<BanishButton>,
        ),
    >,
    mut banish_buttons: Query<
        (&mut Sprite, &BanishButton),
        (
            Without<SkillChoiceRerollButton>,
            Without<SkillChoiceRerollIcon>,
        ),
    >,
    mut banish_labels: Query<(&mut TextColor, &BanishButtonLabel)>,
    mut commands: Commands,
) {
    if !run_unlocks.is_changed() && !skill_queue.is_changed() {
        return;
    }

    let reroll_enabled = run_unlocks.rerolls_remaining > 0;
    let badge_color = if reroll_enabled {
        KEYBIND_BADGE_COLOR
    } else {
        Color::srgba(62. / 255., 58. / 255., 58. / 255., 0.45)
    };
    let icon_color = if reroll_enabled {
        Color::WHITE
    } else {
        Color::srgb(0.45, 0.45, 0.45)
    };
    for (e, mut sprite, children) in reroll_buttons.iter_mut() {
        sprite.color = badge_color;
        if !reroll_enabled {
            commands.entity(e).remove::<Interactable>();
        }
        for child in children.iter() {
            if let Ok(mut icon) = reroll_icons.get_mut(child) {
                icon.color = icon_color;
            }
        }
    }

    for (mut sprite, banish) in banish_buttons.iter_mut() {
        let slot_ok = skill_queue.banish_allowed_for_choice_slot(&time_crystals, banish.0);
        let enabled = run_unlocks.banishes_remaining > 0 && slot_ok;
        sprite.color = if enabled {
            Color::WHITE
        } else {
            Color::srgb(0.5, 0.5, 0.5)
        };
    }
    for (mut text_color, label) in banish_labels.iter_mut() {
        let enabled = run_unlocks.banishes_remaining > 0
            && skill_queue.banish_allowed_for_choice_slot(&time_crystals, label.0);
        text_color.0 = if enabled {
            WHITE
        } else {
            Color::srgb(0.7, 0.7, 0.7)
        };
    }
}

pub fn update_skill_choice_count_text(
    run_unlocks: Res<RunUnlockState>,
    mut reroll_texts: Query<&mut Text2d, (With<RerollCountText>, Without<BanishCountText>)>,
    mut banish_texts: Query<&mut Text2d, (With<BanishCountText>, Without<RerollCountText>)>,
) {
    if !run_unlocks.is_changed() {
        return;
    }

    for mut text in reroll_texts.iter_mut() {
        text.0 = format!("Rerolls: {}", run_unlocks.rerolls_remaining);
    }

    for mut text in banish_texts.iter_mut() {
        text.0 = format!("Banishes: {}", run_unlocks.banishes_remaining);
    }
}
