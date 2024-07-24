use std::fs;

use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    client::GameOverEvent,
    colors::{overwrite_alpha, WHITE},
    container::ContainerRegistry,
    enemy::Mob,
    inputs::FacingDirection,
    item::CraftingTracker,
    night::NightTracker,
    player::Player,
    ui::{screen_effects::HealthScreenEffect, ChestContainer, FurnaceContainer},
    world::{
        chunk::Chunk,
        dimension::{ActiveDimension, EraManager},
        generation::WorldObjectCache,
        y_sort::YSort,
    },
    Game, GameState, RawPosition, GAME_HEIGHT, GAME_WIDTH,
};

#[derive(Component)]
pub struct GameOverFadeout(Timer);

pub fn handle_game_over_fadeout(
    mut commands: Commands,
    mut game_over_events: EventReader<GameOverEvent>,
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
) {
    if game_over_events.iter().count() > 0 {
        let (player_e, dir, mut player_t, mut sprite, texture_atlas_handle) = player.single_mut();
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
            .insert(GameOverFadeout(Timer::from_seconds(5.0, TimerMode::Once)));

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
        Or<(
            With<Mob>,
            With<Chunk>,
            With<Sprite>,
            With<Player>,
            With<Text>,
            With<ActiveDimension>,
            With<HealthScreenEffect>,
            With<YSort>,
        )>,
    >,
    mut next_state: ResMut<NextState<GameState>>,
    asset_server: Res<AssetServer>,
) {
    for (e, mut timer, mut sprite) in query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            println!("Despawning everything, Sending to main menu");
            for e in everything.iter() {
                commands.entity(e).despawn_recursive();
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
            println!("Setting overlay to {:?}", timer.0.percent());
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
