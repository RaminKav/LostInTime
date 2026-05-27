use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::seq::IteratorRandom;
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{Blessing, OwnedBlessings},
    chaos::IncreaseChaosEvent,
    colors::{RED, WHITE},
    cursor::CursorPos,
    player::skills::{Heirloom, HeirloomWithRarity},
    ui::{
        damage_numbers::spawn_floating_text_with_shadow,
        game_fonts::FLOATING_TEXT,
        ui_helpers::{self, spawn_full_screen_ui_overlay},
        Interactable, Interaction, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
    },
    world::dimension::{DimensionSpawnEvent, Era},
    GameParam, ScreenResolution, GAME_HEIGHT,
};

#[derive(Component)]
pub struct BlessingChoiceUI {
    pub selected: bool,
    pub blessing_choice: Blessing,
}

pub struct BlessingSelectEvent {
    pub blessing: Blessing,
}

#[derive(Resource)]
pub struct BlessingTransitionState {
    pub timer: Timer,
    pub heirlooms: Option<Vec<HeirloomWithRarity>>,
}

pub fn enter_blessing_ui(mut next_ui_state: ResMut<NextState<UIState>>) {
    next_ui_state.set(UIState::BlessingChoice);
}

pub fn setup_blessing_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    res: Res<ScreenResolution>,
    owned_blessings: Query<&OwnedBlessings>,
) {
    let mut rng = rand::thread_rng();
    let owned_blessings = owned_blessings.single();
    let blessing_choices = Blessing::iter()
        .filter(|c| !owned_blessings.has_blessing(*c))
        .choose_multiple(&mut rng, 3);
    let t_offset = Vec2::new(4., 4.);

    let _title_text = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Choose a Blessing".to_string(),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 80., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            UIState::BlessingChoice,
        ))
        .id();

    spawn_full_screen_ui_overlay(
        &mut commands,
        &res,
        1.,
        9.,
    );

    // SPAWN CHOICE CARDS

    let size = SKILLS_CHOICE_UI_SIZE;
    let count = blessing_choices.len();
    for i in -1i32..(blessing_choices.len() as i32 - 1) {
        let translation = Vec2::new(
            i as f32 * (size.x + 16.) + if count == 2 { size.x / 2. } else { 0. } + 0.1,
            0.,
        );
        let choice = blessing_choices[(i + 1) as usize].clone();
        let skills_e = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::SkillChoice),
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
            .insert(BlessingChoiceUI {
                selected: false,
                blessing_choice: choice.clone(),
            })
            .insert(UIElement::SkillChoice)
            .insert(UIState::BlessingChoice)
            .insert(Interactable::default())
            .insert(Name::new("BLESSING CHOICE UI"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        // icon - all heirlooms use the heirloom icon (active skills are now separate)
        let _skill_icon = commands
            .spawn(SpriteSheetBundle {
                sprite: graphics.get_heirloom_icon(Heirloom::MPBarCrit),
                texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                transform: Transform {
                    translation: Vec2::new(0., 25.).extend(4.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(BlessingChoiceUI {
                selected: false,
                blessing_choice: choice.clone(),
            })
            .insert(Name::new("BLESSING ICON!!"))
            .set_parent(skills_e)
            .id();

        let mut text_title = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    choice.get_title(),
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
            Name::new("Skill Title TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text_title.set_parent(skills_e);
        for (j, desc) in choice.get_description().iter().enumerate() {
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
                Name::new("Skill Desc TEXT"),
                RenderLayers::from_layers(&[3]),
            ));
            text_desc.set_parent(skills_e);
        }

        // CHAOS TEXT
        let mut text_chaos = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    format!("+{} Chaos", choice.get_chaos_increase()),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: RED,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0.5, -70.5, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("chaos TEXT"),
            RenderLayers::from_layers(&[3]),
        ));
        text_chaos.set_parent(skills_e);
    }
}

pub fn handle_blessing_choice_card_interactions(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut blessing_choices: Query<(Entity, &mut Interactable, &mut BlessingChoiceUI)>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut blessing_event: EventWriter<BlessingSelectEvent>,
    mut chaos_event: EventWriter<IncreaseChaosEvent>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    for (e, mut interactable, mut state) in blessing_choices.iter_mut() {
        match hit_test {
            Some(hit_ent) if hit_ent.0 == e => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillHover, 0.2));

                    let ui_element = UIElement::SkillChoiceHover;
                    // swap to hover img
                    commands
                        .entity(e)
                        .insert(ui_element.clone())
                        .insert(graphics.get_ui_element_texture(ui_element));
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        // TODO: Handle click - apply blessing effect
                        info!("CLICKED BLESSING: {:?}", state.blessing_choice);
                        state.selected = true;
                        blessing_event.send(BlessingSelectEvent {
                            blessing: state.blessing_choice,
                        });

                        chaos_event.send(IncreaseChaosEvent {
                            amount: state.blessing_choice.get_chaos_increase() as f32,
                        });
                        let delay_sec = if state.blessing_choice == Blessing::GainCommonHeirlooms
                            || state.blessing_choice == Blessing::GainLegendaryHeirloom
                            || state.blessing_choice == Blessing::GainRareHeirloom
                        {
                            2.
                        } else {
                            0.75
                        };
                        commands.insert_resource(BlessingTransitionState {
                            timer: Timer::from_seconds(delay_sec, TimerMode::Once),
                            heirlooms: None,
                        });
                    }
                }
                _ => (),
            },
            _ => {
                // reset hovering states if we stop hovering ?
                let Interaction::Hovering = interactable.current() else {
                    continue;
                };
                let ui_element = UIElement::SkillChoice;

                interactable.change(Interaction::None);
                commands
                    .entity(e)
                    .insert(ui_element.clone())
                    .insert(graphics.get_ui_element_texture(ui_element));
            }
        }
    }
}

pub fn transition_to_next_era_after_blessing(
    mut timer: ResMut<BlessingTransitionState>,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    game: GameParam,
    time: Res<Time>,
    mut commands: Commands,
) {
    timer.timer.tick(time.delta());
    if timer.timer.just_finished() {
        // Determine next era based on current era
        let current_era = game.era.current_era.clone();

        let next_era = match current_era {
            Era::Main => Some(Era::Second),
            Era::Second => Some(Era::Third),
            Era::Third => {
                return;
            }
            Era::DungeonMain => {
                // Shouldn't be able to use portal in dungeon
                return;
            }
        };

        if let Some(era) = next_era {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(era),
            });
        }

        next_ui_state.set(UIState::Closed);
        commands.remove_resource::<BlessingTransitionState>();
    }
}

pub fn transition_blessing_ui_after_choice(
    mut state: ResMut<BlessingTransitionState>,
    mut blessing_cards: Query<(
        Entity,
        &mut Sprite,
        &BlessingChoiceUI,
        &Children,
        &GlobalTransform,
    )>,
    mut children_query_1: Query<&mut TextureAtlasSprite>,
    mut children_query_2: Query<&mut Visibility, With<Text>>,
    graphics: Res<Graphics>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    let timer_percent = state.timer.percent();
    for (_ent, mut sprite, blessing_choice_ui, children, transform) in blessing_cards.iter_mut() {
        if blessing_choice_ui.selected {
            // Add icon
            for (i, heirloom) in state
                .heirlooms
                .clone()
                .unwrap_or(Vec::new())
                .iter()
                .enumerate()
            {
                let floating_text = spawn_floating_text_with_shadow(
                    &mut commands,
                    &asset_server,
                    transform.translation() + Vec3::new(0., -90. - 12. * i as f32, 1.),
                    heirloom.rarity.get_color(),
                    heirloom.heirloom.get_title(),
                    FLOATING_TEXT,
                );
                commands
                    .entity(floating_text)
                    .insert(RenderLayers::from_layers(&[3]));
                let icon = commands
                    .spawn(SpriteSheetBundle {
                        sprite: graphics.get_heirloom_icon(heirloom.heirloom.clone()),
                        texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                        transform: Transform {
                            translation: Vec2::new(12., 0.).extend(4.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(RenderLayers::from_layers(&[3]))
                    .insert(Name::new("HEIRLOOM ICON"))
                    .id();
                commands.entity(icon).set_parent(floating_text);

                // Add rarity-based background if not common
                if let Some(glow) = heirloom.rarity.get_item_glow() {
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
                        .set_parent(icon);
                }
            }
            state.heirlooms = None;
        } else {
            sprite
                .color
                .set_a((1.0 - (timer_percent * 8.)).clamp(0.0, 1.0));
            for child in children.iter() {
                if let Ok(mut child_sprite) = children_query_1.get_mut(*child) {
                    child_sprite
                        .color
                        .set_a((1.0 - (timer_percent * 8.)).clamp(0.0, 1.0));
                }
                if let Ok(mut visibility) = children_query_2.get_mut(*child) {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }
}
