use std::time::Duration;

use bevy::prelude::*;

use crate::{
    animations::AttackEvent,
    item::item_actions::ActionSuccessEvent,
    player::{
        skills::{PlayerSkills, Skill},
        Player,
    },
    Game,
};

use super::CurrentHealth;

#[derive(Component, Default)]
pub struct Hunger {
    pub max: u8,
    pub current: u8,
}
#[derive(Component, Default)]
pub struct HungerTracker {
    pub timer: Timer,
    pub action_fatigue: u8,
    pub action_fatigue_max: u8,
}
impl HungerTracker {
    pub fn new(tick_time: f32, action_fatigue_max: u8) -> Self {
        Self {
            timer: Timer::from_seconds(tick_time, TimerMode::Once),
            action_fatigue: 0,
            action_fatigue_max,
        }
    }
    pub fn is_fatigued(&self) -> bool {
        self.action_fatigue >= self.action_fatigue_max
    }
}
impl Hunger {
    pub fn new(max: u8) -> Self {
        Self { max, current: max }
    }

    pub fn is_starving(&self) -> bool {
        self.current == 0
    }
    pub fn modify_hunger(&mut self, amount: i8) {
        if amount < 0 {
            self.current -= amount.unsigned_abs();
        } else {
            self.current += amount as u8;
            if self.current >= self.max {
                self.current = self.max;
            }
        }
    }
}

pub fn tick_hunger(
    mut hunger_query: Query<
        (
            &mut Hunger,
            &mut HungerTracker,
            &mut CurrentHealth,
            &PlayerSkills,
        ),
        With<Player>,
    >,
    game: Res<Game>,
    time: Res<Time>,
) {
    for (mut hunger, mut tracker, mut health, skills) in hunger_query.iter_mut() {
        let is_moving = game.player_state.is_moving;
        let skill_mod = if skills.has(Skill::FullStomach) {
            0.7
        } else {
            1.
        };
        let d = Duration::new(
            (time.delta().as_secs() as f32 * skill_mod) as u64,
            (time.delta().subsec_nanos() as f32 * skill_mod) as u32,
        );
        tracker.timer.tick(if is_moving {
            Duration::new(
                (d.as_secs() as f32 * 1.25) as u64,
                (d.subsec_nanos() as f32 * 1.25) as u32,
            )
        } else {
            d
        });

        if tracker.timer.finished() {
            if hunger.current == 0 {
                health.0 -= 1;
            } else {
                hunger.current -= 1;
            }
            tracker.timer.reset();
        }
    }
}

pub fn handle_actions_drain_hunger(
    mut hunger_query: Query<(&mut Hunger, &mut HungerTracker, &mut CurrentHealth), With<Hunger>>,
    action_events: EventReader<ActionSuccessEvent>,
    attack_event: EventReader<AttackEvent>,
) {
    if !action_events.is_empty() || !attack_event.is_empty() {
        for (mut hunger, mut tracker, mut health) in hunger_query.iter_mut() {
            tracker.action_fatigue += 1;
            if tracker.is_fatigued() {
                tracker.action_fatigue = 0;
                if hunger.current == 0 {
                    health.0 -= 1;
                } else {
                    hunger.current -= 1;
                }
            }
        }
    }
}
