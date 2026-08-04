use bevy::text::Justify;
use crate::ui::game_fonts as gf;
use bevy::{camera::visibility::RenderLayers, prelude::*, sprite::Anchor};

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{WHITE, YELLOW_2},
    cursor::CursorPos,
    player::{skills::time_crystal_heirlooms, time_crystals::TimeCrystals},
    ui::{
        focus::FocusInput,
        heirloom_browser_grid::{
            despawn_heirloom_browser_grid_layers, sorted_grid_entries_with_unlock_state,
            spawn_heirloom_grid_overlay, HeirloomBrowserGridLayer, HeirloomGridContext,
        },
        interactions::{Interactable, Interaction},
        inventory_ui::UIState,
        time_crystal_progress_ui::CrystalUnlockIcon,
        ui_helpers, Focusable, UIElement,
    },
    ScreenResolution,
};

/// Root marker for the main-menu Time Crystals overview screen.
#[derive(Component)]
pub struct TimeCrystalsBrowserUI;

#[derive(Component)]
pub struct TimeCrystalsBrowserDoneButton;

#[derive(Component)]
pub struct TimeCrystalsViewHeirloomsButton;

/// When true, the heirloom grid overlay is visible (spawned entities carry [`HeirloomBrowserGridLayer`]).
#[derive(Resource, Default)]
pub struct TimeCrystalsHeirloomGridOpen(pub bool);

const OVERLAY_Z: f32 = 95.;
const PANEL_Z: f32 = 96.;
const CONTENT_Z: f32 = 97.;
const ICON_SIZE: f32 = 14.;
const ICON_SPACING: f32 = 20.;

fn browser_panel_dimensions(
    resolution: &ScreenResolution,
    time_crystals: &TimeCrystals,
) -> (f32, f32) {
    let inner_w = (resolution.game_width * 0.92).min(560.);
    let n = time_crystals.crystals.len();
    let num_rows = (n + 1) / 2;
    let mut row_heights = vec![0f32; num_rows.max(1)];
    for idx in 0..n {
        let row = idx / 2;
        let h = crystal_entry_height(time_crystals, idx);
        row_heights[row] = row_heights[row].max(h);
    }
    let body_height: f32 = row_heights.iter().sum();
    let title_block = 50f32;
    let help_block = 36f32;
    let footer = 46f32;
    let content_h = title_block + help_block + body_height + footer;
    let max_h = (resolution.game_height - 28.).max(200.);
    let inner_h = content_h.min(max_h).max(220.);
    (inner_w, inner_h)
}

fn spawn_heirloom_grid_for_browser(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    time_crystals: &TimeCrystals,
    inner_w: f32,
    inner_h: f32,
) {
    let entries = sorted_grid_entries_with_unlock_state(time_crystals);
    spawn_heirloom_grid_overlay(
        commands,
        asset_server,
        graphics,
        Vec2::new(0., 8.),
        inner_w,
        inner_h,
        entries,
        HeirloomGridContext::TimeCrystalsBrowser,
        None,
    );
}

fn crystal_entry_height(time_crystals: &TimeCrystals, idx: usize) -> f32 {
    if time_crystals.crystals.get(idx).is_none() {
        return 0.;
    }
    let complete = time_crystals.is_complete(idx);
    let nh = time_crystal_heirlooms(idx).len();
    let header = 12f32;
    let body = if complete && nh > 0 {
        ICON_SIZE + 9.
    } else if complete && nh == 0 {
        12.
    } else {
        14.
    };
    header + body + 6.
}

pub fn setup_time_crystals_browser_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
    time_crystals: Res<TimeCrystals>,
    mut grid_open: ResMut<TimeCrystalsHeirloomGridOpen>,
) {
    grid_open.0 = false;
    let (inner_w, inner_h) = browser_panel_dimensions(&resolution, &time_crystals);
    let n = time_crystals.crystals.len();
    let num_rows = (n + 1) / 2;
    let mut row_heights = vec![0f32; num_rows.max(1)];
    for idx in 0..n {
        let row = idx / 2;
        let h = crystal_entry_height(&time_crystals, idx);
        row_heights[row] = row_heights[row].max(h);
    }

    // Full-screen dim
    commands.spawn((
        (
            Sprite {
                color: Color::srgba(0., 0., 0., 0.82),
                custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&resolution)),
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(0., 0., OVERLAY_Z)),
        ),
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Overlay"),
    ));

    // Large inner panel
    commands.spawn((
        (
            Sprite {
                color: Color::srgba(0.02, 0.02, 0.04, 0.98),
                custom_size: Some(Vec2::new(inner_w, inner_h)),
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(0., 0., PANEL_Z)),
        ),
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Panel"),
    ));

    let half_h = inner_h * 0.5;
    let mut y = half_h - 22.;

    commands.spawn((
        gf::DISPLAY
            .text(&asset_server, "Time Crystals", WHITE)
            .justify(Justify::Center)
            .anchor(Anchor::CENTER)
            .with_transform(Transform {
                translation: Vec3::new(0., y, CONTENT_Z),
                scale: gf::DISPLAY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Title"),
    ));

    y -= 18.;
    commands.spawn((
        gf::BODY.text(&asset_server, "View progress and unlocks for time crystals.\n\nEarn shards in runs to complete more crystals!", WHITE).justify(Justify::Center).anchor(Anchor::CENTER).with_transform(Transform {
                translation: Vec3::new(0., y, CONTENT_Z),
                scale: gf::BODY.transform_scale(),
                ..Default::default()
            }),
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Title"),
    ));
    y -= 32.;

    // Even crystal indices (0,2,4,…) → left column; odd (1,3,5,…) → right. Each *row* shares one
    // baseline `y` for both headers, then advances by max(left, right) height for that row.
    let col_dx = (inner_w * 0.24).clamp(30., 132.);
    let mut y_row_header = y;

    for row in 0..num_rows {
        for col in 0..2 {
            let idx = row * 2 + col;
            if idx >= n {
                continue;
            }
            let col_x = if col == 0 { -col_dx } else { col_dx };
            let Some(crystal) = time_crystals.crystals.get(idx) else {
                continue;
            };
            let complete = time_crystals.is_complete(idx);
            let unlocks = time_crystal_heirlooms(idx);
            let n_unlocks = unlocks.len();

            let line1 = format!(
                "Crystal #{} {}/{}{}",
                idx + 1,
                crystal.shards,
                crystal.required_shards.max(1),
                if complete { " (complete)" } else { "" }
            );
            commands.spawn((
                gf::BODY
                    .text(&asset_server, line1, YELLOW_2)
                    .justify(Justify::Center)
                    .anchor(Anchor::CENTER)
                    .with_transform(Transform {
                        translation: Vec3::new(col_x, y_row_header, CONTENT_Z),
                        scale: gf::BODY.transform_scale(),
                        ..Default::default()
                    }),
                RenderLayers::from_layers(&[3]),
                TimeCrystalsBrowserUI,
                UIState::TimeCrystalsBrowser,
                Name::new("Crystal row header"),
            ));

            let sub_y = y_row_header - 12.;

            if complete && n_unlocks > 0 {
                let row_width = (n_unlocks as f32 - 1.) * ICON_SPACING;
                let start_x = col_x - row_width * 0.5;
                for (i, (heirloom, rarity)) in unlocks.into_iter().enumerate() {
                    let icon_x = start_x + i as f32 * ICON_SPACING;
                    commands.spawn((
                        {
                            let mut sprite = graphics.get_heirloom_icon(heirloom.clone());
                            sprite.custom_size = Some(Vec2::new(ICON_SIZE, ICON_SIZE));
                            sprite
                        },
                        Transform {
                            translation: Vec3::new(icon_x, sub_y - ICON_SIZE * 0.5, CONTENT_Z),
                            ..Default::default()
                        },
                        RenderLayers::from_layers(&[3]),
                        TimeCrystalsBrowserUI,
                        UIState::TimeCrystalsBrowser,
                        Interactable::default(),
                        CrystalUnlockIcon {
                            heirloom: heirloom.clone(),
                            rarity: rarity.clone(),
                        },
                        Name::new("Browser crystal unlock icon"),
                    ));
                }
            } else if complete && n_unlocks == 0 {
                commands.spawn((
                    gf::BODY
                        .text(&asset_server, "—", WHITE)
                        .justify(Justify::Center)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(col_x, sub_y - 4., CONTENT_Z),
                            scale: gf::BODY.transform_scale(),
                            ..Default::default()
                        }),
                    RenderLayers::from_layers(&[3]),
                    TimeCrystalsBrowserUI,
                    UIState::TimeCrystalsBrowser,
                    Name::new("No unlocks line"),
                ));
            } else {
                let mystery = if n_unlocks > 0 {
                    std::iter::repeat("?")
                        .take(n_unlocks)
                        .collect::<Vec<_>>()
                        .join("  ")
                } else {
                    "—".to_string()
                };
                commands.spawn((
                    gf::DISPLAY
                        .text(&asset_server, mystery, WHITE)
                        .justify(Justify::Center)
                        .anchor(Anchor::CENTER)
                        .with_transform(Transform {
                            translation: Vec3::new(col_x, sub_y - 6., CONTENT_Z),
                            scale: gf::DISPLAY.transform_scale(),
                            ..Default::default()
                        }),
                    RenderLayers::from_layers(&[3]),
                    TimeCrystalsBrowserUI,
                    UIState::TimeCrystalsBrowser,
                    Name::new("Locked unlocks placeholder"),
                ));
            }
        }
        y_row_header -= row_heights[row];
    }

    let button_y = -half_h + 16.;
    let done_entity = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::MenuButton),
                    custom_size: Some(Vec2::new(72., 18.)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(-58., button_y, CONTENT_Z)),
            ),
            RenderLayers::from_layers(&[3]),
            UIElement::MenuButton,
            Interactable::default(),
            TimeCrystalsBrowserDoneButton,
            TimeCrystalsBrowserUI,
            UIState::TimeCrystalsBrowser,
            Focusable {
                group: UIState::TimeCrystalsBrowser,
                index: 0,
            },
            Name::new("Time Crystals Browser Done"),
        ))
        .id();

    commands
        .spawn(
            gf::BODY
                .text(&asset_server, "Done", crate::colors::WHITE)
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::TimeCrystalsBrowser)
        .insert(Name::new("Time Crystals Browser Done Text2d"))
        .insert(ChildOf(done_entity));

    let view_entity = commands
        .spawn((
            (
                Sprite {
                    image: graphics.get_ui_element_texture(UIElement::MenuButton),
                    custom_size: Some(Vec2::new(96., 18.)),
                    ..default()
                },
                Transform::from_translation(Vec3::new(58., button_y, CONTENT_Z)),
            ),
            RenderLayers::from_layers(&[3]),
            UIElement::MenuButton,
            Interactable::default(),
            TimeCrystalsViewHeirloomsButton,
            TimeCrystalsBrowserUI,
            UIState::TimeCrystalsBrowser,
            Focusable {
                group: UIState::TimeCrystalsBrowser,
                index: 1,
            },
            Name::new("View Heirlooms"),
        ))
        .id();

    commands
        .spawn(
            gf::BODY
                .text(
                    &asset_server, // Trailing space: Bevy 0.10 derives centered-layout pivot from `TextLayoutInfo.size`
                    // (pipeline.rs: glyph positions + advances) but rasterized sprite positions are built
                    // after `GlyphPlacementAdjuster` rounds each glyph's baseline X (glyph_brush.rs).
                    // Those disagree slightly on scale_factor=1; an extra advance widens `size` and fixes
                    // collapsed pairs ("ei", etc.). Harmless visually — NBSP would also work.
                    "View heirlooms ",
                    crate::colors::WHITE,
                )
                .justify(Justify::Center)
                .anchor(Anchor::CENTER)
                .with_transform(Transform {
                    translation: Vec3::new(0., 0.5, 1.),
                    scale: gf::BODY.transform_scale(),
                    ..Default::default()
                }),
        )
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::TimeCrystalsBrowser)
        .insert(Name::new("View Heirlooms Text2d"))
        .insert(ChildOf(view_entity));
}

pub fn handle_time_crystals_view_heirlooms_button(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<TimeCrystalsViewHeirloomsButton>>,
    mut grid_open: ResMut<TimeCrystalsHeirloomGridOpen>,
    time_crystals: Res<TimeCrystals>,
    resolution: Res<ScreenResolution>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    grid_layers: Query<Entity, With<HeirloomBrowserGridLayer>>,
    focus_input: FocusInput,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);
    // Keyboard/gamepad focus only counts while it's actually driving the UI, and a mouse release
    // only counts as a click on this button if the mouse was actually over it — otherwise a
    // default-focused button would both show a stuck hover *and* fire on any click anywhere.
    let focus_driving = mouseless.0 || cursor_pos.suppress_ui_hover;

    for (entity, mut interactable) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_driving && focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        let show = !grid_open.0;
                        despawn_heirloom_browser_grid_layers(&mut commands, &grid_layers);
                        if show {
                            let (inner_w, inner_h) =
                                browser_panel_dimensions(&resolution, &time_crystals);
                            spawn_heirloom_grid_for_browser(
                                &mut commands,
                                &asset_server,
                                &graphics,
                                &time_crystals,
                                inner_w,
                                inner_h,
                            );
                        }
                        grid_open.0 = show;
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            interactable.change(Interaction::None);
        }
    }
}

pub fn handle_time_crystals_browser_done_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<TimeCrystalsBrowserDoneButton>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
    focus_input: FocusInput,
    mouseless: Res<crate::inputs::MouselessModeState>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);
    let focus_driving = mouseless.0 || cursor_pos.suppress_ui_hover;

    for (entity, mut interactable) in buttons.iter_mut() {
        let is_hit = matches!(hit_test, Some(hit) if hit.0 == entity);
        let is_focused = focus_driving && focus_input.is_focused(entity);
        if is_hit || is_focused {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if (is_hit && left_mouse_released)
                        || (is_focused && focus_input.confirm_just_pressed())
                    {
                        next_ui_state.set(UIState::Closed);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else {
            interactable.change(Interaction::None);
        }
    }
}

pub fn cleanup_time_crystals_browser_ui(
    mut commands: Commands,
    query: Query<Entity, With<TimeCrystalsBrowserUI>>,
    tooltips: Query<Entity, With<super::heirloom_tooltip::HeirloomDynamicTooltip>>,
    mut grid_open: ResMut<TimeCrystalsHeirloomGridOpen>,
    grid_layers: Query<Entity, With<HeirloomBrowserGridLayer>>,
) {
    grid_open.0 = false;
    despawn_heirloom_browser_grid_layers(&mut commands, &grid_layers);
    for entity in query.iter() {
        commands.entity(entity).despawn();
    }
    for entity in tooltips.iter() {
        commands.entity(entity).despawn();
    }
}
