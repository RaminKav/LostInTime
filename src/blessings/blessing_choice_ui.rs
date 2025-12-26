use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::seq::IteratorRandom;
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::{Blessing, OwnedBlessings},
    colors::WHITE,
    inputs::CursorPos,
    player::skills::Heirloom,
    ui::{
        ui_helpers::{self, spawn_ui_overlay},
        Interactable, Interaction, UIElement, UIState, SKILLS_CHOICE_UI_SIZE,
    },
    world::dimension::{DimensionSpawnEvent, Era},
    GameParam, ScreenResolution, GAME_HEIGHT,
};

#[derive(Component)]
pub struct BlessingChoiceUI {
    pub index: usize,
    pub blessing_choice: Blessing,
}

pub struct BlessingSelectEvent {
    pub blessing: Blessing,
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
        .choose_multiple(&mut rng, 2);
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

    spawn_ui_overlay(
        &mut commands,
        Vec2::new(res.game_width + 10., GAME_HEIGHT + 100.),
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
                index: (i + 1) as usize,
                blessing_choice: choice.clone(),
            })
            .insert(UIElement::SkillChoice)
            .insert(UIState::BlessingChoice)
            .insert(Interactable::default())
            .insert(Name::new("BLESSING CHOICE UI"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        // icon - all heirlooms use the heirloom icon (active skills are now separate)
        let skill_icon = commands
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
    }
}

pub fn handle_blessing_choice_card_interactions(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut blessing_choices: Query<(Entity, &mut Interactable, &BlessingChoiceUI)>,
    curr_ui_state: Res<State<UIState>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut chaos_tracker: ResMut<crate::chaos::ChaosTracker>,
    game: GameParam,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut blessing_event: EventWriter<BlessingSelectEvent>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);
    let ui_state = &curr_ui_state.0;

    for (e, mut interactable, state) in blessing_choices.iter_mut() {
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
                        blessing_event.send(BlessingSelectEvent {
                            blessing: state.blessing_choice,
                        });
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
