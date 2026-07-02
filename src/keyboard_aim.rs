//! Keyboard-only aiming for "Mouseless Mode" (options screen toggle, `MouselessModeState`).
//!
//! When enabled, one key group moves the player and the other aims — by default WASD moves
//! and arrow keys aim (see `player_move_inputs` in `inputs.rs`), but the "Swap Move/Aim Keys"
//! options toggle (`SwapMovementAimKeysState`) flips this to arrows-move/WASD-aims for
//! left-handed play or personal preference. Whichever group is currently doing the aiming acts
//! as the aim input, in two different ways depending on context:
//!
//! - **Facing / basic attacks / instant-fire skills**: arrow keys snap the aim direction
//!   instantly (like a digital twin-stick) — pressing Left faces/attacks left immediately,
//!   no ramp-up. This overrides `CursorPos::world_coords` at a fixed distance from the player
//!   (mirrors `gamepad_input::apply_gamepad_aim_to_cursor_world_pos`), which every gameplay
//!   system that aims off the cursor already picks up automatically.
//! - **Ground-targeted skills** (`ActiveSkill::is_ground_targeted`: Fire Pillar, Ice Wall,
//!   Druid Tree, Bomb): holding the skill's button shows a free-roam reticle centered on the
//!   player; while held, arrow keys nudge it anywhere (speed set by the "Aim Sensitivity"
//!   options setting) instead of snapping a fixed distance away, and releasing fires at its
//!   position. This *replaces* the instant-snap override for as long as the skill is held —
//!   see `PendingGroundAimSkill` (`inputs.rs`) for the hold/release state machine.
//!
//! This exists primarily so twin-stick aim/skill logic can be exercised and validated purely
//! on keyboard, sidestepping the current macOS gamepad hardware-support gap (see
//! `gamepad_input.rs`) while sharing the same underlying design.
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::inputs::{MouselessModeState, PendingGroundAimSkill, SwapMovementAimKeysState};
use crate::{cursor::CursorPos, world::y_sort::YSort, Game, GameState};

/// Options screen "Aim Sensitivity" setting (1-10): how fast the free-aim reticle moves while
/// an arrow key is held in Mouseless Mode.
#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct KeyboardAimSensitivity(pub u8);

impl Default for KeyboardAimSensitivity {
    fn default() -> Self {
        Self(5)
    }
}

impl KeyboardAimSensitivity {
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

/// World-space distance from the player used to place the facing/attack aim point when driven
/// by arrow keys outside of a ground-target hold. Matches
/// `gamepad_input::GAMEPAD_AIM_RETICLE_RANGE` so both input methods feel the same.
pub const KEYBOARD_AIM_RETICLE_RANGE: f32 = 80.0;

/// - `facing_dir`: instant, sticky aim direction for facing/attacks/instant skills (holds the
///   last non-zero arrow-key direction so it doesn't snap back to zero on release, same as
///   `gamepad_input::GamepadAimState`). Updated immediately every frame a direction is held —
///   no ramp-up — so pressing a key snaps facing right away.
/// - `ground_aim_offset`: free-roam world-space offset from the player, used *only* while
///   charging a ground-targeted skill's hold-to-aim reticle (see `PendingGroundAimSkill`).
///   Recentered to `Vec2::ZERO` (on top of the player) at the start of each charge.
#[derive(Resource, Default)]
pub struct KeyboardAimState {
    pub facing_dir: Vec2,
    pub ground_aim_offset: Vec2,
}

fn update_keyboard_aim_state(
    mouseless_mode: Res<MouselessModeState>,
    key_input: Res<Input<KeyCode>>,
    pending: Res<PendingGroundAimSkill>,
    sensitivity: Res<KeyboardAimSensitivity>,
    swap_keys: Res<SwapMovementAimKeysState>,
    time: Res<Time>,
    mut aim: ResMut<KeyboardAimState>,
    mut last_pending_slot: Local<Option<usize>>,
) {
    if !mouseless_mode.0 {
        aim.ground_aim_offset = Vec2::ZERO;
        *last_pending_slot = None;
        return;
    }

    // Recenter the reticle on the player at the start of each hold-to-aim charge, so every
    // ground-targeted cast starts predictably on the player rather than wherever a previous
    // charge happened to leave off.
    if pending.0 != *last_pending_slot {
        if pending.0.is_some() {
            aim.ground_aim_offset = Vec2::ZERO;
        }
        *last_pending_slot = pending.0;
    }

    // `SwapMovementAimKeysState` (options: "Swap Move/Aim Keys") picks whether arrow keys or
    // WASD drive aim here — whichever one `player_move_inputs` isn't using for movement.
    let (left, right, up, down) = if swap_keys.0 {
        (KeyCode::A, KeyCode::D, KeyCode::W, KeyCode::S)
    } else {
        (KeyCode::Left, KeyCode::Right, KeyCode::Up, KeyCode::Down)
    };
    let mut dir = Vec2::ZERO;
    if key_input.pressed(left) {
        dir.x -= 1.;
    }
    if key_input.pressed(right) {
        dir.x += 1.;
    }
    if key_input.pressed(up) {
        dir.y += 1.;
    }
    if key_input.pressed(down) {
        dir.y -= 1.;
    }
    if dir != Vec2::ZERO {
        let dir = dir.normalize();
        // Instant snap for facing/attacks...
        aim.facing_dir = dir;
        // ...but only accumulate the free-roam ground-target reticle while actually charging
        // one, otherwise it'd silently drift off-player even when nothing is being aimed.
        if pending.0.is_some() {
            aim.ground_aim_offset += dir * sensitivity.speed_px_per_sec() * time.delta_seconds();
        }
    }
}

/// Overrides `CursorPos::world_coords` with the keyboard aim point while Mouseless Mode is on:
/// the free-roam reticle while charging a ground-targeted skill, otherwise the instant facing
/// direction at a fixed range (mirrors `gamepad_input::apply_gamepad_aim_to_cursor_world_pos`).
/// Every gameplay system that aims off `cursor.world_coords` (attacks, facing, active skills)
/// picks this up automatically. `screen_coords`/`ui_coords` are left untouched, so UI
/// hit-testing stays mouse-only. Must run after `update_cursor_pos`.
fn apply_keyboard_aim_to_cursor_world_pos(
    mouseless_mode: Res<MouselessModeState>,
    pending: Res<PendingGroundAimSkill>,
    aim: Res<KeyboardAimState>,
    game: Res<Game>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    if !mouseless_mode.0 {
        return;
    }
    let player_pos = game.player_state.position.truncate();
    let aim_point = if pending.0.is_some() {
        player_pos + aim.ground_aim_offset
    } else {
        if aim.facing_dir == Vec2::ZERO {
            return;
        }
        player_pos + aim.facing_dir * KEYBOARD_AIM_RETICLE_RANGE
    };
    cursor_pos.world_coords = aim_point.extend(cursor_pos.world_coords.z);
}

/// `YSort` bias for the aim reticle — see `gamepad_input::CROSSHAIR_Y_SORT_BIAS` (same value,
/// duplicated here since the two crosshairs are independent entities/modules). Comfortably
/// above every other `YSort` bias used in the codebase (max observed elsewhere is `11.`), so
/// the reticle reliably renders in front of the player/enemies/world sprites at its position
/// instead of being buried by their Y-sorted depth — a static/unsorted Z (e.g. `15.`) sits far
/// behind `YSort`-driven world sprites near the player, which use depths roughly in the 0-900
/// range.
const CROSSHAIR_Y_SORT_BIAS: f32 = 50.0;

/// Marker for the world-space reticle shown while charging a ground-targeted skill's
/// hold-to-aim in Mouseless Mode (see `PendingGroundAimSkill`).
#[derive(Component)]
struct KeyboardCrosshair;

fn setup_keyboard_crosshair(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    existing: Query<Entity, With<KeyboardCrosshair>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        SpriteBundle {
            texture: asset_server.load("ui/icons/Crosshair.png"),
            sprite: Sprite {
                custom_size: Some(Vec2::new(20., 20.)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., 15.)),
            visibility: Visibility::Hidden,
            ..default()
        },
        YSort(CROSSHAIR_Y_SORT_BIAS),
        KeyboardCrosshair,
        Name::new("KeyboardCrosshair"),
    ));
}

/// Only visible while actually charging a ground-targeted skill (`PendingGroundAimSkill`) —
/// unlike the gamepad crosshair, arrow-key aiming for facing/attacks otherwise stays invisible
/// (the player sprite's facing direction is feedback enough for that).
fn update_keyboard_crosshair(
    mouseless_mode: Res<MouselessModeState>,
    pending: Res<PendingGroundAimSkill>,
    aim: Res<KeyboardAimState>,
    game: Res<Game>,
    mut crosshair_query: Query<(&mut Transform, &mut Visibility), With<KeyboardCrosshair>>,
) {
    let Ok((mut transform, mut visibility)) = crosshair_query.get_single_mut() else {
        return;
    };
    let show = mouseless_mode.0 && pending.0.is_some();
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

pub struct KeyboardAimPlugin;

impl Plugin for KeyboardAimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KeyboardAimState>()
            .insert_resource(KeyboardAimSensitivity::load())
            .add_system(setup_keyboard_crosshair.in_schedule(OnEnter(GameState::Main)))
            .add_systems(
                (
                    update_keyboard_aim_state,
                    update_keyboard_crosshair.after(update_keyboard_aim_state),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                apply_keyboard_aim_to_cursor_world_pos
                    .in_base_set(CoreSet::PostUpdate)
                    .after(crate::cursor::update_cursor_pos)
                    .run_if(in_state(GameState::Main)),
            );
    }
}
