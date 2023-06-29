use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_proto::prelude::*;

use crate::inventory::ItemStack;

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
    pub fn get_item_data(&self, id: &str) -> Option<&ItemStack> {
        if let Some(data) = self.get_prototype(id) {
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
}
