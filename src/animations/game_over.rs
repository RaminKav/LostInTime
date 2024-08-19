use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::seq::IteratorRandom;
use std::{
    fs::{self, File},
    io::BufReader,
};

use crate::{
    client::{analytics::SendAnalyticsDataToServerEvent, GameOverEvent},
    colors::{overwrite_alpha, WHITE},
    inputs::FacingDirection,
    player::Player,
    ui::{damage_numbers::spawn_text, Interactable, MenuButton, UIElement, UIState},
    world::{dimension::ActiveDimension, y_sort::YSort},
    DoNotDespawnOnGameOver, GameState, RawPosition, GAME_HEIGHT, GAME_WIDTH,
};

#[derive(Component)]
pub struct GameOverFadeout(Timer);

pub fn handle_game_over_fadeout(
    mut commands: Commands,
    game_over_events: EventReader<GameOverEvent>,
    mut next_state: ResMut<NextState<GameState>>,
    mut player: Query<
        (
            Entity,
            &FacingDirection,
            &mut Transform,
            &mut TextureAtlasSprite,
            &mut Handle<TextureAtlas>,
        ),
        With<Player>,
    >,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    asset_server: Res<AssetServer>,
    mut next_ui_state: ResMut<NextState<UIState>>,
) {
    if !game_over_events.is_empty() {
        let (player_e, dir, mut player_t, mut sprite, texture_atlas_handle) = player.single_mut();
        next_ui_state.set(UIState::Closed);
        // BLACK OVERLAY
        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.),
                    custom_size: Some(Vec2::new(GAME_WIDTH + 10., GAME_HEIGHT + 20.)),
                    ..default()
                },
                transform: Transform {
                    translation: Vec3::new(0., 0., 10.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Name::new("overlay"))
            .insert(GameOverFadeout(Timer::from_seconds(6.5, TimerMode::Once)));
        // GAME OVER TEXT
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Game Over",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: WHITE.with_a(0.),
                    },
                ),
                transform: Transform {
                    translation: Vec3::new(0., 80., 21.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            GameOverText,
            RenderLayers::from_layers(&[3]),
        ));
        // OK BUTTON
        let ok_text = commands
            .spawn((
                Text2dBundle {
                    text: Text::from_section(
                        "Try Again",
                        TextStyle {
                            font: asset_server.load("fonts/alagard.ttf"),
                            font_size: 15.0,
                            color: WHITE.with_a(0.),
                        },
                    ),
                    transform: Transform {
                        translation: Vec3::new(0., -100.5, 23.),
                        scale: Vec3::new(1., 1., 1.),
                        ..Default::default()
                    },
                    ..default()
                },
                Name::new("INFO OK TEXT"),
                RenderLayers::from_layers(&[3]),
                Interactable::default(),
                UIElement::MenuButton,
                GameOverText,
                MenuButton::GameOverOK,
                Sprite {
                    custom_size: Some(Vec2::new(70., 13.)),
                    ..default()
                },
            ))
            .id();
        next_state.0 = Some(GameState::GameOver);
        // move player to UI camera to be above the fade out overlay
        commands
            .entity(player_e)
            .remove::<YSort>()
            .remove::<RawPosition>()
            .insert(RenderLayers::from_layers(&[3]));
        player_t.translation = Vec3::new(0., 0., 100.);
        let texture_atlas = texture_atlases.get_mut(&texture_atlas_handle).unwrap();
        let dir_str = match dir {
            FacingDirection::Left => "side",
            FacingDirection::Right => "side",
            FacingDirection::Up => "up",
            FacingDirection::Down => "down",
        };
        // set player to death sprite
        let player_texture_handle =
            asset_server.load(format!("textures/player/player_{}_dead.png", dir_str));
        texture_atlas.texture = player_texture_handle.clone();
        if dir == &FacingDirection::Left {
            sprite.flip_x = true;
        }
    }
}
#[derive(Component)]
pub struct GameOverText;

#[derive(Resource)]
pub struct GameOverUITracker {
    pub game_over_text_check: bool,
    pub tip_check: bool,
    pub analytics_check: bool,
}
pub fn tick_game_over_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameOverFadeout, &mut Sprite), Without<GameOverText>>,
    asset_server: Res<AssetServer>,
    mut analytics_events: EventWriter<SendAnalyticsDataToServerEvent>,
    mut game_over_text: Query<&mut Text, With<GameOverText>>,
    mut analytics_check: Local<bool>,
    mut tip_check: Local<bool>,
) {
    if query.iter().count() == 0 {
        *analytics_check = false;
        *tip_check = false;
    }
    for (_e, mut timer, mut sprite) in query.iter_mut() {
        if timer.0.percent() >= 0.5 && !*analytics_check {
            analytics_events.send_default();
            *analytics_check = true;
        }
        if timer.0.percent() >= 0.25 && !*tip_check {
            *tip_check = true;
            // Try to load tips from save
            if let Ok(tips_file) = File::open("assets/tips.json") {
                let reader = BufReader::new(tips_file);

                match serde_json::from_reader::<_, Vec<String>>(reader) {
                    Ok(data) => {
                        let picked_tip = data.iter().choose(&mut rand::thread_rng()).unwrap();
                        spawn_text(
                            &mut commands,
                            &asset_server,
                            Vec3::new(0., -GAME_HEIGHT / 2. + 56.5, 21.),
                            WHITE,
                            format!("Tip: {}", picked_tip.to_string()),
                            Anchor::Center,
                            1.,
                            3,
                        );
                    }
                    Err(err) => println!("Failed to load data from file {err:?}"),
                }
            }
        }
        timer.0.tick(time.delta());

        let alpha = f32::min(1., timer.0.percent() * 5.);
        sprite.color = overwrite_alpha(sprite.color, alpha);
        if alpha >= 0.45 {
            // update text alpha
            game_over_text.iter_mut().for_each(|mut s| {
                s.sections[0].style.color = overwrite_alpha(
                    s.sections[0].style.color,
                    f32::min(1., timer.0.percent() * 2.),
                );
            });
        }
    }
}
