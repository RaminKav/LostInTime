use bevy::prelude::*;

use crate::ecs_helpers::SafeHierarchyExt;

use crate::{
    assets::Graphics,
    attributes::{CurrentHealth, MaxHealth},
    enemy::{EliteMob, MegaEliteMob, Mob},
};

use super::UIElement;
#[derive(Component)]
pub struct EnemyHealthBar;

const BAR_SIZE: f32 = 25.;

pub fn handle_enemy_health_bar_change(
    mut query: Query<(&Children, &MaxHealth, &CurrentHealth), (With<Mob>, Changed<CurrentHealth>)>,
    mut query2: Query<&mut Transform, With<EnemyHealthBar>>,
) {
    for (children, max_health, current_health) in query.iter_mut() {
        for child in children.iter() {
            let Ok(mut bar_txfm) = query2.get_mut(child) else {
                continue;
            };
            bar_txfm.scale.x = current_health.0 as f32 / max_health.0 as f32 * BAR_SIZE;
            bar_txfm.translation.x = -BAR_SIZE / 2. + bar_txfm.scale.x / 2.;
        }
    }
}

pub fn add_ui_icon_for_elite_mobs(
    elites: Query<(Entity, Option<&MegaEliteMob>), Added<EliteMob>>,
    mut commands: Commands,
    graphics: Res<Graphics>,
) {
    for (elite, mega) in elites.iter() {
        let icon = if mega.is_some() {
            UIElement::MegaEliteStar
        } else {
            UIElement::EliteStar
        };
        let size = if mega.is_some() { 9. } else { 5. };
        let offset = if mega.is_some() { 18. } else { 10. };
        commands
            .spawn((
                Sprite {
                    image: graphics.get_ui_element_texture(icon),
                    custom_size: Some(Vec2::new(size, size)),
                    ..default()
                },
                Transform {
                    translation: Vec3::new(0., offset, 1.),
                    scale: Vec3::new(1., 1., 1.),
                    ..Default::default()
                },
            ))
            .safe_set_parent(elite);
    }
}
