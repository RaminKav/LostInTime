use bevy::camera::visibility::RenderLayers;
use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::sprite::Anchor;
use bevy::text::Justify;
use strum::IntoEnumIterator;

use super::{
    focus::FocusInput, game_fonts as gf, main_menu::spawn_exit_icon_button, ui_helpers, Focusable,
    Interactable, Interaction, MenuButton, UIElement, UIState,
};

use crate::{
    animations::enemy_sprites::spawn_looping_attack_warning_aseprite,
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{LIGHT_GREEN, WHITE},
    combat::damage_tracker::format_damage,
    cursor::CursorPos,
    inputs::MouselessModeState,
    player::achievements::{Achievement, Achievements},
    ScreenResolution,
};

#[derive(Component)]
pub struct AchievementsUI;

pub const ACHIEVEMENTS_PER_PAGE: usize = 7;

const ROW_HEIGHT: f32 = 36.0;
const ROW_GAP: f32 = 4.0;
const ROW_SPACING: f32 = ROW_HEIGHT + ROW_GAP;
const LIST_START_Y: f32 = 120.0;
const ACHIEVEMENT_BUTTON_Y: f32 = -166.0;
const ACHIEVEMENT_BUTTON_SIZE: Vec2 = Vec2::new(92., 24.);
const ACHIEVEMENT_BUTTON_GAP: f32 = 12.0;
const ACHIEVEMENT_BUTTON_STEP: f32 = ACHIEVEMENT_BUTTON_SIZE.x + ACHIEVEMENT_BUTTON_GAP;
const ACHIEVEMENTS_TITLE_Y: f32 = 160.0;
const ACHIEVEMENTS_COMPLETION_TRACKER_Y: f32 = 145.0;
const ACHIEVEMENTS_COMPLETION_TRACKER_X: f32 = 225.0;
const CONTAINER_Z: f32 = 0.0;
const ROW_BG_Z: f32 = 1.0;
const ROW_TEXT_Z: f32 = 2.0;
const BUTTON_Z: f32 = 3.0;

#[derive(Resource, Default)]
pub struct AchievementsPagination {
    pub page: usize,
}

#[derive(Component)]
pub struct AchievementRow {
    index: usize,
}

#[derive(Component)]
pub struct AchievementRowBg;

#[derive(Component)]
pub struct AchievementNameText;

#[derive(Component)]
pub struct AchievementDescText;

#[derive(Component)]
pub struct AchievementCheckbox;

#[derive(Component)]
pub struct AchievementCrossout;

#[derive(Component)]
pub struct AchievementRewardText;

#[derive(Component)]
pub struct AchievementProgressText;

#[derive(Component)]
pub struct AchievementWarningAnimation;

#[derive(Component)]
pub struct AchievementsPrevButton;

#[derive(Component)]
pub struct AchievementsNextButton;

#[derive(Component)]
pub struct AchievementsCompletionText;

fn achievements_completion_label(achievements: &Achievements) -> String {
    format!(
        "{}/{}",
        achievements.finished_count(),
        Achievement::iter().count()
    )
}

fn get_row_ui_element(row_index: usize, has_counter: bool) -> UIElement {
    let use_variant_one = row_index % 2 == 0;
    match (has_counter, use_variant_one) {
        (true, true) => UIElement::AchievementsRowWithCounter1,
        (true, false) => UIElement::AchievementsRowWithCounter2,
        (false, true) => UIElement::AchievementsRow1,
        (false, false) => UIElement::AchievementsRow2,
    }
}

fn achievement_has_counter(achievement: Achievement) -> bool {
    achievement.get_progress(None, None, None, None).is_some()
}

fn name_text_color(is_completed: bool, is_claimed: bool) -> Color {
    if is_claimed {
        Color::srgba(0.5, 0.5, 0.5, 1.0)
    } else if is_completed {
        LIGHT_GREEN
    } else {
        crate::colors::WHITE
    }
}

fn status_text_color(is_completed: bool, is_claimed: bool) -> Color {
    if is_claimed {
        Color::srgba(0.5, 0.5, 0.5, 1.0)
    } else if is_completed {
        LIGHT_GREEN
    } else {
        WHITE
    }
}

fn spawn_achievement_button(pos: Vec3, commands: &mut Commands, graphics: &Graphics) -> Entity {
    commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::AchievementButton),
                    custom_size: Some(ACHIEVEMENT_BUTTON_SIZE),
                    ..Default::default()
                },
                Transform::from_translation(pos),
            ),
            Interactable::default(),
            UIElement::AchievementButton,
            RenderLayers::from_layers(&[3]),
            Name::new("Achievement Button"),
        ))
        .id()
}

fn spawn_achievement_button_label(
    commands: &mut Commands,
    asset_server: &AssetServer,
    button: Entity,
    label: &str,
) {
    commands
        .spawn(
            gf::DISPLAY
                .text(&asset_server, label, WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., -1., 1.),
                    scale: gf::DISPLAY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(ChildOf(button));
}

pub fn setup_achievements_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    achievements: Res<Achievements>,
    mut pagination: ResMut<AchievementsPagination>,
) {
    pagination.page = 0;

    let overlay = ui_helpers::spawn_full_screen_ui_overlay(
        &mut commands,
        &resolution,
        0.95,
        ui_helpers::Z_DEPTH_MAIN_MENU_MODAL_OVERLAY,
    );
    commands.entity(overlay).insert(AchievementsUI);

    commands.spawn((
        gf::DISPLAY
            .text(
                &asset_server,
                "Achievements",
                crate::colors::DARK_WOOD_BROWN,
            )
            .justify(Justify::Center)
            .anchor(bevy::sprite::Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., ACHIEVEMENTS_TITLE_Y, 11.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        AchievementsUI,
        UIState::Achievements,
        Name::new("Achievements Title"),
    ));

    commands.spawn((
        gf::BODY
            .text(
                &asset_server,
                achievements_completion_label(&achievements),
                crate::colors::LIGHT_BROWN,
            )
            .justify(Justify::Right)
            .anchor(Anchor::CENTER_RIGHT)
            .with_transform(Transform {
                translation: Vec3::new(
                    ACHIEVEMENTS_COMPLETION_TRACKER_X,
                    ACHIEVEMENTS_COMPLETION_TRACKER_Y,
                    11.,
                ),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        AchievementsUI,
        UIState::Achievements,
        AchievementsCompletionText,
        Name::new("Achievements Completion Tracker"),
    ));

    let achievements_bg = commands
        .spawn((
            (
                Transform::from_translation(Vec3::new(0., 0., 10.)),
                Visibility::default(),
            ),
            UIState::Achievements,
            AchievementsUI,
            RenderLayers::from_layers(&[3]),
            Name::new("ACHIEVEMENTS UI"),
        ))
        .id();

    commands
        .spawn((
            Sprite {
                image: graphics
                    .get_ui_element_texture(UIElement::AchievementsContainer)
                    .clone(),
                custom_size: Some(Vec2::new(482., 340.)),
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(0., 0., CONTAINER_Z)),
        ))
        .insert(AchievementsUI)
        .insert(UIState::Achievements)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Achievements Container"))
        .insert(ChildOf(achievements_bg));

    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    all_achievements.sort_by_key(|achievement| {
        let is_completed = achievements.is_completed(*achievement);
        let is_claimed = achievements.is_claimed(*achievement);

        if is_completed && !is_claimed {
            0
        } else if !is_completed && !is_claimed {
            1
        } else {
            2
        }
    });
    let total_pages = if all_achievements.is_empty() {
        0
    } else {
        (all_achievements.len() + ACHIEVEMENTS_PER_PAGE - 1) / ACHIEVEMENTS_PER_PAGE
    };

    for row_index in 0..ACHIEVEMENTS_PER_PAGE {
        let y_pos = LIST_START_Y - (row_index as f32 * ROW_SPACING);
        let achievement_index = pagination.page * ACHIEVEMENTS_PER_PAGE + row_index;
        let maybe_achievement = all_achievements.get(achievement_index);

        let has_counter = maybe_achievement
            .copied()
            .is_some_and(achievement_has_counter);
        let row_ui_element = get_row_ui_element(row_index, has_counter);

        let (name_text, desc_text, name_color, desc_color, checkbox_texture, crossout_visible) =
            if let Some(achievement) = maybe_achievement {
                let is_completed = achievements.is_completed(*achievement);
                let is_claimed = achievements.is_claimed(*achievement);
                (
                    achievement.get_name(),
                    achievement.get_desc(),
                    name_text_color(is_completed, is_claimed),
                    status_text_color(is_completed, is_claimed),
                    if is_claimed {
                        UIElement::CheckBoxSelected
                    } else {
                        UIElement::CheckBox
                    },
                    is_claimed,
                )
            } else {
                (
                    String::new(),
                    String::new(),
                    WHITE,
                    WHITE,
                    UIElement::CheckBox,
                    false,
                )
            };

        let row_visible = maybe_achievement.is_some();

        let row_bg = commands
            .spawn((
                (
                    Sprite {
                        image: graphics.get_ui_element_texture(row_ui_element.clone()),
                        custom_size: Some(Vec2::new(434., ROW_HEIGHT)),
                        ..Default::default()
                    },
                    Transform::from_translation(Vec3::new(0., y_pos, ROW_BG_Z)),
                    if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ),
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                row_ui_element,
                AchievementRow { index: row_index },
                AchievementRowBg,
                Interactable::default(),
                Focusable {
                    group: UIState::Achievements,
                    index: row_index as u32,
                },
                Name::new("Achievement Row Background"),
            ))
            .id();
        commands.entity(row_bg).insert(ChildOf(achievements_bg));

        let name_pos = Vec3::new(-134., y_pos, ROW_TEXT_Z);
        let name_entity = commands
            .spawn((
                (
                    gf::ACHIEVEMENT_NAME
                        .text(&asset_server, name_text, name_color)
                        .justify(Justify::Left)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: name_pos,
                            scale: gf::ACHIEVEMENT_NAME.transform_scale(),
                            ..Default::default()
                        }),
                    if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ),
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementNameText,
                Name::new("Achievement Name"),
            ))
            .id();

        let desc_entity = commands
            .spawn((
                gf::BODY
                    .text(&asset_server, desc_text, desc_color)
                    .anchor(Anchor::CENTER_LEFT)
                    .with_transform(Transform {
                        translation: Vec3::new(-52., y_pos, ROW_TEXT_Z),
                        scale: gf::BODY.transform_scale(),
                        ..Default::default()
                    }),
                if row_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
            ))
            .insert(RenderLayers::from_layers(&[3]))
            .insert((
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementDescText,
                Name::new("Achievement Description"),
            ))
            .id();

        commands
            .entity(desc_entity)
            .insert(ChildOf(achievements_bg));
        commands
            .entity(name_entity)
            .insert(ChildOf(achievements_bg));

        let reward_amount = maybe_achievement
            .copied()
            .map(|achievement| achievement.reward_currency())
            .unwrap_or(0);
        let reward_text = if reward_amount > 0 {
            reward_amount.to_string()
        } else {
            String::new()
        };

        let reward_entity = commands
            .spawn((
                (
                    gf::BODY
                        .text(&asset_server, reward_text, desc_color)
                        .justify(Justify::Center)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(194., y_pos - 9., ROW_TEXT_Z),
                            scale: gf::BODY.transform_scale(),
                            ..Default::default()
                        }),
                    if row_visible && reward_amount > 0 {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ),
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementRewardText,
                Name::new("Achievement Reward Text2d"),
            ))
            .id();
        commands
            .entity(reward_entity)
            .insert(ChildOf(achievements_bg));

        let progress_text_entity = commands
            .spawn((
                (
                    gf::BODY
                        .text(&asset_server, "", WHITE)
                        .justify(Justify::Right)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(135., y_pos - 1., ROW_TEXT_Z),
                            scale: gf::BODY.transform_scale(),
                            ..Default::default()
                        }),
                    if row_visible && has_counter {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ),
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementProgressText,
                Name::new("Achievement Progress Text2d"),
            ))
            .id();
        commands
            .entity(progress_text_entity)
            .insert(ChildOf(achievements_bg));

        let checkbox_entity = commands
            .spawn((
                (
                    Sprite {
                        image: graphics.get_ui_element_texture(checkbox_texture.clone()),
                        custom_size: Some(Vec2::new(16., 16.)),
                        ..Default::default()
                    },
                    Transform::from_translation(Vec3::new(-68.5, 0., 1.) + name_pos),
                    if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                ),
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                AchievementCheckbox,
                AchievementRow { index: row_index },
                Name::new("Achievement Checkbox"),
            ))
            .id();

        commands
            .entity(checkbox_entity)
            .insert(ChildOf(achievements_bg));

        if let Some(achievement) = maybe_achievement {
            let is_completed = achievements.is_completed(*achievement);
            let is_claimed = achievements.is_claimed(*achievement);
            if is_completed && !is_claimed {
                let warning_pos = Vec3::new(-225., y_pos, 20.5);
                let warning_entity = spawn_looping_attack_warning_aseprite(
                    &mut commands,
                    &asset_server,
                    warning_pos,
                    achievements_bg,
                );
                commands.entity(warning_entity).insert((
                    AchievementsUI,
                    UIState::Achievements,
                    AchievementRow { index: row_index },
                    AchievementWarningAnimation,
                    RenderLayers::from_layers(&[3]),
                    Name::new("Achievement Warning Animation"),
                ));
            }
        }
    }

    let prev_button = spawn_achievement_button(
        Vec3::new(-ACHIEVEMENT_BUTTON_STEP, ACHIEVEMENT_BUTTON_Y, BUTTON_Z),
        &mut commands,
        &graphics,
    );
    commands
        .entity(prev_button)
        .insert((
            AchievementsUI,
            UIState::Achievements,
            MenuButton::AchievementsPrev,
            AchievementsPrevButton,
            Focusable {
                group: UIState::Achievements,
                index: 90,
            },
            Name::new("Achievements Prev Button"),
            Visibility::Hidden,
        ))
        .insert(ChildOf(achievements_bg));
    spawn_achievement_button_label(&mut commands, &asset_server, prev_button, "Prev");

    let next_button = spawn_achievement_button(
        Vec3::new(ACHIEVEMENT_BUTTON_STEP, ACHIEVEMENT_BUTTON_Y, BUTTON_Z),
        &mut commands,
        &graphics,
    );
    commands
        .entity(next_button)
        .insert((
            AchievementsUI,
            UIState::Achievements,
            MenuButton::AchievementsNext,
            AchievementsNextButton,
            Focusable {
                group: UIState::Achievements,
                index: 91,
            },
            Name::new("Achievements Next Button"),
            if pagination.page + 1 < total_pages {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ))
        .insert(ChildOf(achievements_bg));
    spawn_achievement_button_label(&mut commands, &asset_server, next_button, "Next");

    let exit_button = spawn_exit_icon_button(
        Vec3::new(0., ACHIEVEMENT_BUTTON_Y + 2., BUTTON_Z),
        &mut commands,
        &graphics,
    );
    commands
        .entity(exit_button)
        .insert((
            AchievementsUI,
            UIState::Achievements,
            Focusable {
                group: UIState::Achievements,
                index: 100,
            },
            Name::new("Achievements Exit Button"),
        ))
        .insert(ChildOf(achievements_bg));
}

pub fn cleanup_achievements_ui(
    mut commands: Commands,
    achievements_ui: Query<Entity, With<AchievementsUI>>,
) {
    for entity in achievements_ui.iter() {
        commands.entity(entity).despawn();
    }
}

pub fn update_achievements_page_display(
    mut commands: Commands,
    achievements: Res<Achievements>,
    pagination: Res<AchievementsPagination>,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    analytics: Option<Res<crate::client::analytics::AnalyticsData>>,
    bounce_tracker: Option<Res<crate::player::achievements::BounceAchievementTracker>>,
    meteor_shower_state: Query<&crate::player::skills::MeteorShowerSkillState, With<crate::Player>>,
    mut completion_text: Query<
        &mut Text2d,
        (
            With<AchievementsCompletionText>,
            Without<AchievementNameText>,
            Without<AchievementDescText>,
            Without<AchievementRewardText>,
            Without<AchievementProgressText>,
        ),
    >,
    game_data: Option<Res<crate::client::GameData>>,
    achievements_bg_query: Query<Entity, (With<AchievementsUI>, With<UIState>)>,
    warning_animations: Query<(Entity, &AchievementRow), With<AchievementWarningAnimation>>,
    mut param_set: ParamSet<(
        Query<
            (
                &AchievementRow,
                &mut Text2d,
                &mut TextColor,
                &mut Visibility,
            ),
            With<AchievementNameText>,
        >,
        Query<
            (
                &AchievementRow,
                &mut Text2d,
                &mut TextColor,
                &mut Visibility,
            ),
            With<AchievementDescText>,
        >,
        Query<(&AchievementRow, &mut Sprite, &mut Visibility), With<AchievementCheckbox>>,
        Query<(&AchievementRow, &mut Visibility), With<AchievementCrossout>>,
        Query<
            (
                &AchievementRow,
                &mut Text2d,
                &mut TextColor,
                &mut Visibility,
            ),
            With<AchievementRewardText>,
        >,
        Query<
            (
                &AchievementRow,
                &mut Text2d,
                &mut TextColor,
                &mut Visibility,
            ),
            With<AchievementProgressText>,
        >,
        Query<
            (
                &AchievementRow,
                &mut Sprite,
                &mut UIElement,
                &mut Visibility,
                &Interactable,
            ),
            With<AchievementRowBg>,
        >,
    )>,
) {
    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    all_achievements.sort_by_key(|achievement| {
        let is_completed = achievements.is_completed(*achievement);
        let is_claimed = achievements.is_claimed(*achievement);

        if is_completed && !is_claimed {
            0
        } else if !is_completed && !is_claimed {
            1
        } else {
            2
        }
    });
    let start_index = pagination.page * ACHIEVEMENTS_PER_PAGE;

    let mut row_states = Vec::with_capacity(ACHIEVEMENTS_PER_PAGE);

    if let Ok(mut text) = completion_text.single_mut() {
        text.0 = achievements_completion_label(&achievements);
    }

    for offset in 0..ACHIEVEMENTS_PER_PAGE {
        let maybe_achievement = all_achievements.get(start_index + offset).copied();
        row_states.push(maybe_achievement.map(|achievement| {
            let is_completed = achievements.is_completed(achievement);
            let is_claimed = achievements.is_claimed(achievement);
            let cumulative_analytics = game_data
                .as_ref()
                .and_then(|gd| gd.cumulative_analytics.as_ref());
            let current_run_analytics = analytics.as_deref();
            let meteor_state = meteor_shower_state.single().ok();
            let progress = achievement.get_progress(
                cumulative_analytics,
                current_run_analytics,
                bounce_tracker.as_deref(),
                meteor_state,
            );
            (achievement, is_completed, is_claimed, progress)
        }));
    }

    {
        let mut name_query = param_set.p0();
        for (row, mut text, mut text_color, mut visibility) in name_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, _)) => {
                    text.0 = achievement.get_name();
                    text_color.0 = name_text_color(is_completed, is_claimed);
                    *visibility = Visibility::Visible;
                }
                None => {
                    text.0.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut desc_query = param_set.p1();
        for (row, mut text, mut text_color, mut visibility) in desc_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, _)) => {
                    text.0 = achievement.get_desc();
                    text_color.0 = status_text_color(is_completed, is_claimed);
                    *visibility = Visibility::Visible;
                }
                None => {
                    text.0.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut checkbox_query = param_set.p2();
        for (row, mut texture, mut visibility) in checkbox_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((_, _, is_claimed, _)) => {
                    let checkbox_type = if is_claimed {
                        UIElement::CheckBoxSelected
                    } else {
                        UIElement::CheckBox
                    };
                    texture.image = graphics.get_ui_element_texture(checkbox_type).clone();
                    *visibility = Visibility::Visible;
                }
                None => {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut crossout_query = param_set.p3();
        for (row, mut visibility) in crossout_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((_, _, is_claimed, _)) => {
                    *visibility = if is_claimed {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    };
                }
                None => {
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut reward_query = param_set.p4();
        for (row, mut text, mut text_color, mut visibility) in reward_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, _)) => {
                    if is_claimed {
                        text.0.clear();
                        *visibility = Visibility::Hidden;
                    } else {
                        let reward = achievement.reward_currency();
                        if reward > 0 {
                            text.0 = reward.to_string();
                            text_color.0 = status_text_color(is_completed, is_claimed);
                            *visibility = Visibility::Visible;
                        } else {
                            text.0.clear();
                            *visibility = Visibility::Hidden;
                        }
                    }
                }
                None => {
                    text.0.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut progress_text_query = param_set.p5();
        for (row, mut text, mut text_color, mut visibility) in progress_text_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, Some((current, target)))) => {
                    if is_claimed {
                        text.0.clear();
                        *visibility = Visibility::Hidden;
                    } else {
                        text.0 = format!(
                            "{}/{}",
                            format_damage(current as i64),
                            format_damage(target as i64)
                        );
                        text_color.0 = status_text_color(is_completed, is_claimed);
                        *visibility = Visibility::Visible;
                    }
                }
                Some((_, is_completed, is_claimed, None)) => {
                    text.0.clear();
                    text_color.0 = status_text_color(is_completed, is_claimed);
                    *visibility = Visibility::Hidden;
                }
                None => {
                    text.0.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut row_bg_query = param_set.p6();
        for (row, mut texture, mut ui_element, mut visibility, interactable) in
            row_bg_query.iter_mut()
        {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, _, _, _)) => {
                    let has_counter = achievement_has_counter(achievement);
                    let base = get_row_ui_element(row.index, has_counter);
                    let element = if matches!(interactable.current(), Interaction::Hovering) {
                        base.get_hover_state().unwrap_or(base)
                    } else {
                        base
                    };
                    texture.image = graphics.get_ui_element_texture(element.clone());
                    *ui_element = element;
                    *visibility = Visibility::Visible;
                }
                None => {
                    let base = get_row_ui_element(row.index, false);
                    texture.image = graphics.get_ui_element_texture(base.clone());
                    *ui_element = base;
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let existing_warnings: Vec<(Entity, usize)> = warning_animations
            .iter()
            .map(|(entity, row)| (entity, row.index))
            .collect();

        let achievements_bg = achievements_bg_query.iter().next();

        for (entity, row_index) in existing_warnings.iter() {
            let should_have_warning = row_states
                .get(*row_index)
                .and_then(|state| state.as_ref())
                .map(|(_, is_completed, is_claimed, _)| *is_completed && !*is_claimed)
                .unwrap_or(false);

            if !should_have_warning {
                commands.entity(*entity).despawn();
            }
        }

        if let Some(bg_entity) = achievements_bg {
            let existing_indices: Vec<usize> =
                existing_warnings.iter().map(|(_, idx)| *idx).collect();
            for (row_index, row_state) in row_states.iter().enumerate() {
                if existing_indices.contains(&row_index) {
                    continue;
                }

                if let Some((_, is_completed, is_claimed, _)) = row_state {
                    if *is_completed && !*is_claimed {
                        let y_pos = LIST_START_Y - (row_index as f32 * ROW_SPACING);
                        let warning_pos = Vec3::new(-165., y_pos, 20.5);
                        let warning_entity = spawn_looping_attack_warning_aseprite(
                            &mut commands,
                            &asset_server,
                            warning_pos,
                            bg_entity,
                        );
                        commands.entity(warning_entity).insert((
                            AchievementsUI,
                            UIState::Achievements,
                            AchievementRow { index: row_index },
                            AchievementWarningAnimation,
                            RenderLayers::from_layers(&[3]),
                            Name::new("Achievement Warning Animation"),
                        ));
                    }
                }
            }
        }
    }
}

pub fn update_achievements_navigation_buttons(
    pagination: Res<AchievementsPagination>,
    mut pagination_buttons: Query<(
        &mut Visibility,
        &mut Interactable,
        Option<&AchievementsPrevButton>,
        Option<&AchievementsNextButton>,
    )>,
) {
    if !pagination.is_changed() {
        return;
    }

    let total_achievements = Achievement::iter().count();
    let total_pages = if total_achievements == 0 {
        0
    } else {
        (total_achievements + ACHIEVEMENTS_PER_PAGE - 1) / ACHIEVEMENTS_PER_PAGE
    };

    for (mut visibility, mut interactable, prev_button, next_button) in
        pagination_buttons.iter_mut()
    {
        if prev_button.is_some() {
            if pagination.page > 0 && total_pages > 0 {
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                }
            }
        }
        if next_button.is_some() {
            if pagination.page + 1 < total_pages {
                *visibility = Visibility::Visible;
            } else {
                *visibility = Visibility::Hidden;
                if matches!(interactable.current(), Interaction::Hovering) {
                    interactable.change(Interaction::None);
                }
            }
        }
    }
}

pub fn handle_achievement_row_hover(
    cursor_pos: Res<CursorPos>,
    mouseless: Res<MouselessModeState>,
    focus_input: FocusInput,
    graphics: Res<Graphics>,
    computed_visibility: Query<&ViewVisibility>,
    mut row_queries: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
        Query<(Entity, &mut Interactable, &mut UIElement, &mut Sprite), With<AchievementRowBg>>,
    )>,
    mut commands: Commands,
) {
    let hit_entity = {
        let q = row_queries.p0();
        ui_helpers::pointcast_2d(&cursor_pos, &q, None, Some(&computed_visibility)).map(|h| h.0)
    };
    let focus_driving = mouseless.0 || cursor_pos.suppress_ui_hover;

    for (entity, mut interactable, mut ui_element, mut sprite) in row_queries.p1().iter_mut() {
        if computed_visibility
            .get(entity)
            .ok()
            .is_some_and(|visibility| !visibility.get())
        {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
                if let Some(normal) = ui_element.get_normal_state() {
                    sprite.image = graphics.get_ui_element_texture(normal.clone());
                    *ui_element = normal;
                }
            }
            continue;
        }

        let is_hit = hit_entity == Some(entity);
        let is_focused = focus_driving && focus_input.is_focused(entity);
        if is_hit || is_focused {
            if matches!(interactable.current(), Interaction::None) {
                interactable.change(Interaction::Hovering);
                if let Some(hover) = ui_element.get_hover_state() {
                    sprite.image = graphics.get_ui_element_texture(hover.clone());
                    *ui_element = hover;
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            if let Some(normal) = ui_element.get_normal_state() {
                sprite.image = graphics.get_ui_element_texture(normal.clone());
                *ui_element = normal;
            }
        }
    }
}

pub fn handle_achievement_row_clicks(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut achievements: ResMut<Achievements>,
    mut currency: ResMut<crate::player::TimeFragmentCurrency>,
    mut commands: Commands,
    achievement_rows: Query<(Entity, &AchievementRow), With<Interactable>>,
    pagination: Res<AchievementsPagination>,
    focus_input: FocusInput,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);
    let confirm_pressed = focus_input.confirm_just_pressed();

    if !left_mouse_released && !confirm_pressed {
        return;
    }

    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    all_achievements.sort_by_key(|achievement| {
        let is_completed = achievements.is_completed(*achievement);
        let is_claimed = achievements.is_claimed(*achievement);

        if is_completed && !is_claimed {
            0
        } else if !is_completed && !is_claimed {
            1
        } else {
            2
        }
    });

    for (entity, row) in achievement_rows.iter() {
        let is_hit = left_mouse_released && matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = confirm_pressed && focus_input.is_focused(entity);
        if is_hit || is_focused {
            let achievement_index = pagination.page * ACHIEVEMENTS_PER_PAGE + row.index;
            if let Some(achievement) = all_achievements.get(achievement_index).copied() {
                if achievements.is_completed(achievement) {
                    if achievements.claim(achievement) {
                        crate::player::achievements::persist_achievements_state(&*achievements);

                        let reward = achievement.reward_currency();
                        if reward > 0 {
                            currency.time_fragments =
                                currency.time_fragments.saturating_add(reward as i32);
                            crate::player::unlocks::persist_unlock_data(
                                Some(&*currency),
                                None,
                                None,
                                None,
                                None,
                            );
                        }
                        commands.spawn(crate::audio::SoundSpawner::new(
                            crate::audio::AudioSoundEffect::ButtonClick,
                            0.2,
                        ));
                        return;
                    }
                }
            }
        }
    }
}
