use bevy::prelude::*;

use crate::{
    attributes::{CurrentHealth, MaxHealth},
    colors::{RED, YELLOW},
    enemy::Mob,
};
#[derive(Component)]
pub struct EnemyHealthBar;

const BAR_SIZE: f32 = 25.;

pub fn create_enemy_health_bar(
    mut commands: Commands,
    mut query: Query<Entity, (Added<Mob>, With<MaxHealth>)>,
) {
    for entity in query.iter_mut() {
        let bar_frame = commands
            .spawn(SpriteBundle {
                transform: Transform {
                    translation: Vec3::new(0., 10., 1.),
                    scale: Vec3::new(BAR_SIZE, 2., -1.),
                    ..default()
                },
                sprite: Sprite {
                    color: YELLOW,
                    ..default()
                },
                visibility: Visibility::Hidden,
                ..default()
            })
            .id();
        let bar = commands
            .spawn(SpriteBundle {
                transform: Transform {
                    translation: Vec3::new(0., 10., 3.),
                    scale: Vec3::new(BAR_SIZE, 2., 1.),
                    ..default()
                },
                visibility: Visibility::Hidden,
                sprite: Sprite {
                    color: RED,
                    ..default()
                },
                ..default()
            })
            .insert(EnemyHealthBar)
            .id();
        commands.entity(entity).add_child(bar).add_child(bar_frame);
    }
}
pub fn handle_enemy_health_bar_change(
    mut query: Query<(&Children, &MaxHealth, &CurrentHealth), (With<Mob>, Changed<CurrentHealth>)>,
    mut query2: Query<&mut Transform, With<EnemyHealthBar>>,
) {
    for (children, max_health, current_health) in query.iter_mut() {
        for child in children.iter() {
            let Ok(mut bar_txfm) = query2.get_mut(*child) else {continue;};
            bar_txfm.scale.x = current_health.0 as f32 / max_health.0 as f32 * BAR_SIZE;
            bar_txfm.translation.x = -BAR_SIZE / 2. + bar_txfm.scale.x / 2.;
        }
    }
}
pub fn handle_enemy_health_visibility(
    mut query: Query<(&Children, &MaxHealth, &CurrentHealth), (With<Mob>, Changed<CurrentHealth>)>,
    mut query2: Query<&mut Visibility>,
) {
    for (children, max_health, current_health) in query.iter_mut() {
        for child in children.iter() {
            let Ok(mut v) = query2.get_mut(*child) else {continue;};
            if current_health.0 == max_health.0 {
                *v = Visibility::Hidden;
            } else {
                *v = Visibility::Inherited;
            }
        }
    }
}
