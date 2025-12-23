use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::rngs::ThreadRng;
use rand::Rng;

use super::{CHUNK_SIZE, TILE_SIZE};

/// Cached Perlin noise generators to avoid recreating them for every tile.
/// Creating Perlin generators is expensive - this saves ~888,000 allocations during world init.
pub struct CachedNoiseGenerators {
    pub n1: Perlin,
    pub n2: Perlin,
    pub n3: Perlin,
}

impl CachedNoiseGenerators {
    pub fn new(seed: u64) -> Self {
        let seed = seed as u32;
        Self {
            n1: Perlin::new(1 + seed),
            n2: Perlin::new(2 + seed),
            n3: Perlin::new(3 + seed),
        }
    }
}

/// Get perlin noise value for a tile position using cached generators.
/// This is the optimized version that reuses pre-created Perlin generators.
pub fn get_perlin_noise_for_tile_cached(x: f64, y: f64, generators: &CachedNoiseGenerators) -> f64 {
    // base_oct controls the "wavelength" of noise features
    // 1/50 = wavelength of ~50 tiles, giving ~2 full waves across a 96-tile island
    let base_oct = 1. / 50.;

    // e1: large features (ponds, terrain regions) - wavelength ~50 tiles
    let e1 = (generators.n1.get([x * base_oct, y * base_oct]) + 1.) / 2.;
    // e2: medium features - wavelength ~12 tiles
    let e2 = (generators.n2.get([x * base_oct * 4., y * base_oct * 4.]) + 1.) / 2.;
    // e3: small details - wavelength ~6 tiles
    let e3 = (generators.n3.get([x * base_oct * 8., y * base_oct * 8.]) + 1.) / 2.;

    // Use e1 (large features) as base, but blend with min(e2, e3) to create larger water regions
    // This preserves low values (for water) while making them more cohesive
    // e1 dominates for large-scale structure, but if e2 or e3 is low, it pulls the result down
    let detail_min = f64::min(e2, e3);
    // Use min() to ensure low values create water, but e1 creates larger regions
    // This creates larger ponds: if e1 is low in a region, that entire region becomes water
    let result = f64::min(e1, detail_min + 0.15);
    result.clamp(0., 1.)
}

/// Legacy function that creates generators per-call (slow, for backwards compatibility).
/// Kept for use in fallback cases where generators aren't available.
#[allow(dead_code)]
pub fn get_perlin_noise_for_tile(x: f64, y: f64, seed: u64) -> f64 {
    let generators = CachedNoiseGenerators::new(seed);
    get_perlin_noise_for_tile_cached(x, y, &generators)
}

pub fn _poisson_disk_sampling(
    radius: f32,
    k: i8,
    f: f32,
    max_radial_distance: f32,
    max_points: usize,
    start_pos: Vec2,
    mut rng: ThreadRng,
) -> Vec<(f32, f32)> {
    if f <= 0. || max_points == 0 {
        return vec![];
    }
    // TODO: fix this to work w 4 quadrants -/+
    let n = 2.;
    // the final set of points to return
    let mut points: Vec<(f32, f32)> = vec![];
    if k == 0 {
        return points;
    }
    // the currently "Active" set of points
    let mut active: Vec<(f32, f32)> = vec![];
    let p0 = (start_pos.x * TILE_SIZE.x, start_pos.y * TILE_SIZE.y);

    let cell_size = f32::floor(radius / f32::sqrt(n));
    let num_cell: usize = max_radial_distance as usize * 2;
    let mut num_points = 1;
    let mut grid: Vec<Vec<Option<(f32, f32)>>> = vec![vec![None; num_cell]; num_cell];

    let insert_point = |g: &mut Vec<Vec<Option<(f32, f32)>>>, p: (f32, f32)| {
        let xi: usize = f32::floor(p.0 / cell_size) as usize;
        let yi: usize = f32::floor(p.1 / cell_size) as usize;
        g[xi][yi] = Some(p);
    };

    let is_valid_point = move |g: &Vec<Vec<Option<(f32, f32)>>>, p: (f32, f32)| -> bool {
        // make sure p is in the chunk
        let dist_from_orgin = Vec2::new(p.0, p.1).distance(Vec2::new(p0.0, p0.1));
        if dist_from_orgin >= max_radial_distance {
            return false;
        }
        if p.0 < 0. || p.0 > num_cell as f32 || p.1 < 0. || p.1 > num_cell as f32 {
            return false;
        }

        // check neighboring eight cells
        let xi: f32 = f32::floor(p.0 / cell_size);
        let yi: f32 = f32::floor(p.1 / cell_size);
        let i0 = usize::max((xi - 1.) as usize, 0);
        let i1 = usize::min((xi + 1.) as usize, num_cell - 1. as usize);
        let j0 = usize::max((yi - 1.) as usize, 0);
        let j1 = usize::min((yi + 1.) as usize, num_cell - 1. as usize);

        for i in i0..=i1 {
            for j in j0..=j1 {
                if let Some(sample_point) = g[i][j] {
                    let sample_point_vec = Vec2::new(sample_point.0, sample_point.1);
                    let p_vec = Vec2::new(p.0, p.1);
                    if sample_point_vec.distance(p_vec) < radius {
                        return false;
                    }
                }
            }
        }
        true
    };

    insert_point(&mut grid, p0);
    num_points += 1;
    let success = rng.gen::<f32>() < f;

    if success {
        points.push(p0);
    }

    active.push(p0);
    while !active.is_empty() && num_points < max_points {
        let i = rng.gen_range(0..=(active.len() - 1));
        let p = active.get(i).unwrap();
        let mut found = false;

        for _ in 0..k {
            // get a random angle
            let theta: f64 = rng.gen_range(0. ..360.);
            let new_r = rng.gen_range(radius..(2. * radius));

            // create new point from randodm angle r distance away from p
            let new_px = p.0 + new_r * theta.to_radians().cos() as f32;
            let new_py = p.1 + new_r * theta.to_radians().sin() as f32;
            let new_p = (new_px, new_py);

            if !is_valid_point(&grid, new_p) {
                continue;
            }

            //add the new point to our lists and break
            let success = rng.gen::<f32>() < f;

            if success {
                points.push(new_p);
            }
            insert_point(&mut grid, new_p);
            num_points += 1;
            active.push(new_p);
            found = true;
            break;
        }

        if !found {
            active.remove(i);
        }
    }

    points
}

pub fn get_object_points_for_chunk(_seed: u64, f: f64) -> Vec<(f32, f32)> {
    //TODO: figure out seeded rng
    // let mut rng = XorShiftRng::seed_from_u64(seed);
    let mut rng = rand::thread_rng();
    let mut points = vec![];
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            let r = rng.gen::<f64>();
            if r < f {
                points.push((x as f32 * TILE_SIZE.x, y as f32 * TILE_SIZE.y));
            }
        }
    }
    points
}
