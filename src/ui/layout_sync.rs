//! Re-anchor screen-space UI when the UI camera's pixel scale / view size changes.

use bevy::prelude::*;

use crate::ScreenResolution;

use super::options_ui::OptionsUiLayoutRevision;
use super::UIState;

/// Tracks the UI-camera layout bucket so we can rebuild/re-anchor without waiting for a
/// menu close + reopen.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiLayoutKey {
    pub scale: u32,
    pub render_width: u32,
    pub render_height: u32,
}

impl UiLayoutKey {
    pub fn from_resolution(res: &ScreenResolution) -> Self {
        Self {
            scale: res.scale,
            render_width: res.render_width,
            render_height: res.render_height,
        }
    }
}

/// Last UI layout key successfully applied to HUD reposition passes.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiLayoutSyncState {
    pub applied: Option<UiLayoutKey>,
}

/// Returns `true` when HUD elements still reflect a stale UI layout bucket.
pub fn ui_layout_needs_sync(res: &ScreenResolution, state: &UiLayoutSyncState) -> bool {
    state.applied.as_ref() != Some(&UiLayoutKey::from_resolution(res))
}

pub fn commit_ui_layout_sync(res: &ScreenResolution, state: &mut UiLayoutSyncState) {
    state.applied = Some(UiLayoutKey::from_resolution(res));
}

/// Bumps [`OptionsUiLayoutRevision`] when the UI layout key changes while the options menu
/// is open, so `cleanup_options_ui` / `setup_options_ui` rebuild at the new scale.
pub fn bump_options_ui_revision_on_ui_layout_change(
    resolution: Res<ScreenResolution>,
    ui_state: Res<State<UIState>>,
    mut revision: ResMut<OptionsUiLayoutRevision>,
    mut last_key: Local<Option<UiLayoutKey>>,
) {
    let key = UiLayoutKey::from_resolution(&resolution);
    if last_key.as_ref() == Some(&key) {
        return;
    }
    let had_previous = last_key.is_some();
    *last_key = Some(key);
    if had_previous && *ui_state == UIState::Options {
        revision.0 = revision.0.wrapping_add(1);
    }
}

pub fn commit_ui_layout_sync_system(
    res: Res<ScreenResolution>,
    mut sync_state: ResMut<UiLayoutSyncState>,
) {
    if ui_layout_needs_sync(&res, &sync_state) {
        commit_ui_layout_sync(&res, &mut sync_state);
    }
}
