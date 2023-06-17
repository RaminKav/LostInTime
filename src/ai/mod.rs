mod enemy_hostile_basic;

use crate::{CoreGameSet, Plugin};

use bevy::prelude::*;
pub use enemy_hostile_basic::*;
use seldom_state::StateMachinePlugin;

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(StateMachinePlugin).add_systems(
            (follow, attack, idle)
                .in_set(CoreGameSet::Main)
                .in_schedule(CoreSchedule::FixedUpdate),
        );
    }
}
