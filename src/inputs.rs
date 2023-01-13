use bevy::app::AppExit;
use bevy::ecs::system::Despawn;
use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy::utils::HashSet;
use bevy::window::{WindowFocused, WindowId};
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::{
    Collider, KinematicCharacterController, KinematicCharacterControllerOutput, MoveShapeOptions,
    QueryFilter, QueryFilterFlags, RapierContext,
};

use crate::item::{Breakable, DropItem, Equipment, WorldObjectResource};
use crate::world_generation::{GameData, TileMapPositionData, WorldObjectEntityData};
use crate::{
    assets::{Graphics, WORLD_SCALE},
    item::WorldObject,
    world_generation::{ChunkManager, WorldGenerationPlugin},
    Game, GameState, Player, PLAYER_DASH_SPEED, TIME_STEP,
};
use crate::{ItemStack, PLAYER_MOVE_SPEED};

#[derive(Default, Resource)]
pub struct CursorPos(Vec3);

#[derive(Component)]
pub struct Direction(pub f32);

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos(Vec3::new(-100.0, -100.0, 0.0)))
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::move_player)
                    .with_system(Self::update_cursor_pos.after(Self::move_player))
                    .with_system(Self::mouse_click_system),
            )
            .add_system(Self::close_on_esc);
    }
}

impl InputsPlugin {
    fn move_player(
        key_input: ResMut<Input<KeyCode>>,
        mut game: ResMut<Game>,
        mut player_query: Query<
            (
                Entity,
                &mut Transform,
                &mut KinematicCharacterController,
                &Collider,
                &mut Direction,
                Option<&Children>,
                Option<&mut KinematicCharacterControllerOutput>,
            ),
            (With<Player>, Without<Camera>),
        >,
        mut eqp_query: Query<&mut Transform, (With<Equipment>, Without<Camera>, Without<Player>)>,
        drops_query: Query<
            (Entity, &WorldObject),
            (With<DropItem>, Without<Camera>, Without<Player>),
        >,
        mut camera_query: Query<&mut Transform, (With<Camera>, Without<Player>)>,
        time: Res<Time>,
        mut context: ResMut<RapierContext>,
        mut commands: Commands,
    ) {
        let (
            ent,
            mut player_transform,
            mut player_kin_controller,
            player_collider,
            mut dir,
            children,
            mut output,
        ) = player_query.single_mut();
        let mut camera_transform = camera_query.single_mut();
        let mut dx = 0.0;
        let mut dy = 0.0;
        let s = PLAYER_MOVE_SPEED / WORLD_SCALE;

        if key_input.pressed(KeyCode::A) {
            dx -= s;
            game.player.is_moving = true;
            player_transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
            if let Some(c) = children {
                for item in c.iter() {
                    if let Ok(mut e) = eqp_query.get_mut(*item) {
                        e.translation.z = -0.1
                    }
                }
            }
        }
        if key_input.pressed(KeyCode::D) {
            dx += s;
            game.player.is_moving = true;
            player_transform.rotation = Quat::default();
            if let Some(c) = children {
                for item in c.iter() {
                    if let Ok(mut e) = eqp_query.get_mut(*item) {
                        e.translation.z = 0.1
                    }
                }
            }
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
        if dx != 0. && dy != 0. {
            dx = if dx == -s { -(s * 0.66) } else { s * 0.66 };
            dy = if dy == -s { -(s * 0.66) } else { s * 0.66 };
        }

        if game.player.is_dashing {
            game.player_dash_duration.tick(time.delta());

            dx += dx * PLAYER_DASH_SPEED * TIME_STEP;
            dy += dy * PLAYER_DASH_SPEED * TIME_STEP;
            if game.player_dash_duration.just_finished() {
                game.player.is_dashing = false;
            }
        }
        let mut collected_drops = HashSet::new();

        let output_ws = context.move_shape(
            Vec2::new(0., dy),
            player_collider,
            player_transform.translation.truncate(),
            0.,
            0.,
            &MoveShapeOptions::default(),
            QueryFilter {
                // flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_collider: Some(ent),
                predicate: Some(&|e| {
                    if let Some(c) = children {
                        c.iter().find(|cc| **cc == e).is_none()
                    } else {
                        true
                    }
                }),
                ..default()
            },
            |col| {
                for (drop, obj) in drops_query.iter() {
                    if col.entity == drop && !collected_drops.contains(&col.entity) {
                        if let Some(mut ec) = commands.get_entity(drop) {
                            ec.despawn();
                            if let Some(stack) = game.player.inventory.iter().find(|i| i.0 == *obj)
                            {
                                // safe to unwrap, we check for it above
                                let index = game
                                    .player
                                    .inventory
                                    .iter()
                                    .position(|i| i == stack)
                                    .unwrap();
                                let stack = game.player.inventory.get_mut(index).unwrap();
                                stack.1 += 1;
                            } else {
                                game.player.inventory.push(ItemStack(*obj, 1));
                            }
                            collected_drops.insert(col.entity);
                            info!("{:?} | {:?}", drop, game.player.inventory);
                        }
                    }
                }
            },
        );

        let output_ad = context.move_shape(
            Vec2::new(dx, 0.),
            player_collider,
            player_transform.translation.truncate(),
            0.,
            0.,
            &MoveShapeOptions::default(),
            QueryFilter {
                // flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_collider: Some(ent),
                predicate: Some(&|e| {
                    if let Some(c) = children {
                        c.iter().find(|cc| **cc == e).is_none()
                    } else {
                        true
                    }
                }),
                ..default()
            },
            |col| {
                for (drop, obj) in drops_query.iter() {
                    if col.entity == drop && !collected_drops.contains(&col.entity) {
                        if let Some(mut ec) = commands.get_entity(drop) {
                            ec.despawn();
                            if let Some(stack) = game.player.inventory.iter().find(|i| i.0 == *obj)
                            {
                                // safe to unwrap, we check for it above
                                let index = game
                                    .player
                                    .inventory
                                    .iter()
                                    .position(|i| i == stack)
                                    .unwrap();
                                let stack = game.player.inventory.get_mut(index).unwrap();
                                stack.1 += 1;
                            } else {
                                game.player.inventory.push(ItemStack(*obj, 1));
                            }
                            collected_drops.insert(col.entity);
                            info!("{:?} | {:?}", drop, game.player.inventory);
                        }
                    }
                }
            },
        );

        let cx = player_transform.translation.x + output_ad.effective_translation.x;
        let cy = player_transform.translation.y + output_ws.effective_translation.y;
        // player_kin_controller.translation =
        player_transform.translation +=
            output_ws.effective_translation.extend(0.) + output_ad.effective_translation.extend(0.);
        camera_transform.translation.x = cx;
        camera_transform.translation.y = cy;

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
    }

    pub fn update_cursor_pos(
        windows: Res<Windows>,
        camera_q: Query<(&Transform, &Camera)>,
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
        mut commands: Commands,
        graphics: Res<Graphics>,
        world_obj_data: Res<WorldObjectResource>,
        player_query: Query<(Entity, &mut Player)>,
        mut game_data: ResMut<GameData>,
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
            if chunk_manager
                .chunk_generation_data
                .contains_key(&TileMapPositionData {
                    chunk_pos,
                    tile_pos: TilePos {
                        x: tile_pos.x as u32,
                        y: tile_pos.y as u32,
                    },
                })
            {
                let obj_data = chunk_manager
                    .chunk_generation_data
                    .get(&TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    })
                    .unwrap();
                let main_hand_tool = player_query.single().1.main_hand_slot;
                obj_data.object.attempt_to_break_item(
                    &mut commands,
                    &world_obj_data,
                    &graphics,
                    &mut chunk_manager,
                    &mut game_data,
                    tile_pos,
                    chunk_pos,
                    main_hand_tool,
                );
            } else {
                let stone = WorldObject::StoneFull.spawn_and_save(
                    &mut commands,
                    &world_obj_data,
                    &graphics,
                    &mut chunk_manager,
                    &mut game_data,
                    tile_pos,
                    chunk_pos,
                );
                commands
                    .entity(stone)
                    .insert(Breakable(Some(WorldObject::StoneHalf)));
                // commands.spawn(stone);
                // .insert(Name::new("Test Objects"))
                // .push_children(&children)
                chunk_manager.chunk_generation_data.insert(
                    TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    },
                    WorldObjectEntityData {
                        object: WorldObject::StoneFull,
                        entity: stone,
                    },
                );
            }
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
            WorldObject::Sword.spawn_equipment_on_player(
                player_query,
                &mut commands,
                &world_obj_data,
                &graphics,
            );
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let stone = WorldObject::StoneFull.spawn_and_save(
                &mut commands,
                &world_obj_data,
                &graphics,
                &mut chunk_manager,
                &mut game_data,
                tile_pos,
                chunk_pos,
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
        if mouse_button_input.just_released(MouseButton::Middle) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let stone = WorldObject::StoneFull.spawn_and_save(
                &mut commands,
                &world_obj_data,
                &graphics,
                &mut chunk_manager,
                &mut game_data,
                tile_pos,
                chunk_pos,
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
    pub fn close_on_esc(
        mut focused: Local<Option<WindowId>>,
        mut focused_events: EventReader<WindowFocused>,
        mut exit: EventWriter<AppExit>,
        mut windows: ResMut<Windows>,
        input: Res<Input<KeyCode>>,
    ) {
        // TODO: Track this in e.g. a resource to ensure consistent behaviour across similar systems
        for event in focused_events.iter() {
            *focused = event.focused.then_some(event.id);
        }

        if let Some(focused) = &*focused {
            if input.just_pressed(KeyCode::Escape) {
                if let Some(window) = windows.get_mut(*focused) {
                    exit.send(AppExit);
                    window.close();
                }
            }
        }
    }
}
