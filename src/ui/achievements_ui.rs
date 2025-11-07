use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use strum::IntoEnumIterator;

use crate::{
    assets::Graphics,
    player::achievements::{Achievement, Achievements},
    ui::{damage_numbers::spawn_text, spawn_back_button, ui_helpers, UIElement, UIState},
    ScreenResolution,
};

#[derive(Component)]
pub struct AchievementsUI;

#[derive(Component)]

pub struct BackButton;

pub fn setup_achievements_ui(
    mut commands: Commands,
    graphics: Res<Graphics>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    achievements: Res<Achievements>,
) {
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

    // List all achievements
    let all_achievements: Vec<Achievement> = Achievement::iter().collect();
    let start_y = 92.5;
    let row_spacing = 23.0;

    for (i, achievement) in all_achievements.iter().enumerate() {
        let is_unlocked = achievements.has(*achievement);
        let y_pos = start_y - (i as f32 * row_spacing);

        let desc_text_color = if is_unlocked {
            Color::rgba(0.5, 0.5, 0.5, 1.0)
        } else {
            crate::colors::BLACK
        };
        let text_color = if is_unlocked {
            Color::rgba(0.5, 0.5, 0.5, 1.0)
        } else {
            crate::colors::LIGHT_BROWN
        };

        let achievement_name_text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        achievement.get_name(),
                        TextStyle {
                            font: asset_server.load("fonts/alagard.ttf"),
                            font_size: 15.0,
                            color: text_color,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: bevy::sprite::Anchor::CenterLeft,
                    transform: Transform::from_translation(Vec3::new(-147., y_pos, 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                UIState::Achievements,
                Name::new("Achievements desc"),
            ))
            .id();

        let achievement_desc_text = spawn_text(
            &mut commands,
            &asset_server,
            Vec3::new(-21., y_pos, 1.),
            desc_text_color,
            achievement.get_desc(),
            bevy::sprite::Anchor::CenterLeft,
            1.,
            3,
        );
        commands
            .entity(achievement_desc_text)
            .set_parent(achievements_bg);
        commands
            .entity(achievement_name_text)
            .set_parent(achievements_bg);
        // Back Button (parent sprite + child text)
        info!("Is unlocked: {}", is_unlocked);
        let checkbox_type = if is_unlocked {
            UIElement::CheckBoxSelected
        } else {
            UIElement::CheckBox
        };
        let _checkbox = commands
            .spawn((
                SpriteBundle {
                    texture: graphics.get_ui_element_texture(checkbox_type.clone()),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(9., 9.)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(-8.5, 0., 1.)),
                    ..Default::default()
                },
                checkbox_type,
                RenderLayers::from_layers(&[3]),
                AchievementsUI,
                Name::new("Back Button"),
            ))
            .set_parent(achievement_name_text)
            .id();
        if is_unlocked {
            commands
                .spawn((
                    SpriteBundle {
                        texture: graphics.get_ui_element_texture(UIElement::AchievementCrossOut),
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(289., 7.)),
                            ..Default::default()
                        },
                        transform: Transform::from_translation(Vec3::new(142., 0., 1.)),
                        ..Default::default()
                    },
                    UIElement::AchievementCrossOut,
                    RenderLayers::from_layers(&[3]),
                    AchievementsUI,
                    Name::new("Back Button"),
                ))
                .set_parent(achievement_name_text);
        }
    }

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
