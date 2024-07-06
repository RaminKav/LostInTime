use bevy::{ecs::system::SystemParam, prelude::*};
use bevy_proto::prelude::*;
use core::fmt::Display;

use crate::{
    assets::Graphics,
    inventory::ItemStack,
    item::{
        melee::MeleeAttack,
        projectile::{ProjectileState, RangedAttack},
        WorldObject,
    },
    world::WorldGeneration,
};

use super::SpriteSheetProto;

#[derive(SystemParam)]
pub struct ProtoParam<'w, 's> {
    pub proto_commands: ProtoCommands<'w, 's>,
    pub prototypes: Prototypes<'w>,
    pub prototype_assets: Res<'w, Assets<Prototype>>,
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub graphics: Res<'w, Graphics>,
    pub asset_server: Res<'w, AssetServer>,
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
            if let Some(data) = data.schematics().get::<ItemStack>() {
                return data.input().downcast_ref::<ItemStack>();
            }
            None
        } else {
            warn!("Could not get item data for: {}", id);
            None
        }
    }
    pub fn get_component<
        'a,
        C: Component + Schematic,
        T: Display + Schematic + Clone + Into<&'a str>,
    >(
        &self,
        obj: T,
    ) -> Option<&C> {
        let id = <T as Into<&str>>::into(obj).to_owned();
        if let Some(data) = self.get_prototype(&id) {
            if let Some(data) = data.schematics().get::<C>() {
                return data.input().downcast_ref::<C>();
            }
            None
        } else {
            warn!("Could not get component for: {}", id);
            None
        }
    }
    pub fn get_world_object<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&WorldObject> {
        let id = <T as Into<&str>>::into(obj).to_owned();
        if let Some(data) = self.get_prototype(&id) {
            data.schematics()
                .get::<WorldObject>()
                .unwrap()
                .input()
                .downcast_ref::<WorldObject>()
        } else {
            warn!("Could not get world object data for: {}", id);
            None
        }
    }
    /// Returns the [RangedAttack] component for the given item if it exists
    pub fn is_item_ranged_weapon<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&RangedAttack> {
        let id = <T as Into<&str>>::into(obj).to_owned();

        if let Some(data) = self.get_prototype(&id) {
            let Some(data) = data.schematics().get::<RangedAttack>() else {
                return None;
            };
            data.input().downcast_ref::<RangedAttack>()
        } else {
            warn!("Could not get item data for: {}", id);
            None
        }
    }
    /// Returns the [MeleeAttack] component for the given item if it exists
    pub fn is_item_melee_weapon<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&MeleeAttack> {
        let id = <T as Into<&str>>::into(obj).to_owned();

        if let Some(data) = self.get_prototype(&id) {
            let Some(data) = data.schematics().get::<MeleeAttack>() else {
                return None;
            };
            data.input().downcast_ref::<MeleeAttack>()
        } else {
            warn!("Could not get item data for: {}", id);
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
            warn!("Could not get projectile data for: {}", id);
            None
        }
    }
    pub fn get_sprite_sheet_data<'a, T: Display + Schematic + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&SpriteSheetProto> {
        let id = <T as Into<&str>>::into(obj).to_owned();

        if let Some(data) = self.get_prototype(&id) {
            if let Some(data) = data.schematics().get::<SpriteSheetProto>() {
                return data.input().downcast_ref::<SpriteSheetProto>();
            }
            None
        } else {
            warn!("Could not get projectile data for: {}", id);
            None
        }
    }
}
