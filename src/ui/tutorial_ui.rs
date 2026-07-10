use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use crate::attributes::CurrentMana;
use crate::audio::{AudioSoundEffect, SoundSpawner};
use crate::chaos::ChaosTracker;
use crate::client::GameData;
use crate::colors::{BLACK, DARK_GREEN, WHITE, YELLOW_2};
use crate::cursor::CursorPos;
use crate::datafiles;
use crate::item::WorldObject;
use crate::player::score::RunTimer;
use crate::player::skills::PlayerSkills;
use crate::player::Player;
use crate::proto::proto_param::ProtoParam;
use crate::ui::player_hud::{
    hud_bottom_corner_icon_row_y, hud_mana_orb_center, hud_map_icon_x, hud_settings_icon_x,
    HUD_CORNER_ICON_SIZE, HUD_FILL_PIXEL_SIZE,
};
use crate::ui::tip_highlight::{
    spawn_tip_highlight_overlay, TipHighlightMaterial, TipHighlightRect,
};
use crate::ui::{
    global_text_message::GlobalTextMessageEvent,
    game_fonts as gf,
    hud_heirloom_first_icon_x,
    hud_heirloom_row_y,
    hud_progress_bar_center_x,
    hud_row_below_xp_y,
    hud_timeline_center_x,
    Interactable,
    Interaction,
    HUD_ACTION_ROW_Y_FROM_BOTTOM,
    HUD_HEIRLOOM_ICON_HALF,
    HUD_HEIRLOOM_ICON_SPACING,
    HUD_HOTBAR_CENTER_X,
    HUD_HOTBAR_SLOTS,
    HUD_SKILLS_CENTER_X,
    HUD_SKILL_SLOT_HIT_SIZE,
    HUD_SKILL_SPACING_X,
    HUD_TIMELINE_SIZE,
    INV_SLOT_SPACING_X,
    PROGRESS_BACKGROUND_SIZE,
    UI_SLOT_SIZE,
};
use crate::{inventory::Inventory, GameState, ScreenResolution};

/// Z layers (sit above gameplay HUD but below pause overlays).
const Z_TUTORIAL_OVERLAY: f32 = 70.0;
const Z_TUTORIAL_PANEL: f32 = 71.0;
const Z_TUTORIAL_CONTENT: f32 = 72.0;
const Z_TUTORIAL_TEXT: f32 = 73.0;

const PANEL_HEIGHT: f32 = 248.0;
const PANEL_HEIGHT_SINGLE: f32 = PANEL_HEIGHT + 0.0;
const PANEL_WIDTH_SINGLE: f32 = 220.0 + 0.0;
const PANEL_WIDTH_DOUBLE: f32 = 416.0;
/// How far each column shifts toward center when exactly two tips are shown (80px total gap).
const DOUBLE_ENTRY_COLUMN_INSET: f32 = 40.0;
const PANEL_WIDTH_TRIPLE: f32 = 608.0;

const ENTRIES_PER_PAGE: usize = 3;
const CONTENT_BAND_CENTER_Y: f32 = 10.0;
const TUTORIAL_ASEPRITE_SIZE: f32 = 64.0;
const TUTORIAL_ANIM_CENTER_Y: f32 = CONTENT_BAND_CENTER_Y - 54.0;

/// Delay before showing the biomes / environment tutorial.
pub const BIOME_TUTORIAL_DELAY_SECS: f64 = 60.0;

/// Delay before showing the map-navigation tutorial (nudges players to check the map for
/// shrines / the boss shrine around the 2-minute mark).
pub const MAP_NAVIGATION_TUTORIAL_DELAY_SECS: f64 = 120.0;

/// Heirloom picks required before the heirlooms tutorial appears.
pub const HEIRLOOM_TUTORIAL_PICK_COUNT: usize = 3;

/// Buffer kept clear around the highlighted heirloom icons in the [`TutorialContent::Heirlooms`]
/// spotlight overlay.
const HEIRLOOM_HIGHLIGHT_BUFFER: f32 = 4.0;
/// Nudge the heirloom spotlight rect right so its left edge (buffer included) doesn't clip
/// against the screen's left edge, since the first heirloom icon sits very close to it.
const HEIRLOOM_HIGHLIGHT_X_NUDGE: f32 = 3.0;

/// Rect (in UI render-layer-3 coordinates) spanning the first `heirloom_count` heirloom HUD
/// icons plus [`HEIRLOOM_HIGHLIGHT_BUFFER`] of padding, for the heirloom tutorial's spotlight.
fn heirloom_row_highlight_rect(res: &ScreenResolution, heirloom_count: usize) -> TipHighlightRect {
    let heirloom_count = heirloom_count.max(1);
    let first_icon_x = hud_heirloom_first_icon_x(res.game_width) + HEIRLOOM_HIGHLIGHT_X_NUDGE;
    let row_y = hud_heirloom_row_y(res.game_height);
    let last_icon_x = first_icon_x + HUD_HEIRLOOM_ICON_SPACING * (heirloom_count as f32 - 1.0);

    let left_edge = first_icon_x - HUD_HEIRLOOM_ICON_HALF - HEIRLOOM_HIGHLIGHT_BUFFER;
    let right_edge = last_icon_x + HUD_HEIRLOOM_ICON_HALF + HEIRLOOM_HIGHLIGHT_BUFFER;
    let top_edge = row_y + HUD_HEIRLOOM_ICON_HALF + HEIRLOOM_HIGHLIGHT_BUFFER;
    let bottom_edge = row_y - HUD_HEIRLOOM_ICON_HALF - HEIRLOOM_HIGHLIGHT_BUFFER;

    TipHighlightRect {
        center: Vec2::new(
            (left_edge + right_edge) * 0.5,
            (top_edge + bottom_edge) * 0.5,
        ),
        size: Vec2::new(right_edge - left_edge, top_edge - bottom_edge),
    }
}

/// Buffer kept clear around the highlighted map/inventory/settings HUD icons in the
/// [`TutorialContent::MapNavigation`] spotlight overlay.
const MAP_NAV_HIGHLIGHT_BUFFER: f32 = 4.0;

/// Rect (in UI render-layer-3 coordinates) spanning the bottom-left minimap + inventory +
/// options HUD corner icons plus [`MAP_NAV_HIGHLIGHT_BUFFER`] of padding, for the
/// map-navigation tutorial's spotlight.
fn map_nav_highlight_rect(res: &ScreenResolution) -> TipHighlightRect {
    let row_y = hud_bottom_corner_icon_row_y(res.game_height);
    let map_x = hud_map_icon_x(res.game_width);
    let settings_x = hud_settings_icon_x(res.game_width);
    let icon_half = HUD_CORNER_ICON_SIZE * 0.5;

    let left_edge = map_x - icon_half.x - MAP_NAV_HIGHLIGHT_BUFFER;
    let right_edge = settings_x + icon_half.x + MAP_NAV_HIGHLIGHT_BUFFER;
    let top_edge = row_y + icon_half.y + MAP_NAV_HIGHLIGHT_BUFFER;
    let bottom_edge = row_y - icon_half.y - MAP_NAV_HIGHLIGHT_BUFFER;

    TipHighlightRect {
        center: Vec2::new(
            (left_edge + right_edge) * 0.5,
            (top_edge + bottom_edge) * 0.5,
        ),
        size: Vec2::new(right_edge - left_edge, top_edge - bottom_edge),
    }
}

/// Buffer kept clear around the highlighted skill icons in the [`TutorialContent::Skills`]
/// spotlight overlay.
const SKILLS_HIGHLIGHT_BUFFER: f32 = 4.0;
/// Extra padding below the skill icons so the spotlight box extends slightly downward.
const SKILLS_HIGHLIGHT_BOTTOM_EXTRA: f32 = 3.0;

/// Rect (in UI render-layer-3 coordinates) spanning the class-skill + pet-skill HUD icon group
/// on the right side of the action row, for the skills tutorial's spotlight.
fn skills_highlight_rect(res: &ScreenResolution) -> TipHighlightRect {
    let action_row_y = -res.game_height * 0.5 + HUD_ACTION_ROW_Y_FROM_BOTTOM;
    // 3 class skill slots + 1 pet slot, grouped as one visual unit (see `HUD_SKILLS_CENTER_X`).
    let num_skills = 4.0;
    let skill_half_span = (num_skills - 1.0) * 0.5;
    let leftmost_x = HUD_SKILLS_CENTER_X - skill_half_span * HUD_SKILL_SPACING_X;
    let rightmost_x = HUD_SKILLS_CENTER_X + skill_half_span * HUD_SKILL_SPACING_X;
    let icon_half = HUD_SKILL_SLOT_HIT_SIZE * 0.5;

    let left_edge = leftmost_x - icon_half.x - SKILLS_HIGHLIGHT_BUFFER;
    let right_edge = rightmost_x + icon_half.x + SKILLS_HIGHLIGHT_BUFFER;
    let top_edge = action_row_y + icon_half.y + SKILLS_HIGHLIGHT_BUFFER;
    let bottom_edge =
        action_row_y - icon_half.y - SKILLS_HIGHLIGHT_BUFFER - SKILLS_HIGHLIGHT_BOTTOM_EXTRA;

    TipHighlightRect {
        center: Vec2::new(
            (left_edge + right_edge) * 0.5,
            (top_edge + bottom_edge) * 0.5,
        ),
        size: Vec2::new(right_edge - left_edge, top_edge - bottom_edge),
    }
}

/// Buffer kept clear around the highlighted hotbar slots in the [`TutorialContent::HotbarFood`]
/// spotlight overlay (matches the skills tutorial padding).
const HOTBAR_HIGHLIGHT_BUFFER: f32 = SKILLS_HIGHLIGHT_BUFFER;
const HOTBAR_HIGHLIGHT_BOTTOM_EXTRA: f32 = SKILLS_HIGHLIGHT_BOTTOM_EXTRA;

/// Rect (in UI render-layer-3 coordinates) spanning the four keyed hotbar slots on the left side
/// of the HUD action row, for the hotbar tutorial's spotlight.
fn hotbar_highlight_rect(res: &ScreenResolution) -> TipHighlightRect {
    let action_row_y = -res.game_height * 0.5 + HUD_ACTION_ROW_Y_FROM_BOTTOM;
    let slot_spacing = INV_SLOT_SPACING_X - 5.0;
    let half_span = (HUD_HOTBAR_SLOTS as f32 - 1.0) * 0.5;
    let leftmost_x = HUD_HOTBAR_CENTER_X - half_span * slot_spacing;
    let rightmost_x = HUD_HOTBAR_CENTER_X + half_span * slot_spacing;
    let slot_half = UI_SLOT_SIZE * 0.5;

    let left_edge = leftmost_x - slot_half.x - HOTBAR_HIGHLIGHT_BUFFER;
    let right_edge = rightmost_x + slot_half.x + HOTBAR_HIGHLIGHT_BUFFER;
    let top_edge = action_row_y + slot_half.y + HOTBAR_HIGHLIGHT_BUFFER;
    let bottom_edge =
        action_row_y - slot_half.y - HOTBAR_HIGHLIGHT_BUFFER - HOTBAR_HIGHLIGHT_BOTTOM_EXTRA;

    TipHighlightRect {
        center: Vec2::new(
            (left_edge + right_edge) * 0.5,
            (top_edge + bottom_edge) * 0.5,
        ),
        size: Vec2::new(right_edge - left_edge, top_edge - bottom_edge),
    }
}

/// Buffer kept clear around the highlighted mana orb in the [`TutorialContent::Mana`]
/// spotlight overlay.
const MANA_HIGHLIGHT_BUFFER: f32 = 4.0;

/// Rect (in UI render-layer-3 coordinates) spanning the blue mana orb on the right side of
/// the HUD bar, for the mana tutorial's spotlight.
fn mana_orb_highlight_rect(res: &ScreenResolution) -> TipHighlightRect {
    let center = hud_mana_orb_center(res);
    let size = HUD_FILL_PIXEL_SIZE + Vec2::splat(MANA_HIGHLIGHT_BUFFER * 2.0);

    TipHighlightRect { center, size }
}

/// Buffer kept clear around the highlighted progress bar / era timeline in the
/// [`TutorialContent::Chaos`] / [`TutorialContent::Timeline`] spotlight overlays.
const PROGRESS_BAR_HIGHLIGHT_BUFFER: f32 = 4.0;

/// Rect (in UI render-layer-3 coordinates) spanning just the score/chaos progress bar itself
/// (to the right of the era timeline), for the chaos tutorial's spotlight.
fn progress_bar_highlight_rect(res: &ScreenResolution) -> TipHighlightRect {
    let row_y = hud_row_below_xp_y(res.game_height);
    let center_x = hud_progress_bar_center_x(res);
    let size = PROGRESS_BACKGROUND_SIZE + Vec2::splat(PROGRESS_BAR_HIGHLIGHT_BUFFER * 2.0);

    TipHighlightRect {
        center: Vec2::new(center_x, row_y),
        size,
    }
}

/// Rect (in UI render-layer-3 coordinates) spanning the era timeline bar (to the left of the
/// score/chaos progress bar), for the [`TutorialContent::Timeline`] tutorial's spotlight.
fn timeline_highlight_rect(res: &ScreenResolution) -> TipHighlightRect {
    let row_y = hud_row_below_xp_y(res.game_height) + 2.;
    let center_x = hud_timeline_center_x(res.game_width);
    let size = HUD_TIMELINE_SIZE + Vec2::splat(PROGRESS_BAR_HIGHLIGHT_BUFFER * 2.0);

    TipHighlightRect {
        center: Vec2::new(center_x, row_y),
        size,
    }
}

/// Gap kept between a compact tutorial's spotlight rect and its (image-less) panel.
const COMPACT_PANEL_GAP: f32 = 6.0;

/// Compact panel width/height — smaller than the standard image + text panel since there's no
/// aseprite art, just a title + short body + Done button.
const PANEL_WIDTH_COMPACT: f32 = 170.0;
const PANEL_HEIGHT_COMPACT: f32 = 86.0;

/// Screen-edge padding kept clear when clamping a compact panel horizontally on-screen.
const COMPACT_PANEL_EDGE_PADDING: f32 = 6.0;

/// Clamps a compact panel's horizontal center so it stays fully on-screen (with
/// [`COMPACT_PANEL_EDGE_PADDING`] of breathing room), preventing left/right clipping when its
/// spotlight rect sits close to a screen edge.
fn clamp_compact_panel_x(x: f32, res: &ScreenResolution) -> f32 {
    let half_panel = PANEL_WIDTH_COMPACT * 0.5;
    let min_x = -res.game_width * 0.5 + half_panel + COMPACT_PANEL_EDGE_PADDING;
    let max_x = res.game_width * 0.5 - half_panel - COMPACT_PANEL_EDGE_PADDING;
    x.clamp(min_x, max_x)
}

/// Center a compact panel directly above `rect` (its bottom edge `COMPACT_PANEL_GAP` above the
/// rect's top edge), horizontally centered on it (clamped to stay on-screen).
fn compact_panel_above_rect(rect: TipHighlightRect, res: &ScreenResolution) -> Vec2 {
    Vec2::new(
        clamp_compact_panel_x(rect.center.x, res),
        rect.center.y + rect.size.y * 0.5 + COMPACT_PANEL_GAP + PANEL_HEIGHT_COMPACT * 0.5,
    )
}

/// Center a compact panel directly below `rect` (its top edge `COMPACT_PANEL_GAP` below the
/// rect's bottom edge), horizontally centered on it (clamped to stay on-screen).
fn compact_panel_below_rect(rect: TipHighlightRect, res: &ScreenResolution) -> Vec2 {
    Vec2::new(
        clamp_compact_panel_x(rect.center.x, res),
        rect.center.y - rect.size.y * 0.5 - COMPACT_PANEL_GAP - PANEL_HEIGHT_COMPACT * 0.5,
    )
}

/// Computes the spotlight rect + compact panel center for a compact [`TutorialContent`].
/// Returns `None` if `content` isn't a compact tutorial (see [`TutorialContent::is_compact`]).
fn compact_tutorial_layout(
    content: TutorialContent,
    res: &ScreenResolution,
) -> Option<(TipHighlightRect, Vec2)> {
    match content {
        TutorialContent::MapNavigation => {
            let rect = map_nav_highlight_rect(res);
            Some((rect, compact_panel_above_rect(rect, res)))
        }
        TutorialContent::Skills => {
            let rect = skills_highlight_rect(res);
            Some((rect, compact_panel_above_rect(rect, res)))
        }
        TutorialContent::HotbarFood => {
            let rect = hotbar_highlight_rect(res);
            Some((rect, compact_panel_above_rect(rect, res)))
        }
        TutorialContent::Mana => {
            let rect = mana_orb_highlight_rect(res);
            Some((rect, compact_panel_above_rect(rect, res)))
        }
        TutorialContent::Chaos => {
            let rect = progress_bar_highlight_rect(res);
            Some((rect, compact_panel_below_rect(rect, res)))
        }
        TutorialContent::Timeline => {
            let rect = timeline_highlight_rect(res);
            Some((rect, compact_panel_below_rect(rect, res)))
        }
        _ => None,
    }
}

aseprite!(pub TutorialAnims, "ui/TutorialAnims.ase");

#[derive(Resource)]
pub struct TutorialReady;

/// Delay (after the run fade-in completes) before the first-run start tutorial is armed.
/// The 3s game-start fade-in plus this delay lands the popup ~10s into the run, giving the
/// intro input-tip overlay (0.6s fade in + 8s hold + 0.6s fade out) time to finish first.
pub const START_TUTORIAL_DELAY_SECS: f32 = 7.0;

/// Inserted when the run fade-in ends; ticks down and then inserts [`TutorialReady`] so the
/// first-run tutorial popup is delayed rather than shown immediately.
#[derive(Resource)]
pub struct PendingTutorialReady(pub Timer);

/// Seconds after the hint is scheduled (tutorial done or skipped) before the message appears.
const FIND_BOSS_SHRINE_HINT_DELAY_SECS: f32 = 2.3;

#[derive(Resource)]
pub enum PendingFindBossShrineHint {
    /// Inserted when the run fade-in ends; armed when the start tutorial is skipped or closed.
    WaitingForTrigger,
    Delay(Timer),
}

#[derive(Resource)]
pub struct TutorialReplayRequested;

/// Set when the player opens the inventory; consumed by [`process_pending_inventory_tutorials`].
#[derive(Resource)]
pub struct PendingInventoryTutorialCheck;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TutorialContent {
    Attacking,
    Skills,
    Heirlooms,
    Equipment,
    UpgradeTomes,
    InventoryStats,
    ShrinesExplore,
    PinkFlowers,
    Crafting,
    MapNavigation,
    Timeline,
    Chaos,
    HotbarFood,
    Mana,
}

impl TutorialContent {
    pub const ALL: [TutorialContent; 14] = [
        TutorialContent::Attacking,
        TutorialContent::Skills,
        TutorialContent::Heirlooms,
        TutorialContent::Equipment,
        TutorialContent::UpgradeTomes,
        TutorialContent::InventoryStats,
        TutorialContent::ShrinesExplore,
        TutorialContent::PinkFlowers,
        TutorialContent::Crafting,
        TutorialContent::MapNavigation,
        TutorialContent::Timeline,
        TutorialContent::Chaos,
        TutorialContent::HotbarFood,
        TutorialContent::Mana,
    ];

    fn title(self) -> &'static str {
        match self {
            TutorialContent::Attacking => "Weapons",
            TutorialContent::Skills => "Skills",
            TutorialContent::Heirlooms => "Heirlooms",
            TutorialContent::Equipment => "Equipment",
            TutorialContent::UpgradeTomes => "Tomes & Orbs",
            TutorialContent::InventoryStats => "Inventory Stats",
            TutorialContent::ShrinesExplore => "Shrines",
            TutorialContent::PinkFlowers => "Biomes",
            TutorialContent::Crafting => "Crafting",
            TutorialContent::MapNavigation => "Map",
            TutorialContent::Timeline => "Timeline",
            TutorialContent::Chaos => "Chaos",
            TutorialContent::HotbarFood => "Hotbar & Food",
            TutorialContent::Mana => "Mana",
        }
    }

    fn body(self) -> &'static str {
        match self {
            TutorialContent::Attacking => {
                "Weapons attack automatically,\n\nuse your mouse to aim them."
            }
            TutorialContent::Skills => {
                "Skills are powerful, don't forget\n\nto use them! Drag to rearrange them."
            }
            TutorialContent::Heirlooms => {
                "Hover here to view your heirlooms.\n\nHeirlooms boost stats or trigger\n\nstrong effects. They are the key\n\nto getting stronger!"
            }
            TutorialContent::Equipment => {
                "Equipment can be equipped to boost\n\nyour stats. Upgrade them to make\n\nthem stronger!"
            }
            TutorialContent::UpgradeTomes => {
                "Drag and Drop Tomes & Orbs onto\n\nEquipment to improve their bonus\n\nstats and rarity!"
            }
            TutorialContent::InventoryStats => {
                "Place Extra gear in your inventory.\n\nYou will still gain the highlighted\n\nstat, shown here in purple!"
            }
            TutorialContent::ShrinesExplore => {
                "Explore the island and interact\n\nwith shrines, which offer various\n\nchoices or challenges. Find the\n\nBoss Shrine..."
            }
            TutorialContent::PinkFlowers => {
                "Biomes have different environmental\n\ninteractions, like these bouncing flowers\n\nfound in the forest"
            }
            TutorialContent::Crafting => {
                "Craft different foods and tools\n\nto help you on your journey.\n\nBiomes unlock different blueprints."
            }
            TutorialContent::MapNavigation => {
                "Check your map to navigate\n\nthe island and find shrines,\n\nincluding the Boss Shrine!\n\nYou can even mark locations!"
            }
            TutorialContent::Timeline => {
                "Night time is dangerous! Keep\n\ntrack of time here, and find the\n\nboss shrine before its too late"
            }
            TutorialContent::Chaos => {
                "Chaos makes enemies tougher but\n\ngrant more score. It rises over time,\n\nand through some player actions."
            }
            TutorialContent::HotbarFood => {
                "Place food in the hotbar for\n\nquick access when you need it!"
            }
            TutorialContent::Mana => {
                "Some heirloom triggers cost\n\nmana. Hover your mana orb for\n\ninfo on mana efficiency."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TutorialPopupMode {
    #[default]
    /// Bite-sized in-run popups: Done only, 1–3 entries, no pagination.
    Contextual,
    /// Options → Show Tutorial: full paginated walkthrough.
    FullReplay,
}

#[derive(Clone)]
pub struct TutorialPopupEvent {
    pub entries: Vec<TutorialContent>,
    pub mode: TutorialPopupMode,
}

#[derive(Resource, Default, Debug, Clone)]
pub struct SeenTutorialChunks {
    pub seen: HashSet<TutorialContent>,
}

impl SeenTutorialChunks {
    pub fn has_seen(&self, content: &TutorialContent) -> bool {
        self.seen.contains(content)
    }

    pub fn mark_seen(&mut self, content: TutorialContent) {
        self.seen.insert(content);
    }

    pub fn mark_all(&mut self, contents: impl IntoIterator<Item = TutorialContent>) {
        for c in contents {
            self.seen.insert(c);
        }
    }
}

pub fn seen_tutorial_chunks_from_game_data(game_data: &GameData) -> SeenTutorialChunks {
    let mut seen = game_data.seen_tutorial_chunks.clone();
    if game_data.has_seen_tutorial {
        for content in TutorialContent::ALL {
            seen.insert(content);
        }
    }
    SeenTutorialChunks { seen }
}

pub fn persist_seen_tutorial_chunks(chunks: &SeenTutorialChunks) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        GameData::default()
    };

    game_data.seen_tutorial_chunks = chunks.seen.clone();
    if chunks.seen.len() >= TutorialContent::ALL.len() {
        game_data.has_seen_tutorial = true;
    }

    match File::create(&path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            if let Err(err) = serde_json::to_writer(writer, &game_data) {
                error!("Failed to persist tutorial chunks to game_data.json: {err:?}");
            }
        }
        Err(err) => {
            error!("Failed to create game_data.json while saving tutorial chunks: {err:?}");
        }
    }
}

/// Queue a contextual popup for any entries the player has not seen yet.
pub fn try_send_contextual_popup(
    writer: &mut EventWriter<TutorialPopupEvent>,
    seen: &SeenTutorialChunks,
    existing: &Query<(), With<TutorialUI>>,
    entries: &[TutorialContent],
) {
    if !existing.is_empty() {
        return;
    }
    let pending: Vec<_> = entries
        .iter()
        .copied()
        .filter(|e| !seen.has_seen(e))
        .collect();
    if pending.is_empty() {
        return;
    }
    writer.send(TutorialPopupEvent {
        entries: pending,
        mode: TutorialPopupMode::Contextual,
    });
}

#[derive(Resource, Default)]
pub struct TutorialState {
    pub page: usize,
    pub mode: TutorialPopupMode,
    pub entries: Vec<TutorialContent>,
    pub panel_width: f32,
    pub panel_height: f32,
    pub panel_offset_x: f32,
}

#[derive(Component)]
pub struct TutorialUI;

#[derive(Component)]
pub struct TutorialPageEntity;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum TutorialButtonKind {
    Prev,
    Next,
    Done,
}

#[derive(Component)]
pub struct TutorialButton(pub TutorialButtonKind);

pub struct TutorialPlugin;

impl Plugin for TutorialPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TutorialState>()
            .add_event::<TutorialPopupEvent>()
            .add_systems(
                (
                    handle_tutorial_popup_events,
                    try_spawn_tutorial_overlay
                        .after(crate::ui::main_menu::handle_menu_button_click_events),
                    check_heirloom_tutorial,
                    check_biome_timer_tutorial,
                    check_map_navigation_tutorial,
                    check_chaos_tutorial,
                    check_hotbar_food_tutorial,
                    check_mana_tutorial,
                    process_pending_inventory_tutorials,
                    handle_tutorial_buttons,
                    tick_pending_find_boss_shrine_hint,
                    tick_pending_tutorial_ready,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}

fn panel_width_for_entry_count(count: usize) -> f32 {
    match count.min(3) {
        1 => PANEL_WIDTH_SINGLE,
        2 => PANEL_WIDTH_DOUBLE,
        _ => PANEL_WIDTH_TRIPLE,
    }
}

fn panel_height_for_entry_count(count: usize) -> f32 {
    if count == 1 {
        PANEL_HEIGHT_SINGLE
    } else {
        PANEL_HEIGHT
    }
}

/// Align a single-tip panel so its right (or, if `left_aligned`, left) edge sits on the UI view
/// edge. Multi-entry panels stay centered regardless.
fn panel_center_x(
    entry_count: usize,
    panel_width: f32,
    game_width: f32,
    left_aligned: bool,
) -> f32 {
    if entry_count == 1 {
        let edge_x = if left_aligned {
            -game_width * 0.5 + panel_width * 0.5
        } else {
            game_width * 0.5 - panel_width * 0.5
        };
        super::tooltips::clamp_tooltip_center_x(edge_x, panel_width * 0.5, game_width, 0.0)
    } else {
        0.0
    }
}

fn show_find_boss_shrine_hint(events: &mut EventWriter<GlobalTextMessageEvent>) {
    events.send(GlobalTextMessageEvent::new("Find the Boss Shrine", WHITE));
}

fn schedule_pending_find_boss_shrine_hint(
    commands: &mut Commands,
    pending: Option<Res<PendingFindBossShrineHint>>,
) {
    if pending.is_some() {
        commands.insert_resource(PendingFindBossShrineHint::Delay(Timer::from_seconds(
            FIND_BOSS_SHRINE_HINT_DELAY_SECS,
            TimerMode::Once,
        )));
    }
}

fn tick_pending_find_boss_shrine_hint(
    time: Res<Time>,
    mut pending: Option<ResMut<PendingFindBossShrineHint>>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
    mut commands: Commands,
) {
    let Some(PendingFindBossShrineHint::Delay(timer)) = pending.as_deref_mut() else {
        return;
    };
    timer.tick(time.delta());
    if timer.finished() {
        show_find_boss_shrine_hint(&mut global_text_events);
        commands.remove_resource::<PendingFindBossShrineHint>();
    }
}

fn tick_pending_tutorial_ready(
    time: Res<Time>,
    mut pending: Option<ResMut<PendingTutorialReady>>,
    mut commands: Commands,
) {
    let Some(pending) = pending.as_deref_mut() else {
        return;
    };
    pending.0.tick(time.delta());
    if pending.0.finished() {
        commands.insert_resource(TutorialReady);
        commands.remove_resource::<PendingTutorialReady>();
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_tutorial_popup_events(
    mut commands: Commands,
    mut events: EventReader<TutorialPopupEvent>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    mut state: ResMut<TutorialState>,
    existing: Query<(), With<TutorialUI>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut highlight_materials: ResMut<Assets<TipHighlightMaterial>>,
    skills: Query<&PlayerSkills>,
) {
    if !existing.is_empty() {
        events.clear();
        return;
    }
    let mut pending_event: Option<TutorialPopupEvent> = None;
    for event in events.iter() {
        pending_event = Some(event.clone());
    }
    events.clear();
    let Some(event) = pending_event else {
        return;
    };
    if !event.entries.is_empty() {
        // Compact tutorials (see `compact_tutorial_layout`) always show alone, positioned right
        // next to the specific HUD element they're spotlighting, with a small image-less box
        // instead of the standard modal. The heirloom tutorial is also compact, but its
        // spotlight rect depends on the player's current heirloom count, so it's computed here
        // rather than in `compact_tutorial_layout`.
        if let [content] = event.entries[..] {
            let compact_layout = if content == TutorialContent::Heirlooms {
                let heirloom_count = skills
                    .get_single()
                    .map(|s| s.heirlooms.len())
                    .unwrap_or(HEIRLOOM_TUTORIAL_PICK_COUNT);
                let rect = heirloom_row_highlight_rect(&resolution, heirloom_count);
                Some((rect, compact_panel_below_rect(rect, &resolution)))
            } else {
                compact_tutorial_layout(content, &resolution)
            };
            if let Some((highlight, panel_center)) = compact_layout {
                state.page = 0;
                state.mode = event.mode;
                state.entries = event.entries.clone();
                state.panel_width = PANEL_WIDTH_COMPACT;
                state.panel_height = PANEL_HEIGHT_COMPACT;
                state.panel_offset_x = panel_center.x;

                spawn_compact_tutorial(
                    &mut commands,
                    &asset_server,
                    &mut meshes,
                    &mut highlight_materials,
                    &resolution,
                    content,
                    panel_center,
                    Some(highlight),
                );
                return;
            }
        }

        let entry_count = event.entries.len().min(ENTRIES_PER_PAGE);
        let panel_width = panel_width_for_entry_count(entry_count);
        let panel_height = panel_height_for_entry_count(entry_count);
        let panel_offset_x = panel_center_x(entry_count, panel_width, resolution.game_width, false);

        state.page = 0;
        state.mode = event.mode;
        state.entries = event.entries.clone();
        state.panel_width = panel_width;
        state.panel_height = panel_height;
        state.panel_offset_x = panel_offset_x;

        spawn_tutorial_root(
            &mut commands,
            &asset_server,
            &mut meshes,
            &mut highlight_materials,
            &resolution,
            state.mode,
            panel_width,
            panel_height,
            panel_offset_x,
            None,
        );
        spawn_current_page(
            &mut commands,
            &asset_server,
            &state,
            state.page,
            panel_width,
            panel_offset_x,
        );
    }
}

pub(crate) fn try_spawn_tutorial_overlay(
    mut commands: Commands,
    ready: Option<Res<TutorialReady>>,
    replay: Option<Res<TutorialReplayRequested>>,
    game_data: Option<Res<GameData>>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    existing: Query<(), With<TutorialUI>>,
    pending_hint: Option<Res<PendingFindBossShrineHint>>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
) {
    let Some(seen_chunks) = seen_chunks else {
        return;
    };
    let first_run = ready.is_some();
    let replay_requested = replay.is_some();
    if !first_run && !replay_requested {
        return;
    }
    if !existing.is_empty() {
        if first_run {
            commands.remove_resource::<TutorialReady>();
        }
        if replay_requested {
            commands.remove_resource::<TutorialReplayRequested>();
        }
        return;
    }

    if first_run {
        let legacy_skip = game_data
            .as_ref()
            .map(|g| g.has_seen_tutorial)
            .unwrap_or(false);
        if legacy_skip {
            schedule_pending_find_boss_shrine_hint(&mut commands, pending_hint);
            commands.remove_resource::<TutorialReady>();
            return;
        }
        commands.remove_resource::<TutorialReady>();
        let start_tips = [TutorialContent::Attacking, TutorialContent::ShrinesExplore];
        let any_unseen_start = start_tips
            .iter()
            .any(|content| !seen_chunks.has_seen(content));
        if any_unseen_start {
            try_send_contextual_popup(&mut popup_events, &seen_chunks, &existing, &start_tips);
        } else {
            schedule_pending_find_boss_shrine_hint(&mut commands, pending_hint);
        }
        return;
    }

    if replay_requested {
        commands.remove_resource::<TutorialReplayRequested>();
        popup_events.send(TutorialPopupEvent {
            entries: TutorialContent::ALL.to_vec(),
            mode: TutorialPopupMode::FullReplay,
        });
    }
}

fn check_heirloom_tutorial(
    skills: Query<&PlayerSkills>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
) {
    let Some(seen_chunks) = seen_chunks else {
        return;
    };
    if seen_chunks.has_seen(&TutorialContent::Heirlooms) {
        return;
    }
    let Ok(skills) = skills.get_single() else {
        return;
    };
    if skills.heirlooms.len() < HEIRLOOM_TUTORIAL_PICK_COUNT {
        return;
    }
    try_send_contextual_popup(
        &mut popup_events,
        &seen_chunks,
        &existing,
        &[TutorialContent::Heirlooms],
    );
}

fn check_biome_timer_tutorial(
    run_timer: Res<RunTimer>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
) {
    let Some(seen_chunks) = seen_chunks else {
        return;
    };
    if seen_chunks.has_seen(&TutorialContent::PinkFlowers) {
        return;
    }
    if run_timer.elapsed_seconds < BIOME_TUTORIAL_DELAY_SECS {
        return;
    }
    try_send_contextual_popup(
        &mut popup_events,
        &seen_chunks,
        &existing,
        &[TutorialContent::PinkFlowers],
    );
}

fn check_map_navigation_tutorial(
    run_timer: Res<RunTimer>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
) {
    let Some(seen_chunks) = seen_chunks else {
        return;
    };
    if seen_chunks.has_seen(&TutorialContent::MapNavigation) {
        return;
    }
    if run_timer.elapsed_seconds < MAP_NAVIGATION_TUTORIAL_DELAY_SECS {
        return;
    }
    try_send_contextual_popup(
        &mut popup_events,
        &seen_chunks,
        &existing,
        &[TutorialContent::MapNavigation],
    );
}

/// Chaos level required before the chaos tutorial appears.
pub const CHAOS_TUTORIAL_THRESHOLD: f32 = 4.0;

fn check_chaos_tutorial(
    chaos_tracker: Option<Res<ChaosTracker>>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
) {
    let (Some(seen_chunks), Some(chaos_tracker)) = (seen_chunks, chaos_tracker) else {
        return;
    };
    if seen_chunks.has_seen(&TutorialContent::Chaos) {
        return;
    }
    if chaos_tracker.get_chaos() < CHAOS_TUTORIAL_THRESHOLD {
        return;
    }
    try_send_contextual_popup(
        &mut popup_events,
        &seen_chunks,
        &existing,
        &[TutorialContent::Chaos],
    );
}

/// Shows the hotbar tutorial the first time all four keyed hotbar slots (keys 1–4) are occupied.
fn check_hotbar_food_tutorial(
    inventory: Query<&Inventory, (With<Player>, Changed<Inventory>)>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
) {
    let (Some(seen_chunks), Ok(inv)) = (seen_chunks, inventory.get_single()) else {
        return;
    };
    if seen_chunks.has_seen(&TutorialContent::HotbarFood) {
        return;
    }
    let hotbar_full = inv
        .items
        .items
        .iter()
        .take(HUD_HOTBAR_SLOTS)
        .all(|slot| slot.is_some());
    if !hotbar_full {
        return;
    }
    try_send_contextual_popup(
        &mut popup_events,
        &seen_chunks,
        &existing,
        &[TutorialContent::HotbarFood],
    );
}

/// Mana level at or below which the mana tutorial appears for the first time.
pub const MANA_TUTORIAL_THRESHOLD: i32 = 5;

/// Shows the mana tutorial the first time the player's current mana drops below
/// [`MANA_TUTORIAL_THRESHOLD`].
fn check_mana_tutorial(
    player_mana: Query<&CurrentMana, (With<Player>, Changed<CurrentMana>)>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
) {
    let (Some(seen_chunks), Ok(mana)) = (seen_chunks, player_mana.get_single()) else {
        return;
    };
    if seen_chunks.has_seen(&TutorialContent::Mana) {
        return;
    }
    if mana.0 >= MANA_TUTORIAL_THRESHOLD {
        return;
    }
    try_send_contextual_popup(
        &mut popup_events,
        &seen_chunks,
        &existing,
        &[TutorialContent::Mana],
    );
}

/// Call when the active skill shrine UI closes (a skill pick has been finalized).
pub fn try_active_skill_shrine_tutorial(
    popup_events: &mut EventWriter<TutorialPopupEvent>,
    seen_chunks: &SeenTutorialChunks,
    existing: &Query<(), With<TutorialUI>>,
) {
    try_send_contextual_popup(
        popup_events,
        seen_chunks,
        existing,
        &[TutorialContent::Skills],
    );
}

/// Call when the equipment loot chest UI opens (not heirloom chests).
pub fn try_equipment_chest_tutorial(
    popup_events: &mut EventWriter<TutorialPopupEvent>,
    seen_chunks: &SeenTutorialChunks,
    existing: &Query<(), With<TutorialUI>>,
) {
    try_send_contextual_popup(
        popup_events,
        seen_chunks,
        existing,
        &[TutorialContent::Equipment],
    );
}

fn process_pending_inventory_tutorials(
    mut commands: Commands,
    pending: Option<Res<PendingInventoryTutorialCheck>>,
    inv: Query<&Inventory>,
    mut popup_events: EventWriter<TutorialPopupEvent>,
    seen_chunks: Option<Res<SeenTutorialChunks>>,
    existing: Query<(), With<TutorialUI>>,
    proto: ProtoParam,
) {
    if pending.is_none() {
        return;
    }
    commands.remove_resource::<PendingInventoryTutorialCheck>();
    let (Some(seen_chunks), Ok(inventory)) = (seen_chunks, inv.get_single()) else {
        return;
    };
    try_inventory_open_tutorials(
        &mut popup_events,
        &seen_chunks,
        &existing,
        inventory,
        &proto,
    );
}

/// Call when the player opens the inventory (from closed) to surface upgrade / stats tutorials.
pub fn try_inventory_open_tutorials(
    popup_events: &mut EventWriter<TutorialPopupEvent>,
    seen_chunks: &SeenTutorialChunks,
    existing: &Query<(), With<TutorialUI>>,
    inventory: &Inventory,
    proto: &ProtoParam,
) {
    let has_tome = inventory.items.items.iter().any(|slot| {
        slot.as_ref()
            .is_some_and(|item| *item.get_obj() == WorldObject::UpgradeTome)
    });
    let has_orb = inventory.items.items.iter().any(|slot| {
        slot.as_ref()
            .is_some_and(|item| *item.get_obj() == WorldObject::OrbOfTransformation)
    });
    if has_tome && has_orb {
        try_send_contextual_popup(
            popup_events,
            seen_chunks,
            existing,
            &[TutorialContent::UpgradeTomes],
        );
    }

    let has_inventory_gear = inventory.items.items.iter().enumerate().any(|(idx, slot)| {
        if idx < 6 {
            return false;
        }
        let Some(item) = slot else {
            return false;
        };
        let obj = item.get_obj();
        obj.get_equip_type(proto).is_some_and(|equip_type| {
            equip_type.is_weapon() || equip_type.is_armor() || equip_type.is_accessory()
        })
    });
    if has_inventory_gear {
        try_send_contextual_popup(
            popup_events,
            seen_chunks,
            existing,
            &[TutorialContent::InventoryStats],
        );
    }
}

/// Call when the player clicks Craft on the inventory side panel.
pub fn try_craft_button_tutorial(
    popup_events: &mut EventWriter<TutorialPopupEvent>,
    seen_chunks: &SeenTutorialChunks,
    existing: &Query<(), With<TutorialUI>>,
) {
    try_send_contextual_popup(
        popup_events,
        seen_chunks,
        existing,
        &[TutorialContent::Crafting],
    );
}

#[allow(clippy::too_many_arguments)]
fn spawn_tutorial_root(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    highlight_materials: &mut Assets<TipHighlightMaterial>,
    res: &ScreenResolution,
    mode: TutorialPopupMode,
    panel_width: f32,
    panel_height: f32,
    panel_offset_x: f32,
    highlight: Option<TipHighlightRect>,
) {
    if let Some(rect) = highlight {
        // Spotlight overlay: dark everywhere except a cutout + glowing border around `rect`,
        // instead of the plain dim below, to draw the player's eye to that part of the HUD.
        let overlay = spawn_tip_highlight_overlay(
            commands,
            meshes,
            highlight_materials,
            res,
            rect,
            Z_TUTORIAL_OVERLAY,
        );
        commands.entity(overlay).insert(TutorialUI);
    } else {
        let use_dim = mode == TutorialPopupMode::FullReplay
            || panel_width > PANEL_WIDTH_SINGLE + f32::EPSILON;
        if use_dim {
            commands.spawn((
                SpriteBundle {
                    sprite: Sprite {
                        color: Color::rgba(0., 0., 0., 0.85),
                        custom_size: Some(Vec2::new(2000., 2000.)),
                        ..default()
                    },
                    transform: Transform::from_translation(Vec3::new(0., 0., Z_TUTORIAL_OVERLAY)),
                    ..default()
                },
                RenderLayers::from_layers(&[3]),
                TutorialUI,
                Name::new("Tutorial Overlay Dim"),
            ));
        }
    }

    let panel_alpha = if mode == TutorialPopupMode::Contextual {
        0.98
    } else {
        0.95
    };
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.15, 0.12, 0.10, panel_alpha),
                custom_size: Some(Vec2::new(panel_width, panel_height)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(panel_offset_x, 0., Z_TUTORIAL_PANEL)),
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Panel"),
    ));

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Tutorial",
                gf::DISPLAY_LARGE.text_style(&asset_server, YELLOW_2),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(
                panel_offset_x,
                panel_height * 0.5 - 24.0,
                Z_TUTORIAL_TEXT,
            ),
                scale: gf::DISPLAY_LARGE.transform_scale(),
                ..Default::default()
            },
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Title"),
    ));

    let button_y = -panel_height * 0.5 + 14.0;
    let btn_spread = panel_width * 0.14;
    if mode == TutorialPopupMode::FullReplay {
        spawn_button(
            commands,
            asset_server,
            Vec3::new(panel_offset_x - btn_spread, button_y, Z_TUTORIAL_CONTENT),
            "Prev",
            TutorialButtonKind::Prev,
        );
        spawn_button(
            commands,
            asset_server,
            Vec3::new(panel_offset_x + btn_spread, button_y, Z_TUTORIAL_CONTENT),
            "Next",
            TutorialButtonKind::Next,
        );
    }
    spawn_button(
        commands,
        asset_server,
        Vec3::new(panel_offset_x, button_y, Z_TUTORIAL_CONTENT),
        "Done",
        TutorialButtonKind::Done,
    );
}

/// Spawns a compact, image-less tutorial box (title + short body + Done button) at
/// `panel_center`, optionally with a spotlight overlay cutting out `highlight`. Used for
/// compact entries (see [`compact_tutorial_layout`]) instead of
/// [`spawn_tutorial_root`]/[`spawn_current_page`].
fn spawn_compact_tutorial(
    commands: &mut Commands,
    asset_server: &AssetServer,
    meshes: &mut Assets<Mesh>,
    highlight_materials: &mut Assets<TipHighlightMaterial>,
    res: &ScreenResolution,
    content: TutorialContent,
    panel_center: Vec2,
    highlight: Option<TipHighlightRect>,
) {
    if let Some(rect) = highlight {
        let overlay = spawn_tip_highlight_overlay(
            commands,
            meshes,
            highlight_materials,
            res,
            rect,
            Z_TUTORIAL_OVERLAY,
        );
        commands.entity(overlay).insert(TutorialUI);
    }

    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.15, 0.12, 0.10, 0.98),
                custom_size: Some(Vec2::new(PANEL_WIDTH_COMPACT, PANEL_HEIGHT_COMPACT)),
                ..default()
            },
            transform: Transform::from_translation(panel_center.extend(Z_TUTORIAL_PANEL)),
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Compact Panel"),
    ));

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                content.title(),
                gf::DISPLAY.text_style(&asset_server, YELLOW_2),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(
                panel_center.x,
                panel_center.y + PANEL_HEIGHT_COMPACT * 0.5 - 14.0,
                Z_TUTORIAL_TEXT,
            ),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Compact Title"),
    ));

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                content.body(),
                gf::BODY.text_style(&asset_server, WHITE),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(
                panel_center.x,
                panel_center.y + 2.0,
                Z_TUTORIAL_TEXT,
            ),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Compact Body"),
    ));

    spawn_button(
        commands,
        asset_server,
        Vec3::new(
            panel_center.x,
            panel_center.y - PANEL_HEIGHT_COMPACT * 0.5 + 12.0,
            Z_TUTORIAL_CONTENT,
        ),
        "Done",
        TutorialButtonKind::Done,
    );
}

fn spawn_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    label: &str,
    kind: TutorialButtonKind,
) {
    let focus_index = match kind {
        TutorialButtonKind::Prev => 0,
        TutorialButtonKind::Next => 1,
        TutorialButtonKind::Done => 2,
    };
    let button_size = Vec2::new(54., 16.);
    let button_e = commands
        .spawn((
            SpriteBundle {
                sprite: Sprite {
                    color: BLACK,
                    custom_size: Some(button_size),
                    ..default()
                },
                transform: Transform::from_translation(pos),
                ..default()
            },
            Interactable::default(),
            RenderLayers::from_layers(&[3]),
            TutorialUI,
            TutorialButton(kind),
            crate::ui::focus::OverlayFocusable {
                index: focus_index,
            },
            Name::new(format!("Tutorial Button: {}", label)),
        ))
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    label,
                    gf::DISPLAY.text_style(&asset_server, WHITE),
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform {
                translation: Vec3::new(0., -1., 1.),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            TutorialUI,
        ))
        .set_parent(button_e);
}

fn column_index_for_entry(entry_i: usize, entry_count: usize) -> usize {
    match entry_count {
        1 => 1,
        2 => {
            if entry_i == 0 {
                0
            } else {
                2
            }
        }
        _ => entry_i.min(ENTRIES_PER_PAGE - 1),
    }
}

fn column_center_x(col: usize, panel_width: f32, panel_offset_x: f32, entry_count: usize) -> f32 {
    let col_w = panel_width / ENTRIES_PER_PAGE as f32;
    let mut x = panel_offset_x + (-panel_width * 0.5 + col_w * (col as f32 + 0.5));
    if entry_count == 2 {
        match col {
            0 => x += DOUBLE_ENTRY_COLUMN_INSET,
            2 => x -= DOUBLE_ENTRY_COLUMN_INSET,
            _ => {}
        }
    }
    x
}

fn page_slice(state: &TutorialState, page_index: usize) -> Vec<TutorialContent> {
    let entries = &state.entries;
    if state.mode == TutorialPopupMode::Contextual {
        return entries.clone();
    }
    let start = page_index * ENTRIES_PER_PAGE;
    let end = (start + ENTRIES_PER_PAGE).min(entries.len());
    if start >= end {
        return Vec::new();
    }
    entries[start..end].to_vec()
}

fn total_pages(state: &TutorialState) -> usize {
    if state.mode == TutorialPopupMode::Contextual {
        return 1;
    }
    (state.entries.len() + ENTRIES_PER_PAGE - 1) / ENTRIES_PER_PAGE
}

fn spawn_current_page(
    commands: &mut Commands,
    asset_server: &AssetServer,
    state: &TutorialState,
    page_index: usize,
    panel_width: f32,
    panel_offset_x: f32,
) {
    let entries = page_slice(state, page_index);
    let n = entries.len();
    for (i, content) in entries.iter().enumerate() {
        let col = column_index_for_entry(i, n);
        let col_x = column_center_x(col, panel_width, panel_offset_x, n);
        spawn_entry_text(commands, asset_server, *content, col_x);
        spawn_entry_visual(commands, asset_server, *content, col_x);
    }
}

fn spawn_entry_text(
    commands: &mut Commands,
    asset_server: &AssetServer,
    content: TutorialContent,
    column_center_x: f32,
) {
    let title_y = CONTENT_BAND_CENTER_Y + 50.0;
    let body_y = CONTENT_BAND_CENTER_Y + 14.0;

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                content.title(),
                gf::DISPLAY.text_style(&asset_server, YELLOW_2),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(
                column_center_x,
                title_y,
                Z_TUTORIAL_TEXT,
            ),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            },
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        TutorialPageEntity,
        content,
        Name::new("Tutorial Entry Title"),
    ));

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                content.body(),
                gf::BODY.text_style(&asset_server, WHITE),
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform {
                translation: Vec3::new(
                column_center_x,
                body_y,
                Z_TUTORIAL_TEXT,
            ),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            },
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        TutorialPageEntity,
        content,
        Name::new("Tutorial Entry Body"),
    ));
}

fn tutorial_content_aseprite_tag(content: TutorialContent) -> &'static str {
    match content {
        TutorialContent::Attacking => TutorialAnims::tags::ATTACK,
        TutorialContent::Skills => TutorialAnims::tags::SKILLS,
        TutorialContent::Heirlooms => TutorialAnims::tags::HEIRLOOMS,
        TutorialContent::Equipment => TutorialAnims::tags::EQUIPMENT,
        TutorialContent::UpgradeTomes => TutorialAnims::tags::TOME,
        TutorialContent::InventoryStats => TutorialAnims::tags::INVENTORY_STATS,
        TutorialContent::ShrinesExplore => TutorialAnims::tags::SHRINE,
        TutorialContent::PinkFlowers => TutorialAnims::tags::PINK_FLOWER,
        TutorialContent::Crafting => TutorialAnims::tags::CRAFTING,
        // Compact tutorials (this one included) never render an aseprite image, so these tags
        // are unused — picked only so this match stays exhaustive.
        TutorialContent::MapNavigation => TutorialAnims::tags::SHRINE,
        TutorialContent::Timeline => TutorialAnims::tags::SHRINE,
        TutorialContent::Chaos => TutorialAnims::tags::SHRINE,
        TutorialContent::HotbarFood => TutorialAnims::tags::SHRINE,
        TutorialContent::Mana => TutorialAnims::tags::SHRINE,
    }
}

fn spawn_entry_visual(
    commands: &mut Commands,
    asset_server: &AssetServer,
    content: TutorialContent,
    column_center_x: f32,
) {
    let mut animation = AsepriteAnimation::from(tutorial_content_aseprite_tag(content));
    animation.play();

    commands.spawn((
        AsepriteBundle {
            aseprite: asset_server.load(TutorialAnims::PATH),
            animation,
            transform: Transform::from_translation(Vec3::new(
                column_center_x,
                TUTORIAL_ANIM_CENTER_Y,
                Z_TUTORIAL_CONTENT,
            ))
            .with_scale(Vec3::splat(TUTORIAL_ASEPRITE_SIZE / 64.0)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        TutorialPageEntity,
        content,
        Name::new("Tutorial Entry Aseprite"),
    ));
}

fn tutorial_button_hit_under_cursor(
    cursor_pos: &Res<CursorPos>,
    query: &Query<(Entity, &Sprite, &GlobalTransform), With<TutorialButton>>,
) -> Option<Entity> {
    if !cursor_pos.ui_hover_hit_allowed() {
        return None;
    }

    let mut ret: Option<Entity> = None;
    for (ent, sprite, xform) in query.iter() {
        let Some(size) = sprite.custom_size else {
            continue;
        };
        let initial_x = xform.translation().x - (0.5 * size.x);
        let initial_y = xform.translation().y - (0.5 * size.y);
        let terminal_x = initial_x + size.x;
        let terminal_y = initial_y + size.y;
        if (initial_x..=terminal_x).contains(&cursor_pos.ui_coords.x)
            && (initial_y..=terminal_y).contains(&cursor_pos.ui_coords.y)
        {
            ret = Some(ent);
        }
    }
    ret
}

#[allow(clippy::too_many_arguments)]
fn handle_tutorial_buttons(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_focus: Res<crate::ui::focus::UiFocus>,
    asset_server: Res<AssetServer>,
    mut button_queries: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<TutorialButton>>,
        Query<(Entity, &mut Interactable, &TutorialButton, &mut Sprite), With<TutorialButton>>,
    )>,
    page_entities: Query<Entity, With<TutorialPageEntity>>,
    all_tutorial: Query<Entity, With<TutorialUI>>,
    mut state: ResMut<TutorialState>,
    mut seen_chunks: Option<ResMut<SeenTutorialChunks>>,
    mut game_data: Option<ResMut<GameData>>,
    pending_hint: Option<Res<PendingFindBossShrineHint>>,
) {
    if button_queries.p1().is_empty() {
        return;
    }
    let hit = {
        let q = button_queries.p0();
        tutorial_button_hit_under_cursor(&cursor_pos, &q)
    };
    let just_clicked = mouse_input.just_released(MouseButton::Left);
    let mut requested: Option<TutorialButtonKind> = None;

    for (e, mut interactable, button, mut sprite) in button_queries.p1().iter_mut() {
        let hovering = matches!(hit, Some(ent) if ent == e);
        let is_focused = ui_focus.is_focused(e);
        let confirm_pressed =
            (hovering && just_clicked) || (is_focused && ui_focus.confirm_just_pressed);
        if hovering || is_focused {
            sprite.color = DARK_GREEN;
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
            }
            if confirm_pressed {
                requested = Some(button.0);
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
            }
        } else {
            sprite.color = BLACK;
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
            }
        }
    }

    let Some(kind) = requested else {
        return;
    };

    match kind {
        TutorialButtonKind::Prev => {
            if state.mode == TutorialPopupMode::FullReplay && state.page > 0 {
                state.page -= 1;
                refresh_page(&mut commands, &asset_server, &page_entities, &state);
            }
        }
        TutorialButtonKind::Next => {
            if state.mode == TutorialPopupMode::FullReplay && state.page + 1 < total_pages(&state) {
                state.page += 1;
                refresh_page(&mut commands, &asset_server, &page_entities, &state);
            }
        }
        TutorialButtonKind::Done => {
            if let Some(seen_chunks) = seen_chunks.as_mut() {
                if state.mode == TutorialPopupMode::Contextual {
                    for content in &state.entries {
                        seen_chunks.mark_seen(*content);
                    }
                    persist_seen_tutorial_chunks(seen_chunks);
                } else {
                    for content in TutorialContent::ALL {
                        seen_chunks.mark_seen(content);
                    }
                    persist_seen_tutorial_chunks(seen_chunks);
                    if let Some(gd) = game_data.as_mut() {
                        gd.has_seen_tutorial = true;
                        gd.seen_tutorial_chunks = seen_chunks.seen.clone();
                    }
                    persist_has_seen_tutorial(true);
                }
            }

            for e in all_tutorial.iter() {
                if let Some(ec) = commands.get_entity(e) {
                    ec.despawn_recursive();
                }
            }
            schedule_pending_find_boss_shrine_hint(&mut commands, pending_hint);
        }
    }
}

fn refresh_page(
    commands: &mut Commands,
    asset_server: &AssetServer,
    page_entities: &Query<Entity, With<TutorialPageEntity>>,
    state: &TutorialState,
) {
    for e in page_entities.iter() {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    spawn_current_page(
        commands,
        asset_server,
        state,
        state.page,
        state.panel_width,
        state.panel_offset_x,
    );
}

fn persist_has_seen_tutorial(value: bool) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        GameData::try_from_json_reader(reader).unwrap_or_default()
    } else {
        GameData::default()
    };

    game_data.has_seen_tutorial = value;

    match File::create(&path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            if let Err(err) = serde_json::to_writer(writer, &game_data) {
                error!("Failed to persist tutorial flag to game_data.json: {err:?}");
            }
        }
        Err(err) => error!("Failed to create game_data.json while saving tutorial flag: {err:?}"),
    }
}
