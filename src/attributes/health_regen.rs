use bevy::prelude::*;

use crate::player::Player;

use super::{modifiers::ModifyHealthEvent, HealthRegen};

#[derive(Component)]
pub struct HealthRegenTimer(pub Timer);

pub fn handle_health_regen(
    mut player_regen: Query<(&HealthRegen, &mut HealthRegenTimer), With<Player>>,
    mut modify_health_event: EventWriter<ModifyHealthEvent>,
    time: Res<Time>,
) {
    let (health_regen, mut timer) = player_regen.single_mut();
    timer.0.tick(time.delta());
    if timer.0.just_finished() {
        modify_health_event.send(ModifyHealthEvent(health_regen.0));
        timer.0.reset();
    }
}
