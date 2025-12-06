use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::modifiers::ModifyHealthEvent,
    audio::{AudioSoundEffect, SoundSpawner},
    client::persist_time_fragments,
    combat::EnemyDeathEvent,
    item::WorldObject,
    player::{
        skills::{Heirloom, PlayerSkills},
        Player,
    },
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
    player_skills: Query<&PlayerSkills, With<Player>>,
    mut modify_health_event: EventWriter<ModifyHealthEvent>,
) {
    let skills = player_skills.get_single().ok();
    let mut rng = rand::thread_rng();

    for event in events.iter() {
        if event.obj == WorldObject::TimeFragment {
            time_fragments.time_fragments = (time_fragments.time_fragments + event.delta).max(0);

            // Persist time fragments immediately when they are spent (delta < 0)
            // This prevents save-scumming by closing and restarting the game
            if event.delta < 0 {
                persist_time_fragments(time_fragments.time_fragments);
            }
        } else if event.obj == WorldObject::Coin {
            coins.coins = (coins.coins as i32 + event.delta).max(0) as u32;

            // CoinHeal: Picking up coins has a chance to heal
            if event.delta > 0 {
                if let Some(skills) = skills {
                    let stacks = skills.get_count(Heirloom::CoinHeal);
                    if stacks > 0 {
                        // Each coin pickup is a separate check
                        for _ in 0..event.delta {
                            let chance = 25 * stacks; // 25% per stack
                            let heal_amount = if chance > 100 {
                                // Past 100%, chance for 2 HP
                                let extra_chance = chance - 100;
                                if rng.gen_ratio(extra_chance.clamp(1, 100) as u32, 100) {
                                    2
                                } else {
                                    1
                                }
                            } else if rng.gen_ratio(chance.clamp(1, 100) as u32, 100) {
                                1
                            } else {
                                0
                            };
                            if heal_amount > 0 {
                                modify_health_event.send(ModifyHealthEvent(heal_amount));
                            }
                        }
                    }
                }
            }
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

pub fn reset_coin_counters(mut coins: ResMut<CoinCurrency>) {
    coins.coins = 0;
    coins.bounce_timer.reset();
}
