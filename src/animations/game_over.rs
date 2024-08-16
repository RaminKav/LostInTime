use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};
use rand::seq::IteratorRandom;
use std::{
    fs::{self, File},
    io::BufReader,
};

use crate::{
    client::{analytics::SendAnalyticsDataToServerEvent, GameOverEvent},
    colors::{overwrite_alpha, WHITE},
    container::ContainerRegistry,
    inputs::FacingDirection,
    item::CraftingTracker,
    night::NightTracker,
    player::Player,
    ui::{damage_numbers::spawn_text, ChestContainer, FurnaceContainer, UIState},
    world::{
        dimension::{ActiveDimension, EraManager},
        generation::WorldObjectCache,
        y_sort::YSort,
    },
    DoNotDespawnOnGameOver, Game, GameState, RawPosition, GAME_HEIGHT, GAME_WIDTH,
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

pub fn tick_game_over_overlay(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut GameOverFadeout, &mut Sprite)>,
    everything: Query<
        Entity,
        (
            Or<(With<Visibility>, With<ActiveDimension>)>,
            Without<DoNotDespawnOnGameOver>,
        ),
    >,
    mut next_state: ResMut<NextState<GameState>>,
    asset_server: Res<AssetServer>,
    mut analytics_events: EventWriter<SendAnalyticsDataToServerEvent>,
    mut analytics_check: Local<bool>,
    mut tip_check: Local<bool>,
) {
    for (e, mut timer, mut sprite) in query.iter_mut() {
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
                            Vec3::new(0., -GAME_HEIGHT / 2. + 40., 21.),
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
        if timer.0.finished() {
            *analytics_check = false;
            println!("Despawning everything, Sending to main menu");
            for e in everything.iter() {
                commands.entity(e).despawn();
            }
            commands.entity(e).despawn();
            let _ = fs::remove_file("save_state.json");
            next_state.0 = Some(GameState::MainMenu);
            //cleanup resources with Entity refs
            commands.remove_resource::<ChestContainer>();
            commands.remove_resource::<FurnaceContainer>();
            commands.remove_resource::<Game>();
            commands.remove_resource::<NightTracker>();
            commands.remove_resource::<ContainerRegistry>();
            commands.remove_resource::<CraftingTracker>();
            commands.remove_resource::<EraManager>();
            commands.remove_resource::<WorldObjectCache>();
        } else {
            let alpha = f32::min(1., timer.0.percent() * 5.);
            sprite.color = overwrite_alpha(sprite.color, alpha);
            if alpha >= 0.7 {
                commands.spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "Game Over",
                            TextStyle {
                                font: asset_server.load("fonts/alagard.ttf"),
                                font_size: 30.0,
                                color: WHITE.with_a(f32::min(1., timer.0.percent() * 2.)),
                            },
                        ),
                        transform: Transform {
                            translation: Vec3::new(0., 80., 21.),
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..default()
                    },
                    RenderLayers::from_layers(&[3]),
                ));
            }
        }
    }
}
