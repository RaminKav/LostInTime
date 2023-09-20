use bevy::prelude::*;
use bevy_hanabi::HanabiPlugin;
mod particles;
use crate::{combat::handle_hits, GameState};
pub use particles::*;

pub struct JuicePlugin;

impl Plugin for JuicePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Particles::default())
            .add_event::<UseItemEvent>()
            .add_plugin(HanabiPlugin)
            .add_systems(
                (
                    update_dust_particle_dir,
                    setup_particles,
                    spawn_use_item_particles,
                    spawn_obj_hit_particles.after(handle_hits),
                    cleanup_object_particles,
                    spawn_enemy_death_particles,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}
