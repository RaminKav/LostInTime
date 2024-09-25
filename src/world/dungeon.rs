use bevy::{prelude::*, render::view::RenderLayers};

use crate::{world::dimension::Dimension, GameState, GAME_HEIGHT};

use super::{
    dimension::{ActiveDimension, DimensionSpawnEvent, EraManager},
    dungeon_generation::{add_dungeon_chests, add_dungeon_exit_block},
    TileMapPosition,
};

#[derive(Component)]
pub struct Dungeon {
    pub grid: Vec<Vec<i8>>,
}
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems((
            add_dungeon_chests,
            tick_dungeon_timer.run_if(in_state(GameState::Main)),
            add_dungeon_exit_block,
            spawn_dungeon_text,
        ));
    }
}

#[derive(Component)]
pub struct Dungeontimer(pub Timer);

#[derive(Component)]
pub struct DungeonText;

#[derive(Component, Default)]
pub struct CachedPlayerPos(pub TileMapPosition);

fn tick_dungeon_timer(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<&mut Dungeontimer, With<Dimension>>,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    mut text_query: Query<(Entity, &mut Text), With<DungeonText>>,
    era: Res<EraManager>,
) {
    for mut timer in query.iter_mut() {
        timer.0.tick(time.delta());
        if let Ok(mut text) = text_query.get_single_mut() {
            text.1.sections[0].value = format!(
                "Time Left: {}:{}",
                timer.0.remaining().as_secs() / 60,
                timer.0.remaining().as_secs() % 60
            );
        }
        if timer.0.just_finished() {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(era.current_era.clone()),
            });
            commands.entity(text_query.single_mut().0).despawn();
        }
    }
}

pub fn spawn_dungeon_text(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    new_dungeon: Query<Entity, (Added<ActiveDimension>, With<Dungeon>)>,
) {
    for _dim_e in new_dungeon.iter() {
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Time Left: 3:00",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: Color::Rgba {
                            red: 75. / 255.,
                            green: 61. / 255.,
                            blue: 68. / 255.,
                            alpha: 1.,
                        },
                    },
                )
                .with_alignment(TextAlignment::Center),
                transform: Transform {
                    translation: Vec3::new(0., GAME_HEIGHT / 2. - 12., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
                ..default()
            },
            Name::new("FPS TEXT"),
            DungeonText,
            RenderLayers::from_layers(&[3]),
        ));
    }
}
