use bevy::prelude::*;

use crate::blessings::PendingRunStartChaos;
use bevy::utils::HashMap;
use serde::{Deserialize, Serialize};

use crate::{enemy::Mob, run_once_per_run};

#[derive(Default, Reflect, Resource, Clone, Debug, Serialize, Deserialize)]
#[reflect(Resource)]
pub struct ChaosTracker {
    pub chaos_level: f32,
}

/// Resource to track era transition state for mob unlocking
#[derive(Resource, Debug)]
pub struct EraTransitionState {
    /// Timers for when new mobs become available in this era
    /// Key: Mob type, Value: Timer that must finish before the mob can spawn
    pub mob_unlock_timers: HashMap<Mob, Timer>,
}

impl Default for EraTransitionState {
    fn default() -> Self {
        Self {
            mob_unlock_timers: HashMap::default(),
        }
    }
}

impl EraTransitionState {
    pub fn is_mob_unlocked(&self, mob: &Mob) -> bool {
        if let Some(timer) = self.mob_unlock_timers.get(mob) {
            timer.finished()
        } else {
            true
        }
    }
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

/// Max HP multiplier from total chaos (global chaos tracker + infinite mode bonus),
/// matching `juice_up_spawned_mobs_per_day` in `crate::enemy`.
pub fn hp_multiplier_for_total_chaos(total_chaos: f32) -> f32 {
    let chaos_factor = 1. + total_chaos;
    let early_cutoff = 20.0_f32;
    if chaos_factor <= early_cutoff {
        chaos_factor.powf(0.7)
    } else {
        1.1 * chaos_factor
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
            .init_resource::<EraTransitionState>()
            .add_event::<IncreaseChaosEvent>()
            .add_system(handle_increase_chaos_event)
            .add_system(
                initialize_chaos_from_era
                    .run_if(run_once_per_run())
                    .in_schedule(OnEnter(crate::GameState::Main)),
            )
            .add_system(update_mob_unlock_timers.in_set(OnUpdate(crate::GameState::Main)));
    }
}

/// Initialize chaos from the starting era when entering Main game state
/// Resets chaos to 0 and then adds the base chaos for the current era
fn initialize_chaos_from_era(
    mut chaos_tracker: ResMut<ChaosTracker>,
    pending_run_start_chaos: Option<Res<PendingRunStartChaos>>,
    mut commands: Commands,
) {
    info!("Resetting chaos from {} to 0", chaos_tracker.chaos_level);
    chaos_tracker.chaos_level = 0.0;

    if let Some(pending) = pending_run_start_chaos {
        if pending.amount > 0.0 {
            chaos_tracker.add_chaos(pending.amount);
        }
        commands.remove_resource::<PendingRunStartChaos>();
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

/// Tick mob unlock timers
fn update_mob_unlock_timers(time: Res<Time>, mut transition_state: ResMut<EraTransitionState>) {
    for timer in transition_state.mob_unlock_timers.values_mut() {
        timer.tick(time.delta());
    }
}
