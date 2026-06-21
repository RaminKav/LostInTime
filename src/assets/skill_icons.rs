use bevy::prelude::*;
use bevy::utils::HashMap;
use strum::IntoEnumIterator;
use strum_macros::Display;

use crate::pets::state::Pet;
use crate::player::skills::{ActiveSkill, SkillClass};

pub const SKILL_ICONS_DIR: &str = "ui/SkillIcons";

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Display)]
pub enum SkillIcon {
    Active(ActiveSkill),
    ClassPassive(SkillClass),
    PetActive(Pet),
    PetPassive(Pet),
}

impl SkillIcon {
    pub fn asset_path(self) -> String {
        match self {
            SkillIcon::Active(skill) => {
                let name = active_skill_icon_stem(skill);
                format!("{SKILL_ICONS_DIR}/{name}Icon.png")
            }
            SkillIcon::ClassPassive(class) => {
                format!("{SKILL_ICONS_DIR}/Passive{class}Icon.png")
            }
            SkillIcon::PetActive(pet) => {
                format!("{SKILL_ICONS_DIR}/{}ActiveIcon.png", pet_icon_stem(pet))
            }
            SkillIcon::PetPassive(pet) => {
                format!("{SKILL_ICONS_DIR}/{}PassiveIcon.png", pet_icon_stem(pet))
            }
        }
    }
}

/// Asset filename stem for an active skill (`SpinAttack` -> `SpinAttackIcon.png`).
fn active_skill_icon_stem(skill: ActiveSkill) -> String {
    match skill {
        // Legacy art uses `RapidFire` while the enum variant is `Rapidfire`.
        ActiveSkill::Rapidfire => "RapidFire".to_string(),
        _ => skill.to_string(),
    }
}

/// Maps [`Pet`] variants to the icon filename prefix in `assets/ui/SkillIcons/`.
fn pet_icon_stem(pet: Pet) -> &'static str {
    match pet {
        Pet::Slime => "Slime",
        Pet::Fairy => "Fairy",
        Pet::Porkipine => "Porkupine",
        Pet::GoldenPig => "Pig",
        Pet::Goliath => "Cyclops",
    }
}

pub fn load_skill_icons(asset_server: &AssetServer) -> HashMap<SkillIcon, Handle<Image>> {
    let mut handles = HashMap::default();

    for skill in ActiveSkill::iter() {
        let icon = SkillIcon::Active(skill);
        handles.insert(icon, asset_server.load(icon.asset_path()));
    }

    for class in SkillClass::iter() {
        if class == SkillClass::None {
            continue;
        }
        let icon = SkillIcon::ClassPassive(class);
        handles.insert(icon, asset_server.load(icon.asset_path()));
    }

    for pet in Pet::iter() {
        for icon in [SkillIcon::PetActive(pet), SkillIcon::PetPassive(pet)] {
            handles.insert(icon, asset_server.load(icon.asset_path()));
        }
    }

    handles
}
