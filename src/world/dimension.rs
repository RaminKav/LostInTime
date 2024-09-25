use bevy::{prelude::*, utils::HashMap};
use bevy_proto::prelude::ProtoCommands;
use bevy_save::{CloneReflect, Snapshot};
use serde::{Deserialize, Serialize};

use crate::{
    enemy::Mob,
    item::Equipment,
    player::{MovePlayerEvent, Player},
    world::{
        dungeon::{Dungeon, Dungeontimer},
        dungeon_generation::{gen_new_dungeon, get_player_spawn_tile, Bias},
        world_helpers::world_pos_to_tile_pos,
        CHUNK_SIZE,
    },
    CustomFlush, GameParam, GameState,
};

use super::{
    chunk::Chunk,
    dungeon::{CachedPlayerPos, DungeonText},
    generation::WorldObjectCache,
    portal::TimePortal,
    wall_auto_tile::ChunkWallCache,
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
    pub swap_to_dim_now: bool,
    pub new_era: Option<Era>,
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

#[derive(Component, Default, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub enum Era {
    #[default]
    Main,
    Second,
    DungeonMain,
}

impl Era {
    pub fn get_texture_index(&self) -> usize {
        match self {
            Era::Main => 0,
            Era::DungeonMain => 1 * 16,
            Era::Second => 2 * 16,
        }
    }
    pub fn index(&self) -> usize {
        match self {
            Era::Main => 0,
            Era::Second => 1,
            Era::DungeonMain => 2,
        }
    }
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Era::Main,
            1 => Era::Second,
            2 => Era::DungeonMain,
            i => panic!("Invalid Era index:{}", i),
        }
    }
    pub fn is_dungeon(&self) -> bool {
        match self {
            Era::DungeonMain => true,
            _ => false,
        }
    }
    pub fn get_assosiated_era_from_dungeon_era(&self) -> Self {
        match self {
            Era::DungeonMain => Era::Main,
            _ => panic!("Invalid era for dungeon era"),
        }
    }
}

#[derive(Resource, Default, Debug, Clone)]
pub struct EraManager {
    pub current_era: Era,
    pub visited_eras: Vec<Era>,
    pub era_generation_cache: HashMap<Era, WorldObjectCache>,
}
pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DimensionSpawnEvent>()
            .add_system(Self::clear_entities_for_dim_swap.before(CustomFlush))
            .add_system(
                Self::new_dim_with_params
                    .in_base_set(CoreSet::PreUpdate)
                    .run_if(in_state(GameState::Main)),
            )
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
        player_cache_pos: Query<(Entity, &CachedPlayerPos), With<Player>>,
        mut game: GameParam,
        mut proto_commands: ProtoCommands,
        mut chunk_wall_cache: Query<&mut ChunkWallCache>,
    ) {
        for new_dim in spawn_event.iter() {
            info!("SPAWNING NEW DIMENSION {:?}", new_dim.new_era);

            let dim_e = commands.spawn((Dimension,)).id();
            if new_dim.swap_to_dim_now {
                commands.entity(dim_e).insert(SpawnDimension);
            }
            for e in dungeon_text.iter() {
                commands.entity(e).despawn();
            }

            //swap era data
            if let Some(new_era) = &new_dim.new_era {
                if new_era.is_dungeon() {
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
                    commands
                        .entity(dim_e)
                        .insert(Dungeon { grid: grid.clone() })
                        .insert(Dungeontimer(Timer::from_seconds(360., TimerMode::Once)));

                    if let Some(pos) = get_player_spawn_tile(grid.clone()) {
                        info!("MOVING PLAYER TO {:?}", pos);
                        move_player_event.send(MovePlayerEvent { pos });
                    }
                }

                let curr_era = game.era.current_era.clone();
                if !curr_era.is_dungeon() {
                    game.era
                        .era_generation_cache
                        .insert(curr_era.clone(), game.world_obj_cache.clone());
                    for mut chunk_wall_cache in chunk_wall_cache.iter_mut() {
                        chunk_wall_cache.walls.clear();
                    }
                }
                game.era.current_era = new_era.clone();

                commands.remove_resource::<WorldObjectCache>();
                let new_world_cache = game
                    .era
                    .era_generation_cache
                    .get(new_era)
                    .cloned()
                    .unwrap_or(WorldObjectCache::default());
                commands.insert_resource(new_world_cache);
                proto_commands.apply(format!("Era{}WorldGenerationParams", new_era.index() + 1));
            } else {
                info!("USE CURR ERA: {:?}", game.era.current_era.index());
                proto_commands.apply(format!(
                    "Era{}WorldGenerationParams",
                    game.era.current_era.index() + 1
                ));
                if let Some(era_cache) = game.era.era_generation_cache.get(&game.era.current_era) {
                    info!("APPLYING ERA CACHE");
                    commands.insert_resource(era_cache.clone());
                }
            }

            if let Ok((e, cached_pos)) = player_cache_pos.get_single() {
                move_player_event.send(MovePlayerEvent { pos: cached_pos.0 });
                commands.entity(e).remove::<CachedPlayerPos>();
            }
        }
    }
    //TODO: integrate this with events to work wiht bevy_save
    pub fn clear_entities_for_dim_swap(
        new_dim: Query<Entity, Added<SpawnDimension>>,
        mut commands: Commands,
        entity_query: Query<
            Entity,
            (
                Or<(With<Mob>, With<Chunk>, With<TimePortal>)>,
                Without<Equipment>,
            ),
        >,
        old_dim: Query<Entity, With<ActiveDimension>>,
    ) {
        // event sent out when we enter a new dimension
        for d in new_dim.iter() {
            //despawn all entities with positions, except the player
            info!("DESPAWNING EVERYTHING!!! {:?}", entity_query.iter().len());
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

pub fn dim_spawned(dim_spawn: Query<Entity, With<ActiveDimension>>) -> bool {
    dim_spawn.iter().count() > 0
}
