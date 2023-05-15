use crate::assets::Graphics;
use crate::world::world_helpers::{camera_pos_to_block_pos, camera_pos_to_chunk_pos};
use crate::world::{ChunkManager, TileMapPositionData, CHUNK_SIZE};
use crate::{GameParam, GameState, Player, GAME_HEIGHT, GAME_WIDTH, TIME_STEP};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;
use bevy_ecs_tilemap::prelude::*;

use super::UIElement;
pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<UpdateMiniMapEvent>()
            .add_system(Self::setup_mini_map.in_set(OnUpdate(GameState::Main)));
    }
}

#[derive(Debug, Clone)]
pub struct UpdateMiniMapEvent;

#[derive(Component)]
pub struct Minimap;

//TODO: Optimize this to not run if player does not move over a tile, maybe w resource to track
impl MinimapPlugin {
    fn setup_mini_map(
        mut commands: Commands,
        graphics: Res<Graphics>,
        mut assets: ResMut<Assets<Image>>,
        mut color_mat: ResMut<Assets<ColorMaterial>>,
        mut game: GameParam,
        mut minimap_update: EventReader<UpdateMiniMapEvent>,
        old_map: Query<Entity, With<Minimap>>,
        p_t: Query<&Transform, With<Player>>,
    ) {
        //NOTES:
        // construct an array of tiles based on whats around the player
        // 16 tiles in each direction
        //set focus point to player's C/T positions
        // iterate an offset from -16..16 on x/y
        // check player's C/T + offset.
        // if we pass chunk boundary, get next chunk
        if minimap_update.len() > 0 {
            minimap_update.clear();
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
            let p_tp = camera_pos_to_block_pos(&pt.translation.truncate());
            let mut data = Vec::default();
            //Every pixel is 4 entries in image.data

            for y in (-(num_tiles as i32 / 2)..num_tiles as i32 / 2).rev() {
                for _ in 0..2 {
                    for x in -(num_tiles as i32 / 2)..num_tiles as i32 / 2 {
                        if x == 0 && y == 0 {
                            for _ in 0..2 {
                                data.push(0);
                                data.push(0);
                                data.push(0);
                                data.push(255);
                            }
                            continue;
                        }
                        let mut chunk_pos = p_cp;
                        let mut tile_x = p_tp.x as i32 + x;
                        let mut tile_y = p_tp.y as i32 + y;
                        while tile_x >= CHUNK_SIZE as i32 {
                            tile_x = tile_x - CHUNK_SIZE as i32;
                            chunk_pos.x += 1;
                        }
                        while tile_x < 0 {
                            tile_x = CHUNK_SIZE as i32 + tile_x;
                            chunk_pos.x -= 1;
                        }
                        while tile_y >= CHUNK_SIZE as i32 {
                            tile_y = tile_y - CHUNK_SIZE as i32;
                            chunk_pos.y += 1;
                        }
                        while tile_y < 0 {
                            tile_y = CHUNK_SIZE as i32 + tile_y;
                            chunk_pos.y -= 1;
                        }
                        let tile_pos = TilePos {
                            x: tile_x as u32,
                            y: (tile_y) as u32,
                        };
                        if let Some(tile_data) = game.get_tile_data(TileMapPositionData {
                            chunk_pos,
                            tile_pos,
                        }) {
                            let mut tile = tile_data.block_type;

                            if let Some(obj_data) = game.get_tile_obj_data(TileMapPositionData {
                                tile_pos,
                                chunk_pos,
                            }) {
                                let obj_type = obj_data.object;
                                tile = [obj_type; 4];
                            }

                            for i in 0..2 {
                                //Copy 1 pixel at index 0,1 2,3
                                let c = tile[i + offset].get_minimap_color();
                                data.push(c.0);
                                data.push(c.1);
                                data.push(c.2);
                                data.push(255);
                            }
                        } else {
                            println!("CHUNK NOT LOADED! {chunk_pos:?} {tile_pos:?}");
                            for _ in 0..2 {
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
                    texture: graphics
                        .ui_image_handles
                        .as_ref()
                        .unwrap()
                        .get(&UIElement::Minimap)
                        .unwrap()
                        .clone(),
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(
                            ((num_tiles + 1) * 2) as f32,
                            ((num_tiles + 1) * 2) as f32,
                        )),

                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(
                        (GAME_WIDTH - ((num_tiles + 1) * 2) as f32) / 2.,
                        (GAME_HEIGHT - ((num_tiles + 1) * 2 + 1) as f32) / 2.,
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
                        mesh: game
                            .meshes
                            .add(
                                shape::Quad {
                                    size: Vec2::new((num_tiles * 2) as f32, (num_tiles * 2) as f32),
                                    ..Default::default()
                                }
                                .into(),
                            )
                            .into(),
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
}
