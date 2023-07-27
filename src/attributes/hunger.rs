use std::time::Duration;

use bevy::prelude::*;

use crate::{
    animations::AttackEvent, item::item_actions::ActionSuccessEvent, player::Player, Game,
};

use super::CurrentHealth;

#[derive(Component, Default)]
pub struct Hunger {
    pub max: u8,
    pub current: u8,
    pub timer: Timer,
    pub action_fatigue: u8,
    pub action_fatigue_max: u8,
}

impl Hunger {
    pub fn new(max: u8, tick_time: f32, action_fatigue_max: u8) -> Self {
        Self {
            max,
            current: max,
            timer: Timer::from_seconds(tick_time, TimerMode::Once),
            action_fatigue: action_fatigue_max,
            action_fatigue_max,
        }
    }
    pub fn is_fatigued(&self) -> bool {
        self.action_fatigue >= self.action_fatigue_max
    }
    pub fn is_starving(&self) -> bool {
        self.current == 0
    }
    pub fn modify_hunger(&mut self, amount: i8) {
        if amount < 0 {
            self.current -= amount.abs() as u8;
        } else {
            self.current += amount as u8;
            if self.current >= self.max {
                self.current = self.max;
            }
        }
    }
}

pub fn tick_hunger(
    mut hunger_query: Query<(&mut Hunger, &mut CurrentHealth), With<Player>>,
    game: Res<Game>,
    time: Res<Time>,
) {
    for (mut hunger, mut health) in hunger_query.iter_mut() {
        let is_moving = game.player_state.is_moving;
        let d = time.delta();
        hunger.timer.tick(if is_moving {
            Duration::new(
                (d.as_secs() as f32 * 1.25) as u64,
                (d.subsec_nanos() as f32 * 1.25) as u32,
            )
        } else {
            d
        });

        if hunger.timer.finished() {
            if hunger.current == 0 {
                health.0 -= 1;
            } else {
                hunger.current -= 1;
            }
            println!("TICK HUNGER: {:?}", hunger.current);
            hunger.timer.reset();
        }
    }
}

pub fn handle_actions_drain_hunger(
    mut hunger_query: Query<(&mut Hunger, &mut CurrentHealth), With<Hunger>>,
    action_events: EventReader<ActionSuccessEvent>,
    attack_event: EventReader<AttackEvent>,
) {
    if action_events.len() > 0 || attack_event.len() > 0 {
        for (mut hunger, mut health) in hunger_query.iter_mut() {
            hunger.action_fatigue += 1;
            if hunger.is_fatigued() {
                hunger.action_fatigue = 0;
                if hunger.current == 0 {
                    health.0 -= 1;
                } else {
                    hunger.current -= 1;
                }
                println!("FATIGUED: {:?}", hunger.current);
            }
        }
    }
}
