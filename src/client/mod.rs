use std::{fs::File, io::BufReader};

use bevy::{
    app::AppExit, ecs::system::SystemState, math::Vec3Swizzles, prelude::*, utils::HashMap,
};
use bevy_ecs_tilemap::{
    prelude::{
        TilemapGridSize, TilemapId, TilemapSize, TilemapSpacing, TilemapTexture, TilemapTileSize,
        TilemapType,
    },
    tiles::{TileColor, TileFlip, TilePos, TilePosOld, TileStorage, TileTextureIndex, TileVisible},
    FrustumCulling,
};
use bevy_save::prelude::*;
use itertools::Itertools;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::{
    assets::SpriteAnchor,
    attributes::{hunger::Hunger, CurrentHealth},
    container::{Container, ContainerRegistry},
    inventory::{Inventory, ItemStack},
    item::{projectile::Projectile, Foliage, MainHand, Wall, WorldObject},
    night::NightTracker,
    player::{
        levels::PlayerLevel,
        stats::{PlayerStats, SkillPoints},
        Player,
    },
    proto::proto_param::ProtoParam,
    ui::{minimap::Minimap, ChestContainer, FurnaceContainer},
    vectorize::vectorize,
    world::{
        chunk::{
            Chunk, CreateChunkEvent, DespawnChunkEvent, ReflectedPos, SpawnChunkEvent,
            TileEntityCollection, TileSpriteData,
        },
        dimension::{ActiveDimension, ChunkCache, Dimension, DimensionSpawnEvent, GenerationSeed},
        dungeon::Dungeon,
        world_helpers::world_pos_to_tile_pos,
        ChunkManager, TileMapPosition, WallTextureData, WorldGeneration,
    },
    CustomFlush, GameParam, GameState, GameUpscale, MainCamera, RawPosition, TextureCamera,
    UICamera, YSort,
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
        app.add_plugins(SavePlugins)
            .register_saveable::<GenerationSeed>()
            .register_saveable::<Dimension>()
            .register_saveable::<ActiveDimension>()
            .register_saveable::<ChunkManager>()
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
            .insert_resource(SaveData::default())
            .insert_resource(SaveTimer {
                timer: Timer::from_seconds(15., TimerMode::Repeating),
            })
            .add_system(
                load_state
                    .run_if(run_once())
                    .in_schedule(OnExit(GameState::MainMenu)),
            )
            .add_system(save_state.in_set(OnUpdate(GameState::Main)))
            .add_system(apply_system_buffers.in_set(CustomFlush));
    }
}

#[derive(Resource, Clone, Serialize, Deserialize, Default)]
pub struct SaveData {
    seed: u64,
    #[serde(with = "vectorize")]
    placed_objs: HashMap<TileMapPosition, WorldObject>,
    #[serde(with = "vectorize")]
    containers: HashMap<TileMapPosition, Container>,
    #[serde(with = "vectorize")]
    craft_reg: HashMap<TileMapPosition, Container>,
    night_tracker: NightTracker,

    //Player Data
    pub inventory: Inventory,
    pub player_level: PlayerLevel,
    pub player_stats: PlayerStats,
    pub skill_points: SkillPoints,
    pub current_health: CurrentHealth,
    pub player_transform: Vec2,
    pub player_hunger: u8,
}

#[derive(Resource, Default)]
pub struct SaveTimer {
    timer: Timer,
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
    mut save_data: ResMut<SaveData>,
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
    craft_reg: Res<ContainerRegistry>,
    dungeon_check: Query<&Dungeon>,
    night_tracker: Res<NightTracker>,
) {
    timer.timer.tick(time.delta());
    // only save if the timer is done and we are not in a dungeon
    if !timer.timer.just_finished() || dungeon_check.get_single().is_ok() {
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

    save_data.placed_objs = placed_objs
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
    save_data.containers = placed_objs
        .iter()
        .filter_map(|(p, _, c, f)| {
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
        })
        .collect();
    save_data.craft_reg = craft_reg.containers.clone();
    save_data.night_tracker = night_tracker.clone();

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
        match serde_json::from_reader::<_, SaveData>(reader) {
            Ok(data) => {
                for (tp, _) in data.placed_objs.iter() {
                    if !game.is_chunk_generated(tp.chunk_pos) {
                        game.set_chunk_generated(tp.chunk_pos);
                    }
                }
                game.world_obj_cache.objects = data.placed_objs;
                seed = data.seed;
                commands.insert_resource(data.night_tracker);
                commands.insert_resource(ContainerRegistry {
                    containers: data.craft_reg,
                });

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
    let params = WorldGeneration {
        sand_frequency: 0.32,
        water_frequency: 0.15,
        obj_allowed_tiles_map: HashMap::default(),
        ..default()
    };
    dim_event.send(DimensionSpawnEvent {
        generation_params: game.world_generation_params.clone(),
        swap_to_dim_now: true,
    });

    println!("DONE LOADING GAME DATA");
}
