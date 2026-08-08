use crate::{
    blessings::{BlessingTriggerCounts, MajorBlessing, OwnedMajorBlessings},
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

use super::{CurrentHealth, CurrentMana, CurrentShield, Healing, MaxHealth, MaxMana};

use bevy::prelude::*;

#[derive(Message)]
pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: MessageReader<ModifyHealthEvent>,
    mut query: Query<
        (
            Entity,
            &mut CurrentHealth,
            &Healing,
            &MaxHealth,
            Option<&mut CurrentShield>,
            Option<&OwnedMajorBlessings>,
        ),
        With<Player>,
    >,
    mut commands: Commands,
    mut blessing_triggers: ResMut<BlessingTriggerCounts>,
) {
    for event in event.read() {
        let Ok((
            player_e,
            mut health,
            bonus_healing_rate,
            max_health,
            mut shield,
            majors,
        )) = query.single_mut()
        else {
            return;
        };

        // Apply healing bonus only to positive health changes
        let final_delta = if event.0 > 0 {
            (event.0 as f32 * (1.0 + bonus_healing_rate.0 as f32 / 100.)) as i32
        } else {
            event.0
        };

        if final_delta > 0 && majors.is_some_and(|m| m.has(MajorBlessing::OverhealToShield)) {
            let hp_room = (max_health.0 - health.0).max(0);
            let heal_to_hp = final_delta.min(hp_room);
            health.0 += heal_to_hp;
            let overflow = final_delta - heal_to_hp;
            if overflow > 0 {
                blessing_triggers.increment(MajorBlessing::OverhealToShield);
                let shield_cap = max_health.0 / 2;
                if let Some(shield) = shield.as_deref_mut() {
                    shield.0 = (shield.0 + overflow).min(shield_cap);
                } else {
                    commands
                        .entity(player_e)
                        .insert(CurrentShield(overflow.min(shield_cap)));
                }
            }
        } else {
            health.0 += final_delta;
        }
    }
}
/// Modifies the player's current mana. The optional [`ManaGainSource`] attributes positive
/// changes to the mana orb HUD tooltip gain breakdown. `None` (or negative amounts) are not
/// tracked as a gain source.
#[derive(Message)]
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
    mut event: MessageReader<ModifyManaEvent>,
    mut query: Query<(&mut CurrentMana, &MaxMana, &GlobalTransform), With<Player>>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    for event in event.read() {
        if event.0 == 0 {
            continue;
        }
        let Ok((mut mana, max_mana, player_t)) = query.single_mut() else {
            return;
        };

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
