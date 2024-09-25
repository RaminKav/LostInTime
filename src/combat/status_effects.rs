use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};
use serde::Deserialize;
use strum_macros::{Display, EnumIter};

use crate::{assets::Graphics, attributes::CurrentHealth};

use super::{EnemyDeathEvent, MarkedForDeath};

#[derive(
    Deserialize, Debug, EnumIter, Display, Hash, Clone, Reflect, FromReflect, Eq, PartialEq,
)]
pub enum StatusEffect {
    Slow,
    Frail,
    Poison,
}

#[derive(Deserialize, Debug, Clone, Reflect, FromReflect)]
pub struct StatusEffectState {
    pub effect: StatusEffect,
    pub num_stacks: i32,
    pub index: usize,
}

#[derive(Component, Deserialize, Debug, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct StatusEffectTracker {
    pub effects: Vec<StatusEffectState>,
}
#[derive(Component)]
pub struct StatusEffectIcon;

pub struct StatusEffectEvent {
    pub effect: StatusEffect,
    pub num_stacks: i32,
    pub entity: Entity,
}

#[derive(Component, Clone)]
pub struct Burning {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub damage: u8,
}
#[derive(Component)]
pub struct Poisoned {
    pub tick_timer: Timer,
    pub duration_timer: Timer,
    pub damage: u8,
}
#[derive(Component, Debug)]
pub struct Frail {
    pub num_stacks: u8,
    pub timer: Timer,
}
#[derive(Component, Debug)]
pub struct Slow {
    pub num_stacks: u8,
    pub timer: Timer,
}

pub fn handle_new_status_effect_event(
    mut query: Query<&mut StatusEffectTracker>,
    mut events: EventReader<StatusEffectEvent>,
) {
    for event in events.iter() {
        let Ok(mut tracker) = query.get_mut(event.entity) else {
            continue;
        };

        if event.num_stacks == 0 {
            tracker.effects.retain(|e| e.effect != event.effect);
            continue;
        }
        if let Some(prev_status_tracker) = tracker
            .effects
            .iter_mut()
            .find(|e| e.effect == event.effect)
        {
            prev_status_tracker.num_stacks = event.num_stacks;
        } else {
            let index = tracker.effects.len();
            tracker.effects.push(StatusEffectState {
                effect: event.effect.clone(),
                num_stacks: event.num_stacks,
                index,
            });
        }
    }
}

pub fn update_status_effect_icons(
    mut query: Query<
        (Entity, Option<&Children>, &StatusEffectTracker),
        Changed<StatusEffectTracker>,
    >,
    mut commands: Commands,
    graphics: Res<Graphics>,
    prev_status_icons: Query<(Entity, &StatusEffectIcon)>,
) {
    for (entity, maybe_children, tracker) in query.iter_mut() {
        // remove old icons
        if let Some(children) = maybe_children {
            for prev_icon in prev_status_icons.iter() {
                if children.iter().any(|c| c == &prev_icon.0) {
                    commands.entity(prev_icon.0).despawn_recursive();
                }
            }
        }
        for (height, effect) in tracker.effects.iter().enumerate() {
            let total_stacks = effect.num_stacks as f32;
            for i in 0..effect.num_stacks {
                let icon = graphics.get_status_effect_icon(effect.effect.clone());
                let d = 1.;
                let s = 5.;
                let i = i as f32;
                let h = height as f32;
                let translation = Vec3::new(
                    i * (s + d) - (total_stacks - 1.) * (s / 2.) - d,
                    7. * h + 12.,
                    1.,
                );
                commands
                    .spawn(SpriteBundle {
                        texture: icon,
                        sprite: Sprite {
                            custom_size: Some(Vec2::new(5., 5.)),
                            ..Default::default()
                        },
                        transform: Transform {
                            translation,
                            scale: Vec3::new(1., 1., 1.),
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .insert(StatusEffectIcon)
                    .set_parent(entity);
            }
        }
    }
}

pub fn handle_burning_ticks(
    mut burning: Query<(Entity, &mut Burning, &mut CurrentHealth, &GlobalTransform)>,
    time: Res<Time>,
    mut commands: Commands,
    mut enemy_death_events: EventWriter<EnemyDeathEvent>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    for (e, mut burning, mut curr_hp, t) in burning.iter_mut() {
        burning.duration_timer.tick(time.delta());
        if !burning.duration_timer.just_finished() {
            burning.tick_timer.tick(time.delta());
            if burning.tick_timer.just_finished() {
                curr_hp.0 -= burning.damage as i32;
                if curr_hp.0 <= 0 {
                    commands
                        .entity(e)
                        .insert(MarkedForDeath)
                        .remove::<Burning>();
                    enemy_death_events.send(EnemyDeathEvent {
                        entity: e,
                        enemy_pos: t.translation().truncate(),
                        killed_by_crit: false,
                    });
                }
                burning.tick_timer.reset();
            }
        } else {
            commands.entity(e).remove::<Burning>();
            status_event.send(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Poison,
                num_stacks: 0,
            });
        }
    }
}
pub fn handle_frail_stack_ticks(
    mut frailed: Query<(Entity, &mut Frail)>,
    mut commands: Commands,
    time: Res<Time>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    for (e, mut frail) in frailed.iter_mut() {
        frail.timer.tick(time.delta());
        if frail.timer.just_finished() {
            frail.num_stacks -= 1;
            if frail.num_stacks == 0 {
                commands.entity(e).remove::<Frail>();
            }
            status_event.send(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Frail,
                num_stacks: frail.num_stacks as i32,
            });
        }
    }
}
pub fn handle_slow_stack_ticks(
    mut slowed: Query<(Entity, &mut Slow)>,
    mut commands: Commands,
    time: Res<Time>,
    mut status_event: EventWriter<StatusEffectEvent>,
) {
    for (e, mut slow) in slowed.iter_mut() {
        slow.timer.tick(time.delta());
        if slow.timer.just_finished() {
            slow.num_stacks -= 1;
            if slow.num_stacks == 0 {
                commands.entity(e).remove::<Slow>();
            }
            status_event.send(StatusEffectEvent {
                entity: e,
                effect: StatusEffect::Slow,
                num_stacks: slow.num_stacks as i32,
            });
        }
    }
}

pub fn try_add_slow_stacks(
    hit_e: Entity,
    commands: &mut Commands,
    status_event: &mut EventWriter<StatusEffectEvent>,
    slowed_option: Option<&mut Slow>,
) {
    if let Some(slow_stacks) = slowed_option {
        if slow_stacks.num_stacks < 3 {
            slow_stacks.num_stacks += 1;
            slow_stacks.timer.reset();
            status_event.send(StatusEffectEvent {
                entity: hit_e,
                effect: StatusEffect::Slow,
                num_stacks: slow_stacks.num_stacks as i32,
            });
        }
    } else {
        commands.entity(hit_e).insert(Slow {
            num_stacks: 1,
            timer: Timer::from_seconds(1.7, TimerMode::Repeating),
        });
        status_event.send(StatusEffectEvent {
            entity: hit_e,
            effect: StatusEffect::Slow,
            num_stacks: 1,
        });
    }
}
