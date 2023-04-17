use bevy::{prelude::*, time::FixedTimestep};
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

use crate::{GameState, Player, TIME_STEP};

pub struct AttributesPlugin;

#[derive(Bundle, Inspectable)]
pub struct BlockAttributeBundle {
    pub health: Health,
}
#[derive(Bundle, Inspectable)]
pub struct EquipmentAttributeBundle {
    pub health: Health,
    pub attack: Attack,
    pub attack_cooldown: AttackCooldown,
}

#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct Health(pub i8);
#[derive(Component, Inspectable, Clone, Debug, Copy)]
pub struct Attack(pub u8);

#[derive(Component, Inspectable, Clone, Debug, Copy)]

pub struct AttackCooldown(pub f32);
#[derive(Component, Inspectable, Clone, Debug, Copy)]

pub struct InvincibilityCooldown(pub f32);

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<BlockAttributeBundle>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::clamp_health),
            );
    }
}

impl AttributesPlugin {
    fn clamp_health(mut health: Query<&mut Health, With<Player>>) {
        for mut h in health.iter_mut() {
            if h.0 < 0 {
                h.0 = 0;
            } else if h.0 > 100 {
                h.0 = 100;
            }
        }
    }
}
