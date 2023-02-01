use std::{marker::PhantomData, time::Duration};

use attributes::Health;
//TODO:
// - get player movement
// - set up tilemap or world generation
// - trees/entities to break/mine
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    render::{
        camera::{RenderTarget, ScalingMode},
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        view::RenderLayers,
    },
    sprite::MaterialMesh2dBundle,
    utils::HashSet,
    window::PresentMode,
};
use bevy_inspector_egui::WorldInspectorPlugin;
use bevy_pixel_camera::{PixelCameraBundle, PixelCameraPlugin};
use bevy_rapier2d::prelude::*;
mod animations;
mod assets;
mod attributes;
mod inputs;
mod item;
mod vectorize;
mod world_generation;
use animations::{
    AnimatedTextureMaterial, AnimationFrameTracker, AnimationPosTracker, AnimationTimer,
    AnimationsPlugin,
};
use assets::{GameAssetsPlugin, Graphics, WORLD_SCALE};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_pkv::PkvStore;
use bevy_tweening::{
    lens::{TransformPositionLens, TransformScaleLens},
    Animator, AnimatorState, EaseFunction, Tween, TweeningPlugin,
};
use inputs::{InputsPlugin, MovementVector};
use item::{
    Block, Equipment, EquipmentMetaData, ItemStack, ItemsPlugin, WorldObject, WorldObjectResource,
};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use world_generation::{ChunkManager, GameData, WorldGenerationPlugin};

const PLAYER_MOVE_SPEED: f32 = 2.;
const PLAYER_DASH_SPEED: f32 = 125.;
pub const TIME_STEP: f32 = 1.0 / 60.0;
pub const HEIGHT: f32 = 1920.;
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;
pub const WORLD_SIZE: usize = 300;

fn main() {
    App::new()
        .init_resource::<Game>()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    window: WindowDescriptor {
                        width: HEIGHT * ASPECT_RATIO,
                        height: HEIGHT,
                        scale_factor_override: Some(1.0),
                        // mode: WindowMode::BorderlessFullscreen,
                        title: "Survival Game".to_string(),
                        present_mode: PresentMode::Fifo,
                        resizable: false,
                        ..Default::default()
                    },
                    ..default()
                }),
        )
        .insert_resource(Msaa { samples: 1 })
        .insert_resource(PkvStore::new("Fleam", "SurvivalRogueLike"))
        // .add_plugin(PixelCameraPlugin)
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugin(WorldInspectorPlugin::new())
        // .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(TilemapPlugin)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(AnimationsPlugin)
        .add_plugin(WorldGenerationPlugin)
        .add_plugin(InputsPlugin)
        .add_plugin(TweeningPlugin)
        .add_startup_system(setup)
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::Main)
                .with_collection::<ImageAssets>(),
        )
        .add_state(GameState::Loading)
        .add_system(y_sort)
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

#[derive(Component)]
pub struct YSort;

fn y_sort(mut q: Query<&mut Transform, With<YSort>>) {
    for mut tf in q.iter_mut() {
        tf.translation.z = 1. - 1.0f32 / (1.0f32 + (2.0f32.powf(-0.01 * tf.translation.y)));
    }
}

#[derive(SystemParam)]
pub struct GameParam<'w, 's> {
    pub game: ResMut<'w, Game>,
    pub graphics: Res<'w, Graphics>,
    pub chunk_manager: ResMut<'w, ChunkManager>,
    pub world_obj_data: ResMut<'w, WorldObjectResource>,
    pub game_data: ResMut<'w, GameData>,
    pub meshes: ResMut<'w, Assets<Mesh>>,

    pub block_query: Query<'w, 's, (Entity, &'static mut Health), With<Block>>,
    pub player_query: Query<'w, 's, (Entity, &'static mut Player)>,
    pub items_query: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static ItemStack),
        (Without<Player>, Without<Equipment>),
    >,
    pub equipment: Query<'w, 's, (Entity, &'static Equipment)>,
    pub camera_query: Query<
        'w,
        's,
        &'static mut Transform,
        (With<MainCamera>, Without<Player>, Without<ItemStack>),
    >,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}
#[derive(Component, Default)]
pub struct Player {
    is_moving: bool,
    is_dashing: bool,
    is_attacking: bool,
    inventory: Vec<ItemStack>,
    main_hand_slot: Option<EquipmentMetaData>,
}
#[derive(Component, EnumIter, Display, PartialEq)]
pub enum Limb {
    Torso,
    Hands,
    Legs,
    Head,
}

#[derive(Component, Default)]
pub struct CameraDirty(bool, bool);
#[derive(Component, Default)]
pub struct MainCamera;
#[derive(Component, Default)]
pub struct TextureCamera;
#[derive(Component, Default)]
pub struct TextureTarget;
#[derive(Component, Default)]
pub struct RawPosition(f32, f32);

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
    mut render_materials: ResMut<Assets<ColorMaterial>>,

    mut game: ResMut<Game>,
    mut images: ResMut<Assets<Image>>,
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
    game.player_dash_duration = Timer::from_seconds(0.05, TimerMode::Once);

    let img_size = Extent3d {
        width: 320,
        height: 180,
        ..default()
    };
    let game_size = Vec2::new((HEIGHT * ASPECT_RATIO) as f32, (HEIGHT) as f32);

    // This is the texture that will be rendered to.
    let mut image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size: img_size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
        },
        ..default()
    };

    // fill image.data with zeroes
    image.resize(img_size);

    let image_handle = images.add(image);

    // This specifies the layer used for the first pass, which will be attached to the first pass camera and cube.
    let first_pass_layer = RenderLayers::layer(1);

    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                // render before the "main pass" camera
                priority: -1,
                target: RenderTarget::Image(image_handle.clone()),
                ..default()
            },
            ..default()
        },
        TextureCamera,
        RawPosition(0., 0.),
    ));

    // This material has the texture that has been rendered.
    let render_material_handle = render_materials.add(ColorMaterial::from(image_handle));

    // Main pass cube, with material containing the rendered first pass texture.
    let texture_image = commands
        .spawn((
            MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(game_size.x as f32, game_size.y as f32),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform::from_scale(Vec3::new(1., 1., 1.)),
                material: render_material_handle,
                ..default()
            },
            TextureTarget,
            first_pass_layer,
        ))
        .id();

    // The main pass camera.
    commands.spawn((Camera2dBundle::default(), MainCamera, first_pass_layer));
    // .push_children(&vec![texture_image]);

    // let camera = PixelCameraBundle::from_resolution(240, 180);

    // commands.spawn((
    //     camera,
    //     CameraDirty(false, false),
    //     AnimationTimer(Timer::from_seconds(4., TimerMode::Once)),
    // ));

    let mut limb_children: Vec<Entity> = vec![];
    //player shadow
    let shadow_texture_handle = asset_server.load("textures/player/player-shadow.png");
    let shadow_texture_atlas =
        TextureAtlas::from_grid(shadow_texture_handle, Vec2::new(32., 32.), 1, 1, None, None);
    let shadow_texture_atlas_handle = texture_atlases.add(shadow_texture_atlas);

    let shadow = commands
        .spawn(SpriteSheetBundle {
            texture_atlas: shadow_texture_atlas_handle,
            transform: Transform::from_translation(Vec3::new(0., 0., -0.00000001)),
            ..default()
        })
        .id();
    limb_children.push(shadow);

    //player
    for l in Limb::iter() {
        let limb_source_handle = asset_server.load(format!(
            "textures/player/player-run-down/player-{}-run-down-source-1.png",
            l.to_string().to_lowercase()
        ));
        let limb_texture_handle = asset_server.load(format!(
            "textures/player/player-texture-{}.png",
            l.to_string().to_lowercase()
        ));
        // let limb_texture_atlas =
        //     TextureAtlas::from_grid(limb_texture_handle, Vec2::new(32., 32.), 5, 1, None, None);

        // let limb_texture_atlas_handle = texture_atlases.add(limb_texture_atlas);
        let transform = if l == Limb::Head {
            Transform::from_translation(Vec3::new(0., 0., 0.))
        } else {
            Transform::default()
        };
        let limb = commands
            .spawn((
                MaterialMesh2dBundle {
                    mesh: meshes
                        .add(
                            shape::Quad {
                                size: Vec2::new(32., 32.),
                                ..Default::default()
                            }
                            .into(),
                        )
                        .into(),
                    transform,
                    material: materials.add(AnimatedTextureMaterial {
                        source_texture: Some(limb_source_handle),
                        lookup_texture: Some(limb_texture_handle),
                        flip: 1.,
                    }),
                    ..default()
                },
                l,
                AnimationFrameTracker(0, 5),
            ))
            .id();
        // .spawn(SpriteSheetBundle {
        //     texture_atlas: limb_texture_atlas_handle,
        //     transform,
        //     ..default()
        // })
        // .id();
        limb_children.push(limb);
    }

    //spawn player entity with limb spritesheets as children
    commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            },
            AnimationTimer(Timer::from_seconds(0.25, TimerMode::Repeating)),
            Player {
                is_moving: false,
                is_dashing: false,
                is_attacking: false,
                inventory: Vec::new(),
                main_hand_slot: None,
            },
            MovementVector(0., 0.),
            KinematicCharacterController::default(),
            Collider::cuboid(7., 10.),
            YSort,
            Name::new("Player"),
            RawPosition(0., 0.),
        ))
        .push_children(&limb_children);
}
