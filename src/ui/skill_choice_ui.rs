use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    colors::{BLACK, WHITE},
    player::skills::{SkillChoiceQueue, SkillChoiceState},
    ScreenResolution, DEBUG, GAME_HEIGHT,
};

use super::{damage_numbers::spawn_text, Interactable, UIElement, UIState, SKILLS_CHOICE_UI_SIZE};

#[derive(Component)]
pub struct SkillChoiceUI {
    pub index: usize,
    pub skill_choice: SkillChoiceState,
}

#[derive(Component)]
pub struct SkillDescText;
#[derive(Component)]
pub struct SkillTitleText;

pub fn setup_skill_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    choices_queue: Res<SkillChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    res: Res<ScreenResolution>,
) {
    if choices_queue.queue.is_empty() {
        next_ui_state.set(UIState::Closed);
        return;
    }
    let choices = &choices_queue.queue[0];
    let (size, t_offset) = (SKILLS_CHOICE_UI_SIZE, Vec2::new(4., 4.));

    // title bar
    let title_sprite = commands
        .spawn(SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::TitleBar).clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(168., 16.)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 80., 7.),
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

    let title_text = spawn_text(
        &mut commands,
        &asset_server,
        Vec3::new(0., 0., 1.),
        BLACK,
        "Choose a new skill".to_string(),
        Anchor::Center,
        2.,
        3,
    );
    commands
        .entity(title_text)
        .insert(UIState::Skills)
        .set_parent(title_sprite);

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(res.game_width + 10., GAME_HEIGHT + 10.)),
                ..default()
            },
            transform: Transform {
                translation: Vec3::new(-t_offset.x, -t_offset.y, -1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("overlay"));

    for i in -1..2 {
        let translation = Vec3::new(i as f32 * (size.x + 16.), 0., 1.);
        let choice = choices[(i + 1) as usize].clone();
        let ui_element = choice.skill.get_ui_element();
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
                index: i as usize,
                skill_choice: choice.clone(),
            })
            .insert(ui_element)
            .insert(UIState::Skills)
            .insert(Interactable::default())
            .insert(Name::new("SKILLS UI"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        // icon
        commands
            .spawn(SpriteBundle {
                texture: graphics.get_skill_icon(choice.skill.clone()),
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
            .insert(Name::new("SKILL ICON!!"))
            .set_parent(skills_e);

        let mut text_title = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    choice.skill.get_title(),
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
        for (j, desc) in choice.skill.get_desc().iter().enumerate() {
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
                            if i == 1 { 0.5 } else { 0. },
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

    // commands.entity(skills_e).push_children(&[overlay]);
}

pub fn toggle_skills_visibility(
    mut next_inv_state: ResMut<NextState<UIState>>,
    key_input: ResMut<Input<KeyCode>>,
    mut queue: ResMut<SkillChoiceQueue>,
) {
    if key_input.just_pressed(KeyCode::B) {
        next_inv_state.set(UIState::Skills);
    }

    if *DEBUG && key_input.just_pressed(KeyCode::N) {
        let remaining_choices = queue.queue.remove(0).to_vec();
        for choice in remaining_choices.iter() {
            queue.pool.push(choice.clone());
        }
        let mut rng = rand::thread_rng();
        queue.add_new_skills_after_levelup(&mut rng);
    }
}
