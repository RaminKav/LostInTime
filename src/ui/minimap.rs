use crate::assets::Graphics;

use crate::colors::LIGHT_RED;
use crate::enemy::Mob;
use crate::item::WorldObject;
use crate::world::dimension::SpawnDimension;
use crate::world::world_helpers::{camera_pos_to_chunk_pos, camera_pos_to_tile_pos};
use crate::world::{TileMapPosition, CHUNK_SIZE};
use crate::{CustomFlush, GameParam, GameState, Player, GAME_HEIGHT, MINIMAP};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;
use bevy::utils::HashMap;
use bevy_ecs_tilemap::prelude::*;

use super::UIElement;
pub struct MinimapPlugin;
//TODO: temp disable minimap
impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MinimapTileCache::default())
            .add_event::<UpdateMiniMapEvent>()
            .add_systems(
                (update_minimap_cache, clear_cache_for_new_dimensions)
                    .in_set(Update(GameState::Main)),
            )
            .add_system(
                setup_mini_map
                    .after(CustomFlush)
                    .in_set(Update(GameState::Main)),
            );
    }
}

#[derive(Debug, Clone, Event)]
pub struct UpdateMiniMapEvent {
    pub pos: Option<TileMapPosition>,
    pub new_tile: Option<WorldObject>,
}

#[derive(Component)]
pub struct Minimap;
#[derive(Resource, Default)]
pub struct MinimapTileCache {
    pub cache: HashMap<TileMapPosition, WorldObject>,
}

//TODO: Optimize this to not run if player does not move over a tile, maybe w resource to track
//TODO: Fix Caching mob positions, they dont show when behind a tile
fn update_minimap_cache(
    mut minimap_update: EventReader<UpdateMiniMapEvent>,
    mut cache: ResMut<MinimapTileCache>,
) {
    if !*MINIMAP {
        return;
    }
    for event in minimap_update.iter() {
        let Some(pos) = event.pos else {
            //TODO: find another way to trigger change detection
            let _ = &mut cache.cache;
            continue;
        };
        if let Some(new_tile) = event.new_tile {
            if new_tile != WorldObject::None {
                cache.cache.insert(pos, new_tile);
            } else {
                cache.cache.remove(&pos);
            }
        } else {
            cache.cache.remove(&pos);
        }
    }
}
fn clear_cache_for_new_dimensions(
    new_dim: Query<Entity, Added<SpawnDimension>>,
    mut cache: ResMut<MinimapTileCache>,
) {
    if !*MINIMAP {
        return;
    }
    for _ in new_dim.iter() {
        cache.cache = HashMap::new();
    }
}
fn setup_mini_map(
    mut commands: Commands,
    graphics: Res<Graphics>,
    mut assets: ResMut<Assets<Image>>,
    mut color_mat: ResMut<Assets<ColorMaterial>>,
    game: GameParam,
    minimap_cache: Res<MinimapTileCache>,
    old_map: Query<Entity, With<Minimap>>,
    p_t: Query<&Transform, With<Player>>,
    mob_t: Query<&GlobalTransform, (With<Mob>, Changed<GlobalTransform>)>,
    mut meshes: ResMut<Assets<Mesh>>,
    // mut cache: ResMut<MinimapTileCache>,
) {
    //NOTES:
    // construct an array of tiles based on whats around the player
    // 16 tiles in each direction
    //set focus point to player's C/T positions
    // iterate an offset from -16..16 on x/y
    // check player's C/T + offset.
    // if we pass chunk boundary, get next chunk

    //TODO: for better performance... ??
    // option 1: Events send a msg with a tile pos and its new color?
    //           need a helper fn to convert the tile pos to data vec index
    // option 2: keep track of every tile color in a vector and update the map every frame with it
    //           events would mutate this vector

    if !*MINIMAP {
        return;
    }
    if minimap_cache.is_changed() {
        for old_map in old_map.iter() {
            commands.entity(old_map).despawn_recursive();
        }
        let num_tiles = 32;
        let mut offset = 0;
        let size = Extent3d {
            width: num_tiles * 2,
            height: num_tiles * 2,
            depth_or_array_layers: 1,
        };
        let pt = p_t.single();
        let p_cp = camera_pos_to_chunk_pos(&pt.translation.truncate());
        let p_tp: TilePos = camera_pos_to_tile_pos(&pt.translation.truncate());
        let mobs: Vec<_> = mob_t
            .iter()
            .map(|t| {
                (
                    camera_pos_to_chunk_pos(&t.translation().truncate()),
                    camera_pos_to_tile_pos(&t.translation().truncate()),
                )
            })
            .collect();

        //Every pixel is 4 entries in image.data
        let mut data = Vec::with_capacity(16384);

        for y in (-(num_tiles as i32 / 2)..num_tiles as i32 / 2).rev() {
            for _ in 0..2 {
                for x in -(num_tiles as i32 / 2)..num_tiles as i32 / 2 {
                    for q in 0..2 {
                        if x == 0 && y == 0 {
                            data.push(0);
                            data.push(0);
                            data.push(0);
                            data.push(255);
                            continue;
                        }
                        let mut chunk_pos = p_cp;
                        let mut tile_x = p_tp.x as i32 + x;
                        let mut tile_y = p_tp.y as i32 + y;

                        while tile_x >= CHUNK_SIZE as i32 {
                            tile_x -= CHUNK_SIZE as i32;
                            chunk_pos.x += 1;
                        }
                        while tile_x < 0 {
                            tile_x += CHUNK_SIZE as i32;
                            chunk_pos.x -= 1;
                        }
                        while tile_y >= CHUNK_SIZE as i32 {
                            tile_y -= CHUNK_SIZE as i32;
                            chunk_pos.y += 1;
                        }
                        while tile_y < 0 {
                            tile_y += CHUNK_SIZE as i32;
                            chunk_pos.y -= 1;
                        }
                        let tile_pos = TilePos {
                            x: tile_x as u32,
                            y: (tile_y) as u32,
                        };
                        if let Some(cached_tile) = minimap_cache
                            .cache
                            .get(&TileMapPosition::new(chunk_pos, tile_pos))
                        {
                            let c = cached_tile.get_obj_color();

                            data.push((c.r() * 255.) as u8);
                            data.push((c.g() * 255.) as u8);
                            data.push((c.b() * 255.) as u8);
                            data.push(255);
                            continue;
                        }

                        if let Some(tile_data) =
                            game.get_tile_data(TileMapPosition::new(chunk_pos, tile_pos))
                        {
                            let tile = tile_data.block_type;
                            if mobs.contains(&(chunk_pos, tile_pos)) {
                                let c = LIGHT_RED;
                                data.push((c.r() * 255.) as u8);
                                data.push((c.g() * 255.) as u8);
                                data.push((c.b() * 255.) as u8);
                                data.push(255);
                                continue;
                            }
                            //Copy 1 pixel at index 0,1 2,3
                            let c = tile[(q + offset) as usize].get_obj_color();

                            data.push((c.r() * 255.) as u8);
                            data.push((c.g() * 255.) as u8);
                            data.push((c.b() * 255.) as u8);
                            data.push(255);
                        } else {
                            //Unloaded chunk, spawn nothing for now
                            data.push(0);
                            data.push(0);
                            data.push(0);
                            data.push(0);
                        }
                    }
                }
                offset = 2;
            }
            offset = 0;
        }
        let image = Image::new(
            size,
            TextureDimension::D2,
            data,
            //FIXME
            TextureFormat::Rgba8UnormSrgb,
        );
        let handle = assets.add(image);
        let mat = color_mat.add(ColorMaterial::from(handle));

        let map_border = commands
            .spawn(SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::Minimap),

                sprite: Sprite {
                    custom_size: Some(Vec2::new(
                        ((num_tiles + 1) * 2) as f32,
                        ((num_tiles + 1) * 2) as f32,
                    )),

                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(
                    (game.resolution.game_width - ((num_tiles + 1) * 2) as f32) / 2. - 2.,
                    (GAME_HEIGHT - ((num_tiles + 1) * 2 + 1) as f32) / 2. - 2.,
                    1.,
                )),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(Minimap)
            .insert(Name::new("MAP"))
            .id();
        let map = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: meshes.add(Rectangle::new(
                        (num_tiles * 2) as f32,
                        (num_tiles * 2) as f32,
                    )),
                    transform: Transform::from_translation(Vec3::new(0., 0., 2.)),
                    material: mat,
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ))
            .id();
        commands.entity(map_border).add_child(map);
    }
}
