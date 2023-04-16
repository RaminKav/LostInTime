use bevy::{prelude::*, time::FixedTimestep};
use bevy_rapier2d::prelude::RapierContext;

use crate::{
    animations::HitAnimationTracker,
    attributes::{Attack, Health},
    item::WorldObject,
    GameState, TIME_STEP,
};

#[derive(Debug, Clone)]
pub struct HitEvent {
    pub hit_entity: Entity,
    pub damage: u8,
    pub dir: Vec2,
}

#[derive(Debug, Clone)]

pub struct EnemyDeathEvent {
    pub entity: Entity,
}

#[derive(Component, Debug, Clone)]
pub struct AttackTimer(pub Timer);

#[derive(Component, Debug, Clone)]

pub struct HitMarker;

pub struct CombatPlugin;
impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<HitEvent>()
            .add_event::<EnemyDeathEvent>()
            .add_system_set(
                SystemSet::on_update(GameState::Main)
                    .with_run_criteria(FixedTimestep::step(TIME_STEP as f64))
                    .with_system(Self::handle_hits)
                    .with_system(Self::handle_enemy_death)
                    .with_system(Self::check_hit_collisions),
            );
    }
}

impl CombatPlugin {
    fn handle_enemy_death(mut commands: Commands, mut death_events: EventReader<EnemyDeathEvent>) {
        for death_event in death_events.iter() {
            commands.entity(death_event.entity).despawn();
        }
    }

    fn handle_hits(
        mut commands: Commands,
        mut health: Query<(Entity, &mut Health, Option<&WorldObject>)>,
        mut hit_events: EventReader<HitEvent>,
        mut death_events: EventWriter<EnemyDeathEvent>,
    ) {
        for hit in hit_events.iter() {
            if let Ok((e, mut hit_health, obj_option)) = health.get_mut(hit.hit_entity) {
                if obj_option.is_none() {
                    commands.entity(hit.hit_entity).insert(HitAnimationTracker {
                        timer: Timer::from_seconds(0.2, TimerMode::Once),
                        knockback: 400.,
                        dir: hit.dir,
                    });
                }

                hit_health.0 -= hit.damage as i8;
                if hit_health.0 <= 0 {
                    death_events.send(EnemyDeathEvent { entity: e })
                }
            }
        }
    }
    fn check_hit_collisions(
        mut commands: Commands,
        context: ResMut<RapierContext>,
        weapons: Query<(Entity, &Parent, &Attack), Without<HitMarker>>,
        mut hit_event: EventWriter<HitEvent>,
    ) {
        for weapon in weapons.iter() {
            let weapon_parent = weapon.1;
            if let Some(hit) = context.intersection_pairs().find(|c| {
                (c.0 == weapon.0 && c.1 != weapon_parent.get())
                    || (c.1 == weapon.0 && c.0 != weapon_parent.get())
            }) {
                commands.entity(weapon.0).insert(HitMarker);

                hit_event.send(HitEvent {
                    hit_entity: if hit.0 == weapon.0 { hit.1 } else { hit.0 },
                    damage: weapon.2 .0,
                    dir: Vec2::new(0., 0.),
                });
            }
        }
    }
}
