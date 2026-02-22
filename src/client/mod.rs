use std::{
    fs::File,
    io::{BufReader, BufWriter},
};

use bevy::{
    math::Vec3Swizzles,
    prelude::*,
    utils::{HashMap, Uuid},
};
use bevy_ecs_tilemap::{
    prelude::{
        TilemapGridSize, TilemapId, TilemapSize, TilemapSpacing, TilemapTexture, TilemapTileSize,
        TilemapType,
    },
    tiles::{TileColor, TileFlip, TilePos, TilePosOld, TileStorage, TileTextureIndex, TileVisible},
    FrustumCulling,
};
use bevy_proto::prelude::ProtoCommands;
use bevy_save::prelude::*;
use rand::Rng;
pub mod analytics;
pub mod leaderboard;
use analytics::*;
use serde::{Deserialize, Serialize};

use crate::{
    animations::ui_animaitons::MoveUIAnimation,
    attributes::{hunger::Hunger, CurrentHealth},
    chaos::ChaosTracker,
    container::{Container, ContainerRegistry},
    datafiles,
    inventory::{Inventory, ItemStack},
    item::{projectile::Projectile, CraftingTracker, Foliage, MainHand, Wall, WorldObject},
    night::NightTracker,
    player::{
        achievements::Achievements,
        check_first_run_achievement,
        class_rank::ClassRankSystem,
        currency::TimeFragmentCurrency,
        levels::PlayerLevel,
        score::{HighScores, RunScore},
        skills::{HeirloomChoiceQueue, PlayerClass, PlayerSkills, SkillClass},
        stats::{PlayerStats, SkillPoints},
        unlocks::{UnlockUpgrades, UnlockedClasses},
        Player,
    },
    ui::{
        tips::{SeenTips, Tip},
        ChestContainer, FurnaceContainer,
    },
    vectorize::{vectorize, vectorize_inner},
    world::{
        chunk::{Chunk, ReflectedPos, TileEntityCollection, TileSpriteData},
        dimension::{
            ActiveDimension, Dimension, DimensionSpawnEvent, Era, EraManager, GenerationSeed,
        },
        dungeon::Dungeon,
        generation::WorldObjectCache,
        world_helpers::world_pos_to_tile_pos,
        y_sort::YSort,
        WallTextureData, WorldGeneration,
    },
    CustomFlush, GameParam, GameState, MainCamera, RawPosition, TextureCamera, TileMapPosition,
    UICamera,
};

#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct ColliderReflect {
    collider: Vec2,
}
pub struct ClientPlugin;
//TODO: Temp does not work, Save/Load WIP
impl Plugin for ClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<GameOverEvent>()
            .add_state::<ClientState>()
            .add_plugins(SavePlugins)
            .register_saveable::<GenerationSeed>()
            .register_saveable::<Dimension>()
            .register_saveable::<ActiveDimension>()
            // register tile bundle types
            .register_saveable::<TileSpriteData>()
            .register_saveable::<TilePos>()
            .register_saveable::<TileTextureIndex>()
            .register_saveable::<TilemapId>()
            .register_saveable::<TileVisible>()
            .register_saveable::<TileFlip>()
            .register_saveable::<TileColor>()
            .register_saveable::<TilePosOld>()
            // register chunk bundle types
            .register_saveable::<Chunk>()
            .register_saveable::<TilemapGridSize>()
            .register_saveable::<TilemapType>()
            .register_saveable::<TilemapSize>()
            .register_saveable::<TilemapSpacing>()
            .register_saveable::<TileStorage>()
            .register_saveable::<TilemapTexture>()
            .register_saveable::<TilemapTileSize>()
            .register_saveable::<FrustumCulling>()
            .register_saveable::<GlobalTransform>()
            .register_saveable::<ComputedVisibility>()
            .register_saveable::<TileEntityCollection>()
            // register obj types
            .register_saveable::<WorldObject>()
            .register_saveable::<Foliage>()
            .register_saveable::<Wall>()
            // .register_saveable::<Mesh2dHandle>()
            // .register_saveable::<Handle<FoliageMaterial>>()
            // .register_saveable::<Handle<TextureAtlas>>()
            .register_saveable::<TextureAtlasSprite>()
            .register_saveable::<CurrentHealth>()
            .register_saveable::<WallTextureData>()
            .register_saveable::<YSort>()
            .register_saveable::<TileMapPosition>()
            .register_saveable::<ColliderReflect>()
            .register_saveable::<Name>()
            .register_saveable::<Parent>()
            .register_saveable::<Children>()
            .register_type::<Option<Entity>>()
            .register_type::<Vec<Option<Entity>>>()
            .register_type::<WorldObject>()
            .register_type::<ReflectedPos>()
            .register_type::<WorldGeneration>()
            .register_type_data::<ReflectedPos, ReflectSerialize>()
            .register_type_data::<ReflectedPos, ReflectDeserialize>()
            .register_type::<HashMap<ReflectedPos, Entity>>()
            .register_type::<[WorldObject; 4]>()
            .insert_resource(AppDespawnMode::new(DespawnMode::None))
            .insert_resource(AppMappingMode::new(MappingMode::Strict))
            .insert_resource(CurrentRunSaveData::default())
            .insert_resource(SaveTimer {
                timer: Timer::from_seconds(100., TimerMode::Repeating),
            })
            .add_plugin(AnalyticsPlugin)
            .add_system(
                load_state
                    .after(add_analytics_resource_on_start)
                    .in_schedule(OnExit(GameState::MainMenu)),
            )
            .add_system(load_game_data_for_ui.in_schedule(OnEnter(GameState::MainMenu)))
            .add_system(
                crate::ui::check_show_name_entry_popup
                    .after(load_game_data_for_ui)
                    .in_schedule(OnEnter(GameState::MainMenu)),
            )
            .add_systems(
                (
                    // save_state.run_if(resource_exists::<AnalyticsData>()),
                    // tick_save_timer,
                    handle_append_run_data_after_death
                        .run_if(resource_exists::<AnalyticsData>())
                        .after(check_first_run_achievement),
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

#[derive(Resource, Clone, Serialize, Deserialize, Default)]
pub struct CurrentRunSaveData {
    seed: u64,

    #[serde(with = "vectorize_inner")]
    placed_objs: Vec<HashMap<TileMapPosition, WorldObject>>,

    unique_objs: Vec<HashMap<WorldObject, TileMapPosition>>,
    #[serde(with = "vectorize")]
    containers: HashMap<TileMapPosition, Container>,
    #[serde(with = "vectorize")]
    container_reg: HashMap<TileMapPosition, Container>,
    craft_tracker: CraftingTracker,
    night_tracker: NightTracker,
    chaos_tracker: ChaosTracker,

    //Player Data
    pub inventory: Inventory,
    pub player_level: PlayerLevel,
    pub player_stats: PlayerStats,
    pub skill_points: SkillPoints,
    pub current_health: CurrentHealth,
    pub player_transform: Vec2,
    pub player_hunger: u8,
    pub player_skills: PlayerSkills,
    pub player_skill_queue: HeirloomChoiceQueue,
    pub currency: (i32, i32),

    // Era
    pub current_era: Era,
    pub visited_eras: Vec<Era>,
    pub analytics_data: AnalyticsData,
}

#[derive(Default)]
pub struct GameOverEvent;

#[derive(Resource, Default)]
pub struct SaveTimer {
    timer: Timer,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States, Component)]
pub enum ClientState {
    #[default]
    Unpaused,
    Paused,
}
#[derive(Resource, Clone, Serialize, Deserialize, Default)]
pub struct GameData {
    pub num_runs: u128,
    pub time_fragments: u128,
    pub melee_points: u128,
    pub rogue_points: u128,
    pub magic_points: u128,
    pub longest_run: u8,
    pub user_id: String,
    pub class_ranks: ClassRankSystem,
    pub high_scores: HighScores,
    pub achievements: Achievements,
    #[serde(default)]
    pub unlocked_classes: Vec<SkillClass>,
    #[serde(default)]
    pub unlock_upgrades: UnlockUpgrades,
    #[serde(default)]
    pub cumulative_analytics: Option<AnalyticsData>,
    #[serde(default)]
    pub keybindings: Option<crate::keybinds::InputMappings>,
    #[serde(default)]
    pub player_name: Option<String>,
    #[serde(default)]
    pub bounce_tracker: crate::player::achievements::BounceAchievementTracker,
    #[serde(default)]
    pub seen_tips: std::collections::HashSet<Tip>,
}
pub fn handle_append_run_data_after_death(
    night: Res<NightTracker>,
    mut game_over: EventReader<GameOverEvent>,
    mut analytics_data: ResMut<AnalyticsData>,
    all_time_fragments: Query<Entity, With<MoveUIAnimation>>,
    mut commands: Commands,
    time_fragments: Res<TimeFragmentCurrency>,
    player_class: Option<Res<PlayerClass>>,
    run_score: Option<Res<RunScore>>,
    achievements: ResMut<Achievements>,
    unlocked_classes: Option<Res<UnlockedClasses>>,
    unlock_upgrades: Option<Res<UnlockUpgrades>>,
    bounce_tracker: Res<crate::player::achievements::BounceAchievementTracker>,
) {
    for _ in game_over.iter() {
        info!("GAME OVER! Storing run data in game_data.json...");
        let mut game_data: GameData = GameData::default();
        let game_data_file_path = datafiles::game_data();
        if let Ok(file_file) = File::open(&game_data_file_path) {
            let reader = BufReader::new(file_file);

            // Read the JSON contents of the file as an instance of `GameData`.
            match serde_json::from_reader::<_, GameData>(reader) {
                Ok(data) => {
                    game_data = data;
                    info!(
                        "Loaded existing game_data.json with {} achievements",
                        game_data.achievements.unlocked.len()
                    );
                }
                Err(err) => {
                    error!("Failed to load data from game_data.json file {err:?}");
                    // If deserialization fails, start fresh but try to preserve achievements from resource
                    game_data.achievements = achievements.clone();
                }
            }
        } else {
            // File doesn't exist, initialize with achievements from resource if available
            game_data.achievements = achievements.clone();
        }
        // game_data.achievements = achievements.clone();
        game_data.num_runs += 1;
        if game_data.longest_run < night.days {
            game_data.longest_run = night.days;
        }
        let currency = time_fragments.as_ref();
        game_data.time_fragments = currency.time_fragments.max(0) as u128;
        if game_data.user_id.is_empty() {
            game_data.user_id = Uuid::new_v4().to_string();
        }
        analytics_data.user_id = game_data.user_id.clone();

        // Award class rank experience based on run performance
        if let Some(class) = &player_class {
            // Get mobs killed from run score (if available)
            let mobs_killed = run_score.as_ref().map(|rs| rs.mobs_killed).unwrap_or(0);
            let run_experience = calculate_class_experience(night.days, mobs_killed);
            let rank_increased = game_data
                .class_ranks
                .add_class_experience(&class.class, run_experience);

            if rank_increased {
                let new_rank = game_data.class_ranks.get_class_rank(&class.class).rank;
                let new_rarity = game_data
                    .class_ranks
                    .get_class_rank(&class.class)
                    .get_starting_weapon_rarity();
                info!(
                    "Class {:?} ranked up to rank {}! Starting weapon rarity upgraded to {:?}",
                    class.class, new_rank, new_rarity
                );
            }

            info!(
                "Awarded {} class experience to {:?} (Total: {})",
                run_experience,
                class.class,
                game_data
                    .class_ranks
                    .get_class_rank(&class.class)
                    .total_experience
            );
        }

        // Update high scores
        if let Some(score) = &run_score {
            if let Some(class) = &player_class {
                game_data
                    .high_scores
                    .update_high_score(score.score, &class.class);
            }
        }

        // Update achievements from resource (merge with any from file)
        // This ensures we preserve achievements that were already saved and add new ones
        // Merge achievements: add any new ones from the resource to the loaded game_data
        info!(
            "Merging {:?} achievements from resource into game data",
            achievements.clone()
        );
        for achievement in achievements.clone().unlocked {
            if !game_data.achievements.has(achievement) {
                game_data.achievements.unlock(achievement);
                info!("Saving newly unlocked achievement: {:?}", achievement);
            }
        }

        // Merge analytics data into cumulative analytics
        if let Some(ref mut cumulative) = game_data.cumulative_analytics {
            // Merge mobs_killed
            for (mob, count) in analytics_data.mobs_killed.iter() {
                *cumulative.mobs_killed.entry(mob.clone()).or_insert(0) += count;
            }
            // Merge items_collected
            for (item, count) in analytics_data.items_collected.iter() {
                *cumulative.items_collected.entry(item.clone()).or_insert(0) += count;
            }
            // Update other cumulative stats
            cumulative.total_recipes_crafted += analytics_data.total_recipes_crafted;
            cumulative.total_damage_taken += analytics_data.total_damage_taken;
            cumulative.total_damage_dealt += analytics_data.total_damage_dealt;
            cumulative.total_objects_broken += analytics_data.total_objects_broken;
            cumulative.total_objects_placed += analytics_data.total_objects_placed;
            cumulative.total_items_consumed += analytics_data.total_items_consumed;
            cumulative.nights_survived += analytics_data.nights_survived;
        } else {
            // Initialize cumulative analytics with current run data
            game_data.cumulative_analytics = Some(analytics_data.clone());
        }

        let class_ranks = game_data.class_ranks.clone();
        let high_scores = game_data.high_scores.clone();
        commands.insert_resource(class_ranks);
        commands.insert_resource(high_scores);

        if let Some(classes) = unlocked_classes.as_ref() {
            game_data.unlocked_classes = classes.to_vec();
        }
        if let Some(upgrades) = unlock_upgrades.as_ref() {
            game_data.unlock_upgrades = upgrades.as_ref().clone();
        }

        // Save bounce tracker
        game_data.bounce_tracker = bounce_tracker.clone();

        let game_data_path = datafiles::game_data();

        let file = File::create(game_data_path)
            .expect("Could not create game data file for serialization");

        // let json_Data: String = serde_json::to_string(&save_data).unwrap();
        if let Err(result) = serde_json::to_writer(file, &game_data.clone()) {
            error!("Failed to save game data after death: {result:?}");
        } else {
            info!(
                "UPDATED GAME DATA with {} achievements",
                game_data.achievements.unlocked.len()
            );
        }

        //despawn ui animations
        for e in all_time_fragments.iter() {
            commands.entity(e).despawn_recursive();
        }
    }
}
pub fn tick_save_timer(mut timer: ResMut<SaveTimer>, time: Res<Time>) {
    timer.timer.tick(time.delta());
}
pub fn save_state(
    mut timer: ResMut<SaveTimer>,
    placed_objs: Query<
        (
            &GlobalTransform,
            &WorldObject,
            Option<&ChestContainer>,
            Option<&FurnaceContainer>,
        ),
        (Without<ItemStack>, Without<MainHand>, Without<Projectile>),
    >,
    mut save_data: ResMut<CurrentRunSaveData>,
    player_data: Query<
        (
            &GlobalTransform,
            &PlayerStats,
            &CurrentHealth,
            &Hunger,
            &Inventory,
            &PlayerSkills,
        ),
        With<Player>,
    >,
    container_reg: Res<ContainerRegistry>,
    craft_tracker: Res<CraftingTracker>,
    dungeon_check: Query<&Dungeon>,
    night_tracker: Res<NightTracker>,
    seed: Res<GenerationSeed>,
    check_open_chest: Option<Res<ChestContainer>>,
    key_input: ResMut<Input<KeyCode>>,
    skills_queue: Res<HeirloomChoiceQueue>,
    analytics_data: Res<AnalyticsData>,
    chaos_tracker: Option<Res<ChaosTracker>>,
    game: GameParam,
) {
    // only save if the timer is done and we are not in a dungeon
    if dungeon_check.get_single().is_ok() {
        return;
    }
    if !timer.timer.just_finished() && !key_input.just_pressed(KeyCode::Escape) {
        return;
    }
    timer.timer.reset();
    //PlayerData
    let (player_txfm, stats, hp, hunger, inv, skills) = player_data.single();
    save_data.player_transform = player_txfm.translation().xy();
    save_data.player_stats = stats.clone();
    save_data.player_level = game.player_query.single().2.clone();
    save_data.current_health = *hp;
    save_data.player_hunger = hunger.current;
    save_data.inventory = inv.clone();
    save_data.craft_tracker = craft_tracker.clone();
    save_data.current_era = game.era.current_era.clone();
    save_data.visited_eras = game.era.visited_eras.clone();
    save_data.player_skills = skills.clone();
    save_data.player_skill_queue = skills_queue.clone();
    save_data.currency = (
        game.time_fragments.time_fragments,
        game.time_fragments.total_collected_time_fragments_this_run,
    );

    save_data.placed_objs = vec![game.world_obj_cache.objects.clone()];

    save_data.unique_objs = vec![game.world_obj_cache.unique_objs.clone()];

    // chain the current chests, and also the ones in registry,
    // since they will be despawned and missed by the query
    save_data.containers = container_reg
        .containers
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .chain(placed_objs.iter().filter_map(|(p, _, c, f)| {
            if let Some(chest) = c {
                return Some((
                    world_pos_to_tile_pos(p.translation().truncate()),
                    chest.items.clone(),
                ));
            } else if let Some(furnace) = f {
                return Some((
                    world_pos_to_tile_pos(p.translation().truncate()),
                    furnace.items.clone(),
                ));
            }
            None
        }))
        .collect();

    // override with resource if we save while player is interacting with a container
    if let Some(chest) = check_open_chest {
        let pos = placed_objs
            .get(chest.parent)
            .unwrap()
            .0
            .translation()
            .truncate();
        save_data
            .containers
            .insert(world_pos_to_tile_pos(pos), chest.items.clone());
    }
    // if let Some(furnace) = check_open_furnace {
    //     let pos = placed_objs
    //         .get(furnace.parent)
    //         .unwrap()
    //         .0
    //         .translation()
    //         .truncate();
    //     save_data
    //         .containers
    //         .insert(world_pos_to_tile_pos(pos), furnace.items.clone());
    // }
    save_data.container_reg = container_reg.containers.clone();
    save_data.night_tracker = night_tracker.clone();
    save_data.chaos_tracker = chaos_tracker
        .as_ref()
        .map(|tracker| (**tracker).clone())
        .unwrap_or_else(|| ChaosTracker::default());
    save_data.seed = seed.seed;
    save_data.analytics_data = analytics_data.clone();
    let async_task_pool = bevy::tasks::IoTaskPool::get();
    let data = save_data.clone();
    info!("STARTING ASYNC SAVE");
    let _ = async_task_pool
        .spawn(async move {
            let file = File::create(datafiles::save_file())
                .expect("Could not open file for serialization");

            // let json_Data: String = serde_json::to_string(&save_data).unwrap();
            if let Err(result) = serde_json::to_writer(file, &data) {
                error!("Failed to save game state: {result:?}");
            } else {
                info!("SAVED GAME STATE!");
            }
        })
        .detach();
}

pub fn load_state(
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    mut game_camera: Query<
        (&mut Transform, &mut RawPosition),
        (Without<MainCamera>, Without<UICamera>, With<TextureCamera>),
    >,
    mut era: ResMut<EraManager>,
) {
    let mut rng = rand::thread_rng();
    let mut seed = rng.gen_range(0..100000);
    let mut save_data_exists = false;
    // Load data if it exists
    // if let Ok(file_file) = File::open(datafiles::save_file()) {
    //     let reader = BufReader::new(file_file);

    //     // Read the JSON contents of the file as an instance of `User`.
    //     match serde_json::from_reader::<_, CurrentRunSaveData>(reader) {
    //         Ok(data) => {
    //             save_data_exists = true;
    //             let mut cache = WorldObjectCache::default();
    //             for (tp, _) in data.placed_objs[data.current_era.index()].iter() {
    //                 if !cache.generated_chunks.contains(&tp.chunk_pos) {
    //                     cache.generated_chunks.push(tp.chunk_pos);
    //                 }
    //             }
    //             cache.objects = data.placed_objs[data.current_era.index()].clone();
    //             cache.unique_objs = data.unique_objs[data.current_era.index()].clone();

    //             commands.insert_resource(cache);
    //             for (i, (objs, unique_objs)) in data
    //                 .placed_objs
    //                 .iter()
    //                 .zip(data.unique_objs.iter())
    //                 .enumerate()
    //             {
    //                 if data.current_era.index() == i {
    //                     continue;
    //                 }
    //                 era.era_generation_cache.insert(
    //                     Era::from_index(i),
    //                     WorldObjectCache {
    //                         objects: objs.clone(),
    //                         unique_objs: unique_objs.clone(),
    //                         ..Default::default()
    //                     },
    //                 );
    //             }
    //             era.current_era = data.current_era;
    //             era.visited_eras = data.visited_eras;
    //             seed = data.seed;
    //             commands.insert_resource(data.night_tracker);
    //             commands.insert_resource(data.chaos_tracker);
    //             commands.insert_resource(ContainerRegistry {
    //                 containers: data.containers,
    //             });
    //             commands.insert_resource(data.player_skill_queue);
    //             commands.insert_resource(data.analytics_data);
    //             commands.insert_resource(data.craft_tracker);
    //             proto_commands.apply(format!(
    //                 "Era{}WorldGenerationParams",
    //                 era.current_era.clone().index() + 1
    //             ));
    //             // PRE-MOVE CAMERAS TO PLAYER
    //             let (mut game_camera_transform, mut raw_camera_pos) = game_camera.single_mut();

    //             raw_camera_pos.0 = data.player_transform;
    //             game_camera_transform.translation.x = data.player_transform.x;
    //             game_camera_transform.translation.y = data.player_transform.y;
    //         }
    //         Err(err) => {
    //             let new_file = File::create(datafiles::save_file())
    //                 .expect("Could not create save data file for serialization");
    //             if let Err(result) = serde_json::to_writer(new_file, "") {
    //                 error!("Failed to save game data after death: {result:?}");
    //             } else {
    //                 info!("UPDATED SAVE DATA...");
    //             }
    //             println!("Failed to load data from file {err:?}")
    //         }
    //     }
    // }
    if !save_data_exists {
        proto_commands.apply("Era1WorldGenerationParams");
        commands.init_resource::<WorldObjectCache>();
    }
    commands.insert_resource(GenerationSeed { seed });

    // Load HighScores and Achievements early so UI (class selection) can use them
    let game_data_file_path = datafiles::game_data();
    if let Ok(file_file) = File::open(game_data_file_path) {
        let reader = BufReader::new(file_file);
        match serde_json::from_reader::<_, GameData>(reader) {
            Ok(game_data) => {
                commands.insert_resource(game_data.high_scores);
                // Also make sure class ranks are available early if present
                commands.insert_resource(game_data.class_ranks);
                commands.insert_resource(game_data.achievements);
            }
            Err(_) => {
                commands.insert_resource(HighScores::default());
                commands.insert_resource(Achievements::default());
            }
        }
    } else {
        commands.insert_resource(HighScores::default());
        commands.insert_resource(Achievements::default());
    }

    dim_event.send(DimensionSpawnEvent {
        swap_to_dim_now: true,
        new_era: None,
    });

    info!("DONE LOADING GAME DATA");
}

/// Load GameData when entering main menu so UI can access cumulative_analytics
pub fn load_game_data_for_ui(mut commands: Commands) {
    let game_data_file_path = datafiles::game_data();
    if let Ok(file_file) = File::open(game_data_file_path) {
        let reader = BufReader::new(file_file);
        match serde_json::from_reader::<_, GameData>(reader) {
            Ok(game_data) => {
                // Insert bounce tracker as a resource
                commands.insert_resource(game_data.bounce_tracker.clone());
                let seen_tips_set = game_data.seen_tips.clone();
                // Insert GameData as a resource so achievements UI can access cumulative_analytics
                commands.insert_resource(game_data);
                commands.insert_resource(SeenTips {
                    seen: seen_tips_set,
                });
            }
            Err(_) => {
                // Insert default GameData if file can't be read
                commands.insert_resource(GameData::default());
            }
        }
    } else {
        // Insert default GameData if file doesn't exist
        commands.insert_resource(GameData::default());
    }
}

/// Persists time fragments to game_data.json immediately when they are spent
/// This prevents save-scumming by closing and restarting the game
pub fn persist_time_fragments(time_fragments: i32) {
    let path = datafiles::game_data();
    let mut game_data = if let Ok(file) = File::open(&path) {
        let reader = BufReader::new(file);
        serde_json::from_reader::<_, GameData>(reader).unwrap_or_default()
    } else {
        GameData::default()
    };

    // Update time fragments (ensure it doesn't go negative)
    game_data.time_fragments = time_fragments.max(0) as u128;

    match File::create(&path) {
        Ok(file) => {
            let writer = BufWriter::new(file);
            if let Err(err) = serde_json::to_writer(writer, &game_data) {
                error!("Failed to persist time fragments to game_data.json: {err:?}");
            }
        }
        Err(err) => error!("Failed to create game_data.json while saving time fragments: {err:?}"),
    }
}

pub fn is_not_paused(state: Res<State<ClientState>>) -> bool {
    state.0 == ClientState::Unpaused
}

/// Calculate class experience based on run performance
fn calculate_class_experience(days_survived: u8, mobs_killed: u32) -> u32 {
    // Primary experience from mobs killed (10 per mob) //
    let mob_exp = mobs_killed * 2;

    // Secondary experience from days survived (50 per day)
    let days_exp = days_survived as u32 * 75;

    mob_exp + days_exp
}
