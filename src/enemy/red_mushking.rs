use crate::{
    custom_commands::CommandsExt,
    enemy::spawn_helpers::can_spawn_mob_here,
    item::{LootTable, WorldObject},
    juice::ShakeEffect,
    night::InfiniteModeStartedEvent,
    player::levels::ExperienceReward,
    world::{
        dimension::{Era, EraManager},
        world_helpers::tile_pos_to_world_pos,
    },
    GameParam, TextureCamera,
};
use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::{
    control::KinematicCharacterController,
    geometry::{Collider, Sensor},
};
use rand::Rng;
use seldom_state::{
    prelude::StateMachine,
    trigger::{BoolTrigger, Trigger},
};

use crate::{
    ai::{EnemyAttackCooldown, FollowState, LeapAttackState},
    attributes::{Attack, CurrentHealth, MaxHealth},
    collisions::DamagesWorldObjects,
    item::projectile::{Projectile, RangedAttackEvent},
    player::Player,
    proto::proto_param::ProtoParam,
    PLAYER_MOVE_SPEED,
};
use bevy::prelude::shape;
use bevy::sprite::{ColorMaterial, MaterialMesh2dBundle};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use super::{FollowSpeed, LeapAttack, Mob, MobIsAttacking};

aseprite!(pub RedMushking, "textures/redmushking/red_mushking.ase");
// Spawn as IDLE
// Always Aggro on player, follow using WALK, unless player out of range from spawn shrine
// perform jump attack in 2 variations
//  - jump in place 3 times quickly (rarely)
//  - jump to player and attack (very often)
// every so often, use summon attack

const MAX_JUMP_DISTANCE: f32 = 16. * 5.5;

pub fn handle_new_red_mushking_state_machine(
    mut commands: Commands,
    spawn_events: Query<(Entity, &Mob, &Transform, &FollowSpeed, &LeapAttack), Added<Mob>>,
    asset_server: Res<AssetServer>,
    game: GameParam,
) {
    for (e, mob, transform, follow_speed, leap_attack) in spawn_events.iter() {
        if mob != &Mob::RedMushking {
            continue;
        }
        let mut e_cmds = commands.entity(e);
        let mut animation = AsepriteAnimation::from(RedMushking::tags::IDLE);
        animation.play();

        let shrine_pos = tile_pos_to_world_pos(
            *game
                .world_obj_cache
                .unique_objs
                .get(&WorldObject::BossShrine)
                .expect("no shrine found"),
            false,
        );

        e_cmds
            .insert(AsepriteBundle {
                aseprite: asset_server.load(RedMushking::PATH),
                animation,
                transform: *transform,
                ..Default::default()
            })
            .insert(FollowState {
                target: game.game.player,
                curr_delta: None,
                curr_path: None,
                speed: follow_speed.0,
            })
            .insert(DamagesWorldObjects)
            .insert(HealthThreshold(1.))
            .insert(AttackCollider(None))
            .insert(AoEAttackTimer {
                random_timer: Timer::from_seconds(0.0, TimerMode::Once), // Will be set randomly in tick_aoe_attack_timer
            });

        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .with_state::<DeathState>()
            .trans::<FollowState>(
                HealthTrigger(0.65),
                SummonAttackState {
                    num_summons_left: 8,
                    timer: Timer::from_seconds(0.2, TimerMode::Repeating),
                },
            )
            .trans::<LeapAttackState>(
                HealthTrigger(0.65),
                SummonAttackState {
                    num_summons_left: 8,
                    timer: Timer::from_seconds(0.2, TimerMode::Repeating),
                },
            )
            .trans::<FollowState>(
                JumpTimer,
                LeapAttackState {
                    target: game.game.player,
                    attack_startup_timer: Timer::from_seconds(leap_attack.startup, TimerMode::Once),
                    attack_duration_timer: Timer::from_seconds(
                        leap_attack.duration,
                        TimerMode::Once,
                    ),
                    attack_cooldown_timer: Timer::from_seconds(
                        leap_attack.cooldown,
                        TimerMode::Once,
                    ),
                    dir: None,
                    speed: leap_attack.speed,
                },
            )
            .trans::<FollowState>(
                AoEAttackTimerTrigger,
                AoEAttackState {
                    delay_timer: Timer::from_seconds(0.65, TimerMode::Once),
                    target_position: None,
                    preview_entity: None,
                    second_preview_entity: None,
                    second_cloud_position: None,
                },
            )
            .trans::<LeapAttackState>(
                AoEAttackTimerTrigger,
                AoEAttackState {
                    delay_timer: Timer::from_seconds(0.65, TimerMode::Once),
                    target_position: None,
                    preview_entity: None,
                    second_preview_entity: None,
                    second_cloud_position: None,
                },
            );
        // .trans::<FollowState>(
        //     Trigger::not(ShrineLOS {
        //         range: TILE_SIZE.x * 16.,
        //         shrine_pos,
        //     }),
        //     ReturnToShrineState,
        // )
        // .trans::<ReturnToShrineState>(
        //     HurtByPlayer,
        //     FollowState {
        //         target: game.game.player,
        //         curr_delta: None,
        //         curr_path: None,
        //         speed: follow_speed.0,
        //     },
        // );

        e_cmds.insert(state_machine);
    }
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct ReturnToShrineState;

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct JumpAttackState;

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct SummonAttackState {
    num_summons_left: usize,
    timer: Timer,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct DeathState;

/// Component that tracks the AoE attack timer (always present on boss)
#[derive(Component)]
pub struct AoEAttackTimer {
    pub random_timer: Timer, // Timer for random interval (1-4s)
}

/// State that is active during the AoE attack
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct AoEAttackState {
    pub delay_timer: Timer,                    // Timer for 1s delay before damage
    pub target_position: Option<Vec2>,         // Player position when attack starts
    pub preview_entity: Option<Entity>,        // Entity for warning preview (first cloud)
    pub second_preview_entity: Option<Entity>, // Entity for second cloud preview (when below half health)
    pub second_cloud_position: Option<Vec2>,   // Position for second cloud (when below half health)
}

#[derive(Component)]
pub struct BossAttackPreview; // Component for warning preview visualization

#[derive(Component)]
pub struct AttackCollider(pub Option<Entity>);

pub fn new_leap_attack(
    mut transforms: Query<&mut Transform>,
    mut attacks: Query<(
        Entity,
        &Attack,
        &mut KinematicCharacterController,
        &mut LeapAttackState,
        &FollowSpeed,
        &mut AsepriteAnimation,
        &mut AttackCollider,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    mut game_camera: Query<Entity, With<TextureCamera>>,
) {
    for (
        entity,
        attack,
        mut kcc,
        mut leap_attack,
        follow_speed,
        mut anim_state,
        mut att_collider,
    ) in attacks.iter_mut()
    {
        let frame = anim_state.current_frame();
        if anim_state.is_paused() {
            anim_state.play();
        }
        if !(14..=28).contains(&frame) {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::ATTACK_HOP);
        } else if frame == 23 {
            // BEGIN DMGING
            commands
                .entity(entity)
                .insert(MobIsAttacking(Mob::RedMushking));
            if att_collider.0.is_none() {
                let hitbox = commands
                    .spawn((
                        TransformBundle::default(),
                        *attack,
                        leap_attack.clone(),
                        Collider::capsule(Vec2::new(-17., -15.), Vec2::new(17., -15.), 20.),
                        MobIsAttacking(Mob::RedMushking),
                        Sensor,
                        DamagesWorldObjects,
                    ))
                    .set_parent(entity)
                    .id();
                att_collider.0 = Some(hitbox);
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                let speed = 10.;
                let max_mag = 80.;
                let noise = 0.5;
                let dir = Vec2::new(1., 1.);
                for e in game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(0.4, TimerMode::Once),
                        speed,
                        seed,
                        max_mag,
                        noise,
                        dir,
                    });
                }
            }
        }
        // Get the positions of the attacker and target
        let target_translation =
            transforms.get(leap_attack.target).unwrap().translation + Vec3::new(0., 18., 0.);
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;
        if frame == 16 && leap_attack.dir.is_none() {
            let delta = (target_translation - attack_translation).clamp(
                Vec3::splat(-MAX_JUMP_DISTANCE),
                Vec3::splat(MAX_JUMP_DISTANCE),
            );
            leap_attack.dir = Some(delta.truncate());
        }

        // BEGIN MOVING
        if (18..=23).contains(&frame) {
            // println!("      begin moveing {:?}", time.delta_seconds());
            kcc.translation =
                Some((leap_attack.dir.unwrap_or(Vec2::ZERO) * time.delta_seconds()) * 10. / 6.);
        }
        // END LEAP ATTACK
        if frame == 28 {
            // println!("              end leap attack");
            commands
                .entity(entity)
                .insert(FollowState {
                    target: leap_attack.target,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .insert(EnemyAttackCooldown(
                    leap_attack.attack_cooldown_timer.clone(),
                ))
                .remove::<LeapAttackState>()
                .remove::<MobIsAttacking>();
            *anim_state = AsepriteAnimation::from(RedMushking::tags::WALK);
            if let Some(hitbox) = att_collider.0 {
                commands.entity(hitbox).despawn_recursive();
                att_collider.0 = None;
            }
        }
    }
}
pub fn summon_attack(
    mut attacks: Query<(
        Entity,
        &mut SummonAttackState,
        &mut AsepriteAnimation,
        &GlobalTransform,
        &FollowSpeed,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    game: GameParam,
) {
    for (entity, mut summon_attack, mut anim_state, txfm, follow_speed) in attacks.iter_mut() {
        let frame = anim_state.current_frame();
        if anim_state.is_paused() {
            anim_state.play();
        }
        if !(29..=42).contains(&frame) {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::START_SUMMON);
        } else if frame == 33 {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::SUMMONING);
        } else if (33..=39).contains(&frame) {
            // SUMMONING

            if summon_attack.num_summons_left > 0
                && summon_attack.timer.tick(time.delta()).just_finished()
            {
                let mut rng = rand::thread_rng();
                let my_txfm = txfm.translation().truncate();
                let summon_range = 110.;
                let offset_x = rng.gen_range(-summon_range..summon_range);
                let offset_y = rng.gen_range(-summon_range..summon_range);
                let mut pos = Vec2::new(my_txfm.x + offset_x, my_txfm.y + offset_y);
                while !can_spawn_mob_here(pos, &game, &proto, false) {
                    pos = Vec2::new(
                        my_txfm.x + rng.gen_range(-summon_range..summon_range),
                        my_txfm.y + rng.gen_range(-summon_range..summon_range),
                    );
                }

                if let Some(mob) =
                    proto_commands.spawn_from_proto(Mob::RedMushling, &proto.prototypes, pos)
                {
                    proto_commands
                        .commands()
                        .entity(mob)
                        .remove::<LootTable>()
                        .remove::<ExperienceReward>();
                }

                summon_attack.num_summons_left -= 1;
            }
        }
        if summon_attack.num_summons_left == 0 && frame == 39 {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::END_SUMMON);
        }
        // END LEAP ATTACK
        if frame == 42 {
            commands
                .entity(entity)
                .insert(FollowState {
                    target: game.game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .insert(EnemyAttackCooldown(Timer::from_seconds(
                    2.,
                    TimerMode::Once,
                )))
                .remove::<SummonAttackState>();
            *anim_state = AsepriteAnimation::from(RedMushking::tags::WALK);
        }
    }
}
pub fn handle_death(
    mut commands: Commands,
    mut death: Query<(Entity, &mut AsepriteAnimation, &super::Mob), With<DeathState>>,
    era_manager: Res<EraManager>,
    mut infinite_mode_event: EventWriter<InfiniteModeStartedEvent>,
) {
    for (entity, mut anim, mob) in death.iter_mut() {
        // Only handle RedMushking death animations
        if mob != &super::Mob::RedMushking {
            continue;
        }

        if anim.current_frame() < 43 {
            *anim = AsepriteAnimation::from(RedMushking::tags::DEATH_START);
        }
        if anim.current_frame() == 48 {
            *anim = AsepriteAnimation::from(RedMushking::tags::DEATH_LOOP);
        }
        if anim.current_frame() == 56 {
            *anim = AsepriteAnimation::from(RedMushking::tags::DEATH_END);
        }
        if anim.current_frame() == 62 {
            // Check if we're in Era 3 - trigger infinite mode when the main boss dies
            if era_manager.current_era == Era::Third {
                info!("Red Mushking defeated in Era 3! Starting INFINITE MODE!");
                infinite_mode_event.send_default();
            }
            commands.entity(entity).despawn_recursive();
        }
    }
}

pub fn new_follow(
    mut transforms: Query<&mut Transform>,
    mut follows: Query<(
        Entity,
        &FollowState,
        Option<&EnemyAttackCooldown>,
        &mut AsepriteAnimation,
        &mut KinematicCharacterController,
        Option<&Mob>,
    )>,
    added: Query<Entity, Added<FollowState>>,
    time: Res<Time>,
) {
    for (entity, follow, att_cooldown, mut anim, mut mover, mob_option) in follows.iter_mut() {
        if att_cooldown.is_some() && att_cooldown.unwrap().0.percent() <= 0.5 {
            return;
        }
        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;
        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_translation = follow_transform.translation;
        let delta = (target_translation - follow_translation)
            .normalize_or_zero()
            .truncate();
        // Find the direction from the follower to the target and go that way
        mover.translation = Some(delta * follow.speed * PLAYER_MOVE_SPEED * time.delta_seconds());

        if added.get(entity).is_ok() {
            if let Some(Mob::RedMushking) = mob_option {
                *anim = AsepriteAnimation::from(RedMushking::tags::WALK);
            }
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct JumpTimer;

impl BoolTrigger for JumpTimer {
    type Param<'w, 's> = Query<'w, 's, &'static EnemyAttackCooldown>;

    fn trigger(&self, entity: Entity, attack_cooldown: Self::Param<'_, '_>) -> bool {
        if attack_cooldown.get(entity).is_ok() {
            return false;
        }
        true
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct AoEAttackTimerTrigger;

impl BoolTrigger for AoEAttackTimerTrigger {
    type Param<'w, 's> = Query<
        'w,
        's,
        (
            &'static AoEAttackTimer,
            Option<&'static EnemyAttackCooldown>,
        ),
    >;

    fn trigger(&self, entity: Entity, query: Self::Param<'_, '_>) -> bool {
        match query.get(entity) {
            Ok((aoe_timer, attack_cooldown)) => {
                let finished = aoe_timer.random_timer.finished();
                // Only trigger if not on cooldown and random timer is finished
                if attack_cooldown.is_some() {
                    let cooldown_percent = attack_cooldown.unwrap().0.percent();
                    return false;
                }

                return finished;
            }
            Err(_) => {
                // This shouldn't happen often, but log it if it does
                false
            }
        }
    }
}
#[derive(Component)]
pub struct HealthThreshold(pub f32);

#[derive(Clone, Copy, Reflect)]
pub struct HealthTrigger(f32);

impl BoolTrigger for HealthTrigger {
    type Param<'w, 's> = Query<
        'w,
        's,
        (
            &'static HealthThreshold,
            &'static CurrentHealth,
            &'static MaxHealth,
        ),
    >;

    fn trigger(&self, entity: Entity, query: Self::Param<'_, '_>) -> bool {
        let (threshold, hp, max_hp) = query.get(entity).unwrap();
        if self.0 >= hp.0 as f32 / max_hp.0 as f32 && threshold.0 > self.0 {
            return true;
        }
        false
    }
}
//TODO: this may be frail and miss some summons. maybe we add this inside the actual summon fn
pub fn handle_boss_health_threshold(
    mut thresholds: Query<
        (&CurrentHealth, &MaxHealth, &mut HealthThreshold),
        Changed<CurrentHealth>,
    >,
) {
    for (hp, max_hp, mut threshold) in thresholds.iter_mut() {
        threshold.0 = hp.0 as f32 / max_hp.0 as f32;
    }
}

pub fn return_to_shrine(
    mut state_machines: Query<
        (
            Entity,
            &mut AsepriteAnimation,
            &mut KinematicCharacterController,
            &mut CurrentHealth,
            &MaxHealth,
        ),
        (With<ReturnToShrineState>, Without<crate::player::Player>),
    >,
    game: GameParam,
    mut transforms: Query<&mut Transform>,
    time: Res<Time>,
) {
    for (entity, mut anim, mut mover, mut hp, max_hp) in state_machines.iter_mut() {
        let shrine_pos = tile_pos_to_world_pos(
            *game
                .world_obj_cache
                .unique_objs
                .get(&WorldObject::BossShrine)
                .expect("no shrine found"),
            false,
        );

        // Get the positions of the follower and target
        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_translation = follow_transform.translation;
        let delta = shrine_pos.extend(0.) - follow_translation;
        let normal_delta = (delta).normalize_or_zero().truncate();
        debug!("d {normal_delta:?}");
        // Find the direction from the follower to the target and go that way
        mover.translation = Some(normal_delta * 2. * PLAYER_MOVE_SPEED * time.delta_seconds());

        if anim.current_frame() < 6 || anim.current_frame() > 13 {
            *anim = AsepriteAnimation::from(RedMushking::tags::WALK);
        }

        if delta.length() < 16. {
            hp.0 = max_hp.0;
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct ShrineLOS {
    pub range: f32,
    pub shrine_pos: Vec2,
}

impl Trigger for ShrineLOS {
    type Param<'w, 's> = Query<'w, 's, &'static Transform>;
    type Ok = f32;
    type Err = f32;

    // Return `Ok` to trigger and `Err` to not trigger
    fn trigger(&self, entity: Entity, transforms: Self::Param<'_, '_>) -> Result<f32, f32> {
        if let Ok(tfxm) = transforms.get(entity) {
            let delta = tfxm.translation.truncate() - self.shrine_pos;

            let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();
            (distance <= self.range).then_some(distance).ok_or(distance)
        } else {
            Err(0.)
        }
    }
}

/// System to tick the AoE attack random timer and initialize the attack
pub fn tick_aoe_attack_timer(
    mut aoe_timers: Query<(Entity, &mut AoEAttackTimer), With<Mob>>,
    boss_health: Query<(&CurrentHealth, &MaxHealth), With<Mob>>,
    time: Res<Time>,
) {
    for (entity, mut aoe_timer) in aoe_timers.iter_mut() {
        // If random timer hasn't been set yet, set it to a random duration
        if aoe_timer.random_timer.duration().as_secs_f32() == 0.0 {
            let mut rng = rand::thread_rng();

            // Check if boss is below half health - if so, use half the timer range (twice as fast)
            let is_below_half_health = boss_health
                .get(entity)
                .map(|(current, max)| (current.0 as f32 / max.0 as f32) < 0.5)
                .unwrap_or(false);

            let random_duration = if is_below_half_health {
                // Half the normal range: 0.15..1.25 (twice as fast)
                rng.gen_range(0.15..1.25)
            } else {
                // Normal range: 0.3..2.5
                rng.gen_range(0.3..2.5)
            };

            aoe_timer.random_timer = Timer::from_seconds(random_duration, TimerMode::Once);
        }

        // Tick the random timer
        let was_finished = aoe_timer.random_timer.finished();
        aoe_timer.random_timer.tick(time.delta());
    }
}

/// System to handle the AoE attack: show preview and spawn damage
pub fn handle_aoe_attack(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut aoe_attacks: Query<(Entity, &mut AoEAttackState, &Attack, &GlobalTransform), With<Mob>>,
    mut aoe_timers: Query<&mut AoEAttackTimer>,
    boss_health: Query<(&CurrentHealth, &MaxHealth), With<Mob>>,
    player_query: Query<&Transform, With<Player>>,
    time: Res<Time>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
    game: GameParam,
) {
    for (boss_entity, mut aoe_state, attack, _boss_txfm) in aoe_attacks.iter_mut() {
        // If target position not set yet, capture player position when attack starts
        if aoe_state.target_position.is_none() {
            if let Ok(player_txfm) = player_query.get_single() {
                let target_pos = player_txfm.translation.truncate();
                aoe_state.target_position = Some(target_pos);

                // Check if boss is below half health and calculate second cloud position
                let is_below_half_health = boss_health
                    .get(boss_entity)
                    .map(|(current, max)| (current.0 as f32 / max.0 as f32) < 0.5)
                    .unwrap_or(false);

                if is_below_half_health {
                    // Calculate second cloud position with minimum distance (64px) from player
                    let mut rng = rand::thread_rng();
                    let min_distance = 64.0;
                    let max_distance = 96.0;
                    let distance = rng.gen_range(min_distance..max_distance);
                    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                    let offset = Vec2::new(angle.cos(), angle.sin()) * distance;
                    let second_pos = target_pos + offset + Vec2::new(0., 16.);
                    aoe_state.second_cloud_position = Some(second_pos);
                }
            }
        }

        // Tick the delay timer
        aoe_state.delay_timer.tick(time.delta());

        // Show warning preview during delay at the captured position
        if let Some(target_pos) = aoe_state.target_position {
            // Spawn first preview if it doesn't exist
            if aoe_state.preview_entity.is_none() {
                // Use MaterialMesh2dBundle with Circle shape for a circular preview
                let preview_entity = commands
                    .spawn((
                        MaterialMesh2dBundle {
                            mesh: meshes
                                .add(
                                    shape::Circle {
                                        radius: 16.0, // 32px diameter
                                        ..Default::default()
                                    }
                                    .into(),
                                )
                                .into(),
                            material: materials
                                .add(ColorMaterial::from(Color::rgba(1.0, 0.0, 0.0, 0.3))), // Red with low alpha
                            transform: Transform {
                                translation: target_pos.extend(990.0), // High Z to be visible
                                ..default()
                            },
                            ..default()
                        },
                        BossAttackPreview,
                    ))
                    .id();
                aoe_state.preview_entity = Some(preview_entity);
            }

            // Spawn second preview if below half health and it doesn't exist
            if let Some(second_pos) = aoe_state.second_cloud_position {
                if aoe_state.second_preview_entity.is_none() {
                    let second_preview_entity = commands
                        .spawn((
                            MaterialMesh2dBundle {
                                mesh: meshes
                                    .add(
                                        shape::Circle {
                                            radius: 16.0, // 32px diameter
                                            ..Default::default()
                                        }
                                        .into(),
                                    )
                                    .into(),
                                material: materials
                                    .add(ColorMaterial::from(Color::rgba(1.0, 0.0, 0.0, 0.3))), // Red with low alpha
                                transform: Transform {
                                    translation: second_pos.extend(990.0), // High Z to be visible
                                    ..default()
                                },
                                ..default()
                            },
                            BossAttackPreview,
                        ))
                        .id();
                    aoe_state.second_preview_entity = Some(second_preview_entity);
                }
            }
        }

        // When delay timer finishes, spawn the actual damage at the captured position
        if aoe_state.delay_timer.just_finished() {
            if let Some(target_pos) = aoe_state.target_position {
                // Always spawn first cloud on player position
                ranged_attack_events.send(RangedAttackEvent {
                    projectile: Projectile::PoisonCloud,
                    direction: Vec2::ZERO,
                    mana_cost: None,
                    from_enemy: true,
                    from_entity: Some(boss_entity),
                    is_followup_proj: false,
                    dmg_override: Some(attack.0),
                    pos_override: Some(target_pos + Vec2::new(0., 16.)), // Centered on target position
                    spawn_delay: 0.0,
                });

                // If below half health, spawn a second cloud at the pre-calculated position
                if let Some(second_pos) = aoe_state.second_cloud_position {
                    ranged_attack_events.send(RangedAttackEvent {
                        projectile: Projectile::PoisonCloud,
                        direction: Vec2::ZERO,
                        mana_cost: None,
                        from_enemy: true,
                        from_entity: Some(boss_entity),
                        is_followup_proj: false,
                        dmg_override: Some(attack.0),
                        pos_override: Some(second_pos),
                        spawn_delay: 0.0,
                    });
                }

                // Despawn previews
                if let Some(preview_e) = aoe_state.preview_entity {
                    commands.entity(preview_e).despawn_recursive();
                }
                if let Some(second_preview_e) = aoe_state.second_preview_entity {
                    commands.entity(second_preview_e).despawn_recursive();
                }

                // Return to FollowState and reset random timer for next attack
                let mut rng = rand::thread_rng();

                // Check if boss is below half health - if so, use half the timer range (twice as fast)
                let is_below_half_health = boss_health
                    .get(boss_entity)
                    .map(|(current, max)| (current.0 as f32 / max.0 as f32) < 0.5)
                    .unwrap_or(false);

                let random_duration = if is_below_half_health {
                    // Half the normal range: 0.15..1.25 (twice as fast)
                    rng.gen_range(0.15..1.25)
                } else {
                    // Normal range: 0.3..2.5
                    rng.gen_range(0.3..2.5)
                };

                commands
                    .entity(boss_entity)
                    .remove::<AoEAttackState>() // Remove current state first
                    .insert(FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: 1.0, // Default speed, will be overridden by FollowSpeed
                    })
                    .insert(EnemyAttackCooldown(Timer::from_seconds(
                        2.0,
                        TimerMode::Once,
                    )));

                // Reset the timer component for next attack
                if let Ok(mut aoe_timer) = aoe_timers.get_mut(boss_entity) {
                    aoe_timer.random_timer = Timer::from_seconds(random_duration, TimerMode::Once);
                }
            }
        }
    }
}
