//! Gamepad support: [`GamepadAction`] covers gameplay (movement, aiming, skills), bound to the
//! `Player` entity. [`UiGamepadAction`] separately covers UI focus navigation (main menu,
//! inventory, options, etc. — see `src/ui/focus.rs`), bound to its own always-present entity
//! since it needs to work before a `Player` even exists (main menu). See the controller
//! support plan.
//!
//! Gameplay bindings are persisted in [`crate::gamepad_bindings::GamepadMappings`] and editable
//! from the options Controls tab when a controller is connected. UI navigation bindings
//! ([`UiGamepadAction`]) remain fixed.
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

use crate::gamepad_bindings::GamepadMappings;
use crate::{player::Player, GameState};

/// Stick magnitude below this is treated as neutral (no input).
pub const GAMEPAD_STICK_DEADZONE: f32 = 0.2;

/// Gameplay actions bound to a fixed Xbox-style layout.
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
    /// Left Bumper — opens/closes the inventory (mirrors the keyboard inventory keybind).
    ToggleInventory,
    /// Right Bumper — opens/closes the island map (mirrors the keyboard map keybind).
    ToggleMap,
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

/// True when `action` was just pressed this frame on the player's gamepad — generic
/// counterpart to [`gamepad_skill_just_pressed`]/[`gamepad_hotbar_just_pressed`] for one-off
/// actions like [`GamepadAction::ToggleInventory`]/[`GamepadAction::ToggleMap`].
pub fn gamepad_action_just_pressed(
    action_state: Option<&ActionState<GamepadAction>>,
    action: GamepadAction,
) -> bool {
    action_state
        .map(|a| a.just_pressed(action))
        .unwrap_or(false)
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

/// Attaches saved gamepad bindings to the player as soon as it spawns (start of every run).
fn insert_gamepad_input_on_player(
    mut commands: Commands,
    added_players: Query<Entity, Added<Player>>,
    mappings: Res<GamepadMappings>,
) {
    let input_map = mappings.to_input_map();
    for player_e in added_players.iter() {
        commands
            .entity(player_e)
            .insert(InputManagerBundle::<GamepadAction> {
                action_state: ActionState::default(),
                input_map: input_map.clone(),
            });
        info!("[Gamepad] Input bindings attached to player entity {player_e:?}");
    }
}

/// Rebuilds the player's gameplay `InputMap` after options rebinding.
pub fn sync_player_gamepad_input_map(
    mappings: Res<GamepadMappings>,
    mut players: Query<&mut InputMap<GamepadAction>, With<Player>>,
) {
    if !mappings.is_changed() {
        return;
    }
    let input_map = mappings.to_input_map();
    for mut map in players.iter_mut() {
        *map = input_map.clone();
    }
}

/// UI-navigation gamepad actions (`src/ui/focus.rs`) — deliberately **separate** from
/// [`GamepadAction`] because that one only lives on the `Player` entity, which doesn't exist
/// yet on the main menu / title screen. These live on a standalone marker entity spawned once
/// at startup instead, so focus navigation works everywhere a `Focusable` might appear
/// (main menu, pause-style overlays during a run, etc.), not just mid-run.
///
/// Some of these intentionally share a physical button with a [`GamepadAction`] (e.g. South is
/// both `Confirm` here and `Skill0` there, the D-pad is both `Nav*` here and `Hotbar*` there) —
/// same pattern as `RightTrigger2` already doing double duty for `Attack`/`Skill1`. There's no
/// real conflict since focus navigation only ever runs while gameplay itself is *not* consuming
/// D-pad/South for hotbar/skills (see `focus_nav_should_run` in `src/ui/focus.rs`).
#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub enum UiGamepadAction {
    /// A button — activates whatever's currently focused (mirrors a mouse click).
    Confirm,
    /// B button — back/close (mirrors the keyboard Escape handling in `close_container`).
    /// While carrying an item in the inventory (Track 4), B first drops the carried item back
    /// onto its origin slot instead of closing the menu.
    Cancel,
    /// Y button — context "quick action" on the focused inventory slot: quick-equip / transfer
    /// the item the same way a shift-click does with the mouse.
    QuickAction,
    /// X button — mark / unmark the focused merchant shop item to track its price. While the
    /// inventory is open, the same button consumes the focused consumable (mirrors right-click).
    Mark,
    /// Left stick, read as a `Vec2` for continuous analog nav (see `focus_nav_should_run`'s
    /// caller for the discrete-step logic built on top of it).
    NavStick,
    NavUp,
    NavDown,
    NavLeft,
    NavRight,
    /// Start button — toggles the gamepad pause overlay (`UIState::Pause`) while playing with
    /// no other menu open. See `toggle_gamepad_pause` in `src/inputs.rs`.
    Pause,
}

/// Marker for the standalone entity carrying [`UiGamepadAction`]'s `ActionState` — see that
/// type's doc comment for why this isn't just attached to the `Player` entity.
#[derive(Component)]
pub struct UiGamepadInputMarker;

fn default_ui_gamepad_input_map() -> InputMap<UiGamepadAction> {
    let mut map = InputMap::default();
    map.insert(DualAxis::left_stick(), UiGamepadAction::NavStick);
    map.insert(GamepadButtonType::South, UiGamepadAction::Confirm);
    map.insert(GamepadButtonType::East, UiGamepadAction::Cancel);
    map.insert(GamepadButtonType::North, UiGamepadAction::QuickAction);
    map.insert(GamepadButtonType::West, UiGamepadAction::Mark);
    map.insert(GamepadButtonType::DPadUp, UiGamepadAction::NavUp);
    map.insert(GamepadButtonType::DPadDown, UiGamepadAction::NavDown);
    map.insert(GamepadButtonType::DPadLeft, UiGamepadAction::NavLeft);
    map.insert(GamepadButtonType::DPadRight, UiGamepadAction::NavRight);
    map.insert(GamepadButtonType::Start, UiGamepadAction::Pause);
    map
}

/// Spawns the standalone `UiGamepadAction` entity once at startup (not gated on `GameState` or
/// the `Player` existing — see [`UiGamepadAction`]'s doc comment).
fn setup_ui_gamepad_input(mut commands: Commands) {
    commands.spawn((
        InputManagerBundle::<UiGamepadAction> {
            action_state: ActionState::default(),
            input_map: default_ui_gamepad_input_map(),
        },
        UiGamepadInputMarker,
        Name::new("UiGamepadInput"),
    ));
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

/// Decides which device is "active" each frame. Keyboard/mouse takes priority whenever it has
/// *any* signal this frame — not just a fresh press, but also keys/buttons still held down —
/// so a stray/phantom gamepad connection (e.g. some OS misreporting a non-game peripheral as a
/// controller, or a real pad drifting slightly above its rest position) can never silently steal
/// input priority away from a player who is actively holding a movement key. Only when keyboard
/// and mouse are both fully idle this frame do we look at the gamepad, and even then a stick
/// axis only counts if it's clearly outside the deadzone (a fresh button press always counts).
/// This mirrors the exact rule requested for Track 4: any controller button swaps to controller,
/// any keyboard input swaps back to keyboard instantly.
/// Real mouse movement per frame is easily a few pixels even for a deliberate nudge; sensor/OS
/// jitter on a perfectly still mouse is sub-pixel. This filters out that jitter so it can never
/// masquerade as "the player is using the mouse" and drown out a genuine gamepad button press
/// that happens to land on the same frame.
const MOUSE_MOTION_JITTER_THRESHOLD: f32 = 1.0;

/// Some setups (certain Bluetooth peripherals, USB dongles, virtual/ghost HID devices, driver
/// quirks) make gilrs report a "gamepad" that's sending noise even though the player has no real
/// controller — we've seen logs where `Input<GamepadButton>`/the stick axes flip on and off every
/// single frame with zero real controller plugged in. A single frame of gamepad evidence is
/// therefore not trustworthy on its own. Switching *to* Gamepad requires this many consecutive
/// frames of evidence (with no competing fresh keyboard/mouse input in between) before we commit
/// to it — a real button hold or stick tilt easily clears this in under a tenth of a second,
/// while a one-frame glitch never does. Switching back to KeyboardMouse stays instant (any real
/// key/click/mouse-move immediately wins) since that direction can't cause a lockout.
const GAMEPAD_SWITCH_DEBOUNCE_FRAMES: u8 = 5;

/// After keyboard/mouse wins, ignore stick-only gamepad evidence for this many frames so a
/// noisy/phantom stick cannot flip `ActiveInputDevice` back to Gamepad the moment the mouse
/// stops moving (which was leaving the essence mark prompt stuck on "Press X").
const KEYBOARD_MOUSE_HOLD_FRAMES: u8 = 45;

fn update_active_input_device(
    mut device: ResMut<ActiveInputDevice>,
    key_input: Res<Input<KeyCode>>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut mouse_motion: EventReader<bevy::input::mouse::MouseMotion>,
    gamepad_buttons: Res<Input<GamepadButton>>,
    action_query: Query<&ActionState<GamepadAction>, With<Player>>,
    mut gamepad_evidence_streak: Local<u8>,
    mut keyboard_mouse_hold: Local<u8>,
) {
    let real_mouse_motion = mouse_motion
        .iter()
        .any(|ev| ev.delta.length() > MOUSE_MOTION_JITTER_THRESHOLD);
    let fresh_keyboard_mouse = real_mouse_motion
        || key_input.get_just_pressed().next().is_some()
        || mouse_button_input.get_just_pressed().next().is_some();

    // Fresh, deliberate keyboard/mouse input always wins outright and instantly, resetting the
    // gamepad debounce streak so a lingering noisy signal can't "carry over" its progress.
    if fresh_keyboard_mouse {
        *gamepad_evidence_streak = 0;
        *keyboard_mouse_hold = KEYBOARD_MOUSE_HOLD_FRAMES;
        device.0 = InputDeviceKind::KeyboardMouse;
        return;
    }

    let fresh_gamepad_button = gamepad_buttons.get_just_pressed().next().is_some();
    let stick_active = action_query.get_single().is_ok_and(|action_state| {
        [GamepadAction::Move, GamepadAction::Aim]
            .into_iter()
            .any(|action| {
                action_state
                    .clamped_axis_pair(action)
                    .map(|pair| pair.xy().length() > GAMEPAD_STICK_DEADZONE)
                    .unwrap_or(false)
            })
    });

    // During the post-mouse hold, stick drift alone cannot reclaim Gamepad — a real button
    // press still can, so deliberate controller use switches immediately.
    if *keyboard_mouse_hold > 0 {
        *keyboard_mouse_hold = keyboard_mouse_hold.saturating_sub(1);
        if !fresh_gamepad_button {
            device.0 = InputDeviceKind::KeyboardMouse;
            *gamepad_evidence_streak = 0;
            return;
        }
    }

    if fresh_gamepad_button || stick_active {
        *gamepad_evidence_streak = gamepad_evidence_streak.saturating_add(1);
        if *gamepad_evidence_streak >= GAMEPAD_SWITCH_DEBOUNCE_FRAMES {
            device.0 = InputDeviceKind::Gamepad;
        }
        return;
    }

    // No gamepad evidence this frame — the streak must be unbroken to count, so reset it. Held
    // (not fresh) keyboard/mouse keeps its device without needing to be re-pressed every frame.
    *gamepad_evidence_streak = 0;
    let held_keyboard_mouse = key_input.get_pressed().next().is_some()
        || mouse_button_input.get_pressed().next().is_some();
    if held_keyboard_mouse {
        device.0 = InputDeviceKind::KeyboardMouse;
    }
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
            .add_plugin(InputManagerPlugin::<UiGamepadAction>::default())
            .init_resource::<ActiveInputDevice>()
            .add_startup_system(setup_ui_gamepad_input)
            // Connection logging runs unconditionally (not gated to GameState::Main) since a
            // controller can be plugged in from the main menu, before a Player even exists.
            .add_system(log_gamepad_connections)
            .add_system(insert_gamepad_input_on_player)
            .add_system(
                sync_player_gamepad_input_map.in_set(OnUpdate(GameState::Main)),
            )
            // Also unconditional (not gated to GameState::Main): the active device needs to stay
            // accurate in menus/pause too, so a stray/phantom gamepad connection detected while
            // paused doesn't leave stale state once gameplay resumes.
            .add_systems((
                update_active_input_device,
                log_active_input_device_changes.after(update_active_input_device),
            ));

        if *crate::DEBUG {
            app.add_system(debug_log_raw_gamepad_state)
                .add_system(debug_log_raw_gamepad_events)
                .add_system(debug_log_gamepad_action_state);
        }
    }
}
