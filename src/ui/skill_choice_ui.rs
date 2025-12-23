use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    animations::DoneAnimation,
    assets::Graphics,
    attributes::{ItemAttributes, LootRateBonus},
    colors::{BLACK, WHITE},
    player::{
        skills::{HeirloomChoiceQueue, HeirloomChoiceState},
        unlocks::RunUnlockState,
        Player,
    },
    ScreenResolution, DEBUG, GAME_HEIGHT,
};

use super::{
    damage_numbers::spawn_text, ui_helpers::spawn_ui_overlay, Interactable, UIElement, UIState,
    SKILLS_CHOICE_UI_SIZE,
};

#[derive(Component)]
pub struct SkillChoiceUI {
    pub index: usize,
    pub skill_choice: HeirloomChoiceState,
    pub interaction_lock_timer: Timer,
}

#[derive(Component)]
pub struct SkillDescText;
#[derive(Component)]
pub struct SkillTitleText;

#[derive(Component)]
pub struct RerollDice(pub usize);

#[derive(Component)]
pub struct BanishButton(pub usize);

#[derive(Component)]
pub struct BanishButtonLabel;

#[derive(Component)]
pub struct RerollCountText;

#[derive(Component)]
pub struct BanishCountText;

aseprite!(pub SkillChoiceFlash, "ui/SkillChoiceFlash.aseprite");

pub fn setup_skill_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    choices_queue: Res<HeirloomChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    res: Res<ScreenResolution>,
    run_unlocks: Res<RunUnlockState>,
) {
    if choices_queue.queue.is_empty() {
        next_ui_state.set(UIState::Closed);
        return;
    }
    let choices = &choices_queue.queue[0];
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
        .insert(UIState::Skills)
        .insert(Name::new("SKILL ICON!!"))
        .id();

    let title_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Choose an Heirloom".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: BLACK,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
        ))
        .id();

    commands
        .entity(title_text)
        .insert(UIState::Skills)
        .set_parent(title_sprite);

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
        0.8,
        9.,
    );

    spawn_skill_choice_entities(
        &graphics,
        &mut commands,
        &asset_server,
        choices.clone().to_vec(),
        t_offset,
    );

    for i in -1i32..2 {
        let slot_index = (i + 1) as usize;
        let enabled = run_unlocks.rerolls_remaining > 0;
        let translation = Vec3::new(i as f32 * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.5, -70., 10.);
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

    let banish_enabled = run_unlocks.banishes_remaining > 0;
    if banish_enabled {
        for i in -1i32..2 {
            let slot_index = (i + 1) as usize;
            let translation =
                Vec3::new(i as f32 * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4., -96.5, 10.);
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
                .insert(Name::new(format!("BANISH BUTTON {slot_index}")));
            if banish_enabled {
                banish_button.insert(Interactable::default());
            }

            commands
                .spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "Banish ",
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: if banish_enabled {
                                    WHITE
                                } else {
                                    Color::rgb(0.7, 0.7, 0.7)
                                },
                            },
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_translation(Vec3::new(2., 0., 1.)),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    UIState::Skills,
                    BanishButtonLabel,
                    Name::new(format!("BANISH BUTTON TEXT {slot_index}")),
                ))
                .set_parent(banish_entity);
        }
    }

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!("Rerolls: {}", run_unlocks.rerolls_remaining),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(-80.5, -110., 15.)),
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
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(80., -110., 15.)),
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
/// Helper function to spawn a single heirloom tooltip card
/// Returns the entity ID of the card
/// scaling_text: Optional text showing current scaling value (e.g., "(+25% damage)")
pub fn spawn_heirloom_tooltip_card(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    heirloom: crate::player::skills::Heirloom,
    rarity: crate::player::skills::HeirloomRarity,
    position: Vec3,
    scaling_text: Option<String>,
) -> Entity {
    let size = SKILLS_CHOICE_UI_SIZE;
    let ui_element = heirloom.get_ui_element(rarity.clone());
    let card_e = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(ui_element.clone()),
            sprite: Sprite {
                custom_size: Some(size),
                ..Default::default()
            },
            transform: Transform {
                translation: position,
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(ui_element)
        .insert(UIState::Essence)
        .insert(Name::new("HEIRLOOM TOOLTIP CARD"))
        .insert(RenderLayers::from_layers(&[3]))
        .id();

    // icon
    let skill_icon = commands
        .spawn(SpriteSheetBundle {
            sprite: graphics.get_heirloom_icon(heirloom.clone()),
            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
            transform: Transform {
                translation: Vec2::new(0., 25.).extend(4.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("HEIRLOOM ICON"))
        .set_parent(card_e)
        .id();

    // Add rarity-based background if not common
    if let Some(glow) = rarity.get_item_glow() {
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_item_glow(glow),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(32., 32.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec2::new(0., 0.).extend(-1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(skill_icon);
    }

    // title
    let mut text_title = commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                heirloom.get_title(),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
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
        Name::new("Heirloom Title"),
        RenderLayers::from_layers(&[3]),
    ));
    text_title.set_parent(card_e);

    // description
    for (j, desc) in heirloom.get_desc().iter().enumerate() {
        let mut text_desc = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    desc,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
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
            Name::new("Heirloom Desc"),
            RenderLayers::from_layers(&[3]),
        ));
        text_desc.set_parent(card_e);
    }

    // Add scaling text if provided (shows current progress for scaling heirlooms)
    if let Some(scaling_text) = scaling_text {
        let desc_count = heirloom.get_desc().len();
        let mut text_scaling = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    scaling_text,
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: crate::colors::LIGHT_GREY, // Dark grey for subtle display
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, -(desc_count as f32 * 9.) - 5.0, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("Heirloom Scaling Text"),
            RenderLayers::from_layers(&[3]),
        ));
        text_scaling.set_parent(card_e);
    }

    card_e
}

pub fn spawn_skill_choice_entities(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    choices: Vec<HeirloomChoiceState>,
    t_offset: Vec2,
) {
    let size = SKILLS_CHOICE_UI_SIZE;
    let count = choices.len();
    for i in -1i32..(choices.len() as i32 - 1) {
        let translation = Vec2::new(
            i as f32 * (size.x + 16.) + if count == 2 { size.x / 2. } else { 0. } + 0.1,
            0.,
        );
        let choice = choices[(i + 1) as usize].clone();
        let ui_element = choice.heirloom.get_ui_element(choice.rarity.clone());
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
            .insert(SkillChoiceUI {
                index: (i + 1) as usize,
                skill_choice: choice.clone(),
                interaction_lock_timer: Timer::from_seconds(0.75, TimerMode::Once),
            })
            .insert(ui_element)
            .insert(UIState::Skills)
            .insert(Interactable::default())
            .insert(Name::new("SKILLS UI"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        // icon - all heirlooms use the heirloom icon (active skills are now separate)
        let skill_icon = commands
            .spawn(SpriteSheetBundle {
                sprite: graphics.get_heirloom_icon(choice.heirloom.clone()),
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: Vec2::new(0., 25.).extend(4.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("SKILL ICON!!"))
            .set_parent(skills_e)
            .id();

        // Add rarity-based background if not common
        if let Some(glow) = choice.rarity.get_item_glow() {
            commands
                .spawn(SpriteBundle {
                    texture: graphics.get_item_glow(glow),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(32., 32.)),
                        ..Default::default()
                    },
                    transform: Transform {
                        translation: Vec2::new(0., 0.).extend(-1.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .set_parent(skill_icon);
        }

        let mut text_title = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    choice.heirloom.get_title(),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
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
            SkillTitleText,
            Name::new("Skill Title TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text_title.set_parent(skills_e);
        for (j, desc) in choice.heirloom.get_desc().iter().enumerate() {
            let mut text_desc = commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        desc,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: WHITE,
                        },
                    ),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(
                            if i == 0 { 0. } else { 0.5 },
                            -(j as f32 * 9.) + 0.5,
                            1.,
                        ),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                SkillDescText,
                Name::new("Skill Desc TEXT"),
                RenderLayers::from_layers(&[3]),
            ));
            text_desc.set_parent(skills_e);
        }
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
    player_atts: Query<&LootRateBonus, With<Player>>,
) {
    if curr_ui_state.0 == UIState::ActiveSkills {
        return;
    }

    if *DEBUG && key_input.just_pressed(KeyCode::N) {
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
        let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);
        queue.add_new_skills_after_levelup(&mut rng, loot_bonus);
        spawn_skill_choice_entities(
            &graphics,
            &mut commands,
            &asset_server,
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
    player_atts: Query<&LootRateBonus, With<Player>>,
) {
    for (e, slot, anim) in flashes.iter() {
        if anim.current_frame() == 3 {
            let loot_bonus = player_atts.get_single().map(|a| a.0).unwrap_or(0);
            commands.entity(e).remove::<RerollDice>();
            skill_queue.handle_reroll_slot(slot.0, &mut rand::thread_rng(), loot_bonus);
            for e in old_skill_entities.iter() {
                commands.entity(e).despawn_recursive();
            }
            spawn_skill_choice_entities(
                &graphics,
                &mut commands,
                &asset_server,
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
    mut reroll_buttons: Query<&mut Sprite, (With<RerollDice>, Without<BanishButton>)>,
    mut banish_buttons: Query<&mut Sprite, (With<BanishButton>, Without<RerollDice>)>,
    mut banish_labels: Query<&mut Text, With<BanishButtonLabel>>,
) {
    if !run_unlocks.is_changed() {
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

    let (banish_color, label_color) = if run_unlocks.banishes_remaining > 0 {
        (Color::WHITE, WHITE)
    } else {
        (Color::rgb(0.5, 0.5, 0.5), Color::rgb(0.7, 0.7, 0.7))
    };
    for mut sprite in banish_buttons.iter_mut() {
        sprite.color = banish_color;
    }
    for mut text in banish_labels.iter_mut() {
        text.sections[0].style.color = label_color;
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
