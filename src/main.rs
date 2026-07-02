#![windows_subsystem = "windows"] // hide console bcus we have nice logs :)
#![allow(non_snake_case)]
use std::{
    env,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use ai::AIPlugin;
use attributes::{
    Attack, AttributesPlugin, BonusDamage, CritChance, CritDamage, CurrentHealth, Defence, Dodge,
    Healing, HealthRegen, Lifesteal, LootRateBonus, MaxHealth, Speed, Thorns, XpRateBonus,
};
mod aim;
mod audio;
mod bounce;
mod container;
mod cursor;
mod datafiles;
mod gamepad_input;
mod keybinds;
mod panic_handler;
mod pets;
mod vectorize;
pub use bounce::*;
pub use display_scale::DisplayScaleSettings;
pub use ecs_helpers::*;
pub use keybinds::*;
pub use pets::*;

use audio::AudioPlugin;
use bevy_aseprite::AsepritePlugin;

use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    diagnostic::FrameTimeDiagnosticsPlugin,
    ecs::{schedule::ScheduleLabel, system::SystemParam},
    log::LogPlugin,
    prelude::*,
    render::{camera::ScalingMode, view::RenderLayers},
    window::{PresentMode, PrimaryWindow, Window, WindowMode, WindowResolution},
};
use bevy_common_assets::ron::RonAssetPlugin;
use bevy_embedded_assets::EmbeddedAssetPlugin;
use chaos::ChaosPlugin;
use juice::JuicePlugin;
use night::NightPlugin;
use rand::Rng;
use sapling::SaplingPlugin;

mod juice;
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::prelude::*;
mod ai;
mod animations;
mod assets;
mod attributes;
mod blessings;
mod chaos;
mod client;
mod collider_load_test;
mod colors;
mod combat;
mod custom_commands;
mod display_scale;
mod ecs_helpers;
mod enemy;
mod gameplay_load_tests;
mod inputs;
mod inventory;
mod item;
mod night;
mod player;
mod proto;
mod sapling;
mod ui;
mod world;
use animations::AnimationsPlugin;
use assets::{GameAssetsPlugin, Graphics, GraphicsDesc, SpriteSize};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use blessings::BlessingsPlugin;
use client::ClientPlugin;
use combat::*;
use enemy::EnemyPlugin;
use inputs::InputsPlugin;
use inventory::ItemStack;
use item::{Equipment, ItemsPlugin, RecipeListProto, WorldObject, WorldObjectResource};
use player::{
    levels::PlayerLevel,
    rogue_skills::ComboCounter,
    skills::{Heirloom, PlayerSkills},
    Player, PlayerPlugin, PlayerState,
};
use proto::{proto_param::ProtoParam, ProtoPlugin};

use tracing::level_filters::LevelFilter;
#[cfg(feature = "tracy")]
use tracing_subscriber::filter::FilterFn;
use tracing_subscriber::fmt::format::{self, FmtSpan};
#[cfg(feature = "tracy")]
use tracing_subscriber::Layer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter};
use ui::{
    cleanup_loading_screen, display_main_menu, handle_menu_button_click_events, remove_main_menu,
    spawn_menu_text_buttons, InventorySlotState, UIPlugin,
};
use world::{
    chunk::{Chunk, TileEntityCollection, TileSpriteData},
    generation::WorldObjectCache,
    world_helpers::world_pos_to_tile_pos,
    y_sort::YSort,
    TileMapPosition, WallTextureData, WorldPlugin,
};
use world::{dimension::EraManager, WorldGeneration};

use crate::player::{skill_heirlooms::Stealthed, skills::HeirloomTriggerCounts, ClassUnlockConfig};
use crate::{
    assets::{ClassPetData, SpriteAnchor},
    blessings::OwnedBlessings,
};
use lazy_static::lazy_static;

use logs_wheel::LogFileInitializer;
use std::sync::Mutex;

const PLAYER_MOVE_SPEED: f32 = 85.;
const PLAYER_DASH_SPEED: f32 = 375.;
pub const TIME_STEP: f32 = 1.0 / 60.0;

pub const HEIGHT: f32 = 1080.;
pub const ASPECT_RATIO: f32 = 16.0 / 10.0;
pub const WIDTH: f32 = HEIGHT * ASPECT_RATIO;
/// Legacy reference layout height (`300 * 1.2`). Runtime UI/world integer scales come from
/// [`DisplayScaleSettings`] in options; this constant is kept for fixed layout art anchors.
pub const GAME_HEIGHT: f32 = display_scale::REFERENCE_VIEW_TARGET_HEIGHT;
const GAME_WIDTH: f32 = GAME_HEIGHT * ASPECT_RATIO; //384 x 240
lazy_static! {
    pub static ref DEBUG: bool = env::var("DEBUG").is_ok();
}
lazy_static! {
    pub static ref NO_GEN: bool = env::var("NO_GEN").is_ok();
}
lazy_static! {
    pub static ref MINIMAP: bool = true;
    // pub static ref MINIMAP: bool = env::var("MINIMAP").is_ok();
}
lazy_static! {
    pub static ref COLLIDERS: bool = env::var("COLLIDERS").is_ok();
}
lazy_static! {
    pub static ref DEBUG_AI: bool = env::var("DEBUG_AI").is_ok();
}
lazy_static! {
    /// Spawns/despawns 100 mobs + 200 item drops in a tight loop to stress Rapier (see `collider_load_test`).
    pub static ref COLLIDER_LOAD_TEST: bool = env::var("COLLIDER_LOAD_TEST").is_ok();
}
lazy_static! {
    /// Spawns/despawns 100 mobs + 200 item drops in a tight loop to stress Rapier (see `collider_load_test`).
    pub static ref NO_DROPS: bool = env::var("NO_DROPS").is_ok();
}
lazy_static! {
    /// Spawns/despawns 100 mobs + 200 item drops in a tight loop to stress Rapier (see `collider_load_test`).
    pub static ref NO_XP: bool = env::var("NO_XP").is_ok();
}
lazy_static! {
    /// Disables overworld mob spawning from `enemy::spawner` (timers, events, Stone Golem timer).
    pub static ref NO_SPAWN: bool = env::var("NO_SPAWN").is_ok();
}
lazy_static! {
    pub static ref HEIRLOOM_LOAD_TEST: bool = env::var("HEIRLOOM_LOAD_TEST").is_ok();
}
lazy_static! {
    pub static ref PARTICLE_LOAD_TEST: bool = env::var("PARTICLE_LOAD_TEST").is_ok();
}
lazy_static! {
    /// Logs entity/heirloom-sim counts every 5s while in `GameState::Main` (see `gameplay_load_tests`).
    pub static ref HEIRLOOM_LOAD_TEST_DIAG: bool = env::var("HEIRLOOM_LOAD_TEST_DIAG").is_ok();
}
lazy_static! {
    /// Disables all audio systems (SoundSpawner, hit/break/use audio, BGM).
    pub static ref NO_AUDIO: bool = env::var("NO_AUDIO").is_ok();
}
lazy_static! {
    /// Applies poison stacks to all spawned enemies every 0.25s (see `gameplay_load_tests`).
    pub static ref POISON_LOAD_TEST: bool = env::var("POISON_LOAD_TEST").is_ok();
}
lazy_static! {
    /// Spams sword projectile + shout skill around the player (see `gameplay_load_tests`).
    pub static ref WEAPON_LOAD_TEST: bool = env::var("WEAPON_LOAD_TEST").is_ok();
}
lazy_static! {
    /// Spawns lingering loot drops that pile up and are collected via real pickup systems.
    pub static ref LOOT_CYCLE_LOAD_TEST: bool = env::var("LOOT_CYCLE_LOAD_TEST").is_ok();
}
lazy_static! {
    /// Logs entity/component counts every 5s to find accumulation leaks.
    pub static ref DIAGNOSTICS: bool = env::var("DIAGNOSTICS").is_ok();
}

fn main() {
    init_global_logger();

    // Export heirloom card data for asset pipeline (run with EXPORT_HEIRLOOMS=1)
    if std::env::var("EXPORT_HEIRLOOMS").is_ok() {
        export_heirloom_cards_data();
        return;
    }

    // Export active skill hover data for asset pipeline (run with EXPORT_SKILL_HOVERS=1)
    if std::env::var("EXPORT_SKILL_HOVERS").is_ok() {
        export_skill_hovers_data();
        return;
    }

    // migrate old save files
    let old_game_data = std::path::Path::new("game_data.json");
    if old_game_data.is_file() {
        std::fs::rename(old_game_data, datafiles::save_file()).expect("move game data file");
    }
    let old_save_state = std::path::Path::new("save_state.json");
    if old_save_state.is_file() {
        std::fs::rename(old_save_state, datafiles::save_file()).expect("move save file");
    }

    // ok now run the game :)
    let mut app = App::new();

    // macos bundles into a .app anyways, so we don't need to bundle assets.
    // doing it in the universal binary would double it since MacOS does M1 + Intel
    if cfg!(not(target_os = "macos")) {
        app.add_plugin(EmbeddedAssetPlugin);
    }

    let app = app
        .insert_resource(StartOfRunActionsHappened(false))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(crate::player::score::RunScore::new(false))
        .insert_resource(PlayerHealthPercent::default())
        .init_resource::<crate::player::skills::HeirloomTriggerCounts>()
        .init_resource::<crate::player::skills::ManaTrackerResetTimer>()
        .add_state::<GameState>()
        .edit_schedule(CoreSchedule::FixedUpdate, |s| {
            s.configure_set(CoreGameSet::Main.run_if(in_state(GameState::Main)));
        })
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Enable hot-reloading of assets:
                    watch_for_changes: false,
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: WindowResolution::new(WIDTH, HEIGHT),
                        title: "Willow: The Last Archivist".to_string(),
                        present_mode: PresentMode::Immediate,
                        resizable: true,
                        transparent: true,
                        mode: WindowMode::BorderlessFullscreen,
                        ..Default::default()
                    }),
                    ..default()
                })
                .build()
                .disable::<LogPlugin>(), // we handle logging ourselves
        )
        .add_plugin(RonAssetPlugin::<GraphicsDesc>::new(&["desc.ron"]))
        .add_plugin(RonAssetPlugin::<ClassPetData>::new(&["class.ron"]))
        .add_plugin(RonAssetPlugin::<ClassUnlockConfig>::new(&[
            "class_unlocks.ron",
        ]))
        .add_plugin(RonAssetPlugin::<RecipeListProto>::new(&["ron"]))
        .add_plugin(RonAssetPlugin::<world::grass_patches::GrassPatchesDesc>::new(&["patches.ron"]))
        .add_plugin(world::grass_patches::GrassPatchesPlugin)
        .insert_resource(Msaa::Off)
        .insert_resource(FixedTime::new_from_secs(TIME_STEP))
        .insert_resource(DisplayScaleSettings::load())
        .insert_resource(cursor::CursorColorSettings::load())
        .add_plugin(panic_handler::PanicHandler::new().build())
        .add_plugin(AsepritePlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugin(WorldInspectorPlugin::new().run_if(should_show_inspector))
        .add_plugin(TilemapPlugin)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(AudioPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(AnimationsPlugin)
        .add_plugin(InputsPlugin)
        .add_plugin(cursor::CustomCursorPlugin)
        .add_plugin(gamepad_input::GamepadInputPlugin)
        .add_plugin(aim::AimPlugin)
        .add_plugin(UIPlugin)
        .add_plugin(NightPlugin)
        .add_plugin(ChaosPlugin)
        .add_plugin(SaplingPlugin)
        .add_plugin(AIPlugin)
        .add_plugin(AttributesPlugin)
        .add_plugin(CombatPlugin)
        .add_plugin(EnemyPlugin)
        .add_plugin(PlayerPlugin)
        .add_plugin(WorldPlugin)
        .add_plugin(ClientPlugin)
        .add_plugin(client::leaderboard::LeaderboardPlugin)
        .add_plugin(ProtoPlugin)
        .add_plugin(JuicePlugin)
        .add_plugin(PetsPlugin)
        .add_plugin(BlessingsPlugin)
        // .add_plugin(DiagnosticExplorerAgentPlugin)
        .add_startup_system(setup)
        .add_system(update_pixel_perfect_viewport)
        .add_loading_state(
            LoadingState::new(GameState::Loading).continue_to_state(GameState::LoadingProtos),
        )
        .add_collection_to_loading_state::<_, ImageAssets>(GameState::Loading)
        .add_system(display_main_menu.in_schedule(OnEnter(GameState::MainMenu)))
        .add_system(
            cleanup_loading_screen
                .in_schedule(OnEnter(GameState::MainMenu))
                .after(display_main_menu),
        )
        .add_system(
            set_start_of_run_action_resource_true
                .run_if(run_once_per_run())
                .in_set(OnUpdate(GameState::Main)),
        )
        .add_systems((
            set_start_of_run_action_resource_false.in_schedule(OnEnter(GameState::GameOver)),
            set_start_of_run_action_resource_false.in_schedule(OnEnter(GameState::MainMenu)),
        ))
        .add_system(spawn_menu_text_buttons.in_schedule(OnEnter(GameState::MainMenu)))
        .add_system(handle_menu_button_click_events.run_if(not(in_state(GameState::Loading))))
        .add_system(remove_main_menu.in_schedule(OnExit(GameState::MainMenu)));

    if *COLLIDER_LOAD_TEST {
        app.add_plugin(collider_load_test::ColliderLoadTestPlugin);
    }
    if *HEIRLOOM_LOAD_TEST {
        app.add_plugin(gameplay_load_tests::HeirloomLoadTestPlugin);
    }
    if *PARTICLE_LOAD_TEST {
        app.add_plugin(gameplay_load_tests::ParticleLoadTestPlugin);
    }
    if *POISON_LOAD_TEST {
        app.add_plugin(gameplay_load_tests::PoisonLoadTestPlugin);
    }
    if *WEAPON_LOAD_TEST {
        app.add_plugin(gameplay_load_tests::WeaponLoadTestPlugin);
    }
    if *LOOT_CYCLE_LOAD_TEST {
        app.add_plugin(gameplay_load_tests::LootCycleLoadTestPlugin);
    }
    if *COLLIDER_LOAD_TEST
        || *HEIRLOOM_LOAD_TEST
        || *PARTICLE_LOAD_TEST
        || *POISON_LOAD_TEST
        || *WEAPON_LOAD_TEST
        || *LOOT_CYCLE_LOAD_TEST
    {
        app.add_system(
            gameplay_load_tests::unified_load_tests_f9_toggle.in_set(OnUpdate(GameState::Main)),
        );
    }

    if *DIAGNOSTICS {
        app.add_system(gameplay_load_tests::diagnostics_tick.in_set(OnUpdate(GameState::Main)));
        // Archetype diagnostic runs in every state so we can see whether
        // archetype counts persist across MainMenu <-> Main transitions.
        app.add_system(gameplay_load_tests::archetype_diagnostics_tick);
    }

    if *COLLIDERS {
        app.add_plugin(RapierDebugRenderPlugin::default());
    }

    if *DEBUG {
        app.add_system(log_entity_count);
    }

    app.run();
}

fn init_global_logger() {
    // init logging
    let log_file = LogFileInitializer {
        directory: datafiles::logs_dir(),
        filename: "survival-game.log",
        max_n_old_files: 10,
        preferred_max_file_size_mib: 0,
    }
    .init()
    .expect("Failed to initialize log file");

    let file_writer = Mutex::new(log_file);

    #[cfg(feature = "tracy")]
    fn skip_tracy_frame_mark(meta: &tracing::Metadata<'_>) -> bool {
        meta.fields().field("tracy.frame_mark").is_none()
    }

    // With trace_tracy, Bevy opens/closes spans for every schedule/system. FmtSpan::CLOSE on the
    // file layer writes one line per close — huge disk I/O and formatting cost; Tracy already records spans.
    #[cfg(feature = "tracy")]
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .event_format(format::format().compact())
        .with_filter(FilterFn::new(skip_tracy_frame_mark));
    #[cfg(not(feature = "tracy"))]
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_span_events(FmtSpan::CLOSE);

    // No FmtSpan on stdout: with trace_tracy, Bevy emits huge span trees; span CLOSE lines
    // flood the terminal and cost a lot to format (lag), while Tracy captures spans itself.
    // Compact format avoids walking/printing the full span stack on every log line (very costly with trace_tracy).
    #[cfg(feature = "tracy")]
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .event_format(format::format().compact())
        .with_filter(FilterFn::new(skip_tracy_frame_mark));
    #[cfg(not(feature = "tracy"))]
    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true);

    let subscriber = tracing_subscriber::registry()
        .with(
            EnvFilter::from_default_env()
                // renderer won't stfu lol
                .add_directive("bevy_render::renderer=warn".parse().unwrap())
                .add_directive(
                    if *DEBUG {
                        LevelFilter::INFO
                    } else {
                        LevelFilter::INFO
                    }
                    .into(),
                ),
        )
        .with(file_layer)
        .with(stdout_layer);

    #[cfg(feature = "tracy")]
    let subscriber = subscriber.with(tracing_tracy::TracyLayer::new());

    tracing::subscriber::set_global_default(subscriber)
        .expect("unable to set global logs subscriber");
}

#[derive(Resource)]
pub struct Game {
    player_state: PlayerState,
    home_pos: Option<TileMapPosition>,
    player: Entity,
}
impl Default for Game {
    fn default() -> Self {
        Self {
            player_state: PlayerState::default(),
            home_pos: None,
            player: Entity::from_raw(0),
        }
    }
}

/// Resource to track player health percentage for damage calculations
/// This avoids query conflicts with systems that modify CurrentHealth
#[derive(Resource, Default)]
pub struct PlayerHealthPercent {
    pub percent: f32,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum CoreGameSet {
    Main,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum GameState {
    #[default]
    Loading,
    LoadingProtos,
    LoadingProtosStage2, // Waits for base templates before loading everything
    MainMenu,
    Initializing, // New state for game initialization with loading screen
    Main,
    GameOver,
    BlessingChoice,
}
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
struct CustomFlush;

#[derive(Resource, AssetCollection)]
pub struct ImageAssets {
    #[asset(path = "bevy_survival_sprites.png")]
    pub sprite_sheet: Handle<Image>,
    #[asset(path = "NewTiles.png")]
    pub tiles_sheet: Handle<Image>,
    #[asset(path = "SmallWallTextures.png")]
    pub walls_sheet: Handle<Image>,
    #[asset(path = "textures/sprites.desc.ron")]
    pub sprite_desc: Handle<GraphicsDesc>,
    #[asset(path = "class_pet_data.class.ron")]
    pub class_desc: Handle<ClassPetData>,
    #[asset(path = "recipes/recipes.ron")]
    pub recipes: Handle<RecipeListProto>,
    #[asset(path = "data/class_unlocks.class_unlocks.ron")]
    pub class_unlocks: Handle<ClassUnlockConfig>,
    #[asset(path = "textures/grass_patches.png")]
    pub grass_patches_sheet: Handle<Image>,
    #[asset(path = "textures/grass_patches.patches.ron")]
    pub grass_patches_desc: Handle<world::grass_patches::GrassPatchesDesc>,
    #[asset(path = "textures/desert_patch.png")]
    pub desert_patch_sheet: Handle<Image>,
    #[asset(path = "textures/desert_patch_small.png")]
    pub desert_patch_small_sheet: Handle<Image>,
}

#[derive(Component)]
pub struct DoNotDespawnOnGameOver;

#[derive(SystemParam)]
pub struct GameParam<'w, 's> {
    pub game: ResMut<'w, Game>,
    pub graphics: Res<'w, Graphics>,
    pub resolution: Res<'w, ScreenResolution>,
    pub era: ResMut<'w, EraManager>,
    pub world_generation_params: ResMut<'w, WorldGeneration>,
    pub world_obj_data: ResMut<'w, WorldObjectResource>,
    pub world_obj_cache: ResMut<'w, WorldObjectCache>,

    //TODO: remove this to use Bevy_Save
    pub player_query:
        Query<'w, 's, (Entity, &'static PlayerSkills, &'static mut PlayerLevel), With<Player>>,
    pub player_stats: Query<
        'w,
        's,
        (
            &'static Attack,
            &'static MaxHealth,
            &'static Defence,
            &'static CritChance,
            &'static CritDamage,
            &'static BonusDamage,
            Option<&'static ComboCounter>,
            &'static HealthRegen,
            &'static Healing,
            &'static Thorns,
            &'static Dodge,
            &'static Speed,
            &'static Lifesteal,
            &'static XpRateBonus,
            &'static LootRateBonus,
        ),
    >,
    pub chunk_query: Query<'w, 's, (Entity, &'static Chunk)>,
    pub tile_collection_query: Query<'w, 's, &'static TileEntityCollection, With<Chunk>>,
    pub tile_data_query: Query<'w, 's, (&'static mut TileSpriteData, Option<&'static Children>)>,
    pub world_object_query: Query<
        'w,
        's,
        (
            Entity,
            &'static GlobalTransform,
            &'static SpriteSize,
            &'static WorldObject,
        ),
        Without<ItemStack>,
    >,
    pub wall_data_query: Query<'w, 's, (Entity, &'static mut WallTextureData)>,
    pub equipment: Query<'w, 's, (Entity, &'static Equipment)>,
    pub inv_slot_query: Query<'w, 's, &'static mut InventorySlotState>,
    pub coins: ResMut<'w, crate::player::currency::CoinCurrency>,
    pub heirloom_trigger_counts: ResMut<'w, HeirloomTriggerCounts>,

    pub time_fragments: ResMut<'w, crate::player::currency::TimeFragmentCurrency>,
    pub player_health_percent: Res<'w, PlayerHealthPercent>,
    pub stand_still_query: Query<
        'w,
        's,
        Option<&'static crate::player::combat_heirlooms::StandStillState>,
        With<Player>,
    >,
    pub crate_break_damage_query: Query<
        'w,
        's,
        Option<&'static crate::player::combat_heirlooms::CrateBreakDamageTracker>,
        With<Player>,
    >,
    pub dodge_crit_query: Query<
        'w,
        's,
        Option<&'static mut crate::player::combat_heirlooms::DodgeCritState>,
        With<Player>,
    >,
    pub mana_charge_damage_query: Query<
        'w,
        's,
        Option<&'static crate::player::combat_heirlooms::ManaChargeDamageState>,
        With<Player>,
    >,
    pub blessings_query: Query<'w, 's, &'static OwnedBlessings, With<Player>>,
    pub stealth_query: Query<'w, 's, Option<&'static Stealthed>, With<Player>>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}

impl<'w, 's> GameParam<'w, 's> {
    pub fn player(&self) -> PlayerState {
        self.game.player_state.clone()
    }
    pub fn get_player_level(&self) -> u8 {
        self.player_query.single().2.level
    }
    pub fn get_player_level_mut(&mut self) -> Mut<PlayerLevel> {
        self.player_query.single_mut().2
    }
    pub fn get_player_skills(&self) -> PlayerSkills {
        self.player_query.single().1.clone()
    }
    pub fn get_xp_rate_bonus(&self) -> i32 {
        self.player_stats.get_single().map(|s| s.13 .0).unwrap_or(0)
    }
    pub fn player_mut(&mut self) -> &mut PlayerState {
        &mut self.game.player_state
    }
    pub fn get_time_fragments(&self) -> i32 {
        self.time_fragments.time_fragments
    }
    pub fn get_coins(&self) -> u32 {
        self.coins.coins
    }
    pub fn get_chunk_entity(&self, chunk_pos: IVec2) -> Option<Entity> {
        for (e, chunk) in self.chunk_query.iter() {
            if chunk.chunk_pos == chunk_pos {
                return Some(e);
            }
        }
        None
    }

    pub fn add_object_to_chunk_cache(&mut self, pos: TileMapPosition, obj: WorldObject) {
        self.world_obj_cache.objects.insert(pos, obj);
    }
    pub fn remove_object_from_chunk_cache(&mut self, pos: TileMapPosition) {
        self.world_obj_cache.objects.remove(&pos);
    }
    pub fn add_object_to_dungeon_cache(&mut self, pos: TileMapPosition, obj: WorldObject) {
        self.world_obj_cache.dungeon_objects.insert(pos, obj);
    }
    pub fn remove_object_from_dungeon_cache(&mut self, pos: TileMapPosition) {
        self.world_obj_cache.dungeon_objects.remove(&pos);
    }
    pub fn clear_dungeon_cache(&mut self) {
        self.world_obj_cache.dungeon_objects.clear();
        self.world_obj_cache.generated_dungeon_chunks.clear();
    }
    pub fn get_objects_from_chunk_cache(
        &self,
        chunk_pos: IVec2,
    ) -> Vec<(TileMapPosition, WorldObject)> {
        let mut cache = vec![];
        for (pos, obj) in self.world_obj_cache.objects.iter() {
            if pos.chunk_pos == chunk_pos {
                cache.push((*pos, *obj));
            }
        }
        cache
    }
    pub fn get_objects_from_dungeon_cache(
        &self,
        chunk_pos: IVec2,
    ) -> Vec<(TileMapPosition, WorldObject)> {
        let mut cache = vec![];
        for (pos, obj) in self.world_obj_cache.dungeon_objects.iter() {
            if pos.chunk_pos == chunk_pos {
                cache.push((*pos, *obj));
            }
        }
        cache
    }
    pub fn is_chunk_generated(&self, chunk_pos: IVec2) -> bool {
        self.world_obj_cache.generated_chunks.contains(&chunk_pos)
    }
    pub fn set_chunk_generated(&mut self, chunk_pos: IVec2) {
        self.world_obj_cache.generated_chunks.push(chunk_pos);
    }
    pub fn is_dungeon_chunk_generated(&self, chunk_pos: IVec2) -> bool {
        self.world_obj_cache
            .generated_dungeon_chunks
            .contains(&chunk_pos)
    }
    pub fn set_dungeon_chunk_generated(&mut self, chunk_pos: IVec2) {
        self.world_obj_cache
            .generated_dungeon_chunks
            .push(chunk_pos);
    }
    pub fn get_object_from_chunk_cache(&self, pos: TileMapPosition) -> Option<&WorldObject> {
        self.world_obj_cache.objects.get(&pos)
    }

    pub fn get_tile_entity(&self, tile: TileMapPosition) -> Option<Entity> {
        if let Some(chunk_e) = self.get_chunk_entity(tile.chunk_pos) {
            let tile_collection = self.tile_collection_query.get(chunk_e).unwrap();
            return tile_collection.map.get(&tile.tile_pos.into()).copied();
        }
        None
    }
    pub fn get_tile_data_mut(&mut self, tile: TileMapPosition) -> Option<Mut<TileSpriteData>> {
        if let Some(tile_e) = self.get_tile_entity(tile) {
            return Some(self.tile_data_query.get_mut(tile_e).unwrap().0);
        }
        None
    }
    pub fn get_tile_data(&self, tile: TileMapPosition) -> Option<TileSpriteData> {
        if let Some(tile_e) = self.get_tile_entity(tile) {
            if let Ok(tile_sprite) = self.tile_data_query.get(tile_e) {
                return Some(tile_sprite.0.clone());
            }
        }
        None
    }
    pub fn get_obj_entity_at_tile(
        &self,
        tile: TileMapPosition,
        proto_param: &ProtoParam,
    ) -> Option<(Entity, WorldObject)> {
        for (obj_e, g_txm, size, obj) in self.world_object_query.iter() {
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(*obj)
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            let pos = world_pos_to_tile_pos(g_txm.translation().truncate() - anchor.0);
            if size.is_medium() {
                for neighbour_pos in pos
                    .get_neighbour_tiles_for_medium_objects()
                    .iter()
                    .chain([pos].iter())
                {
                    if neighbour_pos == &tile {
                        return Some((obj_e, *obj));
                    }
                }
            } else if pos == tile {
                return Some((obj_e, *obj));
            }
        }

        None
    }
    pub fn get_wall_data_at_tile(
        &self,
        tile: TileMapPosition,
        proto_param: &ProtoParam,
    ) -> Option<WallTextureData> {
        if let Some((e, _)) = self.get_obj_entity_at_tile(tile, proto_param) {
            if let Ok(data) = self.wall_data_query.get(e) {
                return Some(data.1.clone());
            }
        }
        None
    }
    pub fn get_wall_data_at_tile_mut(
        &mut self,
        tile: TileMapPosition,
        proto_param: &ProtoParam,
    ) -> Option<Mut<WallTextureData>> {
        if let Some((e, _)) = self.get_obj_entity_at_tile(tile, proto_param) {
            if let Ok(data) = self.wall_data_query.get_mut(e) {
                return Some(data.1);
            }
        }
        None
    }

    /// Returns (damage, was_crit, was_overcrit)
    /// Overcrit happens when crit chance > 100% and a second roll succeeds
    /// Overcrit does an additional 30% damage on top of crit damage
    /// frail_stacks: Number of frail stacks on target (each stack increases damage by 10%)
    /// bonus_crit_damage: Extra crit damage % added on top of `crit_dmg` when the
    /// hit crits (does not affect crit *chance*; non-crit damage is unaffected).
    pub fn calculate_player_damage(
        &self,
        bonus_crit: u32,
        dmg_mult: Option<f32>,
        dmg_bonus: u32,
        attack_override: Option<i32>,
        frail_stacks: u8,
        bonus_crit_damage: i32,
        // When true, this damage is from a weapon (melee swing / weapon projectile)
        // and is eligible for the Telescope (DodgeCrit) "next weapon hit does 2x" bonus.
        is_weapon_attack: bool,
    ) -> (u32, bool, bool) {
        let (attack, max_health, _, crit_chance, crit_dmg, bonus_dmg, combo_option, ..) =
            self.player_stats.single();
        let skills = self.get_player_skills();
        let mut rng = rand::thread_rng();
        let dmg_mult = dmg_mult.unwrap_or(1.);
        let dmg = attack_override.unwrap_or(attack.0);
        // DaggerCombo: counter is capped at increment time; 500 stacks = +100% crit damage
        let crit_dmb_bonus = if let Some(combo) = combo_option {
            (combo.counter as f32 * 100.0 / 500.0).round() as i32
        } else {
            0
        };
        // Convert bonus damage percentage to multiplier (e.g., 30% -> 1.3x)
        // BonusDamage now includes MaxHPDamage and GoldIntoDamage bonuses
        let mut bonus_damage_multiplier = 1.0 + (bonus_dmg.0 as f32 / 100.0);

        // StandStill: Standing still increases damage (ramps up over 3s)
        let stand_still_stacks = skills.get_count(Heirloom::StandStill);
        if stand_still_stacks > 0 {
            if let Ok(Some(stand_still_state)) = self.stand_still_query.get_single() {
                let stand_still_mult = stand_still_state.get_damage_multiplier(stand_still_stacks);

                bonus_damage_multiplier *= stand_still_mult;
            }
        }

        // CrateBreakDamage: Bonus damage from breaking crates
        if let Ok(Some(crate_tracker)) = self.crate_break_damage_query.get_single() {
            if crate_tracker.bonus_damage_percent > 0.0 {
                bonus_damage_multiplier += crate_tracker.bonus_damage_percent / 100.0;
            }
        }

        // AttackManaCost blessing: +10% damage when attacks cost mana
        if let Ok(blessings) = self.blessings_query.get_single() {
            bonus_damage_multiplier += blessings.get_attack_mana_cost_damage_bonus();
        }

        // LowHPDamage: More damage the lower HP is (80% -> 0% = 1x -> 1.75x)
        let low_hp_damage_stacks = skills.get_count(Heirloom::LowHPDamage);
        if low_hp_damage_stacks > 0 {
            let health_percent = self.player_health_percent.percent;
            // Only applies when health is below 80%
            if health_percent < 0.8 {
                // Linear scale from 80% -> 0% = 1x -> 1.75x (base), +0.3x per additional stack
                let max_bonus = 0.75 + (low_hp_damage_stacks - 1) as f32 * 0.3;
                let t = 1.0 - (health_percent / 0.8); // 0 at 80%, 1 at 0%
                bonus_damage_multiplier += max_bonus * t;
            }
        }

        // DodgeCrit (Telescope): the next source of weapon damage after a dodge does 2x.
        // Only weapon hits are eligible; the bonus is consumed in
        // `handle_dodge_crit_next_hit_reset` on the matching weapon HitEvent.
        let dodge_crit_next_hit_bonus = if let Ok(Some(state)) = self.dodge_crit_query.get_single()
        {
            state.next_hit_bonus
        } else {
            false
        };
        if is_weapon_attack && dodge_crit_next_hit_bonus {
            bonus_damage_multiplier *= 2.0;
            info!("[DodgeCrit] Next weapon hit bonus applied! 2x damage.");
        }

        // MPBarDMG: Mana regen charges up bonus flat damage
        // Note: The stored mana is reset in a separate system (handle_mana_charge_damage_reset)
        let mana_charge_bonus = {
            let mana_charge_stacks = skills.get_count(Heirloom::MPBarDMG);
            if mana_charge_stacks > 0 {
                if let Ok(Some(state)) = self.mana_charge_damage_query.get_single() {
                    state.get_damage(mana_charge_stacks)
                } else {
                    0
                }
            } else {
                0
            }
        };

        // Cap crit chance at 200%; excess is converted to crit damage at 1:1
        let total_crit_chance_raw = crit_chance.0.try_into().unwrap_or(0_u32) + bonus_crit;
        let effective_crit_chance = total_crit_chance_raw.min(200);
        let overflow_crit_damage = total_crit_chance_raw.saturating_sub(200) as i32;

        // Stealth: Force all damage to be crits
        let is_stealthed = self
            .stealth_query
            .get_single()
            .map(|s| s.is_some())
            .unwrap_or(false);

        // Determine if we crit and if we overcrit
        let (did_crit, did_overcrit) =
            if effective_crit_chance >= 100 || self.player().next_hit_crit || is_stealthed {
                // Guaranteed crit if >= 100%
                // Check for overcrit if crit > 100%
                let overcrit_chance = effective_crit_chance.saturating_sub(100);
                let is_overcrit =
                    overcrit_chance > 0 && rng.gen_ratio(u32::min(100, overcrit_chance), 100);
                (true, is_overcrit)
            } else {
                // Normal crit roll
                let is_crit = rng.gen_ratio(effective_crit_chance, 100);
                (is_crit, false)
            };

        // Frail multiplier: 1.1x damage per stack (applied multiplicatively at the end)
        let frail_multiplier = if frail_stacks > 0 {
            1.1_f32.powi(frail_stacks as i32)
        } else {
            1.0
        };

        // Total flat bonus damage (including mana charge)
        let total_dmg_bonus = dmg_bonus as i32 + mana_charge_bonus;

        if did_crit {
            // Base crit damage (includes overflow from crit chance > 200% at 1:1 ratio
            // and any per-skill bonus crit damage, e.g. ArrowVolley scaling with crit chance)
            let crit_multiplier = f32::abs(
                (crit_dmg.0 + crit_dmb_bonus + overflow_crit_damage + bonus_crit_damage) as f32,
            ) / 100.;
            // Overcrit adds an extra 50% on top
            let overcrit_multiplier = if did_overcrit { 1.5 } else { 1.0 };
            (
                ((dmg_mult * (dmg + total_dmg_bonus) as f32)
                    * bonus_damage_multiplier
                    * crit_multiplier
                    * overcrit_multiplier
                    * frail_multiplier) as u32, // Apply Frail multiplicatively
                true,
                did_overcrit,
            )
        } else {
            (
                ((dmg_mult * (dmg + total_dmg_bonus) as f32)
                    * bonus_damage_multiplier
                    * frail_multiplier) as u32, // Apply Frail multiplicatively
                false,
                false,
            )
        }
    }
    pub fn has_skill(&self, skill: Heirloom) -> bool {
        self.player_query.single().1.has(skill)
    }
    pub fn skill_count(&self, skill: Heirloom) -> i32 {
        self.player_query.single().1.get_count(skill)
    }
}

#[derive(Component, Default)]
pub struct MainCamera;
#[derive(Component, Default)]
pub struct TextureCamera;
#[derive(Component, Default)]
pub struct UICamera;
#[derive(Component, Debug, Default)]
pub struct RawPosition(Vec2);

impl Deref for RawPosition {
    type Target = Vec2;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for RawPosition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Resource, Clone)]
pub struct ScreenResolution {
    pub width: f32,
    pub height: f32,
    /// UI camera view width in world units (1 unit == [`Self::scale`] physical pixels).
    pub game_width: f32,
    /// UI camera view height in world units (1 unit == [`Self::scale`] physical pixels).
    pub game_height: f32,
    pub aspect_ratio: f32,
    /// Integer scale factor used for the UI camera's pixel-perfect rendering. This is the value
    /// every existing call site that does HUD / screen-edge math wants. World-camera math uses
    /// [`Self::world_scale`] instead.
    pub scale: u32,
    /// Game / world camera view width in world units (1 unit == [`Self::world_scale`] physical pixels).
    pub world_view_width: f32,
    /// Game / world camera view height in world units (1 unit == [`Self::world_scale`] physical pixels).
    pub world_view_height: f32,
    /// Integer scale factor used for the world / game camera's pixel-perfect rendering. Independent
    /// of [`Self::scale`] (the UI scale); configured via [`DisplayScaleSettings`]. When this is
    /// larger than [`Self::scale`] the game is zoomed in relative to the UI.
    pub world_scale: u32,
    /// Actual render texture width (base width * scale)
    pub render_width: u32,
    /// Actual render texture height (base height * scale)
    pub render_height: u32,
    /// Letterbox/pillarbox offset for centering the game view
    pub viewport_offset: Vec2,
    /// Size of the actual viewport (may be smaller than window due to letterboxing)
    pub viewport_size: Vec2,
}
/// Calculate pixel-perfect resolution by deriving `game_height` / `world_view_height` from the
/// window size. Instead of forcing a fixed game_height into a viewport, we adjust each
/// camera's visible-units so that `window_height / view_height` is an exact integer (the scale
/// factor). This means each camera renders to the full window — no viewport, no letterbox,
/// and guaranteed integer texel-to-pixel mapping.
///
/// The UI camera and the world camera each get their own integer scale from
/// [`DisplayScaleSettings`], relative to the legacy `ZOOM_SCALE = 1.2` reference bucket.
fn calculate_pixel_perfect_resolution(
    window_width: f32,
    window_height: f32,
    display_scale: &DisplayScaleSettings,
) -> ScreenResolution {
    let scale = display_scale.ui_scale_for_window(window_height);
    let game_height = window_height / scale as f32;
    let game_width = window_width / scale as f32;

    let world_scale = display_scale.game_scale_for_window(window_height);
    let world_view_height = window_height / world_scale as f32;
    let world_view_width = window_width / world_scale as f32;

    let aspect_ratio = window_width / window_height;

    let render_width = window_width as u32;
    let render_height = window_height as u32;

    let reference = display_scale::reference_scale(window_height);

    info!(
        "Pixel-perfect resolution: window={}x{} | UI scale={} (ref {} step {:+}) game={}x{} | world scale={} (step {:+}) view={}x{}",
        window_width,
        window_height,
        scale,
        reference,
        display_scale.clamped_ui_steps(),
        game_width,
        game_height,
        world_scale,
        display_scale.clamped_game_steps(),
        world_view_width,
        world_view_height,
    );

    let screen = ScreenResolution {
        width: window_width,
        height: window_height,
        game_width,
        game_height,
        aspect_ratio,
        scale,
        world_view_width,
        world_view_height,
        world_scale,
        render_width,
        render_height,
        viewport_offset: Vec2::ZERO,
        viewport_size: Vec2::new(window_width, window_height),
    };
    phase1_log_resolution_after_calc(&screen);
    screen
}

/// Phase 1 pixel-grid diagnostics (`DEBUG=1`). Logs parity of `game_* * scale` vs physical size,
/// odd-scale flag, and render dimensions for correlating soft text with subpixel layout.
fn phase1_log_resolution_after_calc(res: &ScreenResolution) {
    if !*DEBUG {
        return;
    }
    let parity_h = res.game_width * res.scale as f32 - res.width;
    let parity_v = res.game_height * res.scale as f32 - res.height;
    let odd_scale = res.scale % 2 == 1;
    info!(
        "Phase1 pixel_grid: scale={} odd_scale={} game_wh=({:.4}x{:.4}) phys_wh=({:.1}x{:.1}) render={}x{} parity_delta_px=({:.6},{:.6}) target_game_height={}",
        res.scale,
        odd_scale,
        res.game_width,
        res.game_height,
        res.width,
        res.height,
        res.render_width,
        res.render_height,
        parity_h,
        parity_v,
        GAME_HEIGHT,
    );
}

/// Window DPI / logical vs physical (`DEBUG=1`), for cursor vs projection hypotheses.
fn phase1_log_window(context: &str, window: &Window, res: &ScreenResolution) {
    if !*DEBUG {
        return;
    }
    let phys_w = window.resolution.physical_width() as f32;
    let phys_h = window.resolution.physical_height() as f32;
    info!(
        "Phase1 window [{}]: logical={:.1}x{:.1} physical={}x{} scale_factor={:.4} | res.width/height minus phys=({:.3},{:.3})",
        context,
        window.width(),
        window.height(),
        window.resolution.physical_width(),
        window.resolution.physical_height(),
        window.resolution.scale_factor(),
        res.width - phys_w,
        res.height - phys_h,
    );
}

fn setup(
    mut commands: Commands,
    window_query: Query<&Window, With<PrimaryWindow>>,
    display_scale: Res<DisplayScaleSettings>,
) {
    let window = window_query.get_single().ok();
    let resolution = if let Some(window) = window {
        let phys_w = window.resolution.physical_width() as f32;
        let phys_h = window.resolution.physical_height() as f32;
        info!(
            "Window detected: physical={}x{}, logical={}x{}, scale_factor={:.2}",
            phys_w,
            phys_h,
            window.width(),
            window.height(),
            window.resolution.scale_factor()
        );
        let res = calculate_pixel_perfect_resolution(phys_w, phys_h, &display_scale);
        phase1_log_window("startup", window, &res);
        res
    } else {
        info!("No window found, using default resolution");
        calculate_pixel_perfect_resolution(WIDTH, HEIGHT, &display_scale)
    };
    commands.insert_resource(resolution.clone());

    // Game camera — renders the game world directly to the full window.
    // No viewport: `world_view_height` is derived from `window_height / world_scale`, so the
    // scaling is guaranteed integer. Zoom is driven by [`DisplayScaleSettings`].
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                order: 0,
                ..default()
            },
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::Custom(Color::BLACK),
            },
            projection: OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical(resolution.world_view_height),
                scale: 1.0,
                ..default()
            },
            ..default()
        },
        DoNotDespawnOnGameOver,
        MainCamera,
        TextureCamera,
        RawPosition::default(),
    ));

    // UI camera — renders UI elements (layer 3) on top. Uses the UI bucket (`scale`,
    // `game_height`) so every existing HUD layout / screen-edge calculation keeps working
    // regardless of how the world camera is zoomed.
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                order: 1,
                ..default()
            },
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::None,
            },
            projection: OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical(resolution.game_height),
                scale: 1.0,
                ..default()
            },
            ..default()
        },
        DoNotDespawnOnGameOver,
        UICamera,
        RenderLayers::from_layers(&[3]),
    ));
}

/// Recalculates pixel-perfect scaling whenever the window or camera target size changes.
/// Uses the actual Camera render target size (what wgpu renders to) instead of
/// the window's reported size — these can differ on macOS Retina displays.
///
/// The UI camera ([`UICamera`]) and the game camera ([`TextureCamera`]) get **different**
/// `FixedVertical` values — `game_height` vs `world_view_height` — so they can pick different
/// integer pixel buckets. See [`calculate_pixel_perfect_resolution`] and [`DisplayScaleSettings`].
pub fn update_pixel_perfect_viewport(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<
        (&Camera, &mut OrthographicProjection, Option<&UICamera>),
        Or<(With<TextureCamera>, With<UICamera>)>,
    >,
    mut last_key: Local<(UVec2, i8, i8)>,
    display_scale: Res<DisplayScaleSettings>,
    mut resolution: ResMut<ScreenResolution>,
) {
    let Ok(window) = windows.get_single() else {
        return;
    };

    // Use the camera's actual physical render target size.
    // This is what wgpu is *really* rendering to — source of truth.
    let target_size = cameras
        .iter()
        .next()
        .and_then(|(cam, _, _)| cam.physical_target_size())
        .unwrap_or(UVec2::new(
            window.resolution.physical_width(),
            window.resolution.physical_height(),
        ));

    let key = (
        target_size,
        display_scale.clamped_ui_steps(),
        display_scale.clamped_game_steps(),
    );

    if *last_key == key {
        return;
    }

    let size_changed = last_key.0 != target_size;
    if size_changed {
        info!(
            "Render target changed: {}x{} -> {}x{} | window physical: {}x{}, logical: {:.0}x{:.0}, scale_factor: {:.3}",
            last_key.0.x,
            last_key.0.y,
            target_size.x,
            target_size.y,
            window.resolution.physical_width(),
            window.resolution.physical_height(),
            window.width(),
            window.height(),
            window.resolution.scale_factor()
        );
    } else if *DEBUG {
        info!(
            "Display scale changed: ui step {:+} game step {:+}",
            display_scale.clamped_ui_steps(),
            display_scale.clamped_game_steps(),
        );
    }
    *last_key = key;

    let new_res = calculate_pixel_perfect_resolution(
        target_size.x as f32,
        target_size.y as f32,
        &display_scale,
    );

    if *DEBUG {
        phase1_log_window("viewport_resize", &window, &new_res);
        let window_phys = UVec2::new(
            window.resolution.physical_width(),
            window.resolution.physical_height(),
        );
        if target_size != window_phys {
            warn!(
                "Phase1: camera physical_target {}x{} != window physical {}x{} (check HiDPI / viewport)",
                target_size.x,
                target_size.y,
                window_phys.x,
                window_phys.y,
            );
        }
    }

    for (_, mut proj, ui_marker) in cameras.iter_mut() {
        proj.scaling_mode = if ui_marker.is_some() {
            ScalingMode::FixedVertical(new_res.game_height)
        } else {
            ScalingMode::FixedVertical(new_res.world_view_height)
        };
    }

    *resolution = new_res;
}

trait AppExt {
    fn with_default_schedule(&mut self, s: impl ScheduleLabel, f: impl Fn(&mut App)) -> &mut App;
}

impl AppExt for App {
    fn with_default_schedule(
        &mut self,
        schedule: impl ScheduleLabel,
        f: impl Fn(&mut App),
    ) -> &mut App {
        let orig_default = self.default_schedule_label.clone();
        self.default_schedule_label = Box::new(schedule);
        f(self);
        self.default_schedule_label = orig_default;
        self
    }
}

pub fn should_show_inspector() -> bool {
    *DEBUG
}

fn log_entity_count(
    entities: Query<Entity>,
    fog: Option<Res<crate::ui::minimap::FogOfWarData>>,
    cache: Option<Res<crate::ui::minimap::MinimapTileCache>>,
    mut timer: Local<Option<Timer>>,
    time: Res<Time>,
) {
    let timer = timer.get_or_insert_with(|| Timer::from_seconds(5.0, TimerMode::Repeating));
    timer.tick(time.delta());
    if !timer.just_finished() {
        return;
    }
    let count = entities.iter().count();
    let fog_tiles = fog.map_or(0, |f| f.explored_tiles.len());
    let cache_terrain = cache.map_or(0, |c| c.explored_terrain.len());
    info!(
        "[PERF] entities: {}, fog_tiles: {}, cache_terrain: {}",
        count, fog_tiles, cache_terrain
    );
}

#[derive(Resource)]
pub struct StartOfRunActionsHappened(pub bool);

pub fn run_once_per_run() -> impl Fn(Res<StartOfRunActionsHappened>) -> bool {
    move |res: Res<StartOfRunActionsHappened>| {
        if res.0 {
            false
        } else {
            true
        }
    }
}

pub fn set_start_of_run_action_resource_true(mut res: ResMut<StartOfRunActionsHappened>) {
    res.0 = true;
}

pub fn set_start_of_run_action_resource_false(mut res: ResMut<StartOfRunActionsHappened>) {
    res.0 = false;
}

/// Exports active skill hover data (id, title, description_lines) to JSON for the
/// asset pipeline script. Run with: EXPORT_SKILL_HOVERS=1 cargo run
fn export_skill_hovers_data() {
    use player::skills::{
        active_skill_scaling::METEOR_SHOWER_BASE_COUNT, get_disabled_skills, ActiveSkill,
    };
    use std::collections::HashSet;
    use strum::IntoEnumIterator;

    let disabled: HashSet<ActiveSkill> = get_disabled_skills().into_iter().collect();

    let export: Vec<serde_json::Value> = ActiveSkill::iter()
        .filter(|skill| !disabled.contains(skill))
        .map(|skill| {
            serde_json::json!({
                "id": format!("{:?}", skill),
                "title": skill.get_title(),
                "description_lines": skill.get_desc(
                    1.0,
                    100,
                    100,
                    1.0,
                    0,
                    0,
                    0,
                    METEOR_SHOWER_BASE_COUNT,
                ),
                "is_movement_skill": skill.is_movement_skill(),
            })
        })
        .collect();

    let out_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("assets")
        .join("skill_hovers_export.json");
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(
        &out_path,
        serde_json::to_string_pretty(&export).expect("serialize"),
    ) {
        Ok(()) => println!("Exported {} skill hovers to {:?}", export.len(), out_path),
        Err(e) => eprintln!("Failed to write skill hover export: {}", e),
    }
}

/// Exports heirloom card data (id, title, description_lines, rarity) to JSON for the
/// asset pipeline script. Run with: EXPORT_HEIRLOOMS=1 cargo run
fn export_heirloom_cards_data() {
    use player::skills::HeirloomChoiceQueue;

    // Use the all-unlocks variant so the export tool dumps every heirloom regardless
    // of the player's current TimeCrystals progress.
    let pool = HeirloomChoiceQueue::with_all_unlocks().pool;
    let export: Vec<serde_json::Value> = pool
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": format!("{:?}", s.heirloom),
                "title": s.heirloom.get_title(),
                "description_lines": s.heirloom.get_desc(),
                "rarity": format!("{:?}", s.rarity),
            })
        })
        .collect();

    let out_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("assets")
        .join("heirloom_cards_export.json");
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(
        &out_path,
        serde_json::to_string_pretty(&export).expect("serialize"),
    ) {
        Ok(()) => println!("Exported {} heirlooms to {:?}", export.len(), out_path),
        Err(e) => eprintln!("Failed to write heirloom export: {}", e),
    }
}
