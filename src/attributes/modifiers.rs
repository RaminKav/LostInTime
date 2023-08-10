use crate::player::Player;

use super::{CurrentHealth, Healing};

use bevy::prelude::*;

pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: EventReader<ModifyHealthEvent>,
    mut query: Query<(&mut CurrentHealth, &Healing), With<Player>>,
) {
    for event in event.iter() {
        let (mut health, bonus_healing_rate) = query.single_mut();
        health.0 += (event.0 as f32 * (1.0 + bonus_healing_rate.0 as f32 / 100.)) as i32;
    }
}
