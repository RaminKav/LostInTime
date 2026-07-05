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
//! - Left-stick UI navigation uses a separate (stricter) deadzone/commit threshold from gameplay
//!   sticks and only fires once per deliberate tilt — release back to neutral before the next
//!   step. D-pad and keyboard arrows stay digital/unchanged. Tune via Options → "UI Nav
//!   Stability" (1 = responsive, 10 = firm tilt required).
use bevy::ecs::system::SystemParam;
use bevy::input::gamepad::Gamepads;
use bevy::input::mouse::MouseMotion;
use bevy::prelude::*;
use leafwing_input_manager::prelude::ActionState;
use serde::{Deserialize, Serialize};

use crate::cursor::CursorPos;
use crate::gamepad_input::{UiGamepadAction, UiGamepadInputMarker};
use crate::inputs::MouselessModeState;
use crate::ui::{tips::TipBox, tutorial_ui::TutorialUI, InventorySlotState, InventorySlotType, UIState};
use crate::GameState;

/// Options screen "UI Nav Stability" (1-10): how firmly the left stick must be tilted before
/// UI focus moves, and how wide the neutral deadzone is. Higher = less accidental jumping.
#[derive(Resource, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UiNavStickStability(pub u8);

impl Default for UiNavStickStability {
    fn default() -> Self {
        Self(6)
    }
}

impl UiNavStickStability {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 10;

    fn t(&self) -> f32 {
        (self.0.clamp(Self::MIN, Self::MAX) - Self::MIN) as f32 / (Self::MAX - Self::MIN) as f32
    }

    /// Inner deadzone — stick magnitude at or below this reads as neutral.
    pub fn deadzone(&self) -> f32 {
        // 1 -> 0.30, 10 -> 0.60
        0.30 + self.t() * 0.30
    }

    /// Stick must exceed this magnitude to count as a deliberate UI nav input.
    pub fn commit(&self) -> f32 {
        // 1 -> 0.40, 10 -> 0.80
        0.40 + self.t() * 0.40
    }

    pub fn load() -> Self {
        let path = crate::datafiles::game_data();
        if let Ok(file) = std::fs::File::open(&path) {
            let reader = std::io::BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.ui_nav_stick_stability.unwrap_or_default();
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

        game_data.ui_nav_stick_stability = Some(*self);

        if let Ok(file) = std::fs::File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }
}

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

/// Confirmation popup control (wipe data, class/skill unlock, etc.). While any visible
/// [`ModalFocusable`] exists, keyboard/gamepad navigation is limited to those targets.
#[derive(Component, Debug, Clone, Copy)]
pub struct ModalFocusable {
    pub index: u32,
}

/// Options menu footer controls (Back, Wipe, etc.). Only reachable horizontally when focus is
/// already on another bottom-row control; otherwise the player must press Down to get there.
#[derive(Component, Debug, Clone, Copy)]
pub struct FocusNavBottomRow;

/// Left-hand tab strip in the options menu. Horizontal nav from here may enter stepper rows
/// that are otherwise skipped when moving between columns inside the content area.
#[derive(Component, Debug, Clone, Copy)]
pub struct FocusNavTabColumn;

/// Logical focus target that survives inventory slot entity respawns (pickup, upgrade refresh,
/// sort, etc. all despawn + re-spawn slot sprites, which would otherwise invalidate `Entity` ids
/// and trip `ensure_default_focus` back to index 0 / "hotbar slot 1").
#[derive(Clone, Debug, PartialEq)]
pub enum FocusAnchor {
    InvSlot {
        group: UIState,
        slot_type: InventorySlotType,
        slot_index: usize,
    },
    /// Sidebar buttons, craft toggle, and other focusables without [`InventorySlotState`].
    FocusIndex {
        group: UIState,
        index: u32,
    },
}

/// The single currently-focused entity (if any) plus whether Confirm was pressed this frame.
/// Updated by [`ensure_default_focus`], [`focus_nav`], and [`poll_ui_focus_confirm`].
#[derive(Resource, Default)]
pub struct UiFocus {
    pub focused: Option<Entity>,
    pub confirm_just_pressed: bool,
    pub anchor: Option<FocusAnchor>,
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
    Modal,
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

fn modal_focus_active(
    modals: &Query<(Entity, &ModalFocusable)>,
    visibility: &Query<&Visibility>,
) -> bool {
    modals
        .iter()
        .any(|(entity, _)| focus_entity_visible(entity, visibility))
}

fn resolve_focus_mode_for_frame(
    game_state: &GameState,
    ui_state: &UIState,
    has_tip_boxes: bool,
    has_tutorial_ui: bool,
    modals: &Query<(Entity, &ModalFocusable)>,
    visibility: &Query<&Visibility>,
) -> FocusMode {
    if modal_focus_active(modals, visibility) {
        FocusMode::Modal
    } else {
        resolve_focus_mode(game_state, ui_state, has_tip_boxes, has_tutorial_ui)
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
    modals: Query<(Entity, &ModalFocusable)>,
    visibility: Query<&Visibility>,
) -> bool {
    resolve_focus_mode_for_frame(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
        &modals,
        &visibility,
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
    modals: &Query<(Entity, &ModalFocusable)>,
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
        FocusMode::Modal => modals.get(entity).is_ok(),
        FocusMode::None => false,
    }
}

fn mode_matches_screen(mode: &FocusMode, group: &UIState) -> bool {
    matches!(mode, FocusMode::Screen(g) if g == group)
}

fn focusable_inv_eligible(
    entity: Entity,
    ui_state: &UIState,
    inv_slots: &Query<&InventorySlotState>,
) -> bool {
    if !ui_state.is_inv_open() {
        return true;
    }
    // HUD hotbar slots at the bottom of the screen are hidden while the inventory panel is
    // open. Hotbar items are still reachable via the main bag grid (Normal slots 0..N).
    inv_slots
        .get(entity)
        .map(|slot| !slot.r#type.is_hotbar())
        .unwrap_or(true)
}

fn focus_anchor_from_parts(
    entity: Entity,
    focusable: &Focusable,
    inv_slots: &Query<&InventorySlotState>,
) -> FocusAnchor {
    if let Ok(slot) = inv_slots.get(entity) {
        FocusAnchor::InvSlot {
            group: focusable.group.clone(),
            slot_type: slot.r#type,
            slot_index: slot.slot_index,
        }
    } else {
        FocusAnchor::FocusIndex {
            group: focusable.group.clone(),
            index: focusable.index,
        }
    }
}

fn compute_focus_anchor(
    entity: Entity,
    focusables: &Query<(Entity, &Focusable)>,
    inv_slots: &Query<&InventorySlotState>,
) -> Option<FocusAnchor> {
    focusables
        .get(entity)
        .ok()
        .map(|(_, focusable)| focus_anchor_from_parts(entity, focusable, inv_slots))
}

fn find_entity_for_anchor(
    anchor: &FocusAnchor,
    mode: &FocusMode,
    focusables: &Query<(Entity, &Focusable)>,
    inv_slots: &Query<&InventorySlotState>,
    visibility: &Query<&Visibility>,
    ui_state: &UIState,
) -> Option<Entity> {
    match anchor {
        FocusAnchor::InvSlot {
            group,
            slot_type,
            slot_index,
        } => {
            if !mode_matches_screen(mode, group) {
                return None;
            }
            focusables.iter().find_map(|(entity, focusable)| {
                if focusable.group != *group || !focus_entity_visible(entity, visibility) {
                    return None;
                }
                if !focusable_inv_eligible(entity, ui_state, inv_slots) {
                    return None;
                }
                inv_slots.get(entity).ok().and_then(|slot| {
                    (slot.r#type == *slot_type && slot.slot_index == *slot_index).then_some(entity)
                })
            })
        }
        FocusAnchor::FocusIndex { group, index } => {
            if !mode_matches_screen(mode, group) {
                return None;
            }
            focusables.iter().find_map(|(entity, focusable)| {
                (focusable.group == *group
                    && focusable.index == *index
                    && focus_entity_visible(entity, visibility))
                .then_some(entity)
            })
        }
    }
}

fn screen_focus_candidate(
    entity: Entity,
    focusable: &Focusable,
    group: &UIState,
    ui_state: &UIState,
    visibility: &Query<&Visibility>,
    inv_slots: &Query<&InventorySlotState>,
) -> bool {
    focusable.group == *group
        && focus_entity_visible(entity, visibility)
        && focusable_inv_eligible(entity, ui_state, inv_slots)
}

fn default_focus_entity(
    mode: &FocusMode,
    focusables: &Query<(Entity, &Focusable)>,
    overlays: &Query<(Entity, &OverlayFocusable)>,
    modals: &Query<(Entity, &ModalFocusable)>,
    visibility: &Query<&Visibility>,
    inv_slots: &Query<&InventorySlotState>,
    ui_state: &UIState,
) -> Option<Entity> {
    match mode {
        FocusMode::Screen(group) => {
            if group_defers_default_focus(group) {
                return None;
            }
            focusables
                .iter()
                .filter(|(e, f)| {
                    screen_focus_candidate(*e, f, group, ui_state, visibility, inv_slots)
                })
                .min_by_key(|(_, f)| f.index)
                .map(|(e, _)| e)
        }
        FocusMode::Overlay => overlays
            .iter()
            .filter(|(e, _)| focus_entity_visible(*e, visibility))
            .min_by_key(|(_, f)| f.index)
            .map(|(e, _)| e),
        FocusMode::Modal => modals
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
    modals: Query<(Entity, &ModalFocusable)>,
    visibility: Query<&Visibility>,
    mut mouse_motion: EventReader<MouseMotion>,
    mut last_context: Local<Option<FocusContextKey>>,
) {
    if mouse_motion.iter().next().is_some() {
        cursor_pos.suppress_ui_hover = false;
    }

    let mode = resolve_focus_mode_for_frame(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
        &modals,
        &visibility,
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
    modals: Query<(Entity, &ModalFocusable)>,
    visibility: Query<&Visibility>,
    inv_slots: Query<&InventorySlotState>,
) {
    let mode = resolve_focus_mode_for_frame(
        &game_state.0,
        &ui_state.0,
        !tip_boxes.is_empty(),
        !tutorial_ui.is_empty(),
        &modals,
        &visibility,
    );
    let still_valid = ui_focus
        .focused
        .and_then(|e| {
            entity_in_active_focus(e, &mode, &focusables, &overlays, &modals, &visibility)
                .then_some(e)
        })
        .is_some();
    if still_valid {
        if let Some(e) = ui_focus.focused {
            ui_focus.anchor = compute_focus_anchor(e, &focusables, &inv_slots);
        }
        return;
    }
    if let Some(anchor) = ui_focus.anchor.clone() {
        if let Some(e) = find_entity_for_anchor(
            &anchor,
            &mode,
            &focusables,
            &inv_slots,
            &visibility,
            &ui_state.0,
        ) {
            ui_focus.focused = Some(e);
            return;
        }
    }
    ui_focus.focused = default_focus_entity(
        &mode,
        &focusables,
        &overlays,
        &modals,
        &visibility,
        &inv_slots,
        &ui_state.0,
    );
    ui_focus.anchor = ui_focus
        .focused
        .and_then(|e| compute_focus_anchor(e, &focusables, &inv_slots));
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

/// Tracks whether the left stick is currently latched after firing a UI nav step. The stick must
/// return to neutral before another stick-initiated step can fire (d-pad/keyboard unaffected).
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
pub struct UiStickNavLatch {
    armed: bool,
}

/// Returns a direction if the player pressed a UI navigation input this frame.
pub fn ui_nav_dir_just_pressed(
    keys: &Input<KeyCode>,
    gamepad: Option<&ActionState<UiGamepadAction>>,
    stick_latch: &mut UiStickNavLatch,
    stability: UiNavStickStability,
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
    let magnitude = stick.length();
    if magnitude <= stability.deadzone() {
        stick_latch.armed = false;
        return None;
    }
    if magnitude < stability.commit() {
        return None;
    }

    let dir = if stick.x.abs() >= stick.y.abs() {
        if stick.x > 0.0 {
            UiNavDir::Right
        } else {
            UiNavDir::Left
        }
    } else if stick.y > 0.0 {
        UiNavDir::Up
    } else {
        UiNavDir::Down
    };

    if stick_latch.armed {
        return None;
    }
    stick_latch.armed = true;
    Some(dir)
}

fn pressed_nav_dir(
    keys: &Input<KeyCode>,
    gamepad: Option<&ActionState<UiGamepadAction>>,
    stick_latch: &mut UiStickNavLatch,
    stability: UiNavStickStability,
) -> Option<NavDir> {
    ui_nav_dir_just_pressed(keys, gamepad, stick_latch, stability).map(Into::into)
}

/// Layout markers consumed by [`focus_nav`] when choosing the next focus target.
#[derive(SystemParam)]
pub struct FocusNavMarkerQueries<'w, 's> {
    pub horizontal_skip: Query<'w, 's, (), With<FocusNavHorizontalSkip>>,
    pub bottom_row: Query<'w, 's, (), With<FocusNavBottomRow>>,
    pub tab_column: Query<'w, 's, (), With<FocusNavTabColumn>>,
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
    modals: Query<(Entity, &GlobalTransform, &ModalFocusable)>,
    visibility: Query<&Visibility>,
    inv_slots: Query<&InventorySlotState>,
    focus_nav_blocked: Res<FocusNavBlocked>,
    nav_markers: FocusNavMarkerQueries,
    stick_stability: Res<UiNavStickStability>,
    mut stick_latch: Local<UiStickNavLatch>,
) {
    if focus_nav_blocked.0 {
        return;
    }
    let modal_active = modals
        .iter()
        .any(|(entity, _, _)| focus_entity_visible(entity, &visibility));
    let mode = if modal_active {
        FocusMode::Modal
    } else {
        resolve_focus_mode(
            &game_state.0,
            &ui_state.0,
            !tip_boxes.is_empty(),
            !tutorial_ui.is_empty(),
        )
    };
    let Some(dir) = pressed_nav_dir(
        &key_input,
        ui_gamepad_q.get_single().ok(),
        &mut stick_latch,
        *stick_stability,
    ) else {
        return;
    };

    let pick_initial_screen_focus = |group: &UIState| {
        focusables
            .iter()
            .filter(|(e, _, f)| {
                screen_focus_candidate(*e, f, group, &ui_state.0, &visibility, &inv_slots)
            })
            .min_by_key(|(_, _, f)| f.index)
            .map(|(e, _, _)| e)
    };

    let Some(cur_e) = ui_focus.focused else {
        ui_focus.focused = match &mode {
            FocusMode::Screen(group) => pick_initial_screen_focus(group),
            FocusMode::Overlay => overlays
                .iter()
                .filter(|(e, _, _)| focus_entity_visible(*e, &visibility))
                .min_by_key(|(_, _, f)| f.index)
                .map(|(e, _, _)| e),
            FocusMode::Modal => modals
                .iter()
                .filter(|(e, _, _)| focus_entity_visible(*e, &visibility))
                .min_by_key(|(_, _, f)| f.index)
                .map(|(e, _, _)| e),
            FocusMode::None => None,
        };
        if let Some(e) = ui_focus.focused {
            if let Ok((_, _, focusable)) = focusables.get(e) {
                ui_focus.anchor = Some(focus_anchor_from_parts(e, focusable, &inv_slots));
            }
        }
        return;
    };

    if !focus_entity_visible(cur_e, &visibility) {
        if let Some(anchor) = ui_focus.anchor.clone() {
            let restored = match &anchor {
                FocusAnchor::InvSlot {
                    group,
                    slot_type,
                    slot_index,
                } if mode_matches_screen(&mode, group) => focusables.iter().find_map(
                    |(entity, _, focusable)| {
                        if focusable.group != *group
                            || !focus_entity_visible(entity, &visibility)
                            || !focusable_inv_eligible(entity, &ui_state.0, &inv_slots)
                        {
                            return None;
                        }
                        inv_slots.get(entity).ok().and_then(|slot| {
                            (slot.r#type == *slot_type && slot.slot_index == *slot_index)
                                .then_some(entity)
                        })
                    },
                ),
                FocusAnchor::FocusIndex { group, index }
                    if mode_matches_screen(&mode, group) =>
                {
                    focusables.iter().find_map(|(entity, _, focusable)| {
                        (focusable.group == *group
                            && focusable.index == *index
                            && focus_entity_visible(entity, &visibility))
                        .then_some(entity)
                    })
                }
                _ => None,
            };
            if let Some(e) = restored {
                ui_focus.focused = Some(e);
                return;
            }
        }
        ui_focus.focused = match &mode {
            FocusMode::Screen(group) => pick_initial_screen_focus(group),
            FocusMode::Overlay => overlays
                .iter()
                .filter(|(entity, _, _)| focus_entity_visible(*entity, &visibility))
                .min_by_key(|(_, _, overlay)| overlay.index)
                .map(|(entity, _, _)| entity),
            FocusMode::Modal => modals
                .iter()
                .filter(|(entity, _, _)| focus_entity_visible(*entity, &visibility))
                .min_by_key(|(_, _, modal)| modal.index)
                .map(|(entity, _, _)| entity),
            FocusMode::None => None,
        };
        if let Some(e) = ui_focus.focused {
            if let Ok((_, _, focusable)) = focusables.get(e) {
                ui_focus.anchor = Some(focus_anchor_from_parts(e, focusable, &inv_slots));
            }
        }
        return;
    }

    let cur_pos = if let Ok((_, xf, _)) = focusables.get(cur_e) {
        xf.translation().truncate()
    } else if let Ok((_, xf, _)) = overlays.get(cur_e) {
        xf.translation().truncate()
    } else if let Ok((_, xf, _)) = modals.get(cur_e) {
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
    let cur_in_bottom_row = nav_markers.bottom_row.get(cur_e).is_ok();
    let cur_in_tab_column = nav_markers.tab_column.get(cur_e).is_ok();
    let mut consider = |e: Entity, pos: Vec2| {
        if e == cur_e {
            return;
        }
        if horizontal_nav {
            if nav_markers.horizontal_skip.get(e).is_ok() && !cur_in_tab_column {
                return;
            }
            if cur_in_bottom_row != nav_markers.bottom_row.get(e).is_ok() {
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
                if !screen_focus_candidate(e, focusable, group, &ui_state.0, &visibility, &inv_slots)
                {
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
        FocusMode::Modal => {
            for (e, xf, _) in modals.iter() {
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
        if let Ok((_, _, focusable)) = focusables.get(e) {
            ui_focus.anchor = Some(focus_anchor_from_parts(e, focusable, &inv_slots));
        }
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
            .insert_resource(UiNavStickStability::load())
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
