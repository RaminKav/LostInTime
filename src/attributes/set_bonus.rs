use crate::attributes::{AttributeQuality, AttributeValue, ItemAttributes};
use crate::inventory::Inventory;
use crate::item::WorldObject;

/// Number of matching pieces required for a set bonus to activate.
pub const SET_PIECES_REQUIRED: usize = 3;

/// Equipment set families. Three armor pieces of the same family applies the set bonus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EquipmentSet {
    Leather,
    Metal,
    Forest,
}

impl EquipmentSet {
    /// Map a `WorldObject` to its `EquipmentSet`, or `None` if it isn't part of a set.
    pub fn from_world_object(obj: WorldObject) -> Option<Self> {
        match obj {
            WorldObject::LeatherTunic | WorldObject::LeatherPants | WorldObject::LeatherShoes => {
                Some(Self::Leather)
            }
            WorldObject::Chestplate | WorldObject::MetalPants | WorldObject::MetalShoes => {
                Some(Self::Metal)
            }
            WorldObject::ForestShirt | WorldObject::ForestPants | WorldObject::ForestShoes => {
                Some(Self::Forest)
            }
            _ => None,
        }
    }

    /// Short summary shown on tooltips, e.g. "+50 Mana, +20 MP Regen".
    pub fn bonus_description(self) -> &'static str {
        match self {
            Self::Leather => "+50 Mana,\n+20 MP Regen",
            Self::Metal => "+15% Max HP,\n+30 Defence",
            Self::Forest => "+25% Crit DMG,\n+25% Crit Chance",
        }
    }

    /// Count how many pieces of this set are currently equipped in the
    /// equipment (head/chest/legs/feet) container.
    pub fn count_equipped(self, inv: &Inventory) -> usize {
        inv.equipment_items
            .items
            .iter()
            .flatten()
            .filter(|stack| Self::from_world_object(stack.item_stack.obj_type) == Some(self))
            .count()
    }

    /// Whether this set's bonus is active (i.e. the player has at least
    /// `SET_PIECES_REQUIRED` pieces equipped).
    pub fn is_active(self, inv: &Inventory) -> bool {
        self.count_equipped(inv) >= SET_PIECES_REQUIRED
    }
}

/// Apply set bonuses to the player's combined equipment attributes during
/// stat recalculation. Returns the modified attributes with bonuses folded in
/// for any sets the player has fully equipped.
pub fn apply_set_bonuses(mut attrs: ItemAttributes, inv: &Inventory) -> ItemAttributes {
    for set in [
        EquipmentSet::Leather,
        EquipmentSet::Metal,
        EquipmentSet::Forest,
    ] {
        if !set.is_active(inv) {
            continue;
        }
        match set {
            EquipmentSet::Leather => {
                attrs.mana = attrs.mana + AttributeValue::new(50, AttributeQuality::Low, 0.0);
                attrs.mana_regen =
                    attrs.mana_regen + AttributeValue::new(20, AttributeQuality::Low, 0.0);
            }
            EquipmentSet::Metal => {
                // +15% Max HP applied to the equipment-summed HP, then +30 Defence flat.
                let hp_bonus = (attrs.health.value as f32 * 0.15).round() as i32;
                attrs.health = attrs.health + hp_bonus;
                attrs.defence = attrs.defence + AttributeValue::new(30, AttributeQuality::Low, 0.0);
            }
            EquipmentSet::Forest => {
                attrs.crit_chance =
                    attrs.crit_chance + AttributeValue::new(25, AttributeQuality::Low, 0.0);
                attrs.crit_damage =
                    attrs.crit_damage + AttributeValue::new(25, AttributeQuality::Low, 0.0);
            }
        }
    }
    attrs
}
