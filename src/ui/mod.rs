pub mod chest_ui;
pub mod class_selection;
pub mod crafting_ui;
pub mod damage_numbers;
pub mod global_text_message;
pub mod guide_hud;
pub mod item_chest;
mod loading_screen;
pub mod scrapper_ui;
pub mod screen_effects;
use class_selection::*;
use class_selection::{ClassUnlockConfirmState, ClassUnlockHoverState, SkillUnlockConfirmState};
use guide_hud::*;
use item_chest::*;
pub mod ui_container_param;
pub mod tips;
use tips::*;
pub mod tutorial_ui;
use bevy::{render::view::RenderLayers, sprite::Material2dPlugin};
use damage_numbers::FloatingTextQueue;
use scrapper_ui::{
    add_inv_to_new_scrapper_objs, change_ui_state_to_scrapper_when_resource_added,
    handle_scrap_items_in_scrapper, setup_scrapper_slots_ui, ScrapperContainer, ScrapperEvent,
};
use screen_effects::{handle_screen_effects, setup_screen_effects, ScreenEffectMaterial};
pub use ui_container_param::*;
pub mod boss_health_bar;
mod enemy_health_bar;
mod fps_text;

pub mod font_binarize;
pub mod game_fonts;
pub mod layout_sync;
pub mod text_pixel_snap;
pub mod key_input_guide;
use key_input_guide::*;
pub mod intro_guide;
use intro_guide::IntroGuidePlugin;
pub mod furnace_ui;
mod heirloom_tooltip;
pub mod tooltip_info_boxes;
pub use heirloom_tooltip::{
    process_heirloom_tooltip_requests, HeirloomDynamicTooltip, HeirloomTooltipRequest,
    HeirloomTooltipShow,
};
pub use tooltip_info_boxes::{
   HeirloomDescLine, HeirloomDescLineKind, TooltipDefinition
};
pub use skill_choice_ui::*;
mod achievement_banner;
mod active_skill_shrine_ui;
mod interactions;
mod inventory_ui;
pub mod upgrade_drag;
pub mod minimap;
pub mod map_markers;
pub mod hud_bar_fill;
mod player_hud;
mod player_movement_cooldown_bar;
mod skill_choice_ui;
pub mod banish_tracker_ui;
pub use banish_tracker_ui::*;
pub mod stats_ui;
pub use active_skill_shrine_ui::*;
mod icon_hover_tooltips;
mod tile_hover;
mod tooltips;
pub use icon_hover_tooltips::*;
pub mod microwave_shrine_ui;
pub use microwave_shrine_ui::*;
pub mod ui_helpers;
pub use chest_ui::*;
pub use enemy_health_bar::*;
use fps_text::*;
pub use furnace_ui::*;
pub use interactions::*;
pub use inventory_ui::*;
pub use player_hud::*;
pub use player_movement_cooldown_bar::*;
pub use tooltips::*;
mod main_menu;
pub use main_menu::*;
mod essence_ui;
pub use essence_ui::*;
mod unlocks_ui;
pub use unlocks_ui::*;
mod options_ui;
pub use options_ui::*;
mod leaderboard_ui;
pub use leaderboard_ui::*;
mod name_entry_ui;
pub use name_entry_ui::*;
mod time_crystal_progress_ui;
pub use time_crystal_progress_ui::*;
mod time_crystals_browser_ui;
pub use time_crystals_browser_ui::*;
mod heirloom_browser_grid;
pub use heirloom_browser_grid::*;
mod skill_browser_grid;
pub use skill_browser_grid::*;
mod beastiary_browser_ui;
pub use beastiary_browser_ui::*;
mod achievements_ui;
use crate::run_once_per_run;
use crate::ui::achievement_banner::{
    debug_trigger_achievement_banner, handle_achievement_banner_events, update_achievement_banners,
};
use crate::ui::damage_numbers::{
    handle_clamp_screen_locked_icons_worldpos, BeaconGuidanceRegistry,
};
use crate::ui::global_text_message::{
    handle_global_text_message_events, show_pending_era_announcement,
    tick_global_text_messages, GlobalTextMessageEvent,
};
pub use achievements_ui::*;

pub fn reset_microwave_shrine_usages(mut usages: ResMut<MicrowaveShrineUsages>) {
    usages.0 = 0;
}

pub fn reset_break_drop_filter(mut break_drop_filter: ResMut<crate::inventory::BreakDropFilter>) {
    break_drop_filter.0.clear();
}
use loading_screen::*;

use crate::{
    attributes::clamp_health,
    client::{is_not_paused, leaderboard::auto_fetch_leaderboard_on_menu, load_state, ClientState},
    combat::InvincibilityTimer,
    handle_hits,
    inventory::{try_auto_equip_from_upgrade_slot, Inventory},
    item::{
        active_skill_shrine::ActiveSkillShrineOverwrite,
        heirloom_shrine::handle_heirloom_shrine_ui_setup, item_actions::ActionSuccessEvent,
    },
    player::skills::HeirloomChoiceQueue,
    player::unlocks::RunUnlockState,
    player::RunScore,
    proto::proto_param::ProtoParam,
    CustomFlush, Game, GameState, Player, ScreenResolution, DEBUG,
};

use self::{
    crafting_ui::{change_ui_state_to_crafting_when_resource_added, CraftingContainer},
    damage_numbers::{
        add_previous_health, handle_add_damage_numbers_after_hit, handle_add_dodge_text,
        handle_queued_floating_texts, tick_damage_numbers, DodgeEvent,
    },
    minimap::MinimapPlugin,
    tile_hover::spawn_tile_hover_on_cursor_move,
};

/// Full inventory panel sprite size (match background art).
pub const INVENTORY_UI_SIZE: Vec2 = Vec2::new(162., 312.);
pub const INVENTORY_UPGRADE_UI_SIZE: Vec2 = Vec2::new(134., 78.);
/// Side panel art shown in `UIState::InventoryCrafting` in place of the upgrade panel.
pub const INVENTORY_CRAFTING_PANEL_UI_SIZE: Vec2 = Vec2::new(126., 184.);
pub const INVENTORY_EQUIPMENT_UI_SIZE: Vec2 = Vec2::new(130., 140.);
/// Side panel that replaces the stats tooltip in `UIState::InventoryCrafting`.
/// Matches the art height of the stats panel so it occupies the same slot on-screen.
pub const INVENTORY_BLUEPRINT_UI_SIZE: Vec2 = Vec2::new(192., 312.);
pub const INVENTORY_Y_OFFSET: f32 = -6.;

/// Horizontal center of the main inventory panel; shifts left when side stat trackers are shown.
#[inline]
pub fn inventory_panel_center_x(damage_tracker_visible: bool) -> f32 {
    if damage_tracker_visible {
        INVENTORY_PANEL_CENTER_X
    } else {
        INVENTORY_PANEL_CENTER_X_COMPACT
    }
}
/// Pixel extent of the main item slot grid (4 columns × 7 rows).
pub const INVENTORY_GRID_COLS: usize = 4;
pub const SKILLS_CHOICE_UI_SIZE: Vec2 = Vec2::new(164., 192.);
pub const ESSENCE_UI_SIZE: Vec2 = Vec2::new(157., 130.5);
pub const TOOLTIP_UI_SIZE: Vec2 = Vec2::new(172., 312.);
pub const CHEST_INVENTORY_UI_SIZE: Vec2 = Vec2::new(127., 142.);
pub const CRAFTING_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const FURNACE_INVENTORY_UI_SIZE: Vec2 = Vec2::new(171., 166.);
pub const UI_SLOT_SIZE: Vec2 = Vec2::new(24., 24.);
pub const UI_UPGRADE_SLOT_SIZE: Vec2 = Vec2::new(34., 34.);

// --- Main inventory item grid (panel local space; parent = inventory sprite center) ---
/// Horizontal / vertical distance between slot **centers** in the main grid.
pub const INV_SLOT_SPACING_X: f32 = UI_SLOT_SIZE.x + 6.;
pub const INV_SLOT_SPACING_Y: f32 = UI_SLOT_SIZE.y + 6.;
/// Inset from the panel’s left edge (−half width) to the **left** edge of column 0.
pub const INV_GRID_INSET_LEFT: f32 = 26.0;
/// Inset from the panel’s bottom edge (−half height) to the **bottom** edge of row 0.
pub const INV_GRID_INSET_BOTTOM: f32 = 49.0;
pub const INV_GRID_FIRST_ROW_NUDGE_Y: f32 = -29.0;

/// After computing the same grid as `Normal`, chest/scrapper UIs shift the stack down (shared panel layout).
pub const INV_CHEST_SCRAPPER_GRID_OFFSET_Y: f32 = 4.0 * INV_SLOT_SPACING_Y + 11.0;

// --- Equipment panel 3×3 grid (panel-local; aligned to built-in slot art in `EquipmentPanel.png`) ---
/// Offset from the inventory panel center to the equipment panel center (kept in sync with the
/// `equip_panel` sprite spawn in `setup_inv_ui`).
pub const INV_EQUIP_PANEL_OFFSET_X: f32 = 147.0;
pub const INV_EQUIP_PANEL_OFFSET_Y: f32 = 86.0;
/// Spacing between 3×3 grid columns / rows (centers).
pub const INV_EQUIP_GRID_SPACING: f32 = 30.0;
/// Panel-local y of the three grid rows (top, middle, bottom).
pub const INV_EQUIP_GRID_ROW_TOP_Y: f32 = 26.0;
pub const INV_EQUIP_GRID_ROW_MID_Y: f32 = -4.0;
pub const INV_EQUIP_GRID_ROW_BOT_Y: f32 = -34.0;

/// Trash slot (bottom of panel).
pub const INV_TRASH_OFFSET_X: f32 = -20.0 + 0.5 * UI_SLOT_SIZE.x;
pub const INV_TRASH_OFFSET_Y: f32 = -0.5 * UI_SLOT_SIZE.y - 12.0;

/// Sort button — sits directly under the trash slot on the left edge of the inventory panel.
/// Shares the x-position of the trash slot so the two slot-sized tiles line up vertically.
pub const INV_SORT_BUTTON_OFFSET_X: f32 = INV_TRASH_OFFSET_X;
pub const INV_SORT_BUTTON_OFFSET_Y: f32 = INV_TRASH_OFFSET_Y - UI_SLOT_SIZE.y - 6.0;

/// Material-drops toggle — directly under the sort button, same x as trash/sort column.
pub const INV_MATERIAL_DROPS_TOGGLE_OFFSET_X: f32 = INV_SORT_BUTTON_OFFSET_X;
pub const INV_MATERIAL_DROPS_TOGGLE_OFFSET_Y: f32 = INV_SORT_BUTTON_OFFSET_Y - UI_SLOT_SIZE.y - 6.0;

/// Damage-tracker toggle — directly under the material-drops filter button.
pub const INV_DAMAGE_TRACKER_TOGGLE_OFFSET_X: f32 = INV_MATERIAL_DROPS_TOGGLE_OFFSET_X;
pub const INV_DAMAGE_TRACKER_TOGGLE_OFFSET_Y: f32 =
    INV_MATERIAL_DROPS_TOGGLE_OFFSET_Y - UI_SLOT_SIZE.y - 6.0;

/// Main inventory panel center X when damage/mob stat side panels are visible.
pub const INVENTORY_PANEL_CENTER_X: f32 = -193.;
/// Shift right when side stat panels are hidden (more centered on screen).
pub const INVENTORY_PANEL_CENTER_X_COMPACT: f32 = INVENTORY_PANEL_CENTER_X + 45.;

/// Padding around the combined damage + mob stat tracker background panel.
pub const INV_SIDE_STATS_BG_PADDING: f32 = 10.0;
/// Black backdrop opacity behind inventory side stat trackers.
pub const INV_SIDE_STATS_BG_ALPHA: f32 = 0.85;

/// Drop-filter side panel (right of inventory panel).
pub const INV_DROP_FILTER_PANEL_GAP: f32 = 6.0;
pub const INV_DROP_FILTER_PANEL_PADDING: f32 = 6.0;
pub const INV_DROP_FILTER_PANEL_COLS: usize = 8;
pub const INV_DROP_FILTER_ICON_SIZE: f32 = 16.0;
pub const INV_DROP_FILTER_ICON_GAP: f32 = 2.0;
pub const INV_DROP_FILTER_TITLE_ROW_HEIGHT: f32 = 14.0;
pub const INV_DROP_FILTER_BUTTON_ROW_HEIGHT: f32 = 14.0;
pub const INV_DROP_FILTER_BUTTON_SPACING: f32 = 36.0;
/// Drop-filter panel Z; sits above inventory chrome (panel Z = 10) and slot icons (Z ≈ 12-14).
pub const INV_DROP_FILTER_PANEL_Z: f32 = 60.0;

/// Crafting grid inside crafting/furnace-style panels (8 columns).
pub const INV_CRAFTING_COLS: usize = 8;
pub const INV_CRAFTING_ROW_GAP: f32 = 1.0;
/// Added to −half width for crafting column 0 center: `+ INV_CRAFTING_X_ANCHOR + 0.5 * UI_SLOT_SIZE.x`.
pub const INV_CRAFTING_X_ANCHOR: f32 = 6.0;
/// Row 0 baseline: `−half_height + 7 * slot_y + 16`.
pub const INV_CRAFTING_BASE_Y: f32 = 7.0 * UI_SLOT_SIZE.y - 160.0;
/// Nudge when `UIState::Inventory` (crafting strip on main inventory screen).
pub const INV_CRAFTING_NUDGE_IN_MAIN_INV: Vec2 = Vec2::new(-2., -29.);

/// Furnace special slots (panel-local).
pub const INV_FURNACE_SLOT_1: Vec2 = Vec2::new(121., -46.);
pub const INV_FURNACE_SLOT_0: Vec2 = Vec2::new(176., -46.);

/// Number of hotbar slots shown in the on-screen HUD (bound to keys 1-4).
/// The underlying `Inventory::items` container still has slots 4-5 for passive storage,
/// but they are not rendered in the HUD.
pub const HUD_HOTBAR_SLOTS: usize = 4;

/// Y offset from the screen bottom for the `HudBar` frame sprite.
pub const HUD_FRAME_Y_FROM_BOTTOM: f32 = 30.0;

/// Shared y (offset from the screen bottom) for the hotbar + class-skill icon row.
pub const HUD_ACTION_ROW_Y_FROM_BOTTOM: f32 = 19.0;

/// HUD keybind badge draw size (solid grey box, no sprite asset).
pub const KEYBIND_BADGE_SIZE: Vec2 = Vec2::new(19., 9.);

/// Grey keybind badge fill (`KEYBIND_BADGE_SIZE`).
pub const KEYBIND_BADGE_COLOR: Color = Color::rgba(62./255., 58./255., 58./255., 0.85);

/// Gap between the bottom screen edge and the bottom of a HUD keybind badge.
pub const KEYBIND_BADGE_BOTTOM_INSET: f32 = 1.0;

/// World-space y for the center of a bottom-anchored HUD keybind badge.
pub fn hud_keybind_badge_center_y(game_height: f32) -> f32 {
    -game_height * 0.5 + KEYBIND_BADGE_BOTTOM_INSET + KEYBIND_BADGE_SIZE.y * 0.5
}

/// World-space x for hotbar slot `slot_index` (matches [`inventory_ui::inv_slot_local_position`]).
pub fn hud_hotbar_slot_center_x(slot_index: usize) -> f32 {
    let half_span = (HUD_HOTBAR_SLOTS as f32 - 1.0) * 0.5;
    HUD_HOTBAR_CENTER_X + (slot_index as f32 - half_span) * (INV_SLOT_SPACING_X - 5.0)
}

/// Center x of the 4-slot hotbar group (left side of the action row).
pub const HUD_HOTBAR_CENTER_X: f32 = -59.0;

/// Center x of the class-skill icon group (right side of the action row).
pub const HUD_SKILLS_CENTER_X: f32 = 60.0;

/// Center-to-center spacing between class-skill icons.
pub const HUD_SKILL_SPACING_X: f32 = 25.0;

/// Invisible hit area for HUD skill-slot drag-and-drop (matches the former 20×20 slot bg).
pub const HUD_SKILL_SLOT_HIT_SIZE: Vec2 = Vec2::new(20., 20.);

/// `assets/ui/ProgressBackground.png` draw size (compact HUD: score + chaos only).
pub const PROGRESS_BACKGROUND_SIZE: Vec2 = Vec2::new(102., 24.);

/// `assets/ui/TooltipInfoBox.png` draw size (side glossary boxes beside tooltips).
pub const TOOLTIP_INFO_BOX_SIZE: Vec2 = Vec2::new(158., 30.);

/// `assets/ui/CurrencyBackground.png` draw size.
pub const CURRENCY_BACKGROUND_SIZE: Vec2 = Vec2::new(64., 26.);

/// Gap between the two currency background sprites in the HUD row below the XP bar.
pub const HUD_CURRENCY_BACKGROUND_GAP: f32 = 10.;

/// Gap between the coin currency background and the progress (score/chaos) bar.
pub const HUD_PROGRESS_AFTER_CURRENCY_GAP: f32 = 10.;

/// Gap between the progress bar and the era timeline.
pub const HUD_TIMELINE_AFTER_PROGRESS_GAP: f32 = 4.0;

/// Inset from the left screen edge to the first currency background center.
pub const HUD_CURRENCY_LEFT_PADDING: f32 = 4.0;

/// World-space Y for the HUD row (currency backgrounds + progress bar) sitting just under the XP bar.
pub fn hud_row_below_xp_y(game_height: f32) -> f32 {
    // XP bar center ≈ `game_height/2 - 3`, height 6 → bottom at `gh/2 - 6`; leave ~8px gap then
    // center the 30px-tall progress / 23px-tall currency art on that band.
    game_height * 0.5 - 20.
}

/// Endless-mode era timer background width (matches `handle_update_era_timer_hud`).
pub const HUD_ERA_TIMER_ENDLESS_WIDTH: f32 = 68.0;

/// `assets/ui/Timeline.png` draw size.
pub const HUD_TIMELINE_SIZE: Vec2 = Vec2::new(174., 18.);

/// `assets/ui/TimelineArrows.png` draw size.
pub const HUD_TIMELINE_ARROWS_SIZE: Vec2 = Vec2::new(10., 22.);

/// Local X for timeline arrows: `0` = left edge, `1` = right edge of the bar.
pub fn hud_timeline_arrow_local_x(progress: f32) -> f32 {
    (progress.clamp(0., 1.) - 0.5) * (HUD_TIMELINE_SIZE.x -2.0)
}

/// First (time fragment) currency background center X from the left screen edge.
pub fn hud_currency_first_center_x(game_width: f32) -> f32 {
    -game_width * 0.5
        + CURRENCY_BACKGROUND_SIZE.x * 0.5
        + HUD_CURRENCY_LEFT_PADDING
}

/// Second (coin) currency background center X.
pub fn hud_currency_second_center_x(game_width: f32) -> f32 {
    hud_currency_first_center_x(game_width) + CURRENCY_BACKGROUND_SIZE.x + HUD_CURRENCY_BACKGROUND_GAP
}

/// Era timeline center X — anchored to the screen center so the timeline sits in the middle
/// of the HUD row regardless of resolution.
pub fn hud_timeline_center_x(_game_width: f32) -> f32 {
    0.0
}

/// Right edge X of the second (coin) currency background — the right boundary of the
/// left-aligned currency block.
pub fn hud_currency_block_right_edge_x(game_width: f32) -> f32 {
    hud_currency_second_center_x(game_width) + CURRENCY_BACKGROUND_SIZE.x * 0.5
}

/// Symmetric padding between the centered timeline and the score/currency blocks on either
/// side. Computed as the gap between the right edge of the currency block and the timeline's
/// left edge (clamped to a sensible minimum so things don't collide on tiny windows).
pub fn hud_timeline_side_padding(game_width: f32) -> f32 {
    let timeline_left_edge = hud_timeline_center_x(game_width) - HUD_TIMELINE_SIZE.x * 0.5;
    let gap = timeline_left_edge - hud_currency_block_right_edge_x(game_width);
    gap.max(HUD_PROGRESS_AFTER_CURRENCY_GAP)
}

/// Score / chaos progress bar center X — placed to the right of the centered timeline using the
/// same padding as the gap between the currency block and the timeline on the left side.
pub fn hud_progress_bar_center_x(res: &ScreenResolution) -> f32 {
    let timeline_right_edge =
        hud_timeline_center_x(res.game_width) + HUD_TIMELINE_SIZE.x * 0.5;
    let minimap_left_edge = minimap::hud_minimap_left_edge_x_from_res(res);
    (timeline_right_edge + minimap_left_edge) * 0.5
}

/// Endless-mode timer center X, flush against the timeline's right edge.
pub fn hud_era_timer_center_x(game_width: f32, timer_width: f32) -> f32 {
    hud_timeline_center_x(game_width) - HUD_TIMELINE_SIZE.x * 0.5 - timer_width * 0.5
}

/// Gap between the bottom of the progress bar row and the heirloom icon row.
pub const HUD_HEIRLOOM_ROW_GAP_BELOW_PROGRESS: f32 = 8.0;

/// Heirloom HUD icon half-size (sprites are 16×16).
pub const HUD_HEIRLOOM_ICON_HALF: f32 = 8.0;

pub const HUD_HEIRLOOM_ICON_SPACING: f32 = 16.0;

/// Extra inset from the left screen edge to the first heirloom icon center (beyond icon half-width).
pub const HUD_HEIRLOOM_LEFT_PADDING: f32 = 4.0;

/// Nudge the heirloom row upward from its default position below the progress bar.
pub const HUD_HEIRLOOM_ROW_Y_NUDGE: f32 = 9.0;

/// World-space Y for the heirloom icon row (below the progress / currency HUD row).
pub fn hud_heirloom_row_y(game_height: f32) -> f32 {
    hud_row_below_xp_y(game_height)
        - PROGRESS_BACKGROUND_SIZE.y * 0.5
        - HUD_HEIRLOOM_ROW_GAP_BELOW_PROGRESS
        - HUD_HEIRLOOM_ICON_HALF
        + HUD_HEIRLOOM_ROW_Y_NUDGE
}

/// First heirloom icon center X (near the left screen edge).
pub fn hud_heirloom_first_icon_x(game_width: f32) -> f32 {
    -game_width * 0.5 + HUD_HEIRLOOM_ICON_HALF + HUD_HEIRLOOM_LEFT_PADDING
}

/// Padding kept clear between the right-most heirloom icon and the left edge of the HUD minimap
/// so heirlooms never tuck under the minimap circle.
pub const HUD_HEIRLOOM_MINIMAP_PADDING: f32 = 6.0;

/// Largest number of heirloom icons that fit on a single HUD row at the current screen width,
/// leaving [`HUD_HEIRLOOM_MINIMAP_PADDING`] of clearance to the left of the minimap.
pub fn hud_heirloom_max_per_row(res: &ScreenResolution) -> usize {
    let first_x = hud_heirloom_first_icon_x(res.game_width);
    let minimap_left_edge = minimap::hud_minimap_left_edge_x_from_res(res);
    let max_center_x =
        minimap_left_edge - HUD_HEIRLOOM_MINIMAP_PADDING - HUD_HEIRLOOM_ICON_HALF;
    if max_center_x <= first_x {
        return 1;
    }
    let usable = max_center_x - first_x;
    1 + (usable / HUD_HEIRLOOM_ICON_SPACING).floor() as usize
}

/// Parent offset when the inventory UI is in crafting mode (whole panel nudge).
pub const INV_UI_PARENT_OFFSET_CRAFTING: Vec2 = Vec2::new(0., -4.);

// --- Upgrade / Crafting side panel layout overrides for `UIState::InventoryCrafting` ---
/// Inventory-panel-local Y of the upgrade panel when the CRAFT toggle swaps us into crafting mode.
/// Moves the panel up from the default `-75` so the three crafting-input slots line up mid-inventory.
pub const INV_UPGRADE_PANEL_OFFSET_Y_CRAFTING: f32 = 10.0;
/// Horizontal spacing (centers) of the three crafting input slots in `InventoryCrafting` mode.
pub const INV_CRAFTING_INPUT_SLOT_SPACING_X: f32 = INV_SLOT_SPACING_X;
/// Inventory-panel-local Y of the three crafting input slots in `InventoryCrafting` mode
/// (aligns them slightly above the crafting panel's vertical center, matching the reference mock).
pub const INV_CRAFTING_INPUT_SLOTS_Y_LOCAL: f32 = INV_UPGRADE_PANEL_OFFSET_Y_CRAFTING + 20.0;

// --- Blueprint row + crafting panel slot layout (InventoryCrafting mode) ---
/// A single blueprint row on the blueprints panel. Matches the art sprite size.
pub const INV_BLUEPRINT_SLOT_SIZE: Vec2 = Vec2::new(144., 22.);
/// Vertical gap between blueprint rows (edge-to-edge).
pub const INV_BLUEPRINT_SLOT_ROW_GAP: f32 = 4.0;
/// Panel-local Y of the top-most blueprint row (row 0 center) relative to the blueprints panel center.
pub const INV_BLUEPRINT_SLOT_TOP_Y: f32 = INVENTORY_BLUEPRINT_UI_SIZE.y * 0.5 - 45.0;
/// Panel-local X anchor for blueprint rows (centered under the panel title).
pub const INV_BLUEPRINT_SLOT_CENTER_X: f32 = -3.0;
/// Max number of blueprint rows that fit inside the blueprints panel art.
pub const MAX_BLUEPRINT_ROWS: usize = 10;
/// Pagination buttons (`ButtonPageUp` / `ButtonPageDown` art). Matches `assets/ui/ButtonPageUp.png`.
pub const BLUEPRINT_PAGE_BTN_SIZE: Vec2 = Vec2::new(19., 14.);
/// Gap from the panel bottom edge to the button centers.
pub const BLUEPRINT_PAGE_BTN_BOTTOM_PAD: f32 = 8.0;
/// Panel-local **center** Y for both pagination buttons (sits along the bottom of the blueprints panel).
pub const BLUEPRINT_PAGE_BTN_CENTER_Y: f32 = -INVENTORY_BLUEPRINT_UI_SIZE.y * 0.5
    + BLUEPRINT_PAGE_BTN_SIZE.y * 0.5
    + BLUEPRINT_PAGE_BTN_BOTTOM_PAD;
/// Panel-local **center** X for the page-up control (left half of the bottom band).
pub const BLUEPRINT_PAGE_BTN_UP_X: f32 = -INVENTORY_BLUEPRINT_UI_SIZE.x * 0.1;
/// Panel-local **center** X for the page-down control (right half of the bottom band).
pub const BLUEPRINT_PAGE_BTN_DOWN_X: f32 = INVENTORY_BLUEPRINT_UI_SIZE.x * 0.1;
/// X offset (from the row's left edge) where the recipe result icon is centered.
pub const INV_BLUEPRINT_SLOT_ICON_X_OFFSET: f32 = 12.0;
/// X offset (from the row's left edge) where the recipe name label starts (anchored left).
pub const INV_BLUEPRINT_SLOT_LABEL_X_OFFSET: f32 = 36.0;

/// Panel-local Y (crafting panel) of the three ingredient display slots.
pub const INV_CRAFTING_PANEL_INGREDIENT_ROW_Y: f32 = -6.0;
/// Horizontal center-to-center spacing of the three ingredient display slots on the crafting panel.
pub const INV_CRAFTING_PANEL_INGREDIENT_SPACING_X: f32 = 32.;
/// Panel-local Y (crafting panel) of the result slot (sits above the ingredient row).
pub const INV_CRAFTING_PANEL_RESULT_Y: f32 = 41.0;
/// Amount text offset beneath each ingredient icon (e.g. "2/3").
pub const INV_CRAFTING_PANEL_INGREDIENT_COUNT_Y_OFFSET: f32 = -11.0;

/// Reset the blueprints panel page to 0 whenever the player (re-)opens the inventory in
/// `InventoryCrafting` mode so the panel always boots at the first page of recipes.
pub fn reset_blueprints_pagination_on_open(
    mut pagination: ResMut<crate::ui::inventory_ui::BlueprintsPagination>,
) {
    pagination.page = 0;
}

pub(crate) fn snap_world_to_pixel_grid(value: f32, scale: u32) -> f32 {
    let s = scale as f32;
    (value * s).round() / s
}

/// Pixel-snap all layer-3 UI visuals (text + sprites) to the current integer render scale.
/// This keeps glyphs/icons aligned to physical pixels on HiDPI screens.
pub(crate) fn snap_layer3_visuals_to_pixel_grid(
    resolution: Res<ScreenResolution>,
    mut ui_visuals: Query<
        (&mut Transform, &RenderLayers),
        (Or<(With<Text>, With<Sprite>, With<TextureAtlasSprite>)>, Without<Camera>),
    >,
) {
    for (mut transform, layers) in ui_visuals.iter_mut() {
        if !layers.intersects(&RenderLayers::layer(3)) {
            continue;
        }
        transform.translation.x = snap_world_to_pixel_grid(transform.translation.x, resolution.scale);
        transform.translation.y = snap_world_to_pixel_grid(transform.translation.y, resolution.scale);
    }
}

pub struct UIPlugin;
//TODO: extract out ui darken overlay into a helper function
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_state::<UIState>()
            .insert_resource(InventoryState::default())
            .init_resource::<SelectedCraftingRecipe>()
            .init_resource::<crate::ui::inventory_ui::BlueprintsPagination>()
            .init_resource::<crate::inventory::MaterialDropFilterMenuOpen>()
            .init_resource::<crate::inventory::DamageTrackerMenuOpen>()
            .insert_resource(ClassSelectionState::default())
            .init_resource::<ClassUnlockHoverState>()
            .init_resource::<ClassUnlockConfirmState>()
            .init_resource::<SkillUnlockConfirmState>()
            .insert_resource(CheatSettings::load_from_game_data())
            .insert_resource(RunUnlockState::default())
            .init_resource::<AchievementsPagination>()
            .init_resource::<MicrowaveShrineUsages>()
            .insert_resource(crate::keybinds::InputMappings::load())
            .init_resource::<CurrentNameInput>()
            .init_resource::<ActiveSkillDragState>()
            .init_resource::<CursorBlinkTimer>()
            .insert_resource(FloatingTextQueue::new(0.8))
            .insert_resource(TooltipsManager {
                timer: Timer::from_seconds(0.7, TimerMode::Once),
                stats_respawn_delay: None,
            })
            .add_event::<ActionSuccessEvent>()
            .add_event::<ScrapperEvent>()
            .add_event::<FlashExpBarEvent>()
            .add_event::<ItemChestAnimChangeEvent>()
            .add_event::<DropOnSlotEvent>()
            .add_event::<DodgeEvent>()
            .add_event::<GlobalTextMessageEvent>()
            .add_event::<RemoveFromSlotEvent>()
            .add_event::<ToolTipUpdateEvent>()
            .init_resource::<BeaconGuidanceRegistry>()
            .init_resource::<BlacksmithPurchaseTracker>()
            .init_resource::<EssenceShopCache>()
            .init_resource::<MerchantShopUiDirty>()
            .init_resource::<TimeCrystalsHeirloomGridOpen>()
            .init_resource::<DevHeirloomGridOpen>()
            .init_resource::<DevSkillGridOpen>()
            .init_resource::<SelectedBeastiaryMob>()
            .add_event::<TooltipTeardownEvent>()
            .add_event::<ShowInvPlayerStatsEvent>()
            .add_event::<DamageTrackerRefreshEvent>()
            .add_event::<SubmitMerchantPurchase>()
            .add_event::<MerchantCategoryRerollEvent>()
            .add_event::<DropInWorldEvent>()
            .add_event::<MenuButtonClickEvent>()
            .add_event::<GrantHeirloomDevEvent>()
            .add_event::<RevokeHeirloomDevEvent>()
            .add_event::<GrantSkillDevEvent>()
            .add_event::<HeirloomTooltipRequest>()
            .add_plugin(Material2dPlugin::<ScreenEffectMaterial>::default())
            .add_plugin(Material2dPlugin::<ui_helpers::RadialOverlayMaterial>::default())
            .init_resource::<ui_helpers::RadialOverlayMeshCache>()
            .init_resource::<ui_helpers::RadialOverlayMaterialCache>()
            .init_resource::<OptionsUiLayoutRevision>()
            .init_resource::<layout_sync::UiLayoutSyncState>()
            .add_plugin(hud_bar_fill::HudBarFillPlugin)
            .register_type::<InventorySlotState>()
            .add_plugin(MinimapPlugin)
            .add_plugin(map_markers::MapMarkerPlugin)
            .add_plugin(TipPlugin)
            .add_plugin(IntroGuidePlugin)
            .add_plugin(tutorial_ui::TutorialPlugin)
            .add_system(setup_loading_screen.in_schedule(OnEnter(GameState::Initializing)))
            .add_system(
                check_initialization_complete
                    .run_if(in_state(GameState::Initializing)),
            )
            .add_system(spawn_fps_text.run_if(run_once_per_run()).in_schedule(OnEnter(GameState::Main)))
            .add_system(
                phase1_fps_text_viewport_diag
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(
                fps_text::phase2_fps_text_layout_diag
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(
                fps_text::phase3_fps_atlas_dump
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(
                layout_sync::bump_options_ui_revision_on_ui_layout_change
                    .after(crate::update_pixel_perfect_viewport),
            )
            .add_system(
                player_hud::sync_player_hud_layout_to_resolution
                    .after(crate::update_pixel_perfect_viewport),
            )
            .add_system(
                player_hud::sync_player_hud_progress_layout_to_resolution
                    .after(player_hud::sync_player_hud_layout_to_resolution),
            )
            .add_system(
                player_hud::sync_player_hud_slots_layout_to_resolution
                    .after(player_hud::sync_player_hud_progress_layout_to_resolution),
            )
            .add_system(
                minimap::sync_hud_minimap_layout_to_resolution
                    .after(player_hud::sync_player_hud_slots_layout_to_resolution),
            )
            .add_system(
                layout_sync::commit_ui_layout_sync_system
                    .after(minimap::sync_hud_minimap_layout_to_resolution),
            )
            .add_system(
                snap_layer3_visuals_to_pixel_grid
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(not(in_state(GameState::Initializing))),
            )
            .add_system(
                font_binarize::binarize_font_atlas_alpha
                    .in_base_set(CoreSet::PostUpdate)
                    .after(bevy::text::update_text2d_layout),
            )
            .add_system(
                text_pixel_snap::pixel_snap_text_glyphs
                    .in_base_set(CoreSet::PostUpdate)
                    .after(bevy::text::update_text2d_layout),
            )
            .add_system(
                setup_leaderboard_ui
                    .in_schedule(OnEnter(GameState::MainMenu))
                    .after(auto_fetch_leaderboard_on_menu)  // Ensure fetch happens first
            )
            .add_system(cleanup_leaderboard_ui.in_schedule(OnExit(GameState::MainMenu)))
            .add_system(reset_blacksmith_tracker.in_schedule(OnEnter(GameState::MainMenu)))
            .add_system(reset_microwave_shrine_usages.in_schedule(OnEnter(GameState::MainMenu)))
            .add_system(reset_break_drop_filter.in_schedule(OnEnter(GameState::MainMenu)))
            .add_system(cleanup_run_state.in_base_set(CoreSet::PreUpdate).run_if(not(in_state(GameState::MainMenu))))
            .add_systems((
                // Clean up leaderboard when entering other UI states to avoid duplicates
                cleanup_leaderboard_ui
                    .run_if(in_state(GameState::MainMenu)
                        .and_then(state_changed::<UIState>())
                        .and_then(not(in_state(UIState::Closed)))),
                // Recreate leaderboard when returning to main menu view (but not on initial entry)
                setup_leaderboard_ui
                    .after(cleanup_leaderboard_ui)
                    .run_if(in_state(GameState::MainMenu)
                        .and_then(state_changed::<UIState>())
                        .and_then(in_state(UIState::Closed))),
            ))
            .add_systems((
                update_leaderboard_display
                    .run_if(in_state(GameState::MainMenu))
                    .run_if(in_state(UIState::Closed)),
                ensure_leaderboard_entries
                    .run_if(in_state(GameState::MainMenu))
                    .run_if(in_state(UIState::Closed)),
            ))
            .add_systems((
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Inventory))),
                cleanup_dev_heirloom_grid_on_inv_close
                    .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Inventory)))),
                cleanup_dev_skill_grid_on_inv_close
                    .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Inventory)))),
                reset_blueprints_pagination_on_open
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::InventoryCrafting))),
                setup_inv_ui
                    .before(CustomFlush)
                    .after(reset_blueprints_pagination_on_open)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::InventoryCrafting))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Chest))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Scrapper))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Crafting))),
                setup_inv_ui
                    .before(CustomFlush)
                    .run_if(state_changed::<UIState>().and_then(in_state(UIState::Furnace))),
            ))
            // Check for pending level-up rewards when any menu closes during gameplay
            .add_system(
                check_pending_levelup_rewards_on_menu_close
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(in_state(GameState::Main))
                    .run_if(state_changed::<UIState>())
                    .run_if(in_state(UIState::Closed)),
            )
            .add_system(
                grant_iframes_after_chest_or_levelup_ui_close
                    .in_set(OnUpdate(GameState::Main))
                    .before(handle_hits)
                    .run_if(in_state(GameState::Main))
                    .run_if(state_changed::<UIState>())
                    .after(check_pending_levelup_rewards_on_menu_close),
            )
            .add_systems(
                (
                    setup_bars_ui.after(load_state).run_if(run_once_per_run()),
                    setup_hotbar_hud
                        .after(setup_bars_ui)
                        .run_if(run_once_per_run()),
                    setup_xp_bar_ui.after(load_state).run_if(run_once_per_run()),
                    setup_currency_ui.run_if(run_once_per_run()),
                    setup_timeline_hud.run_if(run_once_per_run()),
                    setup_era_timer_hud.run_if(run_once_per_run()),
                    setup_chaos_ui.run_if(run_once_per_run()),
                )
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                setup_screen_effects
                    .in_set(OnUpdate(GameState::Main))
                    .before(handle_screen_effects),
            )
            .add_system(handle_screen_effects.in_set(OnUpdate(GameState::Main)))
            .add_system(ui_helpers::attach_radial_overlay_visuals.before(CustomFlush))
            .add_systems(
                (
                    add_previous_health,
                    handle_flash_bars,
                    update_xp_bar_rainbow.before(update_xp_bar),
                    update_xp_bar,
                    player_hud::drain_pending_xp.after(update_xp_bar),
                    update_decorative_xp_shards.after(update_xp_bar_rainbow),
                    handle_skill_choice_ui_close.after(update_xp_bar),
                    handle_enemy_health_bar_change,
                    add_ui_icon_for_elite_mobs,
                    spawn_player_movement_cooldown_bar,
                    handle_add_dodge_text,
                    boss_health_bar::spawn_boss_health_bar,
                    boss_health_bar::update_boss_health_bar,
                    boss_health_bar::cleanup_boss_health_bar_on_despawn,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((
                show_pending_era_announcement
                    .run_if(in_state(GameState::Main))
                    .run_if(resource_exists::<global_text_message::PendingEraAnnouncement>())
                    .before(handle_global_text_message_events),
                handle_queued_floating_texts
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
                tick_damage_numbers
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
                handle_global_text_message_events
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
                tick_global_text_messages
                    .run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
            ))
            .add_system(
                handle_add_damage_numbers_after_hit
                    .before(handle_hits)
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(resource_exists::<Game>()),
            )
            .add_systems(
                (
                    handle_item_drop_clicks,
                    handle_drop_dragged_items_on_inv_close,
                    handle_dragging,
                    update_dragged_item_stack_count_text,
                    handle_drop_on_slot_events.after(handle_item_drop_clicks),
                    handle_drop_in_world_events.after(handle_item_drop_clicks),
                    handle_interaction_clicks
                        .before(handle_item_drop_clicks)
                        .run_if(not(in_state(UIState::Closed))),
                    upgrade_drag::handle_drag_upgrade_material_on_equipment
                        .before(handle_item_drop_clicks)
                        .run_if(not(in_state(UIState::Closed))),
                    handle_icon_hover_tooltips
                        .after(handle_interaction_clicks)
                        .after(handle_sort_inventory_button_click)
                        .after(handle_material_drops_toggle_button_click)
                        .after(handle_material_drop_filter_menu_click)
                        .run_if(in_state(GameState::Main)),
                    handle_spawn_inv_item_tooltip,
                    update_inventory_ui.after(CustomFlush),
                    handle_update_inv_item_entities,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    change_ui_state_to_chest_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ChestContainer>()),
                    change_ui_state_to_scrapper_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<ScrapperContainer>()),
                    handle_scrap_items_in_scrapper.run_if(in_state(UIState::Scrapper)),
                    text_update_system,
                    add_inv_to_new_scrapper_objs,
                    add_container_to_new_furnace_objs,
                    update_healthbar,
                    change_ui_state_to_crafting_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<CraftingContainer>()),
                    change_ui_state_to_furnace_when_resource_added
                        .before(CustomFlush)
                        .run_if(resource_added::<FurnaceContainer>()),
                    handle_update_era_timer_hud.run_if(
                        resource_exists::<crate::night::EraTimer>(),
                    ),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                add_inv_to_new_chest_objs
                    .in_base_set(CoreSet::PreUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_system(handle_spawn_inv_player_stats.in_base_set(CoreSet::PostUpdate))
            .add_system(
                spawn_damage_tracker_in_inventory
                    .in_base_set(CoreSet::PostUpdate),
            )
            .add_systems(
                (
                    setup_unlocks_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Unlocks))),
                    cleanup_unlocks_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Unlocks)))),
                    update_unlocks_currency_text.run_if(in_state(UIState::Unlocks)),
                    refresh_unlock_button_states.run_if(in_state(UIState::Unlocks)),
                    
                    setup_achievements_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Achievements))),
                    cleanup_achievements_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Achievements)))),
                    update_achievements_page_display
                        .after(CustomFlush)
                        .run_if(in_state(UIState::Achievements).and_then(
                            state_changed::<UIState>()
                                .or_else(resource_changed::<AchievementsPagination>())
                                .or_else(resource_changed::<crate::player::achievements::Achievements>())
                        )),
                    update_achievements_navigation_buttons
                        .run_if(in_state(UIState::Achievements)),
                )
                    .in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_systems(
                (
                    cleanup_options_ui.run_if(
                        state_changed::<UIState>()
                            .and_then(not(in_state(UIState::Options)))
                            .or_else(
                                in_state(UIState::Options)
                                    .and_then(resource_changed::<OptionsUiLayoutRevision>()),
                            ),
                    ),
                    setup_options_ui
                        .before(CustomFlush)
                        .run_if(
                            state_changed::<UIState>()
                                .and_then(in_state(UIState::Options))
                                .or_else(
                                    in_state(UIState::Options)
                                        .and_then(resource_changed::<OptionsUiLayoutRevision>()),
                                ),
                        ),
                )
                    .chain()
                    .after(layout_sync::bump_options_ui_revision_on_ui_layout_change)
                    .in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_systems(
                (
                    setup_unlocks_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Unlocks))),
                    cleanup_unlocks_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Unlocks)))),
                   
                    setup_achievements_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Achievements))),
                    cleanup_achievements_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::Achievements)))),
                    update_achievements_page_display
                        .after(CustomFlush)
                        .run_if(in_state(UIState::Achievements).and_then(
                            state_changed::<UIState>()
                                .or_else(resource_changed::<AchievementsPagination>())
                                .or_else(resource_changed::<crate::player::achievements::Achievements>())
                        )),
                    update_achievements_navigation_buttons
                        .run_if(in_state(UIState::Achievements)),
                    
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    cleanup_options_ui.run_if(
                        state_changed::<UIState>()
                            .and_then(not(in_state(UIState::Options)))
                            .or_else(
                                in_state(UIState::Options)
                                    .and_then(resource_changed::<OptionsUiLayoutRevision>()),
                            ),
                    ),
                    setup_options_ui
                        .before(CustomFlush)
                        .run_if(
                            state_changed::<UIState>()
                                .and_then(in_state(UIState::Options))
                                .or_else(
                                    in_state(UIState::Options)
                                        .and_then(resource_changed::<OptionsUiLayoutRevision>()),
                                ),
                        ),
                )
                    .chain()
                    .after(layout_sync::bump_options_ui_revision_on_ui_layout_change)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_name_entry_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::EnterName))),
                    cleanup_name_entry_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::EnterName)))),
                    handle_name_entry_input
                        .run_if(in_state(UIState::EnterName)),
                    update_name_entry_text
                        .run_if(in_state(UIState::EnterName)),
                    update_cursor_blink
                        .run_if(in_state(UIState::EnterName)),
                    handle_name_entry_ok_button
                        .run_if(in_state(UIState::EnterName)),
                    setup_time_crystal_progress_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::TimeCrystalProgress))),
                    cleanup_time_crystal_progress_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::TimeCrystalProgress)))),
                    handle_time_crystal_progress_ok_button
                        .run_if(in_state(UIState::TimeCrystalProgress)),
                    handle_time_crystal_unlock_hover_tooltip
                        .run_if(
                            in_state(UIState::TimeCrystalProgress)
                                .or_else(in_state(UIState::TimeCrystalsBrowser)),
                        ),
                    setup_time_crystals_browser_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::TimeCrystalsBrowser))),
                    cleanup_time_crystals_browser_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::TimeCrystalsBrowser)))),
                    handle_time_crystals_browser_done_button
                        .run_if(in_state(UIState::TimeCrystalsBrowser)),
                    handle_time_crystals_view_heirlooms_button
                        .run_if(in_state(UIState::TimeCrystalsBrowser)),
                )
                    .in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_systems(
                (
                    setup_beastiary_browser_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::BeastiaryBrowser))),
                    cleanup_beastiary_browser_ui
                        .run_if(state_changed::<UIState>().and_then(not(in_state(UIState::BeastiaryBrowser)))),
                    handle_beastiary_browser_done_button
                        .run_if(in_state(UIState::BeastiaryBrowser)),
                    handle_beastiary_card_click
                        .run_if(in_state(UIState::BeastiaryBrowser)),
                    crate::ui::beastiary_browser_ui::animate_beastiary_previews
                        .run_if(in_state(UIState::BeastiaryBrowser)),
                )
                    .in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_system(
                check_show_time_crystal_progress_popup
                    .after(crate::ui::check_show_name_entry_popup)
                    .in_schedule(OnEnter(GameState::MainMenu)),
            )
            .add_systems((
                    handle_unlocks_clicks.run_if(in_state(UIState::Unlocks)),
                    update_unlocks_currency_text.run_if(in_state(UIState::Unlocks)),
                    refresh_unlock_button_states.run_if(in_state(UIState::Unlocks)),
                    handle_options_clicks.run_if(in_state(UIState::Options)),
                    handle_key_rebind_input.run_if(in_state(UIState::Options)),
                    update_keybind_text
                        .run_if(in_state(UIState::Options))
                        .after(handle_key_rebind_input),
                    handle_cheat_checkbox_click.run_if(in_state(UIState::Options)),
                    update_cheat_checkbox_visual.run_if(in_state(UIState::Options)),
                    handle_volume_button_click.run_if(in_state(UIState::Options)),
                    update_volume_text.run_if(in_state(UIState::Options)),
                    handle_scale_button_click
                        .run_if(in_state(UIState::Options))
                        .before(crate::update_pixel_perfect_viewport),
                    update_scale_text.run_if(in_state(UIState::Options)),
                    handle_cursor_color_button_click.run_if(in_state(UIState::Options)),
                    update_cursor_color_preview.run_if(in_state(UIState::Options)),
                    handle_achievement_row_clicks.run_if(in_state(UIState::Achievements)))
                )
            .add_system(
                handle_tooltip_teardown
                    .in_base_set(CoreSet::PreUpdate)
                    .run_if(in_state(GameState::Main)),
            )
            .add_systems(
                (
                    setup_inv_slots_ui,
                    setup_chest_slots_ui.run_if(in_state(UIState::Chest)),
                    setup_scrapper_slots_ui.run_if(in_state(UIState::Scrapper)),
                    tick_tooltip_timer,
                    handle_populate_essence_shop_on_new_spawn,
                    handle_cursor_skills_buttons.run_if(in_state(UIState::Skills)),
                    update_furnace_bar,
                    setup_skill_choice_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Skills))),
                    handle_heirloom_shrine_ui_setup
                        .before(setup_skill_choice_ui)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::Skills))),
                    setup_item_chest_ui
                        .before(CustomFlush)
                        .run_if(state_changed::<UIState>().and_then(in_state(UIState::ItemChest))),

                    handle_cursor_item_chest_button.run_if(in_state(UIState::ItemChest)),
                    interactions::handle_cursor_heirloom_chest_button
                        .run_if(in_state(UIState::ItemChest)),
                    setup_essence_ui
                        .before(CustomFlush)
                        .run_if(resource_added::<EssenceShopChoices>()),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    handle_submit_merchant_purchase
                        .run_if(resource_exists::<EssenceShopChoices>()),
                    handle_merchant_shop_interactions.run_if(in_state(UIState::Essence)),
                    handle_merchant_done_button.run_if(in_state(UIState::Essence)),
                    handle_merchant_category_reroll_buttons.run_if(in_state(UIState::Essence)),
                    handle_merchant_category_reroll_event.run_if(in_state(UIState::Essence)),
                    refresh_merchant_shop_ui_dirty.run_if(in_state(UIState::Essence)),
                    sync_merchant_marker_overlay.run_if(in_state(UIState::Essence)),
                    update_chest_button_label_hover.run_if(in_state(UIState::Essence)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    sync_merchant_world_marker_displays,
                    update_merchant_world_marker_price_colors,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_blacksmith_coin_display
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(in_state(UIState::Essence)),
            )
            .add_system(
                update_blacksmith_reroll_display
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(in_state(UIState::Essence)),
            )
            .add_system(
                update_merchant_reroll_button_states
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(in_state(UIState::Essence)),
            )
            .add_system(
                update_merchant_price_text_colors
                    .in_set(OnUpdate(GameState::Main))
                    .run_if(in_state(UIState::Essence)),
            )
            .add_system(
                    handle_update_player_skills.after(clamp_health).run_if(in_state(GameState::Main).or_else(in_state(GameState::BlessingChoice))),
            )
            .add_systems(
                (
                    player_hud::sync_consumable_buff_hud,
                    player_hud::sync_heirloom_hud_depth,
                    player_hud::tick_consumable_buff_hud_overlays.run_if(is_not_paused),
                    handle_heirloom_hud_tooltip,
                    player_hud::handle_consumable_buff_hud_tooltip,
                    player_hud::handle_active_skill_hud_tooltip,
                    player_hud::handle_mana_tracker_hud_tooltip,
                    player_hud::handle_health_tracker_hud_tooltip,
                    player_hud::update_skill_tooltip_cooldown.after(player_hud::handle_active_skill_hud_tooltip),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            // PostUpdate: `update_cursor_pos` runs here; ordering drag-drop after it from
            // `OnUpdate` would create an Update ↔ PostUpdate cycle (UpdateFlush → physics →
            // cursor → drag → UpdateFlush).
            .add_system(
                player_hud::handle_active_skill_slot_drag_drop
                    .after(crate::cursor::update_cursor_pos)
                    .run_if(in_state(GameState::Main))
                    .in_base_set(CoreSet::PostUpdate),
            )
            .add_systems(
                (
                    handle_heirloom_hud_tooltip,
                    player_hud::sync_heirloom_hud_depth,
                    process_heirloom_tooltip_requests,
                )
                    .in_set(OnUpdate(GameState::GameOver)),
            )
            .add_system(
                player_hud::hide_xp_bar_in_game_over.in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                player_hud::hide_xp_bar_in_game_over.in_set(OnUpdate(GameState::GameOver)),
            )
            .add_system(
                player_hud::cancel_active_skill_drag_on_state_exit
                    .in_schedule(OnExit(GameState::Main)),
            )
            .add_system(
                update_active_skill_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_inventory_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_minimap_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_hotbar_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                update_interact_guide_keybind_text
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_essence_heirloom_tooltip
                    .in_set(OnUpdate(GameState::Main))
                    .after(handle_merchant_shop_interactions)
                    .run_if(in_state(UIState::Essence)),
            )
            .add_system(
                handle_merchant_item_tooltip
                    .in_set(OnUpdate(GameState::Main))
                    .after(handle_merchant_shop_interactions)
                    .run_if(in_state(UIState::Essence)),
            )
            .add_system(
                handle_item_chest_final_item_hover
                    .run_if(in_state(UIState::ItemChest))
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_heirloom_chest_final_item_hover
                    .run_if(in_state(UIState::ItemChest))
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    setup_active_skill_shrine_ui.before(CustomFlush).run_if(
                        state_changed::<UIState>().and_then(in_state(UIState::ActiveSkillShrine)),
                    ),
                    setup_microwave_shrine_ui.before(CustomFlush).run_if(
                        state_changed::<UIState>().and_then(in_state(UIState::MicrowaveShrine)),
                    ),
                    tick_active_skill_shrine_ui_interaction_lock_timers
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    handle_microwave_shrine_rarity_click
                        .run_if(in_state(UIState::MicrowaveShrine)),
                    handle_microwave_shrine_heirloom_click
                        .run_if(in_state(UIState::MicrowaveShrine)),
                    handle_microwave_shrine_heirloom_tooltip
                        .after(handle_microwave_shrine_heirloom_click)
                        .run_if(in_state(UIState::MicrowaveShrine)),
                    handle_active_skill_shrine_ui_interaction
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    handle_active_skill_shrine_reroll_button
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    update_active_skill_shrine_reroll_button_state
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    update_active_skill_shrine_reroll_count_text
                        .run_if(in_state(UIState::ActiveSkillShrine)),
                    active_skill_shrine_ui::tick_active_skill_slot_choice_ui_interaction_lock_timers
                        .run_if(in_state(UIState::ActiveSkills)),
                    handle_active_skill_shrine_overwrite_interaction
                        .run_if(in_state(UIState::ActiveSkills)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                active_skill_shrine_ui::setup_active_skill_shrine_overwrite_ui
                    .before(CustomFlush)
                    .run_if(
                        state_changed::<UIState>()
                            .and_then(in_state(UIState::ActiveSkills))
                            .and_then(resource_exists::<ActiveSkillShrineOverwrite>()),
                    )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems((setup_class_selection_ui
                .before(CustomFlush)
                .run_if(state_changed::<UIState>().and_then(in_state(UIState::ClassSelection))),
                update_class_unlock_warnings
                    .run_if(in_state(UIState::ClassSelection).and_then(
                        resource_changed::<crate::player::achievements::Achievements>()
                            .or_else(resource_changed::<crate::player::UnlockedClasses>())
                    )),))
            .add_systems(
                (
                    handle_anim_events.run_if(in_state(UIState::ItemChest)),
                    update_chest_button_label_hover.run_if(in_state(UIState::ItemChest)),
                    tick_skill_choice_interaction_lock_timers,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    tick_skill_cooldown_overlays.run_if(is_not_paused),
                    update_player_movement_cooldown_bar
                        .run_if(is_not_paused)
                        .after(tick_skill_cooldown_overlays),
                    player_hud::handle_active_skill_event
                        .run_if(is_not_paused)
                        .after(crate::player::skill_heirlooms::handle_active_skill_event),
                    tick_game_start_overlay,
                    player_hud::tick_xp_bar_fade_in
                        .after(handle_flash_bars)
                        .after(player_hud::drain_pending_xp),
                    handle_clamp_screen_locked_icons_worldpos,
                    spawn_shrine_interact_key_guide,
                    add_guide_to_unique_objs,
                    toggle_skills_visibility,
                    toggle_item_chest_visibility.run_if(resource_exists::<ItemChestState>()),
                    update_mana_bar,
                    player_hud::update_pet_skill_hud_slot,
                    player_hud::tick_pet_skill_cooldown_overlay,
                    player_hud::handle_pet_skill_hud_tooltip
                        .after(player_hud::update_pet_skill_hud_slot),
                    spawn_tile_hover_on_cursor_move,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    shuffle_items.run_if(in_state(UIState::ItemChest)),
                    handle_skill_reroll_after_flash.run_if(in_state(UIState::Skills)),
                    handle_cursor_reroll_dice_buttons.run_if(in_state(UIState::Skills)),
                    handle_cursor_banish_buttons.run_if(in_state(UIState::Skills)),
                    update_skill_choice_button_states.run_if(in_state(UIState::Skills)),
                    update_skill_choice_count_text.run_if(in_state(UIState::Skills)),
                    handle_cursor_inventory_upgrade_button.run_if(in_state(UIState::Inventory)),
                    handle_cursor_inventory_craft_toggle_button.run_if(
                        in_state(UIState::Inventory)
                            .or_else(in_state(UIState::InventoryCrafting)),
                    ),
                    update_upgrade_material_prompt_text.run_if(in_state(UIState::Inventory)),
                    handle_dev_button_clicks.run_if(in_state(UIState::Inventory)),
                    setup_furnace_slots_ui.run_if(in_state(UIState::Furnace)),
                    handle_blueprint_slot_interaction
                        .run_if(in_state(UIState::InventoryCrafting)),
                    refresh_crafting_ingredient_display
                        .run_if(in_state(UIState::InventoryCrafting)),
                    // Runs before `handle_item_drop_clicks` so a click on the result slot is
                    // consumed for crafting instead of being treated as a drop.
                    handle_crafting_result_slot_click
                        .before(handle_item_drop_clicks)
                        .run_if(in_state(UIState::InventoryCrafting)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    handle_dev_heirloom_picker_toggle.run_if(in_state(UIState::Inventory)),
                    handle_dev_heirloom_picker_clicks.run_if(in_state(UIState::Inventory)),
                    handle_dev_skill_picker_toggle.run_if(in_state(UIState::Inventory)),
                    handle_dev_skill_picker_clicks.run_if(in_state(UIState::Inventory)),
                    handle_time_crystal_unlock_hover_tooltip
                        .run_if(in_state(UIState::Inventory)),
                    apply_grant_heirloom_dev.run_if(in_state(UIState::Inventory)),
                    apply_revoke_heirloom_dev.run_if(in_state(UIState::Inventory)),
                    apply_grant_skill_dev.run_if(in_state(UIState::Inventory)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    crate::ui::inventory_ui::handle_blueprint_pagination_clicks
                        .run_if(in_state(UIState::InventoryCrafting)),
                    crate::ui::inventory_ui::refresh_blueprints_on_pagination_change
                        .run_if(in_state(UIState::InventoryCrafting)),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_systems(
                (
                    update_banish_tracker_ui
                        .run_if(in_state(UIState::Skills).or_else(in_state(UIState::ItemChest))),
                    handle_banish_tracker_tooltip
                        .run_if(in_state(UIState::Skills).or_else(in_state(UIState::ItemChest))),
                    handle_skill_choice_info_box_hover
                        .run_if(in_state(UIState::Skills))
                        .after(handle_cursor_skills_buttons),
                    process_heirloom_tooltip_requests,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                process_heirloom_tooltip_requests.in_set(OnUpdate(GameState::MainMenu)),
            )
            .add_systems((
                auto_equip_upgrade_slot_on_inv_close
                    .before(handle_new_ui_state)
                    .in_base_set(CoreSet::PostUpdate),
                handle_new_ui_state.in_base_set(CoreSet::PostUpdate),
                sync_client_pause_with_modal_overlays
                    .after(handle_new_ui_state)
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            ))
            .add_systems((
                handle_class_selection.run_if(in_state(UIState::ClassSelection)),
                handle_slot_deselection.run_if(in_state(UIState::ClassSelection)),
                update_preview_sprites.run_if(in_state(UIState::ClassSelection)),
                update_slot_visuals.run_if(in_state(UIState::ClassSelection)),
                update_class_option_icons.run_if(in_state(UIState::ClassSelection)),
                update_info_card.run_if(in_state(UIState::ClassSelection)),
                update_unlock_currency_text.run_if(in_state(UIState::ClassSelection)),
                update_class_unlock_panel.run_if(in_state(UIState::ClassSelection)),
                update_class_unlock_confirm_panel.run_if(in_state(UIState::ClassSelection)),
                handle_locked_skill_selection.run_if(in_state(UIState::ClassSelection)),
                update_skill_unlock_confirm_panel.run_if(in_state(UIState::ClassSelection)),
                handle_portal_animation.run_if(in_state(UIState::ClassSelection)),
            ))
            .add_system(
                init_goal_state
                    .after(setup_currency_ui)
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_system(
                display_goal_text
                    .after(setup_currency_ui)
                    .run_if(resource_added::<GoalState>())
                    .in_schedule(OnEnter(GameState::Main)),
            )
            .add_systems(
                (
                    handle_goal_state_updates,
                    handle_goal_reset_on_era_change,
                    display_goal_text.after(setup_currency_ui),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_sort_inventory_button_click
                    .run_if(
                        in_state(UIState::Inventory)
                            .or_else(in_state(UIState::InventoryCrafting))
                            .or_else(in_state(UIState::Crafting)),
                    )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_material_drops_toggle_button_click
                    .run_if(
                        in_state(UIState::Inventory)
                            .or_else(in_state(UIState::InventoryCrafting))
                            .or_else(in_state(UIState::Crafting)),
                    )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_damage_tracker_toggle_button_click
                    .run_if(
                        in_state(UIState::Inventory)
                            .or_else(in_state(UIState::InventoryCrafting)),
                    )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                handle_material_drop_filter_menu_click
                    .run_if(
                        in_state(UIState::Inventory)
                            .or_else(in_state(UIState::InventoryCrafting))
                            .or_else(in_state(UIState::Crafting)),
                    )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(handle_hovering.run_if(ui_hover_interactions_condition).after(crate::ui::inventory_ui::update_inventory_ui))
            .add_system(handle_cursor_main_menu_buttons)
            .add_system(update_achievements_notification_icon.run_if(in_state(GameState::MainMenu)));

        app.add_systems(
            (
                debug_trigger_achievement_banner,
                handle_achievement_banner_events,
                update_achievement_banners,
            )
                .in_set(OnUpdate(GameState::Main)),
        );

        app.add_system(update_currency_text.run_if(in_state(GameState::Main)))
            .add_system(update_chaos_ui.run_if(in_state(GameState::Main)))
            .add_system(update_score_text.run_if(resource_changed::<RunScore>()))
            .add_system(player_hud::update_skill_charge_text.run_if(in_state(GameState::Main)))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

fn ui_hover_interactions_condition(state: Res<State<GameState>>) -> bool {
    state.0 == GameState::Main || state.0 == GameState::MainMenu
}

/// Runs just before [`handle_new_ui_state`] in `PostUpdate`. When the main inventory is being
/// closed (Esc / toggle key / any `Closed` transition), tries to move the item in the upgrade
/// slot into the first matching empty equipment slot so it doesn't get hidden behind the closed
/// panel. Kept in its own system to avoid a query conflict with `handle_new_ui_state`'s
/// `hotbar_slots` query, which also borrows `InventorySlotState` mutably.
pub fn auto_equip_upgrade_slot_on_inv_close(
    next_ui_state: Res<NextState<UIState>>,
    curr_ui_state: Res<State<UIState>>,
    mut inv: Query<&mut Inventory>,
    mut inv_slots: Query<&mut InventorySlotState>,
    proto: ProtoParam,
) {
    if !curr_ui_state.0.is_main_inventory() {
        return;
    }
    let Some(next_ui) = &next_ui_state.0 else {
        return;
    };
    if !matches!(next_ui, UIState::Closed) && *next_ui != curr_ui_state.0 {
        return;
    }
    if let Ok(mut inv) = inv.get_single_mut() {
        try_auto_equip_from_upgrade_slot(&mut inv, &proto, &mut inv_slots);
    }
}

/// Mirrors the pause rule in [`handle_new_ui_state`]: gameplay pauses whenever a menu is open,
/// tip boxes are shown, the minimap overlay is up, a tutorial replay is queued from Options
/// ([`tutorial_ui::TutorialReplayRequested`]), or the tutorial overlay is visible.
pub fn sync_client_pause_with_modal_overlays(
    ui_state: Res<State<UIState>>,
    tip_boxes: Query<Entity, With<tips::TipBox>>,
    minimap_open: Res<minimap::IslandMapOpen>,
    tutorial_ui: Query<(), With<tutorial_ui::TutorialUI>>,
    tutorial_replay_pending: Option<Res<tutorial_ui::TutorialReplayRequested>>,
    mut next_client_state: ResMut<NextState<ClientState>>,
) {
    let should_pause = ui_state.0 != UIState::Closed
        || !tip_boxes.is_empty()
        || minimap_open.0
        || !tutorial_ui.is_empty()
        || tutorial_replay_pending.is_some();
    if should_pause {
        next_client_state.set(ClientState::Paused);
    } else {
        next_client_state.set(ClientState::Unpaused);
    }
}

pub fn handle_new_ui_state(
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut next_client_state: ResMut<NextState<ClientState>>,
    curr_ui_state: Res<State<UIState>>,
    old_ui: Query<(Entity, &UIState), With<UIState>>,
    mut commands: Commands,
    chest_option: Option<Res<ChestContainer>>,
    scrapper_option: Option<Res<ScrapperContainer>>,
    furnace_option: Option<Res<FurnaceContainer>>,
    mut hotbar_slots: Query<(&mut Visibility, &mut InventorySlotState), Without<Interactable>>,
    tip_boxes: Query<Entity, With<tips::TipBox>>,
    minimap_open: Res<minimap::IslandMapOpen>,
    mut drop_filter_menu_open: ResMut<crate::inventory::MaterialDropFilterMenuOpen>,
    item_tooltips: Query<Entity, With<crate::ui::ItemOrRecipeTooltip>>,
    shop: Option<Res<EssenceShopChoices>>,
    mut shop_cache: ResMut<EssenceShopCache>,
) {
    if next_ui_state.0.is_none() {
        return;
    }
    let next_ui = next_ui_state.0.as_ref().unwrap().clone();

    let mut should_close_self = false;
    let should_reset_crafting_container =
        next_ui != curr_ui_state.0 && curr_ui_state.0.is_inv_open();
    if *DEBUG {
        debug!(
            "UI State Changed: {:?} -> {:?} | should reset: {should_reset_crafting_container:?}",
            curr_ui_state.0, next_ui
        );
    }
    if next_ui == curr_ui_state.0 {
        next_ui_state.set(UIState::Closed);
        should_close_self = true;
    }

    // Close the drop-filter side menu whenever the player leaves the standard Inventory state
    // (closes UI, swaps to crafting/blueprints, opens chest/scrapper/furnace, etc). The panel
    // entity itself is despawned by the loop below; this just resets the resource so the next
    // time setup_inv_ui spawns the panel it starts off-screen and the filter button reflects
    // the closed state.
    let leaving_inventory = should_close_self
        || (curr_ui_state.0 == UIState::Inventory && next_ui != UIState::Inventory);
    if leaving_inventory && drop_filter_menu_open.0 {
        drop_filter_menu_open.0 = false;
    }
    if should_close_self || curr_ui_state.0 != next_ui {
        for e in item_tooltips.iter() {
            if let Some(ec) = commands.get_entity(e) {
                ec.despawn_recursive();
            }
        }
    }
    for (e, ui) in old_ui.iter() {
        if *ui != next_ui || should_close_self {
            if let Some(entity_commands) = commands.get_entity(e) {
                entity_commands.despawn_recursive();
            }
        }
    }
    if let Some(chest) = chest_option {
        if let Some(mut chest_parent) = commands.get_entity(chest.parent) {
            chest_parent.insert(chest.to_owned());
        }
        if next_ui != UIState::Chest {
            commands.remove_resource::<ChestContainer>();
        }
    }
    if let Some(scrapper) = scrapper_option {
        if let Some(mut scrapper_parent) = commands.get_entity(scrapper.parent) {
            scrapper_parent.insert(scrapper.to_owned());
        }
        if next_ui != UIState::Scrapper {
            commands.remove_resource::<ScrapperContainer>();
        }
    }
    if let Some(furnace) = furnace_option {
        if let Some(mut furnace_parent) = commands.get_entity(furnace.parent) {
            furnace_parent.insert(furnace.to_owned());
        }
        if next_ui != UIState::Furnace {
            commands.remove_resource::<FurnaceContainer>();
        }
    }
    if !next_ui.is_inv_open() || should_close_self || should_reset_crafting_container {
        commands.remove_resource::<CraftingContainer>();
    }
    if next_ui != UIState::Essence {
        if curr_ui_state.0 == UIState::Essence {
            if let Some(shop) = shop.as_ref() {
                sync_merchant_shop_to_world(shop, &mut commands, &mut shop_cache);
            }
        }
        commands.remove_resource::<EssenceShopChoices>();
    }
    if let Some(next_ui) = &next_ui_state.0 {
        for (mut hbv, mut state) in hotbar_slots.iter_mut() {
            if !next_ui.is_inv_open() {
                state.dirty = true;
            }
            *hbv = if next_ui.is_inv_open() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
    }
    info!("{:?}", next_ui);
    let has_tip_boxes = !tip_boxes.is_empty();
    let minimap_is_open = minimap_open.0;

    if next_ui_state.0.as_ref().unwrap() != &UIState::Closed
        || has_tip_boxes
        || minimap_is_open
    {
        next_client_state.set(ClientState::Paused);
    } else {
        next_client_state.set(ClientState::Unpaused);
    }
}

/// System that checks for pending level-up rewards when closing menus.
/// If there are pending heirloom choices from level-ups, redirect to the Skills UI instead of closing.
/// This prevents players from accidentally missing their level-up rewards when they level up
/// while in another menu.
pub fn check_pending_levelup_rewards_on_menu_close(
    heirloom_queue: Res<HeirloomChoiceQueue>,
    mut next_inv_state: ResMut<NextState<UIState>>,
) {
    // This system runs when we just entered UIState::Closed (via run conditions)
    // Check if there are pending heirloom choices from level-ups
    if !heirloom_queue.queue.is_empty() {
        info!(
            "Detected {} pending level-up rewards! Redirecting to Skills UI.",
            heirloom_queue.queue.len()
        );
        // Override the transition to Closed - go to Skills instead
        next_inv_state.set(UIState::Skills);
    }
}

/// Grant the player a brief invulnerability window when they finish interacting with a chest
/// reward (`UIState::ItemChest`) or a level-up screen (`UIState::Skills` /
/// `UIState::ActiveSkills`). This avoids the player taking an instant hit on the frame the menu
/// closes when an enemy has walked on top of them while time was paused.
pub fn grant_iframes_after_chest_or_levelup_ui_close(
    mut commands: Commands,
    mut prev_ui_state: Local<UIState>,
    curr_ui_state: Res<State<UIState>>,
    player: Query<Entity, With<Player>>,
) {
    let prev = prev_ui_state.clone();
    *prev_ui_state = curr_ui_state.0.clone();

    if curr_ui_state.0 == prev || curr_ui_state.0 != UIState::Closed {
        return;
    }

    let was_reward_ui = !matches!(
        prev,
        UIState::Inventory | UIState::Options
    );
    if !was_reward_ui {
        return;
    }

    let Ok(player_e) = player.get_single() else {
        return;
    };
    commands
        .entity(player_e)
        .insert(InvincibilityTimer(Timer::from_seconds(
            1.,
            TimerMode::Once,
        )));
}
