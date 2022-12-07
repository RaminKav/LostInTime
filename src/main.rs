//TODO:
// - get player movement
// - set up tilemap or world generation
// - trees/entities to break/mine
use bevy::{
    prelude::*,
    render::camera::ScalingMode,
    sprite::collide_aabb::{collide, Collision},
    sprite::MaterialMesh2dBundle,
    time::FixedTimestep,
    window::PresentMode,
};
mod assets;
mod item;
mod world_generation;
use assets::{GameAssetsPlugin, TILE_SIZE};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::TilemapPlugin;
use item::ItemsPlugin;
use world_generation::WorldGenerationPlugin;

const PLAYER_MOVE_SPEED: f32 = 800.;
const TIME_STEP: f32 = 1.0 / 60.0;
const PLAYER_SIZE: f32 = 3.2 / TILE_SIZE;
pub const HEIGHT: f32 = 900.;
pub const RESOLUTION: f32 = 16.0 / 9.0;
pub const WORLD_SIZE: usize = 300;

fn main() {
    App::new()
        .init_resource::<Game>()
        .add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    window: WindowDescriptor {
                        width: HEIGHT * RESOLUTION,
                        height: HEIGHT,
                        title: "DST clone".to_string(),
                        present_mode: PresentMode::Fifo,
                        resizable: false,
                        ..Default::default()
                    },
                    ..default()
                }),
        )
        .add_plugin(TilemapPlugin)
        .add_plugin(GameAssetsPlugin)
        .add_plugin(ItemsPlugin)
        .add_plugin(WorldGenerationPlugin)
        .add_startup_system(setup)
        .add_loading_state(
            LoadingState::new(GameState::Loading)
                .continue_to_state(GameState::Main)
                .with_collection::<ImageAssets>(),
        )
        .add_state(GameState::Loading)
        .add_system_set(
            SystemSet::on_update(GameState::Main)
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(animate_sprite)
                .with_system(move_player),
        )
        .run();
}

#[derive(Resource, Default)]
struct Game {
    player: Player,
    world_size: usize,
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

#[derive(Resource, AssetCollection)]
struct ImageAssets {
    #[asset(path = "bevy_survival_sprites.png")]
    pub sprite_sheet: Handle<Image>,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Component, Default)]
struct Player {
    is_moving: bool,
}

#[derive(Component)]
struct Direction(f32);

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut game: ResMut<Game>,
) {
    game.world_size = WORLD_SIZE;
    game.world_generation_params = WorldGeneration {
        tree_frequency: 0.,
        stone_frequency: 0.0,
        dirt_frequency: 0.35,
        sand_frequency: 0.2,
        water_frequency: 0.12,
    };

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

    let mut camera = Camera2dBundle::default();

    // One unit in world space is one tile
    camera.projection.left = -HEIGHT / TILE_SIZE / 2.0 * RESOLUTION;
    camera.projection.right = HEIGHT / TILE_SIZE / 2.0 * RESOLUTION;
    camera.projection.top = HEIGHT / TILE_SIZE / 2.0;
    camera.projection.bottom = -HEIGHT / TILE_SIZE / 2.0;
    camera.projection.scaling_mode = ScalingMode::None;
    commands.spawn(camera);

    commands.spawn((
        SpriteSheetBundle {
            texture_atlas: player_texture_atlas_handle,
            transform: Transform::from_scale(Vec3::splat(PLAYER_SIZE))
                .with_translation(Vec3::new(0., 0., 1.)),
            ..default()
        },
        AnimationTimer(Timer::from_seconds(0.1, TimerMode::Repeating)),
        Player { is_moving: false },
        Direction(1.0),
    ));
}

fn animate_sprite(
    time: Res<Time>,
    texture_atlases: Res<Assets<TextureAtlas>>,
    game: Res<Game>,
    mut query: Query<(
        &mut AnimationTimer,
        &mut TextureAtlasSprite,
        &Handle<TextureAtlas>,
    )>,
) {
    for (mut timer, mut sprite, handle) in &mut query {
        timer.tick(time.delta());
        if timer.just_finished() && game.player.is_moving {
            let texture_atlas = texture_atlases.get(handle).unwrap();
            sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
        } else if !game.player.is_moving {
            sprite.index = 0
        }
    }
}

fn move_player(
    key_input: Res<Input<KeyCode>>,
    mut game: ResMut<Game>,
    mut player_query: Query<(&mut Transform, &mut Direction), (With<Player>, Without<Camera>)>,
    mut camera_query: Query<&mut Transform, (With<Camera>, Without<Player>)>,
) {
    let (mut player_transform, mut dir) = player_query.single_mut();
    let mut camera_transform = camera_query.single_mut();

    let mut dx = 0.0;
    let mut dy = 0.0;
    let s = 1.0 / TILE_SIZE;
    if key_input.pressed(KeyCode::A) {
        dx -= s;
        game.player.is_moving = true;
        player_transform.rotation = Quat::from_rotation_y(std::f32::consts::PI);
    }
    if key_input.pressed(KeyCode::D) {
        dx += s;
        game.player.is_moving = true;
        player_transform.rotation = Quat::default();
    }
    if key_input.pressed(KeyCode::W) {
        dy += s;
        game.player.is_moving = true;
    }
    if key_input.pressed(KeyCode::S) {
        dy -= s;
        game.player.is_moving = true;
    }
    if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]) {
        game.player.is_moving = false;
    }

    let px = player_transform.translation.x + dx * PLAYER_MOVE_SPEED * TIME_STEP;
    let py = player_transform.translation.y + dy * PLAYER_MOVE_SPEED * TIME_STEP;
    player_transform.translation.x = px;
    player_transform.translation.y = py;
    camera_transform.translation.x = px;
    camera_transform.translation.y = py;

    if dx != 0. {
        dir.0 = dx;
    }
}
