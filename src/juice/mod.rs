use bevy::{prelude::*, transform::TransformSystem};
use bevy_hanabi::HanabiPlugin;

pub mod bounce;
mod cpu_particles;
mod particles;
mod screen_flash;
mod screen_shake;
use crate::{
    combat::handle_hits, inputs::move_camera_with_player, item::handle_break_object, CustomFlush,
    GameState,
};
pub use cpu_particles::*;
pub use particles::*;
pub use screen_flash::*;
pub use screen_shake::*;

use self::bounce::bounce_on_hit;
pub struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Particles::default())
            .add_event::<UseItemEvent>()
            .add_plugin(HanabiPlugin)
            .add_systems(
                (
                    test_flash,
                    screen_flash_effect.run_if(resource_exists::<FlashEffect>()),
                    test_shake,
                    update_dust_particle_dir,
                    setup_particles,
                    handle_exp_particles,
                    bounce_on_hit,
                    spawn_use_item_particles,
                    spawn_obj_hit_particles.after(handle_hits),
                    cleanup_object_particles,
                    spawn_enemy_death_particles,
                    handle_generate_cpu_particles,
                    handle_move_exp_particles,
                    spawn_obj_death_particles
                        .before(CustomFlush)
                        .before(handle_break_object),
                )
                    .in_set(Update(GameState::Main)),
            )
            .add_system(
                PostUpdate,
                shake_effect
                    .after(move_camera_with_player)
                    .before(TransformSystem::TransformPropagate)
                    .run_if(in_state(GameState::Main)),
            );
    }
}
