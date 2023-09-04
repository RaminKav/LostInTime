use bevy::prelude::*;

use crate::player::Player;

use super::{
    hunger::Hunger,
    modifiers::{ModifyHealthEvent, ModifyManaEvent},
    HealthRegen, ManaRegen,
};

#[derive(Component)]
pub struct HealthRegenTimer(pub Timer);

pub fn handle_health_regen(
    mut player_regen: Query<(&HealthRegen, &mut HealthRegenTimer, &Hunger), With<Player>>,
    mut modify_health_event: EventWriter<ModifyHealthEvent>,
    time: Res<Time>,
) {
    let (health_regen, mut timer, hunger) = player_regen.single_mut();
    timer.0.tick(time.delta());
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
    mut player_regen: Query<(&ManaRegen, &mut ManaRegenTimer, &Hunger), With<Player>>,
    mut modify_health_event: EventWriter<ModifyManaEvent>,
    time: Res<Time>,
) {
    let (mana_regen, mut timer, hunger) = player_regen.single_mut();
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        if hunger.is_starving() {
            return;
        }
        modify_health_event.send(ModifyManaEvent(mana_regen.0));
        timer.0.reset();
    }
}
