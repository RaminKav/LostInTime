use crate::{
    colors::BLUE,
    player::{
        melee_skills::{
            spawn_delayed_heirloom_cast, spawn_echo_hitbox, DelayedCastType,
            HEIRLOOM_EXTRA_CAST_DELAY,
        },
        skills::{Heirloom, HeirloomTriggerCounts, PlayerSkills},
        Player,
    },
    ui::{
        damage_numbers::{floating_text_font_style, spawn_floating_text_with_shadow},
        CheatSettings,
    },
};

use super::{Attack, CurrentHealth, CurrentMana, Healing, MaxMana, ProjectileSize};

use bevy::prelude::*;

pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: EventReader<ModifyHealthEvent>,
    mut query: Query<
        (
            Entity,
            &mut CurrentHealth,
            &Healing,
            &PlayerSkills,
            &Attack,
            &ProjectileSize,
            &mut CurrentMana,
        ),
        With<Player>,
    >,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut trigger_counts: ResMut<HeirloomTriggerCounts>,
) {
    for event in event.iter() {
        let (
            player_entity,
            mut health,
            bonus_healing_rate,
            skills,
            attack,
            projectile_size,
            mut current_mana,
        ) = query.single_mut();

        // Apply healing bonus only to positive health changes
        let final_delta = if event.0 > 0 {
            (event.0 as f32 * (1.0 + bonus_healing_rate.0 as f32 / 100.)) as i32
        } else {
            event.0
        };

        health.0 += final_delta;

        if final_delta < 0 {
            let echo_count = skills.get_count(Heirloom::OnHitEcho);
            let mana_cost = Heirloom::OnHitEcho.get_mana_cost();
            let dmg = attack.0;
            let size_mult = projectile_size.get_multiplier();

            for i in 0..echo_count {
                if current_mana.0 < mana_cost {
                    break;
                }
                current_mana.0 -= mana_cost;
                trigger_counts.record_mana(Heirloom::OnHitEcho, mana_cost);
                trigger_counts.increment(Heirloom::OnHitEcho);

                if i == 0 {
                    spawn_echo_hitbox(&mut commands, &asset_server, player_entity, dmg, size_mult);
                } else {
                    spawn_delayed_heirloom_cast(
                        &mut commands,
                        HEIRLOOM_EXTRA_CAST_DELAY * i as f32,
                        DelayedCastType::Echo {
                            player: player_entity,
                            dmg,
                            size_multiplier: size_mult,
                        },
                    );
                }
            }
        }
    }
}
pub struct ModifyManaEvent(pub i32);

pub fn handle_modify_mana_event(
    mut event: EventReader<ModifyManaEvent>,
    mut query: Query<(&mut CurrentMana, &MaxMana, &GlobalTransform), With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    for event in event.iter() {
        let (mut mana, max_mana, player_t) = query.single_mut();
        if mana.0 == max_mana.0 && event.0 > 0 {
            return;
        }
        mana.0 += event.0;
        if event.0 > 0 {
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
                format!("+{} MP", event.0),
                floating_text_font_style(cheat_settings.as_deref()),
            );
        }
    }
}
