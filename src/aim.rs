//! Unified non-mouse aiming: gamepad right stick and "Mouseless Mode" keyboard arrow keys (see
//! `MouselessModeState`) drive the exact same [`AimState`]/reticle through one set of systems,
//! instead of each device having its own parallel state machine. This matters because a player
//! can have a gamepad connected while Mouseless Mode also happens to be on (e.g. left over from
//! testing) — with duplicated logic that meant two independent reticles fighting over the
//! cursor; unified, there's just one aim point regardless of which device is currently moving
//! it.
//!
//! Whichever device is currently providing input drives the aim point in two different ways,
//! depending on context:
//!
//! - **Facing / basic attacks / instant-fire skills**: the aim direction snaps instantly (like a
//!   digital twin-stick) — pointing the stick/arrow-keys left faces/attacks left immediately, no
//!   ramp-up. This overrides `CursorPos::world_coords` at a fixed distance from the player, which
//!   every gameplay system that aims off the cursor already picks up automatically. While **Attack
//!   Auto Target** is enabled, holding the aim stick/keys temporarily overrides auto-aim so the
//!   player can aim manually; releasing returns to nearest-enemy targeting.
//! - **Ground-targeted skills** (`ActiveSkill::is_ground_targeted`: Fire Pillar, Ice Wall, Druid
//!   Tree, Bomb): holding the skill's button shows a free-roam reticle centered on the player;
//!   while held, the stick/arrow-keys nudge it anywhere (speed set by the "Aim Sensitivity"
//!   options setting, and scaled by stick magnitude for analog input) instead of snapping a fixed
//!   distance away, and releasing fires at its position. This *replaces* the instant-snap
//!   override for as long as the skill is held — see `PendingGroundAimSkill` (`inputs.rs`) for
//!   the hold/release state machine.
//!
//! Gamepad input takes priority over keyboard whenever the right stick is outside the deadzone
//! (mirrors the left-stick-over-WASD precedence in `player_move_inputs`); keyboard only ever
//! drives aim while `MouselessModeState` is on. Real mouse+click play is unaffected either way —
//! `screen_coords`/`ui_coords` are left untouched, so UI hit-testing stays mouse-only, and ground
//! skills fired via a real mouse click stay instant (see `dispatch_active_skill_events`).
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use serde::{Deserialize, Serialize};

use crate::gamepad_input::{ActiveInputDevice, GamepadAction, InputDeviceKind, GAMEPAD_STICK_DEADZONE};
use crate::inputs::{MouselessModeState, PendingGroundAimSkill, SwapMovementAimKeysState};
use crate::player::Player;
use crate::{cursor::CursorPos, world::y_sort::YSort, Game, GameState};

/// Options screen "Aim Sensitivity" setting (1-10): how fast the free-aim reticle moves while
/// charging a ground-targeted skill (via gamepad stick or, in Mouseless Mode, arrow keys).
#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AimSensitivity(pub u8);

impl Default for AimSensitivity {
    fn default() -> Self {
        Self(5)
    }
}

impl AimSensitivity {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 10;

    const MIN_SPEED_PX_PER_SEC: f32 = 60.0;
    const MAX_SPEED_PX_PER_SEC: f32 = 480.0;

    /// Reticle movement speed (world units/second) for the current sensitivity level.
    pub fn speed_px_per_sec(&self) -> f32 {
        let level = self.0.clamp(Self::MIN, Self::MAX);
        let t = (level - Self::MIN) as f32 / (Self::MAX - Self::MIN) as f32;
        Self::MIN_SPEED_PX_PER_SEC + t * (Self::MAX_SPEED_PX_PER_SEC - Self::MIN_SPEED_PX_PER_SEC)
    }

    pub fn load() -> Self {
        let path = crate::datafiles::game_data();
        if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.keyboard_aim_sensitivity.unwrap_or_default();
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

        game_data.keyboard_aim_sensitivity = Some(*self);

        if let Ok(file) = std::fs::File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

/// World-space distance from the player used to place the facing/attack aim point when not
/// charging a ground-targeted skill. Shared by gamepad and Mouseless Mode keyboard input so
/// both feel identical.
pub const AIM_RETICLE_RANGE: f32 = 80.0;

/// - `facing_dir`: instant, sticky aim direction for facing/attacks/instant skills (holds the
///   last non-zero direction so it doesn't snap back to zero on release). Updated immediately
///   every frame a direction is provided — no ramp-up — so moving the stick/pressing a key snaps
///   facing right away.
/// - `ground_aim_offset`: free-roam world-space offset from the player, used *only* while
///   charging a ground-targeted skill's hold-to-aim reticle (see `PendingGroundAimSkill`).
///   Recentered to `Vec2::ZERO` (on top of the player) at the start of each charge.
#[derive(Resource, Default)]
pub struct AimState {
    pub facing_dir: Vec2,
    pub ground_aim_offset: Vec2,
}

/// True while the player is actively holding an aim direction this frame (right stick outside the
/// deadzone, or mouseless-mode aim keys). Used to temporarily override **Attack Auto Target** so
/// controller / keyboard aim can snap back to manual targeting until the input is released.
#[derive(Resource, Default)]
pub struct ManualAimOverride {
    pub active: bool,
}

/// Reads whichever device is currently providing aim input and updates `AimState` from it.
/// Gamepad right stick wins whenever it's outside the deadzone (same precedence movement uses
/// for left-stick-vs-WASD); otherwise falls back to keyboard arrow keys, but only while
/// Mouseless Mode is on.
fn update_aim_state(
    mouseless_mode: Res<MouselessModeState>,
    key_input: Res<Input<KeyCode>>,
    gamepad_action_q: Query<&ActionState<GamepadAction>, With<Player>>,
    active_device: Res<ActiveInputDevice>,
    pending: Res<PendingGroundAimSkill>,
    sensitivity: Res<AimSensitivity>,
    swap_keys: Res<SwapMovementAimKeysState>,
    time: Res<Time>,
    mut aim: ResMut<AimState>,
    mut manual_override: ResMut<ManualAimOverride>,
    mut last_pending_slot: Local<Option<usize>>,
) {
    // Recenter the reticle on the player at the start of each hold-to-aim charge, so every
    // ground-targeted cast starts predictably on the player rather than wherever a previous
    // charge happened to leave off.
    if pending.0 != *last_pending_slot {
        if pending.0.is_some() {
            aim.ground_aim_offset = Vec2::ZERO;
        }
        *last_pending_slot = pending.0;
    }

    let gamepad_dir = (active_device.0 == InputDeviceKind::Gamepad)
        .then(|| gamepad_action_q.get_single().ok())
        .flatten()
        .and_then(|action_state| action_state.clamped_axis_pair(GamepadAction::Aim))
        .map(|pair| pair.xy())
        .filter(|v| v.length_squared() > GAMEPAD_STICK_DEADZONE.powi(2));

    // `SwapMovementAimKeysState` (options: "Swap Move/Aim Keys") picks whether arrow keys or
    // WASD drive aim here — whichever one `player_move_inputs` isn't using for movement.
    let (dir, analog_magnitude) = if let Some(stick) = gamepad_dir {
        (stick.clamp_length_max(1.0), true)
    } else if mouseless_mode.0 {
        let (left, right, up, down) = if swap_keys.0 {
            (KeyCode::A, KeyCode::D, KeyCode::W, KeyCode::S)
        } else {
            (KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down)
        };
        let mut d = Vec2::ZERO;
        if key_input.pressed(left) {
            d.x -= 1.;
        }
        if key_input.pressed(right) {
            d.x += 1.;
        }
        if key_input.pressed(up) {
            d.y += 1.;
        }
        if key_input.pressed(down) {
            d.y -= 1.;
        }
        (d, false)
    } else {
        (Vec2::ZERO, false)
    };

    manual_override.active = dir != Vec2::ZERO;

    if dir != Vec2::ZERO {
        let normalized = dir.normalize();
        // Instant snap for facing/attacks...
        aim.facing_dir = normalized;
        // ...but only accumulate the free-roam ground-target reticle while actually charging
        // one, otherwise it'd silently drift off-player even when nothing is being aimed.
        // Analog stick input keeps its raw magnitude so a soft push moves the reticle slower
        // than a full push; keyboard is digital, so it's always normalized (full speed).
        if pending.0.is_some() {
            let step = if analog_magnitude { dir } else { normalized };
            aim.ground_aim_offset += step * sensitivity.speed_px_per_sec() * time.delta_seconds();
        }
    }
}

/// Overrides `CursorPos::world_coords` with the aim point while a non-mouse device is actively
/// driving it: the free-roam reticle while charging a ground-targeted skill, otherwise the
/// instant facing direction at a fixed range. Every gameplay system that aims off
/// `cursor.world_coords` (attacks, facing, active skills) picks this up automatically.
/// `screen_coords`/`ui_coords` are left untouched, so UI hit-testing stays mouse-only. Must run
/// after `update_cursor_pos`.
fn apply_aim_to_cursor_world_pos(
    mouseless_mode: Res<MouselessModeState>,
    device: Res<ActiveInputDevice>,
    pending: Res<PendingGroundAimSkill>,
    aim: Res<AimState>,
    game: Res<Game>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    if !mouseless_mode.0 && device.0 != InputDeviceKind::Gamepad {
        return;
    }
    let player_pos = game.player_state.position.truncate();
    let aim_point = if pending.0.is_some() {
        player_pos + aim.ground_aim_offset
    } else {
        if aim.facing_dir == Vec2::ZERO {
            return;
        }
        player_pos + aim.facing_dir * AIM_RETICLE_RANGE
    };
    cursor_pos.world_coords = aim_point.extend(cursor_pos.world_coords.z);
}

/// `YSort` bias for the aim reticle: comfortably above every other `YSort` bias used in the
/// codebase (max observed elsewhere is `11.`), so the reticle reliably renders in front of the
/// player/enemies/world sprites at its position instead of being buried by their Y-sorted
/// depth. Without this, a static/unsorted Z sits far behind `YSort`-driven world sprites near
/// the player — which use depths roughly in the 0-900 range — making the reticle invisible even
/// while correctly positioned and visible.
const AIM_RETICLE_Y_SORT_BIAS: f32 = 50.0;

/// Marker for the world-space reticle shown while charging a ground-targeted skill's
/// hold-to-aim (`PendingGroundAimSkill`) — regardless of whether gamepad or Mouseless Mode
/// keyboard input is driving it. Not shown otherwise (facing/attack aiming has no reticle; the
/// player sprite's facing direction is feedback enough for that).
#[derive(Component)]
struct AimReticle;

fn setup_aim_reticle(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<AimReticle>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        SpriteBundle {
            // Lowercase "icons" — must match the git-tracked path exactly (`git ls-files`),
            // not just what's on disk locally. On Windows/Linux, `bevy_embedded_assets` bakes
            // assets into a case-*sensitive* in-memory map at build time keyed by the checked
            // out path, so a mismatched case silently fails to load there even though it works
            // fine on macOS (which skips embedding and reads the case-insensitive filesystem
            // directly — see the `EmbeddedAssetPlugin` setup in `main.rs`).
            texture: asset_server.load("ui/icons/Crosshair.png"),
            sprite: Sprite {
                custom_size: Some(Vec2::new(20., 20.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 15.)),
            visibility: Visibility::Hidden,
            ..default()
        },
        YSort(AIM_RETICLE_Y_SORT_BIAS),
        AimReticle,
        Name::new("AimReticle"),
    ));
}

fn update_aim_reticle(
    pending: Res<PendingGroundAimSkill>,
    aim: Res<AimState>,
    game: Res<Game>,
    mut crosshair_query: Query<(&mut Transform, &mut Visibility), With<AimReticle>>,
) {
    let Ok((mut transform, mut visibility)) = crosshair_query.get_single_mut() else {
        return;
    };
    let show = pending.0.is_some();
    *visibility = if show {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    if !show {
        return;
    }
    let player_pos = game.player_state.position.truncate();
    let pos = player_pos + aim.ground_aim_offset;
    transform.translation.x = pos.x;
    transform.translation.y = pos.y;
}

pub struct AimPlugin;

impl Plugin for AimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AimState>()
            .init_resource::<ManualAimOverride>()
            .insert_resource(AimSensitivity::load())
            .add_system(setup_aim_reticle.in_schedule(OnEnter(GameState::Main)))
            .add_systems(
                (
                    update_aim_state,
                    update_aim_reticle.after(update_aim_state),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                apply_aim_to_cursor_world_pos
                    .in_base_set(CoreSet::PostUpdate)
                    .after(crate::cursor::update_cursor_pos)
                    .run_if(in_state(GameState::Main)),
            );
    }
}
