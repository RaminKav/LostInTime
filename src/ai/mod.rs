mod enemy_hostile_basic;

use crate::{
    enemy::red_mushling::{gas_attack, sprout},
    GameState, Plugin,
};

use bevy::prelude::*;
pub use enemy_hostile_basic::*;
use seldom_state::StateMachinePlugin;

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(StateMachinePlugin).add_systems(
            (
                follow,
                leap_attack,
                gas_attack,
                sprout,
                projectile_attack,
                tick_enemy_attack_cooldowns,
                idle,
            )
                .in_set(OnUpdate(GameState::Main)),
        );
    }
}
