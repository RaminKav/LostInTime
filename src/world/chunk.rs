use std::hash::Hash;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};

use super::dimension::{ActiveDimension, GenerationSeed};

use super::world_helpers::get_neighbour_tile;

use crate::world::dimension::ChunkCache;
use crate::world::wall_auto_tile::ChunkWallCache;
use crate::{item::WorldObject, GameParam, ImageAssets};
use crate::{CustomFlush, GameState, TextureCamera};
use serde::{Deserialize, Serialize};

use super::tile::TilePlugin;
use super::{
    world_helpers, TileMapPosition, CHUNK_SIZE, MAX_VISIBILITY, NUM_CHUNKS_AROUND_CAMERA, TILE_SIZE,
};

pub struct ChunkPlugin;
impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChunkObjectCache::default())
            .add_event::<SpawnChunkEvent>()
            .add_event::<DespawnChunkEvent>()
            .add_event::<CreateChunkEvent>()
            .add_event::<GenerateObjectsEvent>()
            .add_systems(
                (
                    Self::spawn_chunks_around_camera.after(CustomFlush), //.before(Self::handle_new_chunk_event),
                    Self::handle_new_chunk_event.before(CustomFlush),
                    Self::handle_update_tiles_for_new_chunks
                        .after(Self::register_spawned_chunks)
                        .after(CustomFlush),
                    Self::despawn_outofrange_chunks,
                    Self::toggle_on_screen_mesh_visibility.before(CustomFlush),
                    Self::register_spawned_chunks.after(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}
#[derive(Resource, Debug, Clone, Default)]
pub struct ChunkObjectCache {
    pub cache: HashMap<IVec2, Vec<(WorldObject, TileMapPosition)>>,
}
#[derive(Component)]
pub struct SpawnedChunk;
#[derive(Eq, Hash, Reflect, Component, PartialEq, Default, Debug, Clone)]
#[reflect(Component)]
pub struct TileSpriteData {
    pub block_type: [WorldObject; 4],
    pub raw_block_type: [WorldObject; 4],
    pub tile_bit_index: u8,
    pub texture_offset: u8,
}
#[derive(Clone)]
pub struct SpawnChunkEvent {
    pub chunk_pos: IVec2,
}
#[derive(Clone)]
pub struct DespawnChunkEvent {
    pub chunk_pos: IVec2,
}
#[derive(Clone)]
pub struct GenerateObjectsEvent {
    pub chunk_pos: IVec2,
}

#[derive(Component)]
pub struct VisibleObject;
#[derive(Clone)]
pub struct CreateChunkEvent {
    pub chunk_pos: IVec2,
}
#[derive(Component, Reflect, FromReflect, Default, Debug, Clone)]
#[reflect(Component)]
pub struct TileEntityCollection {
    pub map: HashMap<ReflectedPos, Entity>,
}
#[derive(
    Component,
    Reflect,
    FromReflect,
    Default,
    Clone,
    Copy,
    Debug,
    Hash,
    PartialEq,
    Eq,
    PartialOrd,
    Serialize,
    Deserialize,
)]
#[reflect_value(Component, Hash, Serialize, Deserialize)]
pub struct ReflectedPos {
    x: i32,
    y: i32,
}
impl From<TilePos> for ReflectedPos {
    fn from(val: TilePos) -> ReflectedPos {
        ReflectedPos {
            x: val.x as i32,
            y: val.y as i32,
        }
    }
}
impl From<IVec2> for ReflectedPos {
    fn from(val: IVec2) -> ReflectedPos {
        ReflectedPos { x: val.x, y: val.y }
    }
}
#[derive(Component, Reflect, Default, Debug, Clone)]
#[reflect(Component)]
pub struct Chunk {
    pub chunk_pos: IVec2,
}

impl ChunkPlugin {
    fn handle_new_chunk_event(
        mut cache_events: EventReader<CreateChunkEvent>,
        mut commands: Commands,
        sprite_sheet: Res<ImageAssets>,
        game: GameParam,
        seed: Query<&GenerationSeed, With<ActiveDimension>>,
    ) {
        for e in cache_events.iter() {
            let chunk_pos = e.chunk_pos;
            if game.get_chunk_entity(chunk_pos).is_some() {
                continue;
            }

            let chunk_pos = e.chunk_pos;
            let tilemap_size = TilemapSize {
                x: CHUNK_SIZE,
                y: CHUNK_SIZE,
            };
            let tilemap_entity = commands.spawn_empty().id();
            let mut tiles = HashMap::default();
            let mut tile_storage = TileStorage::empty(tilemap_size);
            let tile_size = TilemapTileSize {
                x: TILE_SIZE.x,
                y: TILE_SIZE.y,
            };
            let grid_size = tile_size.into();
            let map_type = TilemapType::default();

            let mut raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize];
                CHUNK_SIZE as usize] =
                [[[WorldObject::SandTile; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
            println!("Creating new chunk {chunk_pos:?}");

            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };

                    let (bits, index_shift, blocks) = TilePlugin::get_tile_from_perlin_noise(
                        &game.world_generation_params,
                        chunk_pos,
                        tile_pos,
                        seed.single().seed,
                    );

                    let block_bits = bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8;

                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(tilemap_entity),
                            texture_index: TileTextureIndex((block_bits + index_shift).into()),
                            ..Default::default()
                        })
                        .insert(TileSpriteData {
                            tile_bit_index: block_bits,
                            block_type: blocks,
                            raw_block_type: blocks,
                            texture_offset: index_shift,
                        })
                        .id();
                    raw_chunk_blocks[x as usize][y as usize] = blocks;
                    tiles.insert(tile_pos.into(), tile_entity);
                    commands.entity(tilemap_entity).add_child(tile_entity);
                    tile_storage.set(&tile_pos, tile_entity);
                }
            }
            let transform = Transform::from_translation(Vec3::new(
                chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
                chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y,
                0.,
            ));

            let chunk = commands
                .entity(tilemap_entity)
                .insert(TilemapBundle {
                    grid_size,
                    map_type,
                    size: tilemap_size,
                    storage: tile_storage,
                    texture: TilemapTexture::Single(sprite_sheet.tiles_sheet.clone()),
                    tile_size,
                    transform,
                    ..Default::default()
                })
                .insert(TileEntityCollection { map: tiles })
                .insert(Chunk { chunk_pos })
                .insert(Name::new(format!("Pos: {}", chunk_pos)))
                .id();
            commands.entity(chunk).insert(ChunkWallCache {
                walls: HashMap::new(),
            });

            // game.set_chunk_entity(chunk_pos, chunk);
            // minimap_update.send(UpdateMiniMapEvent);
        }
    }
    pub fn register_spawned_chunks(
        mut game: GameParam,
        loaded_chunks: Query<(Entity, &Chunk), Added<Chunk>>,
    ) {
        for (e, chunk) in loaded_chunks.iter() {
            game.set_chunk_entity(chunk.chunk_pos, e);
        }
    }
    pub fn handle_update_tiles_for_new_chunks(
        mut create_events: EventReader<CreateChunkEvent>,
        mut gen_events: EventWriter<GenerateObjectsEvent>,
        mut commands: Commands,
        mut game: GameParam,
    ) {
        for e in create_events.iter() {
            let chunk_pos = e.chunk_pos;
            if !game.get_chunk_entity(chunk_pos).is_some() {
                continue;
            }

            let chunk_pos = e.chunk_pos;
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let pos = TileMapPosition::new(chunk_pos, TilePos { x, y }, 0);
                    let tile_data = game.get_tile_data(pos.clone()).unwrap();

                    for dy in -1i8..=1 {
                        for dx in -1i8..=1 {
                            let TileMapPosition {
                                chunk_pos: adjusted_chunk_pos,
                                tile_pos: neighbour_tile_pos,
                                ..
                            } = get_neighbour_tile(pos.clone(), (dx, dy));
                            if adjusted_chunk_pos != chunk_pos
                                && game.get_chunk_entity(adjusted_chunk_pos).is_some()
                            {
                                TilePlugin::update_this_tile(
                                    &mut commands,
                                    neighbour_tile_pos,
                                    game.get_tile_data(TileMapPosition::new(
                                        adjusted_chunk_pos,
                                        neighbour_tile_pos,
                                        0,
                                    ))
                                    .unwrap()
                                    .texture_offset,
                                    &mut game,
                                    adjusted_chunk_pos,
                                );
                            }
                        }
                    }

                    TilePlugin::update_this_tile(
                        &mut commands,
                        TilePos { x, y },
                        tile_data.texture_offset,
                        &mut game,
                        chunk_pos,
                    );
                    // if tile_data.block_type.contains(&WorldObject::Grass) {
                    // }
                }
            }
            gen_events.send(GenerateObjectsEvent { chunk_pos });

            //TODO: add event for this
            // GenerationPlugin::generate_and_cache_objects(
            //     &mut game,
            //     &mut pkv,
            //     chunk_pos,
            //     seed.single().seed,
            // );
        }
    }

    pub fn spawn_chunks_around_camera(
        game: GameParam,
        mut camera_query: Query<&Transform, With<TextureCamera>>,
        mut create_chunk_event: EventWriter<CreateChunkEvent>,
        _load_chunk_event: EventWriter<SpawnChunkEvent>,
        chunk_cache: Query<&ChunkCache, With<ActiveDimension>>,
    ) {
        if chunk_cache.get_single().is_err() {
            return;
        }
        let transform = camera_query.single_mut();
        let camera_chunk_pos = world_helpers::camera_pos_to_chunk_pos(&transform.translation.xy());
        for y in (camera_chunk_pos.y - NUM_CHUNKS_AROUND_CAMERA)
            ..(camera_chunk_pos.y + NUM_CHUNKS_AROUND_CAMERA)
        {
            for x in (camera_chunk_pos.x - NUM_CHUNKS_AROUND_CAMERA)
                ..(camera_chunk_pos.x + NUM_CHUNKS_AROUND_CAMERA)
            {
                let chunk_pos = IVec2::new(x, y);
                if game.get_chunk_entity(chunk_pos).is_none() {
                    // load_chunk_event.send(SpawnChunkEvent { chunk_pos });
                    // if chunk_cache.single().snapshots.contains_key(&chunk_pos) {
                    //     println!("Sending load event {chunk_pos:?}");
                    // } else {
                    // println!("Sending cache event {chunk_pos:?}");
                    create_chunk_event.send(CreateChunkEvent { chunk_pos });
                    // }
                }
            }
        }
    }
    //TODO: change despawning systems to use playe rpos instead??
    fn despawn_outofrange_chunks(
        game: GameParam,
        mut events: EventWriter<DespawnChunkEvent>,
        camera_query: Query<&Transform, With<TextureCamera>>,
    ) {
        let mut removed_chunks = vec![];
        for camera_transform in camera_query.iter() {
            let max_distance = f32::hypot(
                CHUNK_SIZE as f32 * TILE_SIZE.x,
                CHUNK_SIZE as f32 * TILE_SIZE.y,
            );
            for (_entity, chunk_transform, _) in game.chunk_query.iter() {
                let chunk_pos = chunk_transform.translation.xy();
                let distance = camera_transform.translation.xy().distance(chunk_pos);
                //TODO: calculate maximum possible distance for 2x2 chunksa
                let x = (chunk_pos.x / (CHUNK_SIZE as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y / (CHUNK_SIZE as f32 * TILE_SIZE.y)).floor() as i32;
                if distance > max_distance * 2. * NUM_CHUNKS_AROUND_CAMERA as f32
                    && game.get_chunk_entity(IVec2::new(x, y)).is_some()
                {
                    println!("despawning chunk at {x:?} {y:?} d === {distance:?}");
                    removed_chunks.push(IVec2::new(x, y));
                    // commands.entity(entity).despawn_recursive();
                    events.send(DespawnChunkEvent {
                        chunk_pos: IVec2::new(x, y),
                    })
                }
            }
        }
    }

    fn toggle_on_screen_mesh_visibility(
        camera_query: Query<&Transform, With<TextureCamera>>,
        mut obj_query: Query<(&mut Visibility, &GlobalTransform), With<WorldObject>>,
    ) {
        for camera_transform in camera_query.iter() {
            for (mut v, ft) in obj_query.iter_mut() {
                let pos = ft.translation().xy();
                let distance = camera_transform.translation.xy().distance(pos);
                if (*v == Visibility::Visible || *v == Visibility::Inherited)
                    && distance > (MAX_VISIBILITY * 2_u32) as f32
                {
                    *v = Visibility::Hidden;
                } else if *v != Visibility::Visible && distance <= (MAX_VISIBILITY * 2_u32) as f32 {
                    *v = Visibility::Visible;
                }
            }
        }
    }
}
