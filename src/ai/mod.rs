mod enemy_basic;

use crate::{GameState, Plugin, TIME_STEP};

use bevy::{
    prelude::{App, SystemSet},
    time::FixedTimestep,
};
pub use enemy_basic::*;
use seldom_state::{prelude::TriggerPlugin, StateMachinePlugin};

pub struct AIPlugin;

impl Plugin for AIPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(StateMachinePlugin)
            .add_plugin(TriggerPlugin::<LineOfSight>::default())
            .add_plugin(TriggerPlugin::<AttackDistance>::default())
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(follow)
                    .with_system(attack)
                    .with_system(idle),
            );
    }
}
