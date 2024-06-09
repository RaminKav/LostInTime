use std::fs;

use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    client::GameOverEvent,
    colors::overwrite_alpha,
    container::ContainerRegistry,
    enemy::Mob,
    item::CraftingTracker,
    night::NightTracker,
    player::Player,
    ui::{ChestContainer, FurnaceContainer},
    world::{chunk::Chunk, dimension::ActiveDimension, generation::WorldObjectCache},
    Game, GameState, GAME_HEIGHT, GAME_WIDTH,
};

#[derive(Component)]
pub struct GameOverFadeout(Timer);

pub fn handle_game_over_fadeout(
    mut commands: Commands,
    mut game_over_events: EventReader<GameOverEvent>,
    mut next_state: ResMut<NextState<GameState>>,
) {
    if game_over_events.iter().count() > 0 {
        commands
            .spawn(SpriteBundle {
                sprite: Sprite {
                    color: Color::rgba(0., 0., 0., 0.),
                    custom_size: Some(Vec2::new(GAME_WIDTH + 10., GAME_HEIGHT + 10.)),
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
            .insert(GameOverFadeout(Timer::from_seconds(2.0, TimerMode::Once)));

        next_state.0 = Some(GameState::GameOver);
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
        )>,
    >,
    mut next_state: ResMut<NextState<GameState>>,
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
            commands.remove_resource::<WorldObjectCache>();
        } else {
            println!("Setting overlay to {:?}", timer.0.percent());
            sprite.color = overwrite_alpha(sprite.color, timer.0.percent());
        }
    }
}
