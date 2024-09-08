use crate::{colors::BLUE, player::Player, ui::damage_numbers::spawn_floating_text_with_shadow};

use super::{CurrentHealth, Healing, Mana};

use bevy::prelude::*;

pub struct ModifyHealthEvent(pub i32);

pub fn handle_modify_health_event(
    mut event: EventReader<ModifyHealthEvent>,
    mut query: Query<(&mut CurrentHealth, &Healing), With<Player>>,
) {
    for event in event.iter() {
        let (mut health, bonus_healing_rate) = query.single_mut();
        health.0 += (event.0 as f32 * (1.0 + bonus_healing_rate.0 as f32 / 100.)) as i32;
    }
}
pub struct ModifyManaEvent(pub i32);

pub fn handle_modify_mana_event(
    mut event: EventReader<ModifyManaEvent>,
    mut query: Query<(&mut Mana, &GlobalTransform), With<Player>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
) {
    for event in event.iter() {
        let (mut mana, player_t) = query.single_mut();
        if mana.current == mana.max && event.0 > 0 {
            return;
        }
        mana.current += event.0;
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
