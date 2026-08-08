use std::collections::HashMap;

use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::Rng;
use strum_macros::{EnumIter, IntoStaticStr};

use crate::{
    attributes::ItemRarity,
    blessings::{
        ancestors::Ancestor, format_effect_pool_description, format_stat_conversion_description,
        roll_stat_conversion_pair, EffectPoolStatus, HeirloomEffectFamily, ResolvedStatConversion,
    },
    player::{
        skills::{HeirloomRarity, PlayerSkills},
        unlocks::RunUnlockState,
    },
};

/// Mid-run major blessing tier. Offered after first Era 1 / Era 2 boss kills.
#[derive(
    Debug,
    Reflect,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Hash,
    Component,
    IntoStaticStr,
    Ord,
    PartialOrd,
    EnumIter,
)]
pub enum MajorBlessing {
    // Heirlooms
    LightningCoinChance,
    EchoSizeBoost,
    PoisonTickFaster,
    HeirloomDamageBoost,
    ExtraManaRegen,
    TripleUncommonHeirloom,
    SummonRetrigger,
    TouchThorns,
    EchoAftershock,
    ViralConductor,
    HeirloomOverclock,
    SharedAffliction,
    ConvertUncommons,
    WeaponHeirloomDoubleTrigger,
    // Weapons
    AttackSpeedBoost,
    RandomLegendaryWeapon,
    RandomLegendaryArmor,
    RandomLegendaryAccessory,
    PetSizeAndAttackSpeed,
    EffectPoolApplyFrail,
    StatusApplyShield,
    // Skills
    SkillDamageBoost,
    SkillCooldownCut,
    SkillPoisonStacks,
    MovementSkillSummons,
    IceExplosionChain,
    EffectPoolApplyFreeze,
    EffectPoolApplyPoison,
    ManaRegenHeal,
    SkillsApplyAllStatuses,
    StatusApplyExtra,
    // Chaos
    StatConversion,
    LuckChaosTradeoff,
    ManaDrainShield,
    OverhealToShield,
    // Materials (Resources ancestor)
    MerchantSlotReplenish,
    TomesAndOrbs,
    RerollsAndBanishes,
    CoinDropRate,
    LargeObjectEcho,
    ObjectBreakLoot,
}

impl MajorBlessing {
    pub fn ancestor(&self) -> Ancestor {
        match self {
            MajorBlessing::LightningCoinChance
            | MajorBlessing::EchoSizeBoost
            | MajorBlessing::PoisonTickFaster
            | MajorBlessing::HeirloomDamageBoost
            | MajorBlessing::ExtraManaRegen
            | MajorBlessing::TripleUncommonHeirloom
            | MajorBlessing::SummonRetrigger
            | MajorBlessing::TouchThorns
            | MajorBlessing::EchoAftershock
            | MajorBlessing::ViralConductor
            | MajorBlessing::HeirloomOverclock
            | MajorBlessing::SharedAffliction
            | MajorBlessing::ConvertUncommons
            | MajorBlessing::WeaponHeirloomDoubleTrigger => Ancestor::Heirlooms,
            MajorBlessing::AttackSpeedBoost
            | MajorBlessing::RandomLegendaryWeapon
            | MajorBlessing::RandomLegendaryArmor
            | MajorBlessing::RandomLegendaryAccessory
            | MajorBlessing::PetSizeAndAttackSpeed
            | MajorBlessing::EffectPoolApplyFrail
            | MajorBlessing::StatusApplyShield => Ancestor::Weapons,
            MajorBlessing::SkillDamageBoost
            | MajorBlessing::SkillCooldownCut
            | MajorBlessing::SkillPoisonStacks
            | MajorBlessing::MovementSkillSummons
            | MajorBlessing::IceExplosionChain
            | MajorBlessing::EffectPoolApplyFreeze
            | MajorBlessing::EffectPoolApplyPoison
            | MajorBlessing::ManaRegenHeal
            | MajorBlessing::SkillsApplyAllStatuses
            | MajorBlessing::StatusApplyExtra => Ancestor::Skills,
            MajorBlessing::StatConversion
            | MajorBlessing::LuckChaosTradeoff
            | MajorBlessing::ManaDrainShield
            | MajorBlessing::OverhealToShield => Ancestor::Chaos,
            MajorBlessing::MerchantSlotReplenish
            | MajorBlessing::TomesAndOrbs
            | MajorBlessing::RerollsAndBanishes
            | MajorBlessing::CoinDropRate
            | MajorBlessing::LargeObjectEcho
            | MajorBlessing::ObjectBreakLoot => Ancestor::Resources,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            MajorBlessing::LightningCoinChance => "Charged Fortune",
            MajorBlessing::EchoSizeBoost => "Resounding Echo",
            MajorBlessing::PoisonTickFaster => "Virulent Pace",
            MajorBlessing::HeirloomDamageBoost => "Collector's Fury",
            MajorBlessing::ExtraManaRegen => "Overflowing Mind",
            MajorBlessing::TripleUncommonHeirloom => "Collector's Bargain",
            MajorBlessing::SummonRetrigger => "Echoing Summons",
            MajorBlessing::TouchThorns => "Barbed Embrace",
            MajorBlessing::EchoAftershock => "Echo Aftershock",
            MajorBlessing::ViralConductor => "Viral Conductor",
            MajorBlessing::HeirloomOverclock => "Heirloom Overclock",
            MajorBlessing::SharedAffliction => "Shared Affliction",
            MajorBlessing::ConvertUncommons => "Singular Focus",
            MajorBlessing::WeaponHeirloomDoubleTrigger => "Twin Strike Relics",
            MajorBlessing::AttackSpeedBoost => "Blazing Tempo",
            MajorBlessing::RandomLegendaryWeapon => "Legendary Blade",
            MajorBlessing::RandomLegendaryArmor => "Legendary Plate",
            MajorBlessing::RandomLegendaryAccessory => "Legendary Charm",
            MajorBlessing::PetSizeAndAttackSpeed => "Giant Companion",
            MajorBlessing::EffectPoolApplyFrail => "Crippling Cascade",
            MajorBlessing::StatusApplyShield => "Aegis of Affliction",
            MajorBlessing::SkillDamageBoost => "Arcane Mastery",
            MajorBlessing::SkillCooldownCut => "Swift Casting",
            MajorBlessing::SkillPoisonStacks => "Venomous Arts",
            MajorBlessing::MovementSkillSummons => "Summoner's Step",
            MajorBlessing::IceExplosionChain => "Frost Cascade",
            MajorBlessing::EffectPoolApplyFreeze => "Creeping Frost",
            MajorBlessing::EffectPoolApplyPoison => "Spreading Blight",
            MajorBlessing::ManaRegenHeal => "Vital Current",
            MajorBlessing::SkillsApplyAllStatuses => "Triune Hex",
            MajorBlessing::StatusApplyExtra => "Compounding Malady",
            MajorBlessing::StatConversion => "Twisted Exchange",
            MajorBlessing::LuckChaosTradeoff => "Fool's Gambit",
            MajorBlessing::ManaDrainShield => "Mana Barrier",
            MajorBlessing::OverhealToShield => "Overflowing Guard",
            MajorBlessing::MerchantSlotReplenish => "Merchant's Favor",
            MajorBlessing::TomesAndOrbs => "Upgrade Cache",
            MajorBlessing::RerollsAndBanishes => "Choice Surplus",
            MajorBlessing::CoinDropRate => "Gilded Path",
            MajorBlessing::LargeObjectEcho => "Resonant Ruin",
            MajorBlessing::ObjectBreakLoot => "Scavenger's Eye",
        }
    }

    pub fn description(&self) -> Vec<&'static str> {
        match self {
            MajorBlessing::LightningCoinChance => {
                vec![
                    "Lightning strikes have",
                    "a 15% chance to create",
                    "a coin.",
                ]
            }
            MajorBlessing::EchoSizeBoost => vec!["Echoes gain +25% size."],
            MajorBlessing::PoisonTickFaster => vec!["Poison ticks 30% faster."],
            MajorBlessing::HeirloomDamageBoost => {
                vec!["All Heirloom effects", "gain +15% damage."]
            }
            MajorBlessing::ExtraManaRegen => {
                vec!["Whenever you trigger", "Mana Regen, regen 1", "extra mana."]
            }
            MajorBlessing::TripleUncommonHeirloom => {
                vec![
                    "Pick an uncommon heirloom,",
                    "gain 3 copies of it.",
                    "Lose 1 random uncommon.",
                ]
            }
            MajorBlessing::SummonRetrigger => {
                vec![
                    "Heirloom Summon triggers",
                    "have a 30% chance to",
                    "summon again.",
                ]
            }
            MajorBlessing::TouchThorns => {
                vec!["Deal thorns damage to", "enemies when you touch", "them."]
            }
            MajorBlessing::AttackSpeedBoost => {
                vec!["Gain 25% more attack", "speed from all sources."]
            }
            MajorBlessing::RandomLegendaryWeapon => {
                vec!["Gain a random", "legendary weapon."]
            }
            MajorBlessing::RandomLegendaryArmor => {
                vec!["Gain a random", "legendary armor piece."]
            }
            MajorBlessing::RandomLegendaryAccessory => {
                vec!["Gain a random", "legendary accessory."]
            }
            MajorBlessing::PetSizeAndAttackSpeed => {
                vec![
                    "Your pet attacks gain",
                    "+100% size and",
                    "50% attack speed.",
                ]
            }
            MajorBlessing::SkillDamageBoost => {
                vec!["Gain 35% more skill", "damage from all sources."]
            }
            MajorBlessing::SkillCooldownCut => {
                vec!["Base skill cooldowns are", "reduced by 2s (min 0.5)."]
            }
            MajorBlessing::SkillPoisonStacks => {
                vec!["Skill damage applies", "5 poison stacks."]
            }
            MajorBlessing::MovementSkillSummons => {
                vec![
                    "When you use your",
                    "class movement skill,",
                    "trigger all summon",
                    "heirloom effects.",
                ]
            }
            MajorBlessing::IceExplosionChain => {
                vec![
                    "Enemies killed by",
                    "Ice Explosions have a",
                    "40% chance to trigger",
                    "another Ice Explosion.",
                ]
            }
            MajorBlessing::StatConversion => {
                vec!["Gain +X StatA per Y StatB.", "Gain +1 Chaos per 5Y StatB."]
            }
            MajorBlessing::LuckChaosTradeoff => {
                vec!["Gain 50 Luck.", "-25% Max HP.", "+5 Chaos."]
            }
            MajorBlessing::ManaDrainShield => {
                vec![
                    "Every 2.5s drain 50% of",
                    "your remaining mana to",
                    "gain a shield equal to",
                    "the amount drained.",
                ]
            }
            MajorBlessing::MerchantSlotReplenish => {
                vec![
                    "Purchased merchant",
                    "slots replenish on",
                    "reroll.",
                    "",
                    "Gain 3 rerolls.",
                ]
            }
            MajorBlessing::TomesAndOrbs => {
                vec!["Gain 10 upgrade tomes.", "", "Gain 10 orbs."]
            }
            MajorBlessing::RerollsAndBanishes => {
                vec!["Gain 7 Rerolls.", "", "Gain 3 Banishes."]
            }
            MajorBlessing::CoinDropRate => {
                vec!["Enemies have a +20%", "coin drop rate."]
            }
            MajorBlessing::EchoAftershock => {
                vec![
                    "Echoes spawn a second",
                    "copy that stays where",
                    "they were triggered.",
                ]
            }
            MajorBlessing::ViralConductor => {
                vec![
                    "Poison ticks have a 5%",
                    "chance to trigger",
                    "Lightning on a nearby",
                    "enemy.",
                ]
            }
            MajorBlessing::HeirloomOverclock => {
                vec!["Every 5th heirloom", "triggers that costs mana", "is free."]
            }
            MajorBlessing::SharedAffliction => {
                vec!["Applying a freeze stack", "also applies a poison", "stack."]
            }
            MajorBlessing::ConvertUncommons => {
                vec![
                    "Pick an uncommon",
                    "heirloom. Convert all",
                    "other uncommons into",
                    "copies of it.",
                ]
            }
            MajorBlessing::WeaponHeirloomDoubleTrigger => {
                vec!["Weapon-attack heirlooms", "trigger an extra time."]
            }
            MajorBlessing::EffectPoolApplyFrail => {
                vec!["Your ___ apply", "frail."]
            }
            MajorBlessing::StatusApplyShield => {
                vec!["Whenever you apply a", "status effect, gain 1", "shield."]
            }
            MajorBlessing::EffectPoolApplyFreeze => {
                vec!["Your ___ apply", "freeze."]
            }
            MajorBlessing::EffectPoolApplyPoison => {
                vec!["Your ___ apply", "poison."]
            }
            MajorBlessing::ManaRegenHeal => {
                vec!["Mana Regen heals you", "for 1 HP."]
            }
            MajorBlessing::SkillsApplyAllStatuses => {
                vec!["Skills apply 1 freeze,", "poison, and frail."]
            }
            MajorBlessing::StatusApplyExtra => {
                vec!["Whenever you apply a", "status effect, apply 1", "more."]
            }
            MajorBlessing::OverhealToShield => {
                vec![
                    "Healing at full HP",
                    "converts to shields,",
                    "up to 50% of max HP.",
                    "",
                    "-30% Max HP.",
                ]
            }
            MajorBlessing::LargeObjectEcho => {
                vec!["Breaking an object", "triggers an Echo at its", "location."]
            }
            MajorBlessing::ObjectBreakLoot => {
                vec!["Breaking objects has a", "chance to drop loot!"]
            }
        }
    }

    pub fn display_card_rarity(&self) -> Option<HeirloomRarity> {
        // Temporary: all major cards use the rare frame for visual consistency.
        Some(HeirloomRarity::Rare)
    }

    pub fn max_hp_penalty_pct(&self) -> f32 {
        match self {
            MajorBlessing::LuckChaosTradeoff => 0.25,
            MajorBlessing::OverhealToShield => 0.30,
            _ => 0.0,
        }
    }

    pub fn starting_chaos(&self) -> f32 {
        match self {
            MajorBlessing::LuckChaosTradeoff => 5.0,
            _ => 0.0,
        }
    }

    pub fn needs_heirloom_pick_ui(&self) -> bool {
        matches!(
            self,
            MajorBlessing::TripleUncommonHeirloom | MajorBlessing::ConvertUncommons
        )
    }

    /// Discrete proc blessings that show a "Triggered N times" info box (not passive stats).
    pub fn tracks_triggers(&self) -> bool {
        matches!(
            self,
            MajorBlessing::LightningCoinChance
                | MajorBlessing::SummonRetrigger
                | MajorBlessing::TouchThorns
                | MajorBlessing::EchoAftershock
                | MajorBlessing::ViralConductor
                | MajorBlessing::HeirloomOverclock
                | MajorBlessing::SharedAffliction
                | MajorBlessing::WeaponHeirloomDoubleTrigger
                | MajorBlessing::EffectPoolApplyFrail
                | MajorBlessing::EffectPoolApplyFreeze
                | MajorBlessing::EffectPoolApplyPoison
                | MajorBlessing::StatusApplyShield
                | MajorBlessing::StatusApplyExtra
                | MajorBlessing::SkillsApplyAllStatuses
                | MajorBlessing::SkillPoisonStacks
                | MajorBlessing::MovementSkillSummons
                | MajorBlessing::IceExplosionChain
                | MajorBlessing::ManaRegenHeal
                | MajorBlessing::ManaDrainShield
                | MajorBlessing::OverhealToShield
                | MajorBlessing::LargeObjectEcho
                | MajorBlessing::ObjectBreakLoot
        )
    }

    pub fn rolls_effect_family(&self) -> Option<EffectPoolStatus> {
        match self {
            MajorBlessing::EffectPoolApplyFreeze => Some(EffectPoolStatus::Freeze),
            MajorBlessing::EffectPoolApplyFrail => Some(EffectPoolStatus::Frail),
            MajorBlessing::EffectPoolApplyPoison => Some(EffectPoolStatus::Poison),
            _ => None,
        }
    }

    /// Whether this blessing can appear in the current offer.
    pub fn is_offerable(
        &self,
        owned: &OwnedMajorBlessings,
        player_skills: Option<&PlayerSkills>,
    ) -> bool {
        if owned.has(*self) {
            return false;
        }
        match self {
            MajorBlessing::MerchantSlotReplenish => {
                !owned.has(MajorBlessing::MerchantSlotReplenish)
            }
            MajorBlessing::TripleUncommonHeirloom | MajorBlessing::ConvertUncommons => {
                let Some(skills) = player_skills else {
                    return false;
                };
                let distinct_uncommons = skills
                    .heirlooms
                    .iter()
                    .filter(|h| h.rarity == HeirloomRarity::Uncommon)
                    .map(|h| &h.heirloom)
                    .collect::<std::collections::HashSet<_>>()
                    .len();
                distinct_uncommons >= 2
            }
            _ => true,
        }
    }
}

impl Ancestor {
    pub fn major_pool(&self) -> Vec<MajorBlessing> {
        use strum::IntoEnumIterator;
        MajorBlessing::iter()
            .filter(|b| b.ancestor() == *self)
            .collect()
    }
}

/// Tracks how many times each major blessing has successfully triggered during a run.
#[derive(Resource, Default, Clone, Debug)]
pub struct BlessingTriggerCounts {
    pub counts: HashMap<MajorBlessing, u32>,
}

impl BlessingTriggerCounts {
    pub fn increment(&mut self, blessing: MajorBlessing) {
        if !blessing.tracks_triggers() {
            return;
        }
        *self.counts.entry(blessing).or_insert(0) += 1;
    }

    pub fn get(&self, blessing: &MajorBlessing) -> u32 {
        self.counts.get(blessing).copied().unwrap_or(0)
    }
}

#[derive(Component, Default, Debug, Clone)]
pub struct OwnedMajorBlessings {
    pub blessings: Vec<MajorBlessing>,
}

impl OwnedMajorBlessings {
    pub fn has(&self, blessing: MajorBlessing) -> bool {
        self.blessings.contains(&blessing)
    }

    pub fn add(&mut self, blessing: MajorBlessing) {
        if !self.has(blessing) {
            info!("Player acquired major blessing: {:?}", blessing);
            self.blessings.push(blessing);
        }
    }

    pub fn heirloom_damage_multiplier(&self) -> f32 {
        if self.has(MajorBlessing::HeirloomDamageBoost) {
            1.15
        } else {
            1.0
        }
    }

    pub fn attack_speed_multiplier(&self) -> f32 {
        if self.has(MajorBlessing::AttackSpeedBoost) {
            1.25
        } else {
            1.0
        }
    }

    pub fn skill_damage_multiplier(&self) -> f32 {
        if self.has(MajorBlessing::SkillDamageBoost) {
            1.35
        } else {
            1.0
        }
    }

    pub fn echo_size_multiplier(&self) -> f32 {
        if self.has(MajorBlessing::EchoSizeBoost) {
            1.25
        } else {
            1.0
        }
    }

    pub fn poison_tick_multiplier(&self) -> f32 {
        if self.has(MajorBlessing::PoisonTickFaster) {
            1.3
        } else {
            1.0
        }
    }

    pub fn skill_base_cooldown_reduction(&self) -> f32 {
        if self.has(MajorBlessing::SkillCooldownCut) {
            2.0
        } else {
            0.0
        }
    }

    pub fn coin_drop_rate_multiplier(&self) -> f32 {
        if self.has(MajorBlessing::CoinDropRate) {
            1.2
        } else {
            1.0
        }
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedMajorBlessing {
    pub blessing: MajorBlessing,
    pub title: String,
    pub description: Vec<String>,
    /// Pre-rolled StatA/StatB for Twisted Exchange (shown on the card, applied on pick).
    pub resolved_stat_conversion: Option<ResolvedStatConversion>,
    /// Pre-rolled Lightning/Echo/Ice Explosion/Summon family for pool-status majors.
    pub resolved_effect_family: Option<HeirloomEffectFamily>,
}

impl ResolvedMajorBlessing {
    pub fn from_blessing(blessing: MajorBlessing) -> Self {
        let mut description: Vec<String> = blessing
            .description()
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut resolved_stat_conversion = None;
        let mut resolved_effect_family = None;
        let mut rng = rand::thread_rng();

        if blessing == MajorBlessing::StatConversion {
            let rolled = roll_stat_conversion_pair(&mut rng);
            description = format_stat_conversion_description(&rolled);
            resolved_stat_conversion = Some(rolled);
        } else if let Some(status) = blessing.rolls_effect_family() {
            let family = HeirloomEffectFamily::roll(&mut rng);
            description = format_effect_pool_description(family, status);
            resolved_effect_family = Some(family);
        }

        Self {
            blessing,
            title: blessing.title().to_string(),
            description,
            resolved_stat_conversion,
            resolved_effect_family,
        }
    }

    pub fn display_card_rarity(&self) -> Option<HeirloomRarity> {
        self.blessing.display_card_rarity()
    }

    pub fn max_hp_penalty_pct(&self) -> f32 {
        self.blessing.max_hp_penalty_pct()
    }

    pub fn starting_chaos(&self) -> f32 {
        self.blessing.starting_chaos()
    }
}

#[derive(Resource, Clone, Debug)]
pub struct MajorBlessingOffer {
    pub choices: Vec<(Ancestor, ResolvedMajorBlessing)>,
}

/// Which blessing tier the current choice screen is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlessingTier {
    #[default]
    Minor,
    Major,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct CurrentBlessingTier(pub BlessingTier);

/// Incremented on first Era 1 / Era 2 boss kills. Consumed when opening the major UI.
#[derive(Resource, Default, Debug, Clone)]
pub struct PendingMajorBlessings(pub u32);

/// Era to swap to after the major blessing UI closes (portal path).
/// Always present (init'd in the plugin) so same-frame duplicate portal triggers
/// can see a pending swap immediately — `commands.insert_resource` would not.
#[derive(Resource, Clone, Debug, Default)]
pub struct DeferredEraSwap {
    pub era: Option<crate::world::dimension::Era>,
}

const MAX_ANCESTOR_COPIES_PER_OFFER: u32 = 2;

pub fn build_major_blessing_offer(
    owned: &OwnedMajorBlessings,
    player_skills: Option<&PlayerSkills>,
    _run_unlocks: Option<&RunUnlockState>,
) -> MajorBlessingOffer {
    let mut rng = rand::thread_rng();
    let mut blessing_counts: HashMap<MajorBlessing, u32> = HashMap::new();
    let mut ancestor_counts: HashMap<Ancestor, u32> = HashMap::new();
    let mut choices = Vec::new();

    for _ in 0..3 {
        let eligible_ancestors: Vec<Ancestor> = Ancestor::all()
            .into_iter()
            .filter(|ancestor| {
                ancestor_counts.get(ancestor).copied().unwrap_or(0) < MAX_ANCESTOR_COPIES_PER_OFFER
            })
            .filter(|ancestor| {
                ancestor.major_pool().iter().any(|blessing| {
                    blessing_counts.get(blessing).copied().unwrap_or(0) < 1
                        && blessing.is_offerable(owned, player_skills)
                })
            })
            .collect();

        if eligible_ancestors.is_empty() {
            break;
        }

        let ancestor = Ancestor::roll_weighted_from(&mut rng, &eligible_ancestors);
        *ancestor_counts.entry(ancestor).or_insert(0) += 1;

        let available: Vec<MajorBlessing> = ancestor
            .major_pool()
            .into_iter()
            .filter(|blessing| {
                blessing_counts.get(blessing).copied().unwrap_or(0) < 1
                    && blessing.is_offerable(owned, player_skills)
            })
            .collect();

        if available.is_empty() {
            break;
        }

        let picked = *available.choose(&mut rng).unwrap();
        *blessing_counts.entry(picked).or_insert(0) += 1;
        choices.push((ancestor, ResolvedMajorBlessing::from_blessing(picked)));
    }

    MajorBlessingOffer { choices }
}

/// Convenience for card UI that needs an ItemRarity frame for legendary majors.
pub fn major_blessing_item_rarity(blessing: MajorBlessing) -> Option<ItemRarity> {
    match blessing {
        MajorBlessing::RandomLegendaryWeapon
        | MajorBlessing::RandomLegendaryArmor
        | MajorBlessing::RandomLegendaryAccessory => Some(ItemRarity::Legendary),
        _ => None,
    }
}
