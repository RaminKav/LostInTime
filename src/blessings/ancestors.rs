use std::collections::HashMap;

use bevy::prelude::*;
use rand::seq::{IteratorRandom, SliceRandom};
use rand::Rng;
use strum::IntoEnumIterator;

use crate::{
    colors::{LIGHT_BLUE, LIGHT_GREY, LIGHT_RED, RARE_TOOLTIP_TITLE, YELLOW_2},
    item::{active_skill_shrine::roll_active_skill_shrine_offer_skills, WorldObject},
    player::skills::{
        ActiveSkill, Heirloom, HeirloomChoiceQueue, HeirloomRarity, HeirloomWithRarity,
        PlayerSkills,
    },
    ui::tooltip_info_boxes::TooltipDefinition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ancestor {
    Resources,
    Heirlooms,
    Weapons,
    Skills,
    Chaos,
}

impl Ancestor {
    pub fn weight(&self) -> u32 {
        match self {
            Ancestor::Chaos => 3,
            _ => 10,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Ancestor::Resources => "King Richard",
            Ancestor::Heirlooms => "The Collector",
            Ancestor::Weapons => "Gunter, Void Slayer",
            Ancestor::Skills => "Ophelia the Sorcerer",
            Ancestor::Chaos => "The Lost One",
        }
    }

    pub fn title_color(&self) -> Color {
        match self {
            Ancestor::Heirlooms => RARE_TOOLTIP_TITLE,
            Ancestor::Resources => YELLOW_2,
            Ancestor::Chaos => LIGHT_RED,
            Ancestor::Weapons => LIGHT_GREY,
            Ancestor::Skills => LIGHT_BLUE,
        }
    }

    pub fn pool(&self) -> Vec<AncestorBlessing> {
        match self {
            Ancestor::Resources => vec![
                AncestorBlessing::ThreeTomes,
                AncestorBlessing::ThreeOrbs,
                AncestorBlessing::TwoOrbsTwoTomes,
                AncestorBlessing::FiftyGold,
                AncestorBlessing::ThreeStatFoods,
                AncestorBlessing::ThreeRerolls,
                AncestorBlessing::TwoBanishes,
            ],
            Ancestor::Heirlooms => vec![
                AncestorBlessing::ThreeCommonHeirlooms,
                AncestorBlessing::OneUncommonHeirloom,
                AncestorBlessing::SpecificUncommon,
                AncestorBlessing::TwoOfSpecificCommon,
            ],
            Ancestor::Weapons => vec![
                AncestorBlessing::UpgradeStartingWeapon,
                AncestorBlessing::RandomWeapon,
                AncestorBlessing::RandomEquipment,
                AncestorBlessing::RandomAccessory,
                AncestorBlessing::ReplaceWithSpecificWeapon,
                AncestorBlessing::RandomWeaponHeirloom,
            ],
            Ancestor::Skills => vec![
                AncestorBlessing::RandomSkill,
                AncestorBlessing::SpecificSkill,
                AncestorBlessing::SkillHeirloom,
            ],
            Ancestor::Chaos => vec![
                AncestorBlessing::PlasmaWeapon,
                AncestorBlessing::LaserBeam,
                AncestorBlessing::SpecificRareHeirloom,
                AncestorBlessing::TwoRandomRareHeirlooms,
                AncestorBlessing::FiveOfRandomCommon,
                AncestorBlessing::ThreeOfRandomUncommon,
                AncestorBlessing::RandomRareEquipment,
            ],
        }
    }

    pub fn roll_weighted(rng: &mut impl Rng) -> Self {
        let ancestors = [
            Ancestor::Resources,
            Ancestor::Heirlooms,
            Ancestor::Weapons,
            Ancestor::Skills,
            Ancestor::Chaos,
        ];
        let total_weight: u32 = ancestors.iter().map(|a| a.weight()).sum();
        let mut roll = rng.gen_range(0..total_weight);
        for ancestor in ancestors {
            if roll < ancestor.weight() {
                return ancestor;
            }
            roll -= ancestor.weight();
        }
        Ancestor::Resources
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AncestorBlessing {
    // Resources
    ThreeTomes,
    ThreeOrbs,
    TwoOrbsTwoTomes,
    FiftyGold,
    ThreeStatFoods,
    ThreeRerolls,
    TwoBanishes,
    // Heirlooms
    ThreeCommonHeirlooms,
    OneUncommonHeirloom,
    SpecificUncommon,
    TwoOfSpecificCommon,
    // Weapons
    UpgradeStartingWeapon,
    RandomWeapon,
    RandomEquipment,
    RandomAccessory,
    ReplaceWithSpecificWeapon,
    RandomWeaponHeirloom,
    // Skills
    RandomSkill,
    SpecificSkill,
    SkillHeirloom,
    // Chaos
    PlasmaWeapon,
    LaserBeam,
    SpecificRareHeirloom,
    TwoRandomRareHeirlooms,
    FiveOfRandomCommon,
    ThreeOfRandomUncommon,
    RandomRareEquipment,
}

impl AncestorBlessing {
    pub fn is_repeatable(&self) -> bool {
        matches!(
            self,
            AncestorBlessing::SpecificUncommon
                | AncestorBlessing::TwoOfSpecificCommon
                | AncestorBlessing::SpecificSkill
                | AncestorBlessing::SpecificRareHeirloom
        )
    }

    pub fn max_hp_penalty_pct(&self) -> f32 {
        match self {
            AncestorBlessing::PlasmaWeapon | AncestorBlessing::LaserBeam => 0.40,
            AncestorBlessing::RandomRareEquipment => 0.20,
            AncestorBlessing::TwoRandomRareHeirlooms | AncestorBlessing::ThreeOfRandomUncommon => {
                0.30
            }
            AncestorBlessing::SpecificRareHeirloom | AncestorBlessing::FiveOfRandomCommon => 0.25,
            _ => 0.0,
        }
    }

    pub fn starting_chaos(&self) -> f32 {
        match self {
            AncestorBlessing::PlasmaWeapon | AncestorBlessing::LaserBeam => 1.0,
            AncestorBlessing::TwoRandomRareHeirlooms
            | AncestorBlessing::ThreeOfRandomUncommon
            | AncestorBlessing::RandomRareEquipment => 1.0,
            AncestorBlessing::SpecificRareHeirloom | AncestorBlessing::FiveOfRandomCommon => 1.0,
            _ => 0.0,
        }
    }

    pub fn base_title(&self) -> &'static str {
        match self {
            AncestorBlessing::ThreeTomes => "Tome Cache",
            AncestorBlessing::ThreeOrbs => "Orb Cache",
            AncestorBlessing::TwoOrbsTwoTomes => "Mixed Upgrades",
            AncestorBlessing::FiftyGold => "Golden Start",
            AncestorBlessing::ThreeStatFoods => "Feast",
            AncestorBlessing::ThreeRerolls => "Second Chances",
            AncestorBlessing::TwoBanishes => "Curator's Eye",
            AncestorBlessing::ThreeCommonHeirlooms => "Common Gifts",
            AncestorBlessing::OneUncommonHeirloom => "Uncommon Gift",
            AncestorBlessing::SpecificUncommon => "Chosen Uncommon",
            AncestorBlessing::TwoOfSpecificCommon => "Double Down",
            AncestorBlessing::UpgradeStartingWeapon => "Forged Better",
            AncestorBlessing::RandomWeapon => "New Weapon",
            AncestorBlessing::RandomEquipment => "New Armor",
            AncestorBlessing::RandomAccessory => "New Trinket",
            AncestorBlessing::ReplaceWithSpecificWeapon => "Sacrifice & Gain",
            AncestorBlessing::RandomWeaponHeirloom => "Weapon Heirloom",
            AncestorBlessing::RandomSkill => "New Skill",
            AncestorBlessing::SpecificSkill => "Chosen Skill",
            AncestorBlessing::SkillHeirloom => "Skill Heirloom",
            AncestorBlessing::PlasmaWeapon => "Plasma Pact",
            AncestorBlessing::LaserBeam => "Laser Pact",
            AncestorBlessing::SpecificRareHeirloom => "Royal Pact",
            AncestorBlessing::TwoRandomRareHeirlooms => "Twin Rares",
            AncestorBlessing::FiveOfRandomCommon => "Common Cache",
            AncestorBlessing::ThreeOfRandomUncommon => "Uncommon Cache",
            AncestorBlessing::RandomRareEquipment => "Rare Spoils",
        }
    }

    pub fn display_card_rarity(&self) -> Option<HeirloomRarity> {
        match self {
            AncestorBlessing::ThreeCommonHeirlooms => Some(HeirloomRarity::Common),
            AncestorBlessing::OneUncommonHeirloom => Some(HeirloomRarity::Uncommon),
            AncestorBlessing::TwoRandomRareHeirlooms => Some(HeirloomRarity::Rare),
            _ => None,
        }
    }

    pub fn hides_resolved_reward_from_player(&self) -> bool {
        matches!(
            self,
            AncestorBlessing::RandomWeapon
                | AncestorBlessing::RandomEquipment
                | AncestorBlessing::RandomAccessory
                | AncestorBlessing::OneUncommonHeirloom
                | AncestorBlessing::ThreeCommonHeirlooms
                | AncestorBlessing::ReplaceWithSpecificWeapon
                | AncestorBlessing::RandomWeaponHeirloom
                | AncestorBlessing::SkillHeirloom
                | AncestorBlessing::RandomSkill
                | AncestorBlessing::FiveOfRandomCommon
                | AncestorBlessing::ThreeOfRandomUncommon
                | AncestorBlessing::ThreeStatFoods
                | AncestorBlessing::RandomRareEquipment
        )
    }

    pub fn base_description(&self) -> Vec<&'static str> {
        match self {
            AncestorBlessing::ThreeTomes => vec!["Start with 3 upgrade", "tomes."],
            AncestorBlessing::ThreeOrbs => vec!["Start with 3 orbs", "of transformation."],
            AncestorBlessing::TwoOrbsTwoTomes => vec!["Start with 2 orbs", "and 2 upgrade tomes."],
            AncestorBlessing::FiftyGold => vec!["Start with", "50 gold."],
            AncestorBlessing::ThreeStatFoods => {
                vec!["Start with 3 random", "stat boosts."]
            }
            AncestorBlessing::ThreeRerolls => vec!["Start with 3 extra", "rerolls this run."],
            AncestorBlessing::TwoBanishes => vec!["Start with 2 extra", "banishes this run."],
            AncestorBlessing::ThreeCommonHeirlooms => {
                vec!["Start with 3 random", "common heirlooms."]
            }
            AncestorBlessing::OneUncommonHeirloom => {
                vec!["Start with 1 random", "uncommon heirloom."]
            }
            AncestorBlessing::SpecificUncommon => {
                vec!["Start with a specific", "uncommon heirloom."]
            }
            AncestorBlessing::TwoOfSpecificCommon => {
                vec!["Start with 2 copies of", "a specific common."]
            }
            AncestorBlessing::UpgradeStartingWeapon => {
                vec!["Upgrade starting", "weapon one tier."]
            }
            AncestorBlessing::RandomWeapon => vec!["Start with a random", "weapon."],
            AncestorBlessing::RandomEquipment => vec!["Start with a random", "equipment."],
            AncestorBlessing::RandomAccessory => vec!["Start with a random", "accessory."],
            AncestorBlessing::ReplaceWithSpecificWeapon => {
                vec![
                    "Lose starting weapon.",
                    "Start with a random",
                    "weapon one tier higher.",
                ]
            }
            AncestorBlessing::RandomWeaponHeirloom => {
                vec!["Start with a random", "weapon heirloom."]
            }
            AncestorBlessing::RandomSkill => vec!["Start with a random", "extra skill."],
            AncestorBlessing::SpecificSkill => vec!["Start with a specific", "extra skill."],
            AncestorBlessing::SkillHeirloom => vec!["Start with a", "skill heirloom."],
            AncestorBlessing::PlasmaWeapon => {
                vec!["Start with plasma staff.", "Powerful, but costs mana."]
            }
            AncestorBlessing::LaserBeam => vec!["Start with the laser.", "beam skill."],
            AncestorBlessing::SpecificRareHeirloom => {
                vec!["Start with a specific", "rare heirloom."]
            }
            AncestorBlessing::TwoRandomRareHeirlooms => {
                vec!["Start with 2 random", "rare heirlooms."]
            }
            AncestorBlessing::FiveOfRandomCommon => {
                vec!["Start with 5 copies of", "a random common."]
            }
            AncestorBlessing::ThreeOfRandomUncommon => {
                vec!["Start with 3 copies of", "a random uncommon."]
            }
            AncestorBlessing::RandomRareEquipment => {
                vec!["Start with a random", "rare equipment."]
            }
        }
    }

    pub fn needs_heirloom_reveal_delay(&self) -> bool {
        matches!(
            self,
            AncestorBlessing::ThreeCommonHeirlooms
                | AncestorBlessing::OneUncommonHeirloom
                | AncestorBlessing::SpecificUncommon
                | AncestorBlessing::TwoOfSpecificCommon
                | AncestorBlessing::RandomWeaponHeirloom
                | AncestorBlessing::SkillHeirloom
                | AncestorBlessing::SpecificRareHeirloom
                | AncestorBlessing::TwoRandomRareHeirlooms
                | AncestorBlessing::FiveOfRandomCommon
                | AncestorBlessing::ThreeOfRandomUncommon
        )
    }

    /// Card description naming a specific heirloom, skill, or item reward (not effect text).
    pub fn reward_description_with_name(&self, name: &str) -> Vec<String> {
        match self {
            AncestorBlessing::SpecificUncommon => {
                vec!["Start with".to_string(), format!("{name}.")]
            }
            AncestorBlessing::TwoOfSpecificCommon => {
                vec!["Start with 2 copies of".to_string(), format!("{name}.")]
            }
            AncestorBlessing::SpecificRareHeirloom => {
                vec!["Start with".to_string(), format!("{name}.")]
            }
            AncestorBlessing::FiveOfRandomCommon => {
                vec!["Start with 5 copies of".to_string(), format!("{name}.")]
            }
            AncestorBlessing::ThreeOfRandomUncommon => {
                vec!["Start with 3 copies of".to_string(), format!("{name}.")]
            }
            AncestorBlessing::RandomWeaponHeirloom | AncestorBlessing::SkillHeirloom => {
                vec!["Start with".to_string(), format!("{name}.")]
            }
            AncestorBlessing::RandomSkill | AncestorBlessing::SpecificSkill => {
                vec!["Start with".to_string(), format!("{name}.")]
            }
            AncestorBlessing::LaserBeam => {
                vec!["Start with".to_string(), format!("{name}.")]
            }
            AncestorBlessing::ReplaceWithSpecificWeapon => {
                vec![
                    "Lose starting weapon.".to_string(),
                    "Start with a random".to_string(),
                    "weapon one tier higher.".to_string(),
                ]
            }
            AncestorBlessing::RandomWeapon
            | AncestorBlessing::RandomEquipment
            | AncestorBlessing::RandomAccessory => self
                .base_description()
                .iter()
                .map(|s| s.to_string())
                .collect(),
            AncestorBlessing::UpgradeStartingWeapon => {
                vec!["Upgrade starting".to_string(), format!("{name} one tier.")]
            }
            _ => self
                .base_description()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum AncestorBlessingIcon {
    Heirloom(Heirloom, HeirloomRarity),
    Skill(ActiveSkill),
    Item(WorldObject),
    Mystery,
}

#[derive(Clone, Debug)]
pub struct ResolvedAncestorBlessing {
    pub blessing: AncestorBlessing,
    pub title: String,
    pub description: Vec<String>,
    pub resolved_heirloom: Option<HeirloomWithRarity>,
    pub resolved_weapon: Option<WorldObject>,
    pub resolved_skill: Option<ActiveSkill>,
    pub resolved_item: Option<WorldObject>,
    pub display_icon: Option<AncestorBlessingIcon>,
}

#[derive(Resource, Clone, Debug)]
pub struct AncestorBlessingOffer {
    pub ancestor: Ancestor,
    pub choices: Vec<ResolvedAncestorBlessing>,
}

pub fn build_ancestor_blessing_offer(
    heirloom_queue: &HeirloomChoiceQueue,
    player_skills: Option<&PlayerSkills>,
    player_level: u8,
    starting_weapon: WorldObject,
) -> AncestorBlessingOffer {
    let mut rng = rand::thread_rng();
    let ancestor = Ancestor::roll_weighted(&mut rng);
    let pool = ancestor.pool();
    let mut counts: HashMap<AncestorBlessing, u32> = HashMap::new();
    let mut choices = Vec::new();
    // Track random rolls already shown to the player so repeatable blessings
    // don't offer the same visible option twice (e.g. two "Chosen Skill"
    // cards both rolling Heal).
    let mut used_skills: Vec<ActiveSkill> = Vec::new();
    let mut used_heirlooms: Vec<Heirloom> = Vec::new();

    for _ in 0..3 {
        let available: Vec<AncestorBlessing> = pool
            .iter()
            .copied()
            .filter(|blessing| {
                let count = counts.get(blessing).copied().unwrap_or(0);
                if blessing.is_repeatable() {
                    count < 2
                } else {
                    count < 1
                }
            })
            .collect();

        if available.is_empty() {
            break;
        }

        let picked = *available.choose(&mut rng).unwrap();
        *counts.entry(picked).or_insert(0) += 1;
        let resolved = resolve_ancestor_blessing(
            picked,
            &mut rng,
            heirloom_queue,
            player_level,
            player_skills,
            starting_weapon,
            &used_skills,
            &used_heirlooms,
        );
        if let Some(skill) = resolved.resolved_skill {
            used_skills.push(skill);
        }
        if let Some(heirloom) = resolved.resolved_heirloom.as_ref() {
            used_heirlooms.push(heirloom.heirloom.clone());
        }
        choices.push(resolved);
    }

    AncestorBlessingOffer { ancestor, choices }
}

pub fn resolve_ancestor_blessing(
    blessing: AncestorBlessing,
    rng: &mut rand::rngs::ThreadRng,
    heirloom_queue: &HeirloomChoiceQueue,
    player_level: u8,
    player_skills: Option<&PlayerSkills>,
    starting_weapon: WorldObject,
    used_skills: &[ActiveSkill],
    used_heirlooms: &[Heirloom],
) -> ResolvedAncestorBlessing {
    let mut resolved_heirloom = None;
    let mut resolved_weapon = None;
    let mut resolved_skill = None;
    let mut resolved_item = None;
    let mut display_icon = None;
    let mut title = blessing.base_title().to_string();
    let mut description: Vec<String> = blessing
        .base_description()
        .iter()
        .map(|s| s.to_string())
        .collect();

    match blessing {
        AncestorBlessing::ThreeTomes => {
            resolved_item = Some(WorldObject::UpgradeTome);
            display_icon = Some(AncestorBlessingIcon::Item(WorldObject::UpgradeTome));
        }
        AncestorBlessing::ThreeOrbs => {
            resolved_item = Some(WorldObject::OrbOfTransformation);
            display_icon = Some(AncestorBlessingIcon::Item(WorldObject::OrbOfTransformation));
        }
        AncestorBlessing::TwoOrbsTwoTomes => {
            resolved_item = Some(WorldObject::UpgradeTome);
            display_icon = Some(AncestorBlessingIcon::Item(WorldObject::UpgradeTome));
        }
        AncestorBlessing::FiftyGold => {
            resolved_item = Some(WorldObject::Coin);
            display_icon = Some(AncestorBlessingIcon::Item(WorldObject::Coin));
        }
        AncestorBlessing::ThreeStatFoods => {
            if stat_food_pool().choose(rng).is_some() {
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::UpgradeStartingWeapon => {
            resolved_weapon = Some(starting_weapon);
            resolved_item = Some(starting_weapon);
            description = blessing.reward_description_with_name(&format!("{starting_weapon:?}"));
            display_icon = Some(AncestorBlessingIcon::Item(starting_weapon));
        }
        AncestorBlessing::RandomWeapon => {
            if let Some(weapon) = WorldObject::iter()
                .filter(|o| o.is_weapon() && *o != WorldObject::PlasmaStaff)
                .choose(rng)
            {
                resolved_item = Some(weapon);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::RandomEquipment => {
            if let Some(equipment) = WorldObject::iter().filter(|o| o.is_armor()).choose(rng) {
                resolved_item = Some(equipment);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::RandomAccessory => {
            if let Some(accessory) = WorldObject::iter().filter(|o| o.is_accessory()).choose(rng) {
                resolved_item = Some(accessory);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::RandomRareEquipment => {
            if let Some(equipment) = WorldObject::iter().filter(|o| o.is_armor()).choose(rng) {
                resolved_item = Some(equipment);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::RandomWeaponHeirloom => {
            if let Some(picked) = heirloom_queue.pick_random_heirloom_with_tooltip(
                TooltipDefinition::Weapons,
                rng,
                player_level,
            ) {
                resolved_heirloom = Some(picked);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::SkillHeirloom => {
            if let Some(picked) = heirloom_queue.pick_random_heirloom_with_tooltip(
                TooltipDefinition::Skills,
                rng,
                player_level,
            ) {
                resolved_heirloom = Some(picked);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::PlasmaWeapon => {
            resolved_item = Some(WorldObject::PlasmaStaff);
            display_icon = Some(AncestorBlessingIcon::Item(WorldObject::PlasmaStaff));
        }
        AncestorBlessing::LaserBeam => {
            resolved_skill = Some(ActiveSkill::LaserBeam);
            description =
                blessing.reward_description_with_name(&ActiveSkill::LaserBeam.get_title());
            display_icon = Some(AncestorBlessingIcon::Skill(ActiveSkill::LaserBeam));
        }
        AncestorBlessing::FiveOfRandomCommon => {
            if let Some(picked) = heirloom_queue.get_skill_of_rarity(
                HeirloomRarity::Common,
                rng,
                player_level,
                &|_| true,
            ) {
                resolved_heirloom = Some(HeirloomWithRarity {
                    heirloom: picked.heirloom.clone(),
                    rarity: picked.rarity.clone(),
                });
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::ThreeOfRandomUncommon => {
            if let Some(picked) = heirloom_queue.get_skill_of_rarity(
                HeirloomRarity::Uncommon,
                rng,
                player_level,
                &|_| true,
            ) {
                resolved_heirloom = Some(HeirloomWithRarity {
                    heirloom: picked.heirloom.clone(),
                    rarity: picked.rarity.clone(),
                });
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::SpecificUncommon => {
            if let Some(picked) = heirloom_queue.get_skill_of_rarity(
                HeirloomRarity::Uncommon,
                rng,
                player_level,
                &|state| !used_heirlooms.contains(&state.heirloom),
            ) {
                resolved_heirloom = Some(HeirloomWithRarity {
                    heirloom: picked.heirloom.clone(),
                    rarity: picked.rarity.clone(),
                });
                description = blessing.reward_description_with_name(&picked.heirloom.get_title());
                display_icon = Some(AncestorBlessingIcon::Heirloom(
                    picked.heirloom.clone(),
                    picked.rarity.clone(),
                ));
            }
        }
        AncestorBlessing::TwoOfSpecificCommon => {
            if let Some(picked) = heirloom_queue.get_skill_of_rarity(
                HeirloomRarity::Common,
                rng,
                player_level,
                &|state| !used_heirlooms.contains(&state.heirloom),
            ) {
                resolved_heirloom = Some(HeirloomWithRarity {
                    heirloom: picked.heirloom.clone(),
                    rarity: picked.rarity.clone(),
                });
                description = blessing.reward_description_with_name(&picked.heirloom.get_title());
                display_icon = Some(AncestorBlessingIcon::Heirloom(
                    picked.heirloom.clone(),
                    picked.rarity.clone(),
                ));
            }
        }
        AncestorBlessing::ReplaceWithSpecificWeapon => {
            if let Some(weapon) = WorldObject::iter()
                .filter(|o| {
                    o.is_weapon() && *o != WorldObject::PlasmaStaff && *o != starting_weapon
                })
                .choose(rng)
            {
                resolved_weapon = Some(weapon);
                resolved_item = Some(weapon);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::SpecificSkill => {
            if let Some(skill) = roll_active_skill_shrine_offer_skills(player_skills)
                .into_iter()
                .filter(|skill| !used_skills.contains(skill))
                .choose(rng)
            {
                resolved_skill = Some(skill);
                description = blessing.reward_description_with_name(&skill.get_title());
                display_icon = Some(AncestorBlessingIcon::Skill(skill));
            }
        }
        AncestorBlessing::SpecificRareHeirloom => {
            if let Some(picked) = heirloom_queue.get_skill_of_rarity(
                HeirloomRarity::Rare,
                rng,
                player_level,
                &|state| !used_heirlooms.contains(&state.heirloom),
            ) {
                resolved_heirloom = Some(HeirloomWithRarity {
                    heirloom: picked.heirloom.clone(),
                    rarity: picked.rarity.clone(),
                });
                description = blessing.reward_description_with_name(&picked.heirloom.get_title());
                display_icon = Some(AncestorBlessingIcon::Heirloom(
                    picked.heirloom.clone(),
                    picked.rarity.clone(),
                ));
            }
        }
        AncestorBlessing::RandomSkill => {
            if let Some(skill) = roll_active_skill_shrine_offer_skills(player_skills)
                .choose(rng)
                .copied()
            {
                resolved_skill = Some(skill);
                display_icon = Some(AncestorBlessingIcon::Mystery);
            }
        }
        AncestorBlessing::ThreeCommonHeirlooms
        | AncestorBlessing::OneUncommonHeirloom
        | AncestorBlessing::TwoRandomRareHeirlooms => {
            display_icon = Some(AncestorBlessingIcon::Mystery);
        }
        AncestorBlessing::ThreeRerolls | AncestorBlessing::TwoBanishes => {}
    }

    if display_icon.is_none() && !blessing.hides_resolved_reward_from_player() {
        display_icon = resolved_heirloom
            .as_ref()
            .map(|h| AncestorBlessingIcon::Heirloom(h.heirloom.clone(), h.rarity.clone()))
            .or_else(|| resolved_skill.map(|skill| AncestorBlessingIcon::Skill(skill)))
            .or_else(|| resolved_item.map(|item| AncestorBlessingIcon::Item(item)))
            .or_else(|| resolved_weapon.map(|weapon| AncestorBlessingIcon::Item(weapon)));
    }

    ResolvedAncestorBlessing {
        blessing,
        title,
        description,
        resolved_heirloom,
        resolved_weapon,
        resolved_skill,
        resolved_item,
        display_icon,
    }
}

pub fn heirlooms_with_tooltip(tooltip: TooltipDefinition) -> Vec<Heirloom> {
    Heirloom::iter()
        .filter(|h| h.tooltip_definitions().contains(&tooltip))
        .collect()
}

pub fn stat_food_pool() -> Vec<WorldObject> {
    vec![
        WorldObject::SpeedFood,
        WorldObject::HealthFood,
        WorldObject::ManaFood,
        WorldObject::ThornsFood,
        WorldObject::CritChanceFood,
        WorldObject::LifestealFood,
        WorldObject::SkillPowerFood,
        WorldObject::ManaRegenFood,
        WorldObject::DodgeFood,
        WorldObject::DefenceFood,
        WorldObject::SizeFood,
        WorldObject::AttackSpeedFood,
    ]
}
