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
            .add_system(handle_increase_chaos_event);
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
