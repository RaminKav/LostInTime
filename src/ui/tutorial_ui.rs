use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy::sprite::Anchor;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, BufWriter};

use crate::audio::{AudioSoundEffect, SoundSpawner};
use crate::client::GameData;
use crate::colors::{BLACK, DARK_GREEN, DARK_WOOD_BROWN, WHITE, YELLOW_2};
use crate::cursor::CursorPos;
use crate::datafiles;
use crate::item::WorldObject;
use crate::player::score::RunTimer;
use crate::player::skills::PlayerSkills;
use crate::proto::proto_param::ProtoParam;
use crate::ui::{global_text_message::GlobalTextMessageEvent, Interactable, Interaction};
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

/// Heirloom picks required before the heirlooms tutorial appears.
pub const HEIRLOOM_TUTORIAL_PICK_COUNT: usize = 3;

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
}

impl TutorialContent {
    pub const ALL: [TutorialContent; 9] = [
        TutorialContent::Attacking,
        TutorialContent::Skills,
        TutorialContent::Heirlooms,
        TutorialContent::Equipment,
        TutorialContent::UpgradeTomes,
        TutorialContent::InventoryStats,
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
            TutorialContent::UpgradeTomes => "Tomes & Orbs",
            TutorialContent::InventoryStats => "Inventory Stats",
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
            }
            TutorialContent::Heirlooms => {
                "Heirlooms will boost stats or\n\ngrant strong effects. Use them\n\nto create a strong build!"
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

/// Right-align a single-tip panel so its right edge sits on the UI view edge.
fn panel_center_x(entry_count: usize, panel_width: f32, game_width: f32) -> f32 {
    if entry_count == 1 {
        super::tooltips::clamp_tooltip_center_x(
            game_width * 0.5 - panel_width * 0.5,
            panel_width * 0.5,
            game_width,
            0.0,
        )
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

fn handle_tutorial_popup_events(
    mut commands: Commands,
    mut events: EventReader<TutorialPopupEvent>,
    asset_server: Res<AssetServer>,
    resolution: Res<ScreenResolution>,
    mut state: ResMut<TutorialState>,
    existing: Query<(), With<TutorialUI>>,
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
        let entry_count = event.entries.len().min(ENTRIES_PER_PAGE);
        let panel_width = panel_width_for_entry_count(entry_count);
        let panel_height = panel_height_for_entry_count(entry_count);
        let panel_offset_x = panel_center_x(entry_count, panel_width, resolution.game_width);

        state.page = 0;
        state.mode = event.mode;
        state.entries = event.entries.clone();
        state.panel_width = panel_width;
        state.panel_height = panel_height;
        state.panel_offset_x = panel_offset_x;

        spawn_tutorial_root(
            &mut commands,
            &asset_server,
            state.mode,
            panel_width,
            panel_height,
            panel_offset_x,
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

/// Call when the active skill shrine UI opens.
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

fn spawn_tutorial_root(
    commands: &mut Commands,
    asset_server: &AssetServer,
    mode: TutorialPopupMode,
    panel_width: f32,
    panel_height: f32,
    panel_offset_x: f32,
) {
    let use_dim =
        mode == TutorialPopupMode::FullReplay || panel_width > PANEL_WIDTH_SINGLE + f32::EPSILON;
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
                TextStyle {
                    font: asset_server.load("fonts/alagard.ttf"),
                    font_size: 30.0,
                    color: YELLOW_2,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: Anchor::Center,
            transform: Transform::from_translation(Vec3::new(
                panel_offset_x,
                panel_height * 0.5 - 24.0,
                Z_TUTORIAL_TEXT,
            )),
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
        TutorialContent::InventoryStats => TutorialAnims::tags::INVENTORY_STATS,
        TutorialContent::ShrinesExplore => TutorialAnims::tags::SHRINE,
        TutorialContent::PinkFlowers => TutorialAnims::tags::PINK_FLOWER,
        TutorialContent::Crafting => TutorialAnims::tags::CRAFTING,
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
