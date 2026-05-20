use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::audio::{AudioSoundEffect, SoundSpawner};
use crate::client::GameData;
use crate::colors::{BLACK, DARK_GREEN, DARK_WOOD_BROWN, WHITE, YELLOW_2};
use crate::cursor::CursorPos;
use crate::datafiles;
use crate::ui::{global_text_message::GlobalTextMessageEvent, Interactable, Interaction};
use crate::GameState;

use std::fs::File;
use std::io::{BufReader, BufWriter};

/// Z layers (sit above gameplay HUD but below pause overlays).
const Z_TUTORIAL_OVERLAY: f32 = 70.0;
const Z_TUTORIAL_PANEL: f32 = 71.0;
const Z_TUTORIAL_CONTENT: f32 = 72.0;
const Z_TUTORIAL_TEXT: f32 = 73.0;

/// Wide panel so three columns fit comfortably; tall enough for title + body per column.
const PANEL_WIDTH: f32 = 608.0;
const PANEL_HEIGHT: f32 = 248.0;

const ENTRIES_PER_PAGE: usize = 3;

/// Vertical center (panel space) for the three-column tip row, between title and buttons.
const CONTENT_BAND_CENTER_Y: f32 = 10.0;

/// Draw size for tutorial Aseprite clips (frames are authored at 64×64).
const TUTORIAL_ASEPRITE_SIZE: f32 = 64.0;
/// World-space Y for the center of each tip’s animation (below the body text block).
const TUTORIAL_ANIM_CENTER_Y: f32 = CONTENT_BAND_CENTER_Y - 54.0;

// Tags must match `TutorialAnims.ase`; constants are generated at compile time from the file.
aseprite!(pub TutorialAnims, "ui/TutorialAnims.ase");

/// Inserted by `tick_game_start_overlay` once the world fade-in finishes.
/// Consumed by `try_spawn_tutorial_overlay` on the first run.
#[derive(Resource)]
pub struct TutorialReady;

/// Queued at run start; shown after the first-run tutorial closes, or immediately if
/// the tutorial is skipped (`has_seen_tutorial`).
#[derive(Resource)]
pub struct PendingFindBossShrineHint;

fn show_find_boss_shrine_hint(events: &mut EventWriter<GlobalTextMessageEvent>) {
    events.send(GlobalTextMessageEvent::new(
        "Find the Boss Shrine",
        DARK_WOOD_BROWN,
    ));
}

fn flush_pending_find_boss_shrine_hint(
    commands: &mut Commands,
    pending: Option<Res<PendingFindBossShrineHint>>,
    global_text_events: &mut EventWriter<GlobalTextMessageEvent>,
) {
    if pending.is_some() {
        show_find_boss_shrine_hint(global_text_events);
        commands.remove_resource::<PendingFindBossShrineHint>();
    }
}

/// Queued from Options → "Show Tutorial"; same spawn path as [`TutorialReady`] but skips `has_seen_tutorial`.
#[derive(Resource)]
pub struct TutorialReplayRequested;

/// Each tutorial topic. The set is iterated in declaration order and paginated
/// `ENTRIES_PER_PAGE` at a time.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TutorialContent {
    Attacking,
    Skills,
    Heirlooms,

    Equipment,
    UpgradeTomes,
    Orbs,

    ShrinesExplore,
    PinkFlowers,
    Crafting,
}

impl TutorialContent {
    const ALL: [TutorialContent; 9] = [
        TutorialContent::Attacking,
        TutorialContent::Skills,
        TutorialContent::Heirlooms,
        TutorialContent::Equipment,
        TutorialContent::UpgradeTomes,
        TutorialContent::Orbs,
        TutorialContent::ShrinesExplore,
        TutorialContent::PinkFlowers,
        TutorialContent::Crafting,
    ];

    fn title(self) -> &'static str {
        match self {
            TutorialContent::Attacking => "Weapons",
            TutorialContent::Skills => "Skills",
            TutorialContent::Heirlooms => "Heirlooms",
            TutorialContent::Equipment => "Equipment",
            TutorialContent::UpgradeTomes => "Tomes",
            TutorialContent::Orbs => "Orbs",
            TutorialContent::ShrinesExplore => "Shrines",
            TutorialContent::PinkFlowers => "Biomes",
            TutorialContent::Crafting => "Crafting",
        }
    }

    fn body(self) -> &'static str {
        match self {
            TutorialContent::Attacking => {
                "Weapons attack automatically,\n\nuse your mouse to aim them."
            }
            TutorialContent::Skills => {
                "Skills are powerful, dont forget\n\nto use them! Drag to rearrange them."
                //Hotbar can be used\n\nto consume and use items, food,\n\npotions, or other consumables.
            }
            TutorialContent::Heirlooms => {
                "Heirlooms will boost stats or\n\ngrant strong effects. Use them\n\nto create a strong build!"
            }
            TutorialContent::Equipment => {
                "Equipment can be equipped to boost\n\nyour stats. Upgrade them to make\n\nthem stronger!"
            }
            TutorialContent::UpgradeTomes => {
                "Tomes will level up gear, granting\n\nmore base stats, and improving\n\nrandom bonus stats each level."
            }
            TutorialContent::Orbs => {
                "Orbs will re-roll the bonus stats\n\non gear, with a chance to upgrade\n\ntheir rarity too!"
            }
            TutorialContent::ShrinesExplore => {
                "Explore the island and interact\n\nwith shrines, which offer various\n\nchoices or challenges."
            }
            TutorialContent::PinkFlowers => {
                "Biomes have different environmental\n\ninteractions, like these bouncing flowers\n\nfound in the forest"
            }
            TutorialContent::Crafting => {
                "Craft different foods and tools\n\nto help you on your journey.\n\nBiomes unlock different blueprints."
            }
        }
    }
}

#[derive(Resource, Default)]
pub struct TutorialState {
    pub page: usize,
}

/// Marker for any entity that belongs to the tutorial overlay (panel, title,
/// buttons). Despawned all at once when the player finishes the tutorial.
#[derive(Component)]
pub struct TutorialUI;

/// Marker for per-page content (text + visuals for the 3 active entries).
/// Despawned when the page changes so the next page can spawn fresh entities.
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
            .add_system(
                try_spawn_tutorial_overlay
                    .after(crate::ui::main_menu::handle_menu_button_click_events)
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(handle_tutorial_buttons.in_set(OnUpdate(GameState::Main)));
    }
}

pub(crate) fn try_spawn_tutorial_overlay(
    mut commands: Commands,
    ready: Option<Res<TutorialReady>>,
    replay: Option<Res<TutorialReplayRequested>>,
    game_data: Option<Res<GameData>>,
    asset_server: Res<AssetServer>,
    mut state: ResMut<TutorialState>,
    existing: Query<(), With<TutorialUI>>,
    pending_hint: Option<Res<PendingFindBossShrineHint>>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
) {
    let first_run = ready.is_some();
    let replay = replay.is_some();
    if !first_run && !replay {
        return;
    }
    if !existing.is_empty() {
        if first_run {
            commands.remove_resource::<TutorialReady>();
        }
        if replay {
            commands.remove_resource::<TutorialReplayRequested>();
        }
        return;
    }

    if first_run {
        let already_seen = game_data
            .as_ref()
            .map(|g| g.has_seen_tutorial)
            .unwrap_or(false);
        if already_seen {
            flush_pending_find_boss_shrine_hint(
                &mut commands,
                pending_hint,
                &mut global_text_events,
            );
            commands.remove_resource::<TutorialReady>();
            return;
        }
        commands.remove_resource::<TutorialReady>();
    }
    if replay {
        commands.remove_resource::<TutorialReplayRequested>();
    }

    state.page = 0;
    spawn_tutorial_root(&mut commands, &asset_server);
    spawn_current_page(&mut commands, &asset_server, state.page);
}

fn spawn_tutorial_root(commands: &mut Commands, asset_server: &AssetServer) {
    // Full-screen dim
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0., 0., 0., 0.55),
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

    // Opaque panel (matches the leaderboard panel color/alpha).
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgba(0.15, 0.12, 0.10, 0.95),
                custom_size: Some(Vec2::new(PANEL_WIDTH, PANEL_HEIGHT)),
                ..default()
            },
            transform: Transform::from_translation(Vec3::new(0., 0., Z_TUTORIAL_PANEL)),
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Panel"),
    ));

    // Title
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "Quick Tutorial",
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                0.,
                PANEL_HEIGHT * 0.5 - 24.0,
                Z_TUTORIAL_TEXT,
            )),
            ..default()
        },
        RenderLayers::from_layers(&[3]),
        TutorialUI,
        Name::new("Tutorial Title"),
    ));

    // Bottom row of buttons: Prev (left), Done (center), Next (right).
    let button_y = -PANEL_HEIGHT * 0.5 + 14.0;
    let btn_spread = PANEL_WIDTH * 0.14;
    spawn_button(
        commands,
        asset_server,
        Vec3::new(-btn_spread, button_y, Z_TUTORIAL_CONTENT),
        "Prev",
        TutorialButtonKind::Prev,
    );
    spawn_button(
        commands,
        asset_server,
        Vec3::new(0.0, button_y, Z_TUTORIAL_CONTENT),
        "Done",
        TutorialButtonKind::Done,
    );
    spawn_button(
        commands,
        asset_server,
        Vec3::new(btn_spread, button_y, Z_TUTORIAL_CONTENT),
        "Next",
        TutorialButtonKind::Next,
    );
}

fn spawn_button(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    label: &str,
    kind: TutorialButtonKind,
) {
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
            Name::new(format!("Tutorial Button: {}", label)),
        ))
        .id();

    commands
        .spawn((
            Text2dBundle {
                text: Text::from_section(
                    label,
                    TextStyle {
                        font: asset_server.load("fonts/alagard.ttf"),
                        font_size: 15.0,
                        color: WHITE,
                    },
                )
                .with_alignment(TextAlignment::Center),
                text_anchor: Anchor::Center,
                transform: Transform::from_translation(Vec3::new(0., -1., 1.)),
                ..default()
            },
            RenderLayers::from_layers(&[3]),
            TutorialUI,
        ))
        .set_parent(button_e);
}

/// Maps entry index → column slot (0 = left, 1 = mid, 2 = right). Last page may have fewer than 3
/// topics; we keep a balanced layout (one topic uses center column; two use left + right).
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

fn column_center_x(col: usize) -> f32 {
    let col_w = PANEL_WIDTH / ENTRIES_PER_PAGE as f32;
    -PANEL_WIDTH * 0.5 + col_w * (col as f32 + 0.5)
}

/// Spawn the entries (text + visuals) for the page at `page_index` in a single horizontal row.
fn spawn_current_page(commands: &mut Commands, asset_server: &AssetServer, page_index: usize) {
    let entries = page_slice(page_index);
    let n = entries.len();
    for (i, content) in entries.iter().enumerate() {
        let col = column_index_for_entry(i, n);
        let col_x = column_center_x(col);
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
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 15.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                column_center_x,
                title_y,
                Z_TUTORIAL_TEXT,
            )),
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
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                column_center_x,
                body_y,
                Z_TUTORIAL_TEXT,
            )),
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
        TutorialContent::Orbs => TutorialAnims::tags::ORB,
        TutorialContent::ShrinesExplore => TutorialAnims::tags::SHRINE,
        TutorialContent::PinkFlowers => TutorialAnims::tags::PINK_FLOWER,
        TutorialContent::Crafting => TutorialAnims::tags::CRAFTING,
    }
}

/// Looping Aseprite clip for this tip (`TutorialAnims.ase`). Tagged like page text so
/// [`refresh_page`] despawn of `TutorialPageEntity` removes it with the column.
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

fn page_slice(page_index: usize) -> Vec<TutorialContent> {
    let start = page_index * ENTRIES_PER_PAGE;
    let end = (start + ENTRIES_PER_PAGE).min(TutorialContent::ALL.len());
    if start >= end {
        return Vec::new();
    }
    TutorialContent::ALL[start..end].to_vec()
}

fn total_pages() -> usize {
    (TutorialContent::ALL.len() + ENTRIES_PER_PAGE - 1) / ENTRIES_PER_PAGE
}

/// Same rectangle test as `ui_helpers::pointcast_2d`, but only tutorial buttons and
/// using a read-only `Sprite` query so it can pair with a second query that mutates
/// `Sprite` (Bevy disallows both in one system without `ParamSet`).
fn tutorial_button_hit_under_cursor(
    cursor_pos: &Res<CursorPos>,
    query: &Query<(Entity, &Sprite, &GlobalTransform), With<TutorialButton>>,
) -> Option<Entity> {
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
    asset_server: Res<AssetServer>,
    mut button_queries: ParamSet<(
        Query<(Entity, &Sprite, &GlobalTransform), With<TutorialButton>>,
        Query<(Entity, &mut Interactable, &TutorialButton, &mut Sprite), With<TutorialButton>>,
    )>,
    page_entities: Query<Entity, With<TutorialPageEntity>>,
    all_tutorial: Query<Entity, With<TutorialUI>>,
    mut state: ResMut<TutorialState>,
    mut game_data: Option<ResMut<GameData>>,
    pending_hint: Option<Res<PendingFindBossShrineHint>>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
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
        if hovering {
            sprite.color = DARK_GREEN;
            if !matches!(interactable.current(), Interaction::Hovering) {
                interactable.change(Interaction::Hovering);
                commands.spawn(SoundSpawner::new(AudioSoundEffect::ButtonHover, 0.05));
            }
            if just_clicked {
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
            if state.page > 0 {
                state.page -= 1;
                refresh_page(&mut commands, &asset_server, &page_entities, state.page);
            }
        }
        TutorialButtonKind::Next => {
            if state.page + 1 < total_pages() {
                state.page += 1;
                refresh_page(&mut commands, &asset_server, &page_entities, state.page);
            }
        }
        TutorialButtonKind::Done => {
            for e in all_tutorial.iter() {
                if let Some(ec) = commands.get_entity(e) {
                    ec.despawn_recursive();
                }
            }
            flush_pending_find_boss_shrine_hint(
                &mut commands,
                pending_hint,
                &mut global_text_events,
            );
            if let Some(gd) = game_data.as_mut() {
                gd.has_seen_tutorial = true;
                persist_has_seen_tutorial(true);
            } else {
                persist_has_seen_tutorial(true);
            }
        }
    }
}

fn refresh_page(
    commands: &mut Commands,
    asset_server: &AssetServer,
    page_entities: &Query<Entity, With<TutorialPageEntity>>,
    new_page: usize,
) {
    for e in page_entities.iter() {
        if let Some(ec) = commands.get_entity(e) {
            ec.despawn_recursive();
        }
    }
    spawn_current_page(commands, asset_server, new_page);
}

/// Mirrors `persist_seen_tips`: load the existing `game_data.json`, flip the
/// `has_seen_tutorial` flag, and write it back so the popup never reappears.
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
