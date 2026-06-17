use crate::{
    combat::{
        combat_helpers::DespawnTimer,
        pickup_radius::{pull_all_eligible_ground_items_to_player, BeingPulledToPlayer},
    },
    custom_commands::CommandsExt,
    enemy::{spawn_helpers::can_spawn_mob_here, spawner::MobSpawningPaused},
    inventory::{Inventory, ItemStack},
    item::{boss_shrine::BossSummonIndex, ItemDrop, LootTable, WorldObject},
    juice::ShakeEffect,
    night::{EraTimer, InfiniteModeStartedEvent},
    pets::state::Pet,
    player::levels::ExperienceReward,
    world::{
        dimension::{Era, EraManager},
        portal::BossKillTracker,
        world_helpers::tile_pos_to_world_pos,
    },
    GameParam, TextureCamera,
};
use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::{
    control::KinematicCharacterController,
    geometry::{Collider, Sensor},
    prelude::{CollisionGroups, Group},
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
    ecs_helpers::SafeHierarchyExt,
    item::projectile::{Projectile, RangedAttackEvent},
    player::Player,
    proto::proto_param::ProtoParam,
    ui::{boss_warning_indicator_color, CheatSettings},
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

const MAX_JUMP_DISTANCE: f32 = 16. * 8.0; // 8 tiles = 128 pixels
const AOE_SPAWN_RADIUS_MIN: f32 = 12.5; // 10.0 + 25%
const AOE_SPAWN_RADIUS_MAX: f32 = 87.5; // 70.0 + 25%

/// Fixed delay between boss attacks. The boss cycles jump -> summon -> aoe,
/// pausing this long between each attack.
const ATTACK_ROTATION_INTERVAL: f32 = 1.2;
/// Base summon duration at boss tier 0; grows by 1s per shrine summon tier.
const SUMMON_BASE_DURATION: f32 = 1.5;
/// How often a mushling is spawned while the summon is active.
const SUMMON_SPAWN_INTERVAL: f32 = 0.12;

fn summon_attack_state(boss_summon_index: &BossSummonIndex) -> SummonAttackState {
    let duration = SUMMON_BASE_DURATION + boss_summon_index.0 as f32;
    SummonAttackState {
        duration_timer: Timer::from_seconds(duration, TimerMode::Once),
        spawn_timer: Timer::from_seconds(SUMMON_SPAWN_INTERVAL, TimerMode::Repeating),
        anim_phase: 0,
    }
}

fn aoe_attack_state(boss_summon_index: &BossSummonIndex) -> AoEAttackState {
    let summon_scale = boss_summon_index.aoe_radius_scale();
    AoEAttackState {
        delay_timer: Timer::from_seconds(0.85, TimerMode::Once),
        num_bombs: boss_summon_index.num_poison_bombs() * 2,
        spawn_radius_min: AOE_SPAWN_RADIUS_MIN * summon_scale,
        spawn_radius_max: AOE_SPAWN_RADIUS_MAX * summon_scale,
        cloud_positions: Vec::new(),
        preview_entities: Vec::new(),
    }
}

pub fn handle_new_red_mushking_state_machine(
    mut commands: Commands,
    mut spawn_events: Query<
        (
            Entity,
            &Mob,
            &Transform,
            &FollowSpeed,
            &LeapAttack,
            &mut KinematicCharacterController,
            &BossSummonIndex,
        ),
        Added<Mob>,
    >,
    asset_server: Res<AssetServer>,
    game: GameParam,
) {
    for (e, mob, transform, follow_speed, leap_attack, mut mover, boss_summon_index) in
        spawn_events.iter_mut()
    {
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
        mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));

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
            .insert(AttackRotation {
                timer: Timer::from_seconds(ATTACK_ROTATION_INTERVAL, TimerMode::Once),
                index: 0,
            });

        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .with_state::<DeathState>()
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
                    attack_preview_entity: None,
                },
            )
            .trans::<FollowState>(SummonTrigger, summon_attack_state(boss_summon_index))
            .trans::<FollowState>(AoEAttackTimerTrigger, aoe_attack_state(boss_summon_index));
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
    /// Total time the boss spends summoning; mushlings spawn for this whole window.
    /// This effect timer is fully decoupled from the summon animation.
    duration_timer: Timer,
    /// Rate at which mushlings spawn during the summon.
    spawn_timer: Timer,
    /// Tracks the single play-through of the summon animation (no looping):
    /// 0 = not started, 1 = START_SUMMON, 2 = SUMMONING, 3 = END_SUMMON, 4 = done.
    anim_phase: u8,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct DeathState;

/// Drives the boss's attack cadence: it cycles jump -> summon -> aoe, waiting
/// `ATTACK_ROTATION_INTERVAL` between attacks. `index` selects the next attack.
#[derive(Component)]
pub struct AttackRotation {
    pub timer: Timer,
    pub index: usize,
}

impl AttackRotation {
    /// Advance to the next attack in the cycle and restart the inter-attack delay.
    fn advance(&mut self) {
        self.index = (self.index + 1) % 3;
        self.timer = Timer::from_seconds(ATTACK_ROTATION_INTERVAL, TimerMode::Once);
    }
}

/// State that is active during the AoE attack.
/// Bomb count is doubled from the shrine summon index; doubled again when below half health.
#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct AoEAttackState {
    pub delay_timer: Timer,
    pub num_bombs: usize,
    pub spawn_radius_min: f32,
    pub spawn_radius_max: f32,
    pub cloud_positions: Vec<Vec2>,
    pub preview_entities: Vec<Entity>,
}

/// Warning/preview marker on boss-attack telegraph entities; removed once the
/// real attack resolves. `SparseSet` so telegraph entities don't occupy an
/// extra archetype variant every attack cycle.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct BossAttackPreview;

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
    mut rotations: Query<&mut AttackRotation>,
    time: Res<Time>,
    mut game_camera: Query<Entity, With<TextureCamera>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cheat_settings: Res<CheatSettings>,
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
        // Debug: Log timer states
        let startup_elapsed = leap_attack.attack_startup_timer.elapsed_secs();
        let startup_duration = leap_attack.attack_startup_timer.duration().as_secs_f32();
        let startup_finished = leap_attack.attack_startup_timer.finished();
        let duration_finished = leap_attack.attack_duration_timer.finished();
        const SLAM_TIME: f32 = 0.55;
        let has_slammed = leap_attack.attack_duration_timer.percent() >= SLAM_TIME;

        // PHASE 1: Startup (wind-up animation)
        // Tick startup timer and wait for it to finish
        if !startup_finished {
            leap_attack.attack_startup_timer.tick(time.delta());

            // Set animation if not already set
            let frame = anim_state.current_frame();
            if !(14..=28).contains(&frame) {
                *anim_state = AsepriteAnimation::from(RedMushking::tags::ATTACK_HOP);
                anim_state.play();
            }
            continue;
        }

        // Capture target position at the START of the leap (only once)
        if leap_attack.dir.is_none() {
            let target_translation =
                transforms.get(leap_attack.target).unwrap().translation + Vec3::new(0., 18., 0.);
            let attack_transform = transforms.get(entity).unwrap();
            let attack_translation = attack_transform.translation;

            let delta = target_translation - attack_translation;
            let distance = delta.length();
            let max_leap_distance = MAX_JUMP_DISTANCE; // 8 tiles = 128 pixels

            // Clamp to max distance if needed
            let clamped_delta = if distance > max_leap_distance {
                delta.normalize() * max_leap_distance
            } else {
                delta
            };

            leap_attack.dir = Some(clamped_delta.truncate());
            info!(
                "Leap Attack PHASE 1 START: direction {:?}, distance {:.1}, startup: {:.3}/{:.3}s",
                leap_attack.dir, distance, startup_elapsed, startup_duration
            );
            let preview_entity = commands
                .spawn((
                    MaterialMesh2dBundle {
                        mesh: meshes
                            .add(
                                shape::Circle {
                                    radius: 40.0, // 40px diameter
                                    ..Default::default()
                                }
                                .into(),
                            )
                            .into(),
                        material: materials.add(ColorMaterial::from(boss_warning_indicator_color(
                            &cheat_settings,
                        ))),
                        transform: Transform {
                            translation: target_translation + Vec3::new(0., -15., 100.),
                            ..default()
                        },
                        ..default()
                    },
                    BossAttackPreview,
                    DespawnTimer(Timer::from_seconds(4.0, TimerMode::Once)),
                ))
                .id();
            leap_attack.attack_preview_entity = Some(preview_entity);
        }

        // We've exited phase 1 - log this transition

        // PHASE 2: Active leap (moving and dealing damage)
        // Only tick duration timer AFTER startup is complete
        if !duration_finished {
            leap_attack.attack_duration_timer.tick(time.delta());

            // Spawn damage collider at the start of the active phase (only once)
            if att_collider.0.is_none() && has_slammed {
                commands
                    .entity(entity)
                    .insert(MobIsAttacking(Mob::RedMushking));

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
                    .safe_set_parent(entity)
                    .id();
                att_collider.0 = Some(hitbox);

                // Screen shake
                let mut rng = rand::thread_rng();
                let seed = rng.gen_range(0..100000);
                for e in game_camera.iter_mut() {
                    commands.entity(e).insert(ShakeEffect {
                        timer: Timer::from_seconds(0.4, TimerMode::Once),
                        speed: 10.,
                        seed,
                        max_mag: 80.,
                        noise: 0.5,
                        dir: Vec2::new(1., 1.),
                    });
                }
                if let Some(entity) = leap_attack.attack_preview_entity {
                    if let Some(entity_commands) = commands.get_entity(entity) {
                        entity_commands.despawn_recursive();
                    }
                }
            }

            // Move towards target position
            if let Some(dir) = leap_attack.dir {
                if !has_slammed {
                    // Before slam - use normal speed
                    let leap_duration =
                        leap_attack.attack_duration_timer.duration().as_secs_f32() * SLAM_TIME;
                    let leap_speed = dir.length() / leap_duration;
                    let normalized_dir = dir.normalize_or_zero();

                    kcc.translation = Some(normalized_dir * leap_speed * time.delta_seconds());
                }
            }
            continue;
        }

        // PHASE 3: End of leap - cleanup and transition back

        // Advance the attack rotation for the next attack
        if let Ok(mut rotation) = rotations.get_mut(entity) {
            rotation.advance();
        }

        // Reset animation to WALK
        *anim_state = AsepriteAnimation::from(RedMushking::tags::WALK);
        anim_state.play();

        // Despawn hitbox
        if let Some(hitbox) = att_collider.0.take() {
            commands.entity(hitbox).despawn_recursive();
        }

        // Transition back to FollowState
        commands
            .entity(entity)
            .insert(FollowState {
                target: leap_attack.target,
                curr_delta: None,
                curr_path: None,
                speed: follow_speed.0,
            })
            .insert(EnemyAttackCooldown(Timer::from_seconds(
                ATTACK_ROTATION_INTERVAL,
                TimerMode::Once,
            )))
            .remove::<LeapAttackState>()
            .remove::<MobIsAttacking>();
    }
}
pub fn summon_attack(
    mut attacks: Query<(
        Entity,
        &mut SummonAttackState,
        &mut AsepriteAnimation,
        &GlobalTransform,
        &FollowSpeed,
        &mut AttackRotation,
    )>,
    mut commands: Commands,
    time: Res<Time>,
    mut proto_commands: ProtoCommands,
    proto: ProtoParam,
    game: GameParam,
) {
    for (entity, mut summon_attack, mut anim_state, txfm, follow_speed, mut rotation) in
        attacks.iter_mut()
    {
        if anim_state.is_paused() {
            anim_state.play();
        }

        // --- Effect: spawn mushlings for the full duration, decoupled from the animation. ---
        summon_attack.duration_timer.tick(time.delta());
        if !summon_attack.duration_timer.finished()
            && summon_attack.spawn_timer.tick(time.delta()).just_finished()
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
        }

        // --- Animation: a single play-through (no loop), chained on just_finished. ---
        match summon_attack.anim_phase {
            0 => {
                *anim_state = AsepriteAnimation::from(RedMushking::tags::START_SUMMON);
                summon_attack.anim_phase = 1;
            }
            1 if anim_state.just_finished() => {
                *anim_state = AsepriteAnimation::from(RedMushking::tags::SUMMONING);
                summon_attack.anim_phase = 2;
            }
            2 if anim_state.just_finished() => {
                *anim_state = AsepriteAnimation::from(RedMushking::tags::END_SUMMON);
                summon_attack.anim_phase = 3;
            }
            3 if anim_state.just_finished() => {
                *anim_state = AsepriteAnimation::from(RedMushking::tags::WALK);
                summon_attack.anim_phase = 4;
            }
            _ => {}
        }

        // --- The effect timer (not the animation) drives the end of the summon. ---
        if summon_attack.duration_timer.finished() {
            rotation.advance();
            commands
                .entity(entity)
                .insert(FollowState {
                    target: game.game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                })
                .insert(EnemyAttackCooldown(Timer::from_seconds(
                    ATTACK_ROTATION_INTERVAL,
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
    mut boss_kill_tracker: ResMut<BossKillTracker>,
    era_timer: Res<EraTimer>,
    mut mob_spawning_paused: ResMut<MobSpawningPaused>,
    item_drop_query: Query<(Entity, &ItemStack), (With<ItemDrop>, Without<BeingPulledToPlayer>)>,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
    proto: ProtoParam,
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
            } else {
                // Mark the current era's boss as killed
                boss_kill_tracker.mark_boss_killed(era_manager.current_era.clone());
                info!("Boss killed in era {:?}", era_manager.current_era);

                if (era_manager.current_era == Era::Main || era_manager.current_era == Era::Second)
                    && era_timer.remaining_seconds > 0.0
                {
                    mob_spawning_paused.paused = true;
                }
            }

            pull_all_eligible_ground_items_to_player(
                &mut commands,
                &item_drop_query,
                &inv,
                &pets,
                &proto,
            );

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
            continue;
        }
        // Get the positions of the follower and target
        let follow_transform = &mut transforms.get_mut(entity).unwrap();
        let follow_translation = follow_transform.translation;
        let Ok(target_transform) = transforms.get(follow.target) else {
            continue;
        };
        let delta = (target_transform.translation - follow_translation)
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
    type Param<'w, 's> = (
        Query<
            'w,
            's,
            (
                &'static AttackRotation,
                Option<&'static EnemyAttackCooldown>,
            ),
        >,
        Query<'w, 's, &'static Transform>,
        Query<'w, 's, (Entity, &'static crate::player::Player)>,
    );

    fn trigger(
        &self,
        entity: Entity,
        (rotation_query, transforms, player_query): Self::Param<'_, '_>,
    ) -> bool {
        match rotation_query.get(entity) {
            Ok((rotation, attack_cooldown)) => {
                // Jump is index 0 of the rotation
                if rotation.index != 0 {
                    return false;
                }
                if attack_cooldown.is_some() {
                    return false;
                }
                if !rotation.timer.finished() {
                    return false;
                }

                // Check if player is within 10 tiles (160 pixels)
                if let Ok(boss_txfm) = transforms.get(entity) {
                    if let Ok((player_entity, _)) = player_query.get_single() {
                        if let Ok(player_txfm) = transforms.get(player_entity) {
                            let distance = (boss_txfm.translation.truncate()
                                - player_txfm.translation.truncate())
                            .length();
                            let max_leap_distance = 10.0 * 16.0;

                            if distance <= max_leap_distance {
                                return true;
                            }
                        }
                    }
                }

                false
            }
            Err(_) => false,
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct SummonTrigger;

impl BoolTrigger for SummonTrigger {
    type Param<'w, 's> = Query<
        'w,
        's,
        (
            &'static AttackRotation,
            Option<&'static EnemyAttackCooldown>,
        ),
    >;

    fn trigger(&self, entity: Entity, query: Self::Param<'_, '_>) -> bool {
        match query.get(entity) {
            Ok((rotation, attack_cooldown)) => {
                // Summon is index 1 of the rotation
                rotation.index == 1 && attack_cooldown.is_none() && rotation.timer.finished()
            }
            Err(_) => false,
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct AoEAttackTimerTrigger;

impl BoolTrigger for AoEAttackTimerTrigger {
    type Param<'w, 's> = Query<
        'w,
        's,
        (
            &'static AttackRotation,
            Option<&'static EnemyAttackCooldown>,
        ),
    >;

    fn trigger(&self, entity: Entity, query: Self::Param<'_, '_>) -> bool {
        match query.get(entity) {
            Ok((rotation, attack_cooldown)) => {
                // AoE is index 2 of the rotation
                rotation.index == 2 && attack_cooldown.is_none() && rotation.timer.finished()
            }
            Err(_) => false,
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

/// System to tick the boss attack rotation timer (the inter-attack delay).
pub fn tick_attack_rotation(mut rotations: Query<&mut AttackRotation, With<Mob>>, time: Res<Time>) {
    for mut rotation in rotations.iter_mut() {
        rotation.timer.tick(time.delta());
    }
}

/// System to handle the AoE attack: show preview and spawn damage
pub fn handle_aoe_attack(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut aoe_attacks: Query<(Entity, &mut AoEAttackState, &Attack, &GlobalTransform), With<Mob>>,
    mut rotations: Query<&mut AttackRotation>,
    boss_health: Query<(&CurrentHealth, &MaxHealth), With<Mob>>,
    player_query: Query<&Transform, With<Player>>,
    time: Res<Time>,
    mut ranged_attack_events: EventWriter<RangedAttackEvent>,
    game: GameParam,
    cheat_settings: Res<CheatSettings>,
) {
    for (boss_entity, mut aoe_state, attack, _boss_txfm) in aoe_attacks.iter_mut() {
        // First frame: generate all cloud positions (doubled when below half health)
        if aoe_state.cloud_positions.is_empty() {
            let Ok(player_txfm) = player_query.get_single() else {
                continue;
            };
            let target_pos = player_txfm.translation.truncate();

            let is_below_half_health = boss_health
                .get(boss_entity)
                .map(|(current, max)| (current.0 as f32 / max.0 as f32) < 0.5)
                .unwrap_or(false);

            let total_bombs = if is_below_half_health {
                aoe_state.num_bombs * 2
            } else {
                aoe_state.num_bombs
            };

            let mut rng = rand::thread_rng();
            for _ in 0..total_bombs {
                let distance =
                    rng.gen_range(aoe_state.spawn_radius_min..aoe_state.spawn_radius_max);
                let angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let offset = Vec2::new(angle.cos(), angle.sin()) * distance;
                aoe_state
                    .cloud_positions
                    .push(target_pos + offset + Vec2::new(0., 16.));
            }
        }

        aoe_state.delay_timer.tick(time.delta());

        // Spawn preview circles for any positions that don't have one yet
        while aoe_state.preview_entities.len() < aoe_state.cloud_positions.len() {
            let idx = aoe_state.preview_entities.len();
            let pos = aoe_state.cloud_positions[idx];
            let preview = commands
                .spawn((
                    MaterialMesh2dBundle {
                        mesh: meshes
                            .add(
                                shape::Circle {
                                    radius: 16.0,
                                    ..Default::default()
                                }
                                .into(),
                            )
                            .into(),
                        material: materials.add(ColorMaterial::from(boss_warning_indicator_color(
                            &cheat_settings,
                        ))),
                        transform: Transform {
                            translation: pos.extend(990.0),
                            ..default()
                        },
                        ..default()
                    },
                    BossAttackPreview,
                    DespawnTimer(Timer::from_seconds(2.0, TimerMode::Once)),
                ))
                .id();
            aoe_state.preview_entities.push(preview);
        }

        if aoe_state.delay_timer.just_finished() {
            // Spawn a poison cloud at every position
            for pos in &aoe_state.cloud_positions {
                ranged_attack_events.send(RangedAttackEvent {
                    projectile: Projectile::PoisonCloud,
                    direction: Vec2::ZERO,
                    mana_cost: None,
                    mana_cost_heirloom: None,
                    from_enemy: true,
                    from_entity: Some(boss_entity),
                    is_followup_proj: false,
                    dmg_override: Some(attack.0),
                    pos_override: Some(*pos),
                    spawn_delay: 0.0,
                });
            }

            for preview_e in &aoe_state.preview_entities {
                if let Some(entity_commands) = commands.get_entity(*preview_e) {
                    entity_commands.despawn_recursive();
                }
            }

            if let Ok(mut rotation) = rotations.get_mut(boss_entity) {
                rotation.advance();
            }

            if let Some(mut entity_commands) = commands.get_entity(boss_entity) {
                entity_commands
                    .remove::<AoEAttackState>()
                    .insert(FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: 1.0,
                    })
                    .insert(EnemyAttackCooldown(Timer::from_seconds(
                        ATTACK_ROTATION_INTERVAL,
                        TimerMode::Once,
                    )));
            }
        }
    }
}
