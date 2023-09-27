use bevy::prelude::*;
use bevy_hanabi::HanabiPlugin;
pub mod bounce;
mod particles;
use crate::{combat::handle_hits, GameState};
pub use particles::*;

use self::bounce::bounce_on_hit;
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
                    bounce_on_hit,
                    spawn_use_item_particles,
                    spawn_obj_hit_particles.after(handle_hits),
                    cleanup_object_particles,
                    spawn_enemy_death_particles,
                    spawn_obj_death_particles,
                )
                    .in_set(OnUpdate(GameState::Main)),
            );
    }
}
