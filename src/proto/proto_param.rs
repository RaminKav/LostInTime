use bevy::{ecs::system::SystemParam, prelude::*};
use core::fmt::Display;

use crate::{
    assets::Graphics,
    defs::{
        lookup::DefComponent,
        registry::GameDefs,
        types::SpriteSheetDef,
    },
    inventory::ItemStack,
    item::{
        melee::MeleeAttack,
        projectile::{ProjectileState, RangedAttack},
        WorldObject,
    },
};

/// Game-definition lookups + shared asset handles.
///
/// Formerly backed by `bevy_proto`; now reads exclusively from [`GameDefs`].
/// Spawn goes through [`crate::custom_commands::CommandsExt`] on [`Commands`].
#[derive(SystemParam)]
pub struct ProtoParam<'w> {
    pub meshes: ResMut<'w, Assets<Mesh>>,
    pub graphics: Res<'w, Graphics>,
    pub asset_server: Res<'w, AssetServer>,
    pub defs: Res<'w, GameDefs>,
}

impl<'w> ProtoParam<'w> {
    fn def_by_name<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&crate::defs::types::EntityDef> {
        let id = <T as Into<&str>>::into(obj);
        self.defs.get(id)
    }

    pub fn get_item_data<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&ItemStack> {
        self.def_by_name(obj).and_then(|d| d.item_stack.as_ref())
    }

    pub fn get_component<'a, C: DefComponent, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&C> {
        let def = self.def_by_name(obj)?;
        C::get_from_def(def)
    }

    pub fn get_world_object<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&WorldObject> {
        self.def_by_name(obj).and_then(|d| d.world_object.as_ref())
    }

    pub fn is_item_ranged_weapon<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&RangedAttack> {
        self.def_by_name(obj).and_then(|d| d.ranged.as_ref())
    }

    pub fn is_item_melee_weapon<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&MeleeAttack> {
        self.def_by_name(obj).and_then(|d| d.melee.as_ref())
    }

    pub fn get_projectile_state<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&ProjectileState> {
        self.def_by_name(obj)
            .and_then(|d| d.projectile_state.as_ref())
    }

    pub fn get_sprite_sheet_data<'a, T: Display + Clone + Into<&'a str>>(
        &self,
        obj: T,
    ) -> Option<&SpriteSheetDef> {
        self.def_by_name(obj).and_then(|d| d.sprite_sheet.as_ref())
    }
}
