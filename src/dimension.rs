use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::TilemapId;

use crate::{
    enemy::Enemy,
    item::{Equipment, WorldObject},
    world_generation::ChunkManager,
    GameState,
};

#[derive(Component, Debug)]
pub struct Dimension;
impl Dimension {}

#[derive(Component, Debug)]
pub struct GenerationSeed {
    pub seed: u32,
}

pub struct DimensionSwapEvent {
    pub dimension: Entity,
}
#[derive(Component)]

pub struct ActiveDimension;
pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DimensionSwapEvent>()
            .add_startup_system(Self::hello_world)
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_system(Self::handle_dimension_swap_events),
            );
    }
}
impl DimensionPlugin {
    ///spawns the initial world dimension entity
    fn hello_world(mut commands: Commands) {
        commands.spawn((
            Dimension,
            ActiveDimension,
            GenerationSeed { seed: 0 },
            ChunkManager::new(),
        ));
        commands.insert_resource(ChunkManager::new())
    }
    pub fn handle_dimension_swap_events(
        mut dim_event: EventReader<DimensionSwapEvent>,
        mut commands: Commands,
        entity_query: Query<
            Entity,
            (
                Or<(With<WorldObject>, With<Enemy>, With<TilemapId>)>,
                Without<Equipment>,
            ),
        >,
        old_dim: Query<Entity, With<ActiveDimension>>,
        cm: Query<&ChunkManager>,
        old_cm: Res<ChunkManager>,
    ) {
        // event sent out when we enter a new dimension
        for d in dim_event.iter() {
            //despawn all entities with positions, except the player
            println!("DESPAWNING EVERYTHING!!!");
            for e in entity_query.iter() {
                commands.entity(e).despawn_recursive();
            }
            // clean up old dimension, remove active tag, and update its chunk manager
            println!("inserting new chunk manager/dim");
            commands
                .entity(old_dim.single())
                .remove::<ActiveDimension>()
                .insert(old_cm.clone());
            //give the new dimension active tag, and use its chunk manager as the game resource
            commands
                .entity(d.dimension)
                .insert((ActiveDimension, cm.get(d.dimension).unwrap().clone()));
            commands.insert_resource(cm.get(d.dimension).unwrap().clone());
        }
    }
}
