use crate::attributes::{AttackCooldown, AttributeChangeEvent};
use crate::player::Player;
use bevy::prelude::*;

/// Component for attack speed buff
#[derive(Component, Debug, Clone)]
pub struct AttackSpeedBuff {
    pub timer: Timer,
    pub speed_multiplier: f32,
}

impl AttackSpeedBuff {
    pub fn new(duration: f32, speed_multiplier: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            speed_multiplier,
        }
    }
}

/// Component for movement speed buff
#[derive(Component, Debug, Clone)]
pub struct MovementSpeedBuff {
    pub timer: Timer,
    pub speed_multiplier: f32,
}

impl MovementSpeedBuff {
    pub fn new(duration: f32, speed_multiplier: f32) -> Self {
        Self {
            timer: Timer::from_seconds(duration, TimerMode::Once),
            speed_multiplier,
        }
    }
}

/// System to apply attack speed buff to player's attack cooldown
pub fn apply_attack_speed_buff(
    mut player_query: Query<(&mut AttackCooldown, &AttackSpeedBuff), With<Player>>,
) {
    for (mut attack_cooldown, buff) in player_query.iter_mut() {
        // Apply the speed multiplier to the attack cooldown
        attack_cooldown.0 = attack_cooldown.0 / buff.speed_multiplier;
    }
}

/// System to tick buff timers and remove expired buffs
pub fn tick_potion_buffs(
    mut commands: Commands,
    mut attack_speed_buffs: Query<(Entity, &mut AttackSpeedBuff)>,
    mut movement_speed_buffs: Query<(Entity, &mut MovementSpeedBuff)>,
    mut attribute_event: EventWriter<AttributeChangeEvent>,
    time: Res<Time>,
) {
    // Tick attack speed buffs
    for (entity, mut buff) in attack_speed_buffs.iter_mut() {
        buff.timer.tick(time.delta());
        if buff.timer.finished() {
            attribute_event.send_default();
            commands.entity(entity).remove::<AttackSpeedBuff>();
        }
    }

    // Tick movement speed buffs
    for (entity, mut buff) in movement_speed_buffs.iter_mut() {
        buff.timer.tick(time.delta());
        if buff.timer.finished() {
            commands.entity(entity).remove::<MovementSpeedBuff>();
        }
    }
}
