use bevy::prelude::*;

use crate::{    
    audio::{AudioSoundEffect, SoundSpawner},
    GameState
};

#[derive(Component, Default, Debug)]
pub struct TimeFragmentCurrency {
    pub time_fragments: i32,
    pub total_collected_time_fragments_all_time: u128,
    pub total_collected_time_fragments_this_run: i32,
    pub bounce_timer: Timer,
}
impl TimeFragmentCurrency {
    pub fn new(amount: i32, total_this_run: i32, total_all_time: u128) -> Self {
        TimeFragmentCurrency {
            time_fragments: amount,
            total_collected_time_fragments_all_time: total_all_time,
            total_collected_time_fragments_this_run: total_this_run,
            bounce_timer: Timer::from_seconds(0.5, TimerMode::Once),
        }
    }
}

pub struct ModifyTimeFragmentsEvent {
    pub delta: i32,
}

pub fn handle_modify_time_fragments(
    mut time_fragments: Query<&mut TimeFragmentCurrency>,
    mut events: EventReader<ModifyTimeFragmentsEvent>,
    state: Res<State<GameState>>,
    mut commands: Commands,
) {
    let Ok(mut time_fragments) = time_fragments.get_single_mut() else {
        return;
    };
    for event in events.iter() {
        time_fragments.time_fragments = (time_fragments.time_fragments + event.delta).max(0);

        if event.delta > 0 {
            if state.0 == GameState::Main {
                time_fragments.total_collected_time_fragments_this_run += event.delta;
            } else if state.0 == GameState::GameOver {
                time_fragments.total_collected_time_fragments_all_time += event.delta as u128;
            }
            commands.spawn(SoundSpawner::new(AudioSoundEffect::CurrencyPickup, 0.75));
        }
    }
}
