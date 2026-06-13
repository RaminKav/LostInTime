use bevy::prelude::*;
use strum_macros::{EnumIter, IntoStaticStr};

use crate::{
    attributes::ItemRarity,
    item::WorldObject,
    ui::{handle_spawn_inv_item_tooltip, process_heirloom_tooltip_requests, UIState},
    GameState,
};

mod ancestors;
mod blessing_choice_ui;
mod blessing_effects;

pub use ancestors::*;
pub use blessing_choice_ui::*;
pub use blessing_effects::*;

pub struct BlessingsPlugin;

impl Plugin for BlessingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AncestorBlessingSelectEvent>()
            .init_resource::<BlessingItemRewards>()
            .add_system(
                handle_ancestor_blessing_selected
                    .after(handle_blessing_choice_card_interactions)
                    .run_if(
                        in_state(GameState::BlessingChoice)
                            .and_then(resource_exists::<BlessingTransitionState>()),
                    ),
            )
            .add_system(enter_blessing_ui.in_schedule(OnEnter(GameState::BlessingChoice)))
            .add_system(
                transition_to_main_after_blessing
                    .run_if(resource_exists::<BlessingTransitionState>()),
            )
            .add_system(
                transition_blessing_ui_after_choice
                    .run_if(resource_exists::<BlessingTransitionState>()),
            )
            .add_system(setup_blessing_choice_ui.in_schedule(OnEnter(UIState::BlessingChoice)))
            .add_system(
                handle_blessing_choice_card_interactions.run_if(
                    in_state(UIState::BlessingChoice)
                        .and_then(not(resource_exists::<BlessingTransitionState>())),
                ),
            )
            .add_system(
                handle_blessing_choice_icon_tooltips
                    .after(handle_blessing_choice_card_interactions)
                    .run_if(
                        in_state(GameState::BlessingChoice)
                            .and_then(in_state(UIState::BlessingChoice))
                            .and_then(not(resource_exists::<BlessingTransitionState>())),
                    ),
            )
            .add_system(
                process_heirloom_tooltip_requests
                    .after(handle_blessing_choice_icon_tooltips)
                    .run_if(
                        in_state(GameState::BlessingChoice)
                            .and_then(in_state(UIState::BlessingChoice))
                            .and_then(not(resource_exists::<BlessingTransitionState>())),
                    ),
            )
            // Reuse the real inventory item-tooltip renderer for blessing item cards. It is
            // event-driven; the blessing hover system emits `ToolTipUpdateEvent` with a
            // `world_anchor`, and this dispatcher (normally Main-only) must also run here.
            .add_system(
                handle_spawn_inv_item_tooltip
                    .after(handle_blessing_choice_icon_tooltips)
                    .run_if(
                        in_state(GameState::BlessingChoice)
                            .and_then(in_state(UIState::BlessingChoice))
                            .and_then(not(resource_exists::<BlessingTransitionState>())),
                    ),
            )
            .add_system(spawn_blessing_item_drops.in_schedule(OnEnter(GameState::Main)));
    }
}

/// Set when a new run begins; consumed when showing the run-start blessing screen.
#[derive(Resource, Default)]
pub struct PendingRunStartBlessing;

/// Applied when entering Main after a chaos ancestor pick.
#[derive(Resource, Clone, Debug, Default)]
pub struct PendingRunStartChaos {
    pub amount: f32,
}

/// Overrides the class starting weapon spawn in `give_player_starting_items`.
#[derive(Resource, Clone, Debug)]
pub struct StartingWeaponOverride {
    pub weapon: WorldObject,
    pub rarity: ItemRarity,
    /// When true, skip the default class starting weapon entirely.
    pub replace_starting_weapon: bool,
    /// When true, bump the class starting weapon rarity by one tier instead of using `rarity`.
    pub upgrade_starting_weapon: bool,
}

/// Percentage max HP penalty from a chaos ancestor blessing (e.g. 0.25 = -25%).
#[derive(Component, Clone, Debug)]
pub struct BlessingMaxHpPenalty(pub f32);

#[derive(
    Debug,
    FromReflect,
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
pub enum Blessing {
    SkillAttackSpeed,
    OrbsAndTomes,
    SkillCooldownPower,

    PetAttackSpeed,
    LootChests,
    GainRareHeirloom,
    GainLegendaryHeirloom,
    GainCommonHeirlooms,
    Giant,
    Tiny,

    //TODO: these need to be implemented
    AttackManaCost,
    HeirloomStats,
    PoisonStacks,
    Freeze,
    ManaGuard,
    Kevin,
    ThornsSpikes,
    DoubleXp,
    PlasmaWeapon,
    LaserBeam,
    DoubleGoldDrops,
    RandomActiveSkill,
}

impl Blessing {
    pub fn get_description(&self) -> Vec<String> {
        match self {
            Blessing::SkillAttackSpeed => vec![
                "After using a class".to_string(),
                "skill, gain +30%".to_string(),
                "attack speed for".to_string(),
                "2 seconds.".to_string(),
            ],
            Blessing::OrbsAndTomes => {
                vec!["Gain 5 Orbs and".to_string(), "5 Tomes.".to_string()]
            }
            Blessing::SkillCooldownPower => vec![
                "Base class skill".to_string(),
                "cooldowns are".to_string(),
                "doubled, but they".to_string(),
                "gain +100% power.".to_string(),
            ],
            Blessing::PetAttackSpeed => vec![
                "Your pet gains".to_string(),
                "+25% attack".to_string(),
                "speed.".to_string(),
            ],
            Blessing::LootChests => vec!["Gain 3 Loot".to_string(), "Chests.".to_string()],
            Blessing::GainRareHeirloom => {
                vec!["Gain 1 random".to_string(), "Rare Heirloom.".to_string()]
            }
            Blessing::GainLegendaryHeirloom => vec![
                "Gain 1 random".to_string(),
                "Legendary Heirloom.".to_string(),
            ],
            Blessing::GainCommonHeirlooms => {
                vec!["Gain 3 random".to_string(), "Common Heirlooms.".to_string()]
            }
            Blessing::Giant => vec![
                "Gain +50% max".to_string(),
                "health and +50".to_string(),
                "defence, but move".to_string(),
                "50% slower.".to_string(),
            ],
            Blessing::Tiny => vec![
                "Lose 50% max".to_string(),
                "health, but gain".to_string(),
                "50% attack speed".to_string(),
                "and 50 dodge.".to_string(),
            ],
            Blessing::AttackManaCost => vec![
                "Attacks cost 5".to_string(),
                "mana, but become".to_string(),
                "empowered, dealing".to_string(),
                "+10% Damage.".to_string(),
            ],
            Blessing::HeirloomStats => vec![
                "Heirloom Shrines".to_string(),
                "also grant a random".to_string(),
                "bonus stat".to_string(),
            ],
            Blessing::PoisonStacks => vec![
                "You have a 50%".to_string(),
                "chance to apply".to_string(),
                "an additional".to_string(),
                "poison stack.".to_string(),
            ],
            Blessing::Freeze => vec![
                "Mobs with 3 freeze".to_string(),
                "stacks are frozen".to_string(),
                "and can't move.".to_string(),
            ],
            Blessing::ManaGuard => vec![
                "When taking".to_string(),
                "damage, 80% is".to_string(),
                "drained from".to_string(),
                "mana instead.".to_string(),
            ],
            Blessing::Kevin => vec![
                "Attacking mobs".to_string(),
                "has a 25% to deal".to_string(),
                "1 damage to you.".to_string(),
                "This won't kill you".to_string(),
                "and will proc on".to_string(),
                "hit effects.".to_string(),
            ],
            Blessing::ThornsSpikes => vec![
                "Taking damage".to_string(),
                "shoots out spikes.".to_string(),
                "Damage scales with".to_string(),
                "thorns stat.".to_string(),
            ],
            Blessing::DoubleXp => vec![
                "Mobs have a 10%".to_string(),
                "chance to give".to_string(),
                "double XP.".to_string(),
            ],
            Blessing::PlasmaWeapon => vec![
                "Gain a special".to_string(),
                "weapon that".to_string(),
                "shoots a plasma ".to_string(),
                "ball that explodes.".to_string(),
            ],
            Blessing::LaserBeam => vec![
                "Gain a special".to_string(),
                "Skill that shoots".to_string(),
                "a powerful laser".to_string(),
                "beam.".to_string(),
            ],
            Blessing::DoubleGoldDrops => vec![
                "Mobs have a".to_string(),
                "chance to drop".to_string(),
                "an extra coin.".to_string(),
            ],
            Blessing::RandomActiveSkill => {
                vec![
                    "Gain a random".to_string(),
                    "additional active".to_string(),
                    "class skill.".to_string(),
                ]
            }
        }
    }

    pub fn get_title(&self) -> String {
        match self {
            Blessing::SkillAttackSpeed => "Imbued Skills".to_string(),
            Blessing::OrbsAndTomes => "Not Enough Upgrades".to_string(),
            Blessing::SkillCooldownPower => "Channel".to_string(),
            Blessing::PetAttackSpeed => "Swift Companion".to_string(),
            Blessing::LootChests => "Treasure Hunt".to_string(),
            Blessing::GainRareHeirloom => "Rare Gift".to_string(),
            Blessing::GainLegendaryHeirloom => "Legendary Gift".to_string(),
            Blessing::GainCommonHeirlooms => "Common Gift".to_string(),
            Blessing::Giant => "Giant".to_string(),
            Blessing::Tiny => "Tiny".to_string(),
            Blessing::AttackManaCost => "Empowered".to_string(),
            Blessing::HeirloomStats => "Wisdom".to_string(),
            Blessing::PoisonStacks => "Toxic".to_string(),
            Blessing::Freeze => "Frost".to_string(),
            Blessing::ManaGuard => "Guarded".to_string(),
            Blessing::Kevin => "Insanity".to_string(),
            Blessing::ThornsSpikes => "Spiked".to_string(),
            Blessing::DoubleXp => "Experienced".to_string(),
            Blessing::PlasmaWeapon => "Plasma".to_string(),
            Blessing::LaserBeam => "Laser".to_string(),
            Blessing::DoubleGoldDrops => "Rich".to_string(),
            Blessing::RandomActiveSkill => "Skillful".to_string(),
        }
    }
    pub fn get_chaos_increase(&self) -> u32 {
        match self {
            Blessing::SkillAttackSpeed => 5,
            Blessing::OrbsAndTomes => 3,
            Blessing::SkillCooldownPower => 5,
            Blessing::PetAttackSpeed => 3,
            Blessing::LootChests => 2,
            Blessing::GainRareHeirloom => 2,
            Blessing::GainLegendaryHeirloom => 7,
            Blessing::GainCommonHeirlooms => 2,
            Blessing::Giant => 6,
            Blessing::Tiny => 6,
            Blessing::AttackManaCost => 4,
            Blessing::HeirloomStats => 6,
            Blessing::PoisonStacks => 4,
            Blessing::Freeze => 5,
            Blessing::ManaGuard => 6,
            Blessing::Kevin => 3,
            Blessing::ThornsSpikes => 4,
            Blessing::DoubleXp => 6,
            Blessing::PlasmaWeapon => 7,
            Blessing::LaserBeam => 7,
            Blessing::DoubleGoldDrops => 5,
            Blessing::RandomActiveSkill => 6,
        }
    }
}
