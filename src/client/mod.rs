use std::{fs::File, io::BufReader};

use bevy::{math::Vec3Swizzles, prelude::*, utils::HashMap};
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
use itertools::Itertools;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    assets::SpriteAnchor,
    attributes::{hunger::Hunger, CurrentHealth},
    container::{Container, ContainerRegistry},
    inventory::{Inventory, ItemStack},
    item::{
        projectile::Projectile, CraftingTracker, EquipmentType, Foliage, MainHand, Wall,
        WorldObject,
    },
    night::NightTracker,
    player::{
        levels::PlayerLevel,
        stats::{PlayerStats, SkillPoints},
        Player,
    },
    proto::proto_param::ProtoParam,
    ui::{ChestContainer, FurnaceContainer},
    vectorize::{vectorize, vectorize_inner},
    world::{
        chunk::{Chunk, ReflectedPos, TileEntityCollection, TileSpriteData},
        dimension::{
            ActiveDimension, Dimension, DimensionSpawnEvent, Era, EraManager, GenerationSeed,
        },
        dungeon::Dungeon,
        generation::WorldObjectCache,
        world_helpers::world_pos_to_tile_pos,
        TileMapPosition, WallTextureData, WorldGeneration,
    },
    CustomFlush, GameParam, GameState, MainCamera, RawPosition, TextureCamera, UICamera, YSort,
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
                timer: Timer::from_seconds(15., TimerMode::Repeating),
            })
            .add_system(load_state.in_schedule(OnExit(GameState::MainMenu)))
            .add_systems(
                (save_state, handle_append_run_data_after_death).in_set(OnUpdate(GameState::Main)),
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

    //Player Data
    pub inventory: Inventory,
    pub player_level: PlayerLevel,
    pub player_stats: PlayerStats,
    pub skill_points: SkillPoints,
    pub current_health: CurrentHealth,
    pub player_transform: Vec2,
    pub player_hunger: u8,

    // Era
    pub current_era: Era,
    pub visited_eras: Vec<Era>,
}

#[derive(Default)]
pub struct GameOverEvent;

#[derive(Resource, Default)]
pub struct SaveTimer {
    timer: Timer,
}

#[derive(Resource, Clone, Serialize, Deserialize, Default)]
pub struct GameData {
    pub num_runs: u128,
    pub longest_run: u8,
    pub seen_gear: Vec<ItemStack>,
}
pub fn handle_append_run_data_after_death(
    night: Res<NightTracker>,
    inv: Query<&Inventory>,
    proto_param: ProtoParam,
    game_over: EventReader<GameOverEvent>,
) {
    if game_over.is_empty() {
        return;
    }
    println!("GAME OVER! Storing run data in game_data.json...");
    let mut game_data: GameData = GameData::default();

    if let Ok(file_file) = File::open("game_data.json") {
        let reader = BufReader::new(file_file);

        // Read the JSON contents of the file as an instance of `User`.
        match serde_json::from_reader::<_, GameData>(reader) {
            Ok(data) => game_data = data,
            Err(err) => println!("Failed to load data from game_data.json file {err:?}"),
        }
    };
    game_data.num_runs += 1;
    if game_data.longest_run < night.days {
        game_data.longest_run = night.days;
    }
    let inv = inv.single();
    for item in inv.items.items.clone().iter().flatten() {
        if item.slot < 6 {
            //hotbar item
            if let Some(eqp_type) = item.get_obj().get_equip_type(&proto_param) {
                if eqp_type != EquipmentType::Axe && eqp_type != EquipmentType::Pickaxe {
                    game_data.seen_gear.push(item.item_stack.clone());
                }
            }
        }
    }

    for item in inv.equipment_items.items.iter().flatten() {
        game_data.seen_gear.push(item.item_stack.clone());
    }

    const PATH: &str = "game_data.json";

    let file = File::create(PATH).expect("Could not create game data file for serialization");

    // let json_Data: String = serde_json::to_string(&save_data).unwrap();
    if let Err(result) = serde_json::to_writer(file, &game_data.clone()) {
        println!("Failed to save game data after death: {result:?}");
    } else {
        println!("UPDATED GAME DATA...");
    }
}
pub fn save_state(
    mut timer: ResMut<SaveTimer>,
    time: Res<Time>,
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
    proto_param: ProtoParam,
    player_data: Query<
        (
            &GlobalTransform,
            &PlayerStats,
            &PlayerLevel,
            &CurrentHealth,
            &Hunger,
            &Inventory,
        ),
        With<Player>,
    >,
    container_reg: Res<ContainerRegistry>,
    craft_tracker: Res<CraftingTracker>,
    dungeon_check: Query<&Dungeon>,
    night_tracker: Res<NightTracker>,
    seed: Res<GenerationSeed>,
    check_open_chest: Option<Res<ChestContainer>>,
    check_open_furnace: Option<Res<FurnaceContainer>>,
    key_input: ResMut<Input<KeyCode>>,
    game: GameParam,
) {
    timer.timer.tick(time.delta());
    // only save if the timer is done and we are not in a dungeon
    if (!timer.timer.just_finished() || dungeon_check.get_single().is_ok())
        && !key_input.just_pressed(KeyCode::U)
    {
        return;
    }
    timer.timer.reset();
    //PlayerData
    let (player_txfm, stats, level, hp, hunger, inv) = player_data.single();
    save_data.player_transform = player_txfm.translation().xy();
    save_data.player_stats = stats.clone();
    save_data.player_level = level.clone();
    save_data.current_health = hp.clone();
    save_data.player_hunger = hunger.current;
    save_data.inventory = inv.clone();
    save_data.craft_tracker = craft_tracker.clone();
    save_data.current_era = game.era.current_era.clone();
    save_data.visited_eras = game.era.visited_eras.clone();

    save_data.placed_objs = game
        .era
        .era_generation_cache
        .iter()
        .map(|(_, c)| c.objects.clone())
        .collect();

    let curr_era_objs = placed_objs
        .iter()
        .map(|(p, w, _, _)| {
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(w.clone())
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            (
                world_pos_to_tile_pos(p.translation().truncate() - anchor.0),
                w.clone(),
            )
        })
        .map_into()
        .collect();
    if (save_data.placed_objs.len() as i32) - 1 < game.era.current_era.index() as i32 {
        // we are in the newest era and have not saved it in EraManager yet
        save_data.placed_objs.push(curr_era_objs);
    } else {
        save_data.placed_objs[game.era.current_era.index()] = curr_era_objs;
    }

    save_data.unique_objs = game
        .era
        .era_generation_cache
        .iter()
        .map(|(_e, c)| c.unique_objs.clone())
        .collect();
    let curr_era_unique_objs = game.world_obj_cache.unique_objs.clone();
    if (save_data.unique_objs.len() as i32) - 1 < game.era.current_era.index() as i32 {
        save_data.unique_objs.push(curr_era_unique_objs);
    } else {
        save_data.unique_objs[game.era.current_era.index()] = curr_era_unique_objs;
    }

    // chain the current chests, and also the ones in registry,
    // since they will be despawned and missed by the query
    save_data.containers = container_reg
        .containers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
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
    if let Some(furnace) = check_open_furnace {
        let pos = placed_objs
            .get(furnace.parent)
            .unwrap()
            .0
            .translation()
            .truncate();
        save_data
            .containers
            .insert(world_pos_to_tile_pos(pos), furnace.items.clone());
    }
    save_data.container_reg = container_reg.containers.clone();
    save_data.night_tracker = night_tracker.clone();
    save_data.seed = seed.seed;

    const PATH: &str = "save_state.json";

    let file = File::create(PATH).expect("Could not open file for serialization");

    // let json_Data: String = serde_json::to_string(&save_data).unwrap();
    if let Err(result) = serde_json::to_writer(file, &save_data.clone()) {
        println!("Failed to save game state: {result:?}");
    } else {
        println!("SAVED GAME STATE!");
    }
}

pub fn load_state(
    mut commands: Commands,
    mut proto_commands: ProtoCommands,
    mut game: GameParam,
    mut dim_event: EventWriter<DimensionSpawnEvent>,
    mut game_camera: Query<
        (&mut Transform, &mut RawPosition),
        (Without<MainCamera>, Without<UICamera>, With<TextureCamera>),
    >,
) {
    let mut rng = rand::thread_rng();
    let mut seed = rng.gen_range(0..100000);

    // Load data if it exists
    if let Ok(file_file) = File::open("save_state.json") {
        let reader = BufReader::new(file_file);

        // Read the JSON contents of the file as an instance of `User`.
        match serde_json::from_reader::<_, CurrentRunSaveData>(reader) {
            Ok(data) => {
                for (tp, _) in data.placed_objs[data.current_era.index()].iter() {
                    if !game.is_chunk_generated(tp.chunk_pos) {
                        game.set_chunk_generated(tp.chunk_pos);
                    }
                }
                game.world_obj_cache.objects = data.placed_objs[data.current_era.index()].clone();
                game.world_obj_cache.unique_objs =
                    data.unique_objs[data.current_era.index()].clone();
                for (i, (objs, unique_objs)) in data
                    .placed_objs
                    .iter()
                    .zip(data.unique_objs.iter())
                    .enumerate()
                {
                    if data.current_era.index() == i {
                        continue;
                    }
                    game.era.era_generation_cache.insert(
                        Era::from_index(i),
                        WorldObjectCache {
                            objects: objs.clone(),
                            unique_objs: unique_objs.clone(),
                            ..Default::default()
                        },
                    );
                }
                game.era.current_era = data.current_era;
                game.era.visited_eras = data.visited_eras;
                seed = data.seed;
                commands.insert_resource(data.night_tracker);
                commands.insert_resource(ContainerRegistry {
                    containers: data.containers,
                });
                commands.insert_resource(data.craft_tracker);
                proto_commands.apply(format!(
                    "Era{}WorldGenerationParams",
                    game.era.current_era.clone().index() + 1
                ));
                // PRE-MOVE CAMERAS TO PLAYER
                let (mut game_camera_transform, mut raw_camera_pos) = game_camera.single_mut();

                raw_camera_pos.0 = data.player_transform;
                game_camera_transform.translation.x = data.player_transform.x;
                game_camera_transform.translation.y = data.player_transform.y;
            }
            Err(err) => println!("Failed to load data from file {err:?}"),
        }
    }
    commands.insert_resource(GenerationSeed { seed });

    dim_event.send(DimensionSpawnEvent {
        swap_to_dim_now: true,
        new_era: None,
    });

    println!("DONE LOADING GAME DATA");
}
