use bevy::{prelude::*, utils::HashMap};
use bevy_proto::prelude::ProtoCommands;
use bevy_save::{CloneReflect, Snapshot};
use serde::{Deserialize, Serialize};

use crate::{
    attributes::AttributeChangeEvent,
    bounce::DesertTornado,
    chaos::EraTransitionState,
    colors::{DESERT_TILE, SNOW_TILE},
    enemy::{spawner::MobSpawningPaused, Mob},
    item::{Equipment, ItemDrop, WorldObject},
    ui::global_text_message::PendingEraAnnouncement,
    night::NightTracker,
    player::{MovePlayerEvent, Player},
    world::{
        dungeon::{Dungeon, Dungeontimer},
        dungeon_generation::{gen_new_room_dungeon, get_player_spawn_tile, DUNGEON_GRID_SIZE},
        world_helpers::world_pos_to_tile_pos,
    },
    CustomFlush, GameParam, GameState,
};
use bevy::ecs::schedule::NextState;

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

/// Marker inserted on the dimension entity to trigger a one-shot spawn pass,
/// then removed when spawning completes. `SparseSet` so dimension transitions
/// don't move the dimension entity through an extra archetype each time.
#[derive(Component, Debug)]
#[component(storage = "SparseSet")]
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
    Third,
    DungeonMain,
}

impl Era {
    pub fn get_texture_index(&self) -> usize {
        match self {
            Era::Main => 0,
            Era::DungeonMain => 1 * 16,
            Era::Second => 4 * 16,
            Era::Third => 5 * 16,
        }
    }
    pub fn index(&self) -> usize {
        match self {
            Era::Main => 0,
            Era::Second => 1,
            Era::Third => 2,
            Era::DungeonMain => 3,
        }
    }
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Era::Main,
            1 => Era::Second,
            2 => Era::Third,
            3 => Era::DungeonMain,
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
    pub fn get_chaos_modifier(&self) -> f32 {
        match self {
            Era::Main => 0.,
            Era::Second => 8.0, // era2: slightly harder (+3 over previous 10)
            Era::Third => 22.0, // era3: +10 over previous 20
            Era::DungeonMain => 1.0,
        }
    }

    /// Title shown at run start; matches overworld era progression.
    pub fn run_start_announcement_label(&self) -> Option<&'static str> {
        match self {
            Era::Main => Some("Forest Era"),
            Era::Second => Some("Desert Era"),
            Era::Third => Some("Snow Era"),
            Era::DungeonMain => None,
        }
    }

    /// Grass-tile minimap fill for this era (see [`crate::ui::minimap`]).
    pub fn minimap_grass_background_color(&self) -> Color {
        match self {
            Era::Main | Era::DungeonMain => WorldObject::GrassTile.get_obj_color(),
            Era::Second => DESERT_TILE,
            Era::Third => SNOW_TILE,
        }
    }
}

#[derive(Resource, Debug, Clone)]
pub struct EraManager {
    pub current_era: Era,
    /// Eras the player has actually entered this run (includes the starting overworld era).
    /// Used for unlocks (e.g. blueprint recipes). Duplicates are avoided when recording transitions.
    pub visited_eras: Vec<Era>,
    pub era_generation_cache: HashMap<Era, WorldObjectCache>,
}

impl Default for EraManager {
    fn default() -> Self {
        let start = Era::default();
        Self {
            current_era: start.clone(),
            visited_eras: vec![start],
            era_generation_cache: HashMap::default(),
        }
    }
}
pub struct DimensionPlugin;

impl Plugin for DimensionPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<DimensionSpawnEvent>()
            .add_system(Self::clear_entities_for_dim_swap.before(CustomFlush))
            .add_system(
                Self::new_dim_with_params
                    .in_base_set(CoreSet::PreUpdate)
                    // `GameParam` requires `WorldObjectCache`; gate on it so this system never
                    // runs during run teardown (exit-to-menu sends `CleanUpRunStateEvent`, which
                    // removes the cache in the same `PreUpdate` frame while still in `Main`).
                    .run_if(resource_exists::<WorldObjectCache>())
                    .run_if(
                        in_state(GameState::Main)
                            .or_else(in_state(GameState::Initializing))
                            .or_else(in_state(GameState::BlessingChoice)),
                    ),
            )
            .add_system(rebuild_attributes_on_new_dimension.in_schedule(OnEnter(GameState::Main)))
            .add_system(log_drop_filter_on_dim_swap)
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

/// TEMP DIAGNOSTIC (drop-filter leak): logs the break-drop filter contents whenever an era
/// swap is requested. If size/contents persist across the era2 -> era3 transition, the
/// resource is NOT being reset. Remove once the leak is resolved.
fn log_drop_filter_on_dim_swap(
    mut spawn_event: EventReader<DimensionSpawnEvent>,
    break_drop_filter: Res<crate::inventory::BreakDropFilter>,
) {
    for new_dim in spawn_event.iter() {
        warn!(
            "[DROP-FILTER] era swap -> {:?} | filter_size={} contents={:?}",
            new_dim.new_era,
            break_drop_filter.0.len(),
            break_drop_filter.0,
        );
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
        mut next_state: ResMut<NextState<GameState>>,
        mut night: ResMut<NightTracker>,
        mut chaos_tracker: ResMut<crate::chaos::ChaosTracker>,
        mut era_timer: ResMut<crate::night::EraTimer>,
        mut infinite_mode: ResMut<crate::night::InfiniteMode>,
        mut mob_spawning_paused: ResMut<MobSpawningPaused>,
        mut transition_state: ResMut<EraTransitionState>,
        mut boss_summon_tracker: ResMut<crate::item::boss_shrine::BossSummonTracker>,
    ) {
        // Process only the first dimension spawn per frame to avoid double-firing when the portal
        // is triggered by both click and interact key (F), or by rapid double input.
        let mut processed_one = false;
        for new_dim in spawn_event.iter() {
            if processed_one {
                warn!(
                    "Ignoring duplicate DimensionSpawnEvent for {:?} (portal was likely triggered twice)",
                    new_dim.new_era
                );
                continue;
            }
            processed_one = true;
            let mut sent_dungeon_spawn = false;

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
                // Transition to Initializing state for non-dungeon era changes to show loading screen
                if new_dim.swap_to_dim_now && !new_era.is_dungeon() {
                    info!(
                        "Transitioning to Initializing state for era change to {:?}",
                        new_era
                    );
                    next_state.set(GameState::Initializing);
                }
                if new_era.is_dungeon() {
                    let player = game.player_query.single().0;
                    let player_pos = game.player().position;
                    commands
                        .entity(player)
                        .insert(CachedPlayerPos(world_pos_to_tile_pos(
                            player_pos.truncate(),
                        )));
                    let grid = gen_new_room_dungeon(DUNGEON_GRID_SIZE as usize);
                    commands
                        .entity(dim_e)
                        .insert(Dungeon { grid: grid.clone() })
                        .insert(Dungeontimer(Timer::from_seconds(360., TimerMode::Once)));

                    if let Some(pos) = get_player_spawn_tile(grid.clone()) {
                        info!("MOVING PLAYER TO {:?}", pos);
                        move_player_event.send(MovePlayerEvent { pos });
                        sent_dungeon_spawn = true;
                    } else {
                        error!("Failed to find valid player spawn position in dungeon! This should not happen.");
                    }
                } else {
                    // Check if we're returning from a dungeon vs entering a truly new era
                    let curr_era = &game.era.current_era;
                    let returning_from_dungeon = curr_era.is_dungeon();

                    // Set starting day based on era (only when entering a new era, not returning from dungeon)
                    if !returning_from_dungeon {
                        // 0-based internal days: Main=0, Second=1, Third=2 (display is +1).
                        let era_starting_day = new_era.index() as u8;
                        night.days = era_starting_day;
                        night.time = 0.;
                        info!(
                            "Era {:?} starting at internal day {} (display day {})",
                            new_era,
                            era_starting_day,
                            era_starting_day + 1
                        );

                        crate::night::reset_era_timer_and_infinite_mode(
                            &mut era_timer,
                            &mut infinite_mode,
                            &mut mob_spawning_paused,
                        );
                        boss_summon_tracker.reset();

                        if *new_era != Era::Main {
                            commands.insert_resource(PendingEraAnnouncement::OnEraEnter(
                                new_era.clone(),
                            ));
                        }
                    } else {
                        info!("Returning from dungeon to {:?}, keeping era timer", new_era);
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
                if !curr_era.is_dungeon() && !new_era.is_dungeon() {
                    commands.insert_resource(crate::ui::EssenceShopCache::default());

                    let old_era_chaos = curr_era.get_chaos_modifier();
                    let new_era_chaos = new_era.get_chaos_modifier();
                    // Update chaos tracker: remove old era's chaos, add new era's chaos
                    chaos_tracker.add_chaos(new_era_chaos - old_era_chaos);

                    // Set up mob unlock timers for new mobs in this era (skip for Era::Main)
                    transition_state.mob_unlock_timers.clear();
                    if new_era != &Era::Main {
                        transition_state
                            .mob_unlock_timers
                            .insert(Mob::StingFly, Timer::from_seconds(90.0, TimerMode::Once));
                        transition_state
                            .mob_unlock_timers
                            .insert(Mob::Bushling, Timer::from_seconds(45.0, TimerMode::Once));
                        transition_state
                            .mob_unlock_timers
                            .insert(Mob::SpikeSlime, Timer::from_seconds(160.0, TimerMode::Once));
                        transition_state
                            .mob_unlock_timers
                            .insert(Mob::SmallCactus, Timer::from_seconds(45.0, TimerMode::Once));
                        transition_state
                            .mob_unlock_timers
                            .insert(Mob::BigCactus, Timer::from_seconds(100.0, TimerMode::Once));
                        transition_state
                            .mob_unlock_timers
                            .insert(Mob::Bull, Timer::from_seconds(180.0, TimerMode::Once));

                        info!(
                            "Initialized era transition for {:?}: mobs will unlock gradually",
                            new_era
                        );
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

                if !game.era.visited_eras.contains(new_era) {
                    game.era.visited_eras.push(new_era.clone());
                }
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

            if !sent_dungeon_spawn {
                if let Ok((e, cached_pos)) = player_cache_pos.get_single() {
                    move_player_event.send(MovePlayerEvent { pos: cached_pos.0 });
                    commands.entity(e).remove::<CachedPlayerPos>();
                }
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
                Or<(
                    With<Mob>,
                    With<Chunk>,
                    With<TimePortal>,
                    With<ItemDrop>,
                    With<DesertTornado>,
                )>,
                Without<Equipment>,
            ),
        >,
        old_dim: Query<Entity, With<ActiveDimension>>,
    ) {
        // event sent out when we enter a new dimension
        for d in new_dim.iter() {
            //despawn all entities with positions, except the player
            // clean up old dimension,
            if let Ok(old_dim) = old_dim.get_single() {
                info!("DESPAWNING EVERYTHING!!! {:?}", entity_query.iter().len());
                for e in entity_query.iter() {
                    commands.entity(e).despawn_recursive();
                }
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
pub fn rebuild_attributes_on_new_dimension(mut attribute_event: EventWriter<AttributeChangeEvent>) {
    attribute_event.send_default();
}
pub fn dim_spawned(dim_spawn: Query<Entity, With<ActiveDimension>>) -> bool {
    dim_spawn.iter().count() > 0
}
