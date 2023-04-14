use bevy::app::AppExit;

use bevy::prelude::*;
use bevy::time::FixedTimestep;
use bevy::utils::HashSet;
use bevy::window::{WindowFocused, WindowId};
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_rapier2d::prelude::{
    Collider, KinematicCharacterController, MoveShapeOptions, QueryFilter, RapierContext, RigidBody,
};
use bevy_rapier2d::rapier::prelude::RigidBodyType;
use seldom_state::prelude::{NotTrigger, StateMachine};

use crate::ai::{Attack, AttackDistance, Follow, Idle, LineOfSight, WalkingDirection};
use crate::animations::{AnimatedTextureMaterial, AnimationTimer, AttackEvent};

use crate::attributes::Health;
use crate::inventory::ItemStack;
use crate::item::Equipment;
use crate::ui::{change_hotbar_slot, InventoryState};
use crate::world_generation::TileMapPositionData;
use crate::{
    item::WorldObject, world_generation::WorldGenerationPlugin, GameState, Player,
    PLAYER_DASH_SPEED, TIME_STEP,
};
use crate::{
    GameParam, GameUpscale, MainCamera, RawPosition, TextureCamera, UICamera, YSort,
    PLAYER_MOVE_SPEED,
};

const HOTBAR_KEYCODES: [KeyCode; 6] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
];
#[derive(Default, Resource, Debug)]
pub struct CursorPos {
    pub world_coords: Vec3,
    pub screen_coords: Vec3,
    pub ui_coords: Vec3,
}

#[derive(Component, Default)]
pub struct MovementVector(pub Vec2);

#[derive(Component)]

pub struct LastDirectionInput(pub KeyCode);

impl Default for LastDirectionInput {
    fn default() -> Self {
        Self(KeyCode::Numpad0)
    }
}

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos::default())
            .add_event::<PlayerMoveEvent>()
            .add_event::<AttackEvent>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::move_player)
                    .with_system(Self::handle_hotbar_key_input)
                    .with_system(Self::test_take_damage)
                    .with_system(Self::update_cursor_pos.after(Self::move_player))
                    .with_system(Self::move_camera_with_player.after(Self::move_player)),
            )
            .add_system(Self::close_on_esc)
            .add_system(Self::toggle_inventory)
            .add_system(Self::mouse_click_system);
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
                &mut LastDirectionInput,
                Option<&Children>,
            ),
            (With<Player>, Without<MainCamera>, Without<Equipment>),
        >,
        mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
        mut limb_query: Query<&Handle<AnimatedTextureMaterial>>,
        time: Res<Time>,
        key_input: ResMut<Input<KeyCode>>,
        mut context: ResMut<RapierContext>,
        mut move_event: EventWriter<PlayerMoveEvent>,
    ) {
        let (
            ent,
            mut player_transform,
            mut raw_pos,
            player_collider,
            mut mv,
            mut dir_inp,
            children,
        ) = player_query.single_mut();
        let player = &mut game.game.player_state;
        let mut d = Vec2::ZERO;
        let s = PLAYER_MOVE_SPEED;

        if key_input.pressed(KeyCode::A) {
            d.x -= 1.;

            if !player.is_attacking {
                dir_inp.0 = KeyCode::A;
            }
            player.is_moving = true;
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
            if !player.is_attacking {
                dir_inp.0 = KeyCode::D;
            }
            player.is_moving = true;
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
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::S) {
            d.y -= 1.;
            player.is_moving = true;
        }
        if player.player_dash_cooldown.tick(time.delta()).finished()
            && key_input.pressed(KeyCode::Space)
        {
            player.is_dashing = true;

            player.player_dash_cooldown.reset();
            player.player_dash_duration.reset();
        }
        if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]) &&
        // || (dx == 0. && dy == 0.)
        !key_input.any_pressed([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
        {
            player.is_moving = false;
            move_event.send(PlayerMoveEvent(true));
        }
        if d.x != 0. || d.y != 0. {
            d = d.normalize() * s;
        }

        if player.is_dashing {
            player.player_dash_duration.tick(time.delta());

            d.x += d.x * PLAYER_DASH_SPEED * TIME_STEP;
            d.y += d.y * PLAYER_DASH_SPEED * TIME_STEP;
            if player.player_dash_duration.just_finished() {
                player.is_dashing = false;
            }
        }
        let mut collected_drops = HashSet::new();
        let output_ws = context.move_shape(
            Vec2::new(0., d.y),
            player_collider,
            raw_pos.0,
            0.,
            0.,
            &MoveShapeOptions::default(),
            QueryFilter {
                // flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_collider: Some(ent),
                predicate: Some(&|e| {
                    if let Some(c) = children {
                        !c.iter().any(|cc| *cc == e)
                    } else {
                        true
                    }
                }),
                ..default()
            },
            |col| {
                for (item_stack_entity, _, _) in game.items_query.iter() {
                    if col.entity == item_stack_entity && !collected_drops.contains(&col.entity) {
                        collected_drops.insert(col.entity);
                    }
                }
            },
        );

        let output_ad = context.move_shape(
            Vec2::new(d.x, 0.),
            player_collider,
            raw_pos.0,
            0.,
            0.,
            &MoveShapeOptions::default(),
            QueryFilter {
                // flags: QueryFilterFlags::EXCLUDE_SENSORS,
                exclude_collider: Some(ent),
                predicate: Some(&|e| {
                    if let Some(c) = children {
                        !c.iter().any(|cc| *cc == e)
                    } else {
                        true
                    }
                }),
                ..default()
            },
            |col| {
                for (item_stack_entity, _, _) in game.items_query.iter() {
                    if col.entity == item_stack_entity && !collected_drops.contains(&col.entity) {
                        collected_drops.insert(col.entity);
                    }
                }
            },
        );
        mv.0 = d;
        raw_pos.x += output_ad.effective_translation.x;
        raw_pos.y += output_ws.effective_translation.y;

        player_transform.translation.x = raw_pos.x.round();
        player_transform.translation.y = raw_pos.y.round();
        player.position = player_transform.translation;

        if d.x != 0. || d.y != 0. {
            move_event.send(PlayerMoveEvent(false));
        }
        for drop in collected_drops.iter() {
            let item_stack = game.items_query.get(*drop).unwrap().2;
            item_stack.add_to_inventory(&mut game.game, &mut game.inv_slot_query);

            game.world_obj_data.drop_entities.remove(&drop);
            commands.entity(*drop).despawn();
        }
    }
    pub fn toggle_inventory(
        mut game: GameParam,
        key_input: ResMut<Input<KeyCode>>,
        mut inv_query: Query<(&mut Visibility, &mut InventoryState)>,
        mut commands: Commands,
        player: Query<Entity, With<Player>>,
        asset_server: Res<AssetServer>,
        mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    ) {
        if key_input.just_pressed(KeyCode::I) {
            let mut inv_state = inv_query.single_mut().1;
            inv_state.open = !inv_state.open;
        }
        if key_input.just_pressed(KeyCode::E) {
            let sword_stack = ItemStack {
                obj_type: WorldObject::Sword,
                count: 1,
            };
            sword_stack.add_to_empty_inventory_slot(&mut game.game, &mut game.inv_slot_query);
        }
        if key_input.just_pressed(KeyCode::L) {
            let player_e = player.single();
            let texture_handle = asset_server.load("textures/slime/slime-move.png");
            let texture_atlas =
                TextureAtlas::from_grid(texture_handle, Vec2::new(32.0, 32.0), 7, 1, None, None);
            let texture_atlas_handle = texture_atlases.add(texture_atlas);

            commands.spawn((
                SpriteSheetBundle {
                    texture_atlas: texture_atlas_handle,
                    ..default()
                },
                AnimationTimer(Timer::from_seconds(0.20, TimerMode::Repeating)),
                Health(100),
                KinematicCharacterController::default(),
                Collider::cuboid(10., 6.),
                YSort,
                StateMachine::new(Idle {
                    walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                    direction: WalkingDirection::new_rand_dir(rand::thread_rng()),
                    speed: 0.5,
                })
                .trans::<Idle>(
                    LineOfSight {
                        target: player_e,
                        range: 100.,
                    },
                    Follow {
                        target: player_e,
                        speed: 0.7,
                    },
                )
                .trans::<Follow>(
                    AttackDistance {
                        target: player_e,
                        range: 50.,
                    },
                    Attack {
                        target: player_e,
                        attack_startup_timer: Timer::from_seconds(0.3, TimerMode::Once),
                        attack_cooldown_timer: Timer::from_seconds(1., TimerMode::Once),
                        speed: 1.4,
                        damage: 10,
                    },
                )
                .trans::<Follow>(
                    NotTrigger(LineOfSight {
                        target: player_e,
                        range: 100.,
                    }),
                    Idle {
                        walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
                        direction: WalkingDirection::new_rand_dir(rand::thread_rng()),
                        speed: 0.5,
                    },
                )
                .trans::<Attack>(
                    NotTrigger(AttackDistance {
                        target: player_e,
                        range: 50.,
                    }),
                    Follow {
                        target: player_e,
                        speed: 0.7,
                    },
                ),
                Name::new("Slime"),
            ));
        }
    }
    pub fn test_take_damage(
        mut player_health_query: Query<&mut Health, With<Player>>,
        key_input: ResMut<Input<KeyCode>>,
    ) {
        if key_input.just_pressed(KeyCode::X) {
            player_health_query.single_mut().0 -= 20;
        }
        if key_input.just_pressed(KeyCode::Z) {
            player_health_query.single_mut().0 += 20;
        }
    }
    fn handle_hotbar_key_input(
        mut game: GameParam,
        mut key_input: ResMut<Input<KeyCode>>,
        mut inv_state: Query<&mut InventoryState>,
    ) {
        for (slot, key) in HOTBAR_KEYCODES.iter().enumerate() {
            if key_input.just_pressed(*key) {
                change_hotbar_slot(&mut game, slot, &mut inv_state);
                key_input.clear();
            }
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
                *cursor_pos = CursorPos {
                    world_coords: Self::cursor_pos_in_world(
                        &windows,
                        cursor_moved.position,
                        cam_t,
                        cam,
                    ),
                    ui_coords: Self::cursor_pos_in_ui(&windows, cursor_moved.position, cam),
                    screen_coords: cursor_moved.position.extend(0.),
                };
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
    pub fn cursor_pos_in_ui(windows: &Windows, cursor_pos: Vec2, cam: &Camera) -> Vec3 {
        let window = windows.primary();

        let window_size = Vec2::new(window.width(), window.height());

        // Convert screen position [0..resolution] to ndc [-1..1]
        // (ndc = normalized device coordinates)
        let t = Transform::from_translation(Vec3::new(0., 0., 0.));
        let ndc_to_world = t.compute_matrix() * cam.projection_matrix().inverse();
        let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
        ndc_to_world.project_point3(ndc.extend(0.0))
    }

    pub fn mouse_click_system(
        mut commands: Commands,
        mouse_button_input: Res<Input<MouseButton>>,
        cursor_pos: Res<CursorPos>,
        mut game: GameParam,
        mut attack_event: EventWriter<AttackEvent>,
        inv_query: Query<(&mut Visibility, &InventoryState)>,
    ) {
        let inv_state = inv_query.get_single();
        if let Ok(inv_state) = inv_state {
            if inv_state.0.is_visible {
                return;
            }
        }
        // Hit Item, send attack event
        if mouse_button_input.just_pressed(MouseButton::Left) {
            attack_event.send(AttackEvent);

            let player_pos = game.game.player_state.position;
            println!(
                "{:?} {:?}",
                player_pos
                    .truncate()
                    .distance(cursor_pos.world_coords.truncate()),
                (game.game.player_state.reach_distance * 32) as f32
            );
            if player_pos
                .truncate()
                .distance(cursor_pos.world_coords.truncate())
                > (game.game.player_state.reach_distance * 32) as f32
            {
                return;
            }
            let cursor_chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.world_coords.x,
                cursor_pos.world_coords.y,
            ));
            let cursor_tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.world_coords.x,
                cursor_pos.world_coords.y,
            ));

            if game
                .chunk_manager
                .chunk_generation_data
                .contains_key(&TileMapPositionData {
                    chunk_pos: cursor_chunk_pos,
                    tile_pos: TilePos {
                        x: cursor_tile_pos.x as u32,
                        y: cursor_tile_pos.y as u32,
                    },
                })
            {
                let obj_data = game
                    .chunk_manager
                    .chunk_generation_data
                    .get(&TileMapPositionData {
                        chunk_pos: cursor_chunk_pos,
                        tile_pos: TilePos {
                            x: cursor_tile_pos.x as u32,
                            y: cursor_tile_pos.y as u32,
                        },
                    })
                    .unwrap();
                if game.block_query.contains(obj_data.entity) {
                    obj_data.object.attempt_to_break_item(
                        &mut commands,
                        &mut game,
                        cursor_tile_pos,
                        cursor_chunk_pos,
                    );
                }
            }
        }
        // Attempt to place block in hand
        // TODO: Interact
        if mouse_button_input.just_pressed(MouseButton::Right) {
            let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
                cursor_pos.world_coords.x,
                cursor_pos.world_coords.y,
            ));
            let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
                cursor_pos.world_coords.x,
                cursor_pos.world_coords.y,
            ));
            let hotbar_slot = inv_state.unwrap().1.active_hotbar_slot;
            let held_item_option = game.game.player_state.inventory[hotbar_slot];
            if let Some(mut held_item) = held_item_option {
                if let Some(places_into_item) = game
                    .world_obj_data
                    .properties
                    .get(&held_item.item_stack.obj_type)
                    .unwrap()
                    .places_into
                {
                    if let Some(_able_to_spawn) = places_into_item.spawn_and_save_block(
                        &mut commands,
                        &mut game,
                        tile_pos,
                        chunk_pos,
                    ) {
                        game.game.player_state.inventory[hotbar_slot] = held_item.modify_count(-1);
                    }
                }
            }
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
            (
                With<Player>,
                Without<MainCamera>,
                Without<TextureCamera>,
                Without<UICamera>,
            ),
        >,
        mut game_camera: Query<
            (&mut Transform, &mut RawPosition),
            (Without<MainCamera>, Without<UICamera>, With<TextureCamera>),
        >,
        mut screen_camera: Query<
            (&mut Transform, &GameUpscale),
            (With<MainCamera>, Without<UICamera>, Without<TextureCamera>),
        >,
    ) {
        let (mut game_camera_transform, mut raw_camera_pos) = game_camera.single_mut();

        let (player_pos, player_movement_vec) = player_query.single_mut();

        let camera_lookahead_scale = 15.0;
        raw_camera_pos.0 = raw_camera_pos
            .0
            .lerp(player_movement_vec.0 * camera_lookahead_scale, 0.08);

        let camera_final_pos = Vec2::new(
            player_pos.translation.x - raw_camera_pos.x,
            player_pos.translation.y - raw_camera_pos.y,
        );
        game_camera_transform.translation.x = camera_final_pos.x.trunc();
        game_camera_transform.translation.y = camera_final_pos.y.trunc();
        let (mut screen_camera_transform, game_upscale) = screen_camera.single_mut();
        screen_camera_transform.translation.x = camera_final_pos.x.fract() * game_upscale.0;
        screen_camera_transform.translation.y = camera_final_pos.y.fract() * game_upscale.0;
    }
}
