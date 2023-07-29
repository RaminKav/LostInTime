use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use ai::AIPlugin;
use attributes::AttributesPlugin;

use bevy::{
    core_pipeline::clear_color::ClearColorConfig,
    diagnostic::FrameTimeDiagnosticsPlugin,
    ecs::{schedule::ScheduleLabel, system::SystemParam},
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
    window::{PresentMode, WindowResolution},
};

use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::prelude::*;
mod ai;
mod animations;
mod assets;
mod attributes;
mod client;
mod colors;
mod combat;
mod custom_commands;
mod enemy;
mod inputs;
mod inventory;
mod item;
mod player;
mod proto;
mod schematic;
mod ui;
mod world;
use animations::AnimationsPlugin;
use assets::{GameAssetsPlugin, Graphics, SpriteSize};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use client::ClientPlugin;
use combat::CombatPlugin;
use enemy::{spawner::ChunkSpawners, EnemyPlugin};
use inputs::InputsPlugin;
use inventory::InventoryPlugin;
use item::{Equipment, ItemsPlugin, WorldObject, WorldObjectResource};
use player::{Player, PlayerPlugin, PlayerState};
use proto::{proto_param::ProtoParam, ProtoPlugin};

use schematic::SchematicPlugin;
use ui::{InventorySlotState, UIPlugin};
use world::{
    chunk::{Chunk, ChunkObjectCache, TileEntityCollection, TileSpriteData},
    world_helpers::world_pos_to_tile_pos,
    y_sort::YSort,
    TileMapPosition, WorldObjectEntityData, WorldPlugin,
};
use world::{ChunkManager, WorldGeneration};

use crate::assets::SpriteAnchor;
const ZOOM_SCALE: f32 = 1.;
const PLAYER_MOVE_SPEED: f32 = 2. * ZOOM_SCALE;
const PLAYER_DASH_SPEED: f32 = 125. * ZOOM_SCALE;
pub const TIME_STEP: f32 = 1.0 / 60.0;
pub const HEIGHT: f32 = 1600.;
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;
pub const WIDTH: f32 = HEIGHT * ASPECT_RATIO;
pub const GAME_HEIGHT: f32 = 180. * ZOOM_SCALE;
pub const GAME_WIDTH: f32 = 320. * ZOOM_SCALE;

fn main() {
    App::new()
        .init_resource::<Game>()
        .add_state::<GameState>()
        .edit_schedule(CoreSchedule::FixedUpdate, |s| {
            s.configure_set(CoreGameSet::Main.run_if(in_state(GameState::Main)));
        })
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // Enable hot-reloading of assets:
                    watch_for_changes: true,
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                // .disable::<LogPlugin>()
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: WindowResolution::new(WIDTH, HEIGHT)
                            .with_scale_factor_override(1.0),
                        title: "Survival Game".to_string(),
                        present_mode: PresentMode::Fifo,
                        resizable: false,
                        transparent: true,
                        ..Default::default()
                    }),
                    ..default()
                }),
        )
        .insert_resource(Msaa::Off)
        .insert_resource(FixedTime::new_from_secs(TIME_STEP))
        // .add_plugin(RapierDebugRenderPlugin::default())
        .add_plugin(FrameTimeDiagnosticsPlugin::default())
        .add_plugin(Material2dPlugin::<UITextureMaterial>::default())
        .add_plugin(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
        .add_plugin(WorldInspectorPlugin::new())
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
        .add_plugin(ClientPlugin)
        .add_plugin(ProtoPlugin)
        .add_plugin(SchematicPlugin)
        // .add_plugin(DiagnosticExplorerAgentPlugin)
        .add_startup_system(setup)
        .add_loading_state(LoadingState::new(GameState::Loading).continue_to_state(GameState::Main))
        .add_collection_to_loading_state::<_, ImageAssets>(GameState::Loading)
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
#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub enum CoreGameSet {
    Main,
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
    #[asset(path = "SmallWallTextures.png")]
    pub walls_sheet: Handle<Image>,
}

#[derive(SystemParam)]
pub struct GameParam<'w, 's> {
    pub game: ResMut<'w, Game>,
    pub graphics: Res<'w, Graphics>,
    pub chunk_obj_cache: ResMut<'w, ChunkObjectCache>,
    pub chunk_manager: ResMut<'w, ChunkManager>,
    pub world_generation_params: ResMut<'w, WorldGeneration>,
    pub world_obj_data: ResMut<'w, WorldObjectResource>,
    //TODO: remove this to use Bevy_Save
    pub player_query: Query<'w, 's, (Entity, &'static mut Player)>,
    pub chunk_query: Query<
        'w,
        's,
        (Entity, &'static Transform, &'static mut ChunkSpawners),
        (With<Chunk>, Without<Player>),
    >,
    pub tile_collection_query: Query<'w, 's, &'static TileEntityCollection, With<Chunk>>,
    pub tile_data_query: Query<'w, 's, (&'static mut TileSpriteData, Option<&'static Children>)>,
    pub world_object_query: Query<
        'w,
        's,
        (
            Entity,
            &'static GlobalTransform,
            &'static SpriteSize,
            &'static mut WorldObjectEntityData,
        ),
        (With<WorldObject>, With<WorldObjectEntityData>),
    >,
    pub equipment: Query<'w, 's, (Entity, &'static Equipment)>,
    pub inv_slot_query: Query<'w, 's, &'static mut InventorySlotState>,

    #[system_param(ignore)]
    marker: PhantomData<&'s ()>,
}

impl<'w, 's> GameParam<'w, 's> {
    pub fn player(&self) -> PlayerState {
        self.game.player_state.clone()
    }
    pub fn player_mut(&mut self) -> &mut PlayerState {
        &mut self.game.player_state
    }
    pub fn get_chunk_entity(&self, chunk_pos: IVec2) -> Option<&Entity> {
        self.chunk_manager.chunks.get(&chunk_pos.into())
    }
    pub fn set_chunk_entity(&mut self, chunk_pos: IVec2, e: Entity) {
        self.chunk_manager.chunks.insert(chunk_pos.into(), e);
    }
    pub fn add_object_to_chunk_cache(&mut self, pos: TileMapPosition, obj: WorldObject) {
        self.chunk_obj_cache
            .cache
            .entry(pos.chunk_pos.into())
            .or_insert_with(Vec::new)
            .push((obj, pos));
    }
    pub fn get_objects_from_chunk_cache(
        &self,
        chunk_pos: IVec2,
    ) -> Option<&Vec<(WorldObject, TileMapPosition)>> {
        self.chunk_obj_cache.cache.get(&chunk_pos.into())
    }
    pub fn remove_chunk_entity(&mut self, chunk_pos: IVec2) {
        self.chunk_manager.chunks.remove(&chunk_pos.into());
    }

    pub fn get_tile_entity(&self, tile: TileMapPosition) -> Option<Entity> {
        if let Some(chunk_e) = self.get_chunk_entity(tile.chunk_pos) {
            let tile_collection = self.tile_collection_query.get(*chunk_e).unwrap();
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
    ) -> Option<Entity> {
        for (obj_e, g_txm, size, obj_data) in self.world_object_query.iter() {
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(obj_data.object.clone())
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            let pos = world_pos_to_tile_pos(g_txm.translation().truncate() - anchor.0);
            if size.is_medium() && pos.matches_tile(&tile) {
                return Some(obj_e);
            } else if pos == tile {
                return Some(obj_e);
            }
        }
        None
    }
    pub fn get_tile_obj_data(
        &self,
        tile: TileMapPosition,
        proto_param: &ProtoParam,
    ) -> Option<WorldObjectEntityData> {
        if let Some(e) = self.get_obj_entity_at_tile(tile, proto_param) {
            return Some(self.world_object_query.get(e).unwrap().3.clone());
        }
        None
    }
    pub fn get_tile_obj_data_mut(
        &mut self,
        tile: TileMapPosition,
        proto_param: &ProtoParam,
    ) -> Option<Mut<WorldObjectEntityData>> {
        if let Some(e) = self.get_obj_entity_at_tile(tile, proto_param) {
            return Some(self.world_object_query.get_mut(e).unwrap().3);
        }
        None
    }
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
    mut meshes: ResMut<Assets<Mesh>>,
    mut game_render_materials: ResMut<Assets<ColorMaterial>>,
    mut ui_render_materials: ResMut<Assets<UITextureMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
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
