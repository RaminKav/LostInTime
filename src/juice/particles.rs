use bevy::prelude::*;
use bevy_hanabi::prelude::*;
use bevy_hanabi::Gradient as HanabiGradient;

use crate::{
    assets::SpriteAnchor,
    colors::YELLOW,
    combat::{EnemyDeathEvent, HitEvent, ObjBreakEvent},
    ecs_helpers::SafeHierarchyExt,
    enemy::Mob,
    inputs::MovementVector,
    item::WorldObject,
    night::InfiniteMode,
    player::Player,
    proto::proto_param::ProtoParam,
    world::{
        world_helpers::tile_pos_to_world_pos,
        y_sort::{y_sort_depth, YSort},
    },
    Game, GameParam,
};

use super::{CpuParticleGenerator, CpuParticleType};

const DUST_OFFSET: Vec2 = Vec2::new(3., 4.);

/// Slightly above mob feet (`YSort(0)`) so hit/death bursts sit on the body, not
/// under the sprite. Hanabi 2D sorts by the effect entity's `Transform::z`.
const COMBAT_PARTICLE_Y_SORT: f32 = 0.05;

fn combat_particle_depth(world_pos: Vec2) -> f32 {
    y_sort_depth(COMBAT_PARTICLE_Y_SORT, world_pos.y, world_pos.x, 0.)
}

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
#[derive(Message)]
pub struct UseItemEvent(pub WorldObject);

#[derive(Component)]
pub struct ObjectHitParticles {
    pub despawn_timer: Timer,
    pub velocity: Vec3,
}

struct BurstEffectConfig {
    name: &'static str,
    spawner: SpawnerSettings,
    radius: f32,
    speed: f32,
    lifetime: f32,
    size_range: (f32, f32),
    gradient: Option<HanabiGradient<Vec4>>,
    color_property: bool,
    default_accel: Vec3,
    /// When false, omit `AccelModifier`. 0.10 death FX declared `my_accel` but
    /// never wired accel — so `cleanup_object_particles` gravity was a no-op.
    use_accel: bool,
}

fn build_burst_effect(config: BurstEffectConfig) -> EffectAsset {
    let BurstEffectConfig {
        name,
        spawner,
        radius,
        speed,
        lifetime,
        size_range,
        gradient,
        color_property,
        default_accel,
        use_accel,
    } = config;

    let writer = ExprWriter::new();

    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.).expr());
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer.lit(lifetime).expr());

    let init_pos = SetPositionCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Z).expr(),
        radius: writer.lit(radius).expr(),
        dimension: ShapeDimension::Surface,
    };

    let init_vel = SetVelocityCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Z).expr(),
        speed: writer.lit(speed).expr(),
    };

    let (size_min, size_max) = size_range;
    let size_span = size_max - size_min;
    // Hanabi 0.19 `Attribute::SIZE` is a scalar (uniform size), not Vec3.
    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        (writer.rand(ScalarType::Float) * writer.lit(size_span) + writer.lit(size_min)).expr(),
    );

    let my_accel = writer.add_property("my_accel", default_accel.into());
    let init_color = if color_property {
        let my_color = writer.add_property("my_color", 0xFFFFFFFFu32.into());
        Some(SetAttributeModifier::new(
            Attribute::COLOR,
            writer.prop(my_color).expr(),
        ))
    } else {
        None
    };

    let mut module = writer.finish();
    let update_accel = AccelModifier::via_property(&mut module, my_accel);

    let mut effect = EffectAsset::new(32768, spawner, module)
        .with_name(name)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .init(init_size);

    if use_accel {
        effect = effect.update(update_accel);
    }

    if let Some(init_color) = init_color {
        effect = effect.init(init_color);
    }
    if let Some(gradient) = gradient {
        effect = effect.render(ColorOverLifetimeModifier::new(gradient));
    }

    effect
}

fn build_dust_effect(gradient: HanabiGradient<Vec4>) -> EffectAsset {
    let writer = ExprWriter::new();

    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.).expr());
    let init_lifetime = SetAttributeModifier::new(Attribute::LIFETIME, writer.lit(0.2).expr());

    let init_pos = SetPositionCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Z).expr(),
        radius: writer.lit(3.5).expr(),
        dimension: ShapeDimension::Surface,
    };

    let init_vel = SetVelocityCircleModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        axis: writer.lit(Vec3::Z).expr(),
        speed: writer.lit(5.).expr(),
    };

    let init_size = SetAttributeModifier::new(
        Attribute::SIZE,
        (writer.rand(ScalarType::Float) * writer.lit(1.9) + writer.lit(0.1)).expr(),
    );

    let my_accel = writer.add_property("my_accel", Vec3::new(-3., -3., 0.).into());
    let mut module = writer.finish();
    let update_accel = AccelModifier::via_property(&mut module, my_accel);

    EffectAsset::new(
        32768,
        SpawnerSettings::once(10.0.into()).with_emit_on_start(false),
        module,
    )
    .with_name("dust_particle_emit")
    .init(init_pos)
    .init(init_vel)
    .init(init_age)
    .init(init_lifetime)
    .init(init_size)
    .update(update_accel)
    .render(ColorOverLifetimeModifier::new(gradient))
}

pub fn setup_particles(
    mut commands: Commands,
    mut effects: ResMut<Assets<EffectAsset>>,
    player: Query<Entity, Added<Player>>,
    old_particles: Option<Res<Particles>>,
) {
    for player_e in player.iter() {
        if let Some(ref old) = old_particles {
            effects.remove(&old.obj_hit_particle);
            effects.remove(&old.enemy_death_particle);
            effects.remove(&old.use_item_particle);
            effects.remove(&old.enemy_hit_particles);
            effects.remove(&old.xp_particles);
        }

        let mut gradient = HanabiGradient::new();
        gradient.add_key(0.0, Vec4::new(208. / 255., 165. / 255., 106. / 255., 0.8));
        gradient.add_key(1.0, Vec4::new(208. / 255., 165. / 255., 106. / 255., 0.0));

        let mut gradient3 = HanabiGradient::new();
        gradient3.add_key(0.0, Vec4::new(255. / 255., 255. / 255., 255. / 255., 0.8));
        gradient3.add_key(1.0, Vec3::splat(0.4).extend(0.2));

        let mut gradient4 = HanabiGradient::new();
        gradient4.add_key(0.0, Vec4::new(170. / 255., 39. / 255., 44. / 255., 1.));
        gradient4.add_key(1.0, Vec4::new(170. / 255., 39. / 255., 44. / 255., 0.));

        let effect3 = effects.add(build_dust_effect(gradient));

        let obj_hit_particle = effects.add(build_burst_effect(BurstEffectConfig {
            name: "emit:obj_hit_particle",
            spawner: SpawnerSettings::once(CpuValue::Uniform((10., 35.))),
            radius: 1.5,
            speed: 25.,
            lifetime: 0.23,
            size_range: (0.1, 2.),
            gradient: None,
            color_property: true,
            default_accel: Vec3::new(-3., -3., 0.),
            use_accel: true,
        }));

        let enemy_death_particle = effects.add(build_burst_effect(BurstEffectConfig {
            name: "emit:enemy_death_particles",
            spawner: SpawnerSettings::once(CpuValue::Uniform((10., 20.))),
            radius: 2.,
            speed: 7.,
            lifetime: 0.7,
            size_range: (2., 8.),
            gradient: Some(gradient3),
            color_property: false,
            default_accel: Vec3::ZERO,
            use_accel: false,
        }));

        let use_item_particle = effects.add(build_burst_effect(BurstEffectConfig {
            name: "emit:use_item",
            spawner: SpawnerSettings::once(CpuValue::Uniform((10., 35.))),
            radius: 1.5,
            speed: 25.,
            lifetime: 0.23,
            size_range: (0.1, 4.),
            gradient: Some(gradient4),
            color_property: false,
            default_accel: Vec3::new(-3., -3., 0.),
            use_accel: true,
        }));

        let enemy_hit_particles = effects.add(build_burst_effect(BurstEffectConfig {
            name: "emit:enemy_hit",
            spawner: SpawnerSettings::once(CpuValue::Uniform((20., 55.))),
            radius: 2.,
            speed: 16.,
            lifetime: 0.23,
            size_range: (0.1, 3.),
            gradient: None,
            color_property: true,
            default_accel: Vec3::new(-3., -3., 0.),
            use_accel: true,
        }));

        let xp_particles = effects.add(build_burst_effect(BurstEffectConfig {
            name: "emit:xp",
            spawner: SpawnerSettings::once(CpuValue::Uniform((20., 55.))),
            radius: 1.5,
            speed: 0.,
            lifetime: 0.23,
            size_range: (0.05, 2.),
            gradient: None,
            color_property: true,
            default_accel: Vec3::ZERO,
            use_accel: true,
        }));

        commands.insert_resource(Particles {
            obj_hit_particle,
            enemy_death_particle,
            use_item_particle,
            enemy_hit_particles,
            xp_particles,
        });

        commands
            .spawn((
                Name::new("dust_particles"),
                ParticleEffect::new(effect3),
                Transform::from_translation(DUST_OFFSET.extend(2.)),
                DustParticles,
            ))
            .safe_set_parent(player_e);
    }
}

pub fn update_dust_particle_dir(
    mut dust: Query<&mut Transform, With<DustParticles>>,
    player_move: Query<&MovementVector, (With<Player>, Changed<MovementVector>)>,
) {
    let Ok(mv) = player_move.single() else {
        return;
    };
    let Ok(mut dust_t) = dust.single_mut() else {
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

fn spawn_colored_burst(
    commands: &mut Commands,
    effect: Handle<EffectAsset>,
    world_pos: Vec2,
    depth: f32,
    color: u32,
    despawn_timer: Timer,
    velocity: Vec3,
) {
    commands.spawn((
        Name::new("emit:burst"),
        ParticleEffect::new(effect),
        EffectProperties::default().with_properties([
            ("my_color".to_string(), color.into()),
            ("my_accel".to_string(), Vec3::ZERO.into()),
        ]),
        Transform::from_translation(world_pos.extend(depth)),
        YSort(COMBAT_PARTICLE_Y_SORT),
        ObjectHitParticles {
            despawn_timer,
            velocity,
        },
    ));
}

fn spawn_burst(
    commands: &mut Commands,
    effect: Handle<EffectAsset>,
    world_pos: Vec2,
    depth: f32,
    despawn_timer: Timer,
    velocity: Vec3,
) {
    commands.spawn((
        Name::new("emit:burst"),
        ParticleEffect::new(effect),
        EffectProperties::default().with_properties([("my_accel".to_string(), Vec3::ZERO.into())]),
        Transform::from_translation(world_pos.extend(depth)),
        YSort(COMBAT_PARTICLE_Y_SORT),
        ObjectHitParticles {
            despawn_timer,
            velocity,
        },
    ));
}

pub fn spawn_obj_hit_particles(
    mut commands: Commands,
    mut hit_events: MessageReader<HitEvent>,
    game: Res<Game>,
    transforms: Query<&GlobalTransform>,
    particles: Res<Particles>,
    mob_query: Query<&Mob>,
    world_object: Query<(&WorldObject, &SpriteAnchor)>,
    infinite_mode: Res<InfiniteMode>,
) {
    if infinite_mode.active {
        return;
    }
    for hit in hit_events.read() {
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

        let color = if is_mob {
            mob_query.get(hit.hit_entity).unwrap().get_mob_color()
        } else if is_object {
            world_object.get(hit.hit_entity).unwrap().0.get_obj_color()
        } else {
            continue;
        };

        let world_pos = hit_pos.truncate() + anchor * -1. + Vec2::new(0., 4.);
        let depth = combat_particle_depth(world_pos);

        spawn_colored_burst(
            &mut commands,
            effect,
            world_pos,
            depth,
            color.to_linear().as_u32(),
            Timer::from_seconds(0.23, TimerMode::Once),
            Vec3::new(0., 8000., 0.),
        );
    }
}

pub fn spawn_use_item_particles(
    mut commands: Commands,
    mut use_item_events: MessageReader<UseItemEvent>,
    game: Res<Game>,
    transforms: Query<&GlobalTransform>,
    particles: Res<Particles>,
) {
    for _event in use_item_events.read() {
        let hit_pos = transforms.get(game.player).unwrap().translation();
        let world_pos = hit_pos.truncate() + Vec2::new(0., 5.);
        let depth = combat_particle_depth(world_pos);

        spawn_burst(
            &mut commands,
            particles.use_item_particle.clone(),
            world_pos,
            depth,
            Timer::from_seconds(1., TimerMode::Once),
            Vec3::new(0., 10000., 0.),
        );
    }
}

pub fn spawn_enemy_death_particles(
    mut commands: Commands,
    mut death_events: MessageReader<EnemyDeathEvent>,
    particles: Res<Particles>,
) {
    for death_event in death_events.read() {
        let t = death_event.enemy_pos;
        let world_pos = t + Vec2::new(0., 4.);
        let depth = combat_particle_depth(world_pos);

        spawn_burst(
            &mut commands,
            particles.enemy_death_particle.clone(),
            world_pos,
            depth,
            Timer::from_seconds(1.1, TimerMode::Once),
            Vec3::new(0., 8000., 0.),
        );
    }
}

pub fn spawn_obj_death_particles(
    mut commands: Commands,
    mut death_events: MessageReader<ObjBreakEvent>,
    particles: Res<Particles>,
    proto_param: ProtoParam,
) {
    for death_event in death_events.read() {
        let t = tile_pos_to_world_pos(death_event.pos, true);
        if (!death_event.obj.is_medium_size(&proto_param) && !death_event.obj.is_tree())
            && death_event.obj != WorldObject::Crate
            && death_event.obj != WorldObject::Crate2
        {
            return;
        }

        let world_pos = Vec2::new(t.x as f32, t.y + 4.);
        let depth = combat_particle_depth(world_pos);

        spawn_burst(
            &mut commands,
            particles.enemy_death_particle.clone(),
            world_pos,
            depth,
            Timer::from_seconds(1.1, TimerMode::Once),
            Vec3::new(0., 8000., 0.),
        );
    }
}

pub fn cleanup_object_particles(
    mut commands: Commands,
    mut particles: Query<(Entity, &mut EffectProperties, &mut ObjectHitParticles)>,
    time: Res<Time>,
) {
    for (e, mut effect, mut p) in particles.iter_mut() {
        let accel = -3500.;
        p.velocity.y += accel;

        effect.set("my_accel", (p.velocity * time.delta_secs()).into());

        p.despawn_timer.tick(time.delta());
        if p.despawn_timer.is_finished() {
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
            &mut EffectProperties,
            &mut ObjectHitParticles,
        ),
        With<ExpParticles>,
    >,
    game: GameParam,
    time: Res<Time>,
) {
    for (e, t, mut effect, mut p) in particles.iter_mut() {
        let xp_bar_txfm = game.player().position.truncate();
        let delta = xp_bar_txfm - t.translation.truncate();
        let delta_norm = delta.normalize();

        let accel = 4000.;
        p.velocity.y += delta_norm.y * accel;
        p.velocity.x += delta_norm.x * accel;
        effect.set("my_accel", (p.velocity * time.delta_secs()).into());
        if delta.length() <= 3. {
            p.despawn_timer.tick(time.delta());
        }
        if p.despawn_timer.is_finished() {
            commands.entity(e).despawn();
        }
    }
}

pub fn spawn_xp_particles(t: Vec2, commands: &mut Commands, amount: u32, did_level_up: bool) {
    commands.spawn((
        Transform::from_translation(t.extend(0.)),
        CpuParticleGenerator {
            min_particle_size: 1. + f32::min(f32::floor(amount as f32 / 10.), 3.),
            max_particle_size: 2. + f32::min(f32::floor(amount as f32 / 10.), 10.),
            min_particle_count: 1 + f32::min(f32::floor(amount as f32 / 5.), 2.) as usize,
            max_particle_count: 3 + f32::min(f32::floor(amount as f32 / 5.), 3.) as usize,
            pos_offset: Vec2::ZERO,
            min_spawn_radius: 6.,
            max_spawn_radius: 12.,
            color: YELLOW,
            lifetime: 100.,
            particle_type: CpuParticleType::Exp(amount, did_level_up),
        },
    ));
}
