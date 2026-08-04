use crate::aseprite_assets::{DesertTornadoAseprite, PinkFlowerAseprite};
use crate::aseprite_helpers::{ase_animation, aseprite_bundle, is_paused, pause};
use bevy::{
    math::primitives::Circle,
    prelude::*,
    sprite_render::{ColorMaterial, MeshMaterial2d},
};
use bevy_aseprite_ultra::prelude::{AnimationState, AseAnimation};
use bevy_rapier2d::prelude::{
    ActiveCollisionTypes, ActiveEvents, Collider, CollisionGroups, Group,
    KinematicCharacterController, ReadRapierContext, Sensor,
};

use rand::Rng;

use crate::{
    animations::player_sprite::PlayerAnimation,
    attributes::{hunger::Hunger, Speed},
    audio::{AudioSoundEffect, SoundSpawner},
    ecs_helpers::SafeHierarchyExt,
    inputs::MovementVector,
    item::{Equipment, WorldObject},
    player::Player,
    world::{chunk::Chunk, dimension::Era, y_sort::YSort},
    GameParam, MainCamera, PLAYER_MOVE_SPEED,
};

/// Component that holds bounce state. Inserted on the player when a bounce is
/// triggered (e.g. IceWall, skills that dash) and removed when the bounce
/// timer finishes. Stored `SparseSet` to avoid moving the player between
/// archetypes on every bounce.
#[derive(Component)]
#[component(storage = "SparseSet")]
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
        if self.timer.is_finished() {
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
            is_complete: self.timer.is_finished(),
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
        !self.timer.is_finished()
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

#[derive(Debug, Clone, Copy, Message)]
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
    let Ok((player_e, mut player_kcc, mut mv, bounce_opt)) = player_query.single_mut() else {
        return;
    };

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
        (
            Entity,
            &Transform,
            Option<&BounceEffect>,
            &Speed,
            &Hunger,
            &PlayerAnimation,
        ),
        (
            With<Player>,
            Without<MainCamera>,
            Without<Chunk>,
            Without<Equipment>,
        ),
    >,
    time: Res<Time>,
    key_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut bounce_events: MessageReader<BounceEvent>,
) {
    let Ok((player_e, transform, bounce_opt, speed, hunger, anim)) = player_query.single_mut()
    else {
        return;
    };
    if anim == &PlayerAnimation::Lunge {
        return;
    }
    let player = game.player_mut();
    let mut d = Vec2::ZERO;

    // Check if currently bouncing
    let is_bouncing = bounce_opt.as_ref().map_or(false, |b| b.is_active());

    if !is_bouncing {
        // Normal movement input
        if key_input.pressed(KeyCode::KeyA) || key_input.pressed(KeyCode::ArrowLeft) {
            d.x -= 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::KeyD) || key_input.pressed(KeyCode::ArrowRight) {
            d.x += 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::KeyW) || key_input.pressed(KeyCode::ArrowUp) {
            d.y += 1.;
            player.is_moving = true;
        }
        if key_input.pressed(KeyCode::KeyS) || key_input.pressed(KeyCode::ArrowDown) {
            d.y -= 1.;
            player.is_moving = true;
        }

        if d.x != 0. || d.y != 0. {
            d = d.normalize();
        }

        let mut received_bounce_trigger = false;
        for _event in bounce_events.read() {
            received_bounce_trigger = true;
        }

        // Trigger bounce when we have at least one bounce event this frame and movement input
        if received_bounce_trigger && d.length() > 0.0 {
            let start_pos = Vec2::new(transform.translation.x, transform.translation.y);
            let direction = d.normalize();
            let s = PLAYER_MOVE_SPEED
                * time.delta_secs()
                * (1. + speed.0 as f32 / 100.)
                * (if hunger.is_starving() { 0.7 } else { 1. });
            // Use player's current movement speed
            let bounce_speed = s / time.delta_secs() * 3.; // Convert back to units per second

            // Add or update bounce component
            commands.entity(player_e).insert(BounceEffect::new(
                start_pos,
                direction,
                bounce_speed,
                player.is_dashing && player.player_dash_duration.fraction() < 0.25,
                0.35, // duration in seconds
                20.0, // max height
            ));
            commands
                .spawn((
                    Mesh2d(meshes.add(Mesh::from(Circle::new(7.0)))),
                    MeshMaterial2d(
                        materials.add(ColorMaterial::from(Color::srgba(0.0, 0.0, 0.0, 0.3))),
                    ),
                    Transform::from_xyz(0.0, 0.0, -1.0),
                    PlayerShadow { owner: player_e },
                ))
                .safe_set_parent(player_e);
            commands.spawn(SoundSpawner::new(AudioSoundEffect::ItemPickup, 0.35));
        }
    }
}

/// Desert tornado spawned by the Scorpion boss. Travels in a straight line for
/// its lifetime; on player overlap, the player gets lifted into the air (no
/// horizontal travel) and movement is locked for the duration of the lift.
#[derive(Component)]
pub struct DesertTornado {
    pub direction: Vec2,
    pub speed: f32,
    pub lifetime: Timer,
    /// Every N seconds, steer back toward the player.
    pub retarget_timer: Timer,
}

const TORNADO_BOUNCE_DURATION: f32 = 1.25;
const TORNADO_BOUNCE_MAX_HEIGHT: f32 = 28.0;

/// Spawn a desert tornado that travels in `direction` for `lifetime` seconds.
pub fn spawn_desert_tornado(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec2,
    direction: Vec2,
    speed: f32,
    lifetime: f32,
) -> Entity {
    commands
        .spawn((
            aseprite_bundle(
                asset_server.load(DesertTornadoAseprite::PATH),
                "",
                Transform::from_translation(pos.extend(50.)),
                Visibility::Inherited,
                false,
            ),
            DesertTornado {
                direction: direction.normalize_or_zero(),
                speed,
                lifetime: Timer::from_seconds(lifetime, TimerMode::Once),
                retarget_timer: Timer::from_seconds(5.0, TimerMode::Repeating),
            },
            Collider::capsule(Vec2::new(0., -5.), Vec2::new(0., 0.), 10.),
            Sensor,
            ActiveEvents::COLLISION_EVENTS,
            ActiveCollisionTypes::all(),
            YSort(0.),
            CollisionGroups::new(Group::GROUP_2, Group::GROUP_2),
            Name::new("DesertTornado"),
        ))
        .id()
}

/// Move tornadoes along their direction; periodically re-aim toward the player; despawn when
/// lifetime ends.
pub fn update_desert_tornadoes(
    mut commands: Commands,
    mut tornadoes: Query<(Entity, &mut Transform, &mut DesertTornado)>,
    player_q: Query<&GlobalTransform, With<crate::player::Player>>,
    time: Res<Time>,
) {
    for (e, mut tf, mut tornado) in tornadoes.iter_mut() {
        tornado.lifetime.tick(time.delta());
        tornado.retarget_timer.tick(time.delta());
        if tornado.retarget_timer.just_finished() {
            if let Ok(player_tf) = player_q.single() {
                let my = tf.translation.truncate();
                let to_player = (player_tf.translation().truncate() - my).normalize_or_zero();
                if to_player.length_squared() > 0.0001 {
                    tornado.direction = to_player;
                }
            }
        }
        if tornado.lifetime.is_finished() {
            commands.entity(e).despawn();
            continue;
        }
        let delta = tornado.direction * tornado.speed * time.delta_secs();
        tf.translation.x += delta.x;
        tf.translation.y += delta.y;
    }
}

/// When the player overlaps a tornado, lift them straight up (no horizontal
/// component) and lock their movement for the lift duration. Re-triggers when
/// the previous lift completes if the overlap continues.
pub fn handle_tornado_player_overlap(
    mut commands: Commands,
    rapier_context: ReadRapierContext,
    tornadoes: Query<Entity, With<DesertTornado>>,
    player_query: Query<(Entity, &Transform, Option<&BounceEffect>), With<crate::player::Player>>,
) {
    let Ok(rapier_context) = rapier_context.single() else {
        return;
    };
    let Ok((player_e, player_tf, bounce_opt)) = player_query.single() else {
        return;
    };
    if bounce_opt.map(|b| b.is_active()).unwrap_or(false) {
        return;
    }
    for tornado_e in tornadoes.iter() {
        if rapier_context.intersection_pair(player_e, tornado_e) == Some(true)
            || rapier_context.intersection_pair(tornado_e, player_e) == Some(true)
        {
            let start_pos = player_tf.translation.truncate();
            commands.entity(player_e).insert(BounceEffect::new(
                start_pos,
                Vec2::ZERO,
                0.0,
                false,
                TORNADO_BOUNCE_DURATION,
                TORNADO_BOUNCE_MAX_HEIGHT,
            ));
            break;
        }
    }
}

/// Periodically spawns wandering desert tornadoes in the desert biome (Era::Second)
/// as a natural environmental hazard, independent of the Scorpion boss.
#[derive(Resource)]
pub struct NaturalTornadoSpawner {
    pub timer: Timer,
}

impl Default for NaturalTornadoSpawner {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(NATURAL_TORNADO_INTERVAL, TimerMode::Repeating),
        }
    }
}

const NATURAL_TORNADO_INTERVAL: f32 = 20.0;
const NATURAL_TORNADO_LIFETIME: f32 = 30.0;
const NATURAL_TORNADO_SPEED: f32 = 45.0;
/// Spawn radius around the player (just past typical screen edge so it drifts in).
const NATURAL_TORNADO_SPAWN_RADIUS: f32 = 260.0;

/// Spawn a wandering tornado near the player every [`NATURAL_TORNADO_INTERVAL`] seconds
/// while the current era is the desert biome. The tornado retargets the player periodically
/// via [`update_desert_tornadoes`], so a random initial heading is sufficient.
pub fn spawn_natural_desert_tornadoes(
    mut commands: Commands,
    mut spawner: ResMut<NaturalTornadoSpawner>,
    asset_server: Res<AssetServer>,
    game: GameParam,
    time: Res<Time>,
) {
    if game.era.current_era != Era::Second {
        return;
    }
    spawner.timer.tick(time.delta());
    if !spawner.timer.just_finished() {
        return;
    }
    let player_pos = game.player().position.truncate();
    let mut rng = rand::thread_rng();
    let spawn_angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
    let spawn_offset =
        Vec2::new(spawn_angle.cos(), spawn_angle.sin()) * NATURAL_TORNADO_SPAWN_RADIUS;
    let spawn_pos = player_pos + spawn_offset;
    let dir = (player_pos - spawn_pos).normalize_or_zero();
    spawn_desert_tornado(
        &mut commands,
        &asset_server,
        spawn_pos,
        dir,
        NATURAL_TORNADO_SPEED,
        NATURAL_TORNADO_LIFETIME,
    );
}

/// System to spawn Aseprite animation for pink flowers when they're created
pub fn spawn_pink_flower_aseprite(
    mut commands: Commands,
    graphics: Res<crate::assets::Graphics>,
    pink_flowers: Query<(Entity, &Transform, &WorldObject), Added<WorldObject>>,
) {
    for (entity, transform, world_obj) in pink_flowers.iter() {
        if world_obj == &WorldObject::PinkFlower {
            let mut animation = ase_animation(
                graphics.pink_flower_ase.as_ref().unwrap().clone(),
                PinkFlowerAseprite::tags::BOUNCE,
                false,
            );
            pause(&mut animation);

            let Ok(mut entity_commands) = commands.get_entity(entity) else {
                continue;
            };

            entity_commands.insert((
                animation,
                Sprite::default(),
                *transform,
                GlobalTransform::default(),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ));
        }
    }
}

pub fn handle_pink_flower_animation_loop(
    mut flower_query: Query<
        (&mut AseAnimation, &mut AnimationState, &WorldObject),
        (Without<Player>, With<WorldObject>),
    >,
) {
    for (mut anim, mut state, world_obj) in flower_query.iter_mut() {
        if world_obj == &WorldObject::PinkFlower {
            if !is_paused(&anim) {
                let current_frame = usize::from(state.current_frame());
                if current_frame == 12 {
                    pause(&mut anim);
                    state.current_frame = 0;
                    state.relative_frame = 0;
                }
            }
        }
    }
}
