use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    animations::DoneAnimation,
    assets::Graphics,
    attributes::LootRateBonus,
    colors::{
        COMMON_TOOLTIP_TITLE, LEGENDARY_TOOLTIP_TITLE, RARE_TOOLTIP_TITLE, UNCOMMON_TOOLTIP_TITLE,
        WHITE,
    },
    player::{
        levels::PlayerLevel,
        skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState, HeirloomRarity},
        time_crystals::TimeCrystals,
        unlocks::RunUnlockState,
        Player,
    },
    ui::{game_fonts as gf, ui_helpers::spawn_full_screen_ui_overlay_tuned, CheatSettings},
    ScreenResolution, DEBUG,
};

use super::{
    heirloom_tooltip::{spawn_heirloom_tooltip_card, HeirloomTooltipRequest, HeirloomTooltipShow},
    interactions::Interaction,
    tooltip_info_boxes::{
        build_tooltip_info_boxes, spawn_tooltip_info_boxes_with_resolution, TooltipInfoBoxAnchor,
    },
    ui_helpers::{
        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT, Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
        Z_DEPTH_HEIRLOOM_SKILL_CHOICE_OVERLAY,
    },
    Interactable, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
};

/// Side info boxes spawned on hover for a level-up choice card (despawned on unhover).
#[derive(Component)]
pub struct SkillChoiceInfoBoxRoot;

#[derive(Component)]
pub struct SkillChoiceUI {
    pub index: usize,
    pub skill_choice: HeirloomChoiceState,
    pub interaction_lock_timer: Timer,
}

#[derive(Component)]
pub struct RerollDice(pub usize);

#[derive(Component)]
pub struct BanishButton(pub usize);

#[derive(Component)]
pub struct BanishButtonLabel(pub usize);

/// Root container for the per-rarity banish tracker on the heirloom-choice screen.
#[derive(Component)]
pub struct BanishTrackerRoot;

/// Marker for an individual heirloom icon inside the banish tracker. Carries the heirloom +
/// rarity so the hover tooltip system can mirror the player HUD's heirloom hover behavior.
#[derive(Component, Clone)]
pub struct BanishTrackerIcon {
    pub heirloom: Heirloom,
    pub rarity: HeirloomRarity,
}

#[derive(Component)]
pub struct RerollCountText;

#[derive(Component)]
pub struct BanishCountText;

aseprite!(pub SkillChoiceFlash, "ui/SkillChoiceFlash.aseprite");

const BANISH_TRACKER_ICON_SIZE: f32 = 14.;
const BANISH_TRACKER_ICON_SPACING: f32 = 16.;
/// Vertical spacing between rows of icons inside the same rarity (when wrapping).
const BANISH_TRACKER_ICON_ROW_SPACING: f32 = 16.;
/// Vertical gap between the bottom of one rarity row and the heading of the next.
const BANISH_TRACKER_RARITY_GAP: f32 = 18.;
const BANISH_TRACKER_HEADING_TO_ICONS: f32 = 12.;
const BANISH_TRACKER_ICONS_PER_ROW: usize = 4;

/// Build the banishes tracker (title + per-rarity heading rows + heirloom icon rows) under `root`.
/// Icon entities carry [`BanishTrackerIcon`] + [`Interactable`] so [`handle_banish_tracker_tooltip`]
/// spawns the same heirloom hover card used by the player HUD.
fn build_banish_tracker_children(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    queue: &HeirloomChoiceQueue,
    time_crystals: &TimeCrystals,
    root: Entity,
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
            UIState::Skills,
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
                UIState::Skills,
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
            // No icons placed; skip a row of vertical space so the next heading sits where
            // the icon row would have been.
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
                        UIState::Skills,
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
            // Advance y by the full vertical extent of all icon rows so following segments
            // don't overlap.
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
    )>,
    mut tooltip_requests: EventWriter<HeirloomTooltipRequest>,
    mut last_hovered: Local<Option<Heirloom>>,
) {
    use super::interactions::Interaction;

    let hit_entity = super::ui_helpers::pointcast_2d(&cursor_pos, &hit_detection_sprites, None);

    for (entity, _, mut interactable, _) in tracker_icons.iter_mut() {
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
        .find(|(_, _, interactable, _)| matches!(interactable.current(), Interaction::Hovering))
        .map(|(_, transform, _, icon)| (icon.clone(), transform.translation()));

    let hovered_heirloom = currently_hovered.as_ref().map(|(i, _)| i.heirloom.clone());
    if *last_hovered == hovered_heirloom {
        return;
    }

    match &currently_hovered {
        None => {
            tooltip_requests.send(HeirloomTooltipRequest::Clear);
        }
        Some((icon, icon_pos)) => {
            let tooltip_pos = Vec3::new(icon_pos.x + 90., icon_pos.y, icon_pos.z + 10.);
            tooltip_requests.send(HeirloomTooltipRequest::Show(HeirloomTooltipShow {
                heirloom: icon.heirloom.clone(),
                rarity: icon.rarity,
                position: tooltip_pos,
                scaling_text: None,
                trigger_count: 0,
                ui_state: Some(UIState::Skills),
            }));
        }
    }

    *last_hovered = hovered_heirloom;
}

/// Spawns definition info boxes beside a level-up choice card only while it is hovered.
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
        commands.entity(root).despawn_recursive();
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
        },
        &specs,
    ) {
        commands
            .entity(root)
            .insert(SkillChoiceInfoBoxRoot)
            .set_parent(card_e);
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
            Text2dBundle {
                text: Text::from_section(
                    "Choose an Heirloom".to_string(),
                    gf::MENU_TITLE.text_style(asset_server, WHITE),
                ),
                transform: Transform {
                    translation: Vec3::new(0., 115., Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
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

    for i in -1i32..2 {
        let slot_index = (i + 1) as usize;
        let enabled = run_unlocks.rerolls_remaining > 0;
        let translation = Vec3::new(
            i as f32 * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.5,
            -110.,
            Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT,
        );
        let mut reroll_entity = commands.spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::RerollDice)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(21., 22.)),
                color: if enabled {
                    Color::WHITE
                } else {
                    Color::rgb(0.55, 0.55, 0.55)
                },
                ..Default::default()
            },
            transform: Transform {
                translation,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        });
        reroll_entity
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIElement::RerollDice)
            .insert(UIState::Skills)
            .insert(RerollDice(slot_index))
            .insert(Name::new(format!("REROLL BUTTON {slot_index}")));
        if enabled {
            reroll_entity.insert(Interactable::default());
        }
    }

    for i in -1i32..2 {
        let slot_index = (i + 1) as usize;
        let slot_ok =
            choices_queue.banish_allowed_for_choice_slot(time_crystals.as_ref(), slot_index);
        let banish_enabled = run_unlocks.banishes_remaining > 0 && slot_ok;
        let translation = Vec3::new(
            i as f32 * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.,
            -136.,
            Z_DEPTH_HEIRLOOM_SKILL_CHOICE_CONTENT,
        );
        let mut banish_button = commands.spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::BackButton)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(48., 18.)),
                color: if banish_enabled {
                    Color::WHITE
                } else {
                    Color::rgb(0.5, 0.5, 0.5)
                },
                ..Default::default()
            },
            transform: Transform {
                translation,
                ..Default::default()
            },
            ..Default::default()
        });
        let banish_entity = banish_button.id();
        banish_button
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::Skills)
            .insert(UIElement::BackButton)
            .insert(BanishButton(slot_index))
            .insert(Interactable::default())
            .insert(Name::new(format!("BANISH BUTTON {slot_index}")));

        commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "Banish ",
                        gf::SKILL_CHOICE_MICRO.text_style(
                            asset_server,
                            if banish_enabled {
                                WHITE
                            } else {
                                Color::rgb(0.7, 0.7, 0.7)
                            },
                        ),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(2., 0., 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIState::Skills,
                BanishButtonLabel(slot_index),
                Name::new(format!("BANISH BUTTON TEXT {slot_index}")),
            ))
            .set_parent(banish_entity);
    }

    let tracker_root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::from_translation(Vec3::new(
                -res.game_width * 0.5 + 6.,
                58.,
                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
            ))),
            RenderLayers::from_layers(&[3]),
            UIState::Skills,
            BanishTrackerRoot,
            Name::new("Banish Tracker"),
        ))
        .id();
    build_banish_tracker_children(
        &mut commands,
        &asset_server,
        &graphics,
        choices_queue.as_ref(),
        time_crystals.as_ref(),
        tracker_root,
    );

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("Rerolls: {}", run_unlocks.rerolls_remaining),
                gf::SKILL_CHOICE_MICRO.text_style(asset_server, WHITE),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                -80.5,
                -140.,
                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UIState::Skills,
        RerollCountText,
        Name::new("Reroll Count Text"),
    ));

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("Banishes: {}", run_unlocks.banishes_remaining),
                gf::SKILL_CHOICE_MICRO.text_style(asset_server, WHITE),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                80.,
                -140.,
                Z_DEPTH_HEIRLOOM_SKILL_CHOICE_FOREGROUND,
            )),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UIState::Skills,
        BanishCountText,
        Name::new("Banish Count Text"),
    ));
}

pub fn tick_skill_choice_interaction_lock_timers(
    time: Res<Time>,
    mut query: Query<&mut SkillChoiceUI>,
) {
    for mut skill_ui in query.iter_mut() {
        if skill_ui.interaction_lock_timer.finished() {
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
            .insert(Name::new("SKILLS UI"));
    }
}

pub fn toggle_skills_visibility(
    curr_ui_state: Res<State<UIState>>,
    key_input: ResMut<Input<KeyCode>>,
    mut queue: ResMut<HeirloomChoiceQueue>,
    old_skill_entities: Query<Entity, With<SkillChoiceUI>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    player_atts: Query<(&LootRateBonus, &PlayerLevel), With<Player>>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    if curr_ui_state.0 == UIState::ActiveSkills {
        return;
    }
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);

    if (*DEBUG || dev_mode) && key_input.just_pressed(KeyCode::N) {
        if queue.queue.is_empty() {
            return;
        }
        let remaining_choices = queue.queue.remove(0).to_vec();
        for choice in remaining_choices.iter() {
            queue.pool.push(choice.clone());
        }
        for e in old_skill_entities.iter() {
            commands.entity(e).despawn_recursive();
        }

        let mut rng = rand::thread_rng();
        let (loot_bonus, player_level) = player_atts
            .get_single()
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
pub fn handle_skill_reroll_after_flash(
    flashes: Query<(Entity, &RerollDice, &AsepriteAnimation), With<DoneAnimation>>,
    mut skill_queue: ResMut<HeirloomChoiceQueue>,
    old_skill_entities: Query<Entity, With<SkillChoiceUI>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    player_atts: Query<(&LootRateBonus, &PlayerLevel), With<Player>>,
) {
    for (e, slot, anim) in flashes.iter() {
        if anim.current_frame() == 3 {
            let (loot_bonus, player_level) = player_atts
                .get_single()
                .map(|a| (a.0 .0, a.1.level))
                .unwrap_or((0, 1));
            commands.entity(e).remove::<RerollDice>();
            skill_queue.handle_reroll_slot(
                slot.0,
                &mut rand::thread_rng(),
                loot_bonus,
                player_level,
            );
            for e in old_skill_entities.iter() {
                commands.entity(e).despawn_recursive();
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
    }
}
pub fn spawn_skill_choice_flash(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    slot: usize,
) {
    commands
        .spawn(AsepriteBundle {
            animation: AsepriteAnimation::from(SkillChoiceFlash::tags::FLASH),
            aseprite: asset_server.load(SkillChoiceFlash::PATH),
            transform: Transform {
                translation: pos,
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(VisibilityBundle::default())
        .insert(RerollDice(slot))
        .insert(DoneAnimation);
}

pub fn update_skill_choice_button_states(
    run_unlocks: Res<RunUnlockState>,
    skill_queue: Res<HeirloomChoiceQueue>,
    time_crystals: Res<TimeCrystals>,
    mut reroll_buttons: Query<&mut Sprite, (With<RerollDice>, Without<BanishButton>)>,
    mut banish_buttons: Query<(&mut Sprite, &BanishButton), Without<RerollDice>>,
    mut banish_labels: Query<(&mut Text, &BanishButtonLabel)>,
) {
    if !run_unlocks.is_changed() && !skill_queue.is_changed() {
        return;
    }

    let reroll_color = if run_unlocks.rerolls_remaining > 0 {
        Color::WHITE
    } else {
        Color::rgb(0.55, 0.55, 0.55)
    };
    for mut sprite in reroll_buttons.iter_mut() {
        sprite.color = reroll_color;
    }

    for (mut sprite, banish) in banish_buttons.iter_mut() {
        let slot_ok = skill_queue.banish_allowed_for_choice_slot(&time_crystals, banish.0);
        let enabled = run_unlocks.banishes_remaining > 0 && slot_ok;
        sprite.color = if enabled {
            Color::WHITE
        } else {
            Color::rgb(0.5, 0.5, 0.5)
        };
    }
    for (mut text, label) in banish_labels.iter_mut() {
        let enabled = run_unlocks.banishes_remaining > 0
            && skill_queue.banish_allowed_for_choice_slot(&time_crystals, label.0);
        text.sections[0].style.color = if enabled {
            WHITE
        } else {
            Color::rgb(0.7, 0.7, 0.7)
        };
    }
}

pub fn update_banish_tracker_ui(
    mut commands: Commands,
    skill_queue: Res<HeirloomChoiceQueue>,
    time_crystals: Res<TimeCrystals>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    tracker_roots: Query<Entity, With<BanishTrackerRoot>>,
    children: Query<&Children>,
) {
    if !skill_queue.is_changed() {
        return;
    }
    for root in tracker_roots.iter() {
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
        );
    }
}

pub fn update_skill_choice_count_text(
    run_unlocks: Res<RunUnlockState>,
    mut reroll_texts: Query<&mut Text, (With<RerollCountText>, Without<BanishCountText>)>,
    mut banish_texts: Query<&mut Text, (With<BanishCountText>, Without<RerollCountText>)>,
) {
    if !run_unlocks.is_changed() {
        return;
    }

    for mut text in reroll_texts.iter_mut() {
        text.sections[0].value = format!("Rerolls: {}", run_unlocks.rerolls_remaining);
    }

    for mut text in banish_texts.iter_mut() {
        text.sections[0].value = format!("Banishes: {}", run_unlocks.banishes_remaining);
    }
}
