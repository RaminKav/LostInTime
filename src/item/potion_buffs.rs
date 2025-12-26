use crate::attributes::{AttributeChangeEvent, BonusAttackSpeed};
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

/// System to add attack speed multiplier to BonusAttackSpeed when buff is added
pub fn add_attack_speed_buff_to_bonus(
    added_buffs: Query<(Entity, &AttackSpeedBuff), Added<AttackSpeedBuff>>,
    mut player_query: Query<&mut BonusAttackSpeed, With<Player>>,
    mut attribute_event: EventWriter<AttributeChangeEvent>,
) {
    for (_entity, buff) in added_buffs.iter() {
        if let Ok(mut bonus_speed) = player_query.get_single_mut() {
            bonus_speed.add_multiplier(buff.speed_multiplier);
            attribute_event.send_default();
        }
    }
}

/// System to tick buff timers and remove expired buffs
pub fn tick_potion_buffs(
    mut commands: Commands,
    mut attack_speed_buffs: Query<(Entity, &mut AttackSpeedBuff)>,
    mut movement_speed_buffs: Query<(Entity, &mut MovementSpeedBuff)>,
    mut player_query: Query<&mut BonusAttackSpeed, With<Player>>,
    mut attribute_event: EventWriter<AttributeChangeEvent>,
    time: Res<Time>,
) {
    for (entity, mut buff) in attack_speed_buffs.iter_mut() {
        buff.timer.tick(time.delta());
        if buff.timer.finished() {
            if let Ok(mut bonus_speed) = player_query.get_single_mut() {
                bonus_speed.remove_multiplier(buff.speed_multiplier);
                attribute_event.send_default();
            }
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
