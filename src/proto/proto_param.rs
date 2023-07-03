use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_proto::prelude::*;
use core::fmt::Display;

use crate::{inventory::ItemStack, item::projectile::ProjectileState};

#[derive(SystemParam)]
pub struct ProtoParam<'w, 's> {
    pub proto_commands: ProtoCommands<'w, 's>,
    pub prototypes: Prototypes<'w>,
    pub prototype_assets: Res<'w, Assets<Prototype>>,
}
impl<'w, 's> ProtoParam<'w, 's> {
    pub fn get_prototype(&self, id: &str) -> Option<&Prototype> {
        self.prototype_assets.get(
            self.prototypes
                .get(format!("proto/{}.prototype.ron", id.to_lowercase()))?,
        )
    }
    pub fn get_item_data<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&ItemStack> {
        let id = <T as Into<&str>>::into(obj).to_owned();

        if let Some(data) = self.get_prototype(&id) {
            data.schematics()
                .get::<ItemStack>()
                .unwrap()
                .input()
                .downcast_ref::<ItemStack>()
        } else {
            println!("Could not get item data for: {}", id);
            None
        }
    }
    pub fn get_projectile_state<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&ProjectileState> {
        let id = <T as Into<&str>>::into(obj).to_owned();

        if let Some(data) = self.get_prototype(&id) {
            data.schematics()
                .get::<ProjectileState>()
                .unwrap()
                .input()
                .downcast_ref::<ProjectileState>()
        } else {
            println!("Could not get projectile data for: {}", id);
            None
        }
    }
}
