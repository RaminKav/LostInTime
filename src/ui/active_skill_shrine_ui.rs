use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::skill_power_multiplier, AttackSpeed, BonusAttackSpeed, CritChance,
        MaxHealth, MaxMana, ProjectileSize, SkillPower, Speed,
    },
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    colors::{WHITE, YELLOW_2},
    item::active_skill_shrine::{
        assign_shrine_skill_to_slot, reroll_active_skill_shrine_offer_skills,
        shrine_assign_action, shrine_assignable_slots, skill_choices_from_offer_skills, ActiveSkillShrineOverwrite, ActiveSkillShrineSelection,
        ShrineAssignAction,
    },
    player::{
        skills::{
            active_skill_scaling::METEOR_SHOWER_BASE_COUNT,
            effective_player_attack_speed_multiplier, ActiveSkillChoiceState, PlayerClass,
            PlayerSkills,
        },
        unlocks::{RunUnlockState, UnlockedSkills},
        Player,
    },
    ui::{
        essence_ui::{MERCHANT_REROLL_ICON_PATH, MERCHANT_REROLL_ICON_SIZE},
        ui_helpers::spawn_full_screen_ui_overlay_tuned,
    },
    GameParam, ScreenResolution,
};

use super::{
    interactions::Interaction, main_menu::spawn_back_button, options_ui::CheatSettings,
    player_hud::{spawn_skill_tooltip_content, SKILL_TOOLTIP_ICON_SIZE}, Interactable, UIElement, UIState,
    KEYBIND_BADGE_COLOR, TOOLTIP_INFO_BOX_SIZE,
};

const SKILL_VIEW2_SIZE: Vec2 = Vec2::new(248.5, 171.5);
const SHRINE_BANNER_SPACING: f32 = 60.;
const SHRINE_REROLL_BADGE_SIZE: Vec2 = Vec2::new(14., 12.);
/// Bottom of `SkillView2`, inset from the panel edge (same band as merchant category rerolls).
const SHRINE_REROLL_BUTTON_Y: f32 = -SKILL_VIEW2_SIZE.y * 0.5 + 14.;

#[derive(Component)]
pub struct ActiveSkillShrineRoot;

#[derive(Component)]
pub struct ActiveSkillShrineChoicesRoot;

#[derive(Component)]
pub struct ActiveSkillShrineRerollButton;

#[derive(Component)]
pub struct ActiveSkillShrineRerollIcon;

#[derive(Component)]
pub struct ActiveSkillShrineRerollsText;

#[derive(Component)]
pub struct ActiveSkillShrineUI {
    pub skill_choice: ActiveSkillChoiceState,
    pub interaction_lock_timer: Timer,
}

#[derive(Component)]
pub struct ActiveSkillSlotChoiceUI {
    pub index: usize,
    /// `None` marks an empty unlocked slot the player can fill.
    pub skill_choice: Option<ActiveSkillChoiceState>,
    pub interaction_lock_timer: Timer,
}

fn spawn_active_skill_shrine_skill_choices(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    skill_choices: &[ActiveSkillChoiceState],
    skill_power: (
        &SkillPower,
        &OwnedBlessings,
        &MaxMana,
        &MaxHealth,
        Option<&BonusAttackSpeed>,
        Option<&AttackSpeed>,
        &CritChance,
        &Speed,
        &ProjectileSize,
    ),
) {
    let start_y = -SHRINE_BANNER_SPACING * 0.5;
    let (
        skill_power,
        blessings,
        max_mana,
        max_health,
        bonus_as,
        attack_speed,
        crit,
        spd,
        size,
    ) = skill_power;
    let bonus_as_mult = effective_player_attack_speed_multiplier(
        attack_speed.map(|a| a.0).unwrap_or(0),
        bonus_as.map(|b| b.get_multiplier()).unwrap_or(1.0),
    );

    for (index, skill_choice) in skill_choices.iter().enumerate() {
        let banner_x = -70.;
        let banner_y = start_y + (index as f32 * SHRINE_BANNER_SPACING);
        let tooltip_pos = Vec3::new(banner_x, banner_y, 15.);

        let container = commands
            .spawn(RenderLayers::from_layers(&[3]))
            .insert(SpatialBundle::from_transform(Transform {
                translation: tooltip_pos,
                scale: Vec3::new(1., 1., 11.),
                ..Default::default()
            }))
            .set_parent(parent)
            .id();

        commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::SkillTooltipBanner),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(236., 57.5)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(70., -1., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(ActiveSkillShrineUI {
                skill_choice: skill_choice.clone(),
                interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
            })
            .insert(Interactable::default())
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::ActiveSkillShrine)
            .insert(Name::new(format!("SKILL_BANNER_{index}")))
            .set_parent(container);

        spawn_skill_tooltip_content(
            commands,
            graphics,
            asset_server,
            skill_choice.active_skill.clone(),
            None,
            container,
            skill_power_multiplier(skill_power, blessings.get_skill_power_bonus()),
            max_mana.0,
            max_health.0,
            bonus_as_mult,
            crit.0,
            spd.0,
            size.0,
            METEOR_SHOWER_BASE_COUNT,
            SKILL_TOOLTIP_ICON_SIZE,
        );
    }
}

fn spawn_active_skill_shrine_reroll_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    parent: Entity,
    enabled: bool,
) {
    let color = if enabled {
        Color::WHITE
    } else {
        Color::rgb(0.45, 0.45, 0.45)
    };

    let mut btn = commands.spawn(SpriteBundle {
        sprite: Sprite {
            color: KEYBIND_BADGE_COLOR,
            custom_size: Some(SHRINE_REROLL_BADGE_SIZE),
            ..default()
        },
        transform: Transform::from_translation(Vec3::new(0., SHRINE_REROLL_BUTTON_Y, 3.)),
        ..default()
    });
    btn.insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkillShrine)
        .insert(ActiveSkillShrineRerollButton)
        .insert(Name::new("Active Skill Shrine Reroll"));

    if enabled {
        btn.insert(Interactable::default());
    }

    let btn_e = btn.id();

    commands
        .spawn(SpriteBundle {
            texture: asset_server.load(MERCHANT_REROLL_ICON_PATH),
            sprite: Sprite {
                custom_size: Some(MERCHANT_REROLL_ICON_SIZE),
                color,
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ActiveSkillShrineRerollIcon)
        .set_parent(btn_e);

    commands.entity(btn_e).set_parent(parent);
}

fn spawn_active_skill_shrine_reroll_info_box(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    rerolls_remaining: u32,
) {
    let pos = Vec2::new(
        SKILL_VIEW2_SIZE.x / 2. + TOOLTIP_INFO_BOX_SIZE.x / 2. + 6.,
        50.,
    );
    let box_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::TooltipInfoBox),
            sprite: Sprite {
                custom_size: Some(TOOLTIP_INFO_BOX_SIZE),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(pos.x, pos.y, 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkillShrine)
        .insert(Name::new("Active Skill Shrine Reroll Info Box"))
        .set_parent(parent)
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Rerolls left:".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 5., 2.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
        ))
        .set_parent(box_e);

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    rerolls_remaining.to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.4,
                        color: WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., -6., 2.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            ActiveSkillShrineRerollsText,
        ))
        .set_parent(box_e);
}

pub fn setup_active_skill_shrine_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_selection: Res<ActiveSkillShrineSelection>,
    run_unlocks: Res<RunUnlockState>,
    res: Res<ScreenResolution>,
    mut popup_events: EventWriter<crate::ui::tutorial_ui::TutorialPopupEvent>,
    seen_chunks: Option<Res<crate::ui::tutorial_ui::SeenTutorialChunks>>,
    tutorial_ui: Query<(), With<crate::ui::tutorial_ui::TutorialUI>>,
    skill_power: Query<
        (
            &SkillPower,
            &OwnedBlessings,
            &MaxMana,
            &MaxHealth,
            Option<&BonusAttackSpeed>,
            Option<&AttackSpeed>,
            &CritChance,
            &Speed,
            &ProjectileSize,
        ),
        With<Player>,
    >,
) {
    let shrine_overlay = spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 20.);
    commands
        .entity(shrine_overlay)
        .insert(UIState::ActiveSkillShrine);
    let _title_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Choose a New Skill".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 40., 21.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();
    let _title_text2 = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "(Assign to an active skill slot)".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 62., 21.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    let view2_bg = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SkillView2),
            sprite: Sprite {
                custom_size: Some(SKILL_VIEW2_SIZE),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 21.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkillShrine)
        .insert(ActiveSkillShrineRoot)
        .insert(Name::new("SKILL_VIEW2_BACKGROUND"))
        .id();

    let choices_root = commands
        .spawn((
            SpatialBundle::from_transform(Transform::IDENTITY),
            RenderLayers::from_layers(&[3]),
            UIState::ActiveSkillShrine,
            ActiveSkillShrineChoicesRoot,
            Name::new("Active Skill Shrine Choices"),
        ))
        .set_parent(view2_bg)
        .id();

    let Ok(skill_power) = skill_power.get_single() else {
        return;
    };
    spawn_active_skill_shrine_skill_choices(
        &mut commands,
        &graphics,
        &asset_server,
        choices_root,
        &shrine_selection.skill_choices,
        skill_power,
    );

    let reroll_enabled = run_unlocks.rerolls_remaining > 0;
    spawn_active_skill_shrine_reroll_button(
        &mut commands,
        &asset_server,
        view2_bg,
        reroll_enabled,
    );
    spawn_active_skill_shrine_reroll_info_box(
        &mut commands,
        &graphics,
        &asset_server,
        view2_bg,
        run_unlocks.rerolls_remaining,
    );

    let back_button = spawn_back_button(
        Vec3::new(res.game_width / 2. - 55., -res.game_height / 2. + 38., 11.),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands
        .entity(back_button)
        .insert(UIState::ActiveSkillShrine);

    if let Some(seen_chunks) = seen_chunks.as_ref() {
        crate::ui::tutorial_ui::try_active_skill_shrine_tutorial(
            &mut popup_events,
            seen_chunks,
            &tutorial_ui,
        );
    }
}

pub fn tick_active_skill_shrine_ui_interaction_lock_timers(
    time: Res<Time>,
    mut query: Query<&mut ActiveSkillShrineUI>,
) {
    for mut skill_ui in query.iter_mut() {
        if skill_ui.interaction_lock_timer.finished() {
            continue;
        }
        skill_ui.interaction_lock_timer.tick(time.delta());
    }
}

pub fn tick_active_skill_slot_choice_ui_interaction_lock_timers(
    time: Res<Time>,
    mut query: Query<&mut ActiveSkillSlotChoiceUI>,
) {
    for mut slot_ui in query.iter_mut() {
        if slot_ui.interaction_lock_timer.finished() {
            continue;
        }
        slot_ui.interaction_lock_timer.tick(time.delta());
    }
}

pub fn handle_active_skill_shrine_ui_interaction(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_choices: Query<(Entity, &mut Interactable, &ActiveSkillShrineUI)>,
    shrine_selection: ResMut<ActiveSkillShrineSelection>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut player_skills: Query<(Entity, &mut PlayerSkills, &PlayerClass), With<Player>>,
    unlocked_skills: Res<UnlockedSkills>,
    cheat_settings: Res<CheatSettings>,
    mut att_event: EventWriter<crate::attributes::AttributeChangeEvent>,
    mut shrine_query: Query<&mut crate::item::active_skill_shrine::ActiveSkillShrineState>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, skill_ui) in skill_choices.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::UISkillHover,
                        0.2,
                    ));
                    // Hover effect could be handled by changing banner color/opacity if needed
                    let ui_element = skill_ui
                        .skill_choice
                        .active_skill
                        .get_ui_element_hover(skill_ui.skill_choice.rarity.clone());
                    // swap to hover img
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && skill_ui.interaction_lock_timer.finished() {
                        let picked_skill = skill_ui.skill_choice.clone();
                        let Ok((player_e, mut skills, player_class)) =
                            player_skills.get_single_mut()
                        else {
                            return;
                        };

                        match shrine_assign_action(
                            &skills,
                            &player_class.class,
                            &unlocked_skills,
                            cheat_settings.bypass_class_unlocks,
                        ) {
                            ShrineAssignAction::AutoFill(slot) => {
                                assign_shrine_skill_to_slot(
                                    &mut skills,
                                    slot,
                                    picked_skill.clone(),
                                );
                                picked_skill
                                    .active_skill
                                    .add_skill_components(player_e, &mut commands);
                                if let Ok(mut shrine_state) =
                                    shrine_query.get_mut(shrine_selection.shrine_entity)
                                {
                                    shrine_state.is_used = true;
                                }
                                commands.remove_resource::<ActiveSkillShrineSelection>();
                                next_ui_state.set(UIState::Closed);
                                att_event.send(crate::attributes::AttributeChangeEvent);
                            }
                            ShrineAssignAction::ShowSlotPicker => {
                                commands.insert_resource(ActiveSkillShrineOverwrite {
                                    skill_choice: picked_skill.clone(),
                                    shrine_entity: shrine_selection.shrine_entity,
                                });
                                commands.remove_resource::<ActiveSkillShrineSelection>();
                                next_ui_state.set(UIState::ActiveSkills);
                            }
                        }
                        return;
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                interactable.change(Interaction::None);
                let ui_element = skill_ui
                    .skill_choice
                    .active_skill
                    .get_ui_element(skill_ui.skill_choice.rarity.clone());
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}

pub fn handle_active_skill_shrine_reroll_button(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    mut sprites: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<&mut Sprite, With<ActiveSkillShrineRerollIcon>>,
    )>,
    mut reroll_buttons: Query<(
        Entity,
        &mut Interactable,
        &ActiveSkillShrineRerollButton,
        &Children,
    )>,
    mut reroll_text: Query<&mut Text, With<ActiveSkillShrineRerollsText>>,
    mut commands: Commands,
    mut run_unlocks: ResMut<RunUnlockState>,
    mut shrine_selection: ResMut<ActiveSkillShrineSelection>,
    mut game: GameParam,
    player_skills: Query<&PlayerSkills, With<Player>>,
    skill_power: Query<
        (
            &SkillPower,
            &OwnedBlessings,
            &MaxMana,
            &MaxHealth,
            Option<&BonusAttackSpeed>,
            Option<&AttackSpeed>,
            &CritChance,
            &Speed,
            &ProjectileSize,
        ),
        With<Player>,
    >,
    choices_root: Query<Entity, With<ActiveSkillShrineChoicesRoot>>,
    shrine_state: Query<&crate::item::active_skill_shrine::ActiveSkillShrineState>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    let hit_entity = {
        let ui_sprites = sprites.p0();
        super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None).map(|(e, _, _)| e)
    };
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let mut any_hovered = false;

    for (e, mut interactable, _btn, btn_children) in reroll_buttons.iter_mut() {
        let enabled = run_unlocks.rerolls_remaining > 0;
        let icon_color = if enabled {
            Color::WHITE
        } else {
            Color::rgb(0.45, 0.45, 0.45)
        };

        match hit_entity {
            Some(hit_ent) if hit_ent == e => match interactable.current() {
                Interaction::None if enabled => {
                    interactable.change(Interaction::Hovering);
                    for child in btn_children.iter() {
                        if let Ok(mut sprite) = sprites.p1().get_mut(*child) {
                            sprite.color = YELLOW_2;
                        }
                    }
                }
                Interaction::Hovering => {
                    any_hovered = true;
                    if left_mouse_pressed && enabled {
                        run_unlocks.rerolls_remaining =
                            run_unlocks.rerolls_remaining.saturating_sub(1);

                        let previous_offer: Vec<_> = shrine_selection
                            .skill_choices
                            .iter()
                            .map(|c| c.active_skill)
                            .collect();
                        let offer_skills = reroll_active_skill_shrine_offer_skills(
                            &previous_offer,
                            player_skills.get_single().ok(),
                        );
                        if offer_skills.is_empty() {
                            return;
                        }

                        shrine_selection.skill_choices =
                            skill_choices_from_offer_skills(&offer_skills);

                        if let Ok(shrine) = shrine_state.get(shrine_selection.shrine_entity) {
                            game.world_obj_cache
                                .active_skill_shrine_offers
                                .insert(shrine.tile_pos, offer_skills);
                        }

                        if let Ok(choices_e) = choices_root.get_single() {
                            commands.entity(choices_e).despawn_descendants();
                            if let Ok(stats) = skill_power.get_single() {
                                spawn_active_skill_shrine_skill_choices(
                                    &mut commands,
                                    &graphics,
                                    &asset_server,
                                    choices_e,
                                    &shrine_selection.skill_choices,
                                    stats,
                                );
                            }
                        }

                        interactable.change(Interaction::None);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillReRoll, 0.4));
                        for child in btn_children.iter() {
                            if let Ok(mut sprite) = sprites.p1().get_mut(*child) {
                                sprite.color = icon_color;
                            }
                        }
                    }
                }
                _ => (),
            },
            _ => {
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                    for child in btn_children.iter() {
                        if let Ok(mut sprite) = sprites.p1().get_mut(*child) {
                            sprite.color = icon_color;
                        }
                    }
                }
            }
        }
    }

    for mut text in reroll_text.iter_mut() {
        if let Some(section) = text.sections.first_mut() {
            section.value = run_unlocks.rerolls_remaining.to_string();
            section.style.color = if any_hovered { YELLOW_2 } else { WHITE };
        }
    }
}

pub fn update_active_skill_shrine_reroll_button_state(
    run_unlocks: Res<RunUnlockState>,
    mut reroll_buttons: Query<
        (Entity, &mut Sprite, &Children),
        (With<ActiveSkillShrineRerollButton>, With<Interactable>),
    >,
    mut reroll_icons: Query<&mut Sprite, (With<ActiveSkillShrineRerollIcon>, Without<ActiveSkillShrineRerollButton>)>,
    mut commands: Commands,
) {
    if !run_unlocks.is_changed() {
        return;
    }

    let enabled = run_unlocks.rerolls_remaining > 0;
    let badge_color = if enabled {
        KEYBIND_BADGE_COLOR
    } else {
        Color::rgba(62. / 255., 58. / 255., 58. / 255., 0.45)
    };
    let icon_color = if enabled {
        Color::WHITE
    } else {
        Color::rgb(0.45, 0.45, 0.45)
    };

    for (e, mut sprite, children) in reroll_buttons.iter_mut() {
        sprite.color = badge_color;
        if !enabled {
            commands.entity(e).remove::<Interactable>();
        }
        for child in children.iter() {
            if let Ok(mut icon) = reroll_icons.get_mut(*child) {
                icon.color = icon_color;
            }
        }
    }
}

pub fn update_active_skill_shrine_reroll_count_text(
    run_unlocks: Res<RunUnlockState>,
    mut reroll_text: Query<&mut Text, With<ActiveSkillShrineRerollsText>>,
) {
    if !run_unlocks.is_changed() {
        return;
    }

    for mut text in reroll_text.iter_mut() {
        if let Some(section) = text.sections.first_mut() {
            section.value = run_unlocks.rerolls_remaining.to_string();
        }
    }
}

fn spawn_empty_shrine_slot_row(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    parent: Entity,
    slot_index: usize,
) {
    commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SkillTooltipBanner),
            sprite: Sprite {
                custom_size: Some(Vec2::new(236.5, 57.5)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(70., -1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ActiveSkillSlotChoiceUI {
            index: slot_index,
            skill_choice: None,
            interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
        })
        .insert(Interactable::default())
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkills)
        .insert(Name::new(format!("EMPTY_SKILL_SLOT_BANNER_{}", slot_index)))
        .set_parent(parent);

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Empty Slot",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            ),
            text_anchor: Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(12., 0., 2.)),
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(parent);
}

pub fn setup_active_skill_shrine_overwrite_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_overwrite: Res<ActiveSkillShrineOverwrite>,
    res: Res<ScreenResolution>,
    skills: Query<
        (
            &PlayerSkills,
            &PlayerClass,
            &SkillPower,
            &OwnedBlessings,
            &MaxMana,
            &MaxHealth,
            Option<&BonusAttackSpeed>,
            Option<&AttackSpeed>,
            &CritChance,
            &Speed,
            &ProjectileSize,
        ),
        With<Player>,
    >,
    unlocked_skills: Res<UnlockedSkills>,
    cheat_settings: Res<CheatSettings>,
) {
    let overwrite_overlay = spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);
    commands
        .entity(overwrite_overlay)
        .insert(UIState::ActiveSkills);
    let (
        skills,
        player_class,
        skill_power,
        blessings,
        max_mana,
        max_health,
        bonus_as,
        attack_speed,
        crit,
        spd,
        size,
    ) = skills.single();
    let assignable_slots = shrine_assignable_slots(
        &player_class.class,
        &unlocked_skills,
        cheat_settings.bypass_class_unlocks,
    );
    let has_empty_target = assignable_slots
        .iter()
        .any(|&slot| skills.get_active_skill_in_slot(slot).is_none());
    let title = if has_empty_target {
        "Choose a Slot"
    } else {
        "Choose a Skill to Lose"
    };

    let _title_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    title.to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 30., 21.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    let view4_bg = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SkillView4),
            sprite: Sprite {
                custom_size: Some(Vec2::new(248., 249.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 21.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkills)
        .insert(Name::new("SKILL_VIEW4_BACKGROUND"))
        .id();

    let bonus_as_mult = effective_player_attack_speed_multiplier(
        attack_speed.map(|a| a.0).unwrap_or(0),
        bonus_as.map(|b| b.get_multiplier()).unwrap_or(1.0),
    );

    let banner_spacing = 60.;
    let start_y = -banner_spacing * 0.5;

    for (row, &slot_index) in assignable_slots.iter().enumerate() {
        let choice_option = match slot_index {
            1 => skills.active_skill_slot_1.clone(),
            2 => skills.active_skill_slot_2.clone(),
            _ => None,
        };
        let banner_x = -70.;
        let banner_y = start_y + (row as f32 * banner_spacing);
        let tooltip_pos = Vec3::new(banner_x, banner_y, 15.);

        let container = commands
            .spawn(RenderLayers::from_layers(&[3]))
            .insert(SpatialBundle::from_transform(Transform {
                translation: tooltip_pos,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            }))
            .set_parent(view4_bg)
            .id();

        if let Some(choice) = choice_option {
            commands
                .spawn(SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::SkillTooltipBanner),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(236.5, 57.5)),
                        ..Default::default()
                    },
                    transform: Transform {
                        translation: Vec3::new(70., -1., 1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(ActiveSkillSlotChoiceUI {
                    index: slot_index,
                    skill_choice: Some(choice.clone()),
                    interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
                })
                .insert(Interactable::default())
                .insert(RenderLayers::from_layers(&[3]))
                .insert(UIState::ActiveSkills)
                .insert(Name::new(format!("SKILL_SLOT_BANNER_{}", slot_index)))
                .set_parent(container);

            spawn_skill_tooltip_content(
                &mut commands,
                &graphics,
                &asset_server,
                choice.active_skill.clone(),
                None,
                container,
                skill_power_multiplier(skill_power, blessings.get_skill_power_bonus()),
                max_mana.0,
                max_health.0,
                bonus_as_mult,
                crit.0,
                spd.0,
                size.0,
                METEOR_SHOWER_BASE_COUNT,
                SKILL_TOOLTIP_ICON_SIZE,
            );
        } else {
            spawn_empty_shrine_slot_row(
                &mut commands,
                &graphics,
                &asset_server,
                container,
                slot_index,
            );
        }
    }

    commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_active_skill_icon(shrine_overwrite.skill_choice.active_skill.clone()),
            sprite: Sprite {
                custom_size: Some(Vec2::new(32., 32.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec2::new(0., -100.).extend(20.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::ActiveSkills)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("NEW_ACTIVE_SKILL_ICON"));
}

pub fn handle_active_skill_shrine_overwrite_interaction(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_choices: Query<(Entity, &mut Interactable, &ActiveSkillSlotChoiceUI)>,
    mut player_skills: Query<(
        Entity,
        &mut crate::player::skills::PlayerSkills,
        &GlobalTransform,
    )>,
    graphics: Res<Graphics>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut att_event: EventWriter<crate::attributes::AttributeChangeEvent>,
    mut shrine_query: Query<&mut crate::item::active_skill_shrine::ActiveSkillShrineState>,
    shrine_overwrite_res: Option<Res<ActiveSkillShrineOverwrite>>,
) {
    // Only handle if this is a shrine overwrite, not heirloom limbo
    let overwrite = if let Some(overwrite_res) = shrine_overwrite_res.as_ref() {
        overwrite_res
    } else {
        return; // This is an heirloom limbo overwrite, handled elsewhere
    };

    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, skill_ui) in skill_choices.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::UISkillHover,
                        0.2,
                    ));

                    if let Some(choice) = skill_ui.skill_choice.as_ref() {
                        let ui_element = choice
                            .active_skill
                            .get_ui_element_hover(choice.rarity.clone());
                        commands
                            .entity(e)
                            .insert(ui_element.clone())
                            .insert(graphics.get_ui_element_texture(ui_element));
                    }
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && skill_ui.interaction_lock_timer.finished() {
                        let (player_e, mut skills, _t) = player_skills.single_mut();
                        let new_skill = overwrite.skill_choice.clone();

                        assign_shrine_skill_to_slot(&mut skills, skill_ui.index, new_skill.clone());
                        new_skill
                            .active_skill
                            .add_skill_components(player_e, &mut commands);

                        // Mark shrine as used
                        if let Ok(mut shrine_state) = shrine_query.get_mut(overwrite.shrine_entity)
                        {
                            shrine_state.is_used = true;
                        }

                        // Remove resources
                        commands.remove_resource::<ActiveSkillShrineOverwrite>();
                        commands.remove_resource::<ActiveSkillShrineSelection>();
                        next_ui_state.set(UIState::Closed);
                        att_event.send(crate::attributes::AttributeChangeEvent);
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                interactable.change(Interaction::None);
                if let Some(choice) = skill_ui.skill_choice.as_ref() {
                    let ui_element = choice.active_skill.get_ui_element(choice.rarity.clone());
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
            }
        }
    }
}
