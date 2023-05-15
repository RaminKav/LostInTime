use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};
use bevy_pkv::PkvStore;

use super::dimension::{ActiveDimension, Dimension, GenerationSeed};
use super::dungeon::Dungeon;
use super::generation::GenerationPlugin;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::{assets::FoliageMaterial, item::WorldObject, GameParam, ImageAssets};
use crate::{CustomFlush, GameState, MainCamera, TextureCamera};

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
            .add_systems(
                (
                    Self::spawn_and_cache_init_chunks.before(Self::handle_spawn_chunk_event),
                    Self::handle_spawn_chunk_event
                        .after(Self::handle_cache_chunk_event)
                        .before(CustomFlush),
                    Self::handle_cache_chunk_event,
                    Self::spawn_chunks_around_camera,
                    Self::despawn_outofrange_chunks,
                    Self::toggle_on_screen_mesh_visibility,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}
#[derive(Component)]
pub struct SpawnedChunk;
#[derive(Eq, Hash, Component, PartialEq, Debug, Clone)]
pub struct TileSpriteData {
    pub block_type: [WorldObject; 4],
    pub raw_block_type: [WorldObject; 4],
    pub tile_bit_index: u8,
    pub texture_offset: u8,
}
#[derive(Component, Debug)]
pub struct SpawnedObject(pub Entity);
#[derive(Clone)]
pub struct SpawnChunkEvent {
    pub chunk_pos: IVec2,
}

#[derive(Clone)]
pub struct CacheChunkEvent {
    chunk_pos: IVec2,
}
#[derive(Component, Debug, Clone)]
pub struct TileEntityCollection {
    pub map: HashMap<TilePos, Entity>,
}
#[derive(Component, Debug, Clone)]
pub struct Chunk;

impl ChunkPlugin {
    fn handle_cache_chunk_event(
        mut cache_events: EventReader<CacheChunkEvent>,
        mut commands: Commands,
        mut game: GameParam,
        // mut pkv: ResMut<PkvStore>,
        seed: Query<&GenerationSeed, With<ActiveDimension>>,
    ) {
        // for e in cache_events.iter() {
        //     let chunk_pos = e.chunk_pos;
        //     if game.chunk_manager.cached_chunks.contains(&chunk_pos) {
        //         continue;
        //     }
        //     game.chunk_manager.cached_chunks.insert(chunk_pos);

        //     // for y in 0..CHUNK_SIZE {
        //     //     for x in 0..CHUNK_SIZE {
        //     //         let tile_pos = TilePos { x, y };
        //     //         game.chunk_manager.chunk_tile_entity_data.insert(
        //     //             TileMapPositionData {
        //     //                 chunk_pos,
        //     //                 tile_pos,
        //     //             },
        //     //             TileEntityData {
        //     //                 entity: None,
        //     //                 tile_bit_index: 0b0000,
        //     //                 block_type: [WorldObject::Sand; 4],
        //     //                 texture_offset: 0,
        //     //             },
        //     //         );
        //     //     }
        //     // }

        //     let mut raw_chunk_bits: [[[u8; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize] =
        //         [[[0; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        //     let mut raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize];
        //         CHUNK_SIZE as usize] =
        //         [[[WorldObject::Sand; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
        //     for y in 0..CHUNK_SIZE {
        //         for x in 0..CHUNK_SIZE {
        //             let tile_pos = TilePos { x, y };

        //             let (bits, index_shift, blocks) = TilePlugin::get_tile_from_perlin_noise(
        //                 &game.chunk_manager,
        //                 chunk_pos,
        //                 tile_pos,
        //                 seed.single().seed,
        //             );

        //             raw_chunk_bits[x as usize][y as usize] = bits;
        //             raw_chunk_blocks[x as usize][y as usize] = blocks;
        //             let block_bits = bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8;

        //             game.chunk_manager.chunk_tile_entity_data.insert(
        //                 TileMapPositionData {
        //                     chunk_pos,
        //                     tile_pos,
        //                 },
        //                 TileEntityData {
        //                     entity: None,
        //                     tile_bit_index: block_bits,
        //                     block_type: blocks,
        //                     texture_offset: index_shift,
        //                 },
        //             );
        //         }
        //     }
        //     game.chunk_manager.raw_chunk_data.insert(
        //         chunk_pos,
        //         RawChunkData {
        //             raw_chunk_bits,
        //             raw_chunk_blocks,
        //         },
        //     );
        //     for y in 0..CHUNK_SIZE {
        //         for x in 0..CHUNK_SIZE {
        //             if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Water)
        //                 && raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Sand)
        //             {
        //                 TilePlugin::update_neighbour_tiles(
        //                     TilePos { x, y },
        //                     &mut commands,
        //                     &mut game.chunk_manager,
        //                     chunk_pos,
        //                     false,
        //                 );
        //             } else if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Grass)
        //             {
        //                 TilePlugin::update_this_tile(
        //                     TilePos { x, y },
        //                     16,
        //                     &mut game.chunk_manager,
        //                     chunk_pos,
        //                 );
        //             }
        //         }
        //     }
        //     //TODO: add event for this
        //     GenerationPlugin::generate_and_cache_objects(
        //         &mut game,
        //         &mut pkv,
        //         chunk_pos,
        //         seed.single().seed,
        //     );
        // }
    }
    fn handle_spawn_chunk_event(
        mut spawn_events: EventReader<SpawnChunkEvent>,
        mut commands: Commands,
        sprite_sheet: Res<ImageAssets>,
        mut game: GameParam,
        mut minimap_update: EventWriter<UpdateMiniMapEvent>,
        // mut pkv: ResMut<PkvStore>,
        seed: Query<&GenerationSeed, With<ActiveDimension>>,
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
            let mut tiles = HashMap::default();
            let mut tile_storage = TileStorage::empty(tilemap_size);

            let mut raw_chunk_blocks: [[[WorldObject; 4]; CHUNK_SIZE as usize];
                CHUNK_SIZE as usize] =
                [[[WorldObject::Sand; 4]; CHUNK_SIZE as usize]; CHUNK_SIZE as usize];
            println!("Spawning new chunk {chunk_pos:?}");

            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };
                    let (bits, index_shift, blocks) = TilePlugin::get_tile_from_perlin_noise(
                        &game.chunk_manager,
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

                    tiles.insert(tile_pos, tile_entity);
                    commands.entity(tilemap_entity).add_child(tile_entity);
                    tile_storage.set(&tile_pos, tile_entity);
                }
            }

            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Water)
                        && raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Sand)
                    {
                        TilePlugin::update_neighbour_tiles(
                            TilePos { x, y },
                            &mut commands,
                            &mut game,
                            chunk_pos,
                            false,
                        );
                    } else if raw_chunk_blocks[x as usize][y as usize].contains(&WorldObject::Grass)
                    {
                        TilePlugin::update_this_tile(
                            &mut commands,
                            TilePos { x, y },
                            16,
                            &mut game,
                            chunk_pos,
                        );
                    }
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
                .insert(Chunk)
                .id();
            game.set_chunk_entity(chunk_pos, chunk);
            //TODO: add event for this
            GenerationPlugin::generate_and_cache_objects(
                &mut game,
                // &mut pkv,
                chunk_pos,
                seed.single().seed,
            );

            GenerationPlugin::spawn_objects(&mut commands, &mut game, chunk_pos);

            minimap_update.send(UpdateMiniMapEvent);

            warn!("Chunk {chunk_pos:?} not in CACHE!");
        }
    }
    fn spawn_and_cache_init_chunks(
        mut spawn_event: EventWriter<SpawnChunkEvent>,
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
                    // game.chunk_manager.state = ChunkLoadingState::Spawning;
                    spawn_event.send(SpawnChunkEvent {
                        chunk_pos: IVec2::new(x, y),
                    });
                }
            }
        }
        // game.chunk_manager.state = ChunkLoadingState::None;
    }

    fn spawn_chunks_around_camera(
        mut cache_event: EventWriter<CacheChunkEvent>,
        mut spawn_event: EventWriter<SpawnChunkEvent>,
        mut game: GameParam,
    ) {
        let transform = game.camera_query.single_mut();
        let camera_chunk_pos = world_helpers::camera_pos_to_chunk_pos(&transform.translation.xy());
        // for y in
        //     (camera_chunk_pos.y - CHUNK_CACHE_AMOUNT)..(camera_chunk_pos.y + CHUNK_CACHE_AMOUNT)
        // {
        //     for x in
        //         (camera_chunk_pos.x - CHUNK_CACHE_AMOUNT)..(camera_chunk_pos.x + CHUNK_CACHE_AMOUNT)
        //     {
        //         if !game.chunk_manager.cached_chunks.contains(&IVec2::new(x, y)) {
        //             game.chunk_manager.state = ChunkLoadingState::Caching;
        //             cache_event.send(CacheChunkEvent {
        //                 chunk_pos: IVec2::new(x, y),
        //             });
        //         }
        //     }
        // }
        for y in (camera_chunk_pos.y - NUM_CHUNKS_AROUND_CAMERA)
            ..(camera_chunk_pos.y + NUM_CHUNKS_AROUND_CAMERA)
        {
            for x in (camera_chunk_pos.x - NUM_CHUNKS_AROUND_CAMERA)
                ..(camera_chunk_pos.x + NUM_CHUNKS_AROUND_CAMERA)
            {
                if (game.get_chunk_entity(IVec2::new(x, y))).is_none()
                // && !(game.chunk_manager.state == ChunkLoadingState::Caching)
                {
                    spawn_event.send(SpawnChunkEvent {
                        chunk_pos: IVec2::new(x, y),
                    });
                }
            }
        }
        // game.chunk_manager.state = ChunkLoadingState::None;
    }
    //TODO: change despawning systems to use playe rpos instead??
    fn despawn_outofrange_chunks(mut commands: Commands, mut game: GameParam) {
        let mut removed_chunks = vec![];
        for camera_transform in game.camera_query.iter() {
            let max_distance = f32::hypot(
                CHUNK_SIZE as f32 * TILE_SIZE.x,
                CHUNK_SIZE as f32 * TILE_SIZE.y,
            );
            for (entity, chunk_transform, _) in game.chunk_query.iter() {
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
                    commands.entity(entity).despawn_recursive();
                }
            }
        }
        for chunk_pos in removed_chunks.iter() {
            game.remove_chunk_entity(*chunk_pos);
        }
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

                if *v == Visibility::Inherited && distance > (MAX_VISIBILITY * 2_u32) as f32 {
                    *v = Visibility::Hidden;
                } else if *v != Visibility::Inherited && distance <= (MAX_VISIBILITY * 2_u32) as f32
                {
                    *v = Visibility::Inherited;
                }
            }
        }
        // chunk_manager.state = ChunkLoadingState::None;
    }
}
