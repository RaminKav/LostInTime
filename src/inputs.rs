use crate::attributes::ActiveConsumableBuffs;
use crate::blessings::OwnedBlessings;
use crate::chaos::ChaosTracker;
use crate::cursor::CursorPos;
use crate::gamepad_input::{
    gamepad_hotbar_just_pressed, gamepad_skill_just_pressed, gamepad_skill_pressed, GamepadAction,
};
use leafwing_input_manager::prelude::ActionState;
use std::time::Duration;

use crate::animations::player_sprite::PlayerAnimation;
use crate::animations::{AttackEvent, HitAnimationTracker};
use crate::assets::{Graphics, SpriteAnchor};
use crate::attributes::hunger::Hunger;
use crate::audio::{AudioSoundEffect, AudioVolume, SoundSpawner};
use crate::client::is_not_paused;
use crate::enemy::spawn_helpers::can_spawn_mob_here;
use crate::enemy::spawner::GlobalSpawners;
use crate::juice::{DustParticles, RunDustTimer};
use crate::player::skills::{
    ActiveSkill, ActiveSkillUsedEvent, ClassSkillSlots, Heirloom, PhasingThroughEnemies,
    PlayerSkills,
};
use crate::ui::key_input_guide::InteractionGuideTrigger;
use crate::world::dimension::{DimensionSpawnEvent, Era};
use bevy::prelude::*;
use bevy::transform::TransformSystem;
use bevy::window::PrimaryWindow;

use bevy_hanabi::EffectSpawner;
use bevy_proto::prelude::{ProtoCommands, ReflectSchematic, Schematic};

use bevy_rapier2d::prelude::{
    CollisionGroups, Group, KinematicCharacterController, KinematicCharacterControllerOutput,
    PhysicsSet,
};
use interpolation::Lerp;
use rand::rngs::ThreadRng;
use rand::seq::IteratorRandom;
use rand::Rng;

use crate::attributes::{CurrentMana, Speed};
use crate::combat::{AttackTimer, HitEvent};

use crate::enemy::Mob;
use crate::inventory::Inventory;
use crate::item::ammo::Ammo;
use crate::item::bridge_placement::{
    bridge_placement_blocks_player_attack, try_toggle_bridge_placement_mode, BridgePlacementMode,
};
use crate::item::item_actions::{ItemActionParam, ItemActions, ManaCost};
use crate::item::object_actions::ObjectAction;
use crate::item::projectile::{RangedAttack, RangedAttackEvent};
use crate::item::{Equipment, WorldObject};
use crate::proto::proto_param::ProtoParam;
use crate::ui::{
    class_selection::{ClassUnlockConfirmState, SkillUnlockConfirmState},
    tutorial_ui::PendingInventoryTutorialCheck,
    EssenceShopChoices, FlashExpBarEvent, MenuButton, MenuButtonClickEvent, UIState,
    WaitingForKeyInput, WipeDataPopup,
};
use crate::world::chunk::Chunk;

use crate::world::world_helpers::world_pos_to_tile_pos;

use crate::player::ice_slide::{clear_ice_slide_when_stuck, tick_ice_slide_movement};
use crate::{
    bounce_player, update_bounce_effect, update_shadow, BounceEffect, BounceEvent, Game,
    InputMappings, Player, ScreenResolution, DEBUG, PLAYER_DASH_SPEED, TIME_STEP,
};
use crate::{
    custom_commands::CommandsExt, CustomFlush, GameParam, GameState, MainCamera, RawPosition,
    TextureCamera, UICamera, PLAYER_MOVE_SPEED,
};

/// Number of hotbar slots that are bound to a consume/use key. Keep in sync with the
/// HUD's `HUD_HOTBAR_SLOTS` and the `InputMappings::hotbar_slot_*` fields.
pub const HOTBAR_CONSUME_SLOT_COUNT: usize = 4;
pub struct InputsPlugin;

impl Plugin for InputsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CursorPos::default())
            .insert_resource(AutoAttackState::load())
            .insert_resource(AttackAutoTargetState::load())
            .insert_resource(MouselessModeState::load())
            .insert_resource(SwapMovementAimKeysState::load())
            .init_resource::<PendingGroundAimSkill>()
            .insert_resource(crate::bounce::NaturalTornadoSpawner::default())
            .register_type::<CursorPos>()
            .add_event::<BounceEvent>()
            // AttackEvent must live on Update (default schedule), not FixedUpdate. Writers
            // (`mouse_click_system`, sprint run-attack) and readers (`handle_attack_cooldowns`,
            // stealth break, item abilities) all run on Update. FixedUpdate catch-up rotates
            // the event buffer multiple times per render frame when FPS drops, dropping events
            // before `handle_attack_cooldowns` inserts `AttackTimer` — causing burst attacks.
            .add_event::<AttackEvent>()
            .add_systems(
                (
                    bounce_player.run_if(is_not_paused),
                    update_shadow.run_if(is_not_paused),
                    update_bounce_effect,
                    crate::bounce::update_desert_tornadoes.run_if(is_not_paused),
                    crate::bounce::handle_tornado_player_overlap.run_if(is_not_paused),
                    crate::bounce::spawn_natural_desert_tornadoes.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    player_move_inputs.run_if(is_not_paused),
                    turn_player.run_if(is_not_paused),
                    mouse_click_system.run_if(is_not_paused).after(CustomFlush),
                    dispatch_active_skill_events
                        .run_if(is_not_paused)
                        .after(handle_hotbar_consume_keys),
                    handle_roll
                        .run_if(is_not_paused)
                        .after(dispatch_active_skill_events)
                        .before(crate::player::skill_heirlooms::handle_active_skill_event),
                    handle_hotbar_consume_keys
                        .run_if(is_not_paused)
                        .before(dispatch_active_skill_events),
                    tick_dash_timer.run_if(is_not_paused),
                    manage_ability_phasing.run_if(is_not_paused),
                    handle_open_essence_ui,
                    diagnostics,
                    handle_interact_objects.run_if(is_not_paused),
                    toggle_attack_auto_target.run_if(is_not_paused),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((
                toggle_inventory.run_if(in_state(GameState::Main)),
                close_container
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::MainMenu))),
            ))
            .add_system(
                move_camera_with_player
                    .after(PhysicsSet::SyncBackendFlush)
                    .before(TransformSystem::TransformPropagate)
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            );
    }
}

#[derive(Resource, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AutoAttackState(pub bool);

impl Default for AutoAttackState {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Resource, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AttackAutoTargetState(pub bool);

impl Default for AttackAutoTargetState {
    fn default() -> Self {
        Self(false)
    }
}

/// Options screen toggle: "Mouseless Mode". When on, arrow keys stop moving the player
/// (WASD alone drives movement) and instead act as a virtual aim stick — steering facing /
/// attacks / instant skills the same way the mouse cursor normally would, and letting
/// ground-targeted skills (`ActiveSkill::is_ground_targeted`) be aimed with a hold-and-release
/// reticle instead of firing instantly. See `src/aim.rs` for the aim/reticle systems (shared
/// with gamepad right-stick aiming) and `dispatch_active_skill_events` for the hold-to-aim
/// gating. Exists mainly so twin-stick aim logic can be exercised on keyboard alone,
/// sidestepping the current macOS gamepad hardware-support gap (see `gamepad_input.rs`).
#[derive(Resource, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MouselessModeState(pub bool);

impl Default for MouselessModeState {
    fn default() -> Self {
        Self(false)
    }
}

impl MouselessModeState {
    pub fn load() -> Self {
        let path = crate::datafiles::game_data();
        if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.mouseless_mode.unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = crate::datafiles::game_data();
        let mut game_data = if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.mouseless_mode = Some(*self);

        if let Ok(file) = std::fs::File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

/// Which active-skill slot (if any) is currently mid-"hold to aim" — the button is being held
/// so the ground-target reticle is showing, but the skill hasn't fired yet. Set while charging
/// via gamepad, or via keyboard/mouse while `MouselessModeState` is on; see
/// `dispatch_active_skill_events`.
#[derive(Resource, Default)]
pub struct PendingGroundAimSkill(pub Option<usize>);

/// Options screen toggle (under "Toggles", alongside Mouseless Mode): when on, swaps which key
/// group drives movement vs. aim while `MouselessModeState` is active — arrow keys move and
/// WASD aims, instead of the default WASD-moves/arrows-aim. For left-handed players or anyone
/// who prefers the opposite hand on movement. Has no effect while Mouseless Mode is off (both
/// WASD and arrows always move then, same as always). See `player_move_inputs` and
/// `aim::update_aim_state`.
#[derive(Resource, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SwapMovementAimKeysState(pub bool);

impl Default for SwapMovementAimKeysState {
    fn default() -> Self {
        Self(false)
    }
}

impl SwapMovementAimKeysState {
    pub fn load() -> Self {
        let path = crate::datafiles::game_data();
        if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.swap_movement_aim_keys.unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = crate::datafiles::game_data();
        let mut game_data = if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.swap_movement_aim_keys = Some(*self);

        if let Ok(file) = std::fs::File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

impl AttackAutoTargetState {
    pub fn load() -> Self {
        let path = crate::datafiles::game_data();
        if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.attack_auto_target.unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = crate::datafiles::game_data();
        let mut game_data = if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.attack_auto_target = Some(*self);

        if let Ok(file) = std::fs::File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

/// Aim direction for weapon attacks. When auto-target is enabled, aims at the
/// nearest mob; otherwise uses the cursor position.
pub fn attack_aim_direction(
    player_pos: Vec2,
    cursor_pos: Vec2,
    auto_target: bool,
    enemies: &Query<&GlobalTransform, With<Mob>>,
) -> Vec2 {
    if auto_target {
        let mut nearest_pos = None;
        let mut nearest_dist_sq = f32::MAX;
        for enemy_txfm in enemies.iter() {
            let enemy_pos = enemy_txfm.translation().truncate();
            let dist_sq = player_pos.distance_squared(enemy_pos);
            if dist_sq < nearest_dist_sq {
                nearest_dist_sq = dist_sq;
                nearest_pos = Some(enemy_pos);
            }
        }
        if let Some(target_pos) = nearest_pos {
            let dir = target_pos - player_pos;
            if dir.length_squared() > 1e-4 {
                return dir.normalize();
            }
        }
    }
    (cursor_pos - player_pos).normalize_or_zero()
}

pub fn weapon_projectile_spawn_delay(obj: &WorldObject, burst_index: usize) -> f32 {
    if burst_index > 0 {
        return 0.2;
    }
    if obj == &WorldObject::WoodBow {
        0.12
    } else {
        0.01
    }
}

fn toggle_attack_auto_target(
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    ui_state: Res<State<UIState>>,
    mut auto_target: ResMut<AttackAutoTargetState>,
    mut commands: Commands,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    if ui_state.0 != UIState::Closed {
        return;
    }
    let gamepad_pressed = gamepad_action_q
        .get_single()
        .map(|a| a.just_pressed(GamepadAction::AutoTarget))
        .unwrap_or(false);
    if !keybinds.check_attack_auto_target_input(&key_input, &mouse_input) && !gamepad_pressed {
        return;
    }
    auto_target.0 = !auto_target.0;
    auto_target.save();
    commands.spawn(SoundSpawner::new(AudioSoundEffect::UISkillSelection, 0.1));
}

impl AutoAttackState {
    pub fn load() -> Self {
        let path = crate::datafiles::game_data();
        if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.auto_attack.unwrap_or_default();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = crate::datafiles::game_data();
        let mut game_data = if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.auto_attack = Some(*self);

        if let Ok(file) = std::fs::File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
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
    pub fn get_anim_dir_str(&self) -> &str {
        match self {
            Self::Left => "Side",
            Self::Right => "Side",
            Self::Up => "Back",
            Self::Down => "Front",
        }
    }
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
                Self::Right
            } else {
                Self::Left
            }
        } else if translation.y > 0. {
            Self::Up
        } else {
            Self::Down
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

fn turn_player(
    mut game: ResMut<Game>,
    player_query: Query<&FacingDirection, With<Player>>,
    cursor_pos: Res<CursorPos>,
    auto_target: Res<AttackAutoTargetState>,
    enemies: Query<&GlobalTransform, With<Mob>>,
    mut commands: Commands,
) {
    let player_pos = game.player_state.position.truncate();
    let aim = attack_aim_direction(
        player_pos,
        cursor_pos.world_coords.truncate(),
        auto_target.0,
        &enemies,
    );
    if aim.length_squared() < 1e-4 {
        return;
    }
    let dir = FacingDirection::from_translation(aim);
    let curr_dir = player_query.single();
    if &dir != curr_dir {
        commands.entity(game.player).insert(dir.clone());
        game.player_state.direction = dir.clone();
    }
}
pub fn player_move_inputs(
    mut game: GameParam,
    mut player_query: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            &mut MovementVector,
            &PlayerAnimation,
            &Speed,
            &Hunger,
            &mut RunDustTimer,
            Option<&BounceEffect>,
            &ActiveConsumableBuffs,
            Option<&HitAnimationTracker>,
            Option<&KinematicCharacterControllerOutput>,
        ),
        (
            With<Player>,
            Without<MainCamera>,
            Without<Chunk>,
            Without<Equipment>,
        ),
    >,
    time: Res<Time>,
    key_input: Res<Input<KeyCode>>,
    mut commands: Commands,
    mut particle: Query<&mut EffectSpawner, With<DustParticles>>,
    asset_server: Res<AssetServer>,
    audio: Res<Audio>,
    audio_volume: Res<AudioVolume>,
    mut audio_timer: Local<Timer>,
    mut ammo_query: Query<&mut Ammo>,
    proto_param: ProtoParam,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
    mouseless_mode: Res<MouselessModeState>,
    swap_keys: Res<SwapMovementAimKeysState>,
) {
    if audio_timer.duration() == Duration::ZERO {
        *audio_timer = Timer::from_seconds(0.2, TimerMode::Once);
    }
    let (
        player_e,
        mut player_kcc,
        mut mv,
        curr_anim,
        speed,
        hunger,
        mut run_dust_timer,
        bounce_option,
        consumable_buffs,
        hit_tracker_option,
        kcc_output,
    ) = player_query.single_mut();
    if bounce_option.is_some() {
        return;
    }
    let player_world_pos = game.player().position.truncate();
    let player_tile = world_pos_to_tile_pos(player_world_pos);
    let on_ice = game
        .get_obj_entity_at_tile(player_tile, &proto_param)
        .map(|(_, obj)| obj == WorldObject::IcePatch)
        .unwrap_or(false);

    let mut player = game.player_mut();
    let mut d_raw = Vec2::ZERO;
    let movement_speed_multiplier = consumable_buffs.movement_multiplier_product();

    let s = PLAYER_MOVE_SPEED
        * time.delta_seconds()
        * (1. + speed.0 as f32 / 100.)
        * (if hunger.is_starving() { 0.7 } else { 1. })
        * movement_speed_multiplier;

    // Left stick takes priority over WASD for this frame when it's outside the deadzone;
    // otherwise fall back to keyboard so mixed keyboard/gamepad play keeps working.
    let gamepad_move = gamepad_action_q.get_single().ok().and_then(|action_state| {
        action_state
            .clamped_axis_pair(GamepadAction::Move)
            .map(|pair| pair.xy())
    });
    if let Some(stick) = gamepad_move.filter(|v| {
        v.length_squared() > crate::gamepad_input::GAMEPAD_STICK_DEADZONE.powi(2)
    }) {
        d_raw = stick;
        player.is_moving = true;
    } else {
        // In Mouseless Mode, one key group moves and the other aims (see `aim.rs`);
        // `SwapMovementAimKeysState` picks which is which (arrows move / WASD aims, instead of
        // the default WASD moves / arrows aim). Outside Mouseless Mode both groups always move,
        // same as always.
        let (wasd_moves, arrows_move) = if mouseless_mode.0 {
            (!swap_keys.0, swap_keys.0)
        } else {
            (true, true)
        };
        if (wasd_moves && key_input.pressed(KeyCode::A))
            || (arrows_move && key_input.pressed(KeyCode::Left))
        {
            d_raw.x -= 1.;
            player.is_moving = true;
        }
        if (wasd_moves && key_input.pressed(KeyCode::D))
            || (arrows_move && key_input.pressed(KeyCode::Right))
        {
            d_raw.x += 1.;
            player.is_moving = true;
        }
        if (wasd_moves && key_input.pressed(KeyCode::W))
            || (arrows_move && key_input.pressed(KeyCode::Up))
        {
            d_raw.y += 1.;
            player.is_moving = true;
        }
        if (wasd_moves && key_input.pressed(KeyCode::S))
            || (arrows_move && key_input.pressed(KeyCode::Down))
        {
            d_raw.y -= 1.;
            player.is_moving = true;
        }
    }
    clear_ice_slide_when_stuck(&mut player, on_ice, d_raw, kcc_output);

    let is_dashing = player.is_dashing;
    let hit_active = hit_tracker_option.map_or(false, |h| h.is_active);
    let mut d = tick_ice_slide_movement(
        &mut player,
        on_ice,
        d_raw,
        s,
        time.delta_seconds(),
        is_dashing,
        hit_active,
    );

    if (key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
        && !key_input.any_pressed([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]))
        || (d_raw.x == 0. && d_raw.y == 0.)
    {
        let sliding_ice = on_ice && player.ice_slide_direction.is_some();
        let sliding_momentum =
            player.ice_momentum_remaining > 0.0 && player.ice_momentum_direction.is_some();
        if !(sliding_ice || sliding_momentum) {
            player.is_moving = false;
        }
    }

    // Manual reload on R key for current ranged weapon
    if key_input.just_pressed(KeyCode::R) {
        if let Some(main_hand) = player.main_hand_slot.clone() {
            if let Ok(mut ammo) = ammo_query.get_mut(main_hand.entity) {
                if !ammo.reloading && ammo.current < ammo.max {
                    ammo.start_reload();
                }
            }
        }
    }
    let is_speeding_up = player.player_dash_duration.percent() < 0.5;
    if player.is_dashing {
        if curr_anim != &PlayerAnimation::Roll && player.player_dash_duration.percent() == 0. {
            commands.entity(player_e).insert(PlayerAnimation::Roll);
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
    // Don't overwrite KCC translation while knockback is active (set by animate_hit)
    if !hit_active && (d.x != 0. || d.y != 0.) {
        player_kcc.translation = Some(Vec2::new(d.x, d.y));

        if curr_anim == &PlayerAnimation::Idle {
            commands.entity(player_e).insert(PlayerAnimation::Walk);
        }

        if run_dust_timer.0.percent() == 0. {
            if let Ok(mut p) = particle.get_single_mut() {
                p.reset();
            }
            run_dust_timer.0.tick(time.delta());
        } else {
            run_dust_timer.0.tick(time.delta());
            if run_dust_timer.0.finished() {
                run_dust_timer.0.reset()
            }
        }
        //audio
        audio_timer.tick(time.delta());
        if audio_timer.finished() {
            audio_timer.reset();
            let walk1 = asset_server.load("sounds/walk_grass1.ogg");
            let walk2 = asset_server.load("sounds/walk_grass2.ogg");
            let walk3 = asset_server.load("sounds/walk_grass3.ogg");
            let walk4 = asset_server.load("sounds/walk_grass4.ogg");
            let walk5 = asset_server.load("sounds/walk_grass5.ogg");
            let walks = vec![walk1, walk2, walk3, walk4, walk5];
            let sfx = audio_volume.sfx_fraction();
            walks.iter().choose(&mut rand::thread_rng()).map(|sound| {
                audio.play_with_settings(
                    sound.clone(),
                    PlaybackSettings::ONCE.with_volume(0.35 * sfx),
                )
            });
        }
    } else if curr_anim.is_walking() {
        commands.entity(player_e).insert(PlayerAnimation::Idle);
    }
}

pub fn dispatch_active_skill_events(
    mut ev: EventWriter<ActiveSkillUsedEvent>,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    player_q: Query<(&PlayerSkills, &ClassSkillSlots), With<Player>>,
    blessings_q: Query<&OwnedBlessings, With<Player>>,
    keybinds: Res<crate::keybinds::InputMappings>,
    bridge_mode: Res<BridgePlacementMode>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
    mouseless_mode: Res<MouselessModeState>,
    mut pending_ground_aim: ResMut<PendingGroundAimSkill>,
) {
    if bridge_mode.active {
        return;
    }
    let Ok((skills, class_slots)) = player_q.get_single() else {
        return;
    };
    let Ok(blessings) = blessings_q.get_single() else {
        return;
    };
    let gamepad_action_state = gamepad_action_q.get_single().ok();

    // Already charging a ground-targeted skill's hold-to-aim reticle: only watch for the
    // button being released (fire) — don't look for a new press until this resolves. Checked
    // against whichever input(s) can still be holding it (keyboard/mouse and gamepad are
    // OR'd together since either one may have started the charge).
    if let Some(slot) = pending_ground_aim.0 {
        let still_held = keybinds.check_skill_input_held(slot, &key_input, &mouse_input)
            || gamepad_skill_pressed(gamepad_action_state, slot);
        if !still_held {
            pending_ground_aim.0 = None;
            if let Some(skill) = skills.get_active_skill_in_slot(slot) {
                let effective_cd = skills.effective_skill_cooldown(&skill, blessings);
                let s = &class_slots.0[slot];
                if s.max_charges > 0 && s.current_charges > 0 {
                    ev.send(ActiveSkillUsedEvent {
                        slot,
                        cooldown: effective_cd,
                    });
                }
            }
        }
        return;
    }

    // Higher slot index wins when multiple bindings match; skip empty skill slots so a
    // hidden default (e.g. slot 3 still on Shift) cannot block visible slots 0–2.
    let pressed_slot = [3usize, 2, 1, 0, 4].into_iter().find(|&slot| {
        skills.get_active_skill_in_slot(slot).is_some()
            && (keybinds.check_skill_input(slot, &key_input, &mouse_input)
                || gamepad_skill_just_pressed(gamepad_action_state, slot))
    });

    if let Some(slot) = pressed_slot {
        if let Some(skill) = skills.get_active_skill_in_slot(slot) {
            let s = &class_slots.0[slot];
            if s.max_charges == 0 || s.current_charges == 0 {
                return;
            }
            // Ground-targeted skills use a hold-to-aim-then-release flow instead of firing
            // instantly at whatever the aim point happens to be right now: always for
            // gamepad (a stick-driven reticle is the natural way to place these with a
            // controller — see `gamepad_input.rs`), and for keyboard/mouse only while
            // Mouseless Mode is on (reticle steered by `aim.rs`). Plain mouse play
            // stays instant either way — the real cursor is already precisely positioned
            // before you click, so there's nothing to gain from a hold step.
            let via_keyboard_mouse = keybinds.check_skill_input(slot, &key_input, &mouse_input);
            let via_gamepad = gamepad_skill_just_pressed(gamepad_action_state, slot);
            if skill.is_ground_targeted()
                && (via_gamepad || (mouseless_mode.0 && via_keyboard_mouse))
            {
                pending_ground_aim.0 = Some(slot);
                return;
            }
            let effective_cd = skills.effective_skill_cooldown(&skill, blessings);
            ev.send(ActiveSkillUsedEvent {
                slot,
                cooldown: effective_cd,
            });
        }
    }
}
/// Triggers the dash motion when the Roll skill is cast. Roll is dispatched like any other
/// active skill (`dispatch_active_skill_events`), and charge/cooldown bookkeeping lives in
/// `handle_active_skill_event`. This system only reacts to the event by starting the dash —
/// `player_move_inputs` then applies the actual movement from `is_dashing`.
pub fn handle_roll(
    mut active_skill_events: EventReader<ActiveSkillUsedEvent>,
    mut commands: Commands,
    mut game: GameParam,
    player_q: Query<(Entity, &PlayerSkills), With<Player>>,
    bridge_mode: Res<BridgePlacementMode>,
) {
    if bridge_mode.active {
        return;
    }
    let Ok((player_e, skills)) = player_q.get_single() else {
        return;
    };
    let Some(roll_slot) = skills.has_active_skill(ActiveSkill::Roll) else {
        active_skill_events.clear();
        return;
    };
    let mut should_dash = false;
    for ev in active_skill_events.iter() {
        if ev.slot == roll_slot {
            should_dash = true;
        }
    }
    if !should_dash {
        return;
    }

    let player = game.player_mut();
    player.is_dashing = true;
    player.ice_slide_direction = None;
    player.ice_momentum_remaining = 0.0;
    player.ice_momentum_direction = None;
    player.ice_slide_speed_factor = 1.0;
    // Restart the dash motion so back-to-back rolls (e.g. extra Paintbrush charges) each
    // get a fresh dash arc instead of resuming a partially-elapsed one.
    player.player_dash_duration.reset();

    commands
        .entity(player_e)
        .insert(PhasingThroughEnemies::new(0.28));
    commands.spawn(SoundSpawner::new(AudioSoundEffect::Roll, 0.25));
}

pub fn tick_dash_timer(mut game: GameParam, time: Res<Time>) {
    let player = game.player_mut();

    if player.is_dashing {
        player.player_dash_duration.tick(time.delta());
        if player.player_dash_duration.just_finished() {
            player.player_dash_duration.reset();
            player.is_dashing = false;
        }
    } else {
        player.player_dash_cooldown.tick(time.delta());
    }
}

pub fn manage_ability_phasing(
    mut commands: Commands,
    mut player_query: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            Option<&mut PhasingThroughEnemies>,
        ),
        With<Player>,
    >,
    time: Res<Time>,
) {
    let Ok((entity, mut kcc, phasing_opt)) = player_query.get_single_mut() else {
        return;
    };

    if let Some(mut phasing) = phasing_opt {
        phasing.timer.tick(time.delta());

        if phasing.timer.finished() {
            commands
                .entity(entity)
                .insert(CollisionGroups::new(Group::ALL, Group::ALL))
                .remove::<PhasingThroughEnemies>();
            kcc.filter_groups = Some(CollisionGroups::new(Group::ALL, Group::ALL));
        } else {
            commands
                .entity(entity)
                .insert(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
            kcc.filter_groups = Some(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
        }
    }
}

/// Routes Escape through the same [`MenuButtonClickEvent`] handlers as visible Back buttons
/// (and related cancel buttons), so cleanup, sound, and guard logic stay in one place.
pub fn close_container(
    key_input: Res<Input<KeyCode>>,
    curr_state: Res<State<UIState>>,
    game_state: Res<State<GameState>>,
    waiting_for_key: Query<(), With<WaitingForKeyInput>>,
    wipe_popup: Query<(), With<WipeDataPopup>>,
    class_confirm: Res<ClassUnlockConfirmState>,
    skill_confirm: Res<SkillUnlockConfirmState>,
    mut menu_button_events: EventWriter<MenuButtonClickEvent>,
    mut commands: Commands,
) {
    if !key_input.just_pressed(KeyCode::Escape) {
        return;
    }

    // Options key-rebind capture handles Escape itself.
    if !waiting_for_key.is_empty() {
        return;
    }

    let button = if game_state.0 == GameState::Main && curr_state.0 == UIState::Closed {
        MenuButton::Options
    } else if curr_state.0 == UIState::Closed {
        return;
    } else if matches!(curr_state.0, UIState::ItemChest | UIState::Skills) {
        return;
    } else if !wipe_popup.is_empty() {
        MenuButton::WipeDataCancel
    } else if class_confirm.active {
        MenuButton::ClassUnlockNo
    } else if skill_confirm.active {
        MenuButton::SkillUnlockNo
    } else {
        MenuButton::Back
    };

    menu_button_events.send(MenuButtonClickEvent { button });
    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
}
pub fn toggle_inventory(
    mut commands: Commands,
    mut game: GameParam,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    mut proto_commands: ProtoCommands,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    proto: ProtoParam,
    inv: Query<&Inventory>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    cursor: Res<CursorPos>,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    keybinds: Res<InputMappings>,
    mut chaos_tracker: ResMut<ChaosTracker>,
) {
    if keybinds.check_inv_input(&key_input, &mouse_input) {
        // Don't allow opening inventory while item chest is open
        if curr_ui_state.0 != UIState::ItemChest {
            // Closing the inventory from `InventoryCrafting` still acts as a toggle off.
            // `handle_new_ui_state` maps a no-op transition (next == current) to `Closed`,
            // so re-using the current state here hands that off correctly.
            let target = if curr_ui_state.0 == UIState::InventoryCrafting {
                UIState::InventoryCrafting
            } else {
                UIState::Inventory
            };
            let opening_inventory =
                curr_ui_state.0 == UIState::Closed && target == UIState::Inventory;
            next_ui_state.set(target);

            if opening_inventory {
                commands.insert_resource(PendingInventoryTutorialCheck);
            }
        }
    }
    if *DEBUG {
        if key_input.just_pressed(KeyCode::P) {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(Era::DungeonMain),
            });
        }
        if key_input.just_pressed(KeyCode::O) {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(Era::Third),
            });
        }
        if key_input.just_pressed(KeyCode::C) {
            let xp_rate_bonus = game.get_xp_rate_bonus();
            let (did_level, gained_xp) =
                game.get_player_level_mut()
                    .add_xp(200, xp_rate_bonus, &mut chaos_tracker);

            flash_event.send(FlashExpBarEvent {
                amount: gained_xp,
                did_level,
            });
        }
        if key_input.just_pressed(KeyCode::K) {
            dim_event.send(DimensionSpawnEvent {
                swap_to_dim_now: true,
                new_era: Some(Era::Main),
            });
        }

        if key_input.just_pressed(KeyCode::L) {
            let pos = cursor.world_coords.truncate();
            if !can_spawn_mob_here(pos, &game, &proto, false) {
                return;
            }
            // proto_commands.spawn_item_from_proto(WorldObject::Crate, &proto, pos, 1, None);
            // proto_commands.spawn_item_from_proto(WorldObject::TimeFragment, &proto, pos, 1, None);
            // proto_commands.spawn_item_from_proto(WorldObject::Dagger, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::WoodBow, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::Claw, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::IceStaff, &proto, pos, 1, Some(5));
            // proto_commands.spawn_item_from_proto(WorldObject::BasicStaff, &proto, pos, 1, Some(5));
            // proto_commands.spawn_from_proto(Mob::VoidCrawler, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Lizard, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::StingFly, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::VoidWorm, &proto.prototypes, pos);
            proto_commands.spawn_from_proto(Mob::StoneGolem, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::BigCactus, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::SmallCactus, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Bull, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::Fairy, &proto.prototypes, pos);
            // proto_commands.spawn_from_proto(Mob::FurDevil, &proto.prototypes, pos);
            // commands.entity(t.unwrap()).insert(MobLevel(10));
            // proto_commands.spawn_from_proto(Mob::RedMushking, &proto.prototypes, pos);
            // let f = proto_commands.spawn_from_proto(Mob::SpikeSlime, &proto.prototypes, pos);
            // commands.entity(f.unwrap()).insert(MobLevel(10));
            // proto_commands.spawn_from_proto(Mob::Slime, &proto.prototypes, pos);
        }
    }
}
/// The user-configurable hotbar keys (defaults `1`-`4`) directly consume / use the item
/// stored in hotbar slots 0-3. There is no longer a "selected hotbar slot" concept — pressing
/// the key for slot `i` runs the `ItemActions` of the item currently in `items.items[i]`.
///
/// Slots 4-5 are still part of the hotbar container for passive storage but have no binding.
pub fn handle_hotbar_consume_keys(
    key_input: Res<Input<KeyCode>>,
    mut mouse_input: ResMut<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    mut game: GameParam,
    proto_param: ProtoParam,
    mut commands: Commands,
    graphics: Res<Graphics>,
    inv: Query<&Inventory>,
    mut item_action_param: ItemActionParam,
    cursor_pos: Res<CursorPos>,
    ui_state: Res<State<UIState>>,
    resolution: Res<ScreenResolution>,
    mut bridge_mode: ResMut<BridgePlacementMode>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    let gamepad_action_state = gamepad_action_q.get_single().ok();
    // Left-clicking a HUD hotbar slot (while the inventory is closed) triggers the same
    // consume action as pressing that slot's bound key.
    let mut clicked_slot: Option<usize> = None;
    if ui_state.0 == UIState::Closed && mouse_input.just_pressed(MouseButton::Left) {
        let cursor = cursor_pos.ui_coords.truncate();
        let slot_y = -resolution.game_height * 0.5 + crate::ui::HUD_ACTION_ROW_Y_FROM_BOTTOM;
        let half = crate::ui::UI_SLOT_SIZE * 0.5;
        for slot in 0..HOTBAR_CONSUME_SLOT_COUNT {
            let slot_x = crate::ui::hud_hotbar_slot_center_x(slot);
            if (cursor.x - slot_x).abs() <= half.x && (cursor.y - slot_y).abs() <= half.y {
                clicked_slot = Some(slot);
                break;
            }
        }
        if clicked_slot.is_some() {
            // Prevent this click from also triggering a player attack.
            mouse_input.clear_just_pressed(MouseButton::Left);
        }
    }

    for slot in 0..HOTBAR_CONSUME_SLOT_COUNT {
        let triggered_by_click = clicked_slot == Some(slot);
        let triggered_by_gamepad = gamepad_hotbar_just_pressed(gamepad_action_state, slot);
        if !triggered_by_click
            && !triggered_by_gamepad
            && !keybinds.check_hotbar_input(slot, &key_input, &mouse_input)
        {
            continue;
        }
        let held_item_option = inv.single().items.items[slot].clone();
        let Some(held_item) = held_item_option else {
            continue;
        };
        let held_obj = *held_item.get_obj();
        if try_toggle_bridge_placement_mode(
            slot,
            &held_item.item_stack,
            &mut bridge_mode,
            &mut commands,
            &graphics,
        ) {
            continue;
        }
        let Some(item_actions) = proto_param.get_component::<ItemActions, _>(held_obj) else {
            continue;
        };
        item_actions.run_action(
            held_obj,
            held_item.slot,
            Some(&held_item.item_stack),
            &mut item_action_param,
            &mut game,
            &proto_param,
            &mut commands,
        );
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

    let t = Transform::from_translation(Vec3::ZERO);
    let ndc_to_world = t.compute_matrix() * cam.projection_matrix().inverse();
    let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
    ndc_to_world.project_point3(ndc.extend(0.0))
}
pub fn diagnostics(
    mouse_button_input: Res<Input<MouseButton>>,
    entities: Query<Entity>,
    mobs: Query<&Mob>,
    spawners: Res<GlobalSpawners>,
) {
    if mouse_button_input.just_pressed(MouseButton::Right) {
        debug!("Entity Count: {:?}", entities.iter().count());
        debug!("Mob Count: {:?}", mobs.iter().count());
        debug!("Spawner Count: {:?}", spawners.spawners.iter().count());
    }
}
pub fn mouse_click_system(
    mut commands: Commands,
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    mut game: GameParam,
    mut proto_param: ProtoParam,
    mut attack_event: EventWriter<AttackEvent>,
    mut hit_event: EventWriter<HitEvent>,

    mut player_query: Query<
        (
            Entity,
            Option<&AttackTimer>,
            &PlayerAnimation,
            &OwnedBlessings,
            &mut CurrentMana,
            Option<&ActionState<GamepadAction>>,
        ),
        With<Player>,
    >,
    ui_state: Res<State<UIState>>,
    ranged_query: Query<(&WorldObject, &RangedAttack), With<Equipment>>,
    mut ranged_attack_event: EventWriter<RangedAttackEvent>,
    ammo_query_any: Query<&Ammo>,
    auto_attack: Res<AutoAttackState>,
    auto_target: Res<AttackAutoTargetState>,
    enemies: Query<&GlobalTransform, With<Mob>>,
    bridge_mode: Res<BridgePlacementMode>,
) {
    if ui_state.0 != UIState::Closed {
        return;
    }
    if bridge_placement_blocks_player_attack(bridge_mode) {
        return;
    }

    let cursor_tile_pos = world_pos_to_tile_pos(cursor_pos.world_coords.truncate());
    let player_pos = game.player().position;
    let (player_e, attack_timer_option, player_anim, blessings, mut current_mana, gamepad_action_state) =
        player_query.single_mut();
    let gamepad_attack_pressed = gamepad_action_state
        .map(|a| a.pressed(GamepadAction::Attack))
        .unwrap_or(false);

    // Hit Item, send attack event
    if mouse_button_input.pressed(MouseButton::Left) || auto_attack.0 || gamepad_attack_pressed {
        if *DEBUG && mouse_button_input.just_pressed(MouseButton::Left) {
            let obj = game.get_object_from_chunk_cache(cursor_tile_pos);
            info!(
                "C: {cursor_tile_pos:?} -> {obj:?} {:?}",
                cursor_pos.ui_coords,
            );
        }
        if attack_timer_option.is_some() || player_anim.is_one_time_anim() {
            return;
        }

        // AttackManaCost blessing: check mana cost and deduct before attacking
        let attack_mana_cost = blessings.get_attack_mana_cost();
        if attack_mana_cost > 0 {
            if current_mana.0 < attack_mana_cost {
                return;
            }
            current_mana.0 -= attack_mana_cost;
        }

        let mut main_hand_option = None;
        // if it has AttackTimer, the action is on cooldown, so we abort.
        if let Some(tool) = &game.player().main_hand_slot {
            main_hand_option = Some(tool.get_obj());
        }
        let direction = attack_aim_direction(
            player_pos.truncate(),
            cursor_pos.world_coords.truncate(),
            auto_target.0,
            &enemies,
        );
        if let Ok((obj, ranged_tool)) = ranged_query.get_single() {
            // Gate ranged attacks on ammo availability for non-magic ranged weapons
            if obj.is_ranged_weapon() && !obj.is_magic_weapon() {
                if let Some(main_hand) = game.player().main_hand_slot.clone() {
                    if let Ok(ammo) = ammo_query_any.get(main_hand.entity) {
                        if !ammo.can_fire() {
                            // Out of ammo: do not send any attack or animation for ranged
                            return;
                        }
                    } else {
                        // No ammo component yet: treat as empty and block attack until projectile handler initializes
                        return;
                    }
                }
            }
            let mana_cost_option =
                proto_param.get_component::<ManaCost, _>(main_hand_option.unwrap());
            let mut rng = rand::thread_rng();
            let trigger_count = if game.has_skill(Heirloom::ChanceToProcExtraAttack)
                && rng.gen_bool(
                    (game.skill_count(Heirloom::ChanceToProcExtraAttack) as f64 * 0.25)
                        .clamp(0., 1.),
                ) {
                let mana_cost = Heirloom::ChanceToProcExtraAttack.get_mana_cost();
                if current_mana.0 >= mana_cost {
                    current_mana.0 -= mana_cost;
                    game.heirloom_trigger_counts
                        .record_mana(Heirloom::ChanceToProcExtraAttack, mana_cost);
                    game.heirloom_trigger_counts
                        .increment(Heirloom::ChanceToProcExtraAttack);
                    2
                } else {
                    1
                }
            } else {
                1
            };
            for i in 0..trigger_count {
                ranged_attack_event.send(RangedAttackEvent {
                    projectile: ranged_tool.0.clone(),
                    direction,
                    from_enemy: false,
                    is_followup_proj: false,
                    from_entity: None,
                    mana_cost: mana_cost_option.map(|m| m.0),
                    mana_cost_heirloom: None,
                    dmg_override: None,
                    pos_override: if ranged_tool.0.is_anchored_to_player_pos() {
                        Some(Vec2::ZERO)
                    } else {
                        None
                    },
                    spawn_delay: weapon_projectile_spawn_delay(obj, i),
                })
            }
        }
        let mut did_attack = false;
        if let Some(main_hand) = main_hand_option {
            if main_hand == WorldObject::WoodBow {
                commands
                    .entity(player_e)
                    .insert(PlayerAnimation::Bow)
                    .insert(crate::animations::player_sprite::AttackAnimationTimer(
                        Timer::from_seconds(1.0, TimerMode::Once),
                    ));
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: false,
                });
            } else if main_hand.is_melee_weapon()
                || main_hand.is_ranged_weapon()
                || main_hand.is_tool()
            {
                if !player_anim.is_sprinting() {
                    did_attack = true;
                }
            }
        }
        if did_attack {
            commands
                .entity(player_e)
                .insert(PlayerAnimation::Attack)
                .insert(crate::animations::player_sprite::AttackAnimationTimer(
                    Timer::from_seconds(1.0, TimerMode::Once),
                ));
        }
        attack_event.send(AttackEvent {
            direction,
            ignore_cooldown: false,
        });
        if player_pos
            .truncate()
            .distance(cursor_pos.world_coords.truncate())
            > game.player().reach_distance * 32.
        {
            return;
        }
        if let Some((hit_obj, _)) = game.get_obj_entity_at_tile(cursor_tile_pos, &proto_param) {
            if *DEBUG {
                debug!("OBJ: {hit_obj:?}");
            }
            let (damage, was_crit, was_overcrit) =
                game.calculate_player_damage(0, None, 0, None, 0, 0, true);
            hit_event.send(HitEvent {
                hit_by_pet: None,
                hit_entity: hit_obj,
                damage: damage as i32,
                dir: Vec2::new(0., 0.),
                hit_with_melee: main_hand_option,
                hit_with_projectile: None,
                ignore_tool: false,
                hit_by_mob: None,
                from_heirloom_effect: None,
                was_crit,
                was_overcrit,
                from_active_skill: false,
            });
        }
    }
}

pub fn handle_interact_objects(
    objs: Query<
        (
            Entity,
            &GlobalTransform,
            &ObjectAction,
            &WorldObject,
            &SpriteAnchor,
        ),
        With<InteractionGuideTrigger>,
    >,
    mut player_query: Query<(&GlobalTransform, &mut Inventory), With<Player>>,
    mut game: GameParam,
    mut proto_param: ProtoParam,
    mut item_action_param: ItemActionParam,
    mut commands: Commands,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    let gamepad_pressed = gamepad_action_q
        .get_single()
        .map(|a| a.just_pressed(GamepadAction::Interact))
        .unwrap_or(false);
    if !keybinds.check_interact_input(&key_input, &mouse_input) && !gamepad_pressed {
        return;
    }
    for (obj_e, t, obj_action, obj, anchor) in objs.iter() {
        let obj_t = t.translation().truncate() - anchor.0;
        let (player_t, mut inv) = player_query.single_mut();
        if obj_t.distance(player_t.translation().truncate()) <= 32. {
            obj_action.run_action(
                obj_e,
                world_pos_to_tile_pos(obj_t),
                *obj,
                &mut game,
                &mut item_action_param,
                &mut commands,
                &mut proto_param,
                &mut inv,
            );
        }
    }
}

pub fn handle_open_essence_ui(
    mut commands: Commands,
    key_input: Res<Input<KeyCode>>,
    mouse_input: Res<Input<MouseButton>>,
    keybinds: Res<InputMappings>,
    player_query: Query<&GlobalTransform, With<Player>>,
    nearby_merchant_query: Query<(&GlobalTransform, &EssenceShopChoices)>,
    mut next_inv_state: ResMut<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    open_lock: Option<Res<crate::ui::MerchantShopOpenLock>>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    let gamepad_pressed = gamepad_action_q
        .get_single()
        .map(|a| a.just_pressed(GamepadAction::Interact))
        .unwrap_or(false);
    if !keybinds.check_interact_input(&key_input, &mouse_input) && !gamepad_pressed {
        return;
    }
    // Re-pressing interact while the shop is open toggles `Essence` closed in
    // `handle_new_ui_state`; ignore repeat presses until the player closes via Done.
    if curr_ui_state.0 == UIState::Essence || open_lock.is_some() {
        return;
    }
    let player_t = player_query.single().translation().truncate();
    for (transform, choices) in nearby_merchant_query.iter() {
        if player_t.distance(transform.translation().truncate()) < 32. {
            commands.insert_resource(choices.clone());
            commands.insert_resource(crate::ui::MerchantShopOpenLock(Timer::from_seconds(
                0.45,
                TimerMode::Once,
            )));
            next_inv_state.set(UIState::Essence);
        }
    }
}

pub fn move_camera_with_player(
    player_query: Query<
        (&Transform, &RawPosition, &MovementVector),
        (
            With<Player>,
            Without<MainCamera>,
            Without<TextureCamera>,
            Without<UICamera>,
        ),
    >,
    mut game_camera: Query<(&mut Transform, &mut RawPosition), (With<TextureCamera>,)>,
    time: Res<Time>,
    resolution: Res<ScreenResolution>,
) {
    let (mut game_camera_transform, mut raw_camera_pos) = game_camera.single_mut();
    let Ok((_player_pos, raw_player_pos, _player_movement_vec)) = player_query.get_single() else {
        return;
    };

    let camera_lookahead_scale = 4.0;
    let delta = raw_player_pos.0 - raw_camera_pos.0;
    raw_camera_pos.0 += delta * camera_lookahead_scale * time.delta_seconds();

    let pixel_step = 1.0 / resolution.scale as f32;
    game_camera_transform.translation.x = (raw_camera_pos.x / pixel_step).round() * pixel_step;
    game_camera_transform.translation.y = (raw_camera_pos.y / pixel_step).round() * pixel_step;
}
