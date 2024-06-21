use bevy::{prelude::*, transform::TransformSystem};
use bevy_hanabi::HanabiPlugin;

pub mod bounce;
mod particles;
mod screen_flash;
mod screen_shake;
use crate::{combat::handle_hits, inputs::move_camera_with_player, GameState};
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
                    bounce_on_hit,
                    spawn_use_item_particles,
                    spawn_obj_hit_particles.after(handle_hits),
                    cleanup_object_particles,
                    spawn_enemy_death_particles,
                    spawn_obj_death_particles,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                shake_effect
                    .after(move_camera_with_player)
                    .before(TransformSystem::TransformPropagate)
                    .in_base_set(CoreSet::PostUpdate)
                    .run_if(in_state(GameState::Main)),
            );
    }
}
