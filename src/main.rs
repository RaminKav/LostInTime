use std::time::Duration;

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
use assets::{GameAssetsPlugin, Graphics, WORLD_SCALE};
use bevy_asset_loader::prelude::{AssetCollection, LoadingState, LoadingStateAppExt};
use bevy_ecs_tilemap::{tiles::TilePos, TilemapPlugin};
use bevy_inspector_egui::InspectorPlugin;
use item::{ItemsPlugin, WorldObject};
use world_generation::{ChunkManager, WorldGenerationPlugin, CHUNK_SIZE};

use crate::world_generation::TileMapPositionData;

const PLAYER_MOVE_SPEED: f32 = 450.;
const PLAYER_DASH_SPEED: f32 = 1250.;
const TIME_STEP: f32 = 1.0 / 60.0;
const PLAYER_SIZE: f32 = 3.2 / WORLD_SCALE;
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
        .insert_resource(CursorPos(Vec3::new(-100.0, -100.0, 0.0)))
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
                .with_system(move_player)
                .with_system(update_cursor_pos.after(move_player))
                .with_system(mouse_click_system),
        )
        .run();
}

#[derive(Resource, Default)]
struct Game {
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
        dirt_frequency: 0.52,
        sand_frequency: 0.22,
        water_frequency: 0.05,
    };
    game.player_dash_cooldown = Timer::from_seconds(0.5, TimerMode::Once);
    game.player_dash_duration = Timer::from_seconds(0.05, TimerMode::Once);

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
    camera.projection.left = -HEIGHT / WORLD_SCALE / 2.0 * RESOLUTION;
    camera.projection.right = HEIGHT / WORLD_SCALE / 2.0 * RESOLUTION;
    camera.projection.top = HEIGHT / WORLD_SCALE / 2.0;
    camera.projection.bottom = -HEIGHT / WORLD_SCALE / 2.0;
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
        Player {
            is_moving: false,
            is_dashing: false,
        },
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
        let d = time.delta();
        timer.tick(if game.player.is_dashing {
            Duration::new(
                (d.as_secs() as f32 * 4.) as u64,
                (d.subsec_nanos() as f32 * 4.) as u32,
            )
        } else {
            d
        });
        if timer.just_finished() && game.player.is_moving {
            let texture_atlas = texture_atlases.get(handle).unwrap();
            sprite.index = (sprite.index + 1) % texture_atlas.textures.len();
        } else if !game.player.is_moving {
            sprite.index = 0
        }
    }
}

fn move_player(
    mut key_input: ResMut<Input<KeyCode>>,
    mut game: ResMut<Game>,
    mut player_query: Query<(&mut Transform, &mut Direction), (With<Player>, Without<Camera>)>,
    mut camera_query: Query<&mut Transform, (With<Camera>, Without<Player>)>,
    time: Res<Time>,
) {
    let (mut player_transform, mut dir) = player_query.single_mut();
    let mut camera_transform = camera_query.single_mut();

    let mut dx = 0.0;
    let mut dy = 0.0;
    let s = 1.0 / WORLD_SCALE;

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
    if game.player_dash_cooldown.tick(time.delta()).finished() {
        if key_input.pressed(KeyCode::Space) {
            game.player.is_dashing = true;

            game.player_dash_cooldown.reset();
            game.player_dash_duration.reset();
        }
    }
    if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W])
        || (dx == 0. && dy == 0.)
    {
        game.player.is_moving = false;
        // key_input.release_all();
    }

    let mut px = player_transform.translation.x + dx * PLAYER_MOVE_SPEED * TIME_STEP;
    let mut py = player_transform.translation.y + dy * PLAYER_MOVE_SPEED * TIME_STEP;

    if game.player.is_dashing {
        game.player_dash_duration.tick(time.delta());

        px += dx * PLAYER_DASH_SPEED * TIME_STEP;
        py += dy * PLAYER_DASH_SPEED * TIME_STEP;
        if game.player_dash_duration.just_finished() {
            game.player.is_dashing = false;
        }
    }
    player_transform.translation.x = px;
    player_transform.translation.y = py;
    camera_transform.translation.x = px;
    camera_transform.translation.y = py;
    if game.player.is_moving == true {
        // println!(
        //     "Player is on chunk {:?} at pos: {:?}",
        //     WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
        //         player_transform.translation.x,
        //         player_transform.translation.y
        //     )),
        //     player_transform.translation
        // );
    }

    if dx != 0. {
        dir.0 = dx;
    }
}

#[derive(Default, Resource)]
pub struct CursorPos(Vec3);

pub fn update_cursor_pos(
    windows: Res<Windows>,
    camera_q: Query<(&Transform, &Camera)>,
    mut cursor_moved_events: EventReader<CursorMoved>,
    mut cursor_pos: ResMut<CursorPos>,
) {
    for cursor_moved in cursor_moved_events.iter() {
        // To get the mouse's world position, we have to transform its window position by
        // any transforms on the camera. This is done by projecting the cursor position into
        // camera space (world space).
        for (cam_t, cam) in camera_q.iter() {
            *cursor_pos = CursorPos(cursor_pos_in_world(
                &windows,
                cursor_moved.position,
                cam_t,
                cam,
            ));
        }
    }
}
// Converts the cursor position into a world position, taking into account any transforms applied
// the camera.
pub fn cursor_pos_in_world(
    windows: &Windows,
    cursor_pos: Vec2,
    cam_t: &Transform,
    cam: &Camera,
) -> Vec3 {
    let window = windows.primary();

    let window_size = Vec2::new(window.width(), window.height());

    // Convert screen position [0..resolution] to ndc [-1..1]
    // (ndc = normalized device coordinates)
    let ndc_to_world = cam_t.compute_matrix() * cam.projection_matrix().inverse();
    let ndc = (cursor_pos / window_size) * 2.0 - Vec2::ONE;
    ndc_to_world.project_point3(ndc.extend(0.0))
}

fn mouse_click_system(
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_pos: Res<CursorPos>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    if mouse_button_input.just_released(MouseButton::Left) {
        let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
            cursor_pos.0.x,
            cursor_pos.0.y,
        ));
        let tile_pos = WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
            cursor_pos.0.x,
            cursor_pos.0.y,
        ));
        let stone = WorldObject::StoneHalf.spawn(
            &mut commands,
            &graphics,
            Vec3::new(
                (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                (tile_pos.y * 32 + 16 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                0.1,
            ),
        );
        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            // .push_children(&children)
            .push_children(&[stone]);
        // WorldGenerationPlugin::change_tile_and_update_neighbours(
        //     TilePos {
        //         x: tile_pos.x as u32,
        //         y: tile_pos.y as u32,
        //     },
        //     chunk_pos,
        //     0b0000,
        //     0,
        //     &mut chunk_manager,
        //     &mut commands,
        // );
    }
    if mouse_button_input.just_released(MouseButton::Right) {
        let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
            cursor_pos.0.x,
            cursor_pos.0.y,
        ));
        let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
            cursor_pos.0.x,
            cursor_pos.0.y,
        )));
        let stone = WorldObject::StoneTop.spawn(
            &mut commands,
            &graphics,
            Vec3::new(
                (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                0.1,
            ),
        );
        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            // .push_children(&children)
            .push_children(&[stone]);
        // WorldGenerationPlugin::change_tile_and_update_neighbours(
        //     TilePos {
        //         x: tile_pos.x as u32,
        //         y: tile_pos.y as u32,
        //     },
        //     chunk_pos,
        //     0b0000,
        //     16,
        //     &mut chunk_manager,
        //     &mut commands,
        // );
    }
    if mouse_button_input.just_released(MouseButton::Middle) {
        let chunk_pos = WorldGenerationPlugin::camera_pos_to_chunk_pos(&Vec2::new(
            cursor_pos.0.x,
            cursor_pos.0.y,
        ));
        let tile_pos = dbg!(WorldGenerationPlugin::camera_pos_to_block_pos(&Vec2::new(
            cursor_pos.0.x,
            cursor_pos.0.y,
        )));
        let stone = WorldObject::StoneFull.spawn(
            &mut commands,
            &graphics,
            Vec3::new(
                (tile_pos.x * 32 + chunk_pos.x * CHUNK_SIZE as i32 * 32) as f32,
                (tile_pos.y * 32 + chunk_pos.y * CHUNK_SIZE as i32 * 32) as f32,
                0.1,
            ),
        );
        commands
            .spawn(SpatialBundle::default())
            // .insert(Name::new("Test Objects"))
            // .push_children(&children)
            .push_children(&[stone]);
        // WorldGenerationPlugin::change_tile_and_update_neighbours(
        //     TilePos {
        //         x: tile_pos.x as u32,
        //         y: tile_pos.y as u32,
        //     },
        //     chunk_pos,
        //     0b0000,
        //     16,
        //     &mut chunk_manager,
        //     &mut commands,
        // );
    }
}
