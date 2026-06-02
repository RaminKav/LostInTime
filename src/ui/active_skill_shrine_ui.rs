use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    assets::Graphics,
    attributes::{
        attribute_helpers::skill_power_multiplier, AttackSpeed, BonusAttackSpeed, CritChance,
        MaxHealth, MaxMana, ProjectileSize, SkillPower, Speed,
    },
    blessings::OwnedBlessings,
    colors::WHITE,
    item::active_skill_shrine::{ActiveSkillShrineOverwrite, ActiveSkillShrineSelection},
    player::{
        skills::{
            active_skill_scaling::METEOR_SHOWER_BASE_COUNT,
            effective_player_attack_speed_multiplier, ActiveSkillChoiceState, PlayerSkills,
        },
        Player,
    },
    ui::ui_helpers::spawn_full_screen_ui_overlay_tuned,
    ScreenResolution,
};

use super::{
    interactions::Interaction, main_menu::spawn_back_button,
    player_hud::spawn_skill_tooltip_content, ui_helpers::spawn_full_screen_ui_overlay,
    Interactable, UIElement, UIState,
};

#[derive(Component)]
pub struct ActiveSkillShrineUI {
    pub skill_choice: ActiveSkillChoiceState,
    pub interaction_lock_timer: Timer,
}

#[derive(Component)]
pub struct ActiveSkillSlotChoiceUI {
    pub index: usize,
    pub skill_choice: ActiveSkillChoiceState,
    pub interaction_lock_timer: Timer,
}

pub fn setup_active_skill_shrine_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_selection: Res<ActiveSkillShrineSelection>,
    res: Res<ScreenResolution>,
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
    let shrine_overlay = spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);
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
                    translation: Vec3::new(0., res.game_height / 2. - 40., 10.),
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
                    "(Replaces an existing skill slot)".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 62., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();
    // Spawn SkillView2 background
    let view2_bg = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::SkillView2),
            sprite: Sprite {
                custom_size: Some(Vec2::new(248.5, 171.5)), // Adjust size based on actual asset
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkillShrine)
        .insert(Name::new("SKILL_VIEW2_BACKGROUND"))
        .id();

    let banner_spacing = 80.;
    let start_y = 0. + -banner_spacing / 2.;

    for (index, skill_choice) in shrine_selection.skill_choices.iter().enumerate() {
        let banner_x = -70.;
        let banner_y = start_y + (index as f32 * banner_spacing);
        let tooltip_pos = Vec3::new(banner_x, banner_y, 15.);

        let container = commands
            .spawn(RenderLayers::from_layers(&[3]))
            .insert(SpatialBundle::from_transform(Transform {
                translation: tooltip_pos,
                scale: Vec3::new(1., 1., 11.),
                ..Default::default()
            }))
            .set_parent(view2_bg)
            .id();
        // Spawn SkillTooltipBanner
        let _banner = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::SkillTooltipBanner),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(236., 57.5)), // Same size as SkillTooltip
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
            .insert(Name::new(format!("SKILL_BANNER_{}", index)))
            .set_parent(container)
            .id();
        let (skill_power, blessings, max_mana, max_health, bonus_as, attack_speed, crit, spd, size) =
            skill_power.single();
        let bonus_as_mult = effective_player_attack_speed_multiplier(
            attack_speed.map(|a| a.0).unwrap_or(0),
            bonus_as.map(|b| b.get_multiplier()).unwrap_or(1.0),
        );
        spawn_skill_tooltip_content(
            &mut commands,
            &graphics,
            &asset_server,
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
        );
    }

    let back_button = spawn_back_button(
        Vec3::new(res.game_width / 2. - 55., -res.game_height / 2. + 38., 11.),
        &mut commands,
        &graphics,
        &asset_server,
    );
    commands
        .entity(back_button)
        .insert(UIState::ActiveSkillShrine);
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

                        commands.insert_resource(ActiveSkillShrineOverwrite {
                            skill_choice: picked_skill.clone(),
                            shrine_entity: shrine_selection.shrine_entity,
                        });
                        commands.remove_resource::<ActiveSkillShrineSelection>();
                        next_ui_state.set(UIState::ActiveSkills);
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

pub fn setup_active_skill_shrine_overwrite_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    shrine_overwrite: Res<ActiveSkillShrineOverwrite>,
    res: Res<ScreenResolution>,
    skills: Query<
        (
            &PlayerSkills,
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
    let overwrite_overlay = spawn_full_screen_ui_overlay_tuned(&mut commands, &res, 0.0, 0.95, 9.);
    commands
        .entity(overwrite_overlay)
        .insert(UIState::ActiveSkills);
    let _title_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Choose a Skill to Lose".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., res.game_height / 2. - 30., 10.),
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
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::ActiveSkills)
        .insert(Name::new("SKILL_VIEW4_BACKGROUND"))
        .id();

    let (
        skills,
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
    let bonus_as_mult = effective_player_attack_speed_multiplier(
        attack_speed.map(|a| a.0).unwrap_or(0),
        bonus_as.map(|b| b.get_multiplier()).unwrap_or(1.0),
    );
    let choices = vec![
        skills.active_skill_slot_0.clone(),
        skills.active_skill_slot_1.clone(),
        skills.active_skill_slot_2.clone(),
        skills.active_skill_slot_3.clone(),
    ];

    let banner_spacing = 60.;
    let start_y = 0. + -banner_spacing * 1.5;

    for (i, choice_option) in choices.iter().enumerate() {
        if choice_option.is_none() {
            continue;
        }
        let slot_idx = 3 - i;
        let choice = choice_option.as_ref().unwrap();
        let banner_x = -70.;
        let banner_y = start_y + (slot_idx as f32 * banner_spacing);
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

        let _banner = commands
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
                index: i,
                skill_choice: choice.clone(),
                interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
            })
            .insert(Interactable::default())
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::ActiveSkills)
            .insert(Name::new(format!("SKILL_SLOT_BANNER_{}", i)))
            .set_parent(container)
            .id();

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
        );
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
                        let (player_e, mut skills, _t) = player_skills.single_mut();
                        let new_skill = overwrite.skill_choice.clone();

                        match skill_ui.index {
                            0 => skills.active_skill_slot_0 = Some(new_skill.clone()),
                            1 => skills.active_skill_slot_1 = Some(new_skill.clone()),
                            2 => skills.active_skill_slot_2 = Some(new_skill.clone()),
                            3 => skills.active_skill_slot_3 = Some(new_skill.clone()),
                            _ => (),
                        }
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
