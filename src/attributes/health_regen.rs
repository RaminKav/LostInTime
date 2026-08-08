use std::time::Duration;

use bevy::prelude::*;

use crate::player::{
    skills::{HealthGainSource, Heirloom, ManaGainSource, PlayerSkills},
    Player,
};

use super::{
    hunger::Hunger,
    modifiers::{ModifyHealthEvent, ModifyManaEvent},
    CurrentHealth, HealthRegen, ManaRegen,
};

#[derive(Component)]
pub struct HealthRegenTimer(pub Timer);

/// Calculate the multiplicative regen cooldown reduction
/// Each stack multiplies by 0.75, so it can never reach 0
/// 1 stack = 0.75x, 2 stacks = 0.5625x, 3 stacks = 0.42x, etc.
fn get_regen_cooldown_multiplier(stacks: i32) -> f32 {
    if stacks <= 0 {
        1.0
    } else {
        0.75_f32.powi(stacks)
    }
}

/// Real-time seconds between regen ticks for this timer duration and heirloom stacks.
/// Same scaling as `handle_health_regen` / `handle_mana_regen` (`duration * multiplier`).
pub fn effective_regen_period_secs(timer_duration_secs: f32, heirloom_stacks: i32) -> f32 {
    let multiplier = get_regen_cooldown_multiplier(heirloom_stacks).max(0.01);
    timer_duration_secs * multiplier
}

pub fn handle_health_regen(
    mut player_regen: Query<
        (
            &HealthRegen,
            &mut HealthRegenTimer,
            &Hunger,
            &PlayerSkills,
            Option<&CurrentHealth>,
        ),
        With<Player>,
    >,
    mut modify_health_event: MessageWriter<ModifyHealthEvent>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
    time: Res<Time>,
) {
    let Ok((health_regen, mut timer, hunger, skills, current_health)) = player_regen.single_mut()
    else {
        return;
    };
    let d = time.delta();

    // Multiplicatively reduce regen cooldown per stack (0.75^stacks)
    let hp_regen_stacks = skills.get_count(Heirloom::HPRegenCooldown);
    let multiplier = get_regen_cooldown_multiplier(hp_regen_stacks).max(0.01);

    timer.0.tick(Duration::new(
        (d.as_secs() as f32 / multiplier) as u64,
        (d.subsec_nanos() as f32 / multiplier) as u32,
    ));
    if timer.0.just_finished() {
        if hunger.is_starving() {
            return;
        }
        let regen_delta = health_regen.0;
        if regen_delta < 0 {
            let would_be_lethal = current_health
                .map(|h| h.0 + regen_delta <= 0)
                .unwrap_or(true);
            if would_be_lethal {
                timer.0.reset();
                return;
            }
        }
        if regen_delta > 0 {
            trigger_counts.record_health_gain(HealthGainSource::HealthRegen, regen_delta);
        }
        modify_health_event.write(ModifyHealthEvent(regen_delta));
        timer.0.reset();
    }
}
#[derive(Component)]
pub struct ManaRegenTimer(pub Timer);

/// Brief delay before Overflowing Mind's bonus +1 MP pulse, so floating text / heirloom
/// procs stay visually and mechanically separate from the natural regen tick.
const EXTRA_MANA_REGEN_DELAY_SECS: f32 = 0.12;

/// Queued Overflowing Mind bonus regen pulses. Only scheduled from the natural mana
/// regen timer — never from these pulses themselves (no self-trigger loop).
#[derive(Component)]
pub struct PendingExtraManaRegen {
    pub remaining: u32,
    pub timer: Timer,
}

impl PendingExtraManaRegen {
    fn new_pulse() -> Self {
        Self {
            remaining: 1,
            timer: Timer::from_seconds(EXTRA_MANA_REGEN_DELAY_SECS, TimerMode::Once),
        }
    }

    fn enqueue_pulse(&mut self) {
        self.remaining = self.remaining.saturating_add(1);
        if self.timer.is_finished() {
            self.timer = Timer::from_seconds(EXTRA_MANA_REGEN_DELAY_SECS, TimerMode::Once);
        }
    }
}

pub fn handle_mana_regen(
    mut player_regen: Query<
        (
            Entity,
            &ManaRegen,
            &mut ManaRegenTimer,
            &Hunger,
            &PlayerSkills,
            &crate::blessings::OwnedMajorBlessings,
            Option<&mut PendingExtraManaRegen>,
        ),
        With<Player>,
    >,
    mut modify_mana_event: MessageWriter<ModifyManaEvent>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let Ok((entity, mana_regen, mut timer, hunger, skills, majors, pending_extra)) =
        player_regen.single_mut()
    else {
        return;
    };
    let d = time.delta();

    // Multiplicatively reduce regen cooldown per stack (0.75^stacks)
    let mp_regen_stacks = skills.get_count(Heirloom::MPRegenCooldown);
    let multiplier = get_regen_cooldown_multiplier(mp_regen_stacks).max(0.01);

    timer.0.tick(Duration::new(
        (d.as_secs() as f32 / multiplier) as u64,
        (d.subsec_nanos() as f32 / multiplier) as u32,
    ));
    if timer.0.is_finished() {
        if hunger.is_starving() {
            return;
        }
        modify_mana_event.write(ModifyManaEvent::gain(
            mana_regen.0,
            ManaGainSource::ManaRegen,
        ));
        // Schedule a delayed separate regen pulse (+1) so floating numbers and
        // ManaRegenLightning / ManaRegenPoison see it as its own trigger.
        if majors.has(crate::blessings::MajorBlessing::ExtraManaRegen) {
            if let Some(mut pending) = pending_extra {
                pending.enqueue_pulse();
            } else {
                commands.entity(entity).insert(PendingExtraManaRegen::new_pulse());
            }
        }
        timer.0.reset();
    }
}

/// Fires Overflowing Mind's delayed +1 as a full mana-regen event (floating text + heirlooms).
pub fn handle_pending_extra_mana_regen(
    mut player: Query<(Entity, &mut PendingExtraManaRegen), With<Player>>,
    mut modify_mana_event: MessageWriter<ModifyManaEvent>,
    mut commands: Commands,
    time: Res<Time>,
) {
    let Ok((entity, mut pending)) = player.single_mut() else {
        return;
    };
    if pending.remaining == 0 {
        commands.entity(entity).remove::<PendingExtraManaRegen>();
        return;
    }
    pending.timer.tick(time.delta());
    if !pending.timer.just_finished() {
        return;
    }
    modify_mana_event.write(ModifyManaEvent::gain(1, ManaGainSource::ManaRegen));
    pending.remaining -= 1;
    if pending.remaining > 0 {
        pending.timer = Timer::from_seconds(EXTRA_MANA_REGEN_DELAY_SECS, TimerMode::Once);
    } else {
        commands.entity(entity).remove::<PendingExtraManaRegen>();
    }
}
