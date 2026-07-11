//! Central typography map: change **[`paths`]** and role **[`FontStyle`]** constants here to
//! retheme text across the game (and later swap fonts for localization).
//!
//! Call sites load handles via **[`FontStyle::load_font`]** / **[`FontStyle::text_style`]**,
//! and apply visual size with **[`FontStyle::transform_scale`]** on the text entity `Transform`.
//!
//! **Workspace rules:** Alagard logical sizes use steps of **15.0** (or **30.0** for large
//! display). Visual size is controlled by **`scale`** (`1.0` / `0.7` / `0.5`). Scaled-down
//! roles use [`ALAGARD_SCALED_ATLAS_SIZE`] so they do not share Bevy font atlases with
//! scale-1.0 display text (keeps titles nearest-crisp while body text can use linear).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Floating combat / pickup label size selected in options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DamageTextSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl DamageTextSize {
    #[inline]
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "Sml",
            Self::Medium => "Med",
            Self::Large => "Lrg",
        }
    }

    #[inline]
    pub fn font_style(self) -> FontStyle {
        match self {
            Self::Small => FLOATING_TEXT_SMALL,
            Self::Medium => FLOATING_TEXT,
            Self::Large => FLOATING_TEXT_LARGE,
        }
    }

    #[inline]
    pub fn nudge(self, up: bool) -> Self {
        match (self, up) {
            (Self::Small, true) => Self::Medium,
            (Self::Medium, true) => Self::Large,
            (Self::Medium, false) => Self::Small,
            (Self::Large, false) => Self::Medium,
            (Self::Small, false) | (Self::Large, true) => self,
        }
    }
}

/// Asset path + logical font size + transform scale (Bevy also scales by window `scale_factor`
/// when rasterizing).
#[derive(Clone, Copy, Debug)]
pub struct FontStyle {
    pub path: &'static str,
    pub size: f32,
    /// Uniform `Transform` scale applied to the text entity.
    pub scale: f32,
}

impl FontStyle {
    #[inline]
    pub fn load_font(&self, asset_server: &AssetServer) -> Handle<Font> {
        asset_server.load(self.path)
    }

    #[inline]
    pub fn text_style(&self, asset_server: &AssetServer, color: Color) -> TextStyle {
        TextStyle {
            font: self.load_font(asset_server),
            font_size: self.size,
            color,
        }
    }

    #[inline]
    pub fn transform_scale(&self) -> Vec3 {
        Vec3::splat(self.scale)
    }
}

/// Raw asset paths — use when you only need the handle or must match legacy string compares.
/// Swap these (or role definitions below) to retheme / localize without touching call sites.
pub mod paths {
    pub const SLKSCR: &str = "fonts/slkscr.ttf";
    pub const SLKSCR_BOLD: &str = "fonts/slkscrbold.ttf";
    pub const ALAGARD: &str = "fonts/alagard.ttf";
    pub const MONO_4X5: &str = "fonts/4x5.ttf";
    pub const LOOKOUT: &str = "fonts/lookout.ttf";
    pub const PASSAGE: &str = "fonts/passage.ttf";
}

// --- Semantic roles (prefer these; feature aliases below point here) ----------

/// Logical Alagard size for scale-1.0 display text (crisp nearest sampling).
pub const ALAGARD_DISPLAY_SIZE: f32 = 15.0;
/// Bevy keys font atlases by `font_size`. Scaled-down roles use a nudged size so they get a
/// **separate** atlas that can use linear filtering without blurring scale-1.0 titles
/// (e.g. main-menu "Enter") that share the same font file.
pub const ALAGARD_SCALED_ATLAS_SIZE: f32 = ALAGARD_DISPLAY_SIZE + 1.0 / 32.0;

/// Big menu / screen titles.
pub const DISPLAY_LARGE: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: 30.0,
    scale: 1.0,
};

/// Standard titles, HUD currency, floating combat text.
pub const DISPLAY: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_DISPLAY_SIZE,
    scale: 1.0,
};

/// Mid titles (e.g. heirloom card title).
pub const TITLE: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.7,
};

/// Descriptions and all former `4x5` / `slkscr` / `slkscrbold` body text.
pub const BODY: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.5,
};

pub const HEIRLOOM_BODY: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.56,
};

/// Tiny labels / counts. Same as [`BODY`] for now; kept separate so it can diverge later.
pub const MICRO: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.5,
};

/// Specialty display font used for achievement names.
pub const ACHIEVEMENT_NAME: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.7,
};

// --- Heirloom choice cards + heirloom tooltip ---------------------------------

pub const HEIRLOOM_CARD_TITLE: FontStyle = TITLE;
pub const HEIRLOOM_CARD_BODY: FontStyle = HEIRLOOM_BODY;
pub const HEIRLOOM_CARD_META: FontStyle = BODY;

/// Vertical advance between heirloom card description lines ([`crate::ui::heirloom_tooltip::spawn_heirloom_tooltip_card`]).
pub const HEIRLOOM_CARD_DESC_LINE_STEP: f32 = 9.0;
/// Card sprite height for heirloom choice frames ([`crate::ui::SKILLS_CHOICE_UI_SIZE`].y).
pub const HEIRLOOM_CARD_HEIGHT: f32 = 192.0;
/// Description panel top edge: fraction of card height measured from the card's top edge.
pub const HEIRLOOM_DESC_BOX_TOP_FROM_CARD_TOP: f32 = 0.49;
/// Description panel height (`assets/ui/SmallDescriptionBox.png`).
pub const HEIRLOOM_DESC_BOX_HEIGHT: f32 = 34.0;
/// Inset below the inner top edge of the description panel before stacking lines.
pub const HEIRLOOM_DESC_BOX_TOP_PADDING: f32 = 12.0;

/// Card-local Y of the description panel's top inner edge (Bevy +Y up, origin at card center).
#[inline]
pub fn heirloom_desc_box_top_y() -> f32 {
    let half_h = HEIRLOOM_CARD_HEIGHT * 0.5;
    half_h - HEIRLOOM_DESC_BOX_TOP_FROM_CARD_TOP * HEIRLOOM_CARD_HEIGHT
}

/// Target Y to vertically center description lines inside the panel (below [`HEIRLOOM_DESC_BOX_TOP_PADDING`]).
#[inline]
pub fn heirloom_desc_text_center_y() -> f32 {
    let top = heirloom_desc_box_top_y();
    let bottom = top - HEIRLOOM_DESC_BOX_HEIGHT;
    let box_center = (top + bottom) * 0.5;
    box_center - HEIRLOOM_DESC_BOX_TOP_PADDING
}

/// Y of the first description line before multi-line centering (top of stack inside the panel).
#[inline]
pub fn heirloom_desc_first_line_y() -> f32 {
    heirloom_desc_box_top_y() - HEIRLOOM_DESC_BOX_TOP_PADDING
}

// --- Skill choice UI (banish tracker, small labels) ---------------------------

pub const SKILL_CHOICE_TRACKER_TITLE: FontStyle = BODY;
pub const SKILL_CHOICE_MICRO: FontStyle = MICRO;

// --- HUD active skill panel / generic HUD body --------------------------------

pub const SKILL_PANEL_BODY: FontStyle = BODY;
pub const SKILL_PANEL_TITLE_BOLD: FontStyle = BODY;

/// World-space vertical advance between skill tooltip description lines ([`crate::ui::player_hud::spawn_skill_tooltip_content`]).
pub const SKILL_TOOLTIP_DESC_LINE_STEP: f32 = 9.0;

pub const HUD_PRIMARY: FontStyle = BODY;
/// Time-fragment / coin counts in the HUD currency backgrounds.
pub const HUD_CURRENCY_COUNT: FontStyle = DISPLAY;
/// Current objective line inside the progress background (left side).
pub const HUD_OBJECTIVE: FontStyle = DISPLAY;
/// Score + chaos stack inside the progress background (right side).
pub const HUD_PROGRESS_STAT: FontStyle = BODY;
pub const HUD_MICRO: FontStyle = MICRO;
/// Short icon hover labels (inventory trash / sort / filter, etc.).
pub const ICON_HOVER_TOOLTIP: FontStyle = BODY;
/// Side glossary / trigger info boxes beside tooltip cards.
pub const TOOLTIP_INFO_BOX: FontStyle = BODY;
pub const HUD_FPS_DEBUG: FontStyle = MICRO;

// --- Item / recipe tooltips -----------------------------------------------------

/// Large card item name (top of tooltip).
pub const TOOLTIP_ITEM_TITLE: FontStyle = DISPLAY;
/// Paragraph / attribute lines.
pub const TOOLTIP_BODY: FontStyle = BODY;
/// Section headers like “Base Stats”, “Bonus Stats” (formerly bold; same as body for now).
pub const TOOLTIP_HEADER_BOLD: FontStyle = BODY;
/// Bold subheads on item cards (“Base Stats”, “Description”, “Bonus Stats”) at the card chrome size.
pub const TOOLTIP_CARD_SUBHEAD_BOLD: FontStyle = BODY;
/// Non-body lines on item cards (level, rarity, type, set bonus, action summary).
pub const TOOLTIP_CARD_LINE: FontStyle = BODY;
/// Recipe ingredient stack counts.
pub const TOOLTIP_RECIPE_COUNT: FontStyle = MICRO;

// --- Inventory “Final Stats” / stat tooltip -----------------------------------

pub const STATS_TOOLTIP_TITLE_ROW: FontStyle = DISPLAY;
pub const STATS_TOOLTIP_ROW_NAME: FontStyle = BODY;
pub const STATS_TOOLTIP_ROW_VALUE: FontStyle = BODY;

// --- Menus & large UI titles --------------------------------------------------

pub const MENU_TITLE: FontStyle = DISPLAY;
pub const MENU_TITLE_LARGE: FontStyle = DISPLAY_LARGE;

// --- World / HUD floating text ------------------------------------------------

/// Default floating combat and pickup labels (Alagard @ 0.75).
pub const FLOATING_TEXT: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.75,
};
/// Compact floating text (Alagard @ 0.5).
pub const FLOATING_TEXT_SMALL: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_SCALED_ATLAS_SIZE,
    scale: 0.5,
};
/// Large floating text (Alagard @ 1.0).
pub const FLOATING_TEXT_LARGE: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: ALAGARD_DISPLAY_SIZE,
    scale: 1.0,
};
/// Top-center transient announcements.
pub const GLOBAL_MESSAGE: FontStyle = DISPLAY_LARGE;
/// Subtitle line under [`GLOBAL_MESSAGE`].
pub const GLOBAL_MESSAGE_SUBTEXT: FontStyle = DISPLAY;

/// Default logical size for a tooltip font path when building [`crate::ui::tooltips::TooltipTextProps`].
#[inline]
pub fn tooltip_default_size_for_font_path(font_path: &str) -> f32 {
    match font_path {
        p if p == paths::ALAGARD => TOOLTIP_ITEM_TITLE.size,
        _ => TOOLTIP_BODY.size,
    }
}
