use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::run_once_per_run;

#[derive(Default, Reflect, Resource, Clone, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct ChaosTracker {
    pub chaos_level: f32,
}

impl ChaosTracker {
    pub fn add_chaos(&mut self, amount: f32) {
        self.chaos_level += amount;
        info!("Chaos increased to: {}", self.chaos_level);
    }

    pub fn get_chaos(&self) -> f32 {
        self.chaos_level
    }
}

pub struct IncreaseChaosEvent {
    pub amount: f32,
}

pub struct ChaosPlugin;

impl Plugin for ChaosPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<ChaosTracker>()
            .init_resource::<ChaosTracker>()
            .add_event::<IncreaseChaosEvent>()
            .add_system(handle_increase_chaos_event)
            .add_system(
                initialize_chaos_from_era
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(crate::GameState::Main)),
            );
    }
}

/// Initialize chaos from the starting era when entering Main game state
/// Resets chaos to 0 and then adds the base chaos for the current era
fn initialize_chaos_from_era(
    mut chaos_tracker: ResMut<ChaosTracker>,
    era_manager: Res<crate::world::dimension::EraManager>,
) {
    // Reset chaos to 0 at the start of a run, then add era's base chaos
    // This ensures chaos doesn't carry over from previous runs
    info!("Resetting chaos from {} to 0", chaos_tracker.chaos_level);
    chaos_tracker.chaos_level = 0.0;
}

fn handle_increase_chaos_event(
    mut events: EventReader<IncreaseChaosEvent>,
    mut chaos_tracker: ResMut<ChaosTracker>,
) {
    for event in events.iter() {
        chaos_tracker.add_chaos(event.amount);
    }
}
