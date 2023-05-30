use bevy::{ecs::system::SystemState, prelude::*, utils::HashMap};
use bevy_ecs_tilemap::{
    prelude::{
        TilemapGridSize, TilemapId, TilemapSize, TilemapSpacing, TilemapTexture, TilemapTileSize,
        TilemapType,
    },
    tiles::{TileColor, TileFlip, TilePos, TilePosOld, TileStorage, TileTextureIndex, TileVisible},
    FrustumCulling,
};
use bevy_rapier2d::prelude::Collider;
use bevy_save::prelude::*;
// use bevy_save::{Build, CloneReflect, DespawnMode, MappingMode, SavePlugins, Snapshot};

use crate::{
    attributes::Health,
    item::{Breakable, WorldObject},
    ui::minimap::UpdateMiniMapEvent,
    world::{
        chunk::{
            Chunk, CreateChunkEvent, DespawnChunkEvent, SpawnChunkEvent, TileEntityCollection,
            TileSpriteData,
        },
        dimension::{ActiveDimension, ChunkCache},
        ChunkManager, TileMapPositionData, WorldObjectEntityData,
    },
    CustomFlush, GameParam, GameState, YSort,
};

#[derive(Component, Reflect)]

pub struct ColliderReflect {
    collider: Vec2,
}
impl ColliderReflect {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            collider: Vec2::new(x, y),
        }
    }
}
pub struct ClientPlugin;

impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SavePlugins)
            // register tile bundle types
            .register_saveable::<TileSpriteData>()
            .register_saveable::<TilePos>()
            .register_saveable::<TileTextureIndex>()
            .register_saveable::<TilemapId>()
            .register_saveable::<TileVisible>()
            .register_saveable::<TileFlip>()
            .register_saveable::<TileColor>()
            .register_saveable::<TilePosOld>()
            // register chunk bundle types
            .register_saveable::<Chunk>()
            .register_saveable::<TilemapGridSize>()
            .register_saveable::<TilemapType>()
            .register_saveable::<TilemapSize>()
            .register_saveable::<TilemapSpacing>()
            .register_saveable::<TileStorage>()
            .register_saveable::<TilemapTexture>()
            .register_saveable::<TilemapTileSize>()
            .register_saveable::<FrustumCulling>()
            .register_saveable::<TileEntityCollection>()
            // register obj types
            .register_saveable::<WorldObject>()
            .register_saveable::<Health>()
            .register_saveable::<WorldObjectEntityData>()
            .register_saveable::<YSort>()
            .register_saveable::<TileMapPositionData>()
            .register_saveable::<Breakable>()
            .register_saveable::<ColliderReflect>()
            .add_systems(
                (
                    Self::save_chunk,
                    Self::load_chunk.before(CustomFlush),
                    Self::handle_add_collider_to_loaded_entity.after(CustomFlush),
                    Self::register_loaded_chunks.after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

impl ClientPlugin {
    fn load_chunk(world: &mut World) {
        let mut state: SystemState<(
            Query<&ChunkCache, With<ActiveDimension>>,
            EventReader<SpawnChunkEvent>,
        )> = SystemState::new(world);
        let (dim_query, mut spawn_events) = state.get_mut(world);
        let mut snapshots = vec![];
        for event in spawn_events.iter() {
            if let Some(snapshot) = dim_query.single().snapshots.get(&event.chunk_pos) {
                snapshots.push(snapshot.clone_value());
                println!("Loading chunk from cache {:?}.", event.chunk_pos);
            }
        }
        for snapshot in snapshots.iter() {
            snapshot
                .applier(world)
                .despawn(DespawnMode::None)
                .mapping(MappingMode::Strict)
                .apply()
                .expect("Failed to Load snapshot.");
        }
    }
    fn register_loaded_chunks(
        mut game: GameParam,
        loaded_chunks: Query<(Entity, &Chunk), Added<Chunk>>,
        mut create_events: EventReader<CreateChunkEvent>,
        mut load_events: EventReader<SpawnChunkEvent>,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
    ) {
        for (e, chunk) in loaded_chunks.iter() {
            println!("REGISTERING {:?}", chunk.chunk_pos);
            game.set_chunk_entity(chunk.chunk_pos, e);
            minimap_update.send(UpdateMiniMapEvent);
        }
        // for chunk in load_events.iter() {
        //     println!("REGISTERING  load {:?}", chunk.chunk_pos);
        //     let e = game.get_chunk_entity(chunk.chunk_pos).unwrap();
        //     game.set_chunk_entity(chunk.chunk_pos, *e);
        //     minimap_update.send(UpdateMiniMapEvent);
        // }
        // for chunk in create_events.iter() {
        //     println!("REGISTERING create {:?}", chunk.chunk_pos);
        //     let e = game.get_chunk_entity(chunk.chunk_pos).unwrap();

        //     game.set_chunk_entity(chunk.chunk_pos, *e);
        //     minimap_update.send(UpdateMiniMapEvent);
        // }
    }
    fn handle_add_collider_to_loaded_entity(
        mut commands: Commands,
        loaded_entities: Query<(Entity, &ColliderReflect), Added<ColliderReflect>>,
    ) {
        for (e, collider) in loaded_entities.iter() {
            commands
                .entity(e)
                .insert(Collider::cuboid(collider.collider.x, collider.collider.y));
        }
    }
    //TODO: make this work with Spawn events too ,change event name
    fn save_chunk(
        world: &mut World,
        mut local: Local<
            SystemState<(Commands, Res<ChunkManager>, EventReader<DespawnChunkEvent>)>,
        >,
    ) {
        // let mut state: SystemState<(Res<ChunkManager>, EventReader<DespawnChunkEvent>)> =
        //     SystemState::new(world);

        let (mut commands, chunk_manager, mut save_events) = local.get_mut(world);
        let mut saved_chunks = HashMap::default();
        for saves in save_events.iter() {
            println!("SAVING {:?}...", saves.chunk_pos);

            saved_chunks.insert(
                saves.chunk_pos,
                chunk_manager.chunks.get(&saves.chunk_pos).unwrap().clone(),
            );
        }

        let mut snapshots: HashMap<IVec2, Snapshot> = HashMap::default();
        for (chunk_pos, entity) in saved_chunks.iter() {
            let snapshot = Snapshot::builder(world)
                .extract_entities(vec![].into_iter())
                .build();
            snapshots.insert(*chunk_pos, snapshot);
        }

        let mut state: SystemState<(
            GameParam,
            Commands,
            Query<&mut ChunkCache, With<ActiveDimension>>,
        )> = SystemState::new(world);

        let (mut game, mut commands, mut dim_query) = state.get_mut(world);

        for (chunk_pos, snapshot) in snapshots.iter() {
            println!("Inserting new snapshot for {chunk_pos:?} and despawning it");
            dim_query
                .single_mut()
                .snapshots
                .insert(*chunk_pos, snapshot.clone_value());
            commands
                .entity(*game.get_chunk_entity(*chunk_pos).unwrap())
                .despawn_recursive();
            game.remove_chunk_entity(*chunk_pos);
        }
    }
}
