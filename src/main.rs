use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use ai::AIPlugin;
use attributes::{
    Attack, AttributesPlugin, BonusDamage, CritChance, CritDamage, Defence, Dodge, Healing,
    HealthRegen, Lifesteal, LootRateBonus, MaxHealth, Speed, Thorns, XpRateBonus,
};
mod audio;
mod container;

use container::ContainerRegistry;
use juice::JuicePlugin;
use night::NightPlugin;
use rand::Rng;

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

mod juice;
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
mod night;
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
use combat::*;
use enemy::EnemyPlugin;
use inputs::InputsPlugin;
use inventory::ItemStack;
use item::{Equipment, ItemsPlugin, WorldObject, WorldObjectResource};
use player::{Player, PlayerPlugin, PlayerState};
use proto::{proto_param::ProtoParam, ProtoPlugin};

use schematic::SchematicPlugin;
use ui::{InventorySlotState, UIPlugin};
use world::WorldGeneration;
use world::{
    chunk::{Chunk, TileEntityCollection, TileSpriteData},
    generation::WorldObjectCache,
    world_helpers::world_pos_to_tile_pos,
    y_sort::YSort,
    TileMapPosition, WallTextureData, WorldPlugin,
};

use crate::assets::SpriteAnchor;
const ZOOM_SCALE: f32 = 1.;
const PLAYER_MOVE_SPEED: f32 = 90. * ZOOM_SCALE;
const PLAYER_DASH_SPEED: f32 = 250. * ZOOM_SCALE;
pub const TIME_STEP: f32 = 1.0 / 60.0;
pub const HEIGHT: f32 = 1600.;
pub const ASPECT_RATIO: f32 = 16.0 / 9.0;
pub const WIDTH: f32 = HEIGHT * ASPECT_RATIO;
pub const GAME_HEIGHT: f32 = 180. * ZOOM_SCALE;
pub const GAME_WIDTH: f32 = 320. * ZOOM_SCALE;

fn main() {
    App::new()
        .init_resource::<Game>()
        .insert_resource(ContainerRegistry::default())
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
        .add_plugin(UIPlugin)
        .add_plugin(NightPlugin)
        .add_plugin(AIPlugin)
        .add_plugin(AttributesPlugin)
        .add_plugin(CombatPlugin)
        .add_plugin(EnemyPlugin)
        .add_plugin(PlayerPlugin)
        .add_plugin(WorldPlugin)
        .add_plugin(ClientPlugin)
        .add_plugin(ProtoPlugin)
        .add_plugin(SchematicPlugin)
        .add_plugin(JuicePlugin)
        // .add_plugin(DiagnosticExplorerAgentPlugin)
        .add_startup_system(setup)
        .add_loading_state(LoadingState::new(GameState::Loading))
        .add_collection_to_loading_state::<_, ImageAssets>(GameState::Loading)
        .run();
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
    #[asset(path = "NewTiles.png")]
    pub tiles_sheet: Handle<Image>,
    #[asset(path = "SmallWallTextures.png")]
    pub walls_sheet: Handle<Image>,
}

#[derive(SystemParam)]
pub struct GameParam<'w, 's> {
    pub game: ResMut<'w, Game>,
    pub graphics: Res<'w, Graphics>,
    pub world_generation_params: ResMut<'w, WorldGeneration>,
    pub world_obj_data: ResMut<'w, WorldObjectResource>,
    pub world_obj_cache: ResMut<'w, WorldObjectCache>,
    //TODO: remove this to use Bevy_Save
    pub player_query: Query<'w, 's, Entity, With<Player>>,
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
    ) -> Option<Entity> {
        for (obj_e, g_txm, size, obj) in self.world_object_query.iter() {
            let anchor = proto_param
                .get_component::<SpriteAnchor, _>(*obj)
                .unwrap_or(&SpriteAnchor(Vec2::ZERO));
            let pos = world_pos_to_tile_pos(g_txm.translation().truncate() - anchor.0);
            if size.is_medium() {
                for neighbour_pos in pos
                    .get_neighbour_tiles_for_medium_objects()
                    .iter()
                    .chain(vec![pos].iter())
                {
                    if neighbour_pos == &tile {
                        return Some(obj_e);
                    }
                }
            } else if pos == tile {
                return Some(obj_e);
            }
        }

        None
    }
    pub fn get_wall_data_at_tile(
        &self,
        tile: TileMapPosition,
        proto_param: &ProtoParam,
    ) -> Option<WallTextureData> {
        if let Some(e) = self.get_obj_entity_at_tile(tile, proto_param) {
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
        if let Some(e) = self.get_obj_entity_at_tile(tile, proto_param) {
            if let Ok(data) = self.wall_data_query.get_mut(e) {
                return Some(data.1);
            }
        }
        None
    }

    pub fn calculate_player_damage(&self) -> (u32, bool) {
        let (attack, _, _, crit_chance, crit_dmg, bonus_dmg, ..) = self.player_stats.single();
        let mut rng = rand::thread_rng();
        if rng.gen_ratio(u32::min(100, crit_chance.0.try_into().unwrap_or(0)), 100) {
            (
                ((attack.0 + bonus_dmg.0) as f32 * (f32::abs(crit_dmg.0 as f32) / 100.)) as u32,
                true,
            )
        } else {
            ((attack.0 + bonus_dmg.0) as u32, false)
        }
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
#[derive(Component, Debug, Default)]
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
