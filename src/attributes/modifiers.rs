use crate::{
    colors::BLUE,
    player::{
        melee_skills::spawn_echo_hitbox,
        skills::{Heirloom, PlayerSkills},
        Player,
    },
    ui::{
        damage_numbers::spawn_floating_text_with_shadow,
        tips::{SeenTips, Tip, TipEvent},
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

        // OnHitEcho: Trigger echo when taking damage (any HP loss)
        if final_delta < 0 && skills.has(Heirloom::OnHitEcho) {
            let mana_cost = Heirloom::OnHitEcho.get_mana_cost();
            if current_mana.0 >= mana_cost {
                current_mana.0 -= mana_cost;
                spawn_echo_hitbox(
                    &mut commands,
                    &asset_server,
                    player_entity,
                    attack.0,
                    projectile_size.get_multiplier(),
                );
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
    game: crate::GameParam,
    mut tip_event: EventWriter<TipEvent>,
    seen_tips: Res<SeenTips>,
) {
    for event in event.iter() {
        let (mut mana, max_mana, player_t) = query.single_mut();
        if mana.0 == max_mana.0 && event.0 > 0 {
            return;
        }
        mana.0 += event.0;
        if event.0 > 0 {
            spawn_floating_text_with_shadow(
                &mut commands,
                &asset_server,
                player_t.translation() + Vec3::new(0., 15., 0.),
                BLUE,
                format!("+{} MP", event.0),
            );
        }

        if max_mana.0 > 0 {
            let mana_percentage = (mana.0 as f32 / max_mana.0 as f32) * 100.0;
            if mana_percentage <= 50.0 {
                if let Some(main_hand) = game.player().main_hand_slot.as_ref() {
                    let weapon_obj = main_hand.get_obj();
                    if weapon_obj.is_magic_weapon() && !seen_tips.has_seen(&Tip::MagicWeapons) {
                        tip_event.send(TipEvent {
                            tip: Tip::MagicWeapons,
                            pos: Vec3::new(-184., -116., 100.),
                        });
                    }
                }
            }
        }
    }
}
