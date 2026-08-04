use crate::aseprite_assets::{StoneGolem, StonePillar};
use crate::aseprite_helpers::{
    ase_animation, aseprite_bundle, collect_finished, is_paused, pause, play_loop, play_once, start,
};
use crate::{
    assets::Graphics,
    collisions::DamagesWorldObjects,
    combat::combat_helpers::{spawn_deferred_aseprite_collider, DeferredComponent},
    enemy::{FollowSpeed, Mob},
    item::projectile::Projectile,
    player::Player,
    status_effects::MobStatusEffects,
    ui::{boss_warning_indicator_color, CheatSettings},
    world::y_sort::YSort,
    GameParam, PLAYER_MOVE_SPEED,
};
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy_aseprite_ultra::prelude::{AnimationState, AseAnimation, Aseprite};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group, KinematicCharacterController};
use rand::Rng;
use seldom_state::prelude::StateMachine;

use super::{
    red_mushking::{BossAttackPreview, DeathState},
    FollowState,
};

// Animation tags are generated at compile time from StoneGolem.ase — see StoneGolem::tags.

// Constants
const SPIKE_ATTACK_COUNT_MAX: usize = 7;
const SPIKE_ATTACK_COUNT_MIN: usize = 4;
const SPIKE_ATTACK_INTERVAL_MAX: f32 = 1.;
const SPIKE_ATTACK_INTERVAL_MIN: f32 = 0.1;
const SPIKE_WARNING_DELAY: f32 = 0.65; // Time between warning and damage
/// How long after the wave attack starts before warning circles appear.
const WAVE_WARNING_START_DELAY: f32 = 0.35;
/// How long warning circles show before wave pillars emerge.
const WAVE_PILLAR_SPAWN_DELAY: f32 = 0.85;
/// Extra delay before each subsequent pillar in a row (col 0 unchanged, col 1 +this, col 2 +2×this).
const WAVE_PILLAR_COLUMN_STAGGER: f32 = 0.35;
/// How long the golem stays in wave attack (attack anim + stationary) before returning to follow.
/// Kept just under the full attack animation length (~2.37s).
const WAVE_ATTACK_DURATION: f32 = 1.97;
const SPIKE_DAMAGE: i32 = 20;
const SPIKE_DURATION: f32 = 10.0; // How long the spike hitbox lasts
const SPIKE_DISTANCE_OFFSET_MAX: f32 = 40.0; // Maximum random distance offset from player

const WAVE_ATTACK_DISTANCE: f32 = 120.0;
/// When within wave range, chance to use the wave attack vs the random spike attack.
const WAVE_ATTACK_CHANCE: f32 = 0.5;
const WAVE_ROW_COUNT: usize = 3;
const WAVE_PILLARS_PER_ROW: usize = 3;

/// Minimum delay between any two golem attacks (spike or wave).
const ATTACK_COOLDOWN: f32 = 1.3;
const WAVE_PILLAR_SPACING: f32 = 40.0;
const WAVE_ROW_ANGLE_OFFSET: f32 = 0.45; // ~26° between rows
/// Distance from golem to the first pillar in each row (center pillar was previously here).
const WAVE_FIRST_PILLAR_DISTANCE: f32 = 40.0;

const PILLAR_MAX_Z: f32 = 999.0;
const PILLAR_BASE_Z: f32 = 990.0;
const PILLAR_Z_STAGGER: f32 = 0.6; // 15 wave pillars max out at 998.4

#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect)]
enum GolemFacing {
    Front,
    Back,
    Side,
}

impl GolemFacing {
    fn from_delta(delta: Vec2) -> Self {
        let abs_x = delta.x.abs();
        let abs_y = delta.y.abs();
        if abs_x > abs_y * 1.1 {
            Self::Side
        } else if delta.y > 0. {
            Self::Back
        } else {
            Self::Front
        }
    }

    fn attack_tag(self) -> &'static str {
        match self {
            Self::Front => StoneGolem::tags::ATTACK_FRONT,
            Self::Back => StoneGolem::tags::ATTACK_BACK,
            Self::Side => StoneGolem::tags::ATTACK_SIDE,
        }
    }
}

fn apply_golem_sprite_flip(transform: &mut Transform, delta: Vec2) {
    let abs_x = delta.x.abs();
    let abs_y = delta.y.abs();
    if abs_x > abs_y * 1.1 && abs_x > f32::EPSILON {
        if delta.x < 0. && transform.scale.x > 0. {
            transform.scale.x = -1.0;
        } else if delta.x > 0. && transform.scale.x < 0. {
            transform.scale.x = 1.0;
        }
    }
}

fn compute_wave_pillar_positions(golem_pos: Vec2, player_pos: Vec2) -> Vec<Vec2> {
    let to_player = player_pos - golem_pos;
    let base_angle = to_player.y.atan2(to_player.x);
    let row_center = (WAVE_ROW_COUNT as f32 - 1.0) / 2.0;
    let half_span = (WAVE_PILLARS_PER_ROW as f32 - 1.0) / 2.0 * WAVE_PILLAR_SPACING;
    let mut positions = Vec::with_capacity(WAVE_ROW_COUNT * WAVE_PILLARS_PER_ROW);

    for row in 0..WAVE_ROW_COUNT {
        let row_angle = base_angle + (row as f32 - row_center) * WAVE_ROW_ANGLE_OFFSET;
        let row_dir = Vec2::from_angle(row_angle);
        // Offset row origin so column 0 sits at WAVE_FIRST_PILLAR_DISTANCE, not behind the golem.
        let row_origin = golem_pos + row_dir * (WAVE_FIRST_PILLAR_DISTANCE + half_span);

        for col in 0..WAVE_PILLARS_PER_ROW {
            let along = col as f32 * WAVE_PILLAR_SPACING - half_span;
            positions.push(row_origin + row_dir * along);
        }
    }

    positions
}

/// Tracks the active aseprite tag so walk animations aren't reset every frame.
#[derive(Component, Default)]
pub struct GolemCurrentTag(pub String);

fn set_golem_anim(anim: &mut AseAnimation, current: &mut GolemCurrentTag, tag: &str) {
    if current.0 != tag {
        play_loop(anim, tag);
        current.0 = tag.to_string();
    }
}

/// Always restarts the animation from the first frame, even if the tag is unchanged.
/// Used when entering an attack so it replays instead of staying frozen on the last frame
/// of a previous (identical) attack tag.
fn play_golem_anim_force(anim: &mut AseAnimation, current: &mut GolemCurrentTag, tag: &str) {
    play_once(anim, tag);
    current.0 = tag.to_string();
}

fn is_golem_walk_tag(tag: &str) -> bool {
    matches!(
        tag,
        StoneGolem::tags::WALK_FRONT | StoneGolem::tags::WALK_BACK | StoneGolem::tags::WALK_SIDE
    )
}

#[derive(Component)]
pub struct SpikeAttackTimer {
    pub random_timer: Timer,
    /// Enforces a minimum gap between attacks; reset when any attack starts.
    pub cooldown_timer: Timer,
    /// Rolled once per attack window: `true` = wave, `false` = spike when within wave range.
    pub attack_choice: Option<bool>,
}

/// Component for individual spike warnings that track their own timer
#[derive(Component)]
pub struct SpikeWarning {
    pub timer: Timer,
    pub target_pos: Vec2,
    pub golem_entity: Entity,
    /// Per-pillar Z stagger so wave pillars don't z-fight.
    pub z_offset: f32,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct SpikeAttackState {
    pub num_spikes_left: usize,
    pub attack_timer: Timer, // Controls interval between spikes
    pub initialized: bool, // Track if we've initialized the spike count (for randomization on state entry)
    pub spikes_spawned: usize,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct WaveAttackState {
    pub initialized: bool,
    pub facing: GolemFacing,
    pub finish_timer: Timer,
    pub warnings_spawned: bool,
    pub warning_spawn_timer: Timer,
    pub pillar_positions: Vec<Vec2>,
}

fn golem_spike_attack_trigger(
    In(entity): In<Entity>,
    golems: Query<(&SpikeAttackTimer, &Transform)>,
    player: Query<&Transform, With<Player>>,
) -> bool {
    let Ok((timer, golem_tf)) = golems.get(entity) else {
        return false;
    };
    if !timer.random_timer.is_finished() || !timer.cooldown_timer.is_finished() {
        return false;
    }
    let Ok(player_tf) = player.single() else {
        return false;
    };
    let dist = golem_tf
        .translation
        .truncate()
        .distance(player_tf.translation.truncate());
    if dist <= WAVE_ATTACK_DISTANCE {
        return timer.attack_choice == Some(false);
    }
    true
}

fn golem_wave_attack_trigger(
    In(entity): In<Entity>,
    golems: Query<(&SpikeAttackTimer, &Transform)>,
    player: Query<&Transform, With<Player>>,
) -> bool {
    let Ok((timer, golem_tf)) = golems.get(entity) else {
        return false;
    };
    if !timer.random_timer.is_finished() || !timer.cooldown_timer.is_finished() {
        return false;
    }
    let Ok(player_tf) = player.single() else {
        return false;
    };
    let dist = golem_tf
        .translation
        .truncate()
        .distance(player_tf.translation.truncate());
    dist <= WAVE_ATTACK_DISTANCE && timer.attack_choice == Some(true)
}

pub fn handle_new_stone_golem_state_machine(
    mut commands: Commands,
    spawn_events: Query<(Entity, &Mob, &Transform, &FollowSpeed), Added<Mob>>,
    asset_server: Res<AssetServer>,
    game: GameParam,
) {
    for (e, mob, transform, follow_speed) in spawn_events.iter() {
        if mob != &Mob::StoneGolem {
            continue;
        }
        let mut e_cmds = commands.entity(e);

        let mut rng = rand::thread_rng();
        let initial_timer = rng.gen_range(3.0..5.0);
        e_cmds
            .insert(aseprite_bundle(
                asset_server.load(StoneGolem::PATH),
                StoneGolem::tags::WALK_FRONT,
                *transform,
                Visibility::Inherited,
                false,
            ))
            .insert(YSort(0.001))
            .insert(GolemCurrentTag(StoneGolem::tags::WALK_FRONT.to_string()))
            .insert(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1))
            .insert(FollowState {
                target: game.game.player,
                curr_delta: None,
                curr_path: None,
                speed: follow_speed.0,
            })
            .insert(DamagesWorldObjects)
            .insert(SpikeAttackTimer {
                random_timer: Timer::from_seconds(initial_timer, TimerMode::Once),
                cooldown_timer: {
                    // Start finished so the first attack isn't gated by the cooldown.
                    let mut t = Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Once);
                    t.tick(std::time::Duration::from_secs_f32(ATTACK_COOLDOWN));
                    t
                },
                attack_choice: None,
            });

        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .with_state::<DeathState>() // Add DeathState so state machine is always valid
            .trans::<FollowState, _>(
                golem_wave_attack_trigger,
                WaveAttackState {
                    initialized: false,
                    facing: GolemFacing::Front,
                    finish_timer: Timer::from_seconds(WAVE_ATTACK_DURATION, TimerMode::Once),
                    warnings_spawned: false,
                    warning_spawn_timer: Timer::from_seconds(
                        WAVE_WARNING_START_DELAY,
                        TimerMode::Once,
                    ),
                    pillar_positions: Vec::new(),
                },
            )
            .trans::<FollowState, _>(
                golem_spike_attack_trigger,
                SpikeAttackState {
                    num_spikes_left: 0,
                    attack_timer: Timer::from_seconds(0.0, TimerMode::Once),
                    initialized: false,
                    spikes_spawned: 0,
                },
            );

        // NOTE: No automatic back-transitions to FollowState. They would use
        // `(GolemSpikeAttackTrigger/GolemWaveAttackTrigger)`, which fire the instant we
        // enter an attack (the cooldown reset makes the forward trigger false → `not` true), kicking
        // the golem out of the attack before the animation can play. Instead,
        // `check_spike_attack_completion` / `check_wave_attack_completion` manually remove the attack
        // state and re-insert FollowState once the attack finishes (same pattern as the scorpion boss).

        e_cmds.insert(state_machine);
    }
}

pub fn tick_spike_attack_timer(
    mut timers: Query<&mut SpikeAttackTimer, Without<crate::combat::MarkedForDeath>>,
    time: Res<Time>,
) {
    let mut rng = rand::thread_rng();
    for mut timer in timers.iter_mut() {
        timer.random_timer.tick(time.delta());
        timer.cooldown_timer.tick(time.delta());

        if timer.random_timer.is_finished()
            && timer.cooldown_timer.is_finished()
            && timer.attack_choice.is_none()
        {
            timer.attack_choice = Some(rng.gen_bool(WAVE_ATTACK_CHANCE as f64));
        }
    }
}

/// Helper function to spawn a golem spike hitbox with animation
pub fn spawn_golem_spike_hitbox(
    commands: &mut Commands,
    stone_pillar_asset: Handle<Aseprite>,
    pos: Vec3,
    dmg: i32,
    golem_entity: Entity,
    z_offset: f32,
) {
    // Create default animation (empty tag = default once-play clip on pillar sheet)
    let z = (PILLAR_BASE_Z + z_offset).min(PILLAR_MAX_Z);
    let transform = Transform::from_translation(pos + Vec3::new(0., 32., z));

    // Queue deferred spawn - actual entity will be created in PreUpdate
    spawn_deferred_aseprite_collider(
        commands,
        transform,
        SPIKE_DURATION,
        dmg,
        Collider::capsule(Vec2::new(0., -32.), Vec2::new(0., -32.), 16.),
        stone_pillar_asset,
        "",
        false,
        Projectile::GolemSpike,
        vec![DeferredComponent::EnemyProjectile {
            entity: golem_entity,
            mob: Mob::StoneGolem,
        }],
        None, // No parent
    );
}

/// Initialize spike attack state with random spike count when first entered
pub fn initialize_spike_attack_state(
    mut commands: Commands,
    mut attacks: Query<
        (
            Entity,
            &mut SpikeAttackState,
            &mut AseAnimation,
            &mut GolemCurrentTag,
            &mut KinematicCharacterController,
            &mut SpikeAttackTimer,
        ),
        (
            Added<SpikeAttackState>,
            Without<crate::combat::MarkedForDeath>,
        ),
    >,
) {
    let mut rng = rand::thread_rng();
    for (entity, mut state, mut anim, mut current_tag, mut kcc, mut timer) in attacks.iter_mut() {
        commands.entity(entity).remove::<FollowState>();
        kcc.translation = None;
        timer.cooldown_timer = Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Once);
        timer.attack_choice = None;

        if !state.initialized {
            state.num_spikes_left = rng.gen_range(SPIKE_ATTACK_COUNT_MIN..=SPIKE_ATTACK_COUNT_MAX);
            state.spikes_spawned = 0;
            state.initialized = true;
        }
        play_golem_anim_force(&mut anim, &mut current_tag, StoneGolem::tags::ATTACK_FRONT);
    }
}

/// Handle spawning spike warnings - can spawn multiple simultaneously
pub fn handle_spike_attack(
    mut commands: Commands,
    mut attacks: Query<
        (
            Entity,
            &mut SpikeAttackState,
            &mut KinematicCharacterController,
        ),
        Without<crate::combat::MarkedForDeath>,
    >,
    player_query: Query<&Transform, With<Player>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
    cheat_settings: Res<CheatSettings>,
) {
    for (entity, mut state, mut kcc) in attacks.iter_mut() {
        kcc.translation = None;
        state.attack_timer.tick(time.delta());

        // Spawn new warning when timer finishes and we still have spikes left
        if state.attack_timer.is_finished() && state.num_spikes_left > 0 {
            if let Ok(player_txfm) = player_query.single() {
                let mut rng = rand::thread_rng();

                let offset_distance = rng.gen_range(7.0..=SPIKE_DISTANCE_OFFSET_MAX);
                let offset_angle = rng.gen_range(0.0..std::f32::consts::TAU);
                let offset = Vec2::from_angle(offset_angle) * offset_distance;

                let target_pos = player_txfm.translation.truncate() + offset;
                let z_offset = state.spikes_spawned as f32 * PILLAR_Z_STAGGER;

                commands.spawn((
                    Mesh2d(meshes.add(Mesh::from(Circle::new(16.)))),
                    MeshMaterial2d(materials.add(ColorMaterial::from(
                        boss_warning_indicator_color(&cheat_settings),
                    ))),
                    Transform {
                        translation: target_pos.extend(990.0),
                        ..default()
                    },
                    BossAttackPreview,
                    SpikeWarning {
                        timer: Timer::from_seconds(SPIKE_WARNING_DELAY, TimerMode::Once),
                        target_pos,
                        golem_entity: entity,
                        z_offset,
                    },
                ));

                state.spikes_spawned += 1;
                state.num_spikes_left -= 1;

                if state.num_spikes_left > 0 {
                    state.attack_timer = Timer::from_seconds(
                        rng.gen_range(SPIKE_ATTACK_INTERVAL_MIN..=SPIKE_ATTACK_INTERVAL_MAX),
                        TimerMode::Once,
                    );
                }
            }
        }
    }
}

/// Enter wave attack: stop following, face the player, play the attack anim.
pub fn initialize_wave_attack_state(
    mut commands: Commands,
    game: GameParam,
    global_transforms: Query<&GlobalTransform>,
    mut attacks: Query<
        (
            Entity,
            &mut WaveAttackState,
            &mut AseAnimation,
            &mut Transform,
            &mut GolemCurrentTag,
            &mut KinematicCharacterController,
            &mut SpikeAttackTimer,
        ),
        (
            Added<WaveAttackState>,
            Without<crate::combat::MarkedForDeath>,
        ),
    >,
) {
    let Ok(player_global) = global_transforms.get(game.game.player) else {
        return;
    };
    let player_pos = player_global.translation().truncate();

    for (entity, mut state, mut anim, mut transform, mut current_tag, mut kcc, mut timer) in
        attacks.iter_mut()
    {
        commands.entity(entity).remove::<FollowState>();
        kcc.translation = None;
        timer.cooldown_timer = Timer::from_seconds(ATTACK_COOLDOWN, TimerMode::Once);
        timer.attack_choice = None;

        let golem_pos = transform.translation.truncate();
        let delta = player_pos - golem_pos;
        let facing = GolemFacing::from_delta(delta);
        state.facing = facing;

        play_golem_anim_force(&mut anim, &mut current_tag, facing.attack_tag());
        apply_golem_sprite_flip(&mut transform, delta);

        state.pillar_positions = compute_wave_pillar_positions(golem_pos, player_pos);
        state.warnings_spawned = false;
        state.warning_spawn_timer = Timer::from_seconds(WAVE_WARNING_START_DELAY, TimerMode::Once);
        state.finish_timer = Timer::from_seconds(WAVE_ATTACK_DURATION, TimerMode::Once);
        state.initialized = true;
    }
}

fn spawn_wave_warning_entities(
    commands: &mut Commands,
    golem_entity: Entity,
    positions: &[Vec2],
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<ColorMaterial>,
    cheat_settings: &CheatSettings,
) {
    for (idx, target_pos) in positions.iter().copied().enumerate() {
        let col = idx % WAVE_PILLARS_PER_ROW;
        let pillar_delay = WAVE_PILLAR_SPAWN_DELAY + col as f32 * WAVE_PILLAR_COLUMN_STAGGER;

        commands.spawn((
            Mesh2d(meshes.add(Mesh::from(Circle::new(16.)))),
            MeshMaterial2d(
                materials.add(ColorMaterial::from(boss_warning_indicator_color(
                    cheat_settings,
                ))),
            ),
            Transform {
                translation: target_pos.extend(990.0),
                ..default()
            },
            BossAttackPreview,
            SpikeWarning {
                timer: Timer::from_seconds(pillar_delay, TimerMode::Once),
                target_pos,
                golem_entity,
                z_offset: idx as f32 * PILLAR_Z_STAGGER,
            },
        ));
    }
}

/// Spawn wave warning circles once the start delay has elapsed.
pub fn spawn_delayed_wave_warnings(
    mut commands: Commands,
    time: Res<Time>,
    mut attacks: Query<(Entity, &mut WaveAttackState), Without<crate::combat::MarkedForDeath>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    cheat_settings: Res<CheatSettings>,
) {
    for (entity, mut state) in attacks.iter_mut() {
        if !state.initialized || state.warnings_spawned {
            continue;
        }

        state.warning_spawn_timer.tick(time.delta());
        if !state.warning_spawn_timer.is_finished() {
            continue;
        }

        spawn_wave_warning_entities(
            &mut commands,
            entity,
            &state.pillar_positions,
            &mut meshes,
            &mut materials,
            &cheat_settings,
        );
        state.warnings_spawned = true;
    }
}

/// Keep the golem stationary and on its attack animation for the whole wave attack.
pub fn maintain_wave_attack(
    mut attacks: Query<
        (
            &WaveAttackState,
            &mut AseAnimation,
            &mut GolemCurrentTag,
            &mut KinematicCharacterController,
        ),
        (
            With<WaveAttackState>,
            Without<crate::combat::MarkedForDeath>,
        ),
    >,
) {
    for (state, mut anim, mut current_tag, mut kcc) in attacks.iter_mut() {
        kcc.translation = None;
        if state.initialized {
            set_golem_anim(&mut anim, &mut current_tag, state.facing.attack_tag());
        }
    }
}

/// Check if wave attack is complete and transition back to FollowState
pub fn check_wave_attack_completion(
    mut commands: Commands,
    time: Res<Time>,
    mut attacks: Query<(Entity, &mut WaveAttackState), Without<crate::combat::MarkedForDeath>>,
    mut timers: Query<&mut SpikeAttackTimer>,
    mut anims: Query<(&mut AseAnimation, &mut GolemCurrentTag)>,
    game: GameParam,
    follow_speed_query: Query<&FollowSpeed>,
) {
    for (entity, mut state) in attacks.iter_mut() {
        if !state.initialized {
            continue;
        }

        state.finish_timer.tick(time.delta());
        if !state.finish_timer.is_finished() {
            continue;
        }

        let follow_speed = follow_speed_query.get(entity).map(|f| f.0).unwrap_or(0.65);

        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands
                .remove::<WaveAttackState>()
                .insert(FollowState {
                    target: game.game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed,
                });
        }

        if let Ok(mut timer) = timers.get_mut(entity) {
            let mut rng = rand::thread_rng();
            timer.random_timer = Timer::from_seconds(rng.gen_range(3.0..5.0), TimerMode::Once);
        }

        if let Ok((mut anim, mut current_tag)) = anims.get_mut(entity) {
            set_golem_anim(&mut anim, &mut current_tag, StoneGolem::tags::WALK_FRONT);
        }
    }
}

/// Handle spike warnings - tick timers and spawn spikes when warnings complete
pub fn handle_spike_warnings(
    mut commands: Commands,
    mut warnings: Query<(Entity, &mut SpikeWarning)>,
    graphics: Res<Graphics>,
    time: Res<Time>,
) {
    for (warning_entity, mut warning) in warnings.iter_mut() {
        warning.timer.tick(time.delta());

        if warning.timer.is_finished() {
            spawn_golem_spike_hitbox(
                &mut commands,
                graphics.stone_pillar_ase.as_ref().unwrap().clone(),
                warning.target_pos.extend(0.0),
                SPIKE_DAMAGE,
                warning.golem_entity,
                warning.z_offset,
            );

            commands.entity(warning_entity).despawn();
        }
    }
}

/// Check if spike attack is complete and transition back to FollowState
pub fn check_spike_attack_completion(
    mut commands: Commands,
    attacks: Query<(Entity, &SpikeAttackState), Without<crate::combat::MarkedForDeath>>,
    warnings: Query<&SpikeWarning>,
    mut timers: Query<&mut SpikeAttackTimer>,
    mut anims: Query<(&mut AseAnimation, &mut GolemCurrentTag)>,
    game: GameParam,
    follow_speed_query: Query<&FollowSpeed>,
) {
    for (entity, state) in attacks.iter() {
        if state.num_spikes_left == 0 && state.attack_timer.is_finished() {
            let has_active_warnings = warnings.iter().any(|w| w.golem_entity == entity);

            if !has_active_warnings {
                let follow_speed = follow_speed_query.get(entity).map(|f| f.0).unwrap_or(0.65);

                if let Ok(mut entity_commands) = commands.get_entity(entity) {
                    entity_commands
                        .remove::<SpikeAttackState>()
                        .insert(FollowState {
                            target: game.game.player,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed,
                        });
                }

                if let Ok(mut timer) = timers.get_mut(entity) {
                    let mut rng = rand::thread_rng();
                    timer.random_timer =
                        Timer::from_seconds(rng.gen_range(3.0..5.0), TimerMode::Once);
                }

                if let Ok((mut anim, mut current_tag)) = anims.get_mut(entity) {
                    set_golem_anim(&mut anim, &mut current_tag, StoneGolem::tags::WALK_FRONT);
                }
            }
        }
    }
}

pub fn update_stone_golem_walk_animation(
    mut query: Query<
        (
            Entity,
            &mut Transform,
            &mut AseAnimation,
            &mut GolemCurrentTag,
            &Mob,
        ),
        (With<FollowState>, Without<crate::combat::MarkedForDeath>),
    >,
    player_query: Query<&Transform, (With<Player>, Without<FollowState>)>,
) {
    if let Ok(player_transform) = player_query.single() {
        let player_pos = player_transform.translation.truncate();

        for (_entity, mut transform, mut anim, mut current_tag, mob) in query.iter_mut() {
            if mob != &Mob::StoneGolem {
                continue;
            }

            let my_pos = transform.translation.truncate();
            let delta = player_pos - my_pos;

            // Small deadzone to prevent jitter when stopping or very close
            if delta.length_squared() < 4.0 {
                continue;
            }

            let abs_x = delta.x.abs();
            let abs_y = delta.y.abs();

            let keep_current = if abs_x <= abs_y * 1.1
                && abs_y <= abs_x * 1.1
                && is_golem_walk_tag(&current_tag.0)
            {
                Some(current_tag.0.clone())
            } else {
                None
            };

            // 10% buffer to prevent rapid switching between Side and Vertical
            let desired_anim = if abs_x > abs_y * 1.1 {
                StoneGolem::tags::WALK_SIDE
            } else if abs_y > abs_x * 1.1 {
                if delta.y > 0. {
                    StoneGolem::tags::WALK_BACK
                } else {
                    StoneGolem::tags::WALK_FRONT
                }
            } else if let Some(tag) = keep_current.as_deref() {
                tag
            } else if abs_x > abs_y {
                StoneGolem::tags::WALK_SIDE
            } else if delta.y > 0. {
                StoneGolem::tags::WALK_BACK
            } else {
                StoneGolem::tags::WALK_FRONT
            };

            set_golem_anim(&mut anim, &mut current_tag, desired_anim);

            // Handle Flipping for Side Walk with threshold
            if abs_x > 1.0 {
                if delta.x < 0. {
                    // Moving left
                    if transform.scale.x > 0. {
                        transform.scale.x = -1.0;
                    }
                } else {
                    // Moving right
                    if transform.scale.x < 0. {
                        transform.scale.x = 1.0;
                    }
                }
            }
        }
    }
}

/// Handle StoneGolem death - despawn immediately since it doesn't have death animations
pub fn handle_stone_golem_death(
    mut commands: Commands,
    death: Query<Entity, (With<DeathState>, With<Mob>)>,
    mob_query: Query<&Mob>,
) {
    for entity in death.iter() {
        // Only handle StoneGolem
        if let Ok(mob) = mob_query.get(entity) {
            if mob == &Mob::StoneGolem {
                // Despawn immediately - StoneGolem doesn't have death animations
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Move stone golem towards player when in FollowState
/// This is separate from the regular follow system because stone golem uses Aseprite animations
pub fn stone_golem_follow(
    transforms: Query<&mut Transform>,
    mut mover: Query<&mut KinematicCharacterController>,
    mut follows: Query<
        (Entity, &FollowState, &Mob, Option<&MobStatusEffects>),
        (
            Without<crate::combat::MarkedForDeath>,
            Without<WaveAttackState>,
            Without<SpikeAttackState>,
        ),
    >,
    time: Res<Time>,
) {
    for (entity, follow, mob, status_option) in follows.iter_mut() {
        // Only handle StoneGolem
        if mob != &Mob::StoneGolem {
            continue;
        }

        // Get the positions of the follower and target
        let target_translation = transforms.get(follow.target).unwrap().translation;
        let follow_transform = transforms.get(entity).unwrap();
        let follow_translation = follow_transform.translation;

        // Calculate direction to target
        let delta =
            (target_translation.truncate() - follow_translation.truncate()).normalize_or_zero();

        // Get mover and set collision filter to pass through world objects
        let mut mover = mover.get_mut(entity).unwrap();
        mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));

        // Apply movement with slow debuff
        mover.translation = Some(
            delta
                * follow.speed
                * PLAYER_MOVE_SPEED
                * time.delta_secs()
                * status_option
                    .map(|s| s.movement_speed_multiplier())
                    .unwrap_or(1.0),
        );
    }
}
