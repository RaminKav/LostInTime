use std::hash::Hash;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::{prelude::*, tiles::TilePos};
use bevy_rapier2d::prelude::Collider;

use super::dimension::{dim_spawned, GenerationSeed};

use super::generation::WorldObjectCache;
use super::world_helpers::get_neighbour_tile;

use crate::container::ContainerRegistry;
use crate::player::{handle_move_player, Player};
use crate::ui::{ChestContainer, FurnaceContainer};
use crate::world::wall_auto_tile::ChunkWallCache;
use crate::world::world_helpers::world_pos_to_tile_pos;
use crate::{item::WorldObject, GameParam, ImageAssets};
use crate::{CustomFlush, GameState, TextureCamera};
use serde::{Deserialize, Serialize};

use super::tile::TilePlugin;
use super::{
    world_helpers, TileMapPosition, CHUNK_SIZE, ISLAND_SIZE, MAX_VISIBILITY,
    NUM_CHUNKS_AROUND_CAMERA, TILE_SIZE,
};

pub struct ChunkPlugin;
impl Plugin for ChunkPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnChunkEvent>()
            .add_event::<DespawnChunkEvent>()
            .add_event::<CreateChunkEvent>()
            .add_event::<GenerateObjectsEvent>()
            .add_systems(
                (
                    Self::spawn_chunks_around_camera
                        .after(handle_move_player)
                        .run_if(dim_spawned),
                    Self::handle_new_chunk_event.after(Self::spawn_chunks_around_camera),
                    Self::handle_update_tiles_for_new_chunks.after(CustomFlush),
                    Self::toggle_on_screen_mesh_visibility.before(CustomFlush),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                Self::despawn_outofrange_chunks
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(
                generate_and_cache_island_chunks.run_if(resource_added::<WorldObjectCache>()),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
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

pub fn generate_and_cache_island_chunks(mut game: GameParam, seed: Res<GenerationSeed>) {
    let gen_radius = ((ISLAND_SIZE / CHUNK_SIZE as f32) + 1.) as i32;
    let era = game.era.current_era.clone();
    println!(
        "Caching ALL chunks {:?}",
        game.world_generation_params.water_frequency,
    );
    for y in -gen_radius..=gen_radius {
        for x in -gen_radius..=gen_radius {
            let chunk_pos = IVec2::new(x, y);
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };

                    let (bits, mut index_shift, blocks) = TilePlugin::get_tile_from_perlin_noise(
                        &game.world_generation_params,
                        chunk_pos,
                        tile_pos,
                        seed.seed,
                    );

                    let block_bits = bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8;
                    if index_shift == 0 {
                        index_shift = era.get_texture_index() as u8;
                    }
                    let data = TileSpriteData {
                        tile_bit_index: block_bits,
                        block_type: blocks,
                        raw_block_type: blocks,
                        texture_offset: index_shift,
                    };
                    game.world_obj_cache.tile_data_cache.insert(
                        TileMapPosition {
                            chunk_pos,
                            tile_pos,
                        },
                        data,
                    );
                }
            }
        }
    }
}

impl ChunkPlugin {
    pub fn handle_new_chunk_event(
        mut cache_events: EventReader<CreateChunkEvent>,
        mut commands: Commands,
        sprite_sheet: Res<ImageAssets>,
        game: GameParam,
        seed: Res<GenerationSeed>,
    ) {
        for e in cache_events.iter() {
            let chunk_pos = e.chunk_pos;
            let era = game.era.current_era.clone();

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
            let mut water_colliders = vec![];

            println!("Creating new chunk {chunk_pos:?} with seed {:?}", seed.seed);
            for y in 0..CHUNK_SIZE {
                for x in 0..CHUNK_SIZE {
                    let tile_pos = TilePos { x, y };
                    let tile_data = if let Some(tile_data) = game
                        .world_obj_cache
                        .tile_data_cache
                        .get(&TileMapPosition::new(chunk_pos, tile_pos))
                    {
                        tile_data.clone()
                    } else {
                        // warn!("Tile data not found for {chunk_pos:?} {tile_pos:?}");
                        let (bits, mut index_shift, blocks) =
                            TilePlugin::get_tile_from_perlin_noise(
                                &game.world_generation_params,
                                chunk_pos,
                                tile_pos,
                                seed.seed,
                            );

                        let block_bits = bits[0] + bits[1] * 2 + bits[2] * 4 + bits[3] * 8;
                        if index_shift == 0 {
                            index_shift = era.get_texture_index() as u8;
                        }
                        let data = TileSpriteData {
                            tile_bit_index: block_bits,
                            block_type: blocks,
                            raw_block_type: blocks,
                            texture_offset: index_shift,
                        };
                        data.clone()
                    };
                    let tile_entity = commands
                        .spawn(TileBundle {
                            position: tile_pos,
                            tilemap_id: TilemapId(tilemap_entity),
                            texture_index: TileTextureIndex(
                                (tile_data.tile_bit_index + tile_data.texture_offset).into(),
                            ),
                            ..Default::default()
                        })
                        .insert(tile_data.clone())
                        .id();

                    tiles.insert(tile_pos.into(), tile_entity);
                    commands.entity(tilemap_entity).add_child(tile_entity);
                    tile_storage.set(&tile_pos, tile_entity);

                    // spawn water colliders
                    if tile_data.block_type.contains(&WorldObject::WaterTile) {
                        let mut pos_offset = Vec2::ZERO;
                        if tile_data.block_type[0] == WorldObject::WaterTile {
                            pos_offset += Vec2::new(-3., 3.);
                        }
                        if tile_data.block_type[1] == WorldObject::WaterTile {
                            pos_offset += Vec2::new(3., 3.);
                        }
                        if tile_data.block_type[2] == WorldObject::WaterTile {
                            pos_offset += Vec2::new(-3., -3.);
                        }
                        if tile_data.block_type[3] == WorldObject::WaterTile {
                            pos_offset += Vec2::new(3., -3.);
                        }
                        pos_offset = pos_offset.clamp(Vec2::new(-3., -3.), Vec2::new(3., 3.));
                        water_colliders.push(
                            commands
                                .spawn((
                                    SpatialBundle::from_transform(Transform::from_translation(
                                        Vec3::new(
                                            x as f32 * TILE_SIZE.x,
                                            y as f32 * TILE_SIZE.y,
                                            0.,
                                        ) + pos_offset.extend(0.),
                                    )),
                                    Collider::cuboid(TILE_SIZE.x / 2. - 2., TILE_SIZE.y / 2. - 2.),
                                    Name::new("WATER"),
                                ))
                                .id(),
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
                .insert(Chunk { chunk_pos })
                .insert(Name::new(format!("Pos: {}", chunk_pos)))
                .push_children(&water_colliders)
                .id();
            commands.entity(chunk).insert(ChunkWallCache {
                walls: HashMap::new(),
            });

            // game.set_chunk_entity(chunk_pos, chunk);
            // minimap_update.send(UpdateMiniMapEvent);
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
                    let pos = TileMapPosition::new(chunk_pos, TilePos { x, y });
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
        mut camera_query: Query<&Transform, With<Player>>,
        mut create_chunk_event: EventWriter<CreateChunkEvent>,
        _load_chunk_event: EventWriter<SpawnChunkEvent>,
    ) {
        let transform = camera_query.single_mut();
        let camera_chunk_pos = world_helpers::camera_pos_to_chunk_pos(&transform.translation.xy());
        for y in (camera_chunk_pos.y - NUM_CHUNKS_AROUND_CAMERA)
            ..=(camera_chunk_pos.y + NUM_CHUNKS_AROUND_CAMERA)
        {
            for x in (camera_chunk_pos.x - NUM_CHUNKS_AROUND_CAMERA)
                ..=(camera_chunk_pos.x + NUM_CHUNKS_AROUND_CAMERA)
            {
                let chunk_pos = IVec2::new(x, y);
                if game.get_chunk_entity(chunk_pos).is_none() {
                    // println!("send chunk spawn event {chunk_pos}");
                    create_chunk_event.send(CreateChunkEvent { chunk_pos });
                }
            }
        }
    }
    //TODO: change despawning systems to use playe rpos instead??
    fn despawn_outofrange_chunks(
        game: GameParam,
        camera_query: Query<&Transform, With<Player>>,
        mut commands: Commands,
        chunk_query: Query<(&Transform, &Children), With<Chunk>>,
        containers: Query<(
            &GlobalTransform,
            Option<&FurnaceContainer>,
            Option<&ChestContainer>,
        )>,
        mut container_reg: ResMut<ContainerRegistry>,
    ) {
        for camera_transform in camera_query.iter() {
            let max_distance = f32::hypot(
                CHUNK_SIZE as f32 * TILE_SIZE.x,
                CHUNK_SIZE as f32 * TILE_SIZE.y,
            );
            for (chunk_transform, children) in chunk_query.iter() {
                let chunk_pos = chunk_transform.translation.xy();
                let distance = camera_transform.translation.xy().distance(chunk_pos);
                //TODO: calculate maximum possible distance for 2x2 chunksa
                let x = (chunk_pos.x / (CHUNK_SIZE as f32 * TILE_SIZE.x)).floor() as i32;
                let y = (chunk_pos.y / (CHUNK_SIZE as f32 * TILE_SIZE.y)).floor() as i32;
                if distance > max_distance * 2. * NUM_CHUNKS_AROUND_CAMERA as f32
                    && game.get_chunk_entity(IVec2::new(x, y)).is_some()
                {
                    println!("            despawning chunk {x:?},{y:?}");

                    // add all containers in this chunk into the registry so their contents are safe
                    for child in children.iter() {
                        if let Ok((t, furnace_option, chest_option)) = containers.get(*child) {
                            if let Some(furnace) = furnace_option {
                                println!(
                                    "furnace: {:?}",
                                    world_pos_to_tile_pos(t.translation().xy())
                                );
                                container_reg.containers.insert(
                                    world_pos_to_tile_pos(t.translation().xy()),
                                    furnace.items.clone(),
                                );
                            }
                            if let Some(chest) = chest_option {
                                println!(
                                    "chest: {:?}",
                                    world_pos_to_tile_pos(t.translation().xy())
                                );

                                container_reg.containers.insert(
                                    world_pos_to_tile_pos(t.translation().xy()),
                                    chest.items.clone(),
                                );
                            }
                        }
                    }
                    commands
                        .entity(game.get_chunk_entity(IVec2::new(x, y)).unwrap())
                        .despawn_recursive();
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
