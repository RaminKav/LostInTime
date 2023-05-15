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
    diagnostic::FrameTimeDiagnosticsPlugin,
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

use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::prelude::*;
mod ai;
mod animations;
mod assets;
mod attributes;
mod combat;
mod enemy;
mod inputs;
mod inventory;
mod item;
mod player;
mod ui;
mod vectorize;
mod world;
use animations::{
    AnimatedTextureMaterial, AnimationFrameTracker, AnimationTimer, AnimationsPlugin,
};
use assets::{GameAssetsPlugin, Graphics};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use bevy_pkv::PkvStore;
use combat::CombatPlugin;
use enemy::{spawner::ChunkSpawners, EnemyPlugin};
use inputs::{FacingDirection, InputsPlugin, MovementVector};
use inventory::{Inventory, InventoryPlugin, ItemStack, INVENTORY_INIT, INVENTORY_SIZE};
use item::{Equipment, EquipmentData, ItemsPlugin, LootTableMap, WorldObjectResource};
use player::PlayerPlugin;
use serde::Deserialize;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter};
use ui::{FPSText, InventorySlotState, UIPlugin};
use world::{
    chunk::{Chunk, SpawnedObject, TileEntityCollection, TileSpriteData},
    dimension::DimensionSpawnEvent,
    TileMapPositionData, WorldObjectEntityData, WorldPlugin,
};
use world::{generation::GameData, ChunkManager, WorldGeneration};

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
        .add_state::<GameState>()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (WIDTH, HEIGHT).into(),
                        // width: WIDTH,
                        // height: HEIGHT,
                        // scale_factor_override: Some(1.0),
                        // mode: WindowMode::BorderlessFullscreen,
                        title: "Survival Game".to_string(),
                        present_mode: PresentMode::Fifo,
                        resizable: false,
                        transparent: true,
                        // alpha_mode: CompositeAlphaMode::PostMultiplied,
                        ..Default::default()
                    }),
                    ..default()
                }),
            // .set(LogPlugin {
            //     level: Level::TRACE,
            //     filter: "kayak_ui::context=trace".to_string(),
            //     ..Default::default()
            // }),
        )
        .insert_resource(Msaa::Off)
        .insert_resource(FixedTime::new_from_secs(TIME_STEP))
        // .insert_resource(PkvStore::new("Fleam", "SurvivalRogueLike"))
        // .add_plugin(PixelCameraPlugin)
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(Material2dPlugin::<UITextureMaterial>::default())
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugin(WorldInspectorPlugin::new())
        // .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(TilemapPlugin)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(AnimationsPlugin)
        .add_plugin(InputsPlugin)
        .add_plugin(InventoryPlugin)
        .add_plugin(UIPlugin)
        .add_plugin(AIPlugin)
        .add_plugin(AttributesPlugin)
        .add_plugin(CombatPlugin)
        .add_plugin(EnemyPlugin)
        .add_plugin(PlayerPlugin)
        .add_plugin(WorldPlugin)
        .add_startup_system(setup)
        .add_loading_state(LoadingState::new(GameState::Loading).continue_to_state(GameState::Main))
        .add_collection_to_loading_state::<_, ImageAssets>(GameState::Loading)
        .add_system(y_sort)
        .run();
}

#[derive(Resource)]
pub struct Game {
    player_state: PlayerState,
    player: Entity,
}
impl Default for Game {
    fn default() -> Self {
        Self {
            player_state: PlayerState::default(),
            player: Entity::from_raw(0),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug, Hash, Default, States)]
pub enum GameState {
    #[default]
    Loading,
    Main,
}
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
struct CustomFlush;

#[derive(Resource, AssetCollection)]
pub struct ImageAssets {
    #[asset(path = "bevy_survival_sprites.png")]
    pub sprite_sheet: Handle<Image>,
    #[asset(path = "RPGTiles.png")]
    pub tiles_sheet: Handle<Image>,
    #[asset(path = "WallTextures.png")]
    pub walls_sheet: Handle<Image>,
}

#[derive(Component)]
pub struct YSort;

fn y_sort(mut q: Query<&mut Transform, With<YSort>>) {
    for mut tf in q.iter_mut() {
        // tf.translation.z = 1. - 1.0f32 / (1.0f32 + (2.0f32.powf(-0.01 * tf.translation.y)));
        tf.translation.z = 900. - 900.0f32 / (1.0f32 + (2.0f32.powf(-0.00001 * tf.translation.y)));
    }
}
#[derive(SystemParam)]
pub struct GameParam<'w, 's> {
    pub game: ResMut<'w, Game>,
    pub graphics: Res<'w, Graphics>,
    pub chunk_manager: ResMut<'w, ChunkManager>,
    pub loot_tables: Res<'w, LootTableMap>,
    pub world_obj_data: ResMut<'w, WorldObjectResource>,
    //TODO: remove this to use Bevy_Save
    pub game_data: ResMut<'w, GameData>,
    pub meshes: ResMut<'w, Assets<Mesh>>,

    pub player_query: Query<'w, 's, (Entity, &'static mut Player)>,
    pub chunk_query:
        Query<'w, 's, (Entity, &'static Transform, &'static mut ChunkSpawners), With<Chunk>>,
    pub tile_collection_query: Query<'w, 's, &'static TileEntityCollection, With<Chunk>>,
    pub tile_data_query:
        Query<'w, 's, (&'static mut TileSpriteData, Option<&'static SpawnedObject>)>,
    pub world_obj_data_query: Query<'w, 's, &'static mut WorldObjectEntityData>,

    pub items_query: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static ItemStack),
        (
            Without<Player>,
            Without<Equipment>,
            Without<Chunk>,
            Without<Health>,
        ),
    >,
    pub equipment: Query<'w, 's, (Entity, &'static Equipment)>,
    pub camera_query: Query<
        'w,
        's,
        &'static mut Transform,
        (
            With<TextureCamera>,
            Without<Player>,
            Without<Chunk>,
            Without<ItemStack>,
            Without<Health>,
        ),
    >,
    pub inv_slot_query: Query<'w, 's, &'static mut InventorySlotState>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}

impl<'w, 's> GameParam<'w, 's> {
    pub fn get_chunk_entity(&self, chunk_pos: IVec2) -> Option<&Entity> {
        self.chunk_manager.chunks.get(&chunk_pos)
    }
    pub fn set_chunk_entity(&mut self, chunk_pos: IVec2, e: Entity) {
        self.chunk_manager.chunks.insert(chunk_pos, e);
    }
    pub fn remove_chunk_entity(&mut self, chunk_pos: IVec2) {
        self.chunk_manager.chunks.remove(&chunk_pos);
    }

    pub fn get_tile_entity(&self, tile: TileMapPositionData) -> Option<Entity> {
        if let Some(chunk_e) = self.get_chunk_entity(tile.chunk_pos) {
            let tile_collection = self.tile_collection_query.get(*chunk_e).unwrap();
            return tile_collection.map.get(&tile.tile_pos).copied();
        }
        None
    }
    pub fn get_tile_data_mut(&mut self, tile: TileMapPositionData) -> Option<Mut<TileSpriteData>> {
        if let Some(tile_e) = self.get_tile_entity(tile) {
            return Some(self.tile_data_query.get_mut(tile_e).unwrap().0);
        }
        None
    }
    pub fn get_tile_data(&self, tile: TileMapPositionData) -> Option<TileSpriteData> {
        if let Some(tile_e) = self.get_tile_entity(tile) {
            return Some(self.tile_data_query.get(tile_e).unwrap().0.clone());
        }
        None
    }
    pub fn get_obj_entity_at_tile(&self, tile: TileMapPositionData) -> Option<Entity> {
        if let Some(tile_e) = self.get_tile_entity(tile) {
            if let Some(e) = self.tile_data_query.get(tile_e).unwrap().1 {
                return Some(e.0);
            }
        }
        None
    }
    pub fn get_tile_obj_data(&self, tile: TileMapPositionData) -> Option<WorldObjectEntityData> {
        if let Some(e) = self.get_obj_entity_at_tile(tile) {
            return Some(self.world_obj_data_query.get(e).unwrap().clone());
        }
        None
    }
    pub fn get_tile_obj_data_mut(
        &mut self,
        tile: TileMapPositionData,
    ) -> Option<Mut<WorldObjectEntityData>> {
        if let Some(e) = self.get_obj_entity_at_tile(tile) {
            return Some(self.world_obj_data_query.get_mut(e).unwrap());
        }
        None
    }
}
#[derive(Component, Debug)]
pub struct Player;
#[derive(Debug, Clone)]
pub struct PlayerState {
    is_moving: bool,
    is_dashing: bool,
    is_attacking: bool,
    main_hand_slot: Option<EquipmentData>,
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
            main_hand_slot: None,
            position: Vec3::ZERO,
            reach_distance: 1,
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
    mut dim_event: EventWriter<DimensionSpawnEvent>,

    mut game: ResMut<Game>,
    mut images: ResMut<Assets<Image>>,
) {
    let mut cm = ChunkManager::new();
    cm.world_generation_params = WorldGeneration {
        tree_frequency: 0.,
        dungeon_stone_frequency: 0.,
        stone_frequency: 0.18,
        dirt_frequency: 0.52,
        sand_frequency: 0.22,
        water_frequency: 0.05,
    };
    dim_event.send(DimensionSpawnEvent {
        generation_params: WorldGeneration {
            tree_frequency: 0.,
            dungeon_stone_frequency: 0.,
            stone_frequency: 0.18,
            dirt_frequency: 0.52,
            sand_frequency: 0.22,
            water_frequency: 0.05,
        },
        seed: Some(0),
        swap_to_dim_now: true,
    });
    // dim_event.send(DimensionSpawnEvent {
    //     generation_params: WorldGeneration {
    //         tree_frequency: 0.,
    //         stone_frequency: 0.18,
    //         dirt_frequency: 0.52,
    //         sand_frequency: 0.22,
    //         water_frequency: 0.05,
    //     },
    //     swap_to_dim_now: true,
    // });

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
            view_formats: &[],
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
            view_formats: &[],
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
                order: -2,
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
                order: -1,
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
                order: 0,
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
                order: 1,
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
    let p = commands
        .spawn((
            SpatialBundle {
                transform: Transform::from_translation(Vec3::new(0., 0., 1.)),
                ..Default::default()
            },
            AnimationTimer(Timer::from_seconds(0.25, TimerMode::Repeating)),
            Player,
            Inventory {
                items: [INVENTORY_INIT; INVENTORY_SIZE],
                crafting_items: [INVENTORY_INIT; 4],
                crafting_result_item: None,
            },
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
        .push_children(&limb_children)
        .id();
    // DEBUG FPS
    commands.spawn((
        Text2dBundle {
            text: Text::from_section(
                "FPS: ",
                TextStyle {
                    font: asset_server.load("fonts/Kitchen Sink.ttf"),
                    font_size: 8.0,
                    color: Color::Rgba {
                        red: 75. / 255.,
                        green: 61. / 255.,
                        blue: 68. / 255.,
                        alpha: 1.,
                    },
                },
            )
            .with_alignment(TextAlignment::Right),
            transform: Transform {
                translation: Vec3::new(GAME_WIDTH / 2. - 10., -GAME_HEIGHT / 2. + 10., 1.),
                scale: Vec3::new(1., 1., 1.),
                ..Default::default()
            },
            ..default()
        },
        Name::new("FPS TEXT"),
        FPSText,
        RenderLayers::from_layers(&[3]),
    ));

    game.player = p;
}
