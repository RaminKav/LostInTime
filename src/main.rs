use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use ai::AIPlugin;
use attributes::{AttributesPlugin, Health, InvincibilityCooldown};
//TODO:
// - get player movement
// - set up tilemap or world generation
// - trees/entities to break/mine
use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    ecs::system::SystemParam,
    prelude::*,
    reflect::TypeUuid,
    render::{
        camera::RenderTarget,
        render_resource::{
            AsBindGroup, Extent3d, ShaderRef, TextureDescriptor, TextureDimension, TextureFormat,
            TextureUsages,
        },
        view::RenderLayers,
    },
    sprite::{Material2d, Material2dPlugin, MaterialMesh2dBundle},
    window::{CompositeAlphaMode, PresentMode},
};
use bevy_inspector_egui::WorldInspectorPlugin;

use bevy_rapier2d::prelude::*;
mod ai;
mod animations;
mod assets;
mod attributes;
mod combat;
mod inputs;
mod inventory;
mod item;
mod ui;
mod vectorize;
mod world_generation;
use animations::{
    AnimatedTextureMaterial, AnimationFrameTracker, AnimationTimer, AnimationsPlugin,
};
use assets::{GameAssetsPlugin, Graphics};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_pkv::PkvStore;
use combat::CombatPlugin;
use inputs::{FacingDirection, InputsPlugin, MovementVector};
use inventory::{InventoryItemStack, InventoryPlugin, ItemStack, INVENTORY_SIZE};
use item::{Block, Equipment, EquipmentMetaData, ItemsPlugin, WorldObjectResource};
use serde::Deserialize;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use ui::{InventorySlotState, UIPlugin};
use world_generation::{ChunkManager, GameData, WorldGenerationPlugin};

const PLAYER_MOVE_SPEED: f32 = 2.;
const PLAYER_DASH_SPEED: f32 = 125.;
pub const TIME_STEP: f32 = 1.0 / 60.0;
pub const HEIGHT: f32 = 1600.;
pub const WIDTH: f32 = HEIGHT * ASPECT_RATIO;
pub const GAME_HEIGHT: f32 = 180.;
pub const GAME_WIDTH: f32 = 320.;
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;

fn main() {
    App::new()
        .init_resource::<Game>()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    window: WindowDescriptor {
                        width: WIDTH,
                        height: HEIGHT,
                        scale_factor_override: Some(1.0),
                        // mode: WindowMode::BorderlessFullscreen,
                        title: "Survival Game".to_string(),
                        present_mode: PresentMode::Fifo,
                        resizable: false,
                        transparent: true,
                        alpha_mode: CompositeAlphaMode::PostMultiplied,
                        ..Default::default()
                    },
                    ..default()
                }),
            // .set(LogPlugin {
            //     level: Level::TRACE,
            //     filter: "kayak_ui::context=trace".to_string(),
            //     ..Default::default()
            // }),
        )
        .insert_resource(Msaa { samples: 1 })
        .insert_resource(PkvStore::new("Fleam", "SurvivalRogueLike"))
        // .add_plugin(PixelCameraPlugin)
        .add_plugin(Material2dPlugin::<UITextureMaterial>::default())
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugin(WorldInspectorPlugin::new())
        // .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(TilemapPlugin)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(AnimationsPlugin)
        .add_plugin(WorldGenerationPlugin)
        .add_plugin(InputsPlugin)
        .add_plugin(InventoryPlugin)
        .add_plugin(UIPlugin)
        .add_plugin(AIPlugin)
        .add_plugin(AttributesPlugin)
        .add_plugin(CombatPlugin)
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
    player_state: PlayerState,
    world_generation_params: WorldGeneration,
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
    pub inv_slot_query: Query<'w, 's, &'static mut InventorySlotState>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}
#[derive(Component, Debug)]
pub struct Player;
#[derive(Debug)]
pub struct PlayerState {
    is_moving: bool,
    is_dashing: bool,
    is_attacking: bool,
    inventory: [Option<InventoryItemStack>; INVENTORY_SIZE],
    main_hand_slot: Option<EquipmentMetaData>,
    position: Vec3,
    reach_distance: u8,
    player_dash_cooldown: Timer,
    player_dash_duration: Timer,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            is_moving: false,
            is_dashing: false,
            is_attacking: false,
            inventory: [None; INVENTORY_SIZE],
            main_hand_slot: None,
            position: Vec3::ZERO,
            reach_distance: 2,
            player_dash_cooldown: Timer::from_seconds(0.5, TimerMode::Once),
            player_dash_duration: Timer::from_seconds(0.05, TimerMode::Once),
        }
    }
}
#[derive(Component, EnumIter, Display, Debug, Hash, Copy, Clone, PartialEq, Eq, Deserialize)]
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
pub struct UICamera;
#[derive(Component, Default)]
pub struct TextureTarget;
#[derive(Component, Default)]
pub struct RawPosition(Vec2);

#[derive(Component)]
pub struct GameUpscale(pub f32);

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
impl Material2d for UITextureMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/ui_texture.wgsl".into()
    }
}

#[derive(AsBindGroup, TypeUuid, Debug, Clone)]
#[uuid = "9600f1e5-1911-4286-9810-e9bd9ff685e2"]
pub struct UITextureMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub source_texture: Option<Handle<Image>>,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<AnimatedTextureMaterial>>,
    mut game_render_materials: ResMut<Assets<ColorMaterial>>,
    mut ui_render_materials: ResMut<Assets<UITextureMaterial>>,

    mut game: ResMut<Game>,
    mut images: ResMut<Assets<Image>>,
) {
    game.world_generation_params = WorldGeneration {
        tree_frequency: 0.,
        stone_frequency: 0.0,
        dirt_frequency: 0.52,
        sand_frequency: 0.22,
        water_frequency: 0.05,
    };
    game.player_state.player_dash_cooldown = Timer::from_seconds(0.5, TimerMode::Once);
    game.player_state.player_dash_duration = Timer::from_seconds(0.05, TimerMode::Once);

    let img_size = Extent3d {
        width: GAME_WIDTH as u32,
        height: GAME_HEIGHT as u32,
        ..default()
    };
    let game_size = Vec2::new(HEIGHT * ASPECT_RATIO, HEIGHT);

    // This is the texture that will be rendered to.
    let mut game_image = Image {
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
    let mut ui_image = Image {
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
    game_image.resize(img_size);
    ui_image.resize(img_size);

    let game_image_handle = images.add(game_image);
    let ui_image_handle = images.add(ui_image);

    // This specifies the layer used for the first pass, which will be attached to the first pass camera and cube.
    let first_pass_layer = RenderLayers::layer(1);
    let second_pass_layer = RenderLayers::layer(2);

    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                // render before the "main pass" camera
                priority: -2,
                target: RenderTarget::Image(game_image_handle.clone()),
                ..default()
            },
            ..default()
        },
        TextureCamera,
        RawPosition::default(),
    ));
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                // render before the "main pass" camera
                priority: -1,
                target: RenderTarget::Image(ui_image_handle.clone()),
                ..default()
            },
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::Custom(Color::rgba(0., 0., 0., 0.)),
            },
            ..default()
        },
        RenderLayers::from_layers(&[3]),
    ));

    // This material has the texture that has been rendered.
    let game_render_material_handle =
        game_render_materials.add(ColorMaterial::from(game_image_handle));
    let ui_render_material_handle = ui_render_materials.add(UITextureMaterial {
        source_texture: Some(ui_image_handle),
    });

    // Main pass cube, with material containing the rendered first pass texture.
    let _game_texture_image = commands
        .spawn((
            MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(game_size.x, game_size.y),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform::from_scale(Vec3::new(1., 1., 1.)),
                material: game_render_material_handle,
                ..default()
            },
            TextureTarget,
            first_pass_layer,
        ))
        .id();
    let _ui_texture_image = commands
        .spawn((
            MaterialMesh2dBundle {
                mesh: meshes
                    .add(
                        shape::Quad {
                            size: Vec2::new(game_size.x, game_size.y),
                            ..Default::default()
                        }
                        .into(),
                    )
                    .into(),
                transform: Transform {
                    translation: Vec3::new(0., 0., 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..default()
                },
                material: ui_render_material_handle,
                ..default()
            },
            // TextureTarget,
            second_pass_layer,
        ))
        .id();

    // The main pass camera.
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                priority: 0,
                ..default()
            },
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::None,
            },
            ..default()
        },
        MainCamera,
        GameUpscale(HEIGHT / img_size.height as f32),
        first_pass_layer,
    ));
    commands.spawn((
        Camera2dBundle {
            camera: Camera {
                priority: 1,
                ..default()
            },
            camera_2d: Camera2d {
                clear_color: ClearColorConfig::None,
            },
            ..default()
        },
        UICamera,
        GameUpscale(HEIGHT / img_size.height as f32),
        second_pass_layer,
    ));

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
            "textures/player/player-run-down/player-{}-run-down-source-0.png",
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
                        opacity: 1.,
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
            Player,
            Health(100),
            InvincibilityCooldown(0.3),
            MovementVector::default(),
            FacingDirection::default(),
            KinematicCharacterController::default(),
            Collider::cuboid(7., 10.),
            YSort,
            Name::new("Player"),
            RawPosition::default(),
        ))
        .push_children(&limb_children);
}
