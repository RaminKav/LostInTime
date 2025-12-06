use crate::{
    colors::BLUE,
    player::{
        melee_skills::spawn_echo_hitbox,
        skills::{Heirloom, PlayerSkills},
        Player,
    },
    ui::damage_numbers::spawn_floating_text_with_shadow,
};

use super::{Attack, CurrentHealth, CurrentMana, Healing, MaxMana};

use bevy::prelude::*;

pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: EventReader<ModifyHealthEvent>,
    mut query: Query<(Entity, &mut CurrentHealth, &Healing, &PlayerSkills, &Attack), With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for event in event.iter() {
        let (player_entity, mut health, bonus_healing_rate, skills, attack) = query.single_mut();

        // Apply healing bonus only to positive health changes
        let final_delta = if event.0 > 0 {
            (event.0 as f32 * (1.0 + bonus_healing_rate.0 as f32 / 100.)) as i32
        } else {
            event.0
        };

        health.0 += final_delta;

        // OnHitEcho: Trigger echo when taking damage (any HP loss)
        if final_delta < 0 && skills.has(Heirloom::OnHitEcho) {
            spawn_echo_hitbox(&mut commands, &asset_server, player_entity, attack.0);
        }
    }
}
pub struct ModifyManaEvent(pub i32);

pub fn handle_modify_mana_event(
    mut event: EventReader<ModifyManaEvent>,
    mut query: Query<(&mut CurrentMana, &MaxMana, &GlobalTransform), With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
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
    }
}
