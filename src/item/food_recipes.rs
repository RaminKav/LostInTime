//! Era-gating for the new stat-food recipes (see `assets/proto/*food.prototype.ron`).
//!
//! Recipes themselves live in `assets/recipes/recipes.ron` and are eagerly inserted into
//! [`crate::item::Recipes::crafting_list`] at startup. The blueprints UI calls
//! [`is_food_recipe_unlocked`] to filter rows by [`crate::world::dimension::EraManager::visited_eras`].
//! The starting era is seeded into that list when the run begins; each era entered is appended once
//! (order may vary if progression becomes non-linear later).

use crate::item::WorldObject;
use crate::world::dimension::Era;

/// Required eras for a stat-food recipe to appear in the blueprints UI.
///
/// Single-element slices are single-era recipes. Two-element slices are cross-era legendary
/// recipes, which require the player to have visited BOTH eras.
///
/// Returns `None` for non-food objects.
pub fn food_recipe_required_eras(obj: WorldObject) -> Option<&'static [Era]> {
    Some(match obj {
        // Era 1
        WorldObject::SpeedFood => &[Era::Main],
        WorldObject::HealthFood => &[Era::Main],
        WorldObject::ManaFood => &[Era::Main],
        // Era 2
        WorldObject::ThornsFood => &[Era::Second],
        WorldObject::CritChanceFood => &[Era::Second],
        WorldObject::LifestealFood => &[Era::Second],
        // Era 3
        WorldObject::SkillPowerFood => &[Era::Third],
        WorldObject::ManaRegenFood => &[Era::Third],
        WorldObject::DodgeFood => &[Era::Third],
        // Cross-era legendaries
        WorldObject::DefenceFood => &[Era::Second, Era::Third],
        WorldObject::SizeFood => &[Era::Main, Era::Second],
        WorldObject::AttackSpeedFood => &[Era::Main, Era::Third],
        _ => return None,
    })
}

/// Returns `true` when `obj` is either not era-gated (everything except the stat foods) or every
/// era listed in [`food_recipe_required_eras`] appears in `visited_eras`.
///
/// [`EraManager`] seeds `visited_eras` with the starting overworld era and appends each era when
/// the player enters it (see `DimensionPlugin::new_dim_with_params`).
pub fn is_food_recipe_unlocked(obj: WorldObject, visited_eras: &[Era]) -> bool {
    let Some(required) = food_recipe_required_eras(obj) else {
        return true;
    };
    required.iter().all(|era| visited_eras.contains(era))
}
