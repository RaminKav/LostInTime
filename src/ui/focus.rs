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
//! - [`OverlayFocusable`] is the same idea for transient HUD overlays (game over, tip OK,
//!   tutorial Done) that appear while `UIState::Closed`.
//! - [`UiFocus`] holds the currently-focused entity and whether Confirm was pressed this frame.
//! - Per-screen `handle_cursor_*` handlers OR in `ui_focus.is_focused(entity)` /
//!   `ui_focus.confirm_just_pressed` alongside their existing mouse checks — see
//!   `handle_cursor_main_menu_buttons` in `src/ui/interactions.rs` for the reference patch.
//! - Cancel (gamepad B / keyboard Escape) needed **no** new plumbing — `close_container` in
//!   `src/inputs.rs` already centralizes Escape handling for every screen.
use bevy::ecs::system::SystemParam;
use bevy::input::gamepad::Gamepads;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;

use crate::cursor::CursorPos;
use crate::gamepad_input::{UiGamepadAction, UiGamepadInputMarker, GAMEPAD_STICK_DEADZONE};
use crate::inputs::MouselessModeState;
use crate::ui::{tips::TipBox, tutorial_ui::TutorialUI, UIState};
use crate::GameState;

/// Tags a clickable sprite (alongside the existing `Interactable`) as part of directional focus
/// navigation. `group` scopes navigation to "whichever screen/panel is currently active" — see
/// the module doc for why `index` is a tie-breaker only, not the navigation mechanism.
#[derive(Component, Debug, Clone)]
pub struct Focusable {
    pub group: UIState,
    pub index: u32,
}

/// Focus target for transient overlays that appear without changing [`UIState`] (game over
/// buttons, tip OK, tutorial Done/Prev/Next).
#[derive(Component, Debug, Clone, Copy)]
pub struct OverlayFocusable {
    pub index: u32,
}

/// Skip this entity when [`focus_nav`] moves left/right. Used for wide options stepper row
/// hitboxes that sit left of same-column controls and would otherwise win horizontal nav.
#[derive(Component, Debug, Clone, Copy)]
pub struct FocusNavHorizontalSkip;

/// Options menu footer controls (Back, Wipe, etc.). Only reachable horizontally when focus is
/// already on another bottom-row control; otherwise the player must press Down to get there.
#[derive(Component, Debug, Clone, Copy)]
pub struct FocusNavBottomRow;

/// The single currently-focused entity (if any) plus whether Confirm was pressed this frame.
/// Updated by [`ensure_default_focus`], [`focus_nav`], and [`poll_ui_focus_confirm`].
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

#[derive(Clone, PartialEq, Eq)]
enum FocusMode {
    Screen(UIState),
    Overlay,
    None,
}

/// Tracks the active focus context so we can tell when a UI screen or overlay just opened.
#[derive(Clone, PartialEq, Eq)]
struct FocusContextKey(FocusMode);

fn resolve_focus_mode(
    game_state: &GameState,
    ui_state: &UIState,
    has_tip_boxes: bool,
    has_tutorial_ui: bool,
) -> FocusMode {
    if *ui_state != UIState::Closed {
        FocusMode::Screen(ui_state.clone())
    } else if *game_state == GameState::GameOver || has_tip_boxes || has_tutorial_ui {
        FocusMode::Overlay
    } else if *game_state == GameState::MainMenu {
        // Main-menu buttons use `Focusable { group: UIState::Closed, .. }`.
        FocusMode::Screen(UIState::Closed)
    } else {
        FocusMode::None
    }
}

/// Focus navigation (and therefore the d-pad/left-stick) is only "live" outside of plain
/// gameplay — while actually playing with no panel open, the d-pad/South button mean
/// hotbar/skills instead (see `GamepadAction` in `gamepad_input.rs`).
fn focus_should_run(
    game_state: Res<State<GameState>>,
    ui_state: Res<State<UIState>>,
    tip_boxes: Query<Entity, With<TipBox>>,
    tutorial_ui: Query<(), With<TutorialUI>>,
) -> bool {
    resolve_focus_mode(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
    ) != FocusMode::None
}

fn poll_ui_focus_confirm(
    mut ui_focus: ResMut<UiFocus>,
    key_input: Res<Input<KeyCode>>,
    ui_gamepad_q: Query<&ActionState<UiGamepadAction>, With<UiGamepadInputMarker>>,
) {
    ui_focus.confirm_just_pressed = key_input.just_pressed(KeyCode::Return)
        || key_input.just_pressed(KeyCode::NumpadEnter)
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

pub fn focus_entity_visible(entity: Entity, visibility: &Query<&Visibility>) -> bool {
    visibility
        .get(entity)
        .map(|v| *v != Visibility::Hidden)
        .unwrap_or(true)
}

fn entity_in_active_focus(
    entity: Entity,
    mode: &FocusMode,
    focusables: &Query<(Entity, &Focusable)>,
    overlays: &Query<(Entity, &OverlayFocusable)>,
    visibility: &Query<&Visibility>,
) -> bool {
    if !focus_entity_visible(entity, visibility) {
        return false;
    }
    match mode {
        FocusMode::Screen(group) => focusables
            .get(entity)
            .map(|(_, f)| f.group == *group)
            .unwrap_or(false),
        FocusMode::Overlay => overlays.get(entity).is_ok(),
        FocusMode::None => false,
    }
}

fn default_focus_entity(
    mode: &FocusMode,
    focusables: &Query<(Entity, &Focusable)>,
    overlays: &Query<(Entity, &OverlayFocusable)>,
    visibility: &Query<&Visibility>,
) -> Option<Entity> {
    match mode {
        FocusMode::Screen(group) => {
            if group_defers_default_focus(group) {
                return None;
            }
            focusables
                .iter()
                .filter(|(e, f)| f.group == *group && focus_entity_visible(*e, visibility))
                .min_by_key(|(_, f)| f.index)
                .map(|(e, _)| e)
        }
        FocusMode::Overlay => overlays
            .iter()
            .filter(|(e, _)| focus_entity_visible(*e, visibility))
            .min_by_key(|(_, f)| f.index)
            .map(|(e, _)| e),
        FocusMode::None => None,
    }
}

/// While mouseless mode is on or a gamepad is connected, ignore a stationary cursor when a UI
/// screen/overlay first opens. Without this, a mouse resting in the middle of the screen would
/// immediately hover whatever it sits on (e.g. a skill-choice card) until the player nudges it.
/// Mouse hover resumes as soon as the player actually moves the mouse.
fn update_cursor_ui_hover_suppression(
    mut cursor_pos: ResMut<CursorPos>,
    game_state: Res<State<GameState>>,
    ui_state: Res<State<UIState>>,
    tip_boxes: Query<Entity, With<TipBox>>,
    tutorial_ui: Query<(), With<TutorialUI>>,
    mouseless_mode: Res<MouselessModeState>,
    gamepads: Res<Gamepads>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut last_context: Local<Option<FocusContextKey>>,
) {
    if mouse_motion.iter().next().is_some() {
        cursor_pos.suppress_ui_hover = false;
    }

    let mode = resolve_focus_mode(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
    );
    let context = FocusContextKey(mode.clone());
    let context_changed = last_context.as_ref() != Some(&context);
    if context_changed {
        *last_context = Some(context);
        let prefer_non_mouse_ui = mouseless_mode.0 || gamepads.iter().next().is_some();
        if mode != FocusMode::None && prefer_non_mouse_ui {
            cursor_pos.suppress_ui_hover = true;
        }
    }
}

/// Keeps [`UiFocus::focused`] pointing at a valid focus target for the active mode.
fn ensure_default_focus(
    game_state: Res<State<GameState>>,
    ui_state: Res<State<UIState>>,
    tip_boxes: Query<Entity, With<TipBox>>,
    tutorial_ui: Query<(), With<TutorialUI>>,
    mut ui_focus: ResMut<UiFocus>,
    focusables: Query<(Entity, &Focusable)>,
    overlays: Query<(Entity, &OverlayFocusable)>,
    visibility: Query<&Visibility>,
) {
    let mode = resolve_focus_mode(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
    );
    let still_valid = ui_focus
        .focused
        .and_then(|e| {
            entity_in_active_focus(e, &mode, &focusables, &overlays, &visibility).then_some(e)
        })
        .is_some();
    if still_valid {
        return;
    }
    ui_focus.focused = default_focus_entity(&mode, &focusables, &overlays, &visibility);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NavDir {
    Up,
    Down,
    Left,
    Right,
}

/// Directional UI navigation input (keyboard arrows, d-pad, or left stick).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UiNavDir {
    Up,
    Down,
    Left,
    Right,
}

impl From<UiNavDir> for NavDir {
    fn from(d: UiNavDir) -> Self {
        match d {
            UiNavDir::Up => NavDir::Up,
            UiNavDir::Down => NavDir::Down,
            UiNavDir::Left => NavDir::Left,
            UiNavDir::Right => NavDir::Right,
        }
    }
}

/// Clears [`FocusNavBlocked`] at the start of each frame before row-level handlers run.
pub fn reset_focus_nav_blocked(mut blocked: ResMut<FocusNavBlocked>) {
    blocked.0 = false;
}

/// Returns a direction if the player pressed a UI navigation input this frame.
pub fn ui_nav_dir_just_pressed(
    keys: &Input<KeyCode>,
    gamepad: Option<&ActionState<UiGamepadAction>>,
    last_stick_dir: &mut Option<UiNavDir>,
) -> Option<UiNavDir> {
    if keys.just_pressed(KeyCode::Up) {
        return Some(UiNavDir::Up);
    }
    if keys.just_pressed(KeyCode::Down) {
        return Some(UiNavDir::Down);
    }
    if keys.just_pressed(KeyCode::Left) {
        return Some(UiNavDir::Left);
    }
    if keys.just_pressed(KeyCode::Right) {
        return Some(UiNavDir::Right);
    }

    let Some(gamepad) = gamepad else {
        return None;
    };
    if gamepad.just_pressed(UiGamepadAction::NavUp) {
        return Some(UiNavDir::Up);
    }
    if gamepad.just_pressed(UiGamepadAction::NavDown) {
        return Some(UiNavDir::Down);
    }
    if gamepad.just_pressed(UiGamepadAction::NavLeft) {
        return Some(UiNavDir::Left);
    }
    if gamepad.just_pressed(UiGamepadAction::NavRight) {
        return Some(UiNavDir::Right);
    }

    let stick = gamepad
        .clamped_axis_pair(UiGamepadAction::NavStick)
        .map(|p| p.xy())
        .unwrap_or(Vec2::ZERO);
    let dir = if stick.length_squared() <= GAMEPAD_STICK_DEADZONE.powi(2) {
        None
    } else if stick.x.abs() >= stick.y.abs() {
        Some(if stick.x > 0.0 {
            UiNavDir::Right
        } else {
            UiNavDir::Left
        })
    } else {
        Some(if stick.y > 0.0 {
            UiNavDir::Up
        } else {
            UiNavDir::Down
        })
    };
    if dir != *last_stick_dir {
        *last_stick_dir = dir;
        return dir;
    }
    None
}

fn pressed_nav_dir(
    keys: &Input<KeyCode>,
    gamepad: Option<&ActionState<UiGamepadAction>>,
    last_stick_dir: &mut Option<NavDir>,
) -> Option<NavDir> {
    let mut ui_stick = last_stick_dir.map(|d| match d {
        NavDir::Up => UiNavDir::Up,
        NavDir::Down => UiNavDir::Down,
        NavDir::Left => UiNavDir::Left,
        NavDir::Right => UiNavDir::Right,
    });
    let result = ui_nav_dir_just_pressed(keys, gamepad, &mut ui_stick);
    *last_stick_dir = ui_stick.map(Into::into);
    result.map(Into::into)
}

const NAV_LATERAL_PENALTY: f32 = 3.0;

fn focus_nav(
    key_input: Res<Input<KeyCode>>,
    ui_gamepad_q: Query<&ActionState<UiGamepadAction>, With<UiGamepadInputMarker>>,
    game_state: Res<State<GameState>>,
    ui_state: Res<State<UIState>>,
    tip_boxes: Query<Entity, With<TipBox>>,
    tutorial_ui: Query<(), With<TutorialUI>>,
    mut ui_focus: ResMut<UiFocus>,
    focusables: Query<(Entity, &GlobalTransform, &Focusable)>,
    overlays: Query<(Entity, &GlobalTransform, &OverlayFocusable)>,
    visibility: Query<&Visibility>,
    focus_nav_blocked: Res<FocusNavBlocked>,
    focus_nav_horizontal_skip: Query<(), With<FocusNavHorizontalSkip>>,
    focus_nav_bottom_row: Query<(), With<FocusNavBottomRow>>,
    mut last_stick_dir: Local<Option<NavDir>>,
) {
    if focus_nav_blocked.0 {
        return;
    }
    let mode = resolve_focus_mode(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
    );
    let Some(dir) = pressed_nav_dir(
        &key_input,
        ui_gamepad_q.get_single().ok(),
        &mut last_stick_dir,
    ) else {
        return;
    };

    let Some(cur_e) = ui_focus.focused else {
        ui_focus.focused = match &mode {
            FocusMode::Screen(group) => focusables
                .iter()
                .filter(|(e, _, f)| &f.group == group && focus_entity_visible(*e, &visibility))
                .min_by_key(|(_, _, f)| f.index)
                .map(|(e, _, _)| e),
            FocusMode::Overlay => overlays
                .iter()
                .filter(|(e, _, _)| focus_entity_visible(*e, &visibility))
                .min_by_key(|(_, _, f)| f.index)
                .map(|(e, _, _)| e),
            FocusMode::None => None,
        };
        return;
    };

    if !focus_entity_visible(cur_e, &visibility) {
        ui_focus.focused = match &mode {
            FocusMode::Screen(group) => focusables
                .iter()
                .filter(|(entity, _, focusable)| {
                    &focusable.group == group && focus_entity_visible(*entity, &visibility)
                })
                .min_by_key(|(_, _, focusable)| focusable.index)
                .map(|(entity, _, _)| entity),
            FocusMode::Overlay => overlays
                .iter()
                .filter(|(entity, _, _)| focus_entity_visible(*entity, &visibility))
                .min_by_key(|(_, _, overlay)| overlay.index)
                .map(|(entity, _, _)| entity),
            FocusMode::None => None,
        };
        return;
    }

    let cur_pos = if let Ok((_, xf, _)) = focusables.get(cur_e) {
        xf.translation().truncate()
    } else if let Ok((_, xf, _)) = overlays.get(cur_e) {
        xf.translation().truncate()
    } else {
        return;
    };

    let axis = match dir {
        NavDir::Up | NavDir::Down => Vec2::Y,
        NavDir::Left | NavDir::Right => Vec2::X,
    };
    let sign = match dir {
        NavDir::Up | NavDir::Right => 1.0,
        NavDir::Down | NavDir::Left => -1.0,
    };

    let mut best: Option<(Entity, f32)> = None;

    let horizontal_nav = matches!(dir, NavDir::Left | NavDir::Right);
    let cur_in_bottom_row = focus_nav_bottom_row.get(cur_e).is_ok();
    let mut consider = |e: Entity, pos: Vec2| {
        if e == cur_e {
            return;
        }
        if horizontal_nav {
            if focus_nav_horizontal_skip.get(e).is_ok() {
                return;
            }
            if cur_in_bottom_row != focus_nav_bottom_row.get(e).is_ok() {
                return;
            }
        }
        let delta = pos - cur_pos;
        let forward = delta.dot(axis) * sign;
        if forward <= 0.5 {
            return;
        }
        let lateral = (delta - axis * delta.dot(axis)).length();
        if lateral > forward * NAV_LATERAL_PENALTY {
            return;
        }
        let score = forward + lateral * NAV_LATERAL_PENALTY;
        if best.map_or(true, |(_, best_score)| score < best_score) {
            best = Some((e, score));
        }
    };

    match &mode {
        FocusMode::Screen(group) => {
            for (e, xf, focusable) in focusables.iter() {
                if &focusable.group != group || !focus_entity_visible(e, &visibility) {
                    continue;
                }
                consider(e, xf.translation().truncate());
            }
        }
        FocusMode::Overlay => {
            for (e, xf, _) in overlays.iter() {
                if !focus_entity_visible(e, &visibility) {
                    continue;
                }
                consider(e, xf.translation().truncate());
            }
        }
        FocusMode::None => {}
    }

    if let Some((e, _)) = best {
        ui_focus.focused = Some(e);
    }
}

/// When true, [`focus_nav`] skips this frame (e.g. options stepper rows consumed Left/Right).
#[derive(Resource, Default)]
pub struct FocusNavBlocked(pub bool);

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct FocusNavSet;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub struct FocusConfirmSet;

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiFocus>()
            .init_resource::<FocusNavBlocked>()
            .add_system(
                update_cursor_ui_hover_suppression.in_base_set(CoreSet::PreUpdate),
            )
            .add_system(ensure_default_focus)
            .add_system(reset_focus_nav_blocked.before(FocusNavSet))
            .add_systems(
                (
                    poll_ui_focus_confirm.in_set(FocusConfirmSet),
                    focus_nav.in_set(FocusNavSet),
                )
                    .chain()
                    .after(ensure_default_focus)
                    .distributive_run_if(focus_should_run),
            );
    }
}
