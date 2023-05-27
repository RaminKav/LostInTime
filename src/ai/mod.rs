mod enemy_basic;

use crate::{GameState, Plugin};

use bevy::prelude::{App, IntoSystemConfigs, OnUpdate};
pub use enemy_basic::*;
use seldom_state::StateMachinePlugin;

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(StateMachinePlugin)
            .add_systems((follow, attack, idle).in_set(OnUpdate(GameState::Main)));
    }
}
