use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    animations::DoneAnimation,
    assets::Graphics,
    colors::{BLACK, WHITE},
    player::skills::{SkillChoiceQueue, SkillChoiceState},
    ScreenResolution, DEBUG, GAME_HEIGHT,
};

use super::{
    damage_numbers::spawn_text, ui_helpers::spawn_ui_overlay, Interactable, UIElement, UIState,
    SKILLS_CHOICE_UI_SIZE,
};

#[derive(Component)]
pub struct SkillChoiceUI {
    pub index: usize,
    pub skill_choice: SkillChoiceState,
}

#[derive(Component)]
pub struct SkillDescText;
#[derive(Component)]
pub struct SkillTitleText;

#[derive(Component)]
pub struct RerollDice(pub usize);

aseprite!(pub SkillChoiceFlash, "ui/SkillChoiceFlash.aseprite");

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
        choices.clone(),
        t_offset,
    );

    for i in -1..2 {
        if !choices_queue.rerolls[(i + 1) as usize] {
            continue;
        }
        // reroll dice icon
        commands
            .spawn(SpriteBundle {
                texture: graphics
                    .get_ui_element_texture(UIElement::RerollDice)
                    .clone(),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(21., 22.)),
                    ..Default::default()
                },
                transform: Transform {
                    translation: Vec3::new(
                        i as f32 * (SKILLS_CHOICE_UI_SIZE.x + 16.) + 4.5,
                        -70.,
                        10.,
                    ),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIElement::RerollDice)
            .insert(Interactable::default())
            .insert(UIState::Skills)
            .insert(RerollDice((i + 1) as usize))
            .insert(Name::new("DICE"));
    }
}

pub fn spawn_skill_choice_entities(
    graphics: &Graphics,
    commands: &mut Commands,
    asset_server: &AssetServer,
    choices: [SkillChoiceState; 3],
    t_offset: Vec2,
) {
    let size = SKILLS_CHOICE_UI_SIZE;
    for i in -1..2 {
        let translation = Vec2::new(i as f32 * (size.x + 16.) + 0.1, 0.);
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
    mut next_inv_state: ResMut<NextState<UIState>>,
    key_input: ResMut<Input<KeyCode>>,
    mut queue: ResMut<SkillChoiceQueue>,
    old_skill_entities: Query<Entity, With<SkillChoiceUI>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    if key_input.just_pressed(KeyCode::B) {
        next_inv_state.set(UIState::Skills);
    }

    if *DEBUG && key_input.just_pressed(KeyCode::N) {
        let remaining_choices = queue.queue.remove(0).to_vec();
        for choice in remaining_choices.iter() {
            queue.pool.push(choice.clone());
        }
        for e in old_skill_entities.iter() {
            commands.entity(e).despawn_recursive();
        }

        let mut rng = rand::thread_rng();
        queue.add_new_skills_after_levelup(&mut rng);
        spawn_skill_choice_entities(
            &graphics,
            &mut commands,
            &asset_server,
            queue.queue[0].clone(),
            Vec2::new(4., 4.),
        );
    }
}
pub fn handle_skill_reroll_after_flash(
    flashes: Query<(Entity, &RerollDice, &AsepriteAnimation), With<DoneAnimation>>,
    mut skill_queue: ResMut<SkillChoiceQueue>,
    old_skill_entities: Query<Entity, With<SkillChoiceUI>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
) {
    for (e, slot, anim) in flashes.iter() {
        if anim.current_frame() == 3 {
            commands.entity(e).remove::<RerollDice>();
            skill_queue.handle_reroll_slot(slot.0);
            for e in old_skill_entities.iter() {
                commands.entity(e).despawn_recursive();
            }
            spawn_skill_choice_entities(
                &graphics,
                &mut commands,
                &asset_server,
                skill_queue.queue[0].clone(),
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
