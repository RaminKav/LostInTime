use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{Attack, BonusDamage, CurrentHealth, MaxHealth},
    colors::{
        BLACK, DMG_NUM_GREEN, DMG_NUM_PURPLE, DMG_NUM_RED, DMG_NUM_YELLOW, RED, UI_GRASS_GREEN,
        YELLOW,
    },
    Game, GameParam,
};

#[derive(Component)]
pub struct DamageNumber(pub Timer);

#[derive(Component)]
pub struct PreviousHealth(i32);

pub struct DodgeEvent {
    pub entity: Entity,
}

pub fn add_previous_health(
    mut commands: Commands,
    query: Query<(Entity, &MaxHealth), Added<MaxHealth>>,
) {
    for (entity, max_health) in query.iter() {
        commands.entity(entity).insert(PreviousHealth(max_health.0));
    }
}
// a function that adds damage numbers to the screen in response to a [HitEvent].
// the damage numbers are [Text2DBundle]s with a [DamageNumber] component.
// the [DamageNumber] component is used to delete the damage number after a short delay.

pub fn handle_add_damage_numbers_after_hit(
    mut commands: Commands,
    mut changed_health: Query<
        (Entity, &CurrentHealth, &mut PreviousHealth),
        Changed<CurrentHealth>,
    >,
    txfms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
    raw_dmg: Query<(&Attack, &BonusDamage)>,
    game: Res<Game>,
) {
    for (e, changed_health, mut prev_health) in changed_health.iter_mut() {
        let delta = changed_health.0 - prev_health.0;
        if delta == 0 {
            continue;
        }
        let mut rng = rand::thread_rng();
        let drop_spread = 10.;
        let pos_offset = Vec3::new(
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            2.,
        );
        prev_health.0 = changed_health.0;
        let is_player = e == game.player;
        let dmg = raw_dmg.get(game.player).unwrap().0 .0 + raw_dmg.get(game.player).unwrap().1 .0;
        let is_crit = !is_player && delta.abs() > dmg && dmg != 0;
        for i in 0..2 {
            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        if delta < 0 {
                            format!("{}{}", delta.abs(), if is_crit { "!" } else { "" })
                        } else {
                            format!("+{}", delta)
                        },
                        TextStyle {
                            font: asset_server.load("fonts/Kitchen Sink.ttf"),
                            font_size: 8.0,
                            color: if i == 0 {
                                BLACK
                            } else if delta > 0 {
                                DMG_NUM_GREEN
                            } else if is_player {
                                DMG_NUM_PURPLE
                            } else if is_crit {
                                DMG_NUM_YELLOW
                            } else {
                                DMG_NUM_RED
                            },
                        },
                    ),
                    transform: Transform {
                        translation: txfms.get(e).unwrap().translation()
                            + pos_offset
                            + if i == 0 {
                                Vec3::new(1., -1., -1.)
                            } else {
                                Vec3::ZERO
                            },
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(DamageNumber(Timer::from_seconds(0.75, TimerMode::Once)));
        }
    }
}
pub fn handle_add_dodge_text(
    mut commands: Commands,
    mut dodge_events: EventReader<DodgeEvent>,
    txfms: Query<&GlobalTransform>,
    asset_server: Res<AssetServer>,
) {
    for event in dodge_events.iter() {
        let mut rng = rand::thread_rng();
        let drop_spread = 10.;
        let pos_offset = Vec3::new(
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            i32::max(5 + rng.gen_range(-drop_spread..drop_spread) as i32, 10) as f32,
            2.,
        );

        for i in 0..2 {
            commands
                .spawn(Text2dBundle {
                    text: Text::from_section(
                        "Dodge!",
                        TextStyle {
                            font: asset_server.load("fonts/Kitchen Sink.ttf"),
                            font_size: 8.0,
                            color: if i == 0 { BLACK } else { DMG_NUM_YELLOW },
                        },
                    ),
                    transform: Transform {
                        translation: txfms.get(event.entity).unwrap().translation()
                            + pos_offset
                            + if i == 0 {
                                Vec3::new(1., -1., -1.)
                            } else {
                                Vec3::ZERO
                            },
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .insert(DamageNumber(Timer::from_seconds(0.75, TimerMode::Once)));
        }
    }
}

// function that ticks the [DamageNumber] timer and deletes the [DamageNumber] when the timer is done.
pub fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut DamageNumber, &mut Transform)>,
) {
    for (entity, mut damage_number, mut t) in query.iter_mut() {
        damage_number.0.tick(time.delta());
        t.translation.y += 10. * time.delta_seconds();
        if damage_number.0.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}
