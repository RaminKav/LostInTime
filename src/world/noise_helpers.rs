use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::rngs::ThreadRng;
use rand::Rng;

use super::{CHUNK_SIZE, TILE_SIZE};

pub fn get_perlin_noise_for_tile(x: f64, y: f64, seed: u32) -> f64 {
    let n1 = Perlin::new(1 + seed);
    let n2 = Perlin::new(2 + seed);
    let n3 = Perlin::new(3 + seed);
    // let n1 = Fbm::<Perlin>::new(seed)
    //     .set_octaves(2)
    //     .set_frequency(1.)
    //     .set_lacunarity(2.0)
    //     .set_persistence(0.01);
    // let n2 = Fbm::<Perlin>::new(1 + seed)
    //     .set_octaves(2)
    //     .set_frequency(1. / 2.)
    //     .set_lacunarity(4.0)
    //     .set_persistence(0.01);
    // let n3 = Fbm::<Perlin>::new(2 + seed)
    //     .set_octaves(2)
    //     .set_frequency(1. / 4.)
    //     .set_lacunarity(8.0)
    //     .set_persistence(0.01);
    // .set_persistence(1.);
    // let _noise_m = Simplex::new(4 + seed);
    // let _noise_m2 = Simplex::new(5 + seed);
    // let _noise_m3 = Simplex::new(6 + seed);

    let base_oct = 1. / 10. / 8.;

    let e1 = (n1.get([x * base_oct, y * base_oct]) + 1.) / 2.;
    let e2 = (n2.get([x * base_oct * 8., y * base_oct * 8.]) + 1.) / 2.;
    let e3 = (n3.get([x * base_oct * 16., y * base_oct * 16.]) + 1.) / 2.;

    let e = (f64::min(e1, f64::min(e2, e3) + 0.1)).clamp(0., 1.);
    e
}

pub fn poisson_disk_sampling(r: f64, k: i8, f: f64, mut rng: ThreadRng) -> Vec<(f32, f32)> {
    // TODO: fix this to work w 4 quadrants -/+
    let n = 2.;
    let chunk_pixel_size = CHUNK_SIZE as i32 * TILE_SIZE.x as i32;
    // the final set of points to return
    let mut points: Vec<(f32, f32)> = vec![];
    if k == 0 {
        return points;
    }
    // the currently "Active" set of points
    let mut active: Vec<(f32, f32)> = vec![];
    let p0 = (
        rng.gen_range(0..chunk_pixel_size) as f32,
        rng.gen_range(0..chunk_pixel_size) as f32,
    );

    let cell_size = f64::floor(r / f64::sqrt(n));
    let num_cell: usize =
        (f64::ceil(CHUNK_SIZE as f64 * TILE_SIZE.x as f64 / cell_size) + 1.) as usize;
    let mut grid: Vec<Vec<Option<(f32, f32)>>> = vec![vec![None; num_cell]; num_cell];

    let insert_point = |g: &mut Vec<Vec<Option<(f32, f32)>>>, p: (f32, f32)| {
        let xi: usize = f64::floor(p.0 as f64 / cell_size) as usize;
        let yi: usize = f64::floor(p.1 as f64 / cell_size) as usize;
        g[xi][yi] = Some(p);
    };

    let is_valid_point = move |g: &Vec<Vec<Option<(f32, f32)>>>, p: (f32, f32)| -> bool {
        // make sure p is in the chunk
        if p.0 < 0. || p.0 > chunk_pixel_size as f32 || p.1 < 0. || p.1 > chunk_pixel_size as f32 {
            return false;
        }

        // check neighboring eight cells
        let xi: f64 = f64::floor(p.0 as f64 / cell_size);
        let yi: f64 = f64::floor(p.1 as f64 / cell_size);
        let i0 = usize::max((xi - 1.) as usize, 0);
        let i1 = usize::min((xi + 1.) as usize, num_cell - 1. as usize);
        let j0 = usize::max((yi - 1.) as usize, 0);
        let j1 = usize::min((yi + 1.) as usize, num_cell - 1. as usize);

        for i in i0..=i1 {
            for j in j0..=j1 {
                if let Some(sample_point) = g[i][j] {
                    let sample_point_vec = Vec2::new(sample_point.0, sample_point.1);
                    let p_vec = Vec2::new(p.0, p.1);
                    if sample_point_vec.distance(p_vec) < r as f32 {
                        return false;
                    }
                }
            }
        }
        true
    };

    insert_point(&mut grid, p0);
    points.push(p0);
    active.push(p0);
    while !active.is_empty() {
        let i = rng.gen_range(0..=(active.len() - 1));
        let p = active.get(i).unwrap();
        let mut found = false;

        for _ in 0..k {
            // get a random angle
            let theta: f64 = rng.gen_range(0. ..360.);
            let new_r = rng.gen_range(r..(2. * r));

            // create new point from randodm angle r distance away from p
            let new_px = p.0 as f64 + new_r * theta.to_radians().cos();
            let new_py = p.1 as f64 + new_r * theta.to_radians().sin();
            let new_p = (new_px as f32, new_py as f32);

            if !is_valid_point(&grid, new_p) {
                continue;
            }

            //add the new point to our lists and break
            let success = rng.gen_ratio((f * 100.) as u32, 100);
            if success {
                points.push(new_p);
            }
            insert_point(&mut grid, new_p);
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
