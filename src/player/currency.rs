use bevy::prelude::*;
use rand::Rng;

use crate::{
    audio::{AudioSoundEffect, SoundSpawner},
    combat::EnemyDeathEvent,
    item::WorldObject,
    GameState,
};

#[derive(Resource, Debug)]
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

    pub fn can_spend(&self, amount: u32) -> bool {
        self.time_fragments as i64 >= amount as i64
    }

    pub fn spend(&mut self, amount: u32) -> bool {
        if self.can_spend(amount) {
            self.time_fragments -= amount as i32;
            true
        } else {
            false
        }
    }
}

impl Default for TimeFragmentCurrency {
    fn default() -> Self {
        TimeFragmentCurrency::new(0, 0, 0)
    }
}

#[derive(Resource, Default, Debug)]
pub struct CoinCurrency {
    pub coins: u32,
    pub bounce_timer: Timer,
}

pub struct ModifyCurencyEvent {
    pub delta: i32,
    pub obj: WorldObject,
}

pub fn handle_modify_currency(
    mut time_fragments: ResMut<TimeFragmentCurrency>,
    mut events: EventReader<ModifyCurencyEvent>,
    state: Res<State<GameState>>,
    mut commands: Commands,
    mut coins: ResMut<CoinCurrency>,
) {
    for event in events.iter() {
        if event.obj == WorldObject::TimeFragment {
            time_fragments.time_fragments = (time_fragments.time_fragments + event.delta).max(0);
        } else if event.obj == WorldObject::Coin {
            coins.coins = coins.coins.saturating_add(event.delta as u32);
        }

        if event.delta > 0 {
            if event.obj == WorldObject::TimeFragment {
                if state.0 == GameState::Main {
                    time_fragments.total_collected_time_fragments_this_run += event.delta;
                } else if state.0 == GameState::GameOver {
                    time_fragments.total_collected_time_fragments_all_time += event.delta as u128;
                }
            }

            commands.spawn(SoundSpawner::new(AudioSoundEffect::CurrencyPickup, 0.75));
        }
    }
}

pub fn handle_mob_death_out_of_run_currency(
    mut death_events: EventReader<EnemyDeathEvent>,
    mut currency: ResMut<TimeFragmentCurrency>,
) {
    for _ in death_events.iter() {
        if rand::thread_rng().gen_bool(0.02) {
            info!("CURRENCY GAINED!");
            currency.time_fragments = currency.time_fragments.saturating_add(1);
        }
    }
}

pub fn reset_time_fragment_counters(mut currency: ResMut<TimeFragmentCurrency>) {
    currency.total_collected_time_fragments_this_run = 0;
    currency.bounce_timer.reset();
}
