use std::f32::consts::PI;
use std::time::Duration;

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
use bevy_tweening::lens::{TransformPositionLens, TransformScaleLens};
use bevy_tweening::{Animator, AnimatorState, EaseFunction, Tween};
use interpolation::{lerp, Ease};

use crate::animations::{AnimatedTextureMaterial, AnimationFrameTracker, AnimationTimer};
use crate::attributes::Health;
use crate::item::{Block, Breakable, Equipment, WorldObjectResource};
use crate::world_generation::{GameData, TileMapPositionData, WorldObjectEntityData};
use crate::{
    assets::{Graphics, WORLD_SCALE},
    item::WorldObject,
    world_generation::{ChunkManager, WorldGenerationPlugin},
    Game, GameState, Player, PLAYER_DASH_SPEED, TIME_STEP,
};
use crate::{
    main, CameraDirty, GameParam, ItemStack, Limb, MainCamera, RawPosition, TextureCamera,
    TextureTarget, PLAYER_MOVE_SPEED,
};

#[derive(Default, Resource, Debug)]
pub struct CursorPos(Vec3);

#[derive(Component)]
pub struct MovementVector(pub f32, pub f32);

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos(Vec3::new(-100.0, -100.0, 0.0)))
            .add_event::<PlayerMoveEvent>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::move_player)
                    .with_system(Self::update_cursor_pos.after(Self::move_player))
                    .with_system(Self::move_camera_with_player.after(Self::move_player))
                    .with_system(Self::mouse_click_system),
            )
            .add_system(Self::close_on_esc);
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlayerMoveEvent(bool);

impl InputsPlugin {
    fn move_player(
        mut commands: Commands,
        mut game: GameParam,
        mut player_query: Query<
            (
                Entity,
                &mut Transform,
                &mut RawPosition,
                &Collider,
                &mut MovementVector,
                Option<&Children>,
            ),
            (With<Player>, Without<MainCamera>, Without<Equipment>),
        >,
        mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
        mut limb_query: Query<&Handle<AnimatedTextureMaterial>>,
        // mut eqp_query: Query<
        //     &mut Transform,
        //     (
        //         With<Limb>,
        //         Without<ItemStack>,
        //         Without<Camera>,
        //         Without<Player>,
        //     ),
        // >,
        // mut camera_query: Query<&mut Transform, (With<Camera>, Without<Player>)>,
        time: Res<Time>,
        key_input: ResMut<Input<KeyCode>>,
        mut context: ResMut<RapierContext>,
        mut move_event: EventWriter<PlayerMoveEvent>,
    ) {
        let (ent, mut player_transform, mut raw_pos, player_collider, mut mv, children) =
            player_query.single_mut();
        let mut d = Vec2::ZERO;
        let s = PLAYER_MOVE_SPEED;

        if key_input.pressed(KeyCode::A) {
            d.x -= 1.;
            game.game.player.is_moving = true;
            if let Some(c) = children {
                for l in c.iter() {
                    if let Ok(limb_handle) = limb_query.get_mut(*l) {
                        let limb_material = materials.get_mut(limb_handle);
                        if let Some(mat) = limb_material {
                            mat.flip = 1.;
                        }
                    }
                }
            }
        }

        if key_input.pressed(KeyCode::D) {
            d.x += 1.;
            game.game.player.is_moving = true;
            if let Some(c) = children {
                for l in c.iter() {
                    if let Ok(limb_handle) = limb_query.get_mut(*l) {
                        let limb_material = materials.get_mut(limb_handle);
                        if let Some(mat) = limb_material {
                            mat.flip = 0.;
                        }
                    }
                }
            }
        }
        if key_input.pressed(KeyCode::W) {
            d.y += 1.;
            game.game.player.is_moving = true;
        }
        if key_input.pressed(KeyCode::S) {
            d.y -= 1.;
            game.game.player.is_moving = true;
        }
        if game.game.player_dash_cooldown.tick(time.delta()).finished() {
            if key_input.pressed(KeyCode::Space) {
                game.game.player.is_dashing = true;

                game.game.player_dash_cooldown.reset();
                game.game.player_dash_duration.reset();
            }
        }
        if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]) &&
        // || (dx == 0. && dy == 0.)
        !key_input.any_pressed([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
        {
            game.game.player.is_moving = false;
            move_event.send(PlayerMoveEvent(true));
        }
        if d.x != 0. || d.y != 0. {
            d = d.normalize() * s;
        }

        if game.game.player.is_dashing {
            game.game.player_dash_duration.tick(time.delta());

            d.x += d.x * PLAYER_DASH_SPEED * TIME_STEP;
            d.y += d.y * PLAYER_DASH_SPEED * TIME_STEP;
            if game.game.player_dash_duration.just_finished() {
                game.game.player.is_dashing = false;
            }
        }
        let mut collected_drops = HashSet::new();
        let output_ws = context.move_shape(
            Vec2::new(0., d.y),
            player_collider,
            Vec2::new(raw_pos.0, raw_pos.1),
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
                for (drop, _, item_stack) in game.items_query.iter() {
                    if col.entity == drop && !collected_drops.contains(&col.entity) {
                        if let Some(mut ec) = commands.get_entity(drop) {
                            ec.despawn();
                            game.world_obj_data.drop_entities.remove(&drop);

                            if let Some(stack) = game
                                .game
                                .player
                                .inventory
                                .iter()
                                .find(|i| i.0 == item_stack.0)
                            {
                                // safe to unwrap, we check for it above
                                let index = game
                                    .game
                                    .player
                                    .inventory
                                    .iter()
                                    .position(|i| i == stack)
                                    .unwrap();
                                let stack = game.game.player.inventory.get_mut(index).unwrap();
                                stack.1 += item_stack.1;
                            } else {
                                game.game.player.inventory.push(*item_stack);
                            }
                            collected_drops.insert(col.entity);
                            info!("{:?} | {:?}", drop, game.game.player.inventory);
                        }
                    }
                }
            },
        );

        let output_ad = context.move_shape(
            Vec2::new(d.x, 0.),
            player_collider,
            Vec2::new(raw_pos.0, raw_pos.1),
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
                for (drop, _, item_stack) in game.items_query.iter() {
                    if col.entity == drop && !collected_drops.contains(&col.entity) {
                        if let Some(mut ec) = commands.get_entity(drop) {
                            ec.despawn();
                            game.world_obj_data.drop_entities.remove(&drop);

                            if let Some(stack) = game
                                .game
                                .player
                                .inventory
                                .iter()
                                .find(|i| i.0 == item_stack.0)
                            {
                                // safe to unwrap, we check for it above
                                let index = game
                                    .game
                                    .player
                                    .inventory
                                    .iter()
                                    .position(|i| i == stack)
                                    .unwrap();
                                let stack = game.game.player.inventory.get_mut(index).unwrap();
                                stack.1 += item_stack.1;
                            } else {
                                game.game.player.inventory.push(*item_stack);
                            }
                            collected_drops.insert(col.entity);
                            info!("{:?} | {:?}", drop, game.game.player.inventory);
                        }
                    }
                }
            },
        );
        mv.0 = d.x;
        mv.1 = d.y;
        raw_pos.0 += output_ad.effective_translation.x;
        raw_pos.1 += output_ws.effective_translation.y;

        player_transform.translation.x = raw_pos.0.round();
        player_transform.translation.y = raw_pos.1.round();

        if d.x != 0. || d.y != 0. {
            move_event.send(PlayerMoveEvent(false));
        }
    }

    pub fn update_cursor_pos(
        windows: Res<Windows>,
        camera_q: Query<(&Transform, &Camera), With<TextureCamera>>,
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
                println!("Cursor at: {:?}", cursor_pos);
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
        mut commands: Commands,
        mouse_button_input: Res<Input<MouseButton>>,
        cursor_pos: Res<CursorPos>,
        mut game: GameParam,
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
            info!("POS {:?}", tile_pos);
            if game
                .chunk_manager
                .chunk_generation_data
                .contains_key(&TileMapPositionData {
                    chunk_pos,
                    tile_pos: TilePos {
                        x: tile_pos.x as u32,
                        y: tile_pos.y as u32,
                    },
                })
            {
                let obj_data = game
                    .chunk_manager
                    .chunk_generation_data
                    .get(&TileMapPositionData {
                        chunk_pos,
                        tile_pos: TilePos {
                            x: tile_pos.x as u32,
                            y: tile_pos.y as u32,
                        },
                    })
                    .unwrap();
                if game.block_query.contains(obj_data.entity) {
                    obj_data.object.attempt_to_break_item(
                        &mut commands,
                        &mut game,
                        tile_pos,
                        chunk_pos,
                    );
                }
            } else {
                let stone = WorldObject::StoneHalf.spawn_and_save_block(
                    &mut commands,
                    &mut game,
                    tile_pos,
                    chunk_pos,
                );
                commands
                    .entity(stone)
                    .insert(Breakable(Some(WorldObject::StoneHalf)));
                game.chunk_manager.chunk_generation_data.insert(
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
        }
        if mouse_button_input.just_released(MouseButton::Right) {
            WorldObject::Sword.spawn_equipment_on_player(&mut commands, &mut game);
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.0.x,
                cursor_pos.0.y,
            ));
            let stone = WorldObject::StoneFull.spawn_and_save_block(
                &mut commands,
                &mut game,
                tile_pos,
                chunk_pos,
            );
            commands
                .spawn(SpatialBundle::default())
                .push_children(&[stone]);
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
            let stone = WorldObject::StoneFull.spawn_and_save_block(
                &mut commands,
                &mut game,
                tile_pos,
                chunk_pos,
            );
            commands
                .spawn(SpatialBundle::default())
                .push_children(&[stone]);
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
    pub fn move_camera_with_player(
        mut player_query: Query<
            (&Transform, &MovementVector),
            (With<Player>, Without<MainCamera>, Without<TextureCamera>),
        >,
        mut tex_camera: Query<
            (&mut Transform, &mut RawPosition),
            (Without<MainCamera>, With<TextureCamera>),
        >,
    ) {
        let (mut camera_tf, mut raw_tf) = tex_camera.single_mut();

        let (pt, mv) = player_query.single_mut();

        let raw_x = lerp(&raw_tf.0, &(mv.0), &0.08);
        let raw_y = lerp(&raw_tf.1, &(mv.1), &0.08);

        raw_tf.0 = raw_x;
        raw_tf.1 = raw_y;

        camera_tf.translation.x = (pt.translation.x - raw_x * 15.).round();
        camera_tf.translation.y = (pt.translation.y - raw_y * 15.).round();
    }
}
