//! Central typography map: change **[`paths`]** and **[`FontStyle`]** constants here to retheme text
//! across the game. Call sites load handles via **[`FontStyle::load_font`]** / **[`FontStyle::text_style`]**.
//!
//! **Workspace rules:** `4x5` sizes use steps of **5.0**; **Alagard** uses steps of **15.0**.
//! **`slkscr`** body sizes are tuned for crispness at your pixel scale (often **8.4**).

use bevy::prelude::*;

/// Asset path + logical font size (Bevy scales by window `scale_factor` when rasterizing).
#[derive(Clone, Copy, Debug)]
pub struct FontStyle {
    pub path: &'static str,
    pub size: f32,
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
}

/// Raw asset paths — use when you only need the handle or must match legacy string compares.
pub mod paths {
    pub const SLKSCR: &str = "fonts/slkscr.ttf";
    pub const SLKSCR_BOLD: &str = "fonts/slkscrbold.ttf";
    pub const ALAGARD: &str = "fonts/alagard.ttf";
    pub const MONO_4X5: &str = "fonts/4x5.ttf";
    pub const LOOKOUT: &str = "fonts/lookout.ttf";
    pub const PASSAGE: &str = "fonts/passage.ttf";
}

// --- Heirloom choice cards + heirloom tooltip ---------------------------------

pub const HEIRLOOM_CARD_TITLE: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};
pub const HEIRLOOM_CARD_BODY: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};
pub const HEIRLOOM_CARD_META: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};

// --- Skill choice UI (banish tracker, small labels) ---------------------------

pub const SKILL_CHOICE_TRACKER_TITLE: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.5,
};
pub const SKILL_CHOICE_MICRO: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};

// --- HUD active skill panel / generic HUD body --------------------------------

pub const SKILL_PANEL_BODY: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.4,
};
pub const SKILL_PANEL_TITLE_BOLD: FontStyle = FontStyle {
    path: paths::SLKSCR_BOLD,
    size: 8.4,
};

/// World-space vertical advance between skill tooltip description lines ([`crate::ui::player_hud::spawn_skill_tooltip_content`]).
pub const SKILL_TOOLTIP_DESC_LINE_STEP: f32 = 9.0;

pub const HUD_PRIMARY: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.4,
};
pub const HUD_MICRO: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};
pub const HUD_FPS_DEBUG: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};

// --- Item / recipe tooltips -----------------------------------------------------

/// Large card item name (top of tooltip).
pub const TOOLTIP_ITEM_TITLE: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: 15.0,
};
/// Paragraph / attribute lines (`slkscr`).
pub const TOOLTIP_BODY: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.4,
};
/// Section headers like “Base Stats”, “Bonus Stats”, bold emphasis.
pub const TOOLTIP_HEADER_BOLD: FontStyle = FontStyle {
    path: paths::SLKSCR_BOLD,
    size: 8.4,
};
/// Bold subheads on item cards (“Base Stats”, “Description”, “Bonus Stats”) at the card chrome size.
pub const TOOLTIP_CARD_SUBHEAD_BOLD: FontStyle = FontStyle {
    path: paths::SLKSCR_BOLD,
    size: 8.5,
};
/// Non-body lines on item cards (level, rarity, type, set bonus, action summary).
pub const TOOLTIP_CARD_LINE: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.5,
};
/// Recipe ingredient stack counts (`4x5`).
pub const TOOLTIP_RECIPE_COUNT: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};

// --- Inventory “Final Stats” / stat tooltip -----------------------------------

pub const STATS_TOOLTIP_TITLE_ROW: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: 15.0,
};
pub const STATS_TOOLTIP_ROW_NAME: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.4,
};
pub const STATS_TOOLTIP_ROW_VALUE: FontStyle = FontStyle {
    path: paths::SLKSCR,
    size: 8.4,
};

// --- Menus & large UI titles --------------------------------------------------

pub const MENU_TITLE: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: 15.0,
};

// --- World / HUD floating text ------------------------------------------------

/// Default floating combat and pickup labels (`alagard` 15).
pub const FLOATING_TEXT: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: 15.0,
};
/// Compact floating text when "Small damage text" is enabled (`4x5` 5).
pub const FLOATING_TEXT_SMALL: FontStyle = FontStyle {
    path: paths::MONO_4X5,
    size: 5.0,
};
/// Top-center transient announcements (`alagard` 30).
pub const GLOBAL_MESSAGE: FontStyle = FontStyle {
    path: paths::ALAGARD,
    size: 30.0,
};

/// Default logical size for a tooltip font path when building [`crate::ui::tooltips::TooltipTextProps`].
#[inline]
pub fn tooltip_default_size_for_font_path(font_path: &str) -> f32 {
    match font_path {
        p if p == paths::ALAGARD => TOOLTIP_ITEM_TITLE.size,
        _ => TOOLTIP_BODY.size,
    }
}
