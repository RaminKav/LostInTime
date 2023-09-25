use crate::animations::enemy_sprites::{CharacterAnimationSpriteSheetData, EnemyAnimationState};
use crate::animations::AttackEvent;
use crate::attributes::hunger::Hunger;
use crate::juice::{DustParticles, RunDustTimer};
use crate::player::handle_player_raw_position;
use crate::ui::stats_ui::StatsUI;
use crate::world::dimension::DimensionSpawnEvent;
use crate::world::dungeon::{spawn_new_dungeon_dimension, DungeonPlugin};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_ecs_tilemap::tiles::TilePos;

use bevy_hanabi::EffectSpawner;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};

use bevy_rapier2d::prelude::KinematicCharacterController;
use interpolation::Lerp;
use rand::rngs::ThreadRng;
use rand::Rng;

use crate::attributes::Speed;
use crate::combat::{AttackTimer, HitEvent};

use crate::enemy::Mob;
use crate::inventory::Inventory;
use crate::item::item_actions::{ItemActionParam, ItemActions, ManaCost};
use crate::item::item_upgrades::{
    ArrowSpeedUpgrade, BowUpgradeSpread, BurnOnHitUpgrade, ClawUpgradeMultiThrow,
    FireStaffAOEUpgrade, LethalHitUpgrade, LightningStaffChainUpgrade, VenomOnHitUpgrade,
};
use crate::item::object_actions::ObjectAction;
use crate::item::projectile::{RangedAttack, RangedAttackEvent};
use crate::item::Equipment;
use crate::proto::proto_param::ProtoParam;
use crate::ui::minimap::UpdateMiniMapEvent;
use crate::ui::{change_hotbar_slot, InventoryState, UIContainersParam};
use crate::world::chunk::Chunk;

use crate::world::world_helpers::{tile_pos_to_world_pos, world_pos_to_tile_pos};
use crate::world::TileMapPosition;
use crate::{
    custom_commands::CommandsExt, AppExt, CustomFlush, GameParam, GameState, MainCamera,
    RawPosition, TextureCamera, UICamera, PLAYER_MOVE_SPEED, WIDTH,
};
use crate::{Game, GameUpscale, Player, HEIGHT, PLAYER_DASH_SPEED, TIME_STEP};

const HOTBAR_KEYCODES: [KeyCode; 6] = [
    KeyCode::Key1,
    KeyCode::Key2,
    KeyCode::Key3,
    KeyCode::Key4,
    KeyCode::Key5,
    KeyCode::Key6,
];
#[derive(Default, Reflect, Resource, Debug)]
#[reflect(Resource)]
pub struct CursorPos {
    pub world_coords: Vec3,
    pub screen_coords: Vec3,
    pub ui_coords: Vec3,
}

#[derive(Component, Debug, Default)]
pub struct MovementVector(pub Vec2);

#[derive(Debug, Clone, PartialEq, Component, Eq, Default, Schematic, FromReflect, Reflect)]
#[reflect(Component, Schematic, Default)]

pub enum FacingDirection {
    Left,
    #[default]
    Right,
    Up,
    Down,
}
impl FacingDirection {
    pub fn get_dir_vec(&self) -> Vec2 {
        match self {
            Self::Left => Vec2::new(-1., 0.),
            Self::Right => Vec2::new(1., 0.),
            Self::Up => Vec2::new(0., 1.),
            Self::Down => Vec2::new(0., -1.),
        }
    }
    pub fn from_translation(translation: Vec2) -> Self {
        if translation.x.abs() > translation.y.abs() {
            if translation.x > 0. {
                return Self::Right;
            } else {
                return Self::Left;
            }
        } else {
            if translation.y > 0. {
                return Self::Up;
            } else {
                return Self::Down;
            }
        }
    }
    pub fn get_next_rand_dir(&self, mut rng: ThreadRng) -> &Self {
        let mut new_dir = self;
        while new_dir == self {
            let rng = rng.gen_range(0..=4);
            if rng <= 1 {
                new_dir = &Self::Left;
            } else if rng <= 2 {
                new_dir = &Self::Right;
            } else if rng <= 3 {
                new_dir = &Self::Up;
            } else if rng <= 4 {
                new_dir = &Self::Down;
            }
        }
        new_dir
    }
    pub fn new_rand_dir(mut rng: ThreadRng) -> Self {
        let mut new_dir = Self::Left;

        let rng = rng.gen_range(0..=4);
        if rng <= 1 {
            new_dir = Self::Left;
        } else if rng <= 2 {
            new_dir = Self::Right;
        } else if rng <= 3 {
            new_dir = Self::Up;
        } else if rng <= 4 {
            new_dir = Self::Down;
        }
        new_dir
    }
}

pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos::default())
            .register_type::<CursorPos>()
            // .add_plugin(ResourceInspectorPlugin::<CursorPos>::default())
            .with_default_schedule(CoreSchedule::FixedUpdate, |app| {
                app.add_event::<AttackEvent>();
            })
            .add_systems(
                (
                    move_player,
                    turn_player,
                    move_camera_with_player
                        .after(handle_player_raw_position)
                        .before(move_player),
                    mouse_click_system.after(CustomFlush),
                    handle_hotbar_key_input,
                    tick_dash_timer,
                    update_cursor_pos.after(move_player),
                    toggle_inventory,
                    close_container,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

fn turn_player(
    mut game: ResMut<Game>,
    player_query: Query<&FacingDirection, With<Player>>,
    cursor_pos: Res<CursorPos>,
    mut commands: Commands,
) {
    let dir = if cursor_pos.screen_coords.x > WIDTH / 2. {
        if cursor_pos.screen_coords.x < 3. * WIDTH / 4.
            && cursor_pos.screen_coords.y > HEIGHT * 0.65
        {
            FacingDirection::Up
        } else if cursor_pos.screen_coords.x < 3. * WIDTH / 4.
            && cursor_pos.screen_coords.y < HEIGHT * 0.35
        {
            FacingDirection::Down
        } else {
            FacingDirection::Right
        }
    } else {
        if cursor_pos.screen_coords.x > WIDTH / 4. && cursor_pos.screen_coords.y > HEIGHT * 0.65 {
            FacingDirection::Up
        } else if cursor_pos.screen_coords.x > WIDTH / 4.
            && cursor_pos.screen_coords.y < HEIGHT * 0.35
        {
            FacingDirection::Down
        } else {
            FacingDirection::Left
        }
    };
    //TODO: make center point based on player pos on screen?
    //TODO: add some way for attack to know dir
    let curr_dir = player_query.single();
    if &dir != curr_dir {
        commands.entity(game.player).insert(dir.clone());
        game.player_state.direction = dir.clone();
    }
}
pub fn move_player(
    mut game: GameParam,
    mut player_query: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            &mut MovementVector,
            &EnemyAnimationState,
            &CharacterAnimationSpriteSheetData,
            &TextureAtlasSprite,
            &Speed,
            &Hunger,
            &mut RunDustTimer,
        ),
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
    mut commands: Commands,
    mut particle: Query<&mut EffectSpawner, With<DustParticles>>,
) {
    let (
        player_e,
        mut player_kcc,
        mut mv,
        curr_anim,
        anim_state,
        sprite,
        speed,
        hunger,
        mut run_dust_timer,
    ) = player_query.single_mut();
    let player = game.player_mut();
    if player.is_attacking {
        mv.0 = Vec2::ZERO;
        return;
    }
    let mut d = Vec2::ZERO;
    let s = PLAYER_MOVE_SPEED
        * time.delta_seconds()
        * (1. + speed.0 as f32 / 100.)
        * (if hunger.is_starving() { 0.7 } else { 1. });

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
        let is_speeding_up = player.player_dash_duration.percent() < 0.5;
        if curr_anim != &EnemyAnimationState::Dash
            && (anim_state.is_done_current_animation(sprite.index)
                || curr_anim != &EnemyAnimationState::Attack)
        {
            commands.entity(player_e).insert(EnemyAnimationState::Dash);
        }
        d.x = if is_speeding_up {
            d.x.lerp(
                &(d.x * PLAYER_DASH_SPEED * TIME_STEP),
                &(player.player_dash_duration.percent() * 2.),
            )
        } else {
            d.x.lerp(
                &(d.x * PLAYER_DASH_SPEED * TIME_STEP),
                &(1. - (player.player_dash_duration.percent())),
            )
        };
        d.y = if is_speeding_up {
            d.y.lerp(
                &(d.y * PLAYER_DASH_SPEED * TIME_STEP),
                &(player.player_dash_duration.percent() * 2.),
            )
        } else {
            d.y.lerp(
                &(d.y * PLAYER_DASH_SPEED * TIME_STEP),
                &(1. - (player.player_dash_duration.percent())),
            )
        };
    }
    mv.0 = d;
    if d.x != 0. || d.y != 0. {
        player_kcc.translation = Some(Vec2::new(d.x, d.y));
        if curr_anim != &EnemyAnimationState::Walk
            && !player.is_dashing
            && anim_state.is_done_current_animation(sprite.index)
        {
            commands.entity(player_e).insert(EnemyAnimationState::Walk);
        }
        minimap_event.send(UpdateMiniMapEvent {
            pos: None,
            new_tile: None,
        });
        if run_dust_timer.0.percent() == 0. {
            particle.single_mut().reset();
            run_dust_timer.0.tick(time.delta());
        } else {
            run_dust_timer.0.tick(time.delta());
            if run_dust_timer.0.finished() {
                run_dust_timer.0.reset()
            }
        }
    } else {
        if curr_anim != &EnemyAnimationState::Idle
            && anim_state.is_done_current_animation(sprite.index)
        {
            commands.entity(player_e).insert(EnemyAnimationState::Idle);
        }
    }
}
pub fn tick_dash_timer(mut game: GameParam, time: Res<Time>) {
    let player = game.player_mut();
    if player.is_dashing {
        player.player_dash_duration.tick(time.delta());
        if player.player_dash_duration.just_finished() {
            player.player_dash_duration.reset();
            player.is_dashing = false;
        }
    }
}
pub fn close_container(
    key_input: ResMut<Input<KeyCode>>,
    mut inv_state: ResMut<InventoryState>,
    mut commands: Commands,
    stats_query: Query<Entity, With<StatsUI>>,
) {
    if key_input.just_pressed(KeyCode::Escape) {
        inv_state.open = false;
        if let Ok(e) = stats_query.get_single() {
            commands.entity(e).despawn_recursive();
        }
    }
}
pub fn toggle_inventory(
    mut game: GameParam,
    key_input: ResMut<Input<KeyCode>>,
    mut inv_state: ResMut<InventoryState>,
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    proto: ProtoParam,
    _inv: Query<&mut Inventory>,
) {
    if key_input.just_pressed(KeyCode::I)
        || key_input.just_pressed(KeyCode::Tab)
        || key_input.just_pressed(KeyCode::E)
    {
        inv_state.open = !inv_state.open;
    }

    if key_input.just_pressed(KeyCode::P) {
        spawn_new_dungeon_dimension(&mut game, &mut commands, &mut proto_commands);
    }
    if key_input.just_pressed(KeyCode::O) {
        dim_event.send(DimensionSpawnEvent {
            generation_params: proto.get_world_gen().unwrap(),
            swap_to_dim_now: true,
        });
    }
    if key_input.just_pressed(KeyCode::U) {
        commands
            .entity(game.game.player)
            .insert(FireStaffAOEUpgrade)
            .insert(LightningStaffChainUpgrade)
            .insert(BowUpgradeSpread(2))
            .insert(ArrowSpeedUpgrade(1.))
            .insert(BurnOnHitUpgrade)
            .insert(VenomOnHitUpgrade)
            .insert(LethalHitUpgrade)
            .insert(ClawUpgradeMultiThrow(
                Timer::from_seconds(0.1, TimerMode::Once),
                2,
            ));
    }
    if key_input.just_pressed(KeyCode::L) {
        let pos = tile_pos_to_world_pos(
            TileMapPosition::new(IVec2 { x: 0, y: 0 }, TilePos { x: 0, y: 0 }),
            true,
        );
        // proto_commands.spawn_from_proto(Mob::Slime, &proto.prototypes, pos);
        proto_commands.spawn_from_proto(Mob::StingFly, &proto.prototypes, pos);
        proto_commands.spawn_from_proto(Mob::Bushling, &proto.prototypes, pos);
        proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
        // proto_commands.spawn_from_proto(Mob::Slime, &proto.prototypes, pos);
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
                world_coords: cursor_pos_in_world(&windows, cursor_moved.position, cam_t, cam),
                ui_coords: cursor_pos_in_ui(&windows, cursor_moved.position, cam),
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
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    mut game: GameParam,
    mut proto_param: ProtoParam,
    mut attack_event: EventWriter<AttackEvent>,
    mut hit_event: EventWriter<HitEvent>,

    player_query: Query<(Entity, Option<&AttackTimer>), With<Player>>,
    inv: Query<&mut Inventory>,
    inv_state: Res<InventoryState>,
    ranged_query: Query<&RangedAttack, With<Equipment>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    mut item_action_param: ItemActionParam,
    obj_actions: Query<&ObjectAction>,
) {
    if inv_state.open {
        return;
    }

    let cursor_tile_pos = world_pos_to_tile_pos(cursor_pos.world_coords.truncate());
    let player_pos = game.player().position;
    let (player_e, attack_timer_option) = player_query.single();
    // Hit Item, send attack event
    if mouse_button_input.pressed(MouseButton::Left) {
        // println!("C: {cursor_tile_pos:?}",);
        if attack_timer_option.is_some() {
            return;
        }
        let mut main_hand_option = None;
        // if it has AttackTimer, the action is on cooldown, so we abort.
        if let Some(tool) = &game.player().main_hand_slot {
            main_hand_option = Some(tool.obj);
        }
        let direction =
            (cursor_pos.world_coords.truncate() - player_pos.truncate()).normalize_or_zero();
        if let Ok(ranged_tool) = ranged_query.get_single() {
            let mana_cost_option =
                proto_param.get_component::<ManaCost, _>(main_hand_option.unwrap());
            ranged_attack_event.send(RangedAttackEvent {
                projectile: ranged_tool.0.clone(),
                direction,
                from_enemy: None,
                is_followup_proj: false,
                mana_cost: mana_cost_option.map(|m| -m.0),
            })
        }
        commands
            .entity(player_e)
            .insert(EnemyAnimationState::Attack);
        attack_event.send(AttackEvent { direction });
        if player_pos
            .truncate()
            .distance(cursor_pos.world_coords.truncate())
            > game.player().reach_distance * 32.
            || ranged_query.get_single().is_ok()
        {
            return;
        }
        if let Some(hit_obj) = game.get_obj_entity_at_tile(cursor_tile_pos, &proto_param) {
            hit_event.send(HitEvent {
                hit_entity: hit_obj,
                damage: game.calculate_player_damage().0 as i32,
                dir: Vec2::new(0., 0.),
                hit_with_melee: main_hand_option,
                hit_with_projectile: None,
            });
        }
    }
    // Attempt to place block in hand
    if mouse_button_input.just_pressed(MouseButton::Right) {
        let hotbar_slot = inv_state.active_hotbar_slot;
        let held_item_option = inv.single().items.items[hotbar_slot].clone();
        if let Some(held_item) = held_item_option {
            let held_obj = *held_item.get_obj();
            if let Some(item_actions) = proto_param.get_component::<ItemActions, _>(held_obj) {
                item_actions.run_action(held_obj, &mut item_action_param, &mut game, &proto_param);
            }
        }
        if let Some(obj_e) = game.get_obj_entity_at_tile(cursor_tile_pos, &proto_param) {
            if let Ok(obj_action) = obj_actions.get(obj_e) {
                obj_action.run_action(
                    obj_e,
                    &mut game,
                    &mut item_action_param,
                    &mut commands,
                    &mut proto_param,
                );
            }
        }
    }
}

pub fn move_camera_with_player(
    player_query: Query<
        (&Transform, &RawPosition, &MovementVector),
        (
            With<Player>,
            // Changed<Transform>,
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
    let (_player_pos, raw_player_pos, player_movement_vec) = player_query.single();
    //     game_camera_transform.translation.x = game_camera_transform
    //         .translation
    //         .x
    //         .lerp(&player_pos.translation.x, &0.08);
    //     game_camera_transform.translation.y = game_camera_transform
    //         .translation
    //         .y
    //         .lerp(&player_pos.translation.y, &0.08);
    //     println!(
    //         "  c: {:?} {:?}",
    //         game_camera_transform.translation.x, game_camera_transform.translation.y
    //     );
    // }
    let camera_lookahead_scale = 15.0;
    raw_camera_pos.0 = raw_camera_pos
        .0
        .lerp(player_movement_vec.0 * camera_lookahead_scale, 0.08);
    let camera_final_pos = Vec2::new(
        raw_player_pos.x - raw_camera_pos.x,
        raw_player_pos.y - raw_camera_pos.y,
    );
    game_camera_transform.translation.x = camera_final_pos.x.trunc();
    game_camera_transform.translation.y = camera_final_pos.y.trunc();
    let (mut screen_camera_transform, game_upscale) = screen_camera.single_mut();
    screen_camera_transform.translation.x = camera_final_pos.x.fract() * game_upscale.0;
    screen_camera_transform.translation.y = camera_final_pos.y.fract() * game_upscale.0;
}
