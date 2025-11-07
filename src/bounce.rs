use bevy::{prelude::*, sprite::MaterialMesh2dBundle};
use bevy_rapier2d::prelude::KinematicCharacterController;

use crate::{
    attributes::{hunger::Hunger, Speed},
    audio::{AudioSoundEffect, SoundSpawner},
    inputs::MovementVector,
    item::Equipment,
    player::Player,
    world::chunk::Chunk,
    GameParam, MainCamera, PLAYER_MOVE_SPEED,
};

/// Component that holds bounce state
#[derive(Component)]
pub struct BounceEffect {
    pub start_pos: Vec2,
    pub direction: Vec2,
    pub speed: f32,
    pub max_height: f32,
    pub timer: Timer,
    pub last_pos: Vec2, // Track last frame's position for delta calculation
    pub last_height: f32,
    pub dash_boost: bool,
}

impl BounceEffect {
    pub fn new(
        start_pos: Vec2,
        direction: Vec2,
        speed: f32,
        dash_boost: bool,
        duration: f32,
        max_height: f32,
    ) -> Self {
        Self {
            start_pos,
            direction: direction.normalize_or_zero(),
            speed,
            max_height,
            timer: Timer::from_seconds(duration, TimerMode::Once),
            last_pos: start_pos,
            last_height: 0.,
            dash_boost,
        }
    }

    /// Calculate current position and height based on timer progress
    pub fn update(&mut self) -> BounceState {
        if self.timer.finished() {
            return BounceState::default();
        }

        let elapsed = self.timer.elapsed_secs();
        let t = self.timer.elapsed_secs() / self.timer.duration().as_secs_f32();

        // Calculate distance traveled based on speed and time
        let distance_traveled = (self.speed * if self.dash_boost { 1.75 } else { 1.0 }) * elapsed;
        let offset = self.direction * distance_traveled;

        // Current ground position
        let current_ground_pos = self.start_pos + offset;

        // Calculate delta from last frame
        let mut delta = current_ground_pos - self.last_pos;
        self.last_pos = current_ground_pos;

        // Parabolic arc for height (sine wave)
        let current_z = self.max_height * (std::f32::consts::PI * t).sin();
        let height_delta = current_z - self.last_height;
        self.last_height = current_z;
        delta.y += height_delta;

        // Shadow properties
        let height_ratio = current_z / self.max_height;
        let shadow_scale = 1.0 - height_ratio * 0.5;
        let shadow_alpha = 0.3 + height_ratio * 0.2;

        BounceState {
            ground_pos: current_ground_pos,
            delta,
            height: current_z,
            shadow_scale,
            shadow_alpha,
            is_complete: self.timer.finished(),
            progress: t,
        }
    }

    pub fn reset(&mut self, start_pos: Vec2, direction: Vec2, speed: f32) {
        self.start_pos = start_pos;
        self.direction = direction.normalize_or_zero();
        self.speed = speed;
        self.timer.reset();
    }

    /// Get the total distance that will be traveled during this bounce
    pub fn total_distance(&self) -> f32 {
        self.speed * self.timer.duration().as_secs_f32()
    }

    pub fn is_active(&self) -> bool {
        !self.timer.finished()
    }
}

#[derive(Default)]
pub struct BounceState {
    pub ground_pos: Vec2,
    pub delta: Vec2,
    pub height: f32,
    pub shadow_scale: f32,
    pub shadow_alpha: f32,
    pub is_complete: bool,
    pub progress: f32,
}

/// Shadow component to track the player's shadow
#[derive(Component)]
pub struct PlayerShadow {
    pub owner: Entity,
}

#[derive(Debug, Clone, Copy)]
pub struct BounceEvent;
/// System to update bounce effect and apply to player position
pub fn update_bounce_effect(
    mut player_query: Query<
        (
            Entity,
            &mut KinematicCharacterController,
            &mut MovementVector,
            Option<&mut BounceEffect>,
        ),
        (
            With<Player>,
            Without<MainCamera>,
            Without<Chunk>,
            Without<Equipment>,
        ),
    >,
    time: Res<Time>,
    mut commands: Commands,
    shadow: Query<Entity, With<PlayerShadow>>,
) {
    let (player_e, mut player_kcc, mut mv, bounce_opt) = player_query.single_mut();

    let Some(mut bounce) = bounce_opt else {
        return;
    };

    bounce.timer.tick(time.delta());
    let state = bounce.update();
    let d = state.delta;
    mv.0 = d;
    player_kcc.translation = Some(Vec2::new(d.x, d.y));
    if !bounce.is_active() {
        commands.entity(player_e).remove::<BounceEffect>();
        for shadow in shadow.iter() {
            commands.entity(shadow).despawn();
        }
        return;
    }
}

/// System to update shadow position and appearance
pub fn update_shadow(
    bounce_query: Query<&BounceEffect>,
    mut shadow_query: Query<(&PlayerShadow, &mut Transform), Without<BounceEffect>>,
) {
    for (shadow, mut shadow_transform) in shadow_query.iter_mut() {
        if let Ok(bounce) = bounce_query.get(shadow.owner) {
            if !bounce.is_active() {
                continue;
            }

            // Recalculate ground position for shadow
            let t = bounce.timer.elapsed_secs() / bounce.timer.duration().as_secs_f32();

            let current_z = bounce.max_height * (std::f32::consts::PI * t).sin();
            let height_ratio = current_z / bounce.max_height;
            let shadow_scale = 1.0 - height_ratio * 0.5;

            // Position shadow at ground position
            shadow_transform.translation.y = -current_z;
            shadow_transform.translation.z = -1.0; // Below player

            // Scale shadow based on height
            shadow_transform.scale = Vec3::splat(shadow_scale);

            // Update shadow transparency
        }
    }
}

/// Integration with your existing player movement system
pub fn bounce_player(
    mut game: GameParam,
    mut player_query: Query<
        (Entity, &Transform, Option<&BounceEffect>, &Speed, &Hunger),
        (
            With<Player>,
            Without<MainCamera>,
            Without<Chunk>,
            Without<Equipment>,
        ),
    >,
    time: Res<Time>,
    key_input: Res<Input<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bounce_events: EventReader<BounceEvent>,
) {
    let (player_e, transform, bounce_opt, speed, hunger) = player_query.single_mut();

    let player = game.player_mut();
    let mut d = Vec2::ZERO;

    // Check if currently bouncing
    let is_bouncing = bounce_opt.as_ref().map_or(false, |b| b.is_active());

    if !is_bouncing {
        // Normal movement input
        if key_input.pressed(KeyCode::A) || key_input.pressed(KeyCode::Left) {
            d.x -= 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::D) || key_input.pressed(KeyCode::Right) {
            d.x += 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::W) || key_input.pressed(KeyCode::Up) {
            d.y += 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::S) || key_input.pressed(KeyCode::Down) {
            d.y -= 1.;
            player.is_moving = true;
        }

        if d.x != 0. || d.y != 0. {
            d = d.normalize();
        }

        let mut received_bounce_trigger = false;
        for _event in bounce_events.iter() {
            received_bounce_trigger = true;
        }

        // Trigger bounce when we have at least one bounce event this frame and movement input
        if received_bounce_trigger && d.length() > 0.0 {
            let start_pos = Vec2::new(transform.translation.x, transform.translation.y);
            let direction = d.normalize();
            let s = PLAYER_MOVE_SPEED
                * time.delta_seconds()
                * (1. + speed.0 as f32 / 100.)
                * (if hunger.is_starving() { 0.7 } else { 1. });
            // Use player's current movement speed
            let bounce_speed = s / time.delta_seconds() * 3.; // Convert back to units per second

            // Add or update bounce component
            commands.entity(player_e).insert(BounceEffect::new(
                start_pos,
                direction,
                bounce_speed,
                player.is_dashing && player.player_dash_duration.percent() < 0.25,
                0.35, // duration in seconds
                20.0, // max height
            ));
            commands
                .spawn((
                    MaterialMesh2dBundle {
                        mesh: meshes
                            .add(
                                shape::Circle {
                                    radius: 7.0,
                                    ..Default::default()
                                }
                                .into(),
                            )
                            .into(),
                        material: materials
                            .add(ColorMaterial::from(Color::rgba(0.0, 0.0, 0.0, 0.3))),
                        transform: Transform::from_xyz(0.0, 0.0, -1.0),
                        ..default()
                    },
                    PlayerShadow { owner: player_e },
                ))
                .set_parent(player_e);
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.35));
        }
    }
}
