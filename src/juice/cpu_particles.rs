use bevy::{prelude::*, render::view::RenderLayers};
use rand::Rng;

use crate::{
    player::Player, ui::FlashExpBarEvent, world::world_helpers::world_pos_to_ui_screen_pos,
};

#[derive(Component)]
pub struct CpuParticle {
    velocity: Vec2,
    acceleration: Vec2,
}

#[derive(Component)]
pub struct CpuParticleGenerator {
    pub min_particle_size: f32,
    pub max_particle_size: f32,
    pub min_particle_count: usize,
    pub max_particle_count: usize,
    pub pos_offset: Vec2,
    pub min_spawn_radius: f32,
    pub max_spawn_radius: f32,
    pub color: Color,
    pub lifetime: f32,
    pub particle_type: CpuParticleType,
}

pub enum CpuParticleType {
    Exp(u32, bool),
}

#[derive(Component)]
pub struct ExpParticle {
    pub delay: Timer,
    pub amount: u32,
    pub did_level_up_as_result: bool,
}

pub fn handle_generate_cpu_particles(
    generators: Query<(Entity, &CpuParticleGenerator, &GlobalTransform)>,
    mut commands: Commands,
    player_pos: Query<&GlobalTransform, With<Player>>,
) {
    let mut rng = rand::thread_rng();
    for (e, gen, gen_t) in generators.iter() {
        let count = rng.gen_range(gen.min_particle_count..gen.max_particle_count);
        for _ in 0..count {
            let parent_screen_pos = world_pos_to_ui_screen_pos(
                gen_t.translation().truncate(),
                player_pos.single().translation().truncate(),
            );
            let size = rng.gen_range(gen.min_particle_size..gen.max_particle_size);
            let rand_x_dist = rng.gen_range(gen.min_spawn_radius..gen.max_spawn_radius);
            let rand_y_dist = rng.gen_range(gen.min_spawn_radius..gen.max_spawn_radius);
            let rand_pos_offset_x = rng.gen_range(-rand_x_dist..rand_x_dist);
            let rand_pos_offset_y = rng.gen_range(-rand_y_dist..rand_y_dist);
            let p = commands
                .spawn(SpriteBundle {
                    sprite: Sprite {
                        color: gen.color,
                        custom_size: Some(Vec2::splat(size)),
                        ..default()
                    },
                    transform: Transform {
                        translation: parent_screen_pos.extend(0.)
                            + Vec3::new(
                                gen.pos_offset.x + rand_pos_offset_x,
                                gen.pos_offset.y + rand_pos_offset_y,
                                990.,
                            ),
                        scale: Vec3::splat(1.),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(RenderLayers::from_layers(&[3]))
                .insert(CpuParticle {
                    velocity: Vec2::ZERO,
                    acceleration: Vec2::ZERO,
                })
                .id();

            match gen.particle_type {
                CpuParticleType::Exp(amount, did_level_up_as_result) => {
                    commands.entity(p).insert(ExpParticle {
                        delay: Timer::from_seconds(0.25, TimerMode::Once),
                        amount,
                        did_level_up_as_result,
                    });
                }
            }
        }
        commands.entity(e).despawn();
    }
}

pub fn handle_move_exp_particles(
    mut particles: Query<(
        Entity,
        &mut CpuParticle,
        &mut ExpParticle,
        &GlobalTransform,
        &mut Transform,
    )>,
    mut commands: Commands,
    mut flash_event: EventWriter<FlashExpBarEvent>,
    time: Res<Time>,
) {
    // move exp particles to exp bar at the bottom centre of screen
    let target_pos = Vec2::new(0., -75.);
    for (e, mut p, mut exp_state, g_txfm, mut txfm) in particles.iter_mut() {
        exp_state.delay.tick(time.delta());
        if exp_state.delay.finished() {
            let delta_dist = target_pos - g_txfm.translation().truncate();
            p.acceleration = delta_dist.normalize();
            let acc = p.acceleration;
            p.velocity += acc;
            p.velocity = p.velocity.clamp_length_max(3.);
        } else {
            p.velocity = Vec2::new(0., 0.1);
        }
        txfm.translation += p.velocity.extend(0.);
        if g_txfm.translation().truncate().distance(target_pos) <= 3. {
            commands.entity(e).despawn();
            flash_event.send(FlashExpBarEvent {
                amount: exp_state.amount,
                did_level: exp_state.did_level_up_as_result,
            });
        }
    }
}
