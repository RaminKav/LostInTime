use bevy::{prelude::*, render::view::RenderLayers, sprite::Anchor};

use crate::{
    assets::Graphics,
    audio::{AudioSoundEffect, SoundSpawner},
    colors::{DARK_WOOD_BROWN, GREY, RED, WHITE, YELLOW_2},
    cursor::CursorPos,
    enemy::Mob,
    item::WorldObject,
    juice::bounce::BounceOnHit,
    player::beastiary::{
        card_for_mob, mob_display_name, Beastiary, BeastiaryEntry, BEASTIARY_MOBS,
    },
    proto::proto_param::ProtoParam,
    ui::{
        interactions::{Interactable, Interaction},
        inventory_ui::UIState,
        ui_helpers, UIElement,
    },
    ScreenResolution,
};

/// Marker for every entity belonging to the bestiary browser overlay.
#[derive(Component)]
pub struct BeastiaryBrowserUI;

/// Done button on the bestiary browser.
#[derive(Component)]
pub struct BeastiaryBrowserDoneButton;

/// One clickable card cell in the 3x3 grid.
#[derive(Component, Clone, Copy)]
pub struct BeastiaryCardCell {
    pub mob_index: usize,
}

/// Prev-page control on the left side of the bestiary book.
#[derive(Component)]
pub struct BeastiaryBrowserPrevButton;

/// Next-page control on the left side of the bestiary book.
#[derive(Component)]
pub struct BeastiaryBrowserNextButton;

/// Current page of the 3x3 card grid (9 mobs per page).
#[derive(Resource, Default, Debug)]
pub struct BeastiaryPagination {
    pub page: usize,
}

/// Marker for entities composing the right-side detail panel. Cleared and
/// rebuilt whenever [`SelectedBeastiaryMob`] changes.
#[derive(Component)]
pub struct BeastiaryDetailPanel;

/// Currently-highlighted card on the bestiary browser. `None` means no card
/// has been clicked yet (detail panel shows hint text).
#[derive(Resource, Default, Debug, Clone)]
pub struct SelectedBeastiaryMob(pub Option<Mob>);

const OVERLAY_Z: f32 = 95.;
const PANEL_Z: f32 = 96.;
const CONTENT_Z: f32 = 97.;

const PANEL_W: f32 = 482.;
const PANEL_H: f32 = 344.;
const GRID_COLS: usize = 3;
const CARDS_PER_PAGE: usize = GRID_COLS * GRID_COLS;
/// Card slot background native size (`BestiaryCardBackground.png`).
const CARD_W: f32 = 46.;
const CARD_H: f32 = 66.;
/// Mob card sprite drawn inside the slot background (2:3 aspect).
const CARD_SPRITE_W: f32 = 38.;
const CARD_SPRITE_H: f32 = 57.;
/// Spacing between cards on the grid (sum of card dim and gap).
const GRID_CELL_W: f32 = CARD_W + 18.;
const GRID_CELL_H: f32 = CARD_H + 12.;
const NAV_BTN_W: f32 = 47.;
const NAV_BTN_H: f32 = 27.;
/// Layout offsets tuned to `Bestiary.png` art (panel centered at origin).
const GRID_CENTER_X: f32 = -112.;
const GRID_TOP_Y: f32 = 74.;
const NAV_BTNS_Y: f32 = -164.;
const NAV_BTN_GAP: f32 = 24.;
const DONE_BTN_W: f32 = 60.;
const DONE_BTN_H: f32 = 18.;
/// Bottom-right inset on the book panel art (`Bestiary.png`).
const DONE_BTN_OFFSET_X: f32 = 56.;
const DONE_BTN_INSET_Y: f32 = 8.;
const DETAIL_CENTER_X: f32 = 110.;
const DETAIL_TOP_Y: f32 = 0.;
const DETAIL_WIDTH: f32 = 170.;
const CARD_BOUNCE_STRENGTH: f32 = 0.5;

fn beastiary_total_pages() -> usize {
    BEASTIARY_MOBS.len().div_ceil(CARDS_PER_PAGE)
}

pub fn setup_beastiary_browser_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    resolution: Res<ScreenResolution>,
    beastiary: Res<Beastiary>,
    mut selected: ResMut<SelectedBeastiaryMob>,
    mut pagination: ResMut<BeastiaryPagination>,
    proto: ProtoParam,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    selected.0 = None;
    pagination.page = 0;

    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 0.82),
                custom_size: Some(crate::ui::ui_helpers::full_screen_overlay_size(&resolution)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., OVERLAY_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        Name::new("Beastiary Browser Overlay"),
    ));

    commands.spawn((
        SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::Bestiary),
            sprite: Sprite {
                custom_size: Some(Vec2::new(PANEL_W, PANEL_H)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., PANEL_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        Name::new("Beastiary Browser Panel"),
    ));

    // Grid cells are spawned by `refresh_beastiary_grid_on_pagination_change` so we
    // never double-spawn in the same frame as setup (deferred commands).

    let nav_center_x = GRID_CENTER_X;
    let prev_x = nav_center_x - NAV_BTN_W * 0.5 - NAV_BTN_GAP * 0.5;
    let next_x = nav_center_x + NAV_BTN_W * 0.5 + NAV_BTN_GAP * 0.5;

    commands.spawn((
        SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::BestiaryPrevButton),
            sprite: Sprite {
                custom_size: Some(Vec2::new(NAV_BTN_W, NAV_BTN_H)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(prev_x, NAV_BTNS_Y, CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UIElement::BestiaryPrevButton,
        Interactable::default(),
        BeastiaryBrowserPrevButton,
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        Name::new("Beastiary Browser Prev"),
    ));

    commands.spawn((
        SpriteBundle {
            texture: graphics.get_ui_element_texture(UIElement::BestiaryNextButton),
            sprite: Sprite {
                custom_size: Some(Vec2::new(NAV_BTN_W, NAV_BTN_H)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(next_x, NAV_BTNS_Y, CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        UIElement::BestiaryNextButton,
        Interactable::default(),
        BeastiaryBrowserNextButton,
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        Name::new("Beastiary Browser Next"),
    ));

    // Done button — bottom-right on the book panel, above the panel art layer.
    let button_x = PANEL_W * 0.5 - DONE_BTN_W * 0.5 + DONE_BTN_OFFSET_X;
    let button_y = -PANEL_H * 0.5 + DONE_BTN_H * 0.5 + DONE_BTN_INSET_Y;
    let done_entity = commands
        .spawn((
            SpriteBundle {
                texture: graphics.get_ui_element_texture(UIElement::MenuButton),
                sprite: Sprite {
                    custom_size: Some(Vec2::new(DONE_BTN_W, DONE_BTN_H)),
                    ..Default::default()
                },
                transform: Transform::from_translation(Vec3::new(
                    button_x,
                    button_y,
                    CONTENT_Z + 1.,
                )),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            UIElement::MenuButton,
            Interactable::default(),
            BeastiaryBrowserDoneButton,
            BeastiaryBrowserUI,
            UIState::BeastiaryBrowser,
            Name::new("Beastiary Browser Done"),
        ))
        .id();
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                "Done",
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: crate::colors::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(0., 0.5, 1.)),
            ..Default::default()
        })
        .insert(RenderLayers::from_layers(&[3]))
        .insert(UIState::BeastiaryBrowser)
        .insert(Name::new("Beastiary Browser Done Text"))
        .set_parent(done_entity);

    // Detail panel placeholder (no selection yet).
    spawn_detail_panel(
        &mut commands,
        &asset_server,
        &graphics,
        &beastiary,
        &proto,
        &mut texture_atlases,
        DETAIL_CENTER_X,
        DETAIL_TOP_Y,
        DETAIL_WIDTH,
        None,
    );
}

fn spawn_beastiary_grid(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    beastiary: &Beastiary,
    page: usize,
) {
    let start = page * CARDS_PER_PAGE;
    let grid_w = GRID_COLS as f32 * GRID_CELL_W;
    let grid_left = GRID_CENTER_X - grid_w * 0.5 + GRID_CELL_W * 0.5;

    for (local_i, (card_obj, mob)) in BEASTIARY_MOBS
        .iter()
        .skip(start)
        .take(CARDS_PER_PAGE)
        .enumerate()
    {
        let mob_index = start + local_i;
        let col = local_i % GRID_COLS;
        let row = local_i / GRID_COLS;
        let x = grid_left + col as f32 * GRID_CELL_W;
        let y = GRID_TOP_Y - row as f32 * GRID_CELL_H;
        let entry = beastiary.get(mob);
        spawn_grid_cell(
            commands,
            asset_server,
            graphics,
            x,
            y,
            mob_index,
            *card_obj,
            entry.cards_collected,
        );
    }
}

fn spawn_grid_cell(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    x: f32,
    y: f32,
    mob_index: usize,
    card_obj: WorldObject,
    cards_collected: u32,
) {
    let card_bg = graphics.get_ui_element_texture(UIElement::BestiaryCardBackground);
    if cards_collected == 0 {
        let cell = commands
            .spawn((
                SpriteBundle {
                    texture: card_bg,
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(CARD_W, CARD_H)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(x, y, CONTENT_Z)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                Interactable::default(),
                BounceOnHit::with_strength_fraction(CARD_BOUNCE_STRENGTH),
                BeastiaryBrowserUI,
                UIState::BeastiaryBrowser,
                BeastiaryCardCell { mob_index },
                Name::new("Beastiary locked card cell"),
            ))
            .id();
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    "?",
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 30.0,
                        color: Color::rgba(0.85, 0.85, 0.85, 1.),
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::BeastiaryBrowser)
            .set_parent(cell);
    } else {
        // Owned: card slot background + mob card sprite + bottom-right "x{N}" overlay.
        let mut atlas_sprite = graphics
            .spritesheet_map
            .as_ref()
            .and_then(|map| map.get(&card_obj).cloned())
            .unwrap_or_else(TextureAtlasSprite::default);
        atlas_sprite.custom_size = Some(Vec2::new(CARD_SPRITE_W, CARD_SPRITE_H));
        let cell = commands
            .spawn((
                SpriteBundle {
                    texture: card_bg,
                    sprite: Sprite {
                        custom_size: Some(Vec2::new(CARD_W, CARD_H)),
                        ..Default::default()
                    },
                    transform: Transform::from_translation(Vec3::new(x, y, CONTENT_Z)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                Interactable::default(),
                BounceOnHit::with_strength_fraction(CARD_BOUNCE_STRENGTH),
                BeastiaryBrowserUI,
                UIState::BeastiaryBrowser,
                BeastiaryCardCell { mob_index },
                Name::new("Beastiary owned card cell"),
            ))
            .id();
        commands
            .spawn((
                SpriteSheetBundle {
                    sprite: atlas_sprite,
                    texture_atlas: graphics
                        .texture_atlas
                        .as_ref()
                        .expect("texture atlas loaded")
                        .clone(),
                    transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                UIState::BeastiaryBrowser,
            ))
            .set_parent(cell);
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    format!("x{cards_collected}"),
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                ),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(
                    CARD_W * 0.5 - 4.,
                    -CARD_H * 0.5 - 2.,
                    2.,
                )),
                ..Default::default()
            })
            .insert(RenderLayers::from_layers(&[3]))
            .insert(UIState::BeastiaryBrowser)
            .set_parent(cell);
    }
}

/// Read base HP and Attack from the mob's prototype components.
/// Extra space between the mob preview and the first detail line (large sprites).
fn detail_text_offset_below_preview(mob: &Mob) -> f32 {
    match mob {
        Mob::StoneGolem => 0.,
        _ => 0.,
    }
}

fn mob_base_hp_and_attack(proto: &ProtoParam, mob: &Mob) -> Option<(i32, i32)> {
    let hp = proto
        .get_component::<crate::attributes::MaxHealth, _>(mob.clone())
        .map(|m| m.0);
    let atk = proto
        .get_component::<crate::attributes::Attack, _>(mob.clone())
        .map(|a| a.0);
    match (hp, atk) {
        (Some(h), Some(a)) => Some((h, a)),
        (Some(h), None) => Some((h, 0)),
        _ => None,
    }
}

fn spawn_detail_panel(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    beastiary: &Beastiary,
    proto: &ProtoParam,
    texture_atlases: &mut Assets<TextureAtlas>,
    center_x: f32,
    top_y: f32,
    panel_width: f32,
    selected: Option<&Mob>,
) {
    let Some(mob) = selected else {
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Select a card to view details.",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(
                    center_x,
                    top_y - panel_width * 0.4,
                    CONTENT_Z,
                )),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            BeastiaryBrowserUI,
            UIState::BeastiaryBrowser,
            BeastiaryDetailPanel,
            Name::new("Beastiary detail hint"),
        ));
        return;
    };

    let entry = beastiary.get(mob);
    let copies = entry.cards_collected;
    if copies == 0 {
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    "Collect a card to learn more.",
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(
                    center_x,
                    top_y - panel_width * 0.4,
                    CONTENT_Z,
                )),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            BeastiaryBrowserUI,
            UIState::BeastiaryBrowser,
            BeastiaryDetailPanel,
            Name::new("Beastiary detail locked"),
        ));
        return;
    }

    // copies >= 1 reveals: top-center mob preview + name.
    let preview_y = top_y + 62.;
    spawn_mob_preview(
        commands,
        asset_server,
        graphics,
        texture_atlases,
        mob,
        center_x,
        preview_y,
    );

    let mut y = preview_y - 72. - detail_text_offset_below_preview(mob);
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                mob_display_name(mob),
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: DARK_WOOD_BROWN,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(center_x, 140., CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        BeastiaryDetailPanel,
        Name::new("Beastiary detail name"),
    ));

    if copies >= 2 {
        y -= 18.;
        if let Some((hp, atk)) = mob_base_hp_and_attack(proto, mob) {
            commands.spawn((
                Text2dBundle {
                    text: Text::from_section(
                        format!("HP {hp}     ATK {atk}"),
                        TextStyle {
                            font: asset_server.load("fonts/4x5.ttf"),
                            font_size: 5.0,
                            color: RED,
                        },
                    )
                    .with_alignment(TextAlignment::Center),
                    text_anchor: Anchor::Center,
                    transform: Transform::from_translation(Vec3::new(center_x, y, CONTENT_Z)),
                    ..Default::default()
                },
                RenderLayers::from_layers(&[3]),
                BeastiaryBrowserUI,
                UIState::BeastiaryBrowser,
                BeastiaryDetailPanel,
                Name::new("Beastiary detail base stats"),
            ));
        }
    } else {
        y -= 18.;
        spawn_locked_line(commands, asset_server, center_x, y, "?? HP   ?? ATK");
    }

    if copies >= 3 {
        spawn_run_stats(commands, asset_server, center_x, y - 14., &entry);
    } else {
        spawn_locked_run_stats(commands, asset_server, center_x, y - 14.);
    }
}

fn spawn_locked_line(
    commands: &mut Commands,
    asset_server: &AssetServer,
    x: f32,
    y: f32,
    text: &str,
) {
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                text,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: GREY,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(x, y, CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        BeastiaryDetailPanel,
        Name::new("Beastiary detail locked line"),
    ));
}

fn spawn_run_stats(
    commands: &mut Commands,
    asset_server: &AssetServer,
    x: f32,
    start_y: f32,
    entry: &BeastiaryEntry,
) {
    let lines = [
        format!("Number killed: {}", entry.number_killed),
        format!("Damage dealt: {}", entry.damage_dealt),
        format!("Damage taken: {}", entry.damage_taken),
        format!("Deaths caused: {}", entry.deaths_caused),
    ];
    for (i, line) in lines.iter().enumerate() {
        commands.spawn((
            Text2dBundle {
                text: Text::from_section(
                    line.clone(),
                    TextStyle {
                        font: asset_server.load("fonts/4x5.ttf"),
                        font_size: 5.0,
                        color: DARK_WOOD_BROWN,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(
                    x,
                    start_y - i as f32 * 10.,
                    CONTENT_Z,
                )),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            BeastiaryBrowserUI,
            UIState::BeastiaryBrowser,
            BeastiaryDetailPanel,
            Name::new("Beastiary detail run stat"),
        ));
    }
}

fn spawn_locked_run_stats(
    commands: &mut Commands,
    asset_server: &AssetServer,
    x: f32,
    start_y: f32,
) {
    let labels = [
        "Number killed: ???",
        "Damage dealt: ???",
        "Damage taken: ???",
        "Deaths caused: ???",
    ];
    for (i, line) in labels.iter().enumerate() {
        spawn_locked_line(commands, asset_server, x, start_y - i as f32 * 10., line);
    }
}

/// Per-mob spritesheet metadata for the bestiary preview. Mirrors the values
/// in each mob's `*.prototype.ron` so we can render a single static frame from
/// the same down-facing texture without going through the full proto pipeline.
/// Sheet preview metadata: `(frame_size, cols, rows, walk_row_index, walk_frames)`.
///
/// - `cols` MUST equal the max value in the mob's proto `animation_frames`
///   list, since `TextureAtlas::from_grid` slices the sheet at that width.
/// - `rows` MUST equal `animation_frames.len()` (5 for these mobs).
/// - `walk_row_index` is the walk row (1 — matches `EnemyAnimationState::Walk`
///   in `enemy_sprites.rs`).
/// - `walk_frames` is the number of cells used by the walk row, taken from
///   `animation_frames[walk_row_index]` in the mob's proto file.
fn preview_sheet_data(mob: &Mob) -> Option<(Vec2, usize, usize, usize, usize)> {
    match mob {
        // furdevil proto animation_frames: [4,6,4,8,7]
        Mob::FurDevil => Some((Vec2::new(32., 32.), 8, 5, 1, 6)),
        // bushling proto animation_frames: [4,4,4,9,6]
        Mob::Bushling => Some((Vec2::new(38., 38.), 9, 5, 1, 4)),
        // spikeslime proto animation_frames: [4,4,4,6,4]
        Mob::SpikeSlime => Some((Vec2::new(32., 32.), 6, 5, 1, 4)),
        // stingfly proto animation_frames: [4,4,4,10,7]
        Mob::StingFly => Some((Vec2::new(38., 38.), 10, 5, 1, 4)),
        _ => None,
    }
}

/// Returns the aseprite path and walk-tag for mobs whose in-game preview is
/// driven by `bevy_aseprite` rather than a manual sprite-sheet atlas. The
/// `bevy_aseprite` plugin animates these automatically in every `GameState`,
/// so no local animator system is needed for them.
fn preview_aseprite_data(mob: &Mob) -> Option<(&'static str, &'static str)> {
    use crate::enemy::aseprite_enemy::{
        BigCactusAse, BullAse, LizardAse, SmallCactusAse, VoidCrawlerAse,
    };
    use crate::enemy::red_mushling::RedMushling;
    use crate::enemy::stone_golem::StoneGolem;
    match mob {
        Mob::Bull => Some((BullAse::PATH, "WalkDown")),
        Mob::BigCactus => Some((BigCactusAse::PATH, "WalkDown")),
        Mob::SmallCactus => Some((SmallCactusAse::PATH, "WalkDown")),
        Mob::Lizard => Some((LizardAse::PATH, "WalkDown")),
        Mob::VoidCrawler => Some((VoidCrawlerAse::PATH, "WalkDown")),
        Mob::StoneGolem => Some((StoneGolem::PATH, "WalkFront")),
        Mob::RedMushling => Some((RedMushling::PATH, "IDLE_FRONT")),
        _ => None,
    }
}

/// Tracks the next frame switch time for the locally-animated sheet previews.
/// Stored on the preview entity so the bestiary UI doesn't depend on the main
/// `animate_character_spritesheet_animations` system (which only runs in
/// `GameState::Main`).
#[derive(Component)]
pub struct BeastiaryPreviewSheet {
    timer: Timer,
    /// Start index of the walk row in the atlas (`walk_row_index * cols`).
    walk_start_index: usize,
    /// Number of frames on the walk row.
    walk_frames: usize,
}

/// Spawn a stationary, animated mob preview at (x, y).
///
/// Two render paths:
/// 1. **Sheet-based mobs** (FurDevil, Bushling, SpikeSlime, StingFly): build a
///    `TextureAtlas` from the mob's `_down.png` and attach a
///    `BeastiaryPreviewSheet` component so the local
///    `animate_beastiary_previews` system cycles frames. The global
///    `animate_character_spritesheet_animations` only runs in
///    `GameState::Main`, so we can't reuse it here.
/// 2. **Aseprite mobs** (Bull, cacti, Lizard, VoidCrawler, StoneGolem, RedMushling):
///    spawn `AsepriteBundle` with the matching walk tag. `bevy_aseprite`'s
///    plugin advances animations every frame in every `GameState`.
fn spawn_mob_preview(
    commands: &mut Commands,
    asset_server: &AssetServer,
    graphics: &Graphics,
    texture_atlases: &mut Assets<TextureAtlas>,
    mob: &Mob,
    x: f32,
    y: f32,
) {
    let preview_scale = 1.5;

    if let Some((frame, cols, rows, walk_row, walk_frames)) = preview_sheet_data(mob) {
        if let Some(handles) = graphics.mob_spritesheets.as_ref().and_then(|m| m.get(mob)) {
            // index 2 = down (matches assets/mod.rs build order: side, up, down)
            if let Some(down) = handles.get(2).cloned() {
                let atlas = TextureAtlas::from_grid(down, frame, cols, rows, None, None);
                let atlas_handle = texture_atlases.add(atlas);
                let walk_start_index = walk_row * cols;
                let mut sprite = TextureAtlasSprite::new(walk_start_index);
                sprite.custom_size = Some(frame * preview_scale);
                commands.spawn((
                    SpriteSheetBundle {
                        sprite,
                        texture_atlas: atlas_handle,
                        transform: Transform::from_translation(Vec3::new(x, y, CONTENT_Z)),
                        ..Default::default()
                    },
                    BeastiaryPreviewSheet {
                        timer: Timer::from_seconds(0.12, TimerMode::Repeating),
                        walk_start_index,
                        walk_frames,
                    },
                    RenderLayers::from_layers(&[3]),
                    BeastiaryBrowserUI,
                    UIState::BeastiaryBrowser,
                    BeastiaryDetailPanel,
                    Name::new("Beastiary mob preview (sheet)"),
                ));
                return;
            }
        }
    }

    if let Some((path, walk_tag)) = preview_aseprite_data(mob) {
        let mut animation = bevy_aseprite::anim::AsepriteAnimation::from(walk_tag);
        animation.play();
        commands.spawn((
            bevy_aseprite::AsepriteBundle {
                aseprite: asset_server.load(path),
                animation,
                transform: Transform::from_translation(Vec3::new(x, y, CONTENT_Z))
                    .with_scale(Vec3::splat(preview_scale)),
                ..Default::default()
            },
            RenderLayers::from_layers(&[3]),
            BeastiaryBrowserUI,
            UIState::BeastiaryBrowser,
            BeastiaryDetailPanel,
            Name::new("Beastiary mob preview (aseprite)"),
        ));
        return;
    }

    // Final fallback — should not be reached for mobs in `BEASTIARY_MOBS`.
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.3, 0.3, 0.35, 1.),
                custom_size: Some(Vec2::new(48., 48.)),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(x, y, CONTENT_Z)),
            ..Default::default()
        },
        RenderLayers::from_layers(&[3]),
        BeastiaryBrowserUI,
        UIState::BeastiaryBrowser,
        BeastiaryDetailPanel,
        Name::new("Beastiary mob preview placeholder"),
    ));
}

/// Local animator for the bestiary sheet-based previews. Cycles the walk row
/// at a fixed rate regardless of `GameState`.
pub fn animate_beastiary_previews(
    time: Res<Time>,
    mut q: Query<(&mut BeastiaryPreviewSheet, &mut TextureAtlasSprite)>,
) {
    for (mut sheet, mut sprite) in &mut q {
        sheet.timer.tick(time.delta());
        if sheet.timer.just_finished() {
            let start = sheet.walk_start_index;
            let len = sheet.walk_frames.max(1);
            let next = start + ((sprite.index + 1).saturating_sub(start)) % len;
            sprite.index = next;
        }
    }
}

pub fn handle_beastiary_card_click(
    mut commands: Commands,
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut cells: Query<(
        Entity,
        &mut Interactable,
        &BeastiaryCardCell,
        &mut BounceOnHit,
    )>,
    mut selected: ResMut<SelectedBeastiaryMob>,
    detail_entities: Query<Entity, With<BeastiaryDetailPanel>>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    beastiary: Res<Beastiary>,
    proto: ProtoParam,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);

    let mut clicked: Option<Mob> = None;
    for (entity, mut interactable, cell, mut bounce) in cells.iter_mut() {
        match hit_test {
            Some(hit) if hit.0 == entity => match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    bounce.activate();
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released {
                        if let Some((_, mob)) = BEASTIARY_MOBS.get(cell.mob_index) {
                            clicked = Some(mob.clone());
                        }
                    }
                }
                _ => {}
            },
            _ => {
                interactable.change(Interaction::None);
            }
        }
    }

    if let Some(mob) = clicked {
        selected.0 = Some(mob.clone());
        for entity in detail_entities.iter() {
            commands.entity(entity).despawn_recursive();
        }
        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
        spawn_detail_panel(
            &mut commands,
            &asset_server,
            &graphics,
            &beastiary,
            &proto,
            &mut texture_atlases,
            DETAIL_CENTER_X,
            DETAIL_TOP_Y,
            DETAIL_WIDTH,
            Some(&mob),
        );
        // Quiet usage to discourage tree-shaking warnings on `card_for_mob`,
        // which is provided as part of the public API for symmetry with
        // `mob_for_card`.
        let _ = card_for_mob(&mob);
    }
}

pub fn refresh_beastiary_grid_on_pagination_change(
    mut commands: Commands,
    pagination: Res<BeastiaryPagination>,
    beastiary: Res<Beastiary>,
    asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
    existing_cells: Query<Entity, With<BeastiaryCardCell>>,
) {
    if !pagination.is_changed() {
        return;
    }
    for entity in existing_cells.iter() {
        commands.entity(entity).despawn_recursive();
    }
    spawn_beastiary_grid(
        &mut commands,
        &asset_server,
        &graphics,
        &beastiary,
        pagination.page,
    );
}

fn beastiary_prev_normal_sprite(commands: &mut Commands, graphics: &Graphics, entity: Entity) {
    commands
        .entity(entity)
        .insert(UIElement::BestiaryPrevButton)
        .insert(graphics.get_ui_element_texture(UIElement::BestiaryPrevButton));
}

fn beastiary_next_normal_sprite(commands: &mut Commands, graphics: &Graphics, entity: Entity) {
    commands
        .entity(entity)
        .insert(UIElement::BestiaryNextButton)
        .insert(graphics.get_ui_element_texture(UIElement::BestiaryNextButton));
}

pub fn handle_beastiary_pagination_clicks(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut prev_buttons: Query<
        (Entity, &mut Interactable),
        (
            With<BeastiaryBrowserPrevButton>,
            Without<BeastiaryBrowserNextButton>,
        ),
    >,
    mut next_buttons: Query<
        (Entity, &mut Interactable),
        (
            With<BeastiaryBrowserNextButton>,
            Without<BeastiaryBrowserPrevButton>,
        ),
    >,
    mut pagination: ResMut<BeastiaryPagination>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    let hit_test = ui_helpers::pointcast_2d(&cursor_pos, &ui_sprites, None);
    let left_mouse_released = mouse_input.just_released(MouseButton::Left);
    let total_pages = beastiary_total_pages();

    let Some(hit) = hit_test else {
        for (entity, mut interactable) in prev_buttons.iter_mut() {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
                beastiary_prev_normal_sprite(&mut commands, &graphics, entity);
            }
        }
        for (entity, mut interactable) in next_buttons.iter_mut() {
            if matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::None);
                beastiary_next_normal_sprite(&mut commands, &graphics, entity);
            }
        }
        return;
    };

    for (entity, mut interactable) in prev_buttons.iter_mut() {
        if hit.0 == entity {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands
                        .entity(entity)
                        .insert(UIElement::BestiaryPrevButtonHover)
                        .insert(
                            graphics.get_ui_element_texture(UIElement::BestiaryPrevButtonHover),
                        );
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released && pagination.page > 0 {
                        pagination.page -= 1;
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            beastiary_prev_normal_sprite(&mut commands, &graphics, entity);
        }
    }

    for (entity, mut interactable) in next_buttons.iter_mut() {
        if hit.0 == entity {
            match interactable.current() {
                Interaction::None => {
                    interactable.change(Interaction::Hovering);
                    commands
                        .entity(entity)
                        .insert(UIElement::BestiaryNextButtonHover)
                        .insert(
                            graphics.get_ui_element_texture(UIElement::BestiaryNextButtonHover),
                        );
                    commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
                }
                Interaction::Hovering => {
                    if left_mouse_released && pagination.page + 1 < total_pages {
                        pagination.page += 1;
                        commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonClick, 0.2));
                    }
                }
                _ => {}
            }
        } else if matches!(interactable.current(), Interaction::Hovering) {
            interactable.change(Interaction::None);
            beastiary_next_normal_sprite(&mut commands, &graphics, entity);
        }
    }
}

pub fn handle_beastiary_browser_done_button(
    cursor_pos: Res<CursorPos>,
    mouse_input: Res<Input<MouseButton>>,
    ui_sprites: Query<(Entity, &Sprite, &GlobalTransform), With<Interactable>>,
    mut buttons: Query<(Entity, &mut Interactable), With<BeastiaryBrowserDoneButton>>,
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

pub fn cleanup_beastiary_browser_ui(
    mut commands: Commands,
    query: Query<Entity, With<BeastiaryBrowserUI>>,
    mut selected: ResMut<SelectedBeastiaryMob>,
    mut pagination: ResMut<BeastiaryPagination>,
) {
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    selected.0 = None;
    pagination.page = 0;
}
