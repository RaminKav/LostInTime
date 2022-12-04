//TODO:
// - get player movement
// - set up tilemap or world generation
// - trees/entities to break/mine
use bevy::{
    prelude::*,
    sprite::collide_aabb::{collide, Collision},
    sprite::MaterialMesh2dBundle,
    time::FixedTimestep,
};

const PLAYER_MOVE_SPEED: f32 = 300.;
const TIME_STEP: f32 = 1.0 / 60.0;
const PLAYER_SIZE: f32 = 3.0;

fn main() {
    App::new()
        .init_resource::<Game>()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_startup_system(setup)
        .add_system_set(
            SystemSet::new()
                .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                .with_system(animate_sprite)
                .with_system(move_player),
        )
        .run();
}

#[derive(Resource, Default)]
struct Game {
    player: Player,
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
) {
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

    commands.spawn(Camera2dBundle::default());
    commands.spawn((
        SpriteSheetBundle {
            texture_atlas: player_texture_atlas_handle,
            transform: Transform::from_scale(Vec3::splat(PLAYER_SIZE)),
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
    mut query: Query<(&mut Transform, &mut Direction), With<Player>>,
) {
    let (mut player_transform, mut dir) = query.single_mut();

    let mut dx = 0.0;
    let mut dy = 0.0;
    if key_input.pressed(KeyCode::A) {
        dx -= 1.0;
        game.player.is_moving = true;
    }
    if key_input.pressed(KeyCode::D) {
        dx += 1.0;
        game.player.is_moving = true;
    }
    if key_input.pressed(KeyCode::W) {
        dy += 1.0;
        game.player.is_moving = true;
    }
    if key_input.pressed(KeyCode::S) {
        dy -= 1.0;
        game.player.is_moving = true;
    }
    if key_input.any_just_released([KeyCode::A, KeyCode::D, KeyCode::S, KeyCode::W]) {
        game.player.is_moving = false;
    }

    let px = player_transform.translation.x + dx * PLAYER_MOVE_SPEED * TIME_STEP;
    let py = player_transform.translation.y + dy * PLAYER_MOVE_SPEED * TIME_STEP;
    player_transform.translation.x = px;
    player_transform.translation.y = py;
    player_transform.scale = Vec3::new(
        PLAYER_SIZE * (if dx == 0. { dir.0 } else { dx }),
        PLAYER_SIZE,
        PLAYER_SIZE,
    );
    if dx != 0. {
        dir.0 = dx;
    }
}
