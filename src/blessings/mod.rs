use std::fmt::Display;

use bevy::prelude::*;
use strum_macros::{EnumIter, IntoStaticStr};

use crate::{ui::UIState, GameState};
mod blessing_choice_ui;
mod blessing_effects;
pub use blessing_choice_ui::*;
pub use blessing_effects::*;
pub struct BlessingsPlugin;

impl Plugin for BlessingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<BlessingSelectEvent>()
            .init_resource::<BlessingItemRewards>()
            .add_systems((handle_blessing_selected,))
            .add_system(enter_blessing_ui.in_schedule(OnEnter(GameState::BlessingChoice)))
            .add_system(setup_blessing_choice_ui.in_schedule(OnEnter(UIState::BlessingChoice)))
            .add_system(
                handle_blessing_choice_card_interactions.run_if(in_state(UIState::BlessingChoice)),
            )
            .add_system(spawn_blessing_item_drops.in_schedule(OnEnter(GameState::Main)));
        // .add_system(

        //         .run_if(in_state(GameState::Main))
        //         .in_base_set(CoreSet::PostUpdate),
        // );
    }
}

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
        }
    }

    pub fn get_title(&self) -> String {
        match self {
            Blessing::SkillAttackSpeed => "Imbued Skills".to_string(),
            Blessing::OrbsAndTomes => "Not Enough Upgrades".to_string(),
            Blessing::SkillCooldownPower => "Channel".to_string(),
        }
    }
    pub fn get_chaos_increase(&self) -> u32 {
        match self {
            Blessing::SkillAttackSpeed => 7,
            Blessing::OrbsAndTomes => 3,
            Blessing::SkillCooldownPower => 5,
        }
    }
}
