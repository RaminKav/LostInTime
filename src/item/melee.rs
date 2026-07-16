use bevy::prelude::*;

#[derive(Component, Reflect, FromReflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct MeleeAttack;
