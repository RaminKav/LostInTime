use std::collections::HashSet;

use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};

use crate::{
    assets::Graphics,
    player::{
        skills::{Heirloom, HeirloomChoiceQueue, HeirloomChoiceState, HeirloomRarity},
        time_crystals::TimeCrystals,
    },
    ui::{
        game_fonts as gf, interactions::Interactable, inventory_ui::UIState,
        time_crystal_progress_ui::CrystalUnlockIcon,
    },
};

/// Marks entities belonging to the time-crystals browser heirloom grid overlay.
#[derive(Component)]
pub struct HeirloomBrowserGridLayer;

/// Marks entities belonging to the dev heirloom picker beside the inventory dev buttons.
#[derive(Component)]
pub struct DevHeirloomPickerGridLayer;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HeirloomGridContext {
    TimeCrystalsBrowser,
    DevHeirloomPicker,
}

pub const GRID_COLS: usize = 7;
pub const GRID_CELL: f32 = 22.;
pub const GRID_ICON: f32 = 16.;
const GRID_Z_BACKDROP: f32 = 98.5;
const GRID_Z_CELL: f32 = 99.5;

fn pool_pairs(pool: &[HeirloomChoiceState]) -> HashSet<(Heirloom, HeirloomRarity)> {
    pool.iter()
        .filter(|s| s.heirloom != Heirloom::None)
        .map(|s| (s.heirloom.clone(), s.rarity.clone()))
        .collect()
}

/// Every heirloom in the full unlock pool, sorted for grid display.
pub fn sorted_full_pool_grid_entries() -> Vec<(Heirloom, HeirloomRarity)> {
    let mut v: Vec<_> = pool_pairs(&HeirloomChoiceQueue::with_all_unlocks().pool)
        .into_iter()
        .collect();
    v.sort_by(|(h1, r1), (h2, r2)| {
        r1.cmp(r2)
            .then_with(|| format!("{h1:?}").cmp(&format!("{h2:?}")))
    });
    v
}

/// Full pool entries with an unlock flag for the time-crystals browser (locked slots show `?`).
pub fn sorted_grid_entries_with_unlock_state(
    time_crystals: &TimeCrystals,
) -> Vec<(Heirloom, HeirloomRarity, bool)> {
    let full = pool_pairs(&HeirloomChoiceQueue::with_all_unlocks().pool);
    let unlocked = pool_pairs(&HeirloomChoiceQueue::new_for_player(time_crystals).pool);
    let mut v: Vec<_> = full
        .into_iter()
        .map(|(h, r)| {
            let is_unlocked = unlocked.contains(&(h.clone(), r.clone()));
            (h, r, is_unlocked)
        })
        .collect();
    v.sort_by(|(h1, r1, _), (h2, r2, _)| {
        r1.cmp(r2)
            .then_with(|| format!("{h1:?}").cmp(&format!("{h2:?}")))
    });
    v
}

pub fn grid_backdrop_size(entry_count: usize, inner_w: f32, inner_h: f32) -> (f32, f32, f32) {
    let rows = (entry_count + GRID_COLS - 1) / GRID_COLS;
    let grid_w = GRID_COLS as f32 * GRID_CELL;
    let backdrop_w = (inner_w * 0.92).min(grid_w + 36.);
    let backdrop_h = (inner_h * 0.78).max(273.);
    (backdrop_w, backdrop_h, grid_w)
}

pub fn despawn_heirloom_browser_grid_layers(
    commands: &mut Commands,
    layers: &Query<Entity, With<HeirloomBrowserGridLayer>>,
) {
    for e in layers.iter() {
        commands.entity(e).despawn();
    }
}

pub fn despawn_dev_heirloom_picker_grid_layers(
    commands: &mut Commands,
    layers: &Query<Entity, With<DevHeirloomPickerGridLayer>>,
) {
    for e in layers.iter() {
        commands.entity(e).despawn();
    }
}

fn insert_grid_layer_markers(
    commands: &mut Commands,
    entity: Entity,
    context: HeirloomGridContext,
) {
    match context {
        HeirloomGridContext::TimeCrystalsBrowser => {
            commands.entity(entity).insert((
                HeirloomBrowserGridLayer,
                UIState::TimeCrystalsBrowser,
                crate::ui::time_crystals_browser_ui::TimeCrystalsBrowserUI,
            ));
        }
        HeirloomGridContext::DevHeirloomPicker => {
            commands
                .entity(entity)
                .insert((DevHeirloomPickerGridLayer, UIState::Inventory));
        }
    }
}

/// Spawns the heirloom icon grid used by the time-crystals browser and the dev heirloom picker.
pub fn spawn_heirloom_grid_overlay(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    center: Vec2,
    inner_w: f32,
    inner_h: f32,
    entries: Vec<(Heirloom, HeirloomRarity, bool)>,
    context: HeirloomGridContext,
    parent: Option<Entity>,
) {
    if entries.is_empty() {
        return;
    }

    let (backdrop_w, backdrop_h, grid_w) = grid_backdrop_size(entries.len(), inner_w, inner_h);

    let backdrop = commands
        .spawn((
            (
                Sprite {
                    color: Color::srgba(35. / 255., 70. / 255., 70. / 255., 1.),
                    custom_size: Some(Vec2::new(backdrop_w, backdrop_h)),
                    ..Default::default()
                },
                Transform::from_translation(Vec3::new(center.x, center.y, GRID_Z_BACKDROP)),
            ),
            RenderLayers::from_layers(&[3]),
            Name::new("Heirloom grid backdrop"),
        ))
        .id();
    insert_grid_layer_markers(commands, backdrop, context);
    if let Some(parent_entity) = parent {
        commands.entity(parent_entity).add_child(backdrop);
    }

    let start_x = center.x - grid_w * 0.5 + GRID_CELL * 0.5;
    let start_y = center.y + backdrop_h * 0.5 - GRID_CELL * 0.65;

    for (i, (heirloom, rarity, is_unlocked)) in entries.into_iter().enumerate() {
        let col = i % GRID_COLS;
        let row = i / GRID_COLS;
        let x = start_x + col as f32 * GRID_CELL;
        let y = start_y - row as f32 * GRID_CELL;

        let show_icon = is_unlocked || context == HeirloomGridContext::DevHeirloomPicker;

        if show_icon {
            let icon = commands
                .spawn((
                    {
                        let mut sprite =
                            graphics.get_heirloom_icon(heirloom.clone());
                        sprite.custom_size = Some(Vec2::new(GRID_ICON, GRID_ICON));
                        sprite
                    },
                    Transform::from_translation(Vec3::new(x, y, GRID_Z_CELL)),
                    RenderLayers::from_layers(&[3]),
                    Interactable::default(),
                    CrystalUnlockIcon {
                        heirloom: heirloom.clone(),
                        rarity: rarity.clone(),
                    },
                    Name::new("Heirloom grid icon"),
                ))
                .id();
            insert_grid_layer_markers(commands, icon, context);
            if let Some(parent_entity) = parent {
                commands.entity(parent_entity).add_child(icon);
            }
        } else {
            let cell = commands
                .spawn((
                    (
                        Sprite {
                            color: Color::srgba(0.08, 0.08, 0.1, 0.55),
                            custom_size: Some(Vec2::new(GRID_CELL - 2., GRID_CELL - 2.)),
                            ..Default::default()
                        },
                        Transform::from_translation(Vec3::new(x, y, GRID_Z_CELL)),
                    ),
                    RenderLayers::from_layers(&[3]),
                    Interactable::default(),
                    Name::new("Heirloom grid locked cell"),
                ))
                .id();
            insert_grid_layer_markers(commands, cell, context);
            if let Some(parent_entity) = parent {
                commands.entity(parent_entity).add_child(cell);
            }
            commands.entity(cell).with_children(|parent| {
                parent.spawn((
                    gf::DISPLAY
                        .text(&asset_server, "?", crate::colors::WHITE)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(0., 0., 1.),
                            scale: gf::DISPLAY.transform_scale(),
                            ..Default::default()
                        }),
                    RenderLayers::from_layers(&[3]),
                ));
            });
        }
    }
}

/// Resolves a pool entry from the full unlock pool (canonical child/clash metadata).
pub fn heirloom_choice_from_full_pool(
    heirloom: Heirloom,
    rarity: HeirloomRarity,
) -> HeirloomChoiceState {
    HeirloomChoiceQueue::with_all_unlocks()
        .pool
        .into_iter()
        .find(|s| s.heirloom == heirloom && s.rarity == rarity)
        .unwrap_or_else(|| HeirloomChoiceState::new(heirloom, rarity))
}
