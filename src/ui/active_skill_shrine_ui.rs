use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::BLACK,
    item::active_skill_shrine::{ActiveSkillShrineOverwrite, ActiveSkillShrineSelection},
    player::skills::{ActiveSkillChoiceState, PlayerSkills},
    ScreenResolution, GAME_HEIGHT,
};

use super::{
    damage_numbers::spawn_text, interactions::Interaction, ui_helpers::spawn_ui_overlay,
    Interactable, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
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
) {
    let skill_choice = &shrine_selection.skill_choice;
    let t_offset = Vec2::new(4., 4.);

    // title bar
    let title_sprite = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::TitleBar).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(168., 16.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 80., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIElement::TitleBar)
        .insert(UIState::ActiveSkillShrine)
        .insert(Name::new("ACTIVE_SKILL_TITLE"))
        .id();

    let title_text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(0., 0., 1.),
        BLACK,
        "Gain a new skill".to_string(),
        Anchor::Center,
        2.,
        3,
    );
    commands
        .entity(title_text)
        .insert(UIState::ActiveSkillShrine)
        .set_parent(title_sprite);

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        0.8,
        9.,
    );

    // Spawn the single active skill choice
    spawn_active_skill_shrine_choice(
        &graphics,
        &mut commands,
        &asset_server,
        skill_choice.clone(),
        t_offset,
    );
}

fn spawn_active_skill_shrine_choice(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    choice: ActiveSkillChoiceState,
    t_offset: Vec2,
) {
    let size = SKILLS_CHOICE_UI_SIZE;
    let translation = Vec2::new(0.1, 0.);
    let ui_element = choice.active_skill.get_ui_element(choice.rarity.clone());

    let skills_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(translation.x + t_offset.x, translation.y + t_offset.y, 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ActiveSkillShrineUI {
            skill_choice: choice.clone(),
            interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
        })
        .insert(ui_element)
        .insert(UIState::ActiveSkillShrine)
        .insert(Interactable::default())
        .insert(Name::new("ACTIVE_SKILL_SHRINE_UI"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    // icon
    let _skill_icon = commands
        .spawn(SpriteBundle {
            texture: graphics.get_active_skill_icon(choice.active_skill.clone()),
            sprite: Sprite {
                custom_size: Some(Vec2::new(32., 32.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec2::new(0., 25.).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("SKILL ICON"))
        .set_parent(skills_e)
        .id();

    // Title text
    let mut text_title = commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                choice.active_skill.get_title(),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            ),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0.5, 50.5, 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("Skill Title TEXT"),
        RenderLayers::from_layers(&[3]),
    ));
    text_title.set_parent(skills_e);

    // Description text
    for (j, desc) in choice.active_skill.get_desc().iter().enumerate() {
        let mut text_desc = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    desc,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, -(j as f32 * 9.) + 0.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Skill Desc TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text_desc.set_parent(skills_e);
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
    cursor_pos: Res<crate::inputs::CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut skill_choices: Query<(Entity, &mut Interactable, &ActiveSkillShrineUI)>,
    mut player_skills: Query<(
        Entity,
        &mut crate::player::skills::PlayerSkills,
        &GlobalTransform,
    )>,
    shrine_selection: ResMut<ActiveSkillShrineSelection>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    mut att_event: EventWriter<crate::attributes::AttributeChangeEvent>,
    graphics: Res<Graphics>,
    mut shrine_query: Query<&mut crate::item::active_skill_shrine::ActiveSkillShrineState>,
    unlock_upgrades: Option<Res<crate::player::unlocks::UnlockUpgrades>>,
) {
    let hit_test = super::ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, shrine_ui) in skill_choices.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(crate::audio::SoundSpawner::new(
                        crate::audio::AudioSoundEffect::UISkillHover,
                        0.2,
                    ));

                    let ui_element = shrine_ui
                        .skill_choice
                        .active_skill
                        .get_ui_element_hover(shrine_ui.skill_choice.rarity.clone());
                    // swap to hover img
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed && shrine_ui.interaction_lock_timer.finished() {
                        let (player_e, mut skills, _t) = player_skills.single_mut();
                        let picked_skill = shrine_ui.skill_choice.clone();

                        // Check if player has open slots (check slot 3 first if unlocked)
                        let has_slot_2_unlocked = unlock_upgrades
                            .as_ref()
                            .map(|u| u.second_active_skill_slot_unlocked)
                            .unwrap_or(false);

                        if has_slot_2_unlocked && skills.active_skill_slot_2.is_none() {
                            // Auto-assign to slot 3 if unlocked and empty
                            skills.active_skill_slot_2 = Some(picked_skill.clone());
                        } else if skills.roll_skill_slot.is_none() {
                            skills.roll_skill_slot = Some(picked_skill.clone());
                        } else if skills.active_skill_slot_1.is_none() {
                            skills.active_skill_slot_1 = Some(picked_skill.clone());
                        } else if has_slot_2_unlocked {
                            // Slot 2 is full, show overwrite UI
                            commands.insert_resource(ActiveSkillShrineOverwrite {
                                skill_choice: picked_skill.clone(),
                                shrine_entity: shrine_selection.shrine_entity,
                            });
                            commands.remove_resource::<ActiveSkillShrineSelection>();
                            next_ui_state.set(UIState::ActiveSkills);
                            return;
                        } else {
                            // Both slots full - automatically swap the second active skill slot (slot 2, which is not Roll)
                            skills.active_skill_slot_1 = Some(picked_skill.clone());
                        }

                        // Add skill components
                        picked_skill
                            .active_skill
                            .add_skill_components(player_e, &mut commands);

                        // Mark shrine as used (completion system will handle cleanup)
                        if let Ok(mut shrine_state) =
                            shrine_query.get_mut(shrine_selection.shrine_entity)
                        {
                            shrine_state.is_used = true;
                        }

                        // Remove resource and close UI
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
                let ui_element = shrine_ui
                    .skill_choice
                    .active_skill
                    .get_ui_element(shrine_ui.skill_choice.rarity.clone());

                interactable.change(Interaction::None);
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
    skills: Query<&PlayerSkills>,
    unlock_upgrades: Option<Res<crate::player::unlocks::UnlockUpgrades>>,
) {
    let skills = skills.single();
    let mut choices = vec![skills.active_skill_slot_1.clone()];

    // Add slot 2 if unlocked
    if let Some(upgrades) = unlock_upgrades.as_ref() {
        if upgrades.second_active_skill_slot_unlocked {
            choices.push(skills.active_skill_slot_2.clone());
        }
    }
    let t_offset = Vec2::new(4., 4.);

    // title bar
    let title_sprite = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::TitleBar).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(168., 16.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 80., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIElement::TitleBar)
        .insert(UIState::ActiveSkills)
        .insert(Name::new("ACTIVE_SKILL_SHRINE_OVERWRITE_TITLE"))
        .id();

    let title_text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(0., 0., 1.),
        BLACK,
        "swap active skill".to_string(),
        Anchor::Center,
        2.,
        3,
    );
    commands
        .entity(title_text)
        .insert(UIState::ActiveSkills)
        .set_parent(title_sprite);

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        0.8,
        9.,
    );

    // Spawn the existing active skills so the player can choose which to replace
    let size = super::SKILLS_CHOICE_UI_SIZE;
    let mut spawned_count = 0;
    for (slot_idx, choice) in choices.iter().enumerate() {
        if choice.is_none() {
            continue;
        }
        let choice = choice.as_ref().unwrap();
        // Use the actual slot index (0 for slot 1, 1 for slot 2) for matching
        let actual_slot_idx = slot_idx;
        let translation = Vec2::new(
            (spawned_count as i32 - 1) as f32 * (size.x + 16.)
                + if choices.iter().filter(|c| c.is_some()).count() == 2 {
                    size.x / 2.
                } else {
                    0.
                }
                + 0.1,
            0.,
        );
        let ui_element = choice.active_skill.get_ui_element(choice.rarity.clone());
        let skills_e = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(ui_element.clone()),
                sprite: Sprite {
                    custom_size: Some(size),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(
                        translation.x + t_offset.x,
                        translation.y + t_offset.y,
                        10.,
                    ),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(ActiveSkillSlotChoiceUI {
                index: actual_slot_idx, // 0 for slot 1, 1 for slot 2
                skill_choice: choice.clone(),
                interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
            })
            .insert(ui_element)
            .insert(UIState::ActiveSkills)
            .insert(Interactable::default())
            .insert(Name::new("ACTIVE_SKILL_SLOT_CHOICE"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();

        spawned_count += 1;

        // icon
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_active_skill_icon(choice.active_skill.clone()),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(32., 32.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec2::new(0., 25.).extend(4.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("SKILL ICON"))
            .set_parent(skills_e);

        // Title text
        let mut text_title = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    choice.active_skill.get_title(),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::WHITE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, 50.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Skill Title TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text_title.set_parent(skills_e);

        // Description text
        for (j, desc) in choice.active_skill.get_desc().iter().enumerate() {
            let mut text_desc = commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        desc,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: crate::colors::WHITE,
                        },
                    ),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(0.5, -(j as f32 * 9.) + 0.5, 1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("Skill Desc TEXT"),
                RenderLayers::from_layers(&[3]),
            ));
            text_desc.set_parent(skills_e);
        }
    }

    // New Active Skill Icon (the one we're trying to add)
    commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_active_skill_icon(shrine_overwrite.skill_choice.active_skill.clone()),
            sprite: Sprite {
                custom_size: Some(Vec2::new(32., 32.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec2::new(-4., -60.).extend(20.),
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
    cursor_pos: Res<crate::inputs::CursorPos>,
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
                            1 => {
                                skills.active_skill_slot_1 = Some(new_skill.clone());
                                // Add skill components
                                new_skill
                                    .active_skill
                                    .add_skill_components(player_e, &mut commands);
                            }
                            2 => {
                                skills.active_skill_slot_2 = Some(new_skill.clone());
                                // Add skill components
                                new_skill
                                    .active_skill
                                    .add_skill_components(player_e, &mut commands);
                            }
                            _ => (),
                        }

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
                let ui_element = skill_ui
                    .skill_choice
                    .active_skill
                    .get_ui_element(skill_ui.skill_choice.rarity.clone());

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}
