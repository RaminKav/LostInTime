use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use serde::{Deserialize, Serialize};

use crate::{
    assets::Graphics, colors::BLACK, inventory::ItemStack, player::skills::Skill, GAME_HEIGHT,
    GAME_WIDTH,
};
use rand::seq::IteratorRandom;

use super::{spawn_item_stack_icon, Interactable, UIElement, UIState, SKILLS_CHOICE_UI_SIZE};

#[derive(Component)]
pub struct SkillChoiceUI {
    pub index: usize,
    pub skill_choice: SkillChoiceState,
}

#[derive(Component)]
pub struct SkillDescText;
#[derive(Component)]
pub struct SkillTitleText;

#[derive(Clone, Eq, PartialEq, Default, Debug, Serialize, Deserialize)]
pub struct SkillChoiceState {
    pub skill: Skill,
    pub child_skills: Vec<SkillChoiceState>,
}
impl SkillChoiceState {
    pub fn new(skill: Skill) -> Self {
        Self {
            skill,
            child_skills: Default::default(),
        }
    }
    pub fn with_children(mut self, children: Vec<SkillChoiceState>) -> Self {
        self.child_skills = children;
        self
    }
}

#[derive(Resource, Clone, Serialize, Deserialize)]
pub struct SkillChoiceQueue {
    pub queue: Vec<[SkillChoiceState; 3]>,
    pub pool: Vec<SkillChoiceState>,
}

impl Default for SkillChoiceQueue {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            pool: vec![
                SkillChoiceState::new(Skill::CritChance).with_children(vec![
                    SkillChoiceState::new(Skill::CritDamage),
                    SkillChoiceState::new(Skill::CritLoot),
                ]),
                SkillChoiceState::new(Skill::Health),
                SkillChoiceState::new(Skill::Speed),
                SkillChoiceState::new(Skill::AttackSpeed),
                SkillChoiceState::new(Skill::DodgeChance),
                SkillChoiceState::new(Skill::FireDamage),
                SkillChoiceState::new(Skill::WaveAttack),
                SkillChoiceState::new(Skill::FrailStacks),
                SkillChoiceState::new(Skill::SlowStacks),
                SkillChoiceState::new(Skill::PoisonStacks),
                SkillChoiceState::new(Skill::Teleport).with_children(vec![
                    SkillChoiceState::new(Skill::TeleportShock),
                    SkillChoiceState::new(Skill::TimeSlow),
                ]),
                SkillChoiceState::new(Skill::ClawDoubleThrow),
                SkillChoiceState::new(Skill::BowMultiShot),
                SkillChoiceState::new(Skill::ChainLightning),
            ],
        }
    }
}

impl SkillChoiceQueue {
    pub fn add_new_skills_after_levelup(&mut self, rng: &mut rand::rngs::ThreadRng) {
        let mut new_skills: [SkillChoiceState; 3] = Default::default();
        for i in 0..3 {
            new_skills[i] = self.pool.iter().choose(rng).unwrap().clone();
            self.pool.retain(|x| x != &new_skills[i]);
        }
        self.queue.push(new_skills.clone());
    }

    pub fn handle_pick_skill(&mut self, skill: SkillChoiceState) {
        let mut remaining_choices = self.queue.remove(0).to_vec();
        remaining_choices.retain(|x| x != &skill);
        for choice in remaining_choices.iter() {
            self.pool.push(choice.clone());
        }
        for child in skill.child_skills.iter() {
            self.pool.push(child.clone());
        }
    }
}

pub fn setup_skill_choice_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    choices_queue: Res<SkillChoiceQueue>,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    if choices_queue.queue.is_empty() {
        next_ui_state.set(UIState::Closed);
        return;
    }
    let choices = &choices_queue.queue[0];
    let (size, texture, t_offset) = (
        SKILLS_CHOICE_UI_SIZE,
        graphics.get_ui_element_texture(UIElement::SkillChoice),
        Vec2::new(4., 4.),
    );

    commands
        .spawn(SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(146. / 255., 116. / 255., 65. / 255., 0.3),
                custom_size: Some(Vec2::new(GAME_WIDTH + 10., GAME_HEIGHT + 10.)),
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
        let translation = Vec3::new(i as f32 * (size.x + 16.), 10., 1.);
        let choice = choices[i as usize + 1].clone();
        let skills_e = commands
            .spawn(SpriteBundle {
                texture: texture.clone(),
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
            .insert(UIElement::SkillChoice)
            .insert(UIState::Skills)
            .insert(Interactable::default())
            .insert(Name::new("SKILLS UI"))
            .insert(RenderLayers::from_layers(&[3]))
            .id();
        // icon
        let icon = spawn_item_stack_icon(
            &mut commands,
            &graphics,
            &ItemStack::crate_icon_stack(choice.skill.get_icon()),
            &asset_server,
            Vec2::new(0., 25.),
        );
        commands.entity(icon).set_parent(skills_e);
        let mut text_title = commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    choice.skill.get_title(),
                    TextStyle {
                        font: asset_server.load("fonts/Kitchen Sink.ttf"),
                        font_size: 8.0,
                        color: BLACK,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform {
                    translation: Vec3::new(0., 48., 1.),
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
        for (i, desc) in choice.skill.get_desc().iter().enumerate() {
            let mut text_desc = commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        desc,
                        TextStyle {
                            font: asset_server.load("fonts/Kitchen Sink.ttf"),
                            font_size: 8.0,
                            color: BLACK,
                        },
                    ),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(0., 4. - i as f32 * 9., 1.),
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
) {
    if key_input.just_pressed(KeyCode::B) {
        next_inv_state.set(UIState::Skills);
    }
}
