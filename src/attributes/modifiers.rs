use crate::{
    colors::BLUE,
    player::{
        skills::{HeirloomTriggerCounts, ManaGainSource, PlayerSkills},
        Player,
    },
    ui::{
        damage_numbers::{floating_text_font_style, spawn_floating_text_with_shadow},
        CheatSettings,
    },
};

use super::{CurrentHealth, CurrentMana, Healing, MaxMana};

use bevy::prelude::*;

pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: EventReader<ModifyHealthEvent>,
    mut query: Query<(&mut CurrentHealth, &Healing), With<Player>>,
) {
    for event in event.iter() {
        let (mut health, bonus_healing_rate) = query.single_mut();

        // Apply healing bonus only to positive health changes
        let final_delta = if event.0 > 0 {
            (event.0 as f32 * (1.0 + bonus_healing_rate.0 as f32 / 100.)) as i32
        } else {
            event.0
        };

        health.0 += final_delta;
    }
}
/// Modifies the player's current mana. The optional [`ManaGainSource`] attributes positive
/// changes to the mana orb HUD tooltip gain breakdown. `None` (or negative amounts) are not
/// tracked as a gain source.
pub struct ModifyManaEvent(pub i32, pub Option<ManaGainSource>);

impl ModifyManaEvent {
    /// Mana change with no tracked gain source (e.g. mana costs/consumption).
    pub fn new(amount: i32) -> Self {
        Self(amount, None)
    }

    /// Mana gain attributed to a specific source for the HUD tooltip breakdown.
    pub fn gain(amount: i32, source: ManaGainSource) -> Self {
        Self(amount, Some(source))
    }
}

pub fn handle_modify_mana_event(
    mut event: EventReader<ModifyManaEvent>,
    mut query: Query<(&mut CurrentMana, &MaxMana, &GlobalTransform), With<Player>>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    for event in event.iter() {
        if event.0 == 0 {
            continue;
        }
        let (mut mana, max_mana, player_t) = query.single_mut();

        if event.0 > 0 {
            if let Some(source) = event.1.clone() {
                trigger_counts.record_mana_gained(source, event.0);
            }
            let applied = event.0.min(max_mana.0.saturating_sub(mana.0));
            if applied <= 0 {
                continue;
            }
            mana.0 += applied;
            if cheat_settings
                .as_deref()
                .is_some_and(|s| !s.show_player_damage_numbers)
            {
                continue;
            }
            spawn_floating_text_with_shadow(
                &mut commands,
                &asset_server,
                player_t.translation() + Vec3::new(0., 15., 0.),
                BLUE,
                format!("+{} MP", applied),
                floating_text_font_style(cheat_settings.as_deref()),
            );
        } else {
            mana.0 += event.0;
        }
    }
}
