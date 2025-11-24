use bevy::prelude::*;
use serde::{Deserialize, Serialize};

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
            .add_system(initialize_chaos_from_era.in_schedule(OnEnter(crate::GameState::Main)));
    }
}

/// Initialize chaos from the starting era when entering Main game state for the first time
/// This only runs if chaos_level is 0.0 (fresh start, not a loaded save)
fn initialize_chaos_from_era(
    mut chaos_tracker: ResMut<ChaosTracker>,
    era_manager: Res<crate::world::dimension::EraManager>,
) {
    let era_chaos = era_manager.current_era.get_chaos_modifier();
    
    // Initialize era chaos if tracker is empty (fresh start, not a loaded save)
    // If chaos_level > 0, it means we loaded from a save and chaos is already correct
    if chaos_tracker.chaos_level == 0.0 && era_chaos > 0.0 {
        chaos_tracker.add_chaos(era_chaos);
    }
}

fn handle_increase_chaos_event(
    mut events: EventReader<IncreaseChaosEvent>,
    mut chaos_tracker: ResMut<ChaosTracker>,
) {
    for event in events.iter() {
        chaos_tracker.add_chaos(event.amount);
    }
}
