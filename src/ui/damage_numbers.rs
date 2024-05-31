use bevy::prelude::*;
use rand::Rng;

use crate::{
    attributes::{Attack, BonusDamage, CurrentHealth, MaxHealth},
    colors::{BLACK, DMG_NUM_GREEN, DMG_NUM_PURPLE, DMG_NUM_RED, DMG_NUM_YELLOW},
    Game,
};

#[derive(Component)]
pub struct DamageNumber {
    pub timer: Timer,
    pub velocity: f32,
}

#[derive(Component)]
pub struct PreviousHealth(pub i32);

pub struct DodgeEvent {
    pub entity: Entity,
}

pub fn add_previous_health(
    mut commands: Commands,
    query: Query<(Entity, &MaxHealth), (Added<MaxHealth>, Without<PreviousHealth>)>,
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

        spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            txfms.get(e).unwrap().translation() + pos_offset,
            if delta > 0 {
                DMG_NUM_GREEN
            } else if is_player {
                DMG_NUM_PURPLE
            } else if is_crit {
                DMG_NUM_YELLOW
            } else {
                DMG_NUM_RED
            },
            if delta < 0 {
                format!("{}{}", delta.abs(), if is_crit { "!" } else { "" })
            } else {
                format!("+{}", delta)
            },
        );
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
        spawn_floating_text_with_shadow(
            &mut commands,
            &asset_server,
            txfms.get(event.entity).unwrap().translation() + pos_offset,
            DMG_NUM_YELLOW,
            "Dodge!".to_string(),
        );
    }
}

// function that ticks the [DamageNumber] timer and deletes the [DamageNumber] when the timer is done.
pub fn tick_damage_numbers(
    mut commands: Commands,
    time: Res<Time>,
    mut query: Query<(Entity, &mut Text, &mut DamageNumber, &mut Transform)>,
) {
    for (entity, mut text, mut damage_number, mut t) in query.iter_mut() {
        damage_number.timer.tick(time.delta());
        t.translation.y += damage_number.velocity * time.delta_seconds();
        if damage_number.timer.percent() > 0.3 {
            for section in text.sections.iter_mut() {
                section
                    .style
                    .color
                    .set_a(1. - damage_number.timer.percent() + 0.3);
            }
        }
        damage_number.velocity += 0.7;
        if damage_number.timer.finished() {
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn spawn_floating_text_with_shadow(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    color: Color,
    text: String,
) {
    for i in 0..2 {
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    text.clone(),
                    TextStyle {
                        font: asset_server.load("fonts/Kitchen Sink.ttf"),
                        font_size: 8.0,
                        color: if i == 0 { BLACK } else { color },
                    },
                ),
                transform: Transform {
                    translation: pos
                        + if i == 0 {
                            Vec3::new(1., -1., -1.)
                        } else {
                            Vec3::ZERO
                        },
                    ..Default::default()
                },
                ..Default::default()
            })
            .insert(DamageNumber {
                timer: Timer::from_seconds(0.7, TimerMode::Once),
                velocity: 0.,
            });
    }
}
