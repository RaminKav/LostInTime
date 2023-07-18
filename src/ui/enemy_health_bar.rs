use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{CurrentHealth, MaxHealth},
    colors::{BLACK, RED, YELLOW},
    combat::HitEvent,
    enemy::Mob,
};
#[derive(Component)]
pub struct EnemyHealthBar;

#[derive(Component)]
pub struct DamageNumber(pub Timer);
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
            // println!("hit: {:?} {:?}", current_health, max_health,);
            bar_txfm.scale.x = current_health.0 as f32 / max_health.0 as f32 * BAR_SIZE;
            //shift the bar to the left so its left aligned
            bar_txfm.translation.x = -BAR_SIZE / 2. + bar_txfm.scale.x / 2.;
        }
    }
}

// a function that adds damage numbers to the screen in response to a [HitEvent].
// the damage numbers are [Text2DBundle]s with a [DamageNumber] component.
// the [DamageNumber] component is used to delete the damage number after a short delay.

pub fn handle_add_damage_numbers_after_hit(
    mut commands: Commands,
    mut hit_event: EventReader<HitEvent>,
    changed_health: Query<Entity, Changed<CurrentHealth>>,
    txfms: Query<&Transform>,
    asset_server: Res<AssetServer>,
) {
    for hit in hit_event.iter() {
        let Ok(_) = changed_health.get(hit.hit_entity) else {continue;};
        let mut rng = rand::thread_rng();
        let drop_spread = 10.;
        let pos_offset = Vec3::new(
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            1.,
        );
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    format!("{}", hit.damage),
                    TextStyle {
                        font: asset_server.load("fonts/Kitchen Sink.ttf"),
                        font_size: 8.0,
                        color: BLACK,
                    },
                ),
                transform: Transform {
                    translation: txfms.get(hit.hit_entity).unwrap().translation + pos_offset,
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(DamageNumber(Timer::from_seconds(0.5, TimerMode::Once)));
    }
}

// function that ticks the [DamageNumber] timer and deletes the [DamageNumber] when the timer is done.
pub fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DamageNumber)>,
) {
    for (entity, mut damage_number) in query.iter_mut() {
        damage_number.0.tick(time.delta());
        if damage_number.0.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
