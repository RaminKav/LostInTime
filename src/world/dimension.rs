use bevy::{prelude::*, transform::TransformSystem, utils::HashMap};
use bevy_save::{CloneReflect, Snapshot};

use crate::{
    enemy::Mob,
    item::Equipment,
    player::{MovePlayerEvent, Player},
    CustomFlush, WorldGeneration,
};

use super::{
    chunk::Chunk,
    dungeon::{CachedPlayerPos, DungeonText},
    TileMapPosition,
};

#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component)]
pub struct Dimension;
impl Dimension {}

#[derive(Resource, Reflect, Default, Debug, Clone)]
#[reflect(Resource)]
pub struct GenerationSeed {
    pub seed: u64,
}

#[derive(Component, Debug)]
pub struct SpawnDimension;
pub struct DimensionSpawnEvent {
    pub generation_params: WorldGeneration,
    pub swap_to_dim_now: bool,
}
#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component)]

pub struct ActiveDimension;

#[derive(Component, Default)]
pub struct ChunkCache {
    pub snapshots: HashMap<IVec2, Snapshot>,
}
impl Clone for ChunkCache {
    fn clone(&self) -> Self {
        let mut cloned_map = HashMap::default();
        for v in &self.snapshots {
            cloned_map.insert(*v.0, v.1.clone_value());
        }
        Self {
            snapshots: cloned_map,
        }
    }
}

pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DimensionSpawnEvent>()
            .add_system(Self::handle_dimension_swap_events.before(CustomFlush))
            .add_system(Self::new_dim_with_params.after(CustomFlush))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}
impl DimensionPlugin {
    ///spawns the initial world dimension entity
    pub fn new_dim_with_params(
        mut commands: Commands,
        mut spawn_event: EventReader<DimensionSpawnEvent>,
        dungeon_text: Query<Entity, With<DungeonText>>,
        mut move_player_event: EventWriter<MovePlayerEvent>,
        player_pos: Query<&CachedPlayerPos, With<Player>>,
    ) {
        for new_dim in spawn_event.iter() {
            println!("SPAWNING NEW DIMENSION");
            commands.insert_resource(new_dim.generation_params.clone());
            let dim_e = commands.spawn((Dimension,)).id();
            if new_dim.swap_to_dim_now {
                commands.entity(dim_e).insert(SpawnDimension);
            }
            for e in dungeon_text.iter() {
                commands.entity(e).despawn();
            }
            if let Ok(cached_pos) = player_pos.get_single() {
                move_player_event.send(MovePlayerEvent { pos: cached_pos.0 });
            }
        }
    }
    //TODO: integrate this with events to work wiht bevy_save
    pub fn handle_dimension_swap_events(
        new_dim: Query<Entity, Added<SpawnDimension>>,
        mut commands: Commands,
        entity_query: Query<Entity, (Or<(With<Mob>, With<Chunk>)>, Without<Equipment>)>,
        old_dim: Query<Entity, With<ActiveDimension>>,
    ) {
        // event sent out when we enter a new dimension
        for d in new_dim.iter() {
            //despawn all entities with positions, except the player
            println!("DESPAWNING EVERYTHING!!! {:?}", entity_query.iter().len());
            for e in entity_query.iter() {
                commands.entity(e).despawn_recursive();
            }
            // clean up old dimension,
            if let Ok(old_dim) = old_dim.get_single() {
                commands.entity(old_dim).despawn_recursive();
            }
            //give the new dimension active tag, and use its chunk manager as the game resource
            commands
                .entity(d)
                .insert(ActiveDimension)
                .remove::<SpawnDimension>();
        }
    }
}
