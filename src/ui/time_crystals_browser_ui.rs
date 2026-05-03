use std::collections::HashSet;

use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{WHITE, YELLOW_2},
    cursor::CursorPos,
    player::{
        skills::{
            time_crystal_heirlooms, Heirloom, HeirloomChoiceQueue, HeirloomChoiceState,
            HeirloomRarity,
        },
        time_crystals::TimeCrystals,
    },
    ui::{
        interactions::{Interactable, Interaction},
        inventory_ui::UIState,
        time_crystal_progress_ui::CrystalUnlockIcon,
        ui_helpers, UIElement,
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

/// Marks every entity belonging to the heirloom grid overlay (backdrop + cells) for teardown.
#[derive(Component)]
pub struct HeirloomBrowserGridLayer;

const OVERLAY_Z: f32 = 95.;
const PANEL_Z: f32 = 96.;
const CONTENT_Z: f32 = 97.;
const ICON_SIZE: f32 = 14.;
const ICON_SPACING: f32 = 20.;
const GRID_COLS: usize = 7;
const GRID_CELL: f32 = 22.;
const GRID_ICON: f32 = 16.;
const GRID_Z_BACKDROP: f32 = 98.5;
const GRID_Z_CELL: f32 = 99.5;

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

fn pool_pairs(pool: &[HeirloomChoiceState]) -> HashSet<(Heirloom, HeirloomRarity)> {
    pool.iter()
        .filter(|s| s.heirloom != Heirloom::None)
        .map(|s| (s.heirloom.clone(), s.rarity.clone()))
        .collect()
}

fn sorted_heirloom_grid_entries(
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

fn despawn_heirloom_grid_layers(
    commands: &mut Commands,
    layers: &Query<Entity, With<HeirloomBrowserGridLayer>>,
) {
    for e in layers.iter() {
        commands.entity(e).despawn_recursive();
    }
}

fn spawn_heirloom_grid_overlay(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    time_crystals: &TimeCrystals,
    inner_w: f32,
    inner_h: f32,
) {
    let entries = sorted_heirloom_grid_entries(time_crystals);
    if entries.is_empty() {
        return;
    }

    let rows = (entries.len() + GRID_COLS - 1) / GRID_COLS;
    let grid_w = GRID_COLS as f32 * GRID_CELL;
    let grid_h = rows as f32 * GRID_CELL;
    let backdrop_w = (inner_w * 0.92).min(grid_w + 36.);
    let backdrop_h = (inner_h * 0.78);

    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(35. / 255., 70. / 255., 70. / 255., 1.),
                custom_size: Some(Vec2::new(backdrop_w, backdrop_h)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 8., GRID_Z_BACKDROP)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        HeirloomBrowserGridLayer,
        Name::new("Heirloom grid backdrop"),
    ));

    let start_x = -grid_w * 0.5 + GRID_CELL * 0.5;
    let start_y = 8. + backdrop_h * 0.5 - GRID_CELL * 0.65;

    for (i, (heirloom, rarity, is_unlocked)) in entries.into_iter().enumerate() {
        let col = i % GRID_COLS;
        let row = i / GRID_COLS;
        let x = start_x + col as f32 * GRID_CELL;
        let y = start_y - row as f32 * GRID_CELL;

        if is_unlocked {
            commands.spawn((
                SpriteSheetBundle {
                    sprite: graphics.get_heirloom_icon(heirloom.clone()),
                    texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                    transform: Transform::from_translation(Vec3::new(x, y, GRID_Z_CELL)),
                    ..Default::default()
                },
                Sprite {
                    custom_size: Some(Vec2::new(GRID_ICON, GRID_ICON)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                TimeCrystalsBrowserUI,
                UIState::TimeCrystalsBrowser,
                HeirloomBrowserGridLayer,
                Interactable::default(),
                CrystalUnlockIcon {
                    heirloom: heirloom.clone(),
                    rarity: rarity.clone(),
                },
                Name::new("Heirloom grid icon"),
            ));
        } else {
            let cell = commands
                .spawn((
                    SpriteBundle {
                        sprite: Sprite {
                            color: Color::rgba(0.08, 0.08, 0.1, 0.55),
                            custom_size: Some(Vec2::new(GRID_CELL - 2., GRID_CELL - 2.)),
                            ..Default::default()
                        },
                        transform: Transform::from_translation(Vec3::new(x, y, GRID_Z_CELL)),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    TimeCrystalsBrowserUI,
                    UIState::TimeCrystalsBrowser,
                    HeirloomBrowserGridLayer,
                    Interactable::default(),
                    // HeirloomGridLockedCell {
                    //     heirloom: heirloom.clone(),
                    //     rarity: rarity.clone(),
                    // },
                    Name::new("Heirloom grid locked cell"),
                ))
                .id();
            commands.entity(cell).with_children(|parent| {
                parent.spawn((
                    Text2dBundle {
                        text: Text::from_section(
                            "?",
                            TextStyle {
                                font: asset_server.load("fonts/alagard.ttf"),
                                font_size: 15.0,
                                color: WHITE,
                            },
                        ),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                ));
            });
        }
    }
}

fn crystal_entry_height(time_crystals: &TimeCrystals, idx: usize) -> f32 {
    if time_crystals.crystals.get(idx).is_none() {
        return 0.;
    }
    let complete = time_crystals.is_complete(idx);
    let nh = time_crystal_heirlooms(idx).len();
    let header = 12f32;
    let body = if complete && nh > 0 {
        ICON_SIZE + 12.
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
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 0.82),
                custom_size: Some(Vec2::new(
                    resolution.game_width + 12.,
                    resolution.game_height + 24.,
                )),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., OVERLAY_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Overlay"),
    ));

    // Large inner panel
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.02, 0.02, 0.04, 0.98),
                custom_size: Some(Vec2::new(inner_w, inner_h)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., PANEL_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Panel"),
    ));

    let half_h = inner_h * 0.5;
    let mut y = half_h - 22.;

    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Time Crystals",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., y, CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        TimeCrystalsBrowserUI,
        UIState::TimeCrystalsBrowser,
        Name::new("Time Crystals Browser Title"),
    ));

    y -= 18.;
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "View progress and unlocks for time crystals.\n\nEarn shards in runs to complete more crystals!",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., y, CONTENT_Z)),
            ..Default::default()
        },
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
                Text2dBundle {
                    text: Text::from_section(
                        line1,
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: YELLOW_2,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(
                        col_x,
                        y_row_header,
                        CONTENT_Z,
                    )),
                    ..Default::default()
                },
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
                        SpriteSheetBundle {
                            sprite: graphics.get_heirloom_icon(heirloom.clone()),
                            texture_atlas: graphics.texture_atlas.as_ref().unwrap().clone(),
                            transform: Transform {
                                translation: Vec3::new(icon_x, sub_y - ICON_SIZE * 0.5, CONTENT_Z),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                        Sprite {
                            custom_size: Some(Vec2::new(ICON_SIZE, ICON_SIZE)),
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
                    Text2dBundle {
                        text: Text::from_section(
                            "—",
                            TextStyle {
                                font: asset_server.load("fonts/4x5.ttf"),
                                font_size: 5.0,
                                color: WHITE,
                            },
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_translation(Vec3::new(
                            col_x,
                            sub_y - 4.,
                            CONTENT_Z,
                        )),
                        ..Default::default()
                    },
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
                    Text2dBundle {
                        text: Text::from_section(
                            mystery,
                            TextStyle {
                                font: asset_server.load("fonts/alagard.ttf"),
                                font_size: 15.0,
                                color: WHITE,
                            },
                        )
                        .with_alignment(TextAlignment::Center),
                        text_anchor: Anchor::Center,
                        transform: Transform::from_translation(Vec3::new(
                            col_x,
                            sub_y - 6.,
                            CONTENT_Z,
                        )),
                        ..Default::default()
                    },
                    RenderLayers::from_layers(&[3]),
                    TimeCrystalsBrowserUI,
                    UIState::TimeCrystalsBrowser,
                    Name::new("Locked unlocks placeholder"),
                ));
            }
        }
        y_row_header -= row_heights[row];
    }

    let button_y = -half_h + 20.;
    let done_entity = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(72., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(-58., button_y, CONTENT_Z)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIElement::MenuButton,
            Interactable::default(),
            TimeCrystalsBrowserDoneButton,
            TimeCrystalsBrowserUI,
            UIState::TimeCrystalsBrowser,
            Name::new("Time Crystals Browser Done"),
        ))
        .id();

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Done",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::TimeCrystalsBrowser)
        .insert(Name::new("Time Crystals Browser Done Text"))
        .set_parent(done_entity);

    let view_entity = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(118., 18.)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(58., button_y, CONTENT_Z)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIElement::MenuButton,
            Interactable::default(),
            TimeCrystalsViewHeirloomsButton,
            TimeCrystalsBrowserUI,
            UIState::TimeCrystalsBrowser,
            Name::new("View Heirlooms"),
        ))
        .id();

    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "View heirlooms",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::TimeCrystalsBrowser)
        .insert(Name::new("View Heirlooms Text"))
        .set_parent(view_entity);
}

pub fn handle_time_crystals_view_heirlooms_button(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<TimeCrystalsViewHeirloomsButton>>,
    mut grid_open: ResMut<TimeCrystalsHeirloomGridOpen>,
    time_crystals: Res<TimeCrystals>,
    resolution: Res<ScreenResolution>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    grid_layers: Query<Entity, With<HeirloomBrowserGridLayer>>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable) in buttons.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        let show = !grid_open.0;
                        despawn_heirloom_grid_layers(&mut commands, &grid_layers);
                        if show {
                            let (inner_w, inner_h) =
                                browser_panel_dimensions(&resolution, &time_crystals);
                            spawn_heirloom_grid_overlay(
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
            },
            _ => {
                interactable.change(Interaction::None);
            }
        }
    }
}

pub fn handle_time_crystals_browser_done_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<TimeCrystalsBrowserDoneButton>>,
    mut next_ui_state: ResMut<NextState<UIState>>,
    mut commands: Commands,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    for (entity, mut interactable) in buttons.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        next_ui_state.set(UIState::Closed);
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            },
            _ => {
                interactable.change(Interaction::None);
            }
        }
    }
}

pub fn cleanup_time_crystals_browser_ui(
    mut commands: Commands,
    query: Query<Entity, With<TimeCrystalsBrowserUI>>,
    tooltips: Query<Entity, With<super::time_crystal_progress_ui::CrystalUnlockTooltip>>,
    mut grid_open: ResMut<TimeCrystalsHeirloomGridOpen>,
) {
    grid_open.0 = false;
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    for entity in tooltips.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
