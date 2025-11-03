pub mod pet_abilities;
pub mod pet_animation;
pub mod pet_spawner;
pub mod state;
use bevy::prelude::*;
pub use pet_abilities::*;
pub use pet_animation::*;
pub use pet_spawner::*;
pub use state::*;

use crate::{client::is_not_paused, GameState};

pub struct PetsPlugin;

impl Plugin for PetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<UpdatePetWeaponEvent>()
            .add_systems(
                (
                    find_new_target.run_if(is_not_paused),
                    handle_inv_change_pet_wep_update,
                    use_weapon.run_if(is_not_paused),
                    test_spawn_pet.run_if(is_not_paused),
                    // New pet animation systems
                    handle_new_pet_state_machine,
                    handle_pet_idle_state.run_if(is_not_paused),
                    handle_pet_follow_state.run_if(is_not_paused),
                    // Pet configuration system
                    update_pet_weapon_on_inv_change,
                    // Pet abilities
                    slime_shield_ability.run_if(is_not_paused),
                    fairy_heal_ability.run_if(is_not_paused),
                    handle_pet_spawner_interaction,
                )
                    .in_set(OnUpdate(GameState::Main)),
            )
            .add_system(
                configure_pet_on_spawn
                    .run_if(in_state(GameState::Main))
                    .in_base_set(CoreSet::PostUpdate),
            );
    }
}
