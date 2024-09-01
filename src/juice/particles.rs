use bevy::prelude::*;

use bevy_hanabi::prelude::*;

use crate::{
    assets::SpriteAnchor,
    colors::YELLOW,
    combat::{EnemyDeathEvent, HitEvent, JustGotHit, ObjBreakEvent},
    enemy::Mob,
    inputs::MovementVector,
    item::WorldObject,
    player::{levels::ExperienceReward, Player},
    proto::proto_param::ProtoParam,
    world::{world_helpers::tile_pos_to_world_pos, y_sort::YSort},
    Game, GameParam,
};

use super::{CpuParticleGenerator, CpuParticleType};

const DUST_OFFSET: Vec2 = Vec2::new(3., 4.);

#[derive(Component)]
pub struct RunDustTimer(pub Timer);

#[derive(Resource, Default, Debug)]
pub struct Particles {
    pub obj_hit_particle: Handle<EffectAsset>,
    pub enemy_death_particle: Handle<EffectAsset>,
    pub use_item_particle: Handle<EffectAsset>,
    pub enemy_hit_particles: Handle<EffectAsset>,
    pub xp_particles: Handle<EffectAsset>,
}

#[derive(Component)]
pub struct DustParticles;
#[derive(Component)]
pub struct ExpParticles;
#[derive(Event)]
pub struct UseItemEvent(pub WorldObject);

#[derive(Component)]
pub struct ObjectHitParticles {
    pub despawn_timer: Timer,
    pub velocity: Vec3,
}

pub fn setup_particles(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    player: Query<Entity, Added<Player>>,
) {
    for player_e in player.iter() {
        // Note: same as gradient2, will yield shared render shader between effects #2
        let mut gradient = Gradient::new();
        gradient.add_key(0.0, Vec4::new(208. / 255., 165. / 255., 106. / 255., 0.8));
        gradient.add_key(1.0, Vec4::new(208. / 255., 165. / 255., 106. / 255., 0.0));
        // gradient.add_key(1.0, Vec4::splat(0.0));
        let mut gradient2 = Gradient::new();
        gradient2.add_key(0.0, Vec4::new(163. / 255., 182. / 255., 69. / 255., 1.));
        gradient2.add_key(1.0, Vec4::new(163. / 255., 182. / 255., 69. / 255., 0.0));

        let mut gradient3 = Gradient::new();
        gradient3.add_key(0.0, Vec4::new(255. / 255., 255. / 255., 255. / 255., 0.8));
        gradient3.add_key(1.0, Vec3::splat(0.4).extend(0.2));
        let mut gradient4 = Gradient::new();
        gradient4.add_key(0.0, Vec4::new(170. / 255., 39. / 255., 44. / 255., 1.));
        gradient4.add_key(1.0, Vec4::new(170. / 255., 39. / 255., 44. / 255., 0.));
        let mut gradient4 = Gradient::new();
        gradient4.add_key(0.0, Vec4::new(170. / 255., 39. / 255., 44. / 255., 1.));
        gradient4.add_key(1.0, Vec4::new(170. / 255., 39. / 255., 44. / 255., 0.));
        let effect3 = effects.add(
            EffectAsset {
                name: "emit:burst".to_string(),
                capacity: 32768,
                spawner: Spawner::once(10.0.into(), false),
                ..Default::default()
            }
            .with_property("my_accel", graph::Value::Float3(Vec3::new(-3., -3., 0.)))
            .init(InitPositionCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                radius: 3.5,
                dimension: ShapeDimension::Surface,
            })
            .init(InitVelocityCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                speed: 5_f32.into(),
            })
            .init(InitLifetimeModifier {
                lifetime: 0.2_f32.into(),
            })
            .init(InitSizeModifier {
                // At spawn time, assign each particle a random size between 0.3 and 0.7
                size: Value::<f32>::Uniform((0.1, 2.)).into(),
            })
            .update(AccelModifier::via_property("my_accel"))
            .render(ColorOverLifetimeModifier { gradient }),
        );
        let obj_hit_particle = effects.add(
            EffectAsset {
                name: "emit:hit".to_string(),
                capacity: 32768,
                spawner: Spawner::once(Value::Uniform((10., 35.)), true),
                ..Default::default()
            }
            .with_property("my_accel", graph::Value::Float3(Vec3::new(-3., -3., 0.)))
            .with_property("my_color", graph::Value::Uint(0xFFFFFFFF))
            .init(InitPositionCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                radius: 1.5,
                dimension: ShapeDimension::Surface,
            })
            .init(InitVelocityCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                speed: 25_f32.into(),
            })
            .init(InitLifetimeModifier {
                lifetime: 0.23_f32.into(),
            })
            .init(InitSizeModifier {
                // At spawn time, assign each particle a random size between 0.3 and 0.7
                size: Value::<f32>::Uniform((0.1, 2.)).into(),
            })
            .init(InitAttributeModifier {
                attribute: Attribute::COLOR,
                value: "my_color".into(),
            })
            .update(AccelModifier::via_property("my_accel")),
        );
        let enemy_death_particle = effects.add(
            EffectAsset {
                name: "emit:hit".to_string(),
                capacity: 32768,
                spawner: Spawner::once(Value::Uniform((10., 20.)), true),
                ..Default::default()
            }
            .with_property("my_accel", graph::Value::Float3(Vec3::new(-3., -3., 0.)))
            .init(InitPositionCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                radius: 2.,
                dimension: ShapeDimension::Surface,
            })
            .init(InitVelocityCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                speed: 7_f32.into(),
            })
            .init(InitLifetimeModifier {
                lifetime: 0.7_f32.into(),
            })
            .init(InitSizeModifier {
                // At spawn time, assign each particle a random size between 0.3 and 0.7
                size: Value::<f32>::Uniform((2., 8.)).into(),
            })
            .render(ColorOverLifetimeModifier {
                gradient: gradient3,
            }),
        );
        let use_item_particle = effects.add(
            EffectAsset {
                name: "emit:hit".to_string(),
                capacity: 32768,
                spawner: Spawner::once(Value::Uniform((10., 35.)), true),
                ..Default::default()
            }
            .with_property("my_accel", graph::Value::Float3(Vec3::new(-3., -3., 0.)))
            .init(InitPositionCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                radius: 1.5,
                dimension: ShapeDimension::Surface,
            })
            .init(InitVelocityCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                speed: 25_f32.into(),
            })
            .init(InitLifetimeModifier {
                lifetime: 0.23_f32.into(),
            })
            .init(InitSizeModifier {
                // At spawn time, assign each particle a random size between 0.3 and 0.7
                size: Value::<f32>::Uniform((0.1, 4.)).into(),
            })
            .update(AccelModifier::via_property("my_accel"))
            .render(ColorOverLifetimeModifier {
                gradient: gradient4,
            }),
        );
        let enemy_hit_particles = effects.add(
            EffectAsset {
                name: "emit:hit".to_string(),
                capacity: 32768,
                spawner: Spawner::once(Value::Uniform((20., 55.)), true),
                ..Default::default()
            }
            .with_property("my_accel", graph::Value::Float3(Vec3::new(-3., -3., 0.)))
            .with_property("my_color", graph::Value::Uint(0xFFFFFFFF))
            .init(InitPositionCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                radius: 2.,
                dimension: ShapeDimension::Surface,
            })
            .init(InitVelocityCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                speed: 16_f32.into(),
            })
            .init(InitLifetimeModifier {
                lifetime: 0.23_f32.into(),
            })
            .init(InitSizeModifier {
                // At spawn time, assign each particle a random size between 0.3 and 0.7
                size: Value::<f32>::Uniform((0.1, 3.)).into(),
            })
            .init(InitAttributeModifier {
                attribute: Attribute::COLOR,
                value: "my_color".into(),
            })
            .update(AccelModifier::via_property("my_accel")),
        );
        let xp_particles = effects.add(
            EffectAsset {
                name: "emit:hit".to_string(),
                capacity: 32768,
                spawner: Spawner::once(Value::Uniform((20., 55.)), true),
                ..Default::default()
            }
            .with_property("my_accel", graph::Value::Float3(Vec3::new(0., 0., 0.)))
            .with_property("my_color", graph::Value::Uint(0xFFFFFFFF))
            .init(InitPositionCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                radius: 1.5,
                dimension: ShapeDimension::Surface,
            })
            .init(InitVelocityCircleModifier {
                center: Vec3::ZERO,
                axis: Vec3::Z,
                speed: 0_f32.into(),
            })
            .init(InitLifetimeModifier {
                lifetime: 0.23_f32.into(),
            })
            .init(InitSizeModifier {
                size: Value::<f32>::Uniform((0.05, 2.)).into(),
            })
            .init(InitAttributeModifier {
                attribute: Attribute::COLOR,
                value: "my_color".into(),
            })
            .update(AccelModifier::via_property("my_accel")),
        );
        commands.insert_resource(Particles {
            obj_hit_particle,
            enemy_death_particle,
            use_item_particle,
            enemy_hit_particles,
            xp_particles,
        });
        commands
            .spawn((
                Name::new("emit:burst"),
                ParticleEffectBundle {
                    effect: ParticleEffect::new(effect3).with_z_layer_2d(Some(2.)),
                    transform: Transform::from_translation(DUST_OFFSET.extend(1.)),
                    ..Default::default()
                },
                DustParticles,
            ))
            .set_parent(player_e);
    }
}

pub fn update_dust_particle_dir(
    mut dust: Query<&mut Transform, With<DustParticles>>,
    player_move: Query<&MovementVector, (With<Player>, Changed<MovementVector>)>,
) {
    let Ok(mv) = player_move.get_single() else {
        return;
    };
    let Ok(mut dust_t) = dust.get_single_mut() else {
        return;
    };
    let mut is_moving_up = true;
    let movement_offset = Vec2::new(
        if mv.0.x > 0. {
            1.
        } else if mv.0.x < 0. {
            -1.
        } else {
            0.
        },
        if mv.0.y > 0. {
            1.
        } else if mv.0.y < 0. {
            is_moving_up = false;
            -1.
        } else {
            0.
        },
    );
    dust_t.translation.x = DUST_OFFSET.x * movement_offset.x * -1.;
    dust_t.translation.y =
        DUST_OFFSET.y * movement_offset.y * -1. + if is_moving_up { -6. } else { 6. };
}

pub fn spawn_obj_hit_particles(
    mut commands: Commands,
    mut hit_events: EventReader<HitEvent>,
    game: Res<Game>,
    transforms: Query<&GlobalTransform>,
    particles: Res<Particles>,
    mob_query: Query<&Mob>,
    world_object: Query<(&WorldObject, &SpriteAnchor)>,
) {
    // add spark animation entity as child, will animate once and remove itself.
    for hit in hit_events.iter() {
        if hit.hit_entity == game.player {
            continue;
        }
        let hit_pos = if let Ok(txfm) = transforms.get(hit.hit_entity) {
            txfm.translation()
        } else {
            continue;
        };
        let anchor = if let Ok(anchor) = world_object.get(hit.hit_entity) {
            anchor.1 .0
        } else {
            Vec2::ZERO
        };

        let is_mob = mob_query.get(hit.hit_entity).is_ok();
        let is_object = world_object.get(hit.hit_entity).is_ok();
        let effect = if is_mob {
            particles.enemy_hit_particles.clone()
        } else {
            particles.obj_hit_particle.clone()
        };
        //TODO: fix this bs unwrap panic
        let color = if is_mob {
            mob_query.get(hit.hit_entity).unwrap().get_mob_color()
        } else if is_object {
            world_object.get(hit.hit_entity).unwrap().0.get_obj_color()
        } else {
            continue;
        };

        commands.spawn((
            Name::new("emit:burst"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(effect)
                    .with_properties::<ParticleEffect>(vec![(
                        "my_color".to_string(),
                        graph::Value::Uint(color.as_linear_rgba_u32()),
                    )])
                    .with_z_layer_2d(Some(999.)),
                transform: Transform::from_translation(
                    Vec3::new(hit_pos.x, hit_pos.y + 4., 2.) + (anchor.extend(0.) * -1.),
                ),
                ..Default::default()
            },
            YSort(1.),
            ObjectHitParticles {
                despawn_timer: Timer::from_seconds(0.23, TimerMode::Once),
                velocity: Vec3::new(0., 8000., 0.),
            },
        ));

        commands.entity(hit.hit_entity).remove::<JustGotHit>();
    }
}
pub fn spawn_use_item_particles(
    mut commands: Commands,
    mut use_item_events: EventReader<UseItemEvent>,
    game: Res<Game>,
    transforms: Query<&GlobalTransform>,
    particles: Res<Particles>,
) {
    // add spark animation entity as child, will animate once and remove itself.
    for _event in use_item_events.iter() {
        let hit_pos = transforms.get(game.player).unwrap().translation();

        commands.spawn((
            Name::new("emit:burst"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(particles.use_item_particle.clone())
                    .with_z_layer_2d(Some(999.)),
                transform: Transform::from_translation(Vec3::new(hit_pos.x, hit_pos.y + 5., 2.)),
                ..Default::default()
            },
            YSort(1.),
            ObjectHitParticles {
                despawn_timer: Timer::from_seconds(1., TimerMode::Once),
                velocity: Vec3::new(0., 10000., 0.),
            },
        ));
    }
}
pub fn spawn_enemy_death_particles(
    mut commands: Commands,
    mut death_events: EventReader<EnemyDeathEvent>,
    xp: Query<&ExperienceReward>,
    particles: Res<Particles>,
) {
    for death_event in death_events.iter() {
        let t = death_event.enemy_pos;

        commands.spawn((
            Name::new("emit:burst"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(particles.enemy_death_particle.clone())
                    .with_z_layer_2d(Some(999.)),
                transform: Transform::from_translation(Vec3::new(t.x, t.y + 4., 2.)),
                ..Default::default()
            },
            YSort(1.),
            ObjectHitParticles {
                despawn_timer: Timer::from_seconds(1.1, TimerMode::Once),
                velocity: Vec3::new(0., 8000., 0.),
            },
        ));
        if let Ok(xp) = xp.get(death_event.entity) {
            spawn_xp_particles(t, &mut commands, xp.0 as f32);
        }
    }
}

pub fn spawn_obj_death_particles(
    mut commands: Commands,
    mut death_events: EventReader<ObjBreakEvent>,
    particles: Res<Particles>,
    proto_param: ProtoParam,
) {
    for death_event in death_events.iter() {
        let t = tile_pos_to_world_pos(death_event.pos, true);
        if (!death_event.obj.is_medium_size(&proto_param) && !death_event.obj.is_tree())
            && death_event.obj != WorldObject::Crate
            && death_event.obj != WorldObject::Crate2
        {
            return;
        }

        commands.spawn((
            Name::new("emit:burst"),
            ParticleEffectBundle {
                effect: ParticleEffect::new(particles.enemy_death_particle.clone())
                    .with_z_layer_2d(Some(999.)),
                transform: Transform::from_translation(Vec3::new(t.x as f32, t.y + 4., 2.)),
                ..Default::default()
            },
            YSort(1.),
            ObjectHitParticles {
                despawn_timer: Timer::from_seconds(1.1, TimerMode::Once),
                velocity: Vec3::new(0., 8000., 0.),
            },
        ));
    }
}

pub fn cleanup_object_particles(
    mut commands: Commands,
    mut particles: Query<(Entity, &mut CompiledParticleEffect, &mut ObjectHitParticles)>,
    time: Res<Time>,
) {
    for (e, mut effect, mut p) in particles.iter_mut() {
        // t.translation.y -= 10.;
        let accel = -3500.;
        p.velocity.y += accel;

        effect.set_property(
            "my_accel",
            graph::Value::Float3(p.velocity * time.delta_seconds()),
        );

        p.despawn_timer.tick(time.delta());
        if p.despawn_timer.finished() {
            commands.entity(e).despawn();
        }
    }
}

pub fn handle_exp_particles(
    mut commands: Commands,
    mut particles: Query<
        (
            Entity,
            &Transform,
            &mut CompiledParticleEffect,
            &mut ObjectHitParticles,
        ),
        With<ExpParticles>,
    >,
    game: GameParam,
    time: Res<Time>,
) {
    for (e, t, mut effect, mut p) in particles.iter_mut() {
        let xp_bar_txfm = game.player().position.truncate(); //+ Vec2::new(0., -5.5 * TILE_SIZE.x);
        let delta = xp_bar_txfm - t.translation.truncate();
        let delta_norm = delta.normalize();

        // t.translation.y -= 10.;
        let accel = 4000.;
        p.velocity.y += delta_norm.y * accel;
        p.velocity.x += delta_norm.x * accel;
        effect.set_property(
            "my_accel",
            graph::Value::Float3(p.velocity * time.delta_seconds()),
        );
        if delta.length() <= 3. {
            p.despawn_timer.tick(time.delta());
        }
        if p.despawn_timer.finished() {
            commands.entity(e).despawn();
        }
    }
}

pub fn spawn_xp_particles(t: Vec2, commands: &mut Commands, amount: f32) {
    commands.spawn((
        TransformBundle::from_transform(Transform::from_translation(t.extend(0.))),
        CpuParticleGenerator {
            min_particle_size: 1. + f32::floor(amount / 10.),
            max_particle_size: 2. + f32::floor(amount / 10.),
            min_particle_count: 1 + f32::floor(amount / 5.) as usize,
            max_particle_count: 3 + f32::floor(amount / 5.) as usize,
            pos_offset: Vec2::ZERO,
            min_spawn_radius: 6.,
            max_spawn_radius: 12.,
            color: YELLOW,
            lifetime: 100.,
            particle_type: CpuParticleType::Exp,
        },
    ));
}
