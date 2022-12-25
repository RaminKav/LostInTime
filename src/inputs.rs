use std::time::Duration;

use bevy::time::FixedTimestep;
use bevy::{prelude::*, sprite::MaterialMesh2dBundle};

use crate::{
    assets::{Graphics, WORLD_SCALE},
    gi::{LightOccluder, LightSource},
    item::WorldObject,
    world_generation::{ChunkManager, WorldGenerationPlugin, CHUNK_SIZE},
    AnimationTimer, Game, GameState, Player, PLAYER_DASH_SPEED, PLAYER_MOVE_SPEED, TIME_STEP,
};
use crate::{MainCamera, SCREEN_SIZE};

#[derive(Default, Resource)]
pub struct CursorPos(Vec3);

#[derive(Component)]
pub struct Direction(pub f32);

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos(Vec3::new(0.0, 0.0, 0.0)))
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::move_player)
                    .with_system(Self::update_cursor_pos.after(Self::move_player))
                    .with_system(Self::mouse_click_system),
            );
    }
}

impl InputsPlugin {
    fn move_player(
        mut key_input: ResMut<Input<KeyCode>>,
        cursor_pos: Res<CursorPos>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        mut game: ResMut<Game>,
        mut commands: Commands,

        mut player_query: Query<
            (&mut Transform, &mut Direction),
            (With<Player>, Without<MainCamera>),
        >,
        mut camera_query: Query<&mut Transform, (With<MainCamera>, Without<Player>)>,
        time: Res<Time>,
    ) {
        let (mut player_transform, mut dir) = player_query.single_mut();
        let mut camera_transform = camera_query.single_mut();

        let mut dx = 0.0;
        let mut dy = 0.0;
        let s = 1.0 / WORLD_SCALE;

        if key_input.pressed(KeyCode::A) {
            dx -= s;
            game.player.is_moving = true;
            player_transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
        }
        if key_input.pressed(KeyCode::D) {
            dx += s;
            game.player.is_moving = true;
            player_transform.rotation = Quat::default();
        }
        if key_input.pressed(KeyCode::W) {
            dy += s;
            game.player.is_moving = true;
        }
        if key_input.pressed(KeyCode::S) {
            dy -= s;
            game.player.is_moving = true;
        }
        if game.player_dash_cooldown.tick(time.delta()).finished() {
            if key_input.pressed(KeyCode::Space) {
                game.player.is_dashing = true;

                game.player_dash_cooldown.reset();
                game.player_dash_duration.reset();
            }
        }
        if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
            || (dx == 0. && dy == 0.)
        {
            game.player.is_moving = false;
            // key_input.release_all();
        }

        let mut px = player_transform.translation.x + dx * PLAYER_MOVE_SPEED * TIME_STEP;
        let mut py = player_transform.translation.y + dy * PLAYER_MOVE_SPEED * TIME_STEP;

        if game.player.is_dashing {
            game.player_dash_duration.tick(time.delta());

            px += dx * PLAYER_DASH_SPEED * TIME_STEP;
            py += dy * PLAYER_DASH_SPEED * TIME_STEP;
            if game.player_dash_duration.just_finished() {
                game.player.is_dashing = false;
            }
        }
        player_transform.translation.x = px;
        player_transform.translation.y = py;
        camera_transform.translation.x = px;
        camera_transform.translation.y = py;
        if game.player.is_moving == true {
            // println!(
            //     "Player is on chunk {:?} at pos: {:?}",
            //     WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
            //         player_transform.translation.x,
            //         player_transform.translation.y
            //     )),
            //     player_transform.translation
            // );
        }

        if dx != 0. {
            dir.0 = dx;
        }
        if key_input.pressed(KeyCode::L) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            )));
            let block_mesh = meshes.add(Mesh::from(shape::Circle::default()));

            commands
                .spawn(MaterialMesh2dBundle {
                    mesh: block_mesh.clone().into(),
                    material: materials.add(ColorMaterial::from(Color::YELLOW)).into(),
                    transform: Transform {
                        translation: Vec3::new(
                            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                            (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                            200 as f32
                                - (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32)
                                    as f32
                                    / (SCREEN_SIZE.1 as f32),
                        ),
                        scale: Vec3::splat(8.0),

                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(Name::new("cursor_light"))
                .insert(LightSource {
                    intensity: 1.5,
                    radius: 8.0,
                    color: Color::rgb_u8(219, 104, 72),
                    falloff: Vec3::new(10.0, 4.0, 0.05),
                    ..default()
                });
        }
    }

    pub fn update_cursor_pos(
        windows: Res<Windows>,
        camera_q: Query<(&Transform, &Camera), (With<MainCamera>)>,
        mut cursor_moved_events: EventReader<CursorMoved>,
        mut cursor_pos: ResMut<CursorPos>,
    ) {
        for cursor_moved in cursor_moved_events.iter() {
            // To get the mouse's world position, we have to transform its window position by
            // any transforms on the camera. This is done by projecting the cursor position into
            // camera space (world space).
            for (cam_t, cam) in camera_q.iter() {
                *cursor_pos = CursorPos(Self::cursor_pos_in_world(
                    &windows,
                    cursor_moved.position,
                    cam_t,
                    cam,
                ));
            }
        }
    }
    // Converts the cursor position into a world position, taking into account any transforms applied
    // the camera.
    pub fn cursor_pos_in_world(
        windows: &Windows,
        cursor_pos: Vec2,
        cam_t: &Transform,
        cam: &Camera,
    ) -> Vec3 {
        let window = windows.primary();

        let window_size = Vec2::new(window.width(), window.height());

        // Convert screen position [0..resolution] to ndc [-1..1]
        // (ndc = normalized device coordinates)
        let ndc_to_world = cam_t.compute_matrix() * cam.projection_matrix().inverse();
        let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
        ndc_to_world.project_point3(ndc.extend(0.0))
    }

    fn mouse_click_system(
        mouse_button_input: Res<Input<MouseButton>>,
        cursor_pos: Res<CursorPos>,
        mut chunk_manager: ResMut<ChunkManager>,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
        mut commands: Commands,
        graphics: Res<Graphics>,
    ) {
        if mouse_button_input.just_released(MouseButton::Left) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            // let stone = WorldObject::StoneHalf.spawn(
            //     &mut commands,
            //     &graphics,
            //     Vec3::new(
            //         (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            //         (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
            //         200 as f32
            //             - (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32
            //                 / (SCREEN_SIZE.1 as f32),
            //     ),
            // );

            let sprite = graphics
                .item_map
                .as_ref()
                .unwrap()
                .get(&WorldObject::StoneTop)
                .expect(&format!(
                    "No graphic for object {:?}",
                    WorldObject::StoneTop
                ))
                .0
                .clone();
            commands
                .spawn(SpriteSheetBundle {
                    sprite,
                    texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                    transform: Transform {
                        translation: Vec3::new(
                            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                            (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                            200 as f32
                                - (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32)
                                    as f32
                                    / (SCREEN_SIZE.1 as f32),
                        ),
                        // scale: Vec2::splat(1.0).extend(0.0),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(LightOccluder {
                    h_size: Vec2::new(16., 32.),
                })
                .insert(Name::new("Stone Wall"));
            // .push_children(&children)
            // .push_children(&[stone]);
            // WorldGenerationPlugin::change_tile_and_update_neighbours(
            //     TilePos {
            //         x: tile_pos.x as u32,
            //         y: tile_pos.y as u32,
            //     },
            //     chunk_pos,
            //     0b0000,
            //     0,
            //     &mut chunk_manager,
            //     &mut commands,
            // );
        }
        if mouse_button_input.just_released(MouseButton::Right) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            )));
            // let stone = WorldObject::StoneTop.spawn(
            //     &mut commands,
            //     &graphics,
            //     Vec3::new(
            //         (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
            //         (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
            //         0.1,
            //     ),
            // );
            // commands
            //     .spawn(SpatialBundle::default())
            //     // .insert(Name::new("Test Objects"))
            //     // .push_children(&children)
            //     .push_children(&[stone]);
            let sprite = graphics
                .item_map
                .as_ref()
                .unwrap()
                .get(&WorldObject::StoneHalf)
                .expect(&format!(
                    "No graphic for object {:?}",
                    WorldObject::StoneHalf
                ))
                .0
                .clone();
            commands
                .spawn(SpriteSheetBundle {
                    sprite,
                    texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                    transform: Transform {
                        translation: Vec3::new(
                            (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                            (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                            200 as f32
                                - (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32)
                                    as f32
                                    / (SCREEN_SIZE.1 as f32),
                        ),
                        // scale: Vec2::splat(1.0).extend(0.0),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(LightOccluder {
                    h_size: Vec2::new(16., 32.),
                })
                .insert(Name::new("Stone Wall"));
            // WorldGenerationPlugin::change_tile_and_update_neighbours(
            //     TilePos {
            //         x: tile_pos.x as u32,
            //         y: tile_pos.y as u32,
            //     },
            //     chunk_pos,
            //     0b0000,
            //     16,
            //     &mut chunk_manager,
            //     &mut commands,
            // );
        }
        if mouse_button_input.just_released(MouseButton::Middle) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            )));
            let stone = WorldObject::StoneFull.spawn(
                &mut commands,
                &graphics,
                Vec3::new(
                    (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                    (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                    0.1,
                ),
            );
            commands
                .spawn(SpatialBundle::default())
                // .insert(Name::new("Test Objects"))
                // .push_children(&children)
                .push_children(&[stone]);
            // WorldGenerationPlugin::change_tile_and_update_neighbours(
            //     TilePos {
            //         x: tile_pos.x as u32,
            //         y: tile_pos.y as u32,
            //     },
            //     chunk_pos,
            //     0b0000,
            //     16,
            //     &mut chunk_manager,
            //     &mut commands,
            // );
        }
    }
}
