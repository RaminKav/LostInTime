use crate::player::Player;

use super::CurrentHealth;

use bevy::prelude::*;

pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: EventReader<ModifyHealthEvent>,
    mut query: Query<&mut CurrentHealth, With<Player>>,
) {
    for event in event.iter() {
        let mut health = query.single_mut();
        health.0 += event.0;
    }
}
