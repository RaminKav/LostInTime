use bevy::prelude::*;
use noise::{NoiseFn, Perlin};
use rand::Rng;

use crate::{TextureCamera, DEBUG_MODE};

#[derive(Component)]
pub struct ShakeEffect {
    pub timer: Timer,
    pub speed: f32,
    pub seed: u32,
    pub max_mag: f32,
    pub noise: f32,
    pub dir: Vec2,
}

pub fn shake_effect(
    mut commands: Commands,
    mut shakers: Query<(Entity, &mut Transform, &mut ShakeEffect)>,
    time: Res<Time>,
) {
    for (e, mut t, mut shake) in shakers.iter_mut() {
        shake.timer.tick(time.delta());
        if !shake.timer.finished() {
            let sin = f32::sin(shake.speed * time.delta_seconds());
            // var direction = _Direction + Get2DNoise(_Seed) * _NoiseMagnitude;
            let perlin = Perlin::new(shake.seed);

            let dir = shake.dir
                + perlin.get([time.delta_seconds_f64(), time.delta_seconds_f64()]) as f32
                    * shake.noise;
            let dir = dir.normalize();
            let extend = (dir * sin * shake.max_mag * shake.timer.percent_left()).extend(0.);
            t.translation += extend;
        } else {
            commands.entity(e).remove::<ShakeEffect>();
        }
    }
}

pub fn test_shake(
    mut game_camera: Query<Entity, With<TextureCamera>>,
    keys: Res<Input<KeyCode>>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::G) && *DEBUG_MODE {
        let mut rng = rand::thread_rng();
        let seed = rng.gen_range(0..100000);
        let speed = 10.;
        let max_mag = 120.;
        let noise = 0.5;
        let dir = Vec2::new(1., 1.);
        for e in game_camera.iter_mut() {
            commands.entity(e).insert(ShakeEffect {
                timer: Timer::from_seconds(1.5, TimerMode::Once),
                speed,
                seed,
                max_mag,
                noise,
                dir,
            });
        }
    }
}
