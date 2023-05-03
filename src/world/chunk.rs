use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};
use bevy_pkv::PkvStore;

use super::dimension::{ActiveDimension, Dimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::generation::GenerationPlugin;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::{assets::FoliageMaterial, item::WorldObject, GameParam, ImageAssets};
use crate::{GameState, MainCamera, TextureCamera};

use super::tile::TilePlugin;
use super::{
    world_helpers, ChunkLoadingState, ChunkManager, RawChunkData, TileEntityData,
    TileMapPositionData, CHUNK_CACHE_AMOUNT, CHUNK_SIZE, MAX_VISIBILITY, NUM_CHUNKS_AROUND_CAMERA,
    TILE_SIZE,
};

pub const ZERO_ZERO: IVec2 = IVec2 { x: 0, y: 0 };

pub struct ChunkPlugin;
impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnChunkEvent>()
            .add_event::<CacheChunkEvent>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_system(
                        Self::spawn_and_cache_init_chunks.before(Self::handle_spawn_chunk_event),
                    )
                    .with_system(
                        Self::handle_spawn_chunk_event.after(Self::handle_cache_chunk_event),
                    )
                    .with_system(Self::handle_cache_chunk_event)
                    .with_system(Self::spawn_chunks_around_camera)
                    .with_system(Self::despawn_outofrange_chunks)
                    .with_system(Self::toggle_on_screen_mesh_visibility),
            );
    }
}
#[derive(Clone)]
pub struct SpawnChunkEvent {
    chunk_pos: IVec2,
}

#[derive(Clone)]
pub struct CacheChunkEvent {
    chunk_pos: IVec2,
}

impl ChunkPlugin {
    fn handle_cache_chunk_event(
        mut cache_events: EventReader<CacheChunkEvent>,
        mut commands: Commands,
        mut game: GameParam,
        mut pkv: ResMut<PkvStore>,
        seed: Query<&GenerationSeed, With<ActiveDimension>>,
    ) {
        for e in cache_events.iter() {
            let chunk_pos = e.chunk_pos;
            if game.chunk_manager.cached_chunks.contains(&chunk_pos) {
                continue;
            }
            game.chunk_manager.cached_chunks.insert(chunk_pos);

            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };
                    game.chunk_manager.chunk_tile_entity_data.insert(
                        TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        },
                        TileEntityData {
                            entity: None,
                            tile_bit_index: 0b0000,
                            block_type: [WorldObject::Sand; 4],
                            texture_offset: 0,
                        },
                    );
                }
            }

            let mut raw_chunk_bits: [[[u8; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
                [[[0; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
            let mut raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize];
                CHUNK_SIZE as usize] =
                [[[WorldObject::Sand; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };

                    let (bits, index_shift, blocks) = TilePlugin::get_tile_from_perlin_noise(
                        &game.chunk_manager,
                        chunk_pos,
                        tile_pos,
                        seed.single().seed,
                    );

                    raw_chunk_bits[x as usize][y as usize] = bits;
                    raw_chunk_blocks[x as usize][y as usize] = blocks;
                    let block_bits = bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8;

                    game.chunk_manager.chunk_tile_entity_data.insert(
                        TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        },
                        TileEntityData {
                            entity: None,
                            tile_bit_index: block_bits,
                            block_type: blocks,
                            texture_offset: index_shift,
                        },
                    );
                }
            }
            game.chunk_manager.raw_chunk_data.insert(
                chunk_pos,
                RawChunkData {
                    raw_chunk_bits,
                    raw_chunk_blocks,
                },
            );
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Water)
                        && raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Sand)
                    {
                        TilePlugin::update_neighbour_tiles(
                            TilePos { x, y },
                            &mut commands,
                            &mut game.chunk_manager,
                            chunk_pos,
                            false,
                        );
                    } else if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Grass)
                    {
                        TilePlugin::update_this_tile(
                            TilePos { x, y },
                            16,
                            &mut game.chunk_manager,
                            chunk_pos,
                        );
                    }
                }
            }
            //TODO: add event for this
            GenerationPlugin::generate_and_cache_objects(
                &mut game,
                &mut pkv,
                chunk_pos,
                seed.single().seed,
            );
        }
    }
    fn handle_spawn_chunk_event(
        mut spawn_events: EventReader<SpawnChunkEvent>,
        mut commands: Commands,
        sprite_sheet: Res<ImageAssets>,
        mut game: GameParam,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
    ) {
        for e in spawn_events.iter() {
            let chunk_pos = e.chunk_pos;
            let tilemap_size = TilemapSize {
                x: CHUNK_SIZE,
                y: CHUNK_SIZE,
            };
            let tile_size = TilemapTileSize {
                x: TILE_SIZE.x,
                y: TILE_SIZE.y,
            };
            let grid_size = tile_size.into();
            let map_type = TilemapType::default();

            let tilemap_entity = commands.spawn_empty().id();
            let mut tile_storage = TileStorage::empty(tilemap_size);
            if game.chunk_manager.cached_chunks.contains(&chunk_pos) {
                println!("Loading chunk {chunk_pos:?} from CACHE!");

                for y in 0..CHUNK_SIZE {
                    for x in 0..CHUNK_SIZE {
                        let tile_pos = TilePos { x, y };
                        let tile_entity_data = game
                            .chunk_manager
                            .chunk_tile_entity_data
                            .get(&TileMapPositionData {
                                chunk_pos,
                                tile_pos,
                            })
                            .unwrap();
                        let tile_entity = commands
                            .spawn(TileBundle {
                                position: tile_pos,
                                tilemap_id: TilemapId(tilemap_entity),
                                texture_index: TileTextureIndex(
                                    (tile_entity_data.tile_bit_index
                                        + tile_entity_data.texture_offset)
                                        .into(),
                                ),
                                ..Default::default()
                            })
                            .id();
                        game.chunk_manager
                            .chunk_tile_entity_data
                            .get_mut(&TileMapPositionData {
                                chunk_pos,
                                tile_pos,
                            })
                            .unwrap()
                            .entity = Some(tile_entity);
                        commands.entity(tilemap_entity).add_child(tile_entity);
                        tile_storage.set(&tile_pos, tile_entity);
                    }
                }

                let transform = Transform::from_translation(Vec3::new(
                    chunk_pos.x as f32 * CHUNK_SIZE as f32 * TILE_SIZE.x,
                    chunk_pos.y as f32 * CHUNK_SIZE as f32 * TILE_SIZE.y,
                    0.,
                ));

                commands.entity(tilemap_entity).insert(TilemapBundle {
                    grid_size,
                    map_type,
                    size: tilemap_size,
                    storage: tile_storage,
                    texture: TilemapTexture::Single(sprite_sheet.tiles_sheet.clone()),
                    tile_size,
                    transform,
                    ..Default::default()
                });
                //TODO: add event for this
                GenerationPlugin::spawn_objects(&mut commands, &mut game, chunk_pos);

                minimap_update.send(UpdateMiniMapEvent);
            }
            warn!("Chunk {chunk_pos:?} not in CACHE!");
        }
    }
    fn spawn_and_cache_init_chunks(
        mut cache_event: EventWriter<CacheChunkEvent>,
        mut game: GameParam,
        new_dimension: Query<(&Dimension, Option<&Dungeon>), Added<ActiveDimension>>,
    ) {
        if new_dimension.get_single().is_err() {
            return;
        }

        for transform in game.camera_query.iter() {
            let camera_chunk_pos =
                world_helpers::camera_pos_to_chunk_pos(&transform.translation.xy());
            for y in (camera_chunk_pos.y - CHUNK_CACHE_AMOUNT - 1)
                ..(camera_chunk_pos.y + CHUNK_CACHE_AMOUNT + 1)
            {
                for x in (camera_chunk_pos.x - CHUNK_CACHE_AMOUNT - 1)
                    ..(camera_chunk_pos.x + CHUNK_CACHE_AMOUNT + 1)
                {
                    if let Some(_) = new_dimension.single().1 {
                        if x != 0 || y != 0 {
                            continue;
                        }
                    }
                    if !game.chunk_manager.cached_chunks.contains(&IVec2::new(x, y)) {
                        println!("Init Caching chunk at {x:?} {y:?}");
                        game.chunk_manager.state = ChunkLoadingState::Spawning;
                        cache_event.send(CacheChunkEvent {
                            chunk_pos: IVec2::new(x, y),
                        });
                    }
                }
            }
        }
        game.chunk_manager.state = ChunkLoadingState::None;
    }

    fn spawn_chunks_around_camera(
        mut cache_event: EventWriter<CacheChunkEvent>,
        mut spawn_event: EventWriter<SpawnChunkEvent>,
        mut game: GameParam,
        new_dimension: Query<Option<&Dungeon>, With<ActiveDimension>>,
    ) {
        let transform = game.camera_query.single_mut();
        let camera_chunk_pos = world_helpers::camera_pos_to_chunk_pos(&transform.translation.xy());
        for y in
            (camera_chunk_pos.y - CHUNK_CACHE_AMOUNT)..(camera_chunk_pos.y + CHUNK_CACHE_AMOUNT)
        {
            for x in
                (camera_chunk_pos.x - CHUNK_CACHE_AMOUNT)..(camera_chunk_pos.x + CHUNK_CACHE_AMOUNT)
            {
                if !game.chunk_manager.cached_chunks.contains(&IVec2::new(x, y)) {
                    game.chunk_manager.state = ChunkLoadingState::Caching;
                    cache_event.send(CacheChunkEvent {
                        chunk_pos: IVec2::new(x, y),
                    });
                }
            }
        }
        for y in (camera_chunk_pos.y - NUM_CHUNKS_AROUND_CAMERA)
            ..(camera_chunk_pos.y + NUM_CHUNKS_AROUND_CAMERA)
        {
            for x in (camera_chunk_pos.x - NUM_CHUNKS_AROUND_CAMERA)
                ..(camera_chunk_pos.x + NUM_CHUNKS_AROUND_CAMERA)
            {
                if let Some(_) = new_dimension.single() {
                    if x != 0 || y != 0 {
                        continue;
                    }
                }
                if (!game
                    .chunk_manager
                    .spawned_chunks
                    .contains(&IVec2::new(x, y)))
                    && game.chunk_manager.cached_chunks.contains(&IVec2::new(x, y))
                    && !(game.chunk_manager.state == ChunkLoadingState::Caching)
                {
                    spawn_event.send(SpawnChunkEvent {
                        chunk_pos: IVec2::new(x, y),
                    });
                    game.chunk_manager.spawned_chunks.insert(IVec2::new(x, y));
                }
            }
        }
        game.chunk_manager.state = ChunkLoadingState::None;
    }
    //TODO: change despawning systems to use playe rpos instead??
    fn despawn_outofrange_chunks(
        mut commands: Commands,
        camera_query: Query<&Transform, With<TextureCamera>>,
        chunks_query: Query<(Entity, &Transform), Without<MainCamera>>,
        mut chunk_manager: ResMut<ChunkManager>,
    ) {
        for camera_transform in camera_query.iter() {
            let max_distance = f32::hypot(
                CHUNK_SIZE as f32 * TILE_SIZE.x,
                CHUNK_SIZE as f32 * TILE_SIZE.y,
            );
            for (entity, chunk_transform) in chunks_query.iter() {
                let chunk_pos = chunk_transform.translation.xy();
                let distance = camera_transform.translation.xy().distance(chunk_pos);
                //TODO: calculate maximum possible distance for 2x2 chunksa
                let x = (chunk_pos.x / (CHUNK_SIZE as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y / (CHUNK_SIZE as f32 * TILE_SIZE.y)).floor() as i32;
                if distance > max_distance * 2. * NUM_CHUNKS_AROUND_CAMERA as f32
                    && chunk_manager.spawned_chunks.contains(&IVec2::new(x, y))
                {
                    println!("despawning chunk at {x:?} {y:?} d === {distance:?}");
                    chunk_manager.state = ChunkLoadingState::Despawning;
                    chunk_manager.spawned_chunks.remove(&IVec2::new(x, y));
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        chunk_manager.state = ChunkLoadingState::None;
    }
    fn toggle_on_screen_mesh_visibility(
        camera_query: Query<&Transform, With<TextureCamera>>,
        mut foliage_query: Query<(&mut Visibility, &Transform, &Handle<FoliageMaterial>)>,
        mut chunk_manager: ResMut<ChunkManager>,
    ) {
        for camera_transform in camera_query.iter() {
            for (mut v, ft, _) in foliage_query.iter_mut() {
                let foliage_pos = ft.translation.xy();
                let distance = camera_transform.translation.xy().distance(foliage_pos);

                if v.is_visible && distance > (MAX_VISIBILITY * 2_u32) as f32 {
                    v.is_visible = false;
                } else if !v.is_visible && distance <= (MAX_VISIBILITY * 2_u32) as f32 {
                    v.is_visible = true;
                }
            }
        }
        chunk_manager.state = ChunkLoadingState::None;
    }
}
