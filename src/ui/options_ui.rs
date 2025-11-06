use bevy::prelude::*;
use bevy::render::view::RenderLayers;

use crate::{
    assets::Graphics,
    inputs::CursorPos,
    player::{skills::PlayerSkills, Player},
    ui::{interactions::Interaction, ui_helpers, Interactable, UIElement},
    ScreenResolution,
};

#[derive(Resource, Default, Debug, Clone)]
pub struct AudioSettings {
    pub master_volume: f32,
    pub sfx_volume: f32,
    pub bgm_volume: f32,
}

impl AudioSettings {
    pub fn new() -> Self {
        Self {
            master_volume: 1.0,
            sfx_volume: 1.0,
            bgm_volume: 0.75,
        }
    }
}

#[derive(Component)]
pub struct OptionsUI;

#[derive(Component)]
pub struct SwapSkillSlotsButton;

pub fn setup_options_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    audio_settings: Res<AudioSettings>,
) {
    // Spawn overlay
    let overlay = ui_helpers::spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width, resolution.game_height),
        0.9,
        0.,
    );
    commands.entity(overlay).insert(OptionsUI);

    // Title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Options",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 20.0,
                    color: crate::colors::YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 80., 1.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Options Title"),
    ));

    // Audio Volume Label
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                format!(
                    "Master Volume: {:.0}%",
                    audio_settings.master_volume * 100.0
                ),
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 8.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 40., 1.)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        OptionsUI,
        Name::new("Audio Volume Text"),
    ));

    // Swap Skill Slots Button (parent sprite + child text)
    let swap_button_e = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(100., 20.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., -20., 1.)),
                ..Default::default()
            },
            Interactable::default(),
            SwapSkillSlotsButton,
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            Name::new("Swap Skill Slots Button"),
        ))
        .id();

    let swap_text_e = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Swap Skill Slots",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 10.0,
                        color: crate::colors::YELLOW_2,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            Name::new("Swap Skill Slots Text"),
        ))
        .id();
    commands.entity(swap_button_e).add_child(swap_text_e);

    // Back Button (parent sprite + child text)
    let back_button_e = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(60., 20.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(0., -60., 1.)),
                ..Default::default()
            },
            Interactable::default(),
            UIElement::MenuButton,
            crate::ui::main_menu::MenuButton::Options,
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            Name::new("Back Button"),
        ))
        .id();

    let back_text_e = commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Back",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 10.0,
                        color: crate::colors::YELLOW_2,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: bevy::sprite::Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            OptionsUI,
            Name::new("Back Text"),
        ))
        .id();
    commands.entity(back_button_e).add_child(back_text_e);
}

pub fn handle_options_clicks(
    mut param_set: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<(Entity, &mut Interactable), With<SwapSkillSlotsButton>>,
        Query<
            (Entity, &mut Interactable),
            (With<UIElement>, With<crate::ui::main_menu::MenuButton>),
        >,
    )>,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    mut player_skills: Query<&mut PlayerSkills, With<Player>>,
    mut next_ui_state: ResMut<NextState<crate::ui::UIState>>,
    curr_ui_state: Res<State<crate::GameState>>,
) {
    return;
    let left_mouse_pressed = mouse_input.just_pressed(MouseButton::Left);

    // First, find which entity was clicked using read-only query
    let ui_sprites = param_set.p0();
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let clicked_entity = if let Some((hit_entity, _, _)) = hit_test {
        hit_entity
    } else {
        // No click, reset hover states
        let mut swap_button = param_set.p1();
        for (_, mut interactable) in swap_button.iter_mut() {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
        let mut back_button = param_set.p2();
        for (_, mut interactable) in back_button.iter_mut() {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
        return;
    };

    // Handle swap skill slots button
    let mut swap_button = param_set.p1();
    for (entity, mut interactable) in swap_button.iter_mut() {
        if entity == clicked_entity {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        // Swap skill slots (only if in-game, not in main menu)
                        if curr_ui_state.0 == crate::GameState::Main {
                            if let Ok(mut skills) = player_skills.get_single_mut() {
                                let temp = skills.active_skill_slot_1.take();
                                skills.active_skill_slot_1 = skills.active_skill_slot_2.take();
                                skills.active_skill_slot_2 = temp;
                            }
                        }
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

    // Handle back button
    let mut back_button = param_set.p2();
    for (entity, mut interactable) in back_button.iter_mut() {
        if entity == clicked_entity {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                }
                Interaction::Hovering => {
                    if left_mouse_pressed {
                        next_ui_state.set(crate::ui::UIState::Closed);
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

pub fn cleanup_options_ui(mut commands: Commands, options_ui: Query<Entity, With<OptionsUI>>) {
    for entity in options_ui.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
