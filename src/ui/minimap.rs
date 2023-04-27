use crate::world::{ChunkManager, TileMapPositionData, CHUNK_SIZE};
use crate::{GameState, GAME_HEIGHT, GAME_WIDTH};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::RenderLayers;
use bevy::sprite::MaterialMesh2dBundle;
use bevy_ecs_tilemap::prelude::*;
pub struct MinimapPlugin;

impl Plugin for MinimapPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<UpdateMiniMapEvent>().add_system_set(
            SystemSet::on_update(GameState::Main).with_system(Self::setup_mini_map),
        );
    }
}
#[derive(Debug, Clone)]
pub struct UpdateMiniMapEvent;
impl MinimapPlugin {
    fn setup_mini_map(
        mut commands: Commands,
        mut assets: ResMut<Assets<Image>>,
        mut color_mat: ResMut<Assets<ColorMaterial>>,
        mut meshes: ResMut<Assets<Mesh>>,
        cm: Res<ChunkManager>,
        mut minimap_update: EventReader<UpdateMiniMapEvent>,
    ) {
        for _ in minimap_update.iter() {
            let num_tiles = 64;
            let mut offset = 0;
            let size = Extent3d {
                width: num_tiles * 2,
                height: num_tiles * 2,
                depth_or_array_layers: 1,
            };
            let mut data = Vec::default();
            //Every pixel is 4 entries in image.data

            for y in 0..num_tiles as u32 {
                // 0, 1, 2 ... 32

                for _ in 0..2 {
                    for x in 0..num_tiles as u32 {
                        let tile = cm
                            .chunk_tile_entity_data
                            .get(&TileMapPositionData {
                                chunk_pos: IVec2 { x: 0, y: 0 },
                                tile_pos: TilePos {
                                    x,
                                    y: CHUNK_SIZE - 1 - y,
                                },
                            })
                            .unwrap()
                            .block_type;
                        for i in 0..2 {
                            //Copy 1 pixel at index 0,1 2,3
                            let c = tile[i + offset].get_minimap_color();
                            data.push(c.0);
                            data.push(c.1);
                            data.push(c.2);
                            data.push(255);
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
            commands.spawn((
                MaterialMesh2dBundle {
                    mesh: meshes
                        .add(
                            shape::Quad {
                                size: Vec2::new((num_tiles * 2) as f32, (num_tiles * 2) as f32),
                                ..Default::default()
                            }
                            .into(),
                        )
                        .into(),
                    transform: Transform::from_translation(Vec3::new(
                        (GAME_WIDTH - (num_tiles * 2) as f32) / 2.,
                        (GAME_HEIGHT - (num_tiles * 2) as f32) / 2.,
                        1.,
                    )),
                    material: mat,
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
            ));
        }
    }
}
