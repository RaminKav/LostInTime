//! Void Worm: an endless-mode void-family enemy.
//!
//! Movement reuses the shared aseprite walk/follow behavior (see
//! [`crate::enemy::aseprite_enemy`]). The laser is a separate aseprite whose
//! **animations** draw the beam — we never rotate the laser entity, only flip
//! and position it.
//!
//! Art defaults (scale 1,1, no rotation):
//! - `Vertical` tag: beam points **down**, base on the left edge of the sprite.
//! - `Angled` tag: beam points **down-left**, base on the right edge ~⅓ down.

use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::Collider;

use crate::{
    ai::{EnemyAttackCooldown, FollowState},
    assets::Graphics,
    attributes::Attack,
    combat::{
        combat_helpers::{spawn_temp_collider, DespawnTimer},
        status_effects::MobStatusEffects,
    },
    enemy::{aseprite_enemy::CurrentAsepriteTag, FollowSpeed, Mob, MobIsAttacking},
    inputs::FacingDirection,
    item::projectile::{EnemyProjectile, Projectile},
    player::{combat_heirlooms::DeathDefianceFrozen, Player},
};

aseprite!(pub VoidLaserAse, "textures/VoidWorm/VoidLaser.ase");

const WALK_DOWN: &str = "WalkDown";
const ATTACK_UP: &str = "AttackUp";
const ATTACK_DOWN: &str = "AttackDown";
const ATTACK_SIDE: &str = "AttackSide";
const LASER_VERTICAL: &str = "Vertical";
const LASER_ANGLED: &str = "Angled";

const EYE_OFFSET: Vec2 = Vec2::new(0., 20.);

const LASER_DAMAGE_LENGTH: f32 = 80.;
const LASER_DAMAGE_RADIUS: f32 = 2.5;

/// Damage hitbox nudge in **unrotated laser art space** (+x right, +y up).
/// One value per laser asset, rotated by `spawn_cfg.rotation` into world space so
/// every beam direction gets the same correction (mirrored/flipped as needed).
/// Tuned at down / down-left (rotation 0); other orientations follow via rotation.
const HITBOX_OFFSET_STRAIGHT: Vec2 = Vec2::new(0.6, 0.8);
const HITBOX_OFFSET_ANGLED: Vec2 = Vec2::new(-8.4, -16.4);

const ANGLED_BEAM_OFFSET_RAD: f32 = std::f32::consts::FRAC_PI_4;

// --- Unflipped art defaults (67×83 VoidLaser.ase, center anchor) ---
/// Vertical tag: beam shoots toward screen bottom.
const ART_VERTICAL_DIR: Vec2 = Vec2::new(0., -1.);
/// Angled tag: beam shoots down-left.
const ART_ANGLED_DIR: Vec2 = Vec2::new(
    -std::f32::consts::FRAC_1_SQRT_2,
    -std::f32::consts::FRAC_1_SQRT_2,
);
/// Vector from laser sprite **center** → **eye / beam base** (unflipped Vertical).
const ART_VERTICAL_CENTER_TO_EYE: Vec2 = Vec2::new(-27.5, 38.5);
/// Vector from laser sprite **center** → **eye / beam base** (unflipped Angled).
const ART_ANGLED_CENTER_TO_EYE: Vec2 = Vec2::new(27.5, -13.5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WormFacing {
    Up,
    Down,
    Left,
    Right,
}

impl WormFacing {
    fn world_dir(self) -> Vec2 {
        match self {
            Self::Up => Vec2::Y,
            Self::Down => Vec2::NEG_Y,
            Self::Left => Vec2::NEG_X,
            Self::Right => Vec2::X,
        }
    }

    fn attack_tag(self) -> &'static str {
        match self {
            Self::Up => ATTACK_UP,
            Self::Down => ATTACK_DOWN,
            Self::Left | Self::Right => ATTACK_SIDE,
        }
    }

    fn worm_scale_x(self) -> f32 {
        match self {
            Self::Left => -1.,
            Self::Right => 1.,
            Self::Up | Self::Down => 1.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LaserKind {
    Straight,
    Angled,
}

#[derive(Clone, Copy, Debug)]
struct LaserSpawn {
    tag: &'static str,
    /// Z rotation (radians) applied to the laser sprite to orient the art's
    /// default beam direction onto `aim_dir`. Always a multiple of 90°.
    rotation: f32,
    /// Vector from laser sprite center → eye (after rotation is applied).
    center_to_eye: Vec2,
    /// Actual world beam direction after rotation.
    beam_dir: Vec2,
}

/// Orient the laser by **rotating** the art's default beam direction onto
/// `aim_dir`. Flips alone can't produce horizontal beams (the Vertical art only
/// points up/down), so a left/right-facing worm needs the sprite rotated. Since
/// `aim_dir` is always a compass direction and the art defaults are cardinal /
/// diagonal, the rotation works out to a clean multiple of 90°.
fn build_laser_spawn(kind: LaserKind, aim_dir: Vec2) -> LaserSpawn {
    let (art_dir, center_to_eye, tag) = match kind {
        LaserKind::Straight => (ART_VERTICAL_DIR, ART_VERTICAL_CENTER_TO_EYE, LASER_VERTICAL),
        LaserKind::Angled => (ART_ANGLED_DIR, ART_ANGLED_CENTER_TO_EYE, LASER_ANGLED),
    };
    let art_angle = art_dir.y.atan2(art_dir.x);
    let aim_angle = aim_dir.y.atan2(aim_dir.x);
    let rotation = aim_angle - art_angle;
    let center_to_eye = rotate_vec2(center_to_eye, rotation);
    let beam_dir = aim_dir.normalize_or_zero();
    LaserSpawn {
        tag,
        rotation,
        center_to_eye,
        beam_dir,
    }
}

/// Per-(mob facing, beam direction) pixel nudge for the **laser sprite** only.
/// World space (+x right, +y up). Tweak these values to fine-tune; any combo not
/// listed defaults to no correction. Hitbox placement uses
/// [`hitbox_offset_world`] instead.
///
/// `octant` is the beam direction snapped to eighths: 0=E, 1=NE, 2=N, 3=NW,
/// 4=W, 5=SW, 6=S, 7=SE.
fn laser_offset_correction(facing: WormFacing, beam_dir: Vec2) -> Vec2 {
    use WormFacing::{Down, Left, Right, Up};
    let octant = beam_octant(beam_dir);
    match (facing, octant) {
        (Down, 6) => Vec2::new(-3., -2.),   // straight down
        (Down, 5) => Vec2::new(-5., -31.),  // angled down-left
        (Down, 7) => Vec2::new(30., -2.),   // angled down-right
        (Up, 2) => Vec2::new(3., 13.),      // straight up
        (Up, 3) => Vec2::new(-31., 15.),    // angled up-left
        (Up, 1) => Vec2::new(5., 41.),      // angled up-right
        (Left, 4) => Vec2::new(-15., -2.),  // straight left
        (Left, 5) => Vec2::new(-17., -36.), // angled down-left
        (Left, 3) => Vec2::new(-42., -2.),  // angled up-left
        (Right, 0) => Vec2::new(15., -9.),  // straight right
        (Right, 1) => Vec2::new(17., 24.),  // angled up-right
        (Right, 7) => Vec2::new(42., -11.), // angled down-right
        _ => Vec2::ZERO,
    }
}

fn hitbox_offset_world(kind: LaserKind, rotation: f32) -> Vec2 {
    let local = match kind {
        LaserKind::Straight => HITBOX_OFFSET_STRAIGHT,
        LaserKind::Angled => HITBOX_OFFSET_ANGLED,
    };
    rotate_vec2(local, rotation)
}

/// Per-(mob facing, beam direction) hitbox nudge in world space (+x right, +y up).
/// Applied on top of [`hitbox_offset_world`]. `(Down, straight down)` needs no
/// extra entry — the rotated base offset is already correct there.
fn laser_hitbox_offset_correction(facing: WormFacing, beam_dir: Vec2) -> Vec2 {
    use WormFacing::{Down, Left, Right, Up};
    let octant = beam_octant(beam_dir);
    match (facing, octant) {
        (Left, 4) => Vec2::new(-13., -5.), // straight left
        (Left, 3) => Vec2::new(4., -13.),  // angled top left
        (Left, 5) => Vec2::new(-6., 8.),   // angled bot left
        (Up, 3) => Vec2::new(14., 3.),     // angled top left
        (Up, 2) => Vec2::new(0., 10.),     // straight up
        (Up, 1) => Vec2::new(-6., -5.),    // angled top right
        (Right, 0) => Vec2::new(11., -6.), // straight right
        (Right, 1) => Vec2::new(4., -22.), // angled top right
        (Right, 7) => Vec2::new(-3., 1.),  // angled bot right
        (Down, 7) => Vec2::new(-14., 11.), // angled bot right
        (Down, 5) => Vec2::new(5., 12.),   // angled bot left
        _ => Vec2::ZERO,
    }
}

fn beam_octant(beam_dir: Vec2) -> i32 {
    ((beam_dir.y.atan2(beam_dir.x).to_degrees() / 45.).round() as i32).rem_euclid(8)
}

fn rotate_vec2(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    Vec2::new(c * v.x - s * v.y, s * v.x + c * v.y)
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    } else if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d.abs()
}

fn worm_facing_toward_player(to_player: Vec2) -> WormFacing {
    let abs_x = to_player.x.abs();
    let abs_y = to_player.y.abs();
    if abs_x > abs_y * 1.1 {
        if to_player.x > 0. {
            WormFacing::Right
        } else {
            WormFacing::Left
        }
    } else if to_player.y > 0. {
        WormFacing::Up
    } else {
        WormFacing::Down
    }
}

/// Pick straight (cardinal) vs angled (~45°), and the world direction to aim.
fn pick_laser_aim(facing: WormFacing, target_angle: f32) -> (LaserKind, Vec2) {
    let straight = facing.world_dir();
    let straight_angle = straight.y.atan2(straight.x);
    let diff_straight = angle_diff(straight_angle, target_angle);

    let angled_ccw = rotate_vec2(straight, ANGLED_BEAM_OFFSET_RAD);
    let angled_cw = rotate_vec2(straight, -ANGLED_BEAM_OFFSET_RAD);
    let diff_ccw = angle_diff(angled_ccw.y.atan2(angled_ccw.x), target_angle);
    let diff_cw = angle_diff(angled_cw.y.atan2(angled_cw.x), target_angle);

    if diff_straight <= diff_ccw && diff_straight <= diff_cw {
        (LaserKind::Straight, straight)
    } else if diff_ccw <= diff_cw {
        (LaserKind::Angled, angled_ccw)
    } else {
        (LaserKind::Angled, angled_cw)
    }
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct VoidWormLaserState {
    pub target: Entity,
    pub laser_timer: Timer,
    pub walk_duration: f32,
    pub laser_entity: Option<Entity>,
    pub initialized: bool,
}

impl VoidWormLaserState {
    pub fn new(target: Entity, laser_duration: f32, walk_duration: f32) -> Self {
        Self {
            target,
            laser_timer: Timer::from_seconds(laser_duration, TimerMode::Once),
            walk_duration,
            laser_entity: None,
            initialized: false,
        }
    }
}

#[derive(Component)]
pub struct VoidWormLaserVisual {
    pub owner: Entity,
}

fn set_worm_tag(anim: &mut AsepriteAnimation, current: &mut CurrentAsepriteTag, tag: &str) {
    if current.0 != tag {
        *anim = AsepriteAnimation::from(tag);
        if anim.is_paused() {
            anim.play();
        }
        current.0 = tag.to_string();
    }
}

#[allow(clippy::type_complexity)]
pub fn void_worm_laser_attack(
    mut commands: Commands,
    time: Res<Time>,
    graphics: Res<Graphics>,
    player_query: Query<&GlobalTransform, With<Player>>,
    mut worms: Query<(
        Entity,
        &Attack,
        &FollowSpeed,
        &mut VoidWormLaserState,
        &mut Transform,
        &mut AsepriteAnimation,
        &mut CurrentAsepriteTag,
        Option<&FacingDirection>,
        Option<&MobStatusEffects>,
        Option<&DeathDefianceFrozen>,
    )>,
) {
    let player_pos = player_query
        .get_single()
        .ok()
        .map(|t| t.translation().truncate());

    for (
        entity,
        attack,
        follow_speed,
        mut state,
        mut transform,
        mut anim,
        mut current_tag,
        facing_option,
        status_option,
        defiance_frozen_option,
    ) in worms.iter_mut()
    {
        if defiance_frozen_option.is_some() || status_option.map(|s| s.is_frozen()).unwrap_or(false)
        {
            continue;
        }

        if !state.initialized {
            let worm_pos = transform.translation;
            let eye_world = worm_pos.truncate() + EYE_OFFSET;

            let to_player = player_pos
                .map(|p| p - eye_world)
                .filter(|v| v.length_squared() > f32::EPSILON)
                .unwrap_or(Vec2::NEG_Y);
            let to_player_norm = to_player.normalize_or_zero();
            let target_angle = to_player_norm.y.atan2(to_player_norm.x);

            let facing = worm_facing_toward_player(to_player_norm);
            let (kind, aim_dir) = pick_laser_aim(facing, target_angle);
            let spawn_cfg = build_laser_spawn(kind, aim_dir);

            set_worm_tag(&mut anim, &mut current_tag, facing.attack_tag());
            transform.scale.x = facing.worm_scale_x();
            commands
                .entity(entity)
                .insert(MobIsAttacking(Mob::VoidWorm));

            // eye = laser_center + center_to_eye  =>  laser_center = eye - center_to_eye
            let offset_correction = laser_offset_correction(facing, spawn_cfg.beam_dir);
            let beam_base_visual = eye_world + offset_correction;
            let beam_base_hitbox = eye_world
                + hitbox_offset_world(kind, spawn_cfg.rotation)
                + laser_hitbox_offset_correction(facing, spawn_cfg.beam_dir);
            let laser_translation =
                (beam_base_visual - spawn_cfg.center_to_eye).extend(worm_pos.z + 1.);

            let mut laser_anim = AsepriteAnimation::from(spawn_cfg.tag);
            // Start at frame 0 and ensure it's playing (matches the robust
            // one-time-aseprite spawn pattern used elsewhere).
            laser_anim.current_frame = 0;
            laser_anim.play();

            // Use the retained handle from Graphics so the asset + atlas stay
            // resident; loading on demand lets it unload between spawns and the
            // beam randomly fails to render or freezes.
            let laser_handle = graphics
                .void_laser_ase
                .clone()
                .unwrap_or_else(|| Handle::<bevy_aseprite::Aseprite>::default());

            let laser_duration = state.laser_timer.duration().as_secs_f32();
            let laser_entity = commands
                .spawn(AsepriteBundle {
                    aseprite: laser_handle,
                    animation: laser_anim,
                    transform: Transform {
                        translation: laser_translation,
                        rotation: Quat::from_rotation_z(spawn_cfg.rotation),
                        scale: Vec3::ONE,
                    },
                    ..Default::default()
                })
                .insert(VisibilityBundle::default())
                .insert(VoidWormLaserVisual { owner: entity })
                .insert(DespawnTimer(Timer::from_seconds(
                    laser_duration + 0.5,
                    TimerMode::Once,
                )))
                .id();
            state.laser_entity = Some(laser_entity);

            let beam_dir = spawn_cfg.beam_dir;
            let half = beam_dir * (LASER_DAMAGE_LENGTH / 2.0);
            let center = beam_base_hitbox + half;
            let hitbox = spawn_temp_collider(
                &mut commands,
                Transform::from_translation(center.extend(worm_pos.z)),
                laser_duration,
                attack.0,
                Collider::capsule(-half, half, LASER_DAMAGE_RADIUS),
                Projectile::VoidWormLaser,
            );
            commands.entity(hitbox).insert(EnemyProjectile {
                entity,
                mob: Mob::VoidWorm,
            });

            state.initialized = true;
            continue;
        }

        state.laser_timer.tick(time.delta());
        if state.laser_timer.finished() {
            if let Some(laser_entity) = state.laser_entity.take() {
                if let Some(e) = commands.get_entity(laser_entity) {
                    e.despawn_recursive();
                }
            }
            set_worm_tag(&mut anim, &mut current_tag, WALK_DOWN);
            commands
                .entity(entity)
                .remove::<VoidWormLaserState>()
                .remove::<MobIsAttacking>()
                .insert(FollowState {
                    target: state.target,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .insert(EnemyAttackCooldown(Timer::from_seconds(
                    state.walk_duration,
                    TimerMode::Once,
                )));
        }
    }
}

pub fn cleanup_orphan_void_lasers(
    mut commands: Commands,
    lasers: Query<(Entity, &VoidWormLaserVisual)>,
    worms: Query<&VoidWormLaserState>,
) {
    for (laser_entity, visual) in lasers.iter() {
        if worms.get(visual.owner).is_err() {
            if let Some(e) = commands.get_entity(laser_entity) {
                e.despawn_recursive();
            }
        }
    }
}
