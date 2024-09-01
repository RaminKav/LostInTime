use bevy::{prelude::*, render::view::RenderLayers};
use bevy_proto::prelude::ProtoCommands;

use crate::{
    player::MovePlayerEvent,
    world::dimension::{Dimension, SpawnDimension},
    GameParam, GameState, GAME_HEIGHT,
};

use super::{
    dimension::{ActiveDimension, DimensionSpawnEvent, EraManager},
    dungeon_generation::{
        add_dungeon_chests, add_dungeon_exit_block, gen_new_dungeon, get_player_spawn_tile, Bias,
    },
    generation::WorldObjectCache,
    world_helpers::world_pos_to_tile_pos,
    TileMapPosition, CHUNK_SIZE,
};

#[derive(Component)]
pub struct Dungeon {
    pub grid: Vec<Vec<i8>>,
}
pub struct DungeonPlugin;
impl Plugin for DungeonPlugin {
    fn build(&self, app: &mut App) {
        app.add_system(handle_move_player_after_dungeon_gen)
            .add_systems((
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

pub fn spawn_new_dungeon_dimension(
    game: &mut GameParam,
    commands: &mut Commands,
    proto_commands: &mut ProtoCommands,
    move_player_event: &mut EventWriter<MovePlayerEvent>,
) {
    game.clear_dungeon_cache();
    let player = game.player_query.single().0;
    let player_pos = game.player().position;
    commands
        .entity(player)
        .insert(CachedPlayerPos(world_pos_to_tile_pos(
            player_pos.truncate(),
        )));
    let grid = gen_new_dungeon(
        3000,
        (CHUNK_SIZE * 4 * 2) as usize,
        Bias {
            bias: super::dungeon_generation::Direction::Left,
            strength: 0,
        },
    );

    let dim_e = commands
        .spawn((
            Dimension,
            Dungeon { grid: grid.clone() },
            Dungeontimer(Timer::from_seconds(360., TimerMode::Once)),
        ))
        .id();
    proto_commands.apply("DungeonWorldGenerationParams");

    //cache era data
    let curr_era = game.era.current_era.clone();
    game.era
        .era_generation_cache
        .insert(curr_era, game.world_obj_cache.clone());

    commands.remove_resource::<WorldObjectCache>();
    commands.insert_resource(WorldObjectCache::default());
    commands.entity(dim_e).insert(SpawnDimension);

    if let Some(pos) = get_player_spawn_tile(grid.clone()) {
        move_player_event.send(MovePlayerEvent { pos });
    }
}
fn handle_move_player_after_dungeon_gen(
    _new_dungeon: Query<&Dungeon, Added<ActiveDimension>>,
    _move_player_event: EventWriter<MovePlayerEvent>,
) {
    // if let Ok(dungeon) = new_dungeon.get_single() {
    //     let grid = &dungeon.grid;
    //     if let Some(pos) = get_player_spawn_tile(grid.clone()) {
    //         move_player_event.send(MovePlayerEvent { pos });
    //     }
    // }
}

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
                .with_justify(JustifyText::Center),
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
