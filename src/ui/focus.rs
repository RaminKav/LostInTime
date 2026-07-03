//! UI focus navigation ("Track 3" of the controller support plan): lets keyboard arrow keys
//! and a gamepad d-pad/left-stick drive the *existing* mouse-hover/click UI instead of building
//! a parallel input path per screen.
//!
//! Design (see the controller support plan for the full rationale):
//! - [`Focusable`] tags a clickable sprite with which screen/panel (`group`) it belongs to.
//!   Navigation is **position-based**, not index-based: every `Focusable` already has a
//!   `Sprite` + `GlobalTransform` (the same data [`crate::ui::ui_helpers::pointcast_2d`] uses
//!   for mouse hit-testing), so pressing a direction finds the nearest `Focusable` in that
//!   direction with no hand-authored grid geometry needed. `index` is only a tie-breaker /
//!   default-focus pick, never the thing that drives navigation.
//! - [`UiFocus`] holds the currently-focused entity and whether Confirm was pressed this frame.
//! - Per-screen `handle_cursor_*` handlers OR in `ui_focus.is_focused(entity)` /
//!   `ui_focus.confirm_just_pressed` alongside their existing mouse checks — see
//!   `handle_cursor_main_menu_buttons` in `src/ui/interactions.rs` for the reference patch.
//! - Cancel (gamepad B / keyboard Escape) needed **no** new plumbing — `close_container` in
//!   `src/inputs.rs` already centralizes Escape handling for every screen.
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::gamepad_input::{UiGamepadAction, UiGamepadInputMarker, GAMEPAD_STICK_DEADZONE};
use crate::ui::UIState;
use crate::GameState;

/// Tags a clickable sprite (alongside the existing `Interactable`) as part of directional focus
/// navigation. `group` scopes navigation to "whichever screen/panel is currently active" — see
/// the module doc for why `index` is a tie-breaker only, not the navigation mechanism.
#[derive(Component, Debug, Clone)]
pub struct Focusable {
    pub group: UIState,
    pub index: u32,
}

/// The single currently-focused `Focusable` entity (if any) plus whether Confirm was pressed
/// this frame. Updated by [`ensure_default_focus`], [`focus_nav`], and [`poll_ui_focus_confirm`].
#[derive(Resource, Default)]
pub struct UiFocus {
    pub focused: Option<Entity>,
    pub confirm_just_pressed: bool,
}

impl UiFocus {
    pub fn is_focused(&self, entity: Entity) -> bool {
        self.focused == Some(entity)
    }
}

/// Thin alias so handlers can take one `SystemParam` instead of `Res<UiFocus>` when they're
/// not already at Bevy's system-param tuple limit.
#[derive(SystemParam)]
pub struct FocusInput<'w> {
    pub ui_focus: Res<'w, UiFocus>,
}

impl FocusInput<'_> {
    pub fn is_focused(&self, entity: Entity) -> bool {
        self.ui_focus.is_focused(entity)
    }

    pub fn confirm_just_pressed(&self) -> bool {
        self.ui_focus.confirm_just_pressed
    }
}

/// Focus navigation (and therefore the d-pad/left-stick) is only "live" outside of plain
/// gameplay — while actually playing with no panel open, the d-pad/South button mean
/// hotbar/skills instead (see `GamepadAction` in `gamepad_input.rs`).
fn focus_should_run(game_state: Res<State<GameState>>, ui_state: Res<State<UIState>>) -> bool {
    game_state.0 == GameState::MainMenu || ui_state.0 != UIState::Closed
}

fn poll_ui_focus_confirm(
    mut ui_focus: ResMut<UiFocus>,
    key_input: Res<Input<KeyCode>>,
    ui_gamepad_q: Query<&ActionState<UiGamepadAction>, With<UiGamepadInputMarker>>,
) {
    ui_focus.confirm_just_pressed = key_input.just_pressed(KeyCode::Return)
        || ui_gamepad_q
            .get_single()
            .map(|a| a.just_pressed(UiGamepadAction::Confirm))
            .unwrap_or(false);
}

/// Screens where nothing should appear focused/hovered until the player actually presses a nav
/// direction — a heirloom/blessing card auto-highlighting itself (hover art, scale-up, sound)
/// the instant these choice screens open reads as an unintended default pick rather than
/// deliberate navigation. Everywhere else (main menu, class selection, etc.) keeps the original
/// "always something focused" behavior, which is what game-pad-first UIs normally expect.
fn group_defers_default_focus(group: &UIState) -> bool {
    matches!(
        group,
        UIState::Pause | UIState::Skills | UIState::BlessingChoice
    )
}

/// Keeps [`UiFocus::focused`] pointing at a valid `Focusable` in the currently-active group —
/// except for [`group_defers_default_focus`] groups, which start with nothing focused at all
/// until [`focus_nav`] sees the first explicit direction press.
fn ensure_default_focus(
    ui_state: Res<State<UIState>>,
    mut ui_focus: ResMut<UiFocus>,
    focusables: Query<(Entity, &Focusable)>,
) {
    let group = &ui_state.0;
    let still_valid = ui_focus
        .focused
        .and_then(|e| focusables.get(e).ok())
        .is_some_and(|(_, f)| &f.group == group);
    if still_valid {
        return;
    }
    if group_defers_default_focus(group) {
        ui_focus.focused = None;
        return;
    }
    ui_focus.focused = focusables
        .iter()
        .filter(|(_, f)| &f.group == group)
        .min_by_key(|(_, f)| f.index)
        .map(|(e, _)| e);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

fn pressed_nav_dir(
    keys: &Input<KeyCode>,
    gamepad: Option<&ActionState<UiGamepadAction>>,
    last_stick_dir: &mut Option<NavDir>,
) -> Option<NavDir> {
    if keys.just_pressed(KeyCode::Up) {
        return Some(NavDir::Up);
    }
    if keys.just_pressed(KeyCode::Down) {
        return Some(NavDir::Down);
    }
    if keys.just_pressed(KeyCode::Left) {
        return Some(NavDir::Left);
    }
    if keys.just_pressed(KeyCode::Right) {
        return Some(NavDir::Right);
    }

    let Some(gamepad) = gamepad else {
        return None;
    };
    if gamepad.just_pressed(UiGamepadAction::NavUp) {
        return Some(NavDir::Up);
    }
    if gamepad.just_pressed(UiGamepadAction::NavDown) {
        return Some(NavDir::Down);
    }
    if gamepad.just_pressed(UiGamepadAction::NavLeft) {
        return Some(NavDir::Left);
    }
    if gamepad.just_pressed(UiGamepadAction::NavRight) {
        return Some(NavDir::Right);
    }

    let stick = gamepad
        .clamped_axis_pair(UiGamepadAction::NavStick)
        .map(|p| p.xy())
        .unwrap_or(Vec2::ZERO);
    let dir = if stick.length_squared() <= GAMEPAD_STICK_DEADZONE.powi(2) {
        None
    } else if stick.x.abs() >= stick.y.abs() {
        Some(if stick.x > 0.0 {
            NavDir::Right
        } else {
            NavDir::Left
        })
    } else {
        Some(if stick.y > 0.0 {
            NavDir::Up
        } else {
            NavDir::Down
        })
    };
    if dir != *last_stick_dir {
        *last_stick_dir = dir;
        return dir;
    }
    None
}

const NAV_LATERAL_PENALTY: f32 = 3.0;

fn focus_nav(
    key_input: Res<Input<KeyCode>>,
    ui_gamepad_q: Query<&ActionState<UiGamepadAction>, With<UiGamepadInputMarker>>,
    ui_state: Res<State<UIState>>,
    mut ui_focus: ResMut<UiFocus>,
    focusables: Query<(Entity, &GlobalTransform, &Focusable)>,
    mut last_stick_dir: Local<Option<NavDir>>,
) {
    let Some(dir) = pressed_nav_dir(
        &key_input,
        ui_gamepad_q.get_single().ok(),
        &mut last_stick_dir,
    ) else {
        return;
    };
    let group = &ui_state.0;
    let cur_e = match ui_focus.focused {
        Some(e) => e,
        // No focus yet — either a `group_defers_default_focus` screen (see that fn) that starts
        // with nothing picked, or the group just changed. Either way, this first direction press
        // just reveals the default pick instead of navigating from it.
        None => {
            ui_focus.focused = focusables
                .iter()
                .filter(|(_, _, f)| &f.group == group)
                .min_by_key(|(_, _, f)| f.index)
                .map(|(e, _, _)| e);
            return;
        }
    };
    let Ok((_, cur_xf, cur_focusable)) = focusables.get(cur_e) else {
        return;
    };
    if &cur_focusable.group != group {
        return;
    }
    let cur_pos = cur_xf.translation().truncate();

    let axis = match dir {
        NavDir::Up | NavDir::Down => Vec2::Y,
        NavDir::Left | NavDir::Right => Vec2::X,
    };
    let sign = match dir {
        NavDir::Up | NavDir::Right => 1.0,
        NavDir::Down | NavDir::Left => -1.0,
    };

    let mut best: Option<(Entity, f32)> = None;
    for (e, xf, focusable) in focusables.iter() {
        if e == cur_e || &focusable.group != group {
            continue;
        }
        let delta = xf.translation().truncate() - cur_pos;
        let forward = delta.dot(axis) * sign;
        if forward <= 0.5 {
            continue;
        }
        let lateral = (delta - axis * delta.dot(axis)).length();
        if lateral > forward * NAV_LATERAL_PENALTY {
            continue;
        }
        let score = forward + lateral * NAV_LATERAL_PENALTY;
        // `map_or` (not `is_none_or`, stabilized in Rust 1.82) so this builds on older
        // toolchains too — the Windows build machine has been lagging behind macOS's rustc.
        if best.map_or(true, |(_, best_score)| score < best_score) {
            best = Some((e, score));
        }
    }

    if let Some((e, _)) = best {
        ui_focus.focused = Some(e);
    }
}

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFocus>()
            // `ensure_default_focus` runs unconditionally (not gated on `focus_should_run`) so
            // it can still clear `UiFocus::focused` the moment a screen closes (e.g. gamepad
            // pause via `UIState::Pause` -> `Closed`). Without this, closing a screen while
            // something was focused left `UiFocus::focused` pointing at a now-stale entity
            // forever (nothing ever ran to reset it), which downstream per-screen handlers
            // (e.g. HUD icon tooltips) kept reading as "still focused" — a tooltip stuck open
            // after unpausing with no way to dismiss it.
            .add_system(ensure_default_focus)
            .add_systems(
                (
                    poll_ui_focus_confirm,
                    focus_nav.after(ensure_default_focus),
                )
                    .distributive_run_if(focus_should_run),
            );
    }
}
