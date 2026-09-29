use bevy::prelude::*;

use super::{Ancestor, AncestorBlessing, MajorBlessing};

/// Native effect-icon size. Every blessing icon PNG is 42×42.
const ICON_SIZE: Vec2 = Vec2::new(42.0, 42.0);

const BLESSING_ICON_Z: f32 = 4.0;

/// Card-local Y of the blessing title. The diamond icon sits above this
/// (center around y 45–48, half-height 21), so the title starts just under the slot.
pub const BLESSING_CARD_TITLE_Y: f32 = 4.0;

/// Ancestor card frame, drawn at its PNG's exact pixel size.
#[derive(Clone, Copy, Debug)]
pub struct BlessingFrameArt {
    pub path: &'static str,
    pub size: Vec2,
    /// Center of the diamond icon slot, in card-local space (+Y up).
    pub icon_offset: Vec2,
}

impl BlessingFrameArt {
    pub fn icon_center(self) -> Vec2 {
        self.icon_offset
    }

    pub fn icon_translation(self) -> Vec3 {
        self.icon_center().extend(BLESSING_ICON_Z)
    }
}

/// Per-blessing diamond icon, drawn at its PNG's exact pixel size on choice cards.
#[derive(Clone, Copy, Debug)]
pub struct BlessingIconArt {
    pub path: &'static str,
    pub size: Vec2,
}

/// Which blessing a HUD slot is showing. An empty slot draws [`HUD_EMPTY_SLOT_ART`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlessingHudIconId {
    Minor(AncestorBlessing),
    Major(MajorBlessing),
}

impl BlessingHudIconId {
    pub fn art(self) -> BlessingIconArt {
        match self {
            BlessingHudIconId::Minor(blessing) => minor_icon(blessing),
            BlessingHudIconId::Major(blessing) => major_icon(blessing),
        }
    }
}

/// Empty HUD blessing slot. 44×44, a 1px border around the 42×42 effect icons.
pub const HUD_EMPTY_SLOT_ART: BlessingIconArt = BlessingIconArt {
    path: "ui/blessings/icons/BlessingBackground.png",
    size: Vec2::new(44.0, 44.0),
};

const MINOR_FRAME_SIZE: Vec2 = Vec2::new(124.0, 168.0);
const MAJOR_FRAME_SIZE: Vec2 = Vec2::new(124.0, 172.0);
/// Icon center in card space. Frames and icons are even-sized, so these must stay whole
/// numbers or every edge lands on a half pixel and the diamond border stops lining up.
/// Icon top-left sits at texel (39, 15) on minor frames and (39, 20) on major frames.
const MINOR_ICON_OFFSET: Vec2 = Vec2::new(-2.0, 48.0);
const MAJOR_ICON_OFFSET: Vec2 = Vec2::new(-2.0, 45.0);

pub fn frame_art(ancestor: Ancestor, major: bool) -> BlessingFrameArt {
    let (size, icon_offset) = if major {
        (MAJOR_FRAME_SIZE, MAJOR_ICON_OFFSET)
    } else {
        (MINOR_FRAME_SIZE, MINOR_ICON_OFFSET)
    };
    BlessingFrameArt {
        path: frame_path(ancestor, major),
        size,
        icon_offset,
    }
}

fn frame_path(ancestor: Ancestor, major: bool) -> &'static str {
    match (ancestor, major) {
        (Ancestor::Resources, false) => "ui/blessings/frames/MinorResources.png",
        (Ancestor::Resources, true) => "ui/blessings/frames/MajorResources.png",
        (Ancestor::Heirlooms, false) => "ui/blessings/frames/MinorHeirlooms.png",
        (Ancestor::Heirlooms, true) => "ui/blessings/frames/MajorHeirlooms.png",
        (Ancestor::Weapons, false) => "ui/blessings/frames/MinorWeapons.png",
        (Ancestor::Weapons, true) => "ui/blessings/frames/MajorWeapons.png",
        (Ancestor::Skills, false) => "ui/blessings/frames/MinorSkills.png",
        (Ancestor::Skills, true) => "ui/blessings/frames/MajorSkills.png",
        (Ancestor::Chaos, false) => "ui/blessings/frames/MinorChaos.png",
        (Ancestor::Chaos, true) => "ui/blessings/frames/MajorChaos.png",
    }
}

fn minor_icon_art(file: &'static str) -> BlessingIconArt {
    BlessingIconArt {
        path: file,
        size: ICON_SIZE,
    }
}

pub fn minor_icon(blessing: AncestorBlessing) -> BlessingIconArt {
    match blessing {
        AncestorBlessing::ThreeTomes => minor_icon_art("ui/blessings/icons/minor/ThreeTomes.png"),
        AncestorBlessing::ThreeOrbs => minor_icon_art("ui/blessings/icons/minor/ThreeOrbs.png"),
        AncestorBlessing::TwoOrbsTwoTomes => {
            minor_icon_art("ui/blessings/icons/minor/TwoOrbsTwoTomes.png")
        }
        AncestorBlessing::FiftyGold => minor_icon_art("ui/blessings/icons/minor/FiftyGold.png"),
        AncestorBlessing::ThreeStatFoods => {
            minor_icon_art("ui/blessings/icons/minor/ThreeStatFoods.png")
        }
        AncestorBlessing::ThreeRerolls => {
            minor_icon_art("ui/blessings/icons/minor/ThreeRerolls.png")
        }
        AncestorBlessing::TwoBanishes => minor_icon_art("ui/blessings/icons/minor/TwoBanishes.png"),
        AncestorBlessing::ThreeCommonHeirlooms => {
            minor_icon_art("ui/blessings/icons/minor/ThreeCommonHeirlooms.png")
        }
        AncestorBlessing::OneUncommonHeirloom => {
            minor_icon_art("ui/blessings/icons/minor/OneUncommonHeirloom.png")
        }
        AncestorBlessing::SpecificUncommon => {
            minor_icon_art("ui/blessings/icons/minor/SpecificUncommon.png")
        }
        AncestorBlessing::TwoOfSpecificCommon => {
            minor_icon_art("ui/blessings/icons/minor/TwoOfSpecificCommon.png")
        }
        AncestorBlessing::UpgradeStartingWeapon => {
            minor_icon_art("ui/blessings/icons/minor/UpgradeStartingWeapon.png")
        }
        AncestorBlessing::RandomWeapon => {
            minor_icon_art("ui/blessings/icons/minor/RandomWeapon.png")
        }
        AncestorBlessing::RandomEquipment => {
            minor_icon_art("ui/blessings/icons/minor/RandomEquipment.png")
        }
        AncestorBlessing::RandomAccessory => {
            minor_icon_art("ui/blessings/icons/minor/RandomAccessory.png")
        }
        AncestorBlessing::ReplaceWithSpecificWeapon => {
            minor_icon_art("ui/blessings/icons/minor/ReplaceWithSpecificWeapon.png")
        }
        AncestorBlessing::RandomWeaponHeirloom => {
            minor_icon_art("ui/blessings/icons/minor/RandomWeaponHeirloom.png")
        }
        AncestorBlessing::RandomSkill => minor_icon_art("ui/blessings/icons/minor/RandomSkill.png"),
        AncestorBlessing::SpecificSkill => {
            minor_icon_art("ui/blessings/icons/minor/SpecificSkill.png")
        }
        AncestorBlessing::SkillHeirloom => {
            minor_icon_art("ui/blessings/icons/minor/SkillHeirloom.png")
        }
        AncestorBlessing::PlasmaWeapon => {
            minor_icon_art("ui/blessings/icons/minor/PlasmaWeapon.png")
        }
        AncestorBlessing::LaserBeam => minor_icon_art("ui/blessings/icons/minor/LaserBeam.png"),
        AncestorBlessing::SpecificRareHeirloom => {
            minor_icon_art("ui/blessings/icons/minor/SpecificRareHeirloom.png")
        }
        AncestorBlessing::TwoRandomRareHeirlooms => {
            minor_icon_art("ui/blessings/icons/minor/TwoRandomRareHeirlooms.png")
        }
        AncestorBlessing::FiveOfRandomCommon => {
            minor_icon_art("ui/blessings/icons/minor/FiveOfRandomCommon.png")
        }
        AncestorBlessing::ThreeOfRandomUncommon => {
            minor_icon_art("ui/blessings/icons/minor/ThreeOfRandomUncommon.png")
        }
        AncestorBlessing::RandomRareEquipment => {
            minor_icon_art("ui/blessings/icons/minor/RandomRareEquipment.png")
        }
    }
}

fn major_icon_art(file: &'static str) -> BlessingIconArt {
    BlessingIconArt {
        path: file,
        size: ICON_SIZE,
    }
}

pub fn major_icon(blessing: MajorBlessing) -> BlessingIconArt {
    match blessing {
        MajorBlessing::LightningCoinChance => {
            major_icon_art("ui/blessings/icons/major/LightningCoinChance.png")
        }
        MajorBlessing::EchoSizeBoost => {
            major_icon_art("ui/blessings/icons/major/EchoSizeBoost.png")
        }
        MajorBlessing::PoisonTickFaster => {
            major_icon_art("ui/blessings/icons/major/PoisonTickFaster.png")
        }
        MajorBlessing::HeirloomDamageBoost => {
            major_icon_art("ui/blessings/icons/major/HeirloomDamageBoost.png")
        }
        MajorBlessing::ExtraManaRegen => {
            major_icon_art("ui/blessings/icons/major/ExtraManaRegen.png")
        }
        MajorBlessing::TripleUncommonHeirloom => {
            major_icon_art("ui/blessings/icons/major/TripleUncommonHeirloom.png")
        }
        MajorBlessing::SummonRetrigger => {
            major_icon_art("ui/blessings/icons/major/SummonRetrigger.png")
        }
        MajorBlessing::TouchThorns => major_icon_art("ui/blessings/icons/major/TouchThorns.png"),
        MajorBlessing::EchoAftershock => {
            major_icon_art("ui/blessings/icons/major/EchoAftershock.png")
        }
        MajorBlessing::ViralConductor => {
            major_icon_art("ui/blessings/icons/major/ViralConductor.png")
        }
        MajorBlessing::HeirloomOverclock => {
            major_icon_art("ui/blessings/icons/major/HeirloomOverclock.png")
        }
        MajorBlessing::SharedAffliction => {
            major_icon_art("ui/blessings/icons/major/SharedAffliction.png")
        }
        MajorBlessing::ConvertUncommons => {
            major_icon_art("ui/blessings/icons/major/ConvertUncommons.png")
        }
        MajorBlessing::WeaponHeirloomDoubleTrigger => {
            major_icon_art("ui/blessings/icons/major/WeaponHeirloomDoubleTrigger.png")
        }
        MajorBlessing::AttackSpeedBoost => {
            major_icon_art("ui/blessings/icons/major/AttackSpeedBoost.png")
        }
        MajorBlessing::RandomLegendaryWeapon => {
            major_icon_art("ui/blessings/icons/major/RandomLegendaryWeapon.png")
        }
        MajorBlessing::RandomLegendaryArmor => {
            major_icon_art("ui/blessings/icons/major/RandomLegendaryArmor.png")
        }
        MajorBlessing::RandomLegendaryAccessory => {
            major_icon_art("ui/blessings/icons/major/RandomLegendaryAccessory.png")
        }
        MajorBlessing::PetSizeAndAttackSpeed => {
            major_icon_art("ui/blessings/icons/major/PetSizeAndAttackSpeed.png")
        }
        MajorBlessing::EffectPoolApplyFrail => {
            major_icon_art("ui/blessings/icons/major/EffectPoolApplyFrail.png")
        }
        MajorBlessing::StatusApplyShield => {
            major_icon_art("ui/blessings/icons/major/StatusApplyShield.png")
        }
        MajorBlessing::SkillDamageBoost => {
            major_icon_art("ui/blessings/icons/major/SkillDamageBoost.png")
        }
        MajorBlessing::SkillCooldownCut => {
            major_icon_art("ui/blessings/icons/major/SkillCooldownCut.png")
        }
        MajorBlessing::SkillPoisonStacks => {
            major_icon_art("ui/blessings/icons/major/SkillPoisonStacks.png")
        }
        MajorBlessing::MovementSkillSummons => {
            major_icon_art("ui/blessings/icons/major/MovementSkillSummons.png")
        }
        MajorBlessing::IceExplosionChain => {
            major_icon_art("ui/blessings/icons/major/IceExplosionChain.png")
        }
        MajorBlessing::EffectPoolApplyFreeze => {
            major_icon_art("ui/blessings/icons/major/EffectPoolApplyFreeze.png")
        }
        MajorBlessing::EffectPoolApplyPoison => {
            major_icon_art("ui/blessings/icons/major/EffectPoolApplyPoison.png")
        }
        MajorBlessing::ManaRegenHeal => {
            major_icon_art("ui/blessings/icons/major/ManaRegenHeal.png")
        }
        MajorBlessing::SkillsApplyAllStatuses => {
            major_icon_art("ui/blessings/icons/major/SkillsApplyAllStatuses.png")
        }
        MajorBlessing::StatusApplyExtra => {
            major_icon_art("ui/blessings/icons/major/StatusApplyExtra.png")
        }
        MajorBlessing::StatConversion => {
            major_icon_art("ui/blessings/icons/major/StatConversion.png")
        }
        MajorBlessing::LuckChaosTradeoff => {
            major_icon_art("ui/blessings/icons/major/LuckChaosTradeoff.png")
        }
        MajorBlessing::ManaDrainShield => {
            major_icon_art("ui/blessings/icons/major/ManaDrainShield.png")
        }
        MajorBlessing::OverhealToShield => {
            major_icon_art("ui/blessings/icons/major/OverhealToShield.png")
        }
        MajorBlessing::MerchantSlotReplenish => {
            major_icon_art("ui/blessings/icons/major/MerchantSlotReplenish.png")
        }
        MajorBlessing::TomesAndOrbs => major_icon_art("ui/blessings/icons/major/TomesAndOrbs.png"),
        MajorBlessing::RerollsAndBanishes => {
            major_icon_art("ui/blessings/icons/major/RerollsAndBanishes.png")
        }
        MajorBlessing::CoinDropRate => major_icon_art("ui/blessings/icons/major/CoinDropRate.png"),
        MajorBlessing::LargeObjectEcho => {
            major_icon_art("ui/blessings/icons/major/LargeObjectEcho.png")
        }
        MajorBlessing::ObjectBreakLoot => {
            major_icon_art("ui/blessings/icons/major/ObjectBreakLoot.png")
        }
    }
}
