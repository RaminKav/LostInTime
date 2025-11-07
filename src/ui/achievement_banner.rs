use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;

use crate::{
    player::achievements::{Achievement, AchievementUnlockedEvent},
    DEBUG,
};

const BANNER_SLIDE_DURATION: f32 = 0.3;

const BANNER_DISPLAY_DURATION: f32 = 2.0;

const BANNER_WIDTH: f32 = 160.0;

const BANNER_HEIGHT: f32 = 42.0;

#[derive(Component)]

pub struct AchievementBanner {
    start: Vec3,
    target: Vec3,
    state: BannerState,
    timer: Timer,
}

#[derive(Clone, Copy, PartialEq, Eq)]

pub enum BannerState {
    SlidingIn,
    Display,
    SlidingOut,
}

pub fn handle_achievement_banner_events(
    mut commands: Commands,
    mut events: EventReader<AchievementUnlockedEvent>,
    asset_server: Res<AssetServer>,
    res: Res<crate::ScreenResolution>,
) {
    for event in events.iter() {
        let start = Vec3::new(res.game_width / 2. - 70., res.game_height / 2. + 16., 20.);
        let target = Vec3::new(
            res.game_width / 2. - 70.,
            start.y - BANNER_HEIGHT / 2. - 20.,
            20.,
        );
        let banner_entity = commands
            .spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgba(0.08, 0.08, 0.08, 0.93),
                        custom_size: Some(Vec2::new(BANNER_WIDTH, BANNER_HEIGHT)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(start),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                AchievementBanner {
                    start,
                    target,
                    state: BannerState::SlidingIn,
                    timer: Timer::from_seconds(BANNER_SLIDE_DURATION, TimerMode::Once),
                },
                Name::new("Achievement Banner"),
            ))
            .id();

        let title_text = format!("{}", event.achievement.get_name());
        let font = asset_server.load("fonts/4x5.ttf");
        let body_font = asset_server.load("fonts/alagard.ttf");

        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "Achievement!",
                    TextStyle {
                        font: body_font,
                        font_size: 15.0,
                        color: Color::WHITE,
                    },
                ),
                text_anchor: Anchor::TopLeft,
                transform: Transform::from_translation(Vec3::new(-74., 18., 1.0)),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(banner_entity);

        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    title_text,
                    TextStyle {
                        font: font.clone(),
                        font_size: 10.0,
                        color: Color::WHITE,
                    },
                ),
                text_anchor: Anchor::TopLeft,
                transform: Transform::from_translation(Vec3::new(-74., -4., 1.0)),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .set_parent(banner_entity);
    }
}

pub fn update_achievement_banners(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(Entity, &mut AchievementBanner, &mut Transform)>,
) {
    for (entity, mut banner, mut transform) in query.iter_mut() {
        banner.timer.tick(time.delta());
        match banner.state {
            BannerState::SlidingIn => {
                let t = banner.timer.elapsed_secs() / banner.timer.duration().as_secs_f32();
                transform.translation = banner.start.lerp(banner.target, t.clamp(0.0, 1.0));
                if banner.timer.finished() {
                    banner.state = BannerState::Display;
                    banner.timer = Timer::from_seconds(BANNER_DISPLAY_DURATION, TimerMode::Once);
                    transform.translation = banner.target;
                }
            }
            BannerState::Display => {
                transform.translation = banner.target;
                if banner.timer.finished() {
                    banner.state = BannerState::SlidingOut;
                    banner.timer = Timer::from_seconds(BANNER_SLIDE_DURATION, TimerMode::Once);
                }
            }
            BannerState::SlidingOut => {
                let t = banner.timer.elapsed_secs() / banner.timer.duration().as_secs_f32();
                transform.translation = banner.target.lerp(banner.start, t.clamp(0.0, 1.0));
                if banner.timer.finished() {
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
    }
}

pub fn debug_trigger_achievement_banner(
    key_input: Res<Input<KeyCode>>,
    mut events: EventWriter<AchievementUnlockedEvent>,
) {
    if *DEBUG && key_input.just_pressed(KeyCode::Q) {
        events.send(AchievementUnlockedEvent {
            achievement: Achievement::Bouncy,
            reward_currency: 0,
        });
    }
}
