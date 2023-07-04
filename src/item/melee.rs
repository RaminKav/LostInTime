use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

#[derive(Component, Reflect, Schematic, FromReflect, Default, Clone)]
#[reflect(Component, Schematic)]
pub struct MeleeAttack;
