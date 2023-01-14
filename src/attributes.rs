use bevy::prelude::*;
use bevy_inspector_egui::{Inspectable, RegisterInspectable};

pub struct AttributesPlugin;

#[derive(Bundle, Inspectable)]
pub struct BlockAttributeBundle {
    pub health: Health,
}
#[derive(Bundle, Inspectable)]
pub struct EquipmentAttributeBundle {
    pub health: Health,
    pub attack: Attack,
}

#[derive(Component, Inspectable, Clone, Copy)]
pub struct Health(pub u8);
#[derive(Component, Inspectable, Clone, Copy)]
pub struct Attack(pub u8);

impl Plugin for AttributesPlugin {
    fn build(&self, app: &mut App) {
        app.register_inspectable::<BlockAttributeBundle>();
    }
}

impl AttributesPlugin {}
