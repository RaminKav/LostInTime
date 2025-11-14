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
                Vec2::new(-4., 0.),
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
    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    all_achievements.sort_by_key(|achievement| !achievements.has(*achievement));
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
                let is_unlocked = achievements.has(*achievement);
                (
                    achievement.get_name(),
                    achievement.get_desc(),
                    if is_unlocked {
                        Color::rgba(0.5, 0.5, 0.5, 1.0)
                    } else {
                        crate::colors::LIGHT_BROWN
                    },
                    if is_unlocked {
                        Color::rgba(0.5, 0.5, 0.5, 1.0)
                    } else {
                        crate::colors::BLACK
                    },
                    if is_unlocked {
                        UIElement::CheckBoxSelected
                    } else {
                        UIElement::CheckBox
                    },
                    is_unlocked,
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

        let desc_entity = spawn_text(
            &mut commands,
            &asset_server,
            Vec3::new(-70., y_pos, 1.),
            desc_color,
            desc_text,
            bevy::sprite::Anchor::CenterLeft,
            1.,
            3,
        );

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
        spawn_back_button_texture_only(Vec3::new(-90., -108., 1.), &mut commands, &graphics);
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
        spawn_back_button_texture_only(Vec3::new(90., -108., 1.), &mut commands, &graphics);
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
    mut param_set: ParamSet<(
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementNameText>>,
        Query<(&AchievementRow, &mut Text, &mut Visibility), With<AchievementDescText>>,
        Query<(&AchievementRow, &mut Handle<Image>, &mut Visibility), With<AchievementCheckbox>>,
        Query<(&AchievementRow, &mut Visibility), With<AchievementCrossout>>,
        Query<(Entity, &AchievementRow), With<AchievementRewardRoot>>,
    )>,
) {
    if !pagination.is_changed() && !achievements.is_changed() {
        return;
    }

    let mut all_achievements: Vec<Achievement> = Achievement::iter().collect();
    all_achievements.sort_by_key(|achievement| !achievements.has(*achievement));
    let start_index = pagination.page * ACHIEVEMENTS_PER_PAGE;

    let mut row_states = Vec::with_capacity(ACHIEVEMENTS_PER_PAGE);
    for offset in 0..ACHIEVEMENTS_PER_PAGE {
        let maybe_achievement = all_achievements.get(start_index + offset).copied();
        row_states.push(maybe_achievement.map(|achievement| {
            let unlocked = achievements.has(achievement);
            (achievement, unlocked)
        }));
    }

    {
        let mut name_query = param_set.p0();
        for (row, mut text, mut visibility) in name_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, unlocked)) => {
                    text.sections[0].value = achievement.get_name();
                    text.sections[0].style.color = if unlocked {
                        Color::rgba(0.5, 0.5, 0.5, 1.0)
                    } else {
                        crate::colors::LIGHT_BROWN
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
        let mut desc_query = param_set.p1();
        for (row, mut text, mut visibility) in desc_query.iter_mut() {
            match row_states.get(row.index).and_then(|state| *state) {
                Some((achievement, unlocked)) => {
                    text.sections[0].value = achievement.get_desc();
                    text.sections[0].style.color = if unlocked {
                        Color::rgba(0.5, 0.5, 0.5, 1.0)
                    } else {
                        crate::colors::BLACK
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
                Some((_, unlocked)) => {
                    let checkbox_type = if unlocked {
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
                Some((_achievement, unlocked)) => {
                    *visibility = if unlocked {
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
            let reward_amount = row_states
                .get(row.index)
                .and_then(|state| state.map(|(achievement, _)| achievement.reward_currency()));
            refresh_reward_icon(
                &mut commands,
                &graphics,
                &asset_server,
                entity,
                reward_amount,
            );
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
