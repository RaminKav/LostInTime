//! Typed component extraction from [`EntityDef`] — replaces proto schematic downcasts.

use crate::{
    assets::{SpriteAnchor, SpriteSize},
    attributes::{Attack, MaxHealth, RawItemBaseAttributes, RawItemBonusAttributes},
    enemy::{CombatAlignment, Mob},
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemActions, ManaCost},
        melee::MeleeAttack,
        object_actions::{ObjectAction, ObjectActionCost},
        projectile::{ArcProjectileData, Projectile, RangedAttack},
        EquipmentType, Wall, WorldObject,
    },
    ui::scrapper_ui::ScrapsInto,
    world::WallTextureData,
};

use super::types::{ColliderDef, EntityDef};

/// Components that can be read from an [`EntityDef`].
pub trait DefComponent: Sized {
    fn get_from_def(def: &EntityDef) -> Option<&Self>;
}

macro_rules! def_component {
    ($ty:ty, $field:ident) => {
        impl DefComponent for $ty {
            fn get_from_def(def: &EntityDef) -> Option<&Self> {
                def.$field.as_ref()
            }
        }
    };
}

def_component!(WorldObject, world_object);
def_component!(Mob, mob);
def_component!(Projectile, projectile);
def_component!(ItemStack, item_stack);
def_component!(SpriteSize, sprite_size);
def_component!(SpriteAnchor, sprite_anchor);
def_component!(MaxHealth, max_health);
def_component!(Attack, attack);
def_component!(EquipmentType, equipment_type);
def_component!(RangedAttack, ranged);
def_component!(MeleeAttack, melee);
def_component!(CombatAlignment, combat_alignment);
def_component!(ItemActions, item_actions);
def_component!(ConsumableItem, consumable);
def_component!(ManaCost, mana_cost);
def_component!(ObjectAction, object_action);
def_component!(ObjectActionCost, object_action_cost);
def_component!(RawItemBaseAttributes, raw_item_base);
def_component!(RawItemBonusAttributes, raw_item_bonus);
def_component!(ScrapsInto, scraps_into);
def_component!(Wall, wall);
def_component!(WallTextureData, wall_texture_data);
def_component!(ArcProjectileData, arc_projectile_data);
def_component!(ColliderDef, collider);

impl EntityDef {
    pub fn scaled_capsule_collider(&self, scale: f32) -> Option<bevy_rapier2d::prelude::Collider> {
        let col = self.collider.as_ref()?;
        match col.kind {
            super::types::ColliderKind::Capsule {
                x1,
                y1,
                x2,
                y2,
                r,
            } => Some(bevy_rapier2d::prelude::Collider::capsule(
                bevy::prelude::Vec2::new(x1 * scale, y1 * scale),
                bevy::prelude::Vec2::new(x2 * scale, y2 * scale),
                r * scale,
            )),
            super::types::ColliderKind::Cuboid { x, y } => Some(
                bevy_rapier2d::prelude::Collider::cuboid(x * scale, y * scale),
            ),
        }
    }
}
