//! Gamepad support for gameplay only (movement, aiming, skills). UI screens (menus,
//! inventory, hovers, options, etc.) are intentionally NOT wired to the gamepad yet — the
//! mouse remains the only way to interact with them. See the controller support plan.
//!
//! Bindings are a fixed Xbox-style layout (no rebind UI in v1) built on
//! `leafwing-input-manager`, which is bound alongside the existing keyboard/mouse
//! `InputMappings` rather than replacing it.
//!
//! ## Known macOS limitation (shelved as of Bevy 0.10.1)
//!
//! Confirmed working end-to-end (movement, twin-stick aim, ground-targeted skill hold-to-aim,
//! basic skills) on Windows. On macOS, though, Bevy 0.10.1's gamepad backend (`bevy_gilrs` ->
//! `gilrs` 0.10.x, raw `IOHIDManager`-based) could not reliably see either controller tested:
//! - Xbox Wireless Controller: not detected at all (not at startup, not on hot-plug),
//!   despite macOS/Chrome/hardwaretester.com seeing it fine. Bumping to `gilrs` 0.10.10
//!   (a version cited to help Xbox detection) made no difference.
//! - Nintendo Switch Pro Controller: detected, but HID reports come through garbled
//!   (phantom stuck axes, all-buttons-at-once spam) — `gilrs`'s macOS parser doesn't
//!   handle its non-standard report format.
//!
//! This is a known, still-actively-patched gap in `gilrs`'s macOS/IOKit backend (see e.g.
//! gilrs commit `dc621ea9`, "fix macos iokit mappings", dated well after the `0.10.x` line
//! we're pinned to), and similar reports exist for other engines (Unity, older SDL2) on
//! macOS. The real fix is Apple's native `GameController.framework`, which Bevy only picks
//! up in much newer versions (0.15+) via `bevy_gilrs`'s replacement — out of reach without
//! a major Bevy upgrade.
//!
//! Decision: shelved for now rather than sunk-cost debugging `gilrs` internals. Revisit
//! gamepad hardware testing on macOS after the planned Bevy 0.19 upgrade; this module
//! should still be a solid starting point for wiring gameplay to that newer backend.
use bevy::input::gamepad::{GamepadConnection, GamepadConnectionEvent, GamepadEvent};
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

use crate::{cursor::CursorPos, player::Player, world::y_sort::YSort, Game, GameState};

/// Stick magnitude below this is treated as neutral (no input).
pub const GAMEPAD_STICK_DEADZONE: f32 = 0.2;
/// World-space distance from the player used to place the aim reticle / ground-target
/// skills when driven by the right stick. Direction-only aiming (facing, weapon swings,
/// projectile direction) ignores this and only uses the stick's normalized direction.
pub const GAMEPAD_AIM_RETICLE_RANGE: f32 = 80.0;

/// Gameplay actions bound to a fixed Xbox-style layout.
///
/// Right Bumper / Left Bumper are deliberately left unbound for now — they're reserved for
/// Track 4 (inventory section navigation: bag/equipment/sidebar/craft cycling), matching the
/// original controller support plan.
#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub enum GamepadAction {
    Move,
    Aim,
    /// Basic weapon attack (mirrors the hardcoded Left Mouse Button check).
    Attack,
    /// Only 3 skill slots are ever reachable in normal play (`VISIBLE_CLASS_SKILL_COUNT`
    /// in `player/skills.rs`) — slot 3 is permanently disabled dead code and slot 4 is a
    /// rare blessing-only bonus, so neither gets a gamepad binding.
    Skill0,
    Skill1,
    Skill2,
    Interact,
    AutoTarget,
    Hotbar0,
    Hotbar1,
    Hotbar2,
    Hotbar3,
}

impl GamepadAction {
    /// Maps an active-skill slot index to its gamepad action. Only slots 0-2 are ever
    /// reachable in normal play (see the `GamepadAction` doc comment), so slots 3/4 always
    /// return `None` here — those blessing/dev-only slots stay keyboard/mouse-only.
    pub fn skill_slot(slot: usize) -> Option<Self> {
        match slot {
            0 => Some(Self::Skill0),
            1 => Some(Self::Skill1),
            2 => Some(Self::Skill2),
            _ => None,
        }
    }

    /// Maps a hotbar consume slot index (0..=3) to its gamepad action.
    pub fn hotbar_slot(slot: usize) -> Option<Self> {
        match slot {
            0 => Some(Self::Hotbar0),
            1 => Some(Self::Hotbar1),
            2 => Some(Self::Hotbar2),
            3 => Some(Self::Hotbar3),
            _ => None,
        }
    }
}

/// True when `slot`'s gamepad button was just pressed this frame (gamepad-side
/// counterpart to `InputMappings::check_skill_input`).
pub fn gamepad_skill_just_pressed(
    action_state: Option<&ActionState<GamepadAction>>,
    slot: usize,
) -> bool {
    let (Some(action_state), Some(action)) = (action_state, GamepadAction::skill_slot(slot))
    else {
        return false;
    };
    action_state.just_pressed(action)
}

/// True when `slot`'s gamepad button is currently held down (not just this frame) —
/// gamepad-side counterpart to `InputMappings::check_skill_input_held`. Used to detect
/// release for the ground-targeted skill hold-to-aim flow (see `dispatch_active_skill_events`
/// in `inputs.rs`).
pub fn gamepad_skill_pressed(action_state: Option<&ActionState<GamepadAction>>, slot: usize) -> bool {
    let (Some(action_state), Some(action)) = (action_state, GamepadAction::skill_slot(slot))
    else {
        return false;
    };
    action_state.pressed(action)
}

/// True when `slot`'s gamepad hotbar button was just pressed this frame.
pub fn gamepad_hotbar_just_pressed(
    action_state: Option<&ActionState<GamepadAction>>,
    slot: usize,
) -> bool {
    let (Some(action_state), Some(action)) = (action_state, GamepadAction::hotbar_slot(slot))
    else {
        return false;
    };
    action_state.just_pressed(action)
}

fn default_gamepad_input_map() -> InputMap<GamepadAction> {
    let mut map = InputMap::default();
    map.insert(DualAxis::left_stick(), GamepadAction::Move);
    map.insert(DualAxis::right_stick(), GamepadAction::Aim);
    // Right Trigger doubles as the basic weapon attack and skill slot 1, mirroring the
    // keyboard/mouse default (slot 1 -> Left Mouse Button, same button as basic attack).
    map.insert(GamepadButtonType::RightTrigger2, GamepadAction::Attack);
    map.insert(GamepadButtonType::RightTrigger2, GamepadAction::Skill1);
    map.insert(GamepadButtonType::South, GamepadAction::Skill0);
    map.insert(GamepadButtonType::LeftTrigger2, GamepadAction::Skill2);
    // RightTrigger (RB) / LeftTrigger (LB) intentionally unbound — reserved for Track 4
    // inventory section navigation.
    map.insert(GamepadButtonType::North, GamepadAction::Interact);
    map.insert(GamepadButtonType::West, GamepadAction::AutoTarget);
    map.insert(GamepadButtonType::DPadUp, GamepadAction::Hotbar0);
    map.insert(GamepadButtonType::DPadRight, GamepadAction::Hotbar1);
    map.insert(GamepadButtonType::DPadDown, GamepadAction::Hotbar2);
    map.insert(GamepadButtonType::DPadLeft, GamepadAction::Hotbar3);
    map
}

/// Attaches the fixed gamepad bindings to the player as soon as it spawns (start of every run).
fn insert_gamepad_input_on_player(
    mut commands: Commands,
    added_players: Query<Entity, Added<Player>>,
) {
    for player_e in added_players.iter() {
        commands
            .entity(player_e)
            .insert(InputManagerBundle::<GamepadAction> {
                action_state: ActionState::default(),
                input_map: default_gamepad_input_map(),
            });
        info!("[Gamepad] Input bindings attached to player entity {player_e:?}");
    }
}

/// Logs gamepad connect/disconnect so it's easy to tell, from the log file alone, whether the
/// OS/gilrs backend ever saw the controller at all (as opposed to it being seen but our
/// bindings/deadzones not registering it as active — see `update_active_input_device`).
/// Bevy's own `gamepad_connection_system` also logs a bare `Connected`/`Disconnected` line;
/// this one is easier to grep for and includes the reported device name.
fn log_gamepad_connections(mut events: EventReader<GamepadConnectionEvent>) {
    for event in events.iter() {
        match &event.connection {
            GamepadConnection::Connected(info) => {
                info!(
                    "[Gamepad] Connected: {:?} ({})",
                    event.gamepad, info.name
                );
            }
            GamepadConnection::Disconnected => {
                info!("[Gamepad] Disconnected: {:?}", event.gamepad);
            }
        }
    }
}

/// One-shot-per-change log of which device is currently driving gameplay input, so it's easy
/// to confirm (from the log file) that stick/button presses are actually being recognized as
/// gamepad input, as opposed to the controller being connected but never crossing the deadzone
/// / not mapped the way we expect.
fn log_active_input_device_changes(
    device: Res<ActiveInputDevice>,
    mut last: Local<Option<InputDeviceKind>>,
) {
    // Seed silently on the very first tick (right as GameState::Main starts) instead of
    // logging a bogus "switched to KeyboardMouse" — that's just the default value, not an
    // actual detected transition.
    let Some(prev) = *last else {
        *last = Some(device.0);
        return;
    };
    if prev == device.0 {
        return;
    }
    *last = Some(device.0);
    info!("[Gamepad] Active input device switched to {:?}", device.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputDeviceKind {
    #[default]
    KeyboardMouse,
    Gamepad,
}

/// Which device most recently produced input. Only used to decide whether the gamepad aim
/// reticle is shown and whether it drives the world aim point — UI hit-testing is
/// unaffected regardless of this value (it always follows the real mouse).
#[derive(Resource, Default)]
pub struct ActiveInputDevice(pub InputDeviceKind);

fn update_active_input_device(
    mut device: ResMut<ActiveInputDevice>,
    key_input: Res<Input<KeyCode>>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    gamepad_buttons: Res<Input<GamepadButton>>,
    action_query: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    if mouse_motion.iter().next().is_some()
        || key_input.get_just_pressed().next().is_some()
        || mouse_button_input.get_just_pressed().next().is_some()
    {
        device.0 = InputDeviceKind::KeyboardMouse;
        return;
    }

    if gamepad_buttons.get_just_pressed().next().is_some() {
        device.0 = InputDeviceKind::Gamepad;
        return;
    }

    let Ok(action_state) = action_query.get_single() else {
        return;
    };
    let stick_active = [GamepadAction::Move, GamepadAction::Aim]
        .into_iter()
        .any(|action| {
            action_state
                .clamped_axis_pair(action)
                .map(|pair| pair.xy().length() > GAMEPAD_STICK_DEADZONE)
                .unwrap_or(false)
        });
    if stick_active {
        device.0 = InputDeviceKind::Gamepad;
    }
}

/// Last non-zero right-stick direction, so aim/facing holds steady when the stick is
/// released instead of snapping back to zero.
#[derive(Resource, Default)]
pub struct GamepadAimState {
    pub aim_dir: Vec2,
}

fn update_gamepad_aim_state(
    mut aim: ResMut<GamepadAimState>,
    action_query: Query<&ActionState<GamepadAction>, With<Player>>,
) {
    let Ok(action_state) = action_query.get_single() else {
        return;
    };
    let Some(pair) = action_state.clamped_axis_pair(GamepadAction::Aim) else {
        return;
    };
    let v = pair.xy();
    if v.length_squared() > GAMEPAD_STICK_DEADZONE * GAMEPAD_STICK_DEADZONE {
        aim.aim_dir = v.normalize();
    }
}

/// Overrides `CursorPos::world_coords` with the gamepad aim reticle position while the
/// gamepad is the active device. Every gameplay system that aims off of
/// `cursor.world_coords` (attacks, facing, active skills) picks this up automatically.
/// `screen_coords`/`ui_coords` are left untouched, so all UI hit-testing (`pointcast_2d`)
/// stays mouse-only. Must run after `update_cursor_pos`.
fn apply_gamepad_aim_to_cursor_world_pos(
    device: Res<ActiveInputDevice>,
    aim: Res<GamepadAimState>,
    game: Res<Game>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    if device.0 != InputDeviceKind::Gamepad || aim.aim_dir == Vec2::ZERO {
        return;
    }
    let player_pos = game.player_state.position.truncate();
    let aim_point = player_pos + aim.aim_dir * GAMEPAD_AIM_RETICLE_RANGE;
    cursor_pos.world_coords = aim_point.extend(cursor_pos.world_coords.z);
}

/// `YSort` bias for the aim reticle: comfortably above every other `YSort` bias used in the
/// codebase (max observed elsewhere is `11.`), so the reticle reliably renders in front of the
/// player/enemies/world sprites at its position instead of being buried by their Y-sorted
/// depth. Without this, a static/unsorted Z (e.g. `15.`) sits far behind `YSort`-driven world
/// sprites near the player — which use depths roughly in the 0-900 range — making the
/// reticle invisible even while correctly positioned and visible.
const CROSSHAIR_Y_SORT_BIAS: f32 = 50.0;

/// Marker for the world-space aim reticle shown while aiming with the gamepad right stick.
#[derive(Component)]
struct GamepadCrosshair;

fn setup_gamepad_crosshair(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<GamepadCrosshair>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("ui/Icons/Crosshair.png"),
            sprite: Sprite {
                custom_size: Some(Vec2::new(20., 20.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 15.)),
            visibility: Visibility::Hidden,
            ..default()
        },
        YSort(CROSSHAIR_Y_SORT_BIAS),
        GamepadCrosshair,
        Name::new("GamepadCrosshair"),
    ));
}

fn update_gamepad_crosshair(
    device: Res<ActiveInputDevice>,
    aim: Res<GamepadAimState>,
    game: Res<Game>,
    mut crosshair_query: Query<(&mut Transform, &mut Visibility), With<GamepadCrosshair>>,
) {
    let Ok((mut transform, mut visibility)) = crosshair_query.get_single_mut() else {
        return;
    };
    let show = device.0 == InputDeviceKind::Gamepad && aim.aim_dir != Vec2::ZERO;
    *visibility = if show {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !show {
        return;
    }
    let player_pos = game.player_state.position.truncate();
    let pos = player_pos + aim.aim_dir * GAMEPAD_AIM_RETICLE_RANGE;
    transform.translation.x = pos.x;
    transform.translation.y = pos.y;
}

/// `DEBUG=1` only: periodically logs which gamepads Bevy currently sees connected, plus every
/// raw button press and any stick/trigger axis reading above a small noise threshold,
/// regardless of whether it's mapped to a `GamepadAction`. Useful for telling apart "OS/gilrs
/// doesn't see the controller at all", "it's seen but buttons don't fire" (common for
/// non-XInput controllers whose HID report format gilrs' macOS backend doesn't fully parse,
/// e.g. some Nintendo Switch Pro Controller / gilrs version combos), vs "it's seen and firing,
/// but mapped to the wrong layout".
fn debug_log_raw_gamepad_state(
    gamepads: Res<Gamepads>,
    gamepad_buttons: Res<Input<GamepadButton>>,
    gamepad_axes: Res<Axis<GamepadAxis>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(3.0, TimerMode::Repeating));
    timer.tick(time.delta());
    if timer.just_finished() {
        let connected: Vec<Gamepad> = gamepads.iter().collect();
        info!("[Gamepad] Currently connected: {connected:?}");
    }
    for button in gamepad_buttons.get_just_pressed() {
        info!("[Gamepad] Raw button just pressed: {button:?}");
    }
    // Raw axis noise threshold, deliberately smaller than GAMEPAD_STICK_DEADZONE so we can
    // see axis events arriving at all even if the stick mapping/deadzone logic is wrong.
    const RAW_AXIS_THRESHOLD: f32 = 0.05;
    for gamepad in gamepads.iter() {
        for axis_type in [
            GamepadAxisType::LeftStickX,
            GamepadAxisType::LeftStickY,
            GamepadAxisType::RightStickX,
            GamepadAxisType::RightStickY,
            GamepadAxisType::LeftZ,
            GamepadAxisType::RightZ,
        ] {
            let axis = GamepadAxis { gamepad, axis_type };
            if let Some(value) = gamepad_axes.get(axis) {
                if value.abs() > RAW_AXIS_THRESHOLD {
                    info!("[Gamepad] Raw axis {axis_type:?} = {value:.2}");
                }
            }
        }
    }
}

/// `DEBUG=1` only: logs every raw `GamepadEvent` completely unfiltered — no deadzone, no
/// press-threshold, no mapping. This is the earliest tap point Bevy exposes: `bevy_gilrs`
/// pushes these directly from whatever `gilrs` itself reports, before any
/// `ButtonSettings`/`AxisSettings` filtering is applied to build `Input<GamepadButton>` /
/// `Axis<GamepadAxis>`. If a button mash produces zero lines here, gilrs itself is not
/// receiving/decoding HID reports from the device (a driver/OS-level gap), not a filtering or
/// mapping issue in this game's code.
///
/// Axis events are deduped/throttled (gilrs re-sends the same reading many times a second even
/// when nothing moves) so real signal — especially button presses — doesn't get buried in spam.
fn debug_log_raw_gamepad_events(
    mut events: EventReader<GamepadEvent>,
    mut last_axis: Local<std::collections::HashMap<(Gamepad, GamepadAxisType), f32>>,
) {
    const AXIS_LOG_CHANGE_THRESHOLD: f32 = 0.05;
    for event in events.iter() {
        match event {
            GamepadEvent::Connection(e) => {
                info!("[Gamepad][raw] Connection: {e:?}");
            }
            GamepadEvent::Button(e) => {
                info!(
                    "[Gamepad][raw] Button {:?} on {:?} = {:.3}",
                    e.button_type, e.gamepad, e.value
                );
            }
            GamepadEvent::Axis(e) => {
                let key = (e.gamepad, e.axis_type);
                let changed = match last_axis.get(&key) {
                    Some(prev) => (prev - e.value).abs() > AXIS_LOG_CHANGE_THRESHOLD,
                    None => true,
                };
                if changed {
                    last_axis.insert(key, e.value);
                    info!(
                        "[Gamepad][raw] Axis {:?} on {:?} = {:.3}",
                        e.axis_type, e.gamepad, e.value
                    );
                }
            }
        }
    }
}

/// `DEBUG=1` only: logs the mapped `ActionState<GamepadAction>` axis pairs for `Move`/`Aim` on
/// the player, throttled to changes only. Lets us tell apart "leafwing never maps the raw stick
/// into `Move`/`Aim` at all" (binding/config bug) from "it maps fine but something downstream
/// in `player_move_inputs` ignores it" (consumer bug).
fn debug_log_gamepad_action_state(
    action_query: Query<&ActionState<GamepadAction>, With<Player>>,
    mut last: Local<(Vec2, Vec2)>,
) {
    let Ok(action_state) = action_query.get_single() else {
        return;
    };
    let move_v = action_state
        .clamped_axis_pair(GamepadAction::Move)
        .map(|p| p.xy())
        .unwrap_or(Vec2::ZERO);
    let aim_v = action_state
        .clamped_axis_pair(GamepadAction::Aim)
        .map(|p| p.xy())
        .unwrap_or(Vec2::ZERO);
    if move_v.distance(last.0) > 0.05 || aim_v.distance(last.1) > 0.05 {
        *last = (move_v, aim_v);
        info!("[Gamepad][action_state] Move={move_v:?} Aim={aim_v:?}");
    }
}

pub struct GamepadInputPlugin;

impl Plugin for GamepadInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(InputManagerPlugin::<GamepadAction>::default())
            .init_resource::<ActiveInputDevice>()
            .init_resource::<GamepadAimState>()
            // Connection logging runs unconditionally (not gated to GameState::Main) since a
            // controller can be plugged in from the main menu, before a Player even exists.
            .add_system(log_gamepad_connections)
            .add_system(insert_gamepad_input_on_player)
            .add_system(setup_gamepad_crosshair.in_schedule(OnEnter(GameState::Main)))
            .add_systems(
                (
                    update_active_input_device,
                    log_active_input_device_changes.after(update_active_input_device),
                    update_gamepad_aim_state.after(update_active_input_device),
                    update_gamepad_crosshair.after(update_gamepad_aim_state),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                apply_gamepad_aim_to_cursor_world_pos
                    .in_base_set(CoreSet::PostUpdate)
                    .after(crate::cursor::update_cursor_pos)
                    .run_if(in_state(GameState::Main)),
            );

        if *crate::DEBUG {
            app.add_system(debug_log_raw_gamepad_state)
                .add_system(debug_log_raw_gamepad_events)
                .add_system(debug_log_gamepad_action_state);
        }
    }
}
