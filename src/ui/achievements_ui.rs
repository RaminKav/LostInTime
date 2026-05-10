use bevy::ecs::system::ParamSet;
use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use strum::IntoEnumIterator;

use super::{
    damage_numbers::spawn_text, spawn_back_button, spawn_back_button_texture_only,
    spawn_item_stack_icon, ui_helpers, Interactable, Interaction, MenuButton, UIElement, UIState,
};

use crate::{
    animations::enemy_sprites::spawn_attack_warning_aseprite,
    assets::Graphics,
    inventory::ItemStack,
    item::WorldObject,
    player::achievements::{Achievement, Achievements},
    ScreenResolution,
};

#[derive(Component)]
pub struct AchievementsUI;

pub const ACHIEVEMENTS_PER_PAGE: usize = 10;

#[derive(Resource, Default)]
pub struct AchievementsPagination {
    pub page: usize,
}

#[derive(Component)]
pub struct AchievementRow {
    index: usize,
}

#[derive(Component)]
pub struct AchievementNameText;

#[derive(Component)]
pub struct AchievementDescText;

#[derive(Component)]
pub struct AchievementCheckbox;

#[derive(Component)]
pub struct AchievementCrossout;

#[derive(Component)]
pub struct AchievementRewardRoot;

#[derive(Component)]
pub struct AchievementProgressText;

#[derive(Component)]
pub struct AchievementProgressBar;

#[derive(Component)]
pub struct AchievementProgressBarBg;

#[derive(Component)]
pub struct AchievementWarningAnimation;

#[derive(Component)]
pub struct AchievementsPrevButton;

#[derive(Component)]
pub struct AchievementsNextButton;

fn refresh_reward_icon(
    commands: &mut Commands,
    graphics: &Graphics,
    asset_server: &AssetServer,
    root: Entity,
    reward: Option<u32>,
) {
    commands.entity(root).despawn_descendants();

    match reward {
        Some(amount) if amount > 0 => {
            commands.entity(root).insert(Visibility::Visible);
            let mut stack = ItemStack::crate_icon_stack(WorldObject::TimeFragment);
            stack.count = amount as usize;
            let icon = spawn_item_stack_icon(
                commands,
                graphics,
                &stack,
                asset_server,
                Vec2::ZERO,
                Vec2::new(0., 0.),
                3,
            );
            commands.entity(icon).set_parent(root);
        }
        _ => {
            commands.entity(root).insert(Visibility::Hidden);
        }
    }
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

    // Spawn overlay
    let overlay = ui_helpers::spawn_ui_overlay(
        &mut commands,
        Vec2::new(resolution.game_width, resolution.game_height),
        0.9,
        0.,
    );
    commands.entity(overlay).insert(AchievementsUI);

    // Title
    // commands.spawn((
    //     Text2dBundle {
    //         text: Text::from_section(
    //             "Achievements",
    //             TextStyle {
    //                 font: asset_server.load("fonts/alagard.ttf"),
    //                 font_size: 30.0,
    //                 color: crate::colors::YELLOW_2,
    //             },
    //         )
    //         .with_alignment(TextAlignment::Center),
    //         text_anchor: bevy::sprite::Anchor::Center,
    //         transform: Transform::from_translation(Vec3::new(0., 80., 11.)),
    //         ..Default::default()
    //     },
    //     RenderLayers::from_layers(&[3]),
    //     AchievementsUI,
    //     UIState::Achievements,
    //     Name::new("Achievements Title"),
    // ));
    let achievements_bg = commands
        .spawn(SpriteBundle {
            texture: graphics
                .get_ui_element_texture(UIElement::Achievements)
                .clone(),
            sprite: Sprite {
                custom_size: Some(Vec2::new(351.5, 231.5)),
                ..Default::default()
            },
            transform: Transform {
                translation: Vec3::new(0., 0., 10.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..Default::default()
        })
        .insert(UIState::Achievements)
        .insert(AchievementsUI)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("ACHIEVEMENTS UI"))
        .id();

    // List all achievements (paginated)
    // Sort order: Completed (unclaimed) -> Unfinished -> Claimed
    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    all_achievements.sort_by_key(|achievement| {
        let is_completed = achievements.is_completed(*achievement);
        let is_claimed = achievements.is_claimed(*achievement);

        if is_completed && !is_claimed {
            0 // Completed and ready to claim - highest priority
        } else if !is_completed && !is_claimed {
            1 // Unfinished - medium priority
        } else {
            2 // Claimed - lowest priority
        }
    });
    let total_pages = if all_achievements.is_empty() {
        0
    } else {
        (all_achievements.len() + ACHIEVEMENTS_PER_PAGE - 1) / ACHIEVEMENTS_PER_PAGE
    };
    let start_y = 93.5;
    let row_spacing = 19.0;
    let font_handle = asset_server.load("fonts/4x5.ttf");

    for row_index in 0..ACHIEVEMENTS_PER_PAGE {
        let y_pos = start_y - (row_index as f32 * row_spacing);
        let achievement_index = pagination.page * ACHIEVEMENTS_PER_PAGE + row_index;
        let maybe_achievement = all_achievements.get(achievement_index);

        let (name_text, desc_text, text_color, desc_color, checkbox_texture, crossout_visible) =
            if let Some(achievement) = maybe_achievement {
                let is_completed = achievements.is_completed(*achievement);
                let is_claimed = achievements.is_claimed(*achievement);
                (
                    achievement.get_name(),
                    achievement.get_desc(),
                    if is_claimed {
                        Color::rgba(0.5, 0.5, 0.5, 1.0) // Gray for claimed
                    } else if is_completed {
                        crate::colors::LIGHT_GREEN // Green for completed
                    } else {
                        crate::colors::LIGHT_BROWN // Normal color
                    },
                    if is_claimed {
                        Color::rgba(0.5, 0.5, 0.5, 1.0) // Gray for claimed
                    } else if is_completed {
                        crate::colors::LIGHT_GREEN // Green for completed
                    } else {
                        crate::colors::BLACK // Normal color
                    },
                    if is_claimed {
                        UIElement::CheckBoxSelected
                    } else {
                        UIElement::CheckBox
                    },
                    is_claimed, // Only show crossout for claimed
                )
            } else {
                (
                    "".to_string(),
                    "".to_string(),
                    crate::colors::LIGHT_BROWN,
                    crate::colors::BLACK,
                    UIElement::CheckBox,
                    false,
                )
            };

        let name_bundle = Text2dBundle {
            text: Text::from_section(
                name_text,
                TextStyle {
                    font: font_handle.clone(),
                    font_size: 5.0,
                    color: text_color,
                },
            )
            .with_alignment(TextAlignment::Left),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(-148., y_pos, 1.)),
            visibility: if maybe_achievement.is_some() {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
            ..Default::default()
        };

        // Create a clickable area for the row
        let row_clickable = commands
            .spawn((
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(289., 19.)),
                        color: Color::rgba(0., 0., 0., 0.0), // Almost transparent but still hittable
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(0., y_pos, 0.5)),
                    visibility: if maybe_achievement.is_some() {
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
                Name::new("Achievement Row Clickable"),
            ))
            .id();
        commands.entity(row_clickable).set_parent(achievements_bg);

        let name_entity = commands
            .spawn((
                name_bundle,
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
                text: Text::from_section(
                    desc_text,
                    TextStyle {
                        font: asset_server.load("fonts/slkscr.ttf"),
                        font_size: 8.5,
                        color: desc_color,
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(-70., y_pos, 1.),
                    ..Default::default()
                },
                text_anchor: Anchor::CenterLeft,
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .id();

        commands.entity(desc_entity).insert((
            AchievementsUI,
            UIState::Achievements,
            AchievementRow { index: row_index },
            AchievementDescText,
            Name::new("Achievement Description"),
        ));

        if maybe_achievement.is_none() {
            commands.entity(desc_entity).insert(Visibility::Hidden);
        }

        commands.entity(desc_entity).set_parent(achievements_bg);
        commands.entity(name_entity).set_parent(achievements_bg);

        let reward_root = commands
            .spawn((
                SpatialBundle {
                    transform: Transform::from_translation(Vec3::new(140., y_pos, 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementRewardRoot,
                Name::new("Achievement Reward Root"),
            ))
            .id();
        commands.entity(reward_root).set_parent(achievements_bg);

        let reward_amount = maybe_achievement
            .copied()
            .map(|achievement| achievement.reward_currency());
        refresh_reward_icon(
            &mut commands,
            &graphics,
            &asset_server,
            reward_root,
            reward_amount,
        );

        // Progress text
        let progress_text_entity = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "",
                        TextStyle {
                            font: font_handle.clone(),
                            font_size: 5.0,
                            color: crate::colors::BLACK,
                        },
                    )
                    .with_alignment(TextAlignment::Right),
                    text_anchor: bevy::sprite::Anchor::CenterRight,
                    transform: Transform::from_translation(Vec3::new(132., y_pos, 1.)),
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

        // Progress bar background
        let progress_bar_bg = commands
            .spawn((
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(0., 2.)),
                        color: Color::rgba(0.3, 0.3, 0.3, 1.0),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(120., y_pos - 6., 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementProgressBarBg,
                Name::new("Achievement Progress Bar BG"),
            ))
            .id();
        commands.entity(progress_bar_bg).set_parent(achievements_bg);

        // Progress bar fill
        let progress_bar_fill = commands
            .spawn((
                SpriteBundle {
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(0., 2.)),
                        color: crate::colors::LIGHT_GREEN,
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(120., y_pos - 6., 1.1)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                AchievementRow { index: row_index },
                AchievementProgressBar,
                Name::new("Achievement Progress Bar Fill"),
            ))
            .id();
        commands
            .entity(progress_bar_fill)
            .set_parent(achievements_bg);

        let checkbox_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(checkbox_texture.clone()),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(9., 9.)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(-8.5, 0., 1.)),
                    visibility: if maybe_achievement.is_some() {
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

        commands.entity(checkbox_entity).set_parent(name_entity);

        // Spawn warning animation for completed but not claimed achievements
        if let Some(achievement) = maybe_achievement {
            let is_completed = achievements.is_completed(*achievement);
            let is_claimed = achievements.is_claimed(*achievement);
            if is_completed && !is_claimed {
                // Spawn warning animation on the far left, left of the checkbox
                // Checkbox is at -8.5 relative to name_entity, name_entity is at -148
                // So checkbox is at ~-156.5, we'll put warning at ~-165
                info!(
                    "SPAWNED WARNING ANIMATION FOR ACHIEVEMENT ROW {}",
                    row_index
                );
                let warning_pos = Vec3::new(-165., y_pos, 20.5);
                let warning_entity = spawn_attack_warning_aseprite(
                    &mut commands,
                    &asset_server,
                    warning_pos,
                    achievements_bg,
                    999999.0, // Very long duration so it persists
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

        let crossout_visibility = if crossout_visible {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        let crossout_entity = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(UIElement::AchievementCrossOut),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(289., 7.)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(142., 0., 1.)),
                    visibility: if maybe_achievement.is_some() {
                        crossout_visibility
                    } else {
                        Visibility::Hidden
                    },
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                AchievementCrossout,
                AchievementRow { index: row_index },
                Name::new("Achievement Crossout"),
            ))
            .id();

        commands.entity(crossout_entity).set_parent(name_entity);
    }

    let prev_button =
        spawn_back_button_texture_only(Vec3::new(-90.5, -108., 1.), &mut commands, &graphics);
    commands
        .entity(prev_button)
        .insert((
            AchievementsUI,
            UIState::Achievements,
            MenuButton::AchievementsPrev,
            AchievementsPrevButton,
            Name::new("Achievements Prev Button"),
            Visibility::Hidden,
        ))
        .set_parent(achievements_bg);
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Prev",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(prev_button);

    let next_button =
        spawn_back_button_texture_only(Vec3::new(90.5, -108., 1.), &mut commands, &graphics);
    commands
        .entity(next_button)
        .insert((
            AchievementsUI,
            UIState::Achievements,
            MenuButton::AchievementsNext,
            AchievementsNextButton,
            Name::new("Achievements Next Button"),
            if pagination.page + 1 < total_pages {
                Visibility::Visible
            } else {
                Visibility::Hidden
            },
        ))
        .set_parent(achievements_bg);
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Next",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .set_parent(next_button);

    // Back Button (parent sprite + child text)
    let back_button_e = spawn_back_button(
        Vec3::new(0.5, -108., 1.),
        &mut commands,
        &graphics,
        &asset_server,
    );

    commands
        .entity(back_button_e)
        .insert(AchievementsUI)
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
    game_data: Option<Res<crate::client::GameData>>,
    achievements_bg_query: Query<Entity, (With<AchievementsUI>, With<UIState>)>,
    warning_animations: Query<(Entity, &AchievementRow), With<AchievementWarningAnimation>>,
    mut param_set: ParamSet<(
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementNameText>>,
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementDescText>>,
        Query<(&AchievementRow, &mut Handle<Image>, &mut Visibility), With<AchievementCheckbox>>,
        Query<(&AchievementRow, &mut Visibility), With<AchievementCrossout>>,
        Query<(Entity, &AchievementRow), With<AchievementRewardRoot>>,
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementProgressText>>,
        Query<
            (
                &AchievementRow,
                &mut Sprite,
                &mut Transform,
                &mut Visibility,
            ),
            With<AchievementProgressBar>,
        >,
        Query<(&AchievementRow, &mut Sprite), With<AchievementProgressBarBg>>,
    )>,
) {
    // Always run to ensure progress displays are updated
    // The early return was preventing the system from running when entities were first created
    // We'll let it run every frame when in achievements UI to ensure progress is always up to date

    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    // Sort achievements by priority:
    // 0. Completed but not claimed (highest priority - these need attention!)
    // 1. Not completed (second priority)
    // 2. Completed and claimed (lowest priority)
    all_achievements.sort_by_key(|achievement| {
        let is_completed = achievements.is_completed(*achievement);
        let is_claimed = achievements.is_claimed(*achievement);

        if is_completed && !is_claimed {
            0 // Completed and ready to claim
        } else if !is_completed && !is_claimed {
            1 // Unfinished
        } else {
            2 // Claimed
        }
    });
    let start_index = pagination.page * ACHIEVEMENTS_PER_PAGE;

    let mut row_states = Vec::with_capacity(ACHIEVEMENTS_PER_PAGE);

    for offset in 0..ACHIEVEMENTS_PER_PAGE {
        let maybe_achievement = all_achievements.get(start_index + offset).copied();
        row_states.push(maybe_achievement.map(|achievement| {
            let is_completed = achievements.is_completed(achievement);
            let is_claimed = achievements.is_claimed(achievement);
            // Get both cumulative and current run analytics separately
            let cumulative_analytics = game_data
                .as_ref()
                .and_then(|gd| gd.cumulative_analytics.as_ref());
            let current_run_analytics = analytics.as_deref();
            let progress = achievement.get_progress(
                cumulative_analytics,
                current_run_analytics,
                bounce_tracker.as_deref(),
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
                    text.sections[0].style.color = if is_claimed {
                        Color::rgba(0.5, 0.5, 0.5, 1.0) // Gray for claimed
                    } else if is_completed {
                        crate::colors::LIGHT_GREEN // Green for completed
                    } else {
                        crate::colors::LIGHT_BROWN // Normal color
                    };
                    *visibility = Visibility::Visible;
                }
                None => {
                    text.sections[0].value = String::new();
                    *visibility = Visibility::Visible; // Keep visible but empty
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
                    text.sections[0].style.color = if is_claimed {
                        Color::rgba(0.5, 0.5, 0.5, 1.0) // Gray for claimed
                    } else if is_completed {
                        crate::colors::LIGHT_GREEN // Green for completed
                    } else {
                        crate::colors::BLACK // Normal color
                    };
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
        let reward_query = param_set.p4();
        for (entity, row) in reward_query.iter() {
            let reward_amount = row_states.get(row.index).and_then(|state| {
                state.and_then(|(achievement, _, is_claimed, _)| {
                    // Hide reward if achievement is claimed
                    if is_claimed {
                        None
                    } else {
                        Some(achievement.reward_currency())
                    }
                })
            });
            refresh_reward_icon(
                &mut commands,
                &graphics,
                &asset_server,
                entity,
                reward_amount,
            );
        }
    }

    {
        let mut progress_text_query = param_set.p5();
        for (row, mut text, mut visibility) in progress_text_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((_, _, is_claimed, Some((current, target)))) => {
                    if is_claimed {
                        // Hide progress text when claimed
                        text.sections[0].value = String::new();
                        *visibility = Visibility::Hidden;
                    } else {
                        text.sections[0].value = format!("{}/{}", current, target);
                        *visibility = Visibility::Visible;
                    }
                }
                Some((_, _, is_claimed, None)) => {
                    if is_claimed {
                        // Hide progress text when claimed
                        text.sections[0].value = String::new();
                        *visibility = Visibility::Hidden;
                    } else {
                        text.sections[0].value = String::new();
                        *visibility = Visibility::Visible; // Keep visible but empty
                    }
                }
                None => {
                    text.sections[0].value = String::new();
                    *visibility = Visibility::Visible; // Keep visible but empty
                }
            }
        }
    }

    {
        let mut progress_bar_query = param_set.p6();
        for (row, mut sprite, mut transform, mut visibility) in progress_bar_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((_, _, is_claimed, Some((current, target)))) => {
                    if is_claimed {
                        // Hide progress bar when claimed
                        sprite.custom_size = Some(Vec2::new(0., 2.));
                        transform.translation.x = 120.0;
                        *visibility = Visibility::Hidden;
                    } else {
                        let progress = (current as f32 / target as f32).min(1.0);
                        let bar_width = 24.0 * progress;
                        sprite.custom_size = Some(Vec2::new(bar_width, 2.));
                        transform.translation.x = 120.0 - (24.0 - bar_width) / 2.0; // Align left edge with background
                        *visibility = Visibility::Visible;
                    }
                }
                Some((_, _, is_claimed, None)) => {
                    if is_claimed {
                        // Hide progress bar when claimed
                        sprite.custom_size = Some(Vec2::new(0., 2.));
                        transform.translation.x = 120.0;
                        *visibility = Visibility::Hidden;
                    } else {
                        // Set to 0 width instead of hiding
                        sprite.custom_size = Some(Vec2::new(0., 2.));
                        transform.translation.x = 120.0;
                        *visibility = Visibility::Visible; // Keep visible but 0 width
                    }
                }
                None => {
                    // Set to 0 width instead of hiding
                    sprite.custom_size = Some(Vec2::new(0., 2.));
                    transform.translation.x = 120.0;
                    *visibility = Visibility::Visible; // Keep visible but 0 width
                }
            }
        }
    }

    {
        let mut progress_bar_bg_query = param_set.p7();
        for (row, mut sprite) in progress_bar_bg_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((_, _, is_claimed, Some(_))) => {
                    if is_claimed {
                        // Hide progress bar background when claimed
                        sprite.custom_size = Some(Vec2::new(0., 2.));
                    } else {
                        // Show full width background when there's progress
                        sprite.custom_size = Some(Vec2::new(24., 2.));
                    }
                }
                Some((_, _, is_claimed, None)) => {
                    if is_claimed {
                        // Hide progress bar background when claimed
                        sprite.custom_size = Some(Vec2::new(0., 2.));
                    } else {
                        // Keep at 0 width when no progress
                        sprite.custom_size = Some(Vec2::new(0., 2.));
                    }
                }
                None => {
                    // Keep at 0 width when no progress
                    sprite.custom_size = Some(Vec2::new(0., 2.));
                }
            }
        }
    }

    {
        // Update warning animations - spawn/despawn based on completion/claim status
        let existing_warnings: Vec<(Entity, usize)> = warning_animations
            .iter()
            .map(|(entity, row)| (entity, row.index))
            .collect();

        // Get achievements_bg entity for parenting
        let achievements_bg = achievements_bg_query.iter().next();

        // Constants for positioning
        const START_Y: f32 = 93.5;
        const ROW_SPACING: f32 = 19.0;

        // Despawn warnings for rows that no longer need them
        for (entity, row_index) in existing_warnings.iter() {
            let should_have_warning = row_states
                .get(*row_index)
                .and_then(|state| state.as_ref())
                .map(|(_, is_completed, is_claimed, _)| *is_completed && !*is_claimed)
                .unwrap_or(false);

            if !should_have_warning {
                info!(
                    "Despawning warning animation for achievement row {}",
                    row_index
                );
                commands.entity(*entity).despawn_recursive();
            }
        }

        // Spawn warnings for rows that need them but don't have them
        if let Some(bg_entity) = achievements_bg {
            let existing_indices: Vec<usize> =
                existing_warnings.iter().map(|(_, idx)| *idx).collect();
            for (row_index, row_state) in row_states.iter().enumerate() {
                if existing_indices.contains(&row_index) {
                    continue; // Already has warning
                }

                if let Some((_, is_completed, is_claimed, _)) = row_state {
                    if *is_completed && !*is_claimed {
                        // Spawn warning animation
                        info!(
                            "Spawning warning animation for achievement row {}",
                            row_index
                        );
                        let y_pos = START_Y - (row_index as f32 * ROW_SPACING);
                        let warning_pos = Vec3::new(-165., y_pos, 20.5);
                        let warning_entity = spawn_attack_warning_aseprite(
                            &mut commands,
                            &asset_server,
                            warning_pos,
                            bg_entity,
                            999999.0, // Very long duration so it persists
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
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    if !left_mouse_released {
        return;
    }
    info!("CLICKED MOUSE - Checking achievement rows");

    // Get all achievements sorted using the same logic as update_achievements_page_display
    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    // Sort achievements by priority (same as in setup_achievements_ui):
    // 0. Completed but not claimed (highest priority - these need attention!)
    // 1. Not completed (second priority)
    // 2. Completed and claimed (lowest priority)
    all_achievements.sort_by_key(|achievement| {
        let is_completed = achievements.is_completed(*achievement);
        let is_claimed = achievements.is_claimed(*achievement);

        if is_completed && !is_claimed {
            0 // Completed and ready to claim
        } else if !is_completed && !is_claimed {
            1 // Unfinished
        } else {
            2 // Claimed
        }
    });

    for (entity, row) in achievement_rows.iter() {
        if let Some(hit) = hit_test {
            if hit.0 == entity {
                let achievement_index = pagination.page * ACHIEVEMENTS_PER_PAGE + row.index;
                info!("Clicked achievement row index: {}", achievement_index);
                if let Some(achievement) = all_achievements.get(achievement_index).copied() {
                    // Check if this achievement is completed but not claimed
                    if achievements.is_completed(achievement) {
                        // Claim the reward
                        if achievements.claim(achievement) {
                            // Persist achievements state immediately
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
                            // Refresh the UI
                            return;
                        }
                    }
                }
            }
        }
    }
}
