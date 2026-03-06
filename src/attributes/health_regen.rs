use std::time::Duration;

use bevy::prelude::*;

use crate::player::{
    skills::{Heirloom, PlayerSkills},
    Player,
};

use super::{
    hunger::Hunger,
    modifiers::{ModifyHealthEvent, ModifyManaEvent},
    HealthRegen, ManaRegen,
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

pub fn handle_health_regen(
    mut player_regen: Query<
        (&HealthRegen, &mut HealthRegenTimer, &Hunger, &PlayerSkills),
        With<Player>,
    >,
    mut modify_health_event: EventWriter<ModifyHealthEvent>,
    time: Res<Time>,
) {
    let (health_regen, mut timer, hunger, skills) = player_regen.single_mut();
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
        modify_health_event.send(ModifyHealthEvent(health_regen.0));
        timer.0.reset();
    }
}
#[derive(Component)]
pub struct ManaRegenTimer(pub Timer);

pub fn handle_mana_regen(
    mut player_regen: Query<
        (&ManaRegen, &mut ManaRegenTimer, &Hunger, &PlayerSkills),
        With<Player>,
    >,
    mut modify_mana_event: EventWriter<ModifyManaEvent>,
    time: Res<Time>,
) {
    let (mana_regen, mut timer, hunger, skills) = player_regen.single_mut();
    let d = time.delta();

    // Multiplicatively reduce regen cooldown per stack (0.75^stacks)
    let mp_regen_stacks = skills.get_count(Heirloom::MPRegenCooldown);
    let multiplier = get_regen_cooldown_multiplier(mp_regen_stacks).max(0.01);

    timer.0.tick(Duration::new(
        (d.as_secs() as f32 / multiplier) as u64,
        (d.subsec_nanos() as f32 / multiplier) as u32,
    ));
    if timer.0.finished() {
        if hunger.is_starving() {
            return;
        }
        modify_mana_event.send(ModifyManaEvent(mana_regen.0));
        timer.0.reset();
    }
}
