use bevy::prelude::*;
use bevy::utils::HashMap;

use crate::enemy::Mob;

/// Size (world px) of one spatial-hash cell. Should be ~the separation radius so
/// a 3x3 cell scan covers every neighbor that can repel an enemy.
pub const STEERING_CELL_SIZE: f32 = 24.0;

/// Enemies closer than this (world px) push each other apart.
pub const SEPARATION_RADIUS: f32 = 22.0;

/// Weight of the "seek the player" vector when blending the final move direction.
pub const SEEK_WEIGHT: f32 = 1.0;

/// Weight of the neighbor-repulsion vector. Higher = enemies spread out more.
pub const SEPARATION_WEIGHT: f32 = 0.65;

/// Max arc/flank angle (radians) applied to the seek vector at long range.
/// Enemies curve in from the sides and straighten as they approach.
pub const MAX_ARC_RADIANS: f32 = 0.55;

/// Distance (world px) at/above which the full arc is applied. Inside this range
/// the arc fades linearly to 0 so enemies hone straight in for the kill.
pub const ARC_FALLOFF_DIST: f32 = 140.0;

/// Distance (world px) below which separation fades out, so crowding never
/// prevents an enemy from closing into attack range.
pub const SEPARATION_FADE_DIST: f32 = 18.0;

/// How quickly the steered direction tracks toward its new value each frame
/// (0..1). Lower = smoother but laggier turning.
pub const STEERING_SMOOTHING: f32 = 0.35;

/// Spatial hash of all mob positions, rebuilt each frame before the follow
/// systems run. Enables O(n) neighbor lookups for separation steering.
#[derive(Resource, Default)]
pub struct EnemySpatialGrid {
    cells: HashMap<IVec2, Vec<(Entity, Vec2)>>,
}

impl EnemySpatialGrid {
    fn cell_of(pos: Vec2) -> IVec2 {
        IVec2::new(
            (pos.x / STEERING_CELL_SIZE).floor() as i32,
            (pos.y / STEERING_CELL_SIZE).floor() as i32,
        )
    }

    /// Accumulated normalized repulsion vector from neighbors within
    /// `SEPARATION_RADIUS` of `pos` (excluding `me`).
    pub fn separation_force(&self, me: Entity, pos: Vec2) -> Vec2 {
        let base = Self::cell_of(pos);
        let mut force = Vec2::ZERO;
        let radius_sq = SEPARATION_RADIUS * SEPARATION_RADIUS;
        for dy in -1..=1 {
            for dx in -1..=1 {
                let Some(bucket) = self.cells.get(&(base + IVec2::new(dx, dy))) else {
                    continue;
                };
                for (other, other_pos) in bucket.iter() {
                    if *other == me {
                        continue;
                    }
                    let away = pos - *other_pos;
                    let d_sq = away.length_squared();
                    if d_sq > 0.0001 && d_sq < radius_sq {
                        // Closer neighbors push harder (inverse distance).
                        force += away.normalize_or_zero() / d_sq.sqrt();
                    }
                }
            }
        }
        force.normalize_or_zero()
    }
}

/// Rebuilds the spatial grid from all mob transforms. Runs before follow systems.
pub fn build_enemy_spatial_grid(
    mut grid: ResMut<EnemySpatialGrid>,
    mobs: Query<(Entity, &Transform), With<Mob>>,
) {
    grid.cells.clear();
    for (entity, transform) in mobs.iter() {
        let pos = transform.translation.truncate();
        grid.cells
            .entry(EnemySpatialGrid::cell_of(pos))
            .or_default()
            .push((entity, pos));
    }
}

/// A stable per-entity flank sign (-1 or +1) so each enemy consistently arcs
/// in from one side rather than jittering.
pub fn flank_sign(entity: Entity) -> f32 {
    if entity.index() % 2 == 0 {
        1.0
    } else {
        -1.0
    }
}

/// Blends the direct seek direction with a flank arc and neighbor separation,
/// returning the final (normalized) move direction.
///
/// - `seek_dir`: normalized direction from enemy toward the player.
/// - `pos`: enemy world position.
/// - `dist`: distance from enemy to the player (world px).
pub fn steer_chase(
    grid: &EnemySpatialGrid,
    entity: Entity,
    pos: Vec2,
    seek_dir: Vec2,
    dist: f32,
) -> Vec2 {
    // Arc: full angle at long range, fading to 0 within ARC_FALLOFF_DIST.
    let arc_t = (dist / ARC_FALLOFF_DIST).clamp(0.0, 1.0);
    let angle = MAX_ARC_RADIANS * arc_t * flank_sign(entity);
    let (sin, cos) = angle.sin_cos();
    let arced = Vec2::new(
        seek_dir.x * cos - seek_dir.y * sin,
        seek_dir.x * sin + seek_dir.y * cos,
    );

    // Separation: fade out as the enemy closes into attack range.
    let sep_t = (dist / SEPARATION_FADE_DIST).clamp(0.0, 1.0);
    let separation = grid.separation_force(entity, pos) * sep_t;

    (arced * SEEK_WEIGHT + separation * SEPARATION_WEIGHT).normalize_or_zero()
}
