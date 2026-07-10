use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use strum::IntoEnumIterator;

use super::{
    focus::FocusInput, game_fonts as gf, main_menu::spawn_exit_icon_button, ui_helpers, Focusable,
    Interactable, Interaction, MenuButton, UIElement, UIState,
};

use crate::{
    animations::enemy_sprites::spawn_attack_warning_aseprite,
    assets::Graphics,
    colors::{LIGHT_GREEN, WHITE},
    combat::damage_tracker::format_damage,
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
const ROW_HITBOX_Z: f32 = 1.5;
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

fn row_texture_path(row_index: usize, has_counter: bool) -> &'static str {
    let use_variant_one = row_index % 2 == 0;
    match (has_counter, use_variant_one) {
        (true, true) => "ui/AchievementsRowWithCounter1.png",
        (true, false) => "ui/AchievementsRowWithCounter2.png",
        (false, true) => "ui/AchievementsRow1.png",
        (false, false) => "ui/AchievementsRow2.png",
    }
}

fn load_row_texture(
    asset_server: &AssetServer,
    row_index: usize,
    has_counter: bool,
) -> Handle<Image> {
    asset_server.load(row_texture_path(row_index, has_counter))
}

fn achievement_has_counter(achievement: Achievement) -> bool {
    achievement.get_progress(None, None, None, None).is_some()
}

fn name_text_color(is_completed: bool, is_claimed: bool) -> Color {
    if is_claimed {
        Color::rgba(0.5, 0.5, 0.5, 1.0)
    } else if is_completed {
        LIGHT_GREEN
    } else {
        crate::colors::WHITE
    }
}

fn status_text_color(is_completed: bool, is_claimed: bool) -> Color {
    if is_claimed {
        Color::rgba(0.5, 0.5, 0.5, 1.0)
    } else if is_completed {
        LIGHT_GREEN
    } else {
        WHITE
    }
}

fn spawn_achievement_button(pos: Vec3, commands: &mut Commands, graphics: &Graphics) -> Entity {
    commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::AchievementButton),
                sprite: Sprite {
                    custom_size: Some(ACHIEVEMENT_BUTTON_SIZE),
                    ..Default::default()
                },
                transform: Transform::from_translation(pos),
                ..Default::default()
            },
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
        .spawn(Text2dBundle {
            text: Text::from_section(label, gf::DISPLAY.text_style(&asset_server, WHITE))
                .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., -1., 1.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(button);
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
        Text2dBundle {
            text: Text::from_section(
                "Achievements",
                gf::DISPLAY.text_style(&asset_server, crate::colors::DARK_WOOD_BROWN),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::Center,
            transform: Transform {
                translation: Vec3::new(0., ACHIEVEMENTS_TITLE_Y, 11.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        AchievementsUI,
        UIState::Achievements,
        Name::new("Achievements Title"),
    ));

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                achievements_completion_label(&achievements),
                gf::BODY.text_style(&asset_server, crate::colors::LIGHT_BROWN),
            )
            .with_alignment(TextAlignment::Right),
            text_anchor: Anchor::CenterRight,
            transform: Transform {
                translation: Vec3::new(
                    ACHIEVEMENTS_COMPLETION_TRACKER_X,
                    ACHIEVEMENTS_COMPLETION_TRACKER_Y,
                    11.,
                ),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        AchievementsUI,
        UIState::Achievements,
        AchievementsCompletionText,
        Name::new("Achievements Completion Tracker"),
    ));

    let achievements_bg = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 10.)),
                ..Default::default()
            },
            UIState::Achievements,
            AchievementsUI,
            RenderLayers::from_layers(&[3]),
            Name::new("ACHIEVEMENTS UI"),
        ))
        .id();

    commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::AchievementsContainer)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(482., 340.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., CONTAINER_Z)),
            ..Default::default()
        })
        .insert(AchievementsUI)
        .insert(UIState::Achievements)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Achievements Container"))
        .set_parent(achievements_bg);

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
                SpriteBundle {
                    texture: load_row_texture(&asset_server, row_index, has_counter),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(434., ROW_HEIGHT)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(0., y_pos, ROW_BG_Z)),
                    visibility: if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                row_ui_element,
                AchievementRow { index: row_index },
                AchievementRowBg,
                Name::new("Achievement Row Background"),
            ))
            .id();
        commands.entity(row_bg).set_parent(achievements_bg);

        let row_clickable = commands
            .spawn((
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(434., ROW_HEIGHT)),
                        color: Color::rgba(0., 0., 0., 0.0),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(0., y_pos, ROW_HITBOX_Z)),
                    visibility: if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                Interactable::default(),
                Focusable {
                    group: UIState::Achievements,
                    index: row_index as u32,
                },
                Name::new("Achievement Row Clickable"),
            ))
            .id();
        commands.entity(row_clickable).set_parent(achievements_bg);
        let name_pos = Vec3::new(-134., y_pos, ROW_TEXT_Z);
        let name_entity = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        name_text,
                        gf::ACHIEVEMENT_NAME.text_style(&asset_server, name_color),
                    )
                    .with_alignment(TextAlignment::Left),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: name_pos,
                        scale: gf::ACHIEVEMENT_NAME.transform_scale(),
                        ..Default::default()
                    },
                    visibility: if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementNameText,
                Name::new("Achievement Name"),
            ))
            .id();

        let desc_entity = commands
            .spawn(Text2dBundle {
                text: Text::from_section(desc_text, gf::BODY.text_style(&asset_server, desc_color)),
                transform: Transform {
                    translation: Vec3::new(-52., y_pos, ROW_TEXT_Z),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                },
                text_anchor: Anchor::CenterLeft,
                visibility: if row_visible {
                    Visibility::Visible
                } else {
                    Visibility::Hidden
                },
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert((
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementDescText,
                Name::new("Achievement Description"),
            ))
            .id();

        commands.entity(desc_entity).set_parent(achievements_bg);
        commands.entity(name_entity).set_parent(achievements_bg);

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
                Text2dBundle {
                    text: Text::from_section(
                        reward_text,
                        gf::BODY.text_style(&asset_server, desc_color),
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(194., y_pos - 9., ROW_TEXT_Z),
                        scale: gf::BODY.transform_scale(),
                        ..Default::default()
                    },
                    visibility: if row_visible && reward_amount > 0 {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementRewardText,
                Name::new("Achievement Reward Text"),
            ))
            .id();
        commands.entity(reward_entity).set_parent(achievements_bg);

        let progress_text_entity = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section("", gf::BODY.text_style(&asset_server, WHITE))
                        .with_alignment(TextAlignment::Right),
                    text_anchor: Anchor::Center,
                    transform: Transform {
                        translation: Vec3::new(135., y_pos - 1., ROW_TEXT_Z),
                        scale: gf::BODY.transform_scale(),
                        ..Default::default()
                    },
                    visibility: if row_visible && has_counter {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementProgressText,
                Name::new("Achievement Progress Text"),
            ))
            .id();
        commands
            .entity(progress_text_entity)
            .set_parent(achievements_bg);

        let checkbox_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(checkbox_texture.clone()),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(16., 16.)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(-68.5, 0., 1.) + name_pos),
                    visibility: if row_visible {
                        Visibility::Visible
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                AchievementCheckbox,
                AchievementRow { index: row_index },
                Name::new("Achievement Checkbox"),
            ))
            .id();

        commands.entity(checkbox_entity).set_parent(achievements_bg);

        if let Some(achievement) = maybe_achievement {
            let is_completed = achievements.is_completed(*achievement);
            let is_claimed = achievements.is_claimed(*achievement);
            if is_completed && !is_claimed {
                let warning_pos = Vec3::new(-225., y_pos, 20.5);
                let warning_entity = spawn_attack_warning_aseprite(
                    &mut commands,
                    &asset_server,
                    warning_pos,
                    achievements_bg,
                    999999.0,
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
        .set_parent(achievements_bg);
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
        .set_parent(achievements_bg);
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
        .set_parent(achievements_bg);
}

pub fn cleanup_achievements_ui(
    mut commands: Commands,
    achievements_ui: Query<Entity, With<AchievementsUI>>,
) {
    for entity in achievements_ui.iter() {
        commands.entity(entity).despawn_recursive();
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
        &mut Text,
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
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementNameText>>,
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementDescText>>,
        Query<(&AchievementRow, &mut Handle<Image>, &mut Visibility), With<AchievementCheckbox>>,
        Query<(&AchievementRow, &mut Visibility), With<AchievementCrossout>>,
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementRewardText>>,
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementProgressText>>,
        Query<
            (
                &AchievementRow,
                &mut Handle<Image>,
                &mut UIElement,
                &mut Visibility,
            ),
            With<AchievementRowBg>,
        >,
        Query<
            (&AchievementRow, &mut Visibility),
            (
                With<Interactable>,
                Without<AchievementRowBg>,
                Without<AchievementNameText>,
                Without<AchievementDescText>,
                Without<AchievementCheckbox>,
                Without<AchievementCrossout>,
                Without<AchievementRewardText>,
                Without<AchievementProgressText>,
            ),
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

    if let Ok(mut text) = completion_text.get_single_mut() {
        text.sections[0].value = achievements_completion_label(&achievements);
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
            let meteor_state = meteor_shower_state.get_single().ok();
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
        for (row, mut text, mut visibility) in name_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, _)) => {
                    text.sections[0].value = achievement.get_name();
                    text.sections[0].style.color = name_text_color(is_completed, is_claimed);
                    *visibility = Visibility::Visible;
                }
                None => {
                    text.sections[0].value.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut desc_query = param_set.p1();
        for (row, mut text, mut visibility) in desc_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, _)) => {
                    text.sections[0].value = achievement.get_desc();
                    text.sections[0].style.color = status_text_color(is_completed, is_claimed);
                    *visibility = Visibility::Visible;
                }
                None => {
                    text.sections[0].value.clear();
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
                    *texture = graphics.get_ui_element_texture(checkbox_type).clone();
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
        for (row, mut text, mut visibility) in reward_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, _)) => {
                    if is_claimed {
                        text.sections[0].value.clear();
                        *visibility = Visibility::Hidden;
                    } else {
                        let reward = achievement.reward_currency();
                        if reward > 0 {
                            text.sections[0].value = reward.to_string();
                            text.sections[0].style.color =
                                status_text_color(is_completed, is_claimed);
                            *visibility = Visibility::Visible;
                        } else {
                            text.sections[0].value.clear();
                            *visibility = Visibility::Hidden;
                        }
                    }
                }
                None => {
                    text.sections[0].value.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut progress_text_query = param_set.p5();
        for (row, mut text, mut visibility) in progress_text_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, is_completed, is_claimed, Some((current, target)))) => {
                    if is_claimed {
                        text.sections[0].value.clear();
                        *visibility = Visibility::Hidden;
                    } else {
                        text.sections[0].value = format!(
                            "{}/{}",
                            format_damage(current as i64),
                            format_damage(target as i64)
                        );
                        text.sections[0].style.color = status_text_color(is_completed, is_claimed);
                        *visibility = Visibility::Visible;
                    }
                }
                Some((_, is_completed, is_claimed, None)) => {
                    text.sections[0].value.clear();
                    text.sections[0].style.color = status_text_color(is_completed, is_claimed);
                    *visibility = Visibility::Hidden;
                }
                None => {
                    text.sections[0].value.clear();
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut row_bg_query = param_set.p6();
        for (row, mut texture, mut ui_element, mut visibility) in row_bg_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, _, _, _)) => {
                    let has_counter = achievement_has_counter(achievement);
                    *ui_element = get_row_ui_element(row.index, has_counter);
                    *texture = load_row_texture(&asset_server, row.index, has_counter);
                    *visibility = Visibility::Visible;
                }
                None => {
                    *ui_element = get_row_ui_element(row.index, false);
                    *texture = load_row_texture(&asset_server, row.index, false);
                    *visibility = Visibility::Hidden;
                }
            }
        }
    }

    {
        let mut row_clickable_query = param_set.p7();
        for (row, mut visibility) in row_clickable_query.iter_mut() {
            *visibility = if row_states.get(row.index).and_then(|state| *state).is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
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
                commands.entity(*entity).despawn_recursive();
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
                        let warning_entity = spawn_attack_warning_aseprite(
                            &mut commands,
                            &asset_server,
                            warning_pos,
                            bg_entity,
                            999999.0,
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

pub fn handle_achievement_row_clicks(
    cursor_pos: Res<crate::cursor::CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
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
