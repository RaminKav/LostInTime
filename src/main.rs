use std::time::Duration;

//TODO:
// - get player movement
// - set up tilemap or world generation
// - trees/entities to break/mine
use bevy::{
    core_pipeline::bloom::BloomSettings,
    prelude::*,
    render::{
        camera::{RenderTarget, ScalingMode},
        render_resource::{FilterMode, SamplerDescriptor},
    },
    sprite::collide_aabb::{collide, Collision},
    sprite::MaterialMesh2dBundle,
    time::FixedTimestep,
    window::PresentMode,
};
mod animations;
mod assets;
mod gi;
mod inputs;
mod item;
mod world_generation;
use animations::AnimationsPlugin;
use assets::{GameAssetsPlugin, Graphics, WORLD_SCALE};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_inspector_egui::{RegisterInspectable, WorldInspectorPlugin};
use bevy_pkv::PkvStore;
use gi::{
    gi_component::{AmbientMask, GiAmbientLight},
    gi_post_processing::{setup_post_processing_camera, PostProcessingTarget},
    LightOccluder, LightSource,
};
use inputs::{Direction, InputsPlugin};
use item::{ItemsPlugin, WorldObject};
use world_generation::{ChunkManager, WorldGenerationPlugin, CHUNK_SIZE};

const PLAYER_MOVE_SPEED: f32 = 550.;
const PLAYER_DASH_SPEED: f32 = 1250.;
pub const TIME_STEP: f32 = 1.0 / 60.0;
const PLAYER_SIZE: f32 = 3.2 / WORLD_SCALE;
pub const HEIGHT: f32 = 900.;
pub const RESOLUTION: f32 = 16.0 / 9.0;
pub const WORLD_SIZE: usize = 300;
#[derive(Component)]
pub struct MainCamera;

pub const SCREEN_SIZE: (usize, usize) = ((HEIGHT * RESOLUTION) as usize, HEIGHT as usize);
fn main() {
    App::new()
        .init_resource::<Game>()
        .insert_resource(ClearColor(Color::rgb_u8(0, 0, 0)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Tell the asset server to watch for asset changes on disk:
                    watch_for_changes: true,
                    ..default()
                })
                .set(WindowPlugin {
                    window: WindowDescriptor {
                        width: SCREEN_SIZE.0 as f32,
                        height: SCREEN_SIZE.1 as f32,
                        title: "DST clone".to_string(),
                        present_mode: PresentMode::Fifo,
                        resizable: false,
                        ..Default::default()
                    },
                    ..default()
                })
                .set(ImagePlugin {
                    default_sampler: SamplerDescriptor {
                        mag_filter: FilterMode::Nearest,
                        min_filter: FilterMode::Nearest,
                        ..Default::default()
                    },
                }),
        )
        .insert_resource(PkvStore::new("Fleam", "SurvivalRogueLike"))
        .add_plugin(gi::GiComputePlugin)
        .add_plugin(WorldInspectorPlugin::new())
        .register_inspectable::<LightOccluder>()
        .register_inspectable::<LightSource>()
        .register_inspectable::<AmbientMask>()
        .register_inspectable::<GiAmbientLight>()
        .register_type::<BloomSettings>()
        .add_plugin(TilemapPlugin)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(AnimationsPlugin)
        .add_plugin(WorldGenerationPlugin)
        .add_plugin(InputsPlugin)
        .add_startup_system(setup.after(setup_post_processing_camera))
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::Main)
                .with_collection::<ImageAssets>(),
        )
        .add_state(GameState::Loading)
        .run();
}

#[derive(Resource, Default)]
pub struct Game {
    player: Player,
    world_size: usize,
    world_generation_params: WorldGeneration,
    player_dash_cooldown: Timer,
    player_dash_duration: Timer,
}

#[derive(Default)]
pub struct WorldGeneration {
    water_frequency: f64,
    sand_frequency: f64,
    dirt_frequency: f64,
    stone_frequency: f64,
    tree_frequency: f64,
}

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
pub enum GameState {
    Loading,
    Main,
}
#[derive(Clone, Eq, PartialEq, Debug, Hash, SystemLabel)]
pub enum GameSystems {
    Loading,
    Main,
}

#[derive(Resource, AssetCollection)]
pub struct ImageAssets {
    #[asset(path = "bevy_survival_sprites.png")]
    pub sprite_sheet: Handle<Image>,
    #[asset(path = "RPGTiles.png")]
    pub tiles_sheet: Handle<Image>,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Component, Default)]
struct Player {
    is_moving: bool,
    is_dashing: bool,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    post_processing_target: Res<PostProcessingTarget>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut game: ResMut<Game>,
) {
    game.world_size = WORLD_SIZE;
    game.world_generation_params = WorldGeneration {
        tree_frequency: 0.,
        stone_frequency: 0.0,
        dirt_frequency: 0.52,
        sand_frequency: 0.22,
        water_frequency: 0.05,
    };
    game.player_dash_cooldown = Timer::from_seconds(0.5, TimerMode::Once);
    game.player_dash_duration = Timer::from_seconds(0.15, TimerMode::Once);

    let player_texture_handle = asset_server.load("textures/gabe-idle-run.png");
    let player_texture_atlas = TextureAtlas::from_grid(
        player_texture_handle,
        Vec2::new(24.0, 24.0),
        7,
        1,
        None,
        None,
    );
    let player_texture_atlas_handle = texture_atlases.add(player_texture_atlas);

    commands
        .spawn((
            SpriteSheetBundle {
                texture_atlas: player_texture_atlas_handle,
                transform: Transform::from_scale(Vec3::splat(PLAYER_SIZE))
                    .with_translation(Vec3::new(0., 0., 1.)),
                ..default()
            },
            AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
            Player {
                is_moving: false,
                is_dashing: false,
            },
            Direction(1.0),
        ))
        .insert(Name::new("Player"));
    // let block_mesh = meshes.add(Mesh::from(shape::Circle::default()));

    // // Add roof.
    // commands
    //     .spawn(SpatialBundle {
    //         transform: Transform {
    //             translation: Vec3::new(0.0, 0.0, 0.0),
    //             ..Default::default()
    //         },
    //         ..Default::default()
    //     })
    //     .insert(Name::new("ambient_mask"))
    //     .insert(AmbientMask {
    //         h_size: Vec2::new(0., 0.),
    //     });
    commands.spawn((
        GiAmbientLight {
            color: Color::rgb_u8(93, 158, 179),
            intensity: 0.04,
        },
        Name::new("ambient_light"),
    ));
    // commands
    //     .spawn(MaterialMesh2dBundle {
    //         mesh: block_mesh.clone().into(),
    //         material: materials.add(ColorMaterial::from(Color::YELLOW)).into(),
    //         transform: Transform {
    //             scale: Vec3::splat(8.0),
    //             ..Default::default()
    //         },
    //         ..Default::default()
    //     })
    //     .insert(Name::new("cursor_light"))
    //     .insert(LightSource {
    //         intensity: 10.0,
    //         radius: 3.0,
    //         color: Color::rgb_u8(219, 104, 72),
    //         falloff: Vec3::new(6.0, 3.0, 0.05),
    //         ..default()
    //     });
    let render_target = post_processing_target
        .handle
        .clone()
        .expect("No post processing target");
    let mut camera = Camera2dBundle {
        camera: Camera {
            hdr: true,
            priority: 0,
            target: RenderTarget::Image(render_target),
            ..Default::default()
        },
        ..Default::default()
    };

    // One unit in world space is one tile
    camera.projection.left = -HEIGHT / WORLD_SCALE / 2.0 * RESOLUTION;
    camera.projection.right = HEIGHT / WORLD_SCALE / 2.0 * RESOLUTION;
    camera.projection.top = HEIGHT / WORLD_SCALE / 2.0;
    camera.projection.bottom = -HEIGHT / WORLD_SCALE / 2.0;
    camera.projection.scale = 0.25;
    // camera.projection.scaling_mode = ScalingMode::None;
    commands
        .spawn(camera)
        .insert(MainCamera)
        .insert(Name::new("MainCamera"))
        .insert(UiCameraConfig {
            show_ui: false,
            ..default()
        });
}
