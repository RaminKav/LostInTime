use bevy::{prelude::Resource, utils::HashMap};

use crate::{
    assets::{SpriteAnchor, SpriteSize},
    attributes::{Attack, MaxHealth, RawItemBaseAttributes, RawItemBonusAttributes},
    enemy::{CombatAlignment, Mob},
    inventory::ItemStack,
    item::{
        item_actions::{ConsumableItem, ItemActions, ManaCost},
        melee::MeleeAttack,
        object_actions::{ObjectAction, ObjectActionCost},
        projectile::{Projectile, ProjectileState, RangedAttack},
        EquipmentType, Wall, WorldObject,
    },
    ui::scrapper_ui::ScrapsInto,
};

use super::types::{EntityDef, EraDef, SpriteSheetDef};

#[derive(Resource, Default, Clone)]
pub struct GameDefs {
    /// Primary lookup by prototype Display name ("Bushling", "Sword", "Fireball")
    by_name: HashMap<String, EntityDef>,
    by_world_object: HashMap<WorldObject, EntityDef>,
    by_mob: HashMap<Mob, EntityDef>,
    by_projectile: HashMap<Projectile, EntityDef>,
    eras: HashMap<String, EraDef>,
}

impl GameDefs {
    pub fn insert_entity(&mut self, def: EntityDef) {
        if let Some(obj) = def.world_object {
            self.by_world_object.insert(obj, def.clone());
        }
        if let Some(mob) = def.mob.clone() {
            self.by_mob.insert(mob, def.clone());
        }
        if let Some(projectile) = def.projectile {
            self.by_projectile.insert(projectile, def.clone());
        }
        self.by_name.insert(def.name.clone(), def);
    }

    pub fn insert_era(&mut self, def: EraDef) {
        self.eras.insert(def.name.clone(), def);
    }

    pub fn get(&self, name: &str) -> Option<&EntityDef> {
        self.by_name.get(name)
    }

    pub fn get_world_object_def(&self, obj: WorldObject) -> Option<&EntityDef> {
        self.by_world_object.get(&obj)
    }

    pub fn get_mob_def(&self, mob: Mob) -> Option<&EntityDef> {
        self.by_mob.get(&mob)
    }

    pub fn get_projectile_def(&self, p: Projectile) -> Option<&EntityDef> {
        self.by_projectile.get(&p)
    }

    pub fn get_era(&self, name: &str) -> Option<&EraDef> {
        self.eras.get(name)
    }

    pub fn get_item_data(&self, obj: WorldObject) -> Option<&ItemStack> {
        self.get_world_object_def(obj)
            .and_then(|d| d.item_stack.as_ref())
    }

    pub fn get_sprite_sheet_data(&self, name: &str) -> Option<&SpriteSheetDef> {
        self.get(name).and_then(|d| d.sprite_sheet.as_ref())
    }

    pub fn is_item_ranged_weapon(&self, obj: WorldObject) -> Option<&RangedAttack> {
        self.get_world_object_def(obj)
            .and_then(|d| d.ranged.as_ref())
    }

    pub fn is_item_melee_weapon(&self, obj: WorldObject) -> Option<&MeleeAttack> {
        self.get_world_object_def(obj)
            .and_then(|d| d.melee.as_ref())
    }

    pub fn get_projectile_state(&self, p: Projectile) -> Option<&ProjectileState> {
        self.get_projectile_def(p)
            .and_then(|d| d.projectile_state.as_ref())
    }

    pub fn get_equipment_type(&self, obj: WorldObject) -> Option<&EquipmentType> {
        self.get_world_object_def(obj)
            .and_then(|d| d.equipment_type.as_ref())
    }

    pub fn get_sprite_anchor(&self, obj: WorldObject) -> Option<&SpriteAnchor> {
        self.get_world_object_def(obj)
            .and_then(|d| d.sprite_anchor.as_ref())
    }

    pub fn get_sprite_size(&self, obj: WorldObject) -> Option<&SpriteSize> {
        self.get_world_object_def(obj)
            .and_then(|d| d.sprite_size.as_ref())
    }

    pub fn get_item_actions(&self, obj: WorldObject) -> Option<&ItemActions> {
        self.get_world_object_def(obj)
            .and_then(|d| d.item_actions.as_ref())
    }

    pub fn get_consumable(&self, obj: WorldObject) -> Option<&ConsumableItem> {
        self.get_world_object_def(obj)
            .and_then(|d| d.consumable.as_ref())
    }

    pub fn get_mana_cost(&self, obj: WorldObject) -> Option<&ManaCost> {
        self.get_world_object_def(obj)
            .and_then(|d| d.mana_cost.as_ref())
    }

    pub fn get_raw_item_base(&self, obj: WorldObject) -> Option<&RawItemBaseAttributes> {
        self.get_world_object_def(obj)
            .and_then(|d| d.raw_item_base.as_ref())
    }

    pub fn get_raw_item_bonus(&self, obj: WorldObject) -> Option<&RawItemBonusAttributes> {
        self.get_world_object_def(obj)
            .and_then(|d| d.raw_item_bonus.as_ref())
    }

    pub fn get_max_health_mob(&self, mob: Mob) -> Option<&MaxHealth> {
        self.get_mob_def(mob).and_then(|d| d.max_health.as_ref())
    }

    pub fn get_attack_mob(&self, mob: Mob) -> Option<&Attack> {
        self.get_mob_def(mob).and_then(|d| d.attack.as_ref())
    }

    pub fn get_wall(&self, obj: WorldObject) -> Option<&Wall> {
        self.get_world_object_def(obj).and_then(|d| d.wall.as_ref())
    }

    pub fn get_scraps_into(&self, obj: WorldObject) -> Option<&ScrapsInto> {
        self.get_world_object_def(obj)
            .and_then(|d| d.scraps_into.as_ref())
    }

    pub fn get_object_action(&self, obj: WorldObject) -> Option<&ObjectAction> {
        self.get_world_object_def(obj)
            .and_then(|d| d.object_action.as_ref())
    }

    pub fn get_object_action_cost(&self, obj: WorldObject) -> Option<&ObjectActionCost> {
        self.get_world_object_def(obj)
            .and_then(|d| d.object_action_cost.as_ref())
    }

    pub fn get_combat_alignment(&self, mob: Mob) -> Option<&CombatAlignment> {
        self.get_mob_def(mob)
            .and_then(|d| d.combat_alignment.as_ref())
    }

    pub fn get_ranged_attack(&self, obj: WorldObject) -> Option<&RangedAttack> {
        self.is_item_ranged_weapon(obj)
    }
}
