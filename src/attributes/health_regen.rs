use std::time::Duration;

use bevy::prelude::*;

use crate::player::{
    skills::{PlayerSkills, Skill},
    Player,
};

use super::{
    hunger::Hunger,
    modifiers::{ModifyHealthEvent, ModifyManaEvent},
    HealthRegen, ManaRegen,
};

#[derive(Component)]
pub struct HealthRegenTimer(pub Timer);

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

    timer.0.tick(if skills.has(Skill::HPRegenCooldown) {
        Duration::new(
            (d.as_secs() as f32 * 0.75) as u64,
            (d.subsec_nanos() as f32 * 0.75) as u32,
        )
    } else {
        d
    });
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

    timer.0.tick(if skills.has(Skill::MPRegenCooldown) {
        Duration::new(
            (d.as_secs() as f32 * 0.75) as u64,
            (d.subsec_nanos() as f32 * 0.75) as u32,
        )
    } else {
        d
    });
    if timer.0.finished() {
        if hunger.is_starving() {
            return;
        }
        modify_mana_event.send(ModifyManaEvent(
            mana_regen.0 + skills.get_count(Skill::MPRegen) * 5,
        ));
        timer.0.reset();
    }
}
