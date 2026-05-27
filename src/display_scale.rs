//! Player-controlled UI / world integer zoom steps, relative to the legacy `ZOOM_SCALE = 1.2`
//! reference bucket at the current window height.

use std::fs::File;
use std::io::{BufReader, BufWriter};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::datafiles;

/// Legacy reference zoom (`300 * 1.2 = 360` target height). The reference integer scale at a
/// given window height is `floor(window_height / 360)`.
pub const REFERENCE_ZOOM_SCALE: f32 = 1.2;
pub const REFERENCE_VIEW_TARGET_HEIGHT: f32 = 300.0 * REFERENCE_ZOOM_SCALE;

/// How many integer-scale steps the player can zoom **in** past the reference (game camera).
pub const GAME_ZOOM_IN_STEPS: i8 = 2;
/// UI can zoom **out** one integer step below reference (`Large` = reference, `Small` = -1).
pub const UI_ZOOM_OUT_STEPS: i8 = 1;

#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
pub struct DisplayScaleSettings {
    /// Relative to [`reference_scale`]: `0` = reference zoom; `+1` / `+2` = further in.
    pub game_zoom_steps: i8,
    /// Relative to reference: `0` = `Large` (legacy 1.2 UI); `-1` = `Small` (one step out).
    pub ui_zoom_steps: i8,
}

impl Default for DisplayScaleSettings {
    fn default() -> Self {
        Self {
            game_zoom_steps: 0,
            ui_zoom_steps: 0,
        }
    }
}

impl DisplayScaleSettings {
    pub fn load() -> Self {
        let path = datafiles::game_data();
        if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            if let Ok(game_data) = crate::client::GameData::try_from_json_reader(reader) {
                return game_data.display_scale.unwrap_or_default().sanitized();
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let path = datafiles::game_data();
        let mut game_data = if let Ok(file) = File::open(&path) {
            let reader = BufReader::new(file);
            crate::client::GameData::try_from_json_reader(reader).unwrap_or_default()
        } else {
            crate::client::GameData::default()
        };

        game_data.display_scale = Some(self.clone().sanitized());

        if let Ok(file) = File::create(&path) {
            let _ = serde_json::to_writer_pretty(file, &game_data);
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            game_zoom_steps: self.game_zoom_steps.clamp(0, GAME_ZOOM_IN_STEPS),
            ui_zoom_steps: self.ui_zoom_steps.clamp(-UI_ZOOM_OUT_STEPS, 0),
        }
    }

    pub fn clamped_game_steps(&self) -> i8 {
        self.game_zoom_steps.clamp(0, GAME_ZOOM_IN_STEPS)
    }

    pub fn clamped_ui_steps(&self) -> i8 {
        self.ui_zoom_steps.clamp(-UI_ZOOM_OUT_STEPS, 0)
    }

    pub fn game_scale_for_window(&self, window_height: f32) -> u32 {
        scale_with_steps(reference_scale(window_height), self.clamped_game_steps())
    }

    pub fn ui_scale_for_window(&self, window_height: f32) -> u32 {
        scale_with_steps(reference_scale(window_height), self.clamped_ui_steps())
    }

    /// Player-facing game zoom label: `1` = reference, `2` / `3` = further in.
    pub fn game_zoom_display_level(&self) -> u8 {
        (self.clamped_game_steps() + 1) as u8
    }

    pub fn format_game_zoom_display(&self) -> String {
        format!("{}", self.game_zoom_display_level())
    }

    /// Player-facing UI size label.
    pub fn ui_size_label(&self) -> &'static str {
        if self.clamped_ui_steps() < 0 {
            "Sml"
        } else {
            "Lrg"
        }
    }

    pub fn format_ui_zoom_display(&self) -> String {
        self.ui_size_label().to_string()
    }

    pub fn nudge_game(&mut self, zoom_in: bool) {
        let steps = self.clamped_game_steps();
        self.game_zoom_steps = if zoom_in {
            (steps + 1).min(GAME_ZOOM_IN_STEPS)
        } else {
            (steps - 1).max(0)
        };
    }

    /// Toggle between `Large` (reference) and `Small` (one step out). No UI zoom-in.
    pub fn nudge_ui(&mut self, zoom_in: bool) {
        let steps = self.clamped_ui_steps();
        self.ui_zoom_steps = if zoom_in {
            0
        } else {
            (-UI_ZOOM_OUT_STEPS).max(steps - 1)
        };
    }
}

/// Reference integer pixel scale for the legacy `ZOOM_SCALE = 1.2` setup at `window_height`.
pub fn reference_scale(window_height: f32) -> u32 {
    (window_height / REFERENCE_VIEW_TARGET_HEIGHT)
        .floor()
        .max(1.0) as u32
}

fn scale_with_steps(reference: u32, steps: i8) -> u32 {
    ((reference as i32) + steps as i32).max(1) as u32
}
