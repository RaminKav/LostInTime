use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_ecs_tilemap::tiles::TilePos;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::prelude::KinematicCharacterController;

use crate::animations::{AnimatedTextureMaterial, AttackEvent};

use crate::attributes::{Attack, AttackCooldown, AttributeModifier, CurrentHealth, MaxHealth};
use crate::combat::{AttackTimer, HitEvent};
use crate::enemy::NeutralMob;
use crate::inventory::{Container, Inventory};
use crate::item::item_actions::{ItemAction, ItemActionParam};
use crate::item::object_actions::ObjectAction;
use crate::item::projectile::{RangedAttack, RangedAttackEvent};
use crate::item::{Equipment, WorldObject};
use crate::proto::proto_param::ProtoParam;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{change_hotbar_slot, ChestInventory, InventoryState, CHEST_SIZE};
use crate::world::chunk::Chunk;
use crate::world::dungeon::DungeonPlugin;
use crate::world::world_helpers::{tile_pos_to_world_pos, world_pos_to_tile_pos};
use crate::world::TileMapPosition;
use crate::{
    custom_commands::CommandsExt, AppExt, CoreGameSet, CustomFlush, GameParam, GameState,
    GameUpscale, MainCamera, RawPosition, TextureCamera, UICamera, PLAYER_MOVE_SPEED, WIDTH,
};
use crate::{Game, Player, PLAYER_DASH_SPEED, TIME_STEP};

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

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum FacingDirection {
    Left,
    Right,
    // Up,
    // Down,
}

impl Default for FacingDirection {
    fn default() -> Self {
        Self::Right
    }
}

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos::default())
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<AttackEvent>();
            })
            .add_systems(
                (
                    Self::turn_player,
                    Self::move_player,
                    Self::move_camera_with_player.after(Self::move_player),
                    Self::test_take_damage,
                )
                    .in_set(CoreGameSet::Main)
                    .in_schedule(CoreSchedule::FixedUpdate),
            )
            .add_systems(
                (
                    Self::mouse_click_system.after(CustomFlush),
                    Self::handle_hotbar_key_input,
                    Self::update_cursor_pos.after(Self::move_player),
                    Self::toggle_inventory,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
        // .add_system(Self::toggle_inventory);
    }
}

impl InputsPlugin {
    fn turn_player(
        mut game: ResMut<Game>,
        mut player_query: Query<&Children, With<Player>>,
        mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
        mut limb_query: Query<&Handle<AnimatedTextureMaterial>>,
        cursor_pos: Res<CursorPos>,
    ) {
        let (flip, dir) = if cursor_pos.screen_coords.x > WIDTH / 2. {
            (0., FacingDirection::Right)
        } else {
            (1., FacingDirection::Left)
        };
        //TODO: make center point based on player pos on screen?
        //TODO: add some way for attack to know dir
        if let Ok(c) = player_query.get_single_mut() {
            for l in c.iter() {
                if let Ok(limb_handle) = limb_query.get_mut(*l) {
                    let limb_material = materials.get_mut(limb_handle).unwrap();
                    limb_material.flip = flip;
                    game.player_state.direction = dir.clone();
                }
            }
        }
    }
    pub fn move_player(
        mut game: GameParam,
        mut player_query: Query<
            (&mut KinematicCharacterController, &mut MovementVector),
            (
                With<Player>,
                Without<MainCamera>,
                Without<Chunk>,
                Without<Equipment>,
            ),
        >,
        time: Res<Time>,
        key_input: ResMut<Input<KeyCode>>,
        mut minimap_event: EventWriter<UpdateMiniMapEvent>,
    ) {
        let (mut player_transform, mut mv) = player_query.single_mut();
        let mut player = game.player_mut();
        let mut d = Vec2::ZERO;
        let s = PLAYER_MOVE_SPEED;

        if key_input.pressed(KeyCode::A) {
            d.x -= 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::D) {
            d.x += 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::W) {
            d.y += 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::S) {
            d.y -= 1.;
            player.is_moving = true;
        }
        //TODO: move this tick to animations.rs
        if player.player_dash_cooldown.tick(time.delta()).finished()
            && key_input.pressed(KeyCode::Space)
        {
            player.is_dashing = true;

            player.player_dash_cooldown.reset();
            player.player_dash_duration.reset();
        }
        if (key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
            && !key_input.any_pressed([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]))
            || (d.x == 0. && d.y == 0.)
        {
            player.is_moving = false;
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

        mv.0 = d;

        if d.x != 0. || d.y != 0. {
            player_transform.translation = Some(Vec2::new(d.x, d.y));
            minimap_event.send(UpdateMiniMapEvent {
                pos: None,
                new_tile: None,
            });
        } else {
            player_transform.translation = Some(Vec2::ZERO);
        }
    }
    pub fn toggle_inventory(
        game: GameParam,
        key_input: ResMut<Input<KeyCode>>,
        mut inv_state: ResMut<InventoryState>,
        mut commands: Commands,
        mut proto_commands: ProtoCommands,
        proto: ProtoParam,
        mut inv: Query<&mut Inventory>,
        chest_query: Query<&ChestInventory>,
    ) {
        if key_input.just_pressed(KeyCode::I) {
            inv_state.open = !inv_state.open;
        }
        if key_input.just_pressed(KeyCode::C) {
            let chest = commands.spawn_empty().id();
            commands.entity(chest).insert(ChestInventory {
                items: Container::with_size(CHEST_SIZE),
                parent: chest,
            });
        }
        if key_input.just_pressed(KeyCode::V) {
            commands.insert_resource(chest_query.single().clone());
        }

        if key_input.just_pressed(KeyCode::E) {
            proto_commands.spawn_item_from_proto(
                WorldObject::BasicStaff,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::DualStaff,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::FireStaff,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::Dagger,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::Chestplate,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::Pants,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::Ring,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::Pendant,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::ChestBlock,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::DungeonEntranceBlock,
                &proto,
                game.player().position.truncate(),
                1,
            );
            proto_commands.spawn_item_from_proto(
                WorldObject::GrassBlock,
                &proto,
                game.player().position.truncate(),
                1,
            );
        }

        if key_input.just_pressed(KeyCode::P) {
            DungeonPlugin::spawn_new_dungeon_dimension(&mut commands, &mut proto_commands);
        }
        if key_input.just_pressed(KeyCode::L) {
            let pos = tile_pos_to_world_pos(
                TileMapPosition::new(IVec2 { x: 0, y: 0 }, TilePos { x: 0, y: 0 }, 0),
                true,
            );
            proto_commands.spawn_from_proto(NeutralMob::Slime, &proto.prototypes, pos);
        }
        if key_input.just_pressed(KeyCode::M) {
            let item = inv.single().items.items[0].clone();
            if let Some(item) = item {
                item.modify_attributes(
                    AttributeModifier {
                        modifier: "attack".to_owned(),
                        delta: 10,
                    },
                    &mut inv,
                );
            }
        }
    }
    pub fn test_take_damage(
        // mut player_health_query: Query<&mut Health, With<Player>>,
        key_input: ResMut<Input<KeyCode>>,
        att_query: Query<(&mut CurrentHealth, &MaxHealth, &Attack, &AttackCooldown), With<Player>>,
    ) {
        if key_input.just_pressed(KeyCode::X) {
            // att_query.single_mut().0 -= 20;
        }
        if key_input.just_pressed(KeyCode::Z) {
            // att_query.single_mut().0 += 20;
        }
        if key_input.just_pressed(KeyCode::T) {
            println!("STATS: {:?}", att_query.single());
        }
    }
    fn handle_hotbar_key_input(
        mut game: GameParam,
        mut key_input: ResMut<Input<KeyCode>>,
        mut inv_state: ResMut<InventoryState>,
    ) {
        for (slot, key) in HOTBAR_KEYCODES.iter().enumerate() {
            if key_input.just_pressed(*key) {
                change_hotbar_slot(slot, &mut inv_state, &mut game.inv_slot_query);
                key_input.clear();
            }
        }
    }
    pub fn update_cursor_pos(
        windows: Query<&Window, With<PrimaryWindow>>,
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
        windows: &Query<&Window, With<PrimaryWindow>>,
        cursor_pos: Vec2,
        cam_t: &Transform,
        cam: &Camera,
    ) -> Vec3 {
        let window = windows.single();

        let window_size = Vec2::new(window.width(), window.height());

        // Convert screen position [0..resolution] to ndc [-1..1]
        // (ndc = normalized device coordinates)
        let ndc_to_world = cam_t.compute_matrix() * cam.projection_matrix().inverse();
        let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
        ndc_to_world.project_point3(ndc.extend(0.0))
    }
    pub fn cursor_pos_in_ui(
        windows: &Query<&Window, With<PrimaryWindow>>,
        cursor_pos: Vec2,
        cam: &Camera,
    ) -> Vec3 {
        let window = windows.single();

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
        mut proto_commands: ProtoCommands,
        mouse_button_input: Res<Input<MouseButton>>,
        cursor_pos: Res<CursorPos>,
        game: GameParam,
        proto_param: ProtoParam,
        mut attack_event: EventWriter<AttackEvent>,
        mut hit_event: EventWriter<HitEvent>,

        att_cooldown_query: Query<(Entity, Option<&AttackTimer>), With<Player>>,
        inv: Query<&mut Inventory>,
        inv_state: Res<InventoryState>,
        parent_attack: Query<&Attack>,
        tool_query: Query<&RangedAttack, With<Equipment>>,
        mut ranged_attack_event: EventWriter<RangedAttackEvent>,
        mut item_action_param: ItemActionParam,
        obj_actions: Query<&ObjectAction>,
    ) {
        if inv_state.open {
            return;
        }

        let cursor_tile_pos = world_pos_to_tile_pos(cursor_pos.world_coords.truncate());
        let player_pos = game.player().position;

        // Hit Item, send attack event
        if mouse_button_input.pressed(MouseButton::Left) {
            // println!(
            //     "C: {cursor_tile_pos:?} || P: {player_pos:?}  || {:?} {:?}",
            //     game.get_tile_data(cursor_tile_pos),
            //     game.get_tile_obj_data(cursor_tile_pos)
            // );
            if att_cooldown_query.single().1.is_some() {
                return;
            }
            let mut main_hand_option = None;
            // if it has AttackTimer, the action is on cooldown, so we abort.
            if let Some(tool) = &game.player().main_hand_slot {
                main_hand_option = Some(tool.obj);
            }

            if let Ok(ranged_tool) = tool_query.get_single() {
                ranged_attack_event.send(RangedAttackEvent {
                    projectile: ranged_tool.0.clone(),
                    direction: (cursor_pos.world_coords.truncate() - player_pos.truncate())
                        .normalize_or_zero(),
                })
            }
            attack_event.send(AttackEvent);
            if player_pos
                .truncate()
                .distance(cursor_pos.world_coords.truncate())
                > game.player().reach_distance * 32.
            {
                return;
            }
            if let Some(hit_obj) = game.get_obj_entity_at_tile(cursor_tile_pos) {
                hit_event.send(HitEvent {
                    hit_entity: hit_obj,
                    damage: parent_attack.get(game.game.player).unwrap_or(&Attack(5)).0,
                    dir: Vec2::new(0., 0.),
                    hit_with: main_hand_option,
                });
            }
        }
        // Attempt to place block in hand
        // TODO: Interact
        if mouse_button_input.just_pressed(MouseButton::Right) {
            let hotbar_slot = inv_state.active_hotbar_slot;
            let held_item_option = inv.single().items.items[hotbar_slot].clone();
            if let Some(held_item) = held_item_option {
                let held_obj = *held_item.get_obj();
                if let Some(item_action) = proto_param.get_component::<ItemAction, _>(held_obj) {
                    item_action.run_action(held_obj, &mut item_action_param, &game);
                }
            }

            if let Some(obj) = game.get_obj_entity_at_tile(cursor_tile_pos) {
                if let Ok(obj_action) = obj_actions.get(obj) {
                    obj_action.run_action(
                        obj,
                        &mut item_action_param,
                        &mut commands,
                        &mut proto_commands,
                    );
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
