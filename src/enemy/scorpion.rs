use crate::aseprite_assets::Scorpion;
use crate::aseprite_helpers::{
    ase_animation, aseprite_bundle, collect_finished, is_paused, pause, play_loop, play_once, start,
};
use bevy::prelude::*;
use bevy_aseprite_ultra::prelude::{AnimationEvents, AnimationState, AseAnimation, Aseprite};
use bevy_rapier2d::prelude::{
    Collider, CollisionGroups, Group, KinematicCharacterController, Sensor,
};
use seldom_state::prelude::StateMachine;
use serde::Deserialize;

use crate::{
    ai::{EnemyAttackCooldown, FollowState},
    animations::enemy_sprites::spawn_attack_warning_aseprite,
    animations::HitAnimationTracker,
    attributes::Attack,
    bounce::spawn_desert_tornado,
    collisions::DamagesWorldObjects,
    combat::{
        pickup_radius::{pull_all_eligible_ground_items_to_player, BeingPulledToPlayer},
        status_effects::MobStatusEffects,
    },
    ecs_helpers::SafeHierarchyExt,
    enemy::{
        red_mushking::DeathState, spawner::MobSpawningPaused, FollowSpeed, Mob, MobIsAttacking,
    },
    inventory::{Inventory, ItemStack},
    item::ItemDrop,
    item::{
        boss_shrine::BossSummonIndex,
        projectile::{Projectile, RangedAttackEvent},
    },
    night::EraTimer,
    pets::state::Pet,
    player::Player,
    proto::proto_param::ProtoParam,
    world::{
        dimension::{Era, EraManager},
        portal::BossKillTracker,
    },
    GameParam, PLAYER_MOVE_SPEED,
};

/// Target standoff while following (px). Boss backs away when closer than this minus a buffer.
const SCORPION_COMFORT_DIST: f32 = 60.0;
/// Stay inside this distance before retreat starts (player can close ~this much under 40px).
const SCORPION_RETREAT_INNER_BUFFER: f32 = 16.0;
/// Hysteresis: keep retreating until at least this far past [`SCORPION_COMFORT_DIST`].
const SCORPION_RETREAT_OUTER_BUFFER: f32 = 16.0;

const SCORPION_RETREAT_ENTER_DIST_SQ: f32 = (SCORPION_COMFORT_DIST - SCORPION_RETREAT_INNER_BUFFER)
    * (SCORPION_COMFORT_DIST - SCORPION_RETREAT_INNER_BUFFER);
const SCORPION_RETREAT_EXIT_DIST_SQ: f32 = (SCORPION_COMFORT_DIST + SCORPION_RETREAT_OUTER_BUFFER)
    * (SCORPION_COMFORT_DIST + SCORPION_RETREAT_OUTER_BUFFER);

// Animation tag names (match the ase file).
const WALK_SIDE: &str = "walk-side";
const WALK_SOUTH: &str = "walk-south";
const WALK_NORTH: &str = "walk-north";

const PREP_CLAW_SIDE: &str = "prep-claw-side";
const PREP_CLAW_SOUTH: &str = "prep-claw-south";
const PREP_CLAW_NORTH: &str = "prep-claw-north";
const CLAW_LOOP_SIDE: &str = "claw-loop-side";
const CLAW_LOOP_SOUTH: &str = "claw-loop-south";
const CLAW_LOOP_NORTH: &str = "claw-loop-north";
const CLAW_ATTACK_SIDE: &str = "claw-attack-side";
const CLAW_ATTACK_SOUTH: &str = "claw-attack-south";
const CLAW_ATTACK_NORTH: &str = "claw-attack-north";

const PREP_TAIL_SIDE: &str = "prep-tail-side";
const PREP_TAIL_SOUTH: &str = "prep-tail-south";
const PREP_TAIL_NORTH: &str = "prep-tail-north";
const TAIL_LOOP_SIDE: &str = "tail-loop-side";
const TAIL_LOOP_SOUTH: &str = "tail-loop-south";
const TAIL_LOOP_NORTH: &str = "tail-loop-north";
const TAIL_ATTACK_SIDE: &str = "tail-attack-side";
const TAIL_ATTACK_SOUTH: &str = "tail-attack-south";
const TAIL_ATTACK_NORTH: &str = "tail-attack-north";

/// Scorpion boss claw attack config: 3-phase (prep -> loop -> attack with lunge + 34x34 hitbox).
#[derive(Debug, Default, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
pub struct ScorpionClawAttack {
    pub activation_distance: f32,
    pub cooldown: f32,
    /// Duration of the prep-claw animation tag.
    pub prep_duration: f32,
    /// Duration of the looping warning phase before the strike.
    pub loop_duration: f32,
    /// Duration of the lunge movement at the start of the attack frame.
    pub lunge_duration: f32,
    /// Forward speed during the lunge (pixels/sec).
    pub lunge_speed: f32,
    /// How long the 34x34 hitbox in front of the boss exists.
    pub hitbox_duration: f32,
    /// Offset in front of the boss where the hitbox spawns.
    pub hitbox_offset: f32,
}

/// Scorpion boss tail attack config: 3-phase (prep -> loop -> attack, 3 waves of cone projectiles).
#[derive(Debug, Default, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
pub struct ScorpionTailAttack {
    pub activation_distance: f32,
    pub cooldown: f32,
    pub prep_duration: f32,
    pub loop_duration: f32,
    /// Number of waves of projectiles (each wave re-aims at player).
    pub num_waves: u8,
    /// Projectiles per wave (fanned out in a cone).
    pub projectiles_per_wave: u8,
    /// Cone half-angle (radians) for the projectile spread.
    pub cone_half_angle: f32,
    /// Duration of one tail-attack animation loop (per wave).
    pub wave_duration: f32,
}

/// Scorpion boss passive tornado attack config: periodically spawns a desert tornado.
#[derive(Debug, Default, Reflect, Clone, Component, Deserialize)]
#[reflect(Component, Default)]
pub struct ScorpionTornadoAttack {
    /// Seconds between tornado spawns.
    pub interval: f32,
    /// Lifetime of each tornado (seconds).
    pub tornado_duration: f32,
    /// Tornado travel speed (pixels/sec).
    pub tornado_speed: f32,
    /// Spawn offset distance from the boss.
    pub spawn_distance: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default, Reflect)]
pub enum ScorpionFacing {
    #[default]
    South,
    North,
    Side,
}

impl ScorpionFacing {
    fn from_delta(delta: Vec2) -> Self {
        let abs_x = delta.x.abs();
        let abs_y = delta.y.abs();
        if abs_x > abs_y * 1.1 {
            Self::Side
        } else if delta.y > 0. {
            Self::North
        } else {
            Self::South
        }
    }

    fn walk_tag(self) -> &'static str {
        match self {
            Self::Side => WALK_SIDE,
            Self::North => WALK_NORTH,
            Self::South => WALK_SOUTH,
        }
    }
    fn prep_claw(self) -> &'static str {
        match self {
            Self::Side => PREP_CLAW_SIDE,
            Self::North => PREP_CLAW_NORTH,
            Self::South => PREP_CLAW_SOUTH,
        }
    }
    fn loop_claw(self) -> &'static str {
        match self {
            Self::Side => CLAW_LOOP_SIDE,
            Self::North => CLAW_LOOP_NORTH,
            Self::South => CLAW_LOOP_SOUTH,
        }
    }
    fn attack_claw(self) -> &'static str {
        match self {
            Self::Side => CLAW_ATTACK_SIDE,
            Self::North => CLAW_ATTACK_NORTH,
            Self::South => CLAW_ATTACK_SOUTH,
        }
    }
    fn prep_tail(self) -> &'static str {
        match self {
            Self::Side => PREP_TAIL_SIDE,
            Self::North => PREP_TAIL_NORTH,
            Self::South => PREP_TAIL_SOUTH,
        }
    }
    fn loop_tail(self) -> &'static str {
        match self {
            Self::Side => TAIL_LOOP_SIDE,
            Self::North => TAIL_LOOP_NORTH,
            Self::South => TAIL_LOOP_SOUTH,
        }
    }
    fn attack_tail(self) -> &'static str {
        match self {
            Self::Side => TAIL_ATTACK_SIDE,
            Self::North => TAIL_ATTACK_NORTH,
            Self::South => TAIL_ATTACK_SOUTH,
        }
    }
}

/// Hysteresis for follow-state "too close" backing: avoids flip-flop on the comfort ring.
#[derive(Component, Default)]
pub struct ScorpionComfortRing {
    pub retreating: bool,
}

/// Tracks current scorpion animation tag so we only reset when changing.
#[derive(Component, Default)]
pub struct ScorpionCurrentTag(pub String);

/// Tracks current scorpion facing for animation selection / lunge direction.
#[derive(Component, Default)]
pub struct ScorpionFacingDir(ScorpionFacing);

/// Cooldown before the next queued attack; reset with the finishing attack's own cooldown.
#[derive(Component)]
pub struct ScorpionAttackTimers {
    pub attack_cooldown: Timer,
}

/// Always-on tornado spawn timer (passive attack).
#[derive(Component)]
pub struct ScorpionTornadoTimer {
    pub timer: Timer,
    pub spawns_per_tick: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScorpionQueuedAttackKind {
    Claw,
    Tail,
}

/// Picked by [`scorpion_queue_next_attack`]; consumed when a claw/tail state begins.
#[derive(Component, Clone, Copy)]
#[component(storage = "SparseSet")]
pub struct ScorpionQueuedAttack(pub ScorpionQueuedAttackKind);

/// Child claw hitbox entity while lunging; despawned when the claw attack ends.
#[derive(Component, Default)]
pub struct ClawAttackCollider(pub Option<Entity>);

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct ClawAttackState {
    pub phase: ClawPhase,
    pub phase_timer: Timer,
    pub hitbox_timer: Timer,
    pub lunge_timer: Timer,
    pub cooldown_timer: Timer,
    pub facing: ScorpionFacing,
    /// Aim toward player at the start of [`ClawPhase::Attack`] (claw-attack tag); used for lunge + hitbox.
    pub lunge_dir: Vec2,
}

#[derive(Clone, Copy, Reflect, Debug, PartialEq, Eq)]
pub enum ClawPhase {
    Prep,
    Loop,
    Attack,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct TailAttackState {
    pub phase: TailPhase,
    pub phase_timer: Timer,
    pub cooldown_timer: Timer,
    pub waves_left: u8,
    pub projectiles_per_wave: u8,
    pub cone_half_angle: f32,
    pub wave_duration: f32,
    pub facing: ScorpionFacing,
    pub fired_this_wave: bool,
}

#[derive(Clone, Copy, Reflect, Debug, PartialEq, Eq)]
pub enum TailPhase {
    Prep,
    Loop,
    Wave,
}

/// World-space direction for the claw lunge: full 2D toward player, with cardinal fallback if overlapped.
fn scorpion_claw_lunge_dir(to_player: Vec2, facing_fallback: ScorpionFacing) -> Vec2 {
    let n = to_player.normalize_or_zero();
    if n.length_squared() > 1e-6 {
        return n;
    }
    match facing_fallback {
        ScorpionFacing::Side => {
            let sign = if to_player.x <= 0. { -1. } else { 1. };
            Vec2::new(sign, 0.)
        }
        ScorpionFacing::North => Vec2::Y,
        ScorpionFacing::South => -Vec2::Y,
    }
}

fn set_anim_tag(anim: &mut AseAnimation, current: &mut ScorpionCurrentTag, new_tag: &str) {
    if current.0 != new_tag {
        play_loop(anim, new_tag);
        current.0 = new_tag.to_string();
    }
}

/// Horizontal flip when aim is side-dominant; matches `apply_horizontal_sprite_flip_for_dir` in `aseprite_enemy`.
fn apply_scorpion_sprite_flip(transform: &mut Transform, dir: Vec2) {
    let abs_x = dir.x.abs();
    let abs_y = dir.y.abs();
    if abs_x > abs_y * 1.1 && abs_x > f32::EPSILON {
        if dir.x < 0. && transform.scale.x > 0. {
            transform.scale.x = -1.0;
        } else if dir.x > 0. && transform.scale.x < 0. {
            transform.scale.x = 1.0;
        }
    }
}

/// Re-aim walk / attack tags toward the player (call between attack phases and when walking).
fn refresh_scorpion_facing<'a>(
    delta: Vec2,
    facing_out: &mut ScorpionFacing,
    facing_dir: &mut ScorpionFacingDir,
    anim: &mut AseAnimation,
    current_tag: &mut ScorpionCurrentTag,
    tag_for_facing: impl Fn(ScorpionFacing) -> &'static str,
    transform: Option<Mut<'a, Transform>>,
    hit: Option<&HitAnimationTracker>,
) {
    // Knockback temporarily moves the body; `to_player` can cross zero / flip dominance for a
    // frame and spuriously toggle `scale.x`. Skip facing until hit-react finishes.
    if hit.map(|h| h.is_active).unwrap_or(false) {
        return;
    }
    if delta.length_squared() < 0.01 {
        return;
    }
    let f = ScorpionFacing::from_delta(delta);
    *facing_out = f;
    facing_dir.0 = f;
    set_anim_tag(anim, current_tag, tag_for_facing(f));
    if let Some(mut tf) = transform {
        apply_scorpion_sprite_flip(&mut tf, delta);
    }
}

pub fn handle_new_scorpion_state_machine(
    mut commands: Commands,
    mut spawn_events: Query<
        (
            Entity,
            &Mob,
            &Transform,
            &FollowSpeed,
            &ScorpionClawAttack,
            &ScorpionTailAttack,
            &ScorpionTornadoAttack,
            &mut KinematicCharacterController,
            Option<&BossSummonIndex>,
        ),
        Added<Mob>,
    >,
    asset_server: Res<AssetServer>,
    game: GameParam,
) {
    for (
        e,
        mob,
        transform,
        follow_speed,
        claw_cfg,
        tail_cfg,
        tornado_cfg,
        mut mover,
        boss_summon_index,
    ) in spawn_events.iter_mut()
    {
        if mob != &Mob::Scorpion {
            continue;
        }
        let summon_index = boss_summon_index.copied().unwrap_or(BossSummonIndex(0));
        let tail_waves = tail_cfg
            .num_waves
            .saturating_add(summon_index.scorpion_extra_tail_waves());
        let tornado_interval =
            tornado_cfg.interval / summon_index.scorpion_tornado_frequency_scale();
        let tornado_spawns_per_tick = summon_index.scorpion_tornado_spawns_per_tick();
        let mut animation = ase_animation(asset_server.load(Scorpion::PATH), WALK_SOUTH, false);
        start(&mut animation);
        mover.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));

        commands
            .entity(e)
            .insert(CollisionGroups::new(Group::GROUP_1, Group::GROUP_1))
            .insert((
                animation,
                Sprite::default(),
                *transform,
                GlobalTransform::default(),
                Visibility::Inherited,
                InheritedVisibility::default(),
                ViewVisibility::default(),
            ))
            .insert(FollowState {
                target: game.game.player,
                curr_delta: None,
                curr_path: None,
                speed: follow_speed.0,
            })
            .insert(DamagesWorldObjects)
            .insert(ScorpionCurrentTag(WALK_SOUTH.to_string()))
            .insert(ScorpionFacingDir::default())
            .insert(ScorpionComfortRing::default())
            .insert(ScorpionAttackTimers {
                attack_cooldown: Timer::from_seconds(1., TimerMode::Once),
            })
            .insert(ScorpionTornadoTimer {
                timer: Timer::from_seconds(tornado_interval, TimerMode::Repeating),
                spawns_per_tick: tornado_spawns_per_tick,
            })
            .insert(ClawAttackCollider::default());

        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .with_state::<DeathState>()
            .trans::<FollowState, _>(
                claw_trigger,
                ClawAttackState {
                    phase: ClawPhase::Prep,
                    phase_timer: Timer::from_seconds(claw_cfg.prep_duration, TimerMode::Once),
                    hitbox_timer: Timer::from_seconds(claw_cfg.hitbox_duration, TimerMode::Once),
                    lunge_timer: Timer::from_seconds(claw_cfg.lunge_duration, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(claw_cfg.cooldown, TimerMode::Once),
                    facing: ScorpionFacing::South,
                    lunge_dir: Vec2::ZERO,
                },
            )
            .trans::<FollowState, _>(
                tail_trigger,
                TailAttackState {
                    phase: TailPhase::Prep,
                    phase_timer: Timer::from_seconds(tail_cfg.prep_duration, TimerMode::Once),
                    cooldown_timer: Timer::from_seconds(tail_cfg.cooldown, TimerMode::Once),
                    waves_left: tail_waves,
                    projectiles_per_wave: tail_cfg.projectiles_per_wave,
                    cone_half_angle: tail_cfg.cone_half_angle,
                    wave_duration: tail_cfg.wave_duration,
                    facing: ScorpionFacing::South,
                    fired_this_wave: false,
                },
            );

        commands.entity(e).insert(state_machine);

        debug!(
            entity = ?e,
            path = Scorpion::PATH,
            "Scorpion boss: inserted aseprite components and state machine"
        );
    }
}

/// Walks toward the player, picking walk-side / walk-south / walk-north.
pub fn scorpion_follow(
    mut transforms: Query<&mut Transform>,
    mut movers: Query<&mut KinematicCharacterController>,
    mut follows: Query<
        (
            Entity,
            &mut FollowState,
            &Mob,
            &mut AseAnimation,
            &mut ScorpionCurrentTag,
            &mut ScorpionFacingDir,
            &mut ScorpionComfortRing,
            Option<&FollowSpeed>,
            Option<&MobStatusEffects>,
            Option<&HitAnimationTracker>,
        ),
        Without<crate::combat::MarkedForDeath>,
    >,
    time: Res<Time>,
) {
    for (
        entity,
        mut follow,
        mob,
        mut anim,
        mut current_tag,
        mut facing_dir,
        mut comfort,
        follow_speed,
        status_option,
        hit_option,
    ) in follows.iter_mut()
    {
        if mob != &Mob::Scorpion {
            continue;
        }
        if status_option.map(|s| s.is_frozen()).unwrap_or(false) {
            continue;
        }
        let Ok(target_tf) = transforms.get(follow.target) else {
            continue;
        };
        let target_pos = target_tf.translation.truncate();
        let my_pos = transforms.get(entity).unwrap().translation.truncate();
        let to_target = target_pos - my_pos;
        let dist_sq = to_target.length_squared();
        if comfort.retreating {
            if dist_sq >= SCORPION_RETREAT_EXIT_DIST_SQ {
                comfort.retreating = false;
            }
        } else if dist_sq < SCORPION_RETREAT_ENTER_DIST_SQ {
            comfort.retreating = true;
        }

        let delta = to_target.normalize_or_zero();
        let move_dir = if comfort.retreating { -delta } else { delta };
        let move_speed_mul = if comfort.retreating { 0.65 } else { 1.0 };
        let speed = follow_speed.map(|s| s.0).unwrap_or(follow.speed);
        follow.speed = speed;

        follow.curr_delta = Some(delta);
        follow.curr_path = Some(delta);

        let mut mover = movers.get_mut(entity).unwrap();
        mover.translation = Some(
            move_dir
                * speed
                * PLAYER_MOVE_SPEED
                * time.delta_secs()
                * move_speed_mul
                * status_option
                    .map(|s| s.movement_speed_multiplier())
                    .unwrap_or(1.0),
        );

        if to_target.length_squared() >= 0.0625 && !hit_option.map(|h| h.is_active).unwrap_or(false)
        {
            let new_facing = ScorpionFacing::from_delta(to_target);
            facing_dir.0 = new_facing;
            set_anim_tag(&mut anim, &mut current_tag, new_facing.walk_tag());
            let mut tf = transforms.get_mut(entity).unwrap();
            apply_scorpion_sprite_flip(&mut tf, to_target);
            // Do not insert `FacingDirection` here: `change_character_anim_direction` would treat
            // this mob as a legacy atlas mob and replace the aseprite atlas with `mob_spritesheets`.
        }
    }
}

// ---------- triggers ----------
fn claw_trigger(
    In(entity): In<Entity>,
    q: Query<(
        &Transform,
        &ScorpionAttackTimers,
        Option<&EnemyAttackCooldown>,
        Option<&ScorpionQueuedAttack>,
        &ScorpionClawAttack,
    )>,
    player: Query<&Transform, With<Player>>,
) -> bool {
    let Ok((my_tf, timers, cd, queued, attack)) = q.get(entity) else {
        return false;
    };
    if cd.is_some() {
        return false;
    }
    if !timers.attack_cooldown.is_finished() {
        return false;
    }
    if !matches!(
        queued,
        Some(&ScorpionQueuedAttack(ScorpionQueuedAttackKind::Claw))
    ) {
        return false;
    }
    let Ok(player_tf) = player.single() else {
        return false;
    };
    let dist_sq = my_tf
        .translation
        .truncate()
        .distance_squared(player_tf.translation.truncate());
    dist_sq <= attack.activation_distance * attack.activation_distance
}

fn tail_trigger(
    In(entity): In<Entity>,
    q: Query<(
        &Transform,
        &ScorpionAttackTimers,
        Option<&EnemyAttackCooldown>,
        Option<&ScorpionQueuedAttack>,
        &ScorpionTailAttack,
    )>,
    player: Query<&Transform, With<Player>>,
) -> bool {
    let Ok((my_tf, timers, cd, queued, attack)) = q.get(entity) else {
        return false;
    };
    if cd.is_some() {
        return false;
    }
    if !timers.attack_cooldown.is_finished() {
        return false;
    }
    if !matches!(
        queued,
        Some(&ScorpionQueuedAttack(ScorpionQueuedAttackKind::Tail))
    ) {
        return false;
    }
    let Ok(player_tf) = player.single() else {
        return false;
    };
    let dist_sq = my_tf
        .translation
        .truncate()
        .distance_squared(player_tf.translation.truncate());
    dist_sq <= attack.activation_distance * attack.activation_distance
}

pub fn tick_scorpion_timers(
    mut timers: Query<(&mut ScorpionAttackTimers, Option<&EnemyAttackCooldown>), With<Mob>>,
    time: Res<Time>,
) {
    for (mut t, cd) in timers.iter_mut() {
        if cd.is_some() {
            continue;
        }
        t.attack_cooldown.tick(time.delta());
    }
}

/// When off cooldown and in range, queue a random claw vs. tail attack for the state machine.
pub fn scorpion_queue_next_attack(
    mut commands: Commands,
    bosses: Query<
        (
            Entity,
            &Mob,
            &GlobalTransform,
            &ScorpionAttackTimers,
            &ScorpionClawAttack,
            &ScorpionTailAttack,
            Option<&ScorpionQueuedAttack>,
            Option<&EnemyAttackCooldown>,
        ),
        (
            With<FollowState>,
            Without<ClawAttackState>,
            Without<TailAttackState>,
            Without<crate::combat::MarkedForDeath>,
        ),
    >,
    player_q: Query<&GlobalTransform, With<Player>>,
) {
    let Ok(player_tf) = player_q.single() else {
        return;
    };
    let player_pos = player_tf.translation();

    for (entity, mob, my_tf, timers, claw_cfg, tail_cfg, queued, cd) in bosses.iter() {
        if mob != &Mob::Scorpion {
            continue;
        }
        if cd.is_some() {
            continue;
        }
        if !timers.attack_cooldown.is_finished() {
            continue;
        }

        let my_pos = my_tf.translation();
        let delta = player_pos - my_pos;
        let dist_sq = delta.truncate().length_squared();
        let claw_sq = claw_cfg.activation_distance * claw_cfg.activation_distance;
        let tail_sq = tail_cfg.activation_distance * tail_cfg.activation_distance;

        if let Some(ScorpionQueuedAttack(kind)) = queued.copied() {
            let still_valid = match kind {
                ScorpionQueuedAttackKind::Claw => dist_sq <= claw_sq,
                ScorpionQueuedAttackKind::Tail => dist_sq <= tail_sq,
            };
            if !still_valid {
                commands.entity(entity).remove::<ScorpionQueuedAttack>();
            }
            continue;
        }

        if dist_sq > tail_sq {
            continue;
        }

        let kind = if dist_sq <= claw_sq {
            ScorpionQueuedAttackKind::Claw
        } else {
            ScorpionQueuedAttackKind::Tail
        };

        commands.entity(entity).insert(ScorpionQueuedAttack(kind));
    }
}

// ---------- Claw attack ----------
pub fn handle_claw_attack(
    mut commands: Commands,
    global_transforms: Query<&GlobalTransform>,
    mut local_transforms: Query<&mut Transform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &Attack,
            &mut KinematicCharacterController,
            &mut ClawAttackState,
            &ScorpionClawAttack,
            &FollowSpeed,
            &mut AseAnimation,
            &mut ScorpionCurrentTag,
            &mut ScorpionFacingDir,
            &mut ScorpionAttackTimers,
            &mut ClawAttackCollider,
            Option<&BossSummonIndex>,
            Option<&HitAnimationTracker>,
        ),
        Without<crate::combat::MarkedForDeath>,
    >,
    time: Res<Time>,
    game: GameParam,
    asset_server: Res<AssetServer>,
    mut finished_events: MessageReader<AnimationEvents>,
) {
    let finished = collect_finished(&mut finished_events);
    for (
        entity,
        mob,
        enemy_attack,
        mut kcc,
        mut state,
        claw_cfg,
        follow_speed,
        mut anim,
        mut current_tag,
        mut facing_dir,
        mut timers,
        mut claw_collider,
        boss_summon_index,
        hit_tracker,
    ) in attacks.iter_mut()
    {
        if mob != &Mob::Scorpion {
            continue;
        }
        let lunge_scale = boss_summon_index
            .map(|idx| idx.scorpion_lunge_scale())
            .unwrap_or(1.0);
        let lunge_speed = claw_cfg.lunge_speed * lunge_scale;
        let hitbox_offset = claw_cfg.hitbox_offset * lunge_scale;
        let my_pos = global_transforms.get(entity).unwrap().translation();
        let player_pos = global_transforms
            .get(game.game.player)
            .unwrap()
            .translation();
        let delta = (player_pos - my_pos).truncate();

        match state.phase {
            ClawPhase::Prep => {
                if state.phase_timer.elapsed() == std::time::Duration::ZERO {
                    commands.entity(entity).remove::<ScorpionQueuedAttack>();
                }
                if state.phase_timer.elapsed_secs() == 0. {
                    refresh_scorpion_facing(
                        delta,
                        &mut state.facing,
                        &mut facing_dir,
                        &mut anim,
                        &mut current_tag,
                        |f| f.prep_claw(),
                        local_transforms.get_mut(entity).ok(),
                        hit_tracker,
                    );
                }
                state.phase_timer.tick(time.delta());
                if state.phase_timer.is_finished() {
                    state.phase = ClawPhase::Loop;
                    state.phase_timer =
                        Timer::from_seconds(claw_cfg.loop_duration, TimerMode::Once);
                    refresh_scorpion_facing(
                        delta,
                        &mut state.facing,
                        &mut facing_dir,
                        &mut anim,
                        &mut current_tag,
                        |f| f.loop_claw(),
                        local_transforms.get_mut(entity).ok(),
                        hit_tracker,
                    );
                }
            }
            ClawPhase::Loop => {
                state.phase_timer.tick(time.delta());
                if state.phase_timer.is_finished() {
                    state.phase = ClawPhase::Attack;
                    state.lunge_timer.reset();
                    state.hitbox_timer.reset();
                    refresh_scorpion_facing(
                        delta,
                        &mut state.facing,
                        &mut facing_dir,
                        &mut anim,
                        &mut current_tag,
                        |f| f.attack_claw(),
                        local_transforms.get_mut(entity).ok(),
                        hit_tracker,
                    );
                    state.lunge_dir = scorpion_claw_lunge_dir(delta, state.facing);
                    commands.entity(entity).insert(MobIsAttacking(mob.clone()));
                }
            }
            ClawPhase::Attack => {
                if claw_collider.0.is_none() {
                    let offset = Vec3::new(
                        state.lunge_dir.x * hitbox_offset,
                        state.lunge_dir.y * hitbox_offset,
                        1.,
                    );
                    let hitbox = commands
                        .spawn((
                            Transform::from_translation(offset),
                            Attack(enemy_attack.0),
                            Collider::cuboid(17., 17.),
                            Sensor,
                            MobIsAttacking(mob.clone()),
                        ))
                        .safe_set_parent(entity)
                        .id();
                    claw_collider.0 = Some(hitbox);
                    spawn_attack_warning_aseprite(
                        &mut commands,
                        &asset_server,
                        Vec3::new(offset.x, offset.y + 12., 10.),
                        entity,
                        claw_cfg.lunge_duration,
                    );
                }

                state.lunge_timer.tick(time.delta());
                if !state.lunge_timer.is_finished() {
                    kcc.translation = Some(state.lunge_dir * lunge_speed * time.delta_secs());
                }

                state.hitbox_timer.tick(time.delta());
                if finished.contains(&entity) {
                    if let Some(hitbox) = claw_collider.0.take() {
                        commands.entity(hitbox).despawn();
                    }
                    timers.attack_cooldown =
                        Timer::from_seconds(claw_cfg.cooldown, TimerMode::Once);
                    commands
                        .entity(entity)
                        .remove::<ClawAttackState>()
                        .remove::<MobIsAttacking>()
                        .insert(FollowState {
                            target: game.game.player,
                            curr_delta: None,
                            curr_path: None,
                            speed: follow_speed.0,
                        })
                        .insert(EnemyAttackCooldown(state.cooldown_timer.clone()));
                    set_anim_tag(&mut anim, &mut current_tag, state.facing.walk_tag());
                }
            }
        }
    }
}

// ---------- Tail attack ----------
pub fn handle_tail_attack(
    mut commands: Commands,
    global_transforms: Query<&GlobalTransform>,
    mut local_transforms: Query<&mut Transform>,
    mut attacks: Query<
        (
            Entity,
            &Mob,
            &Attack,
            &mut TailAttackState,
            &ScorpionTailAttack,
            &FollowSpeed,
            &mut AseAnimation,
            &mut ScorpionCurrentTag,
            &mut ScorpionFacingDir,
            &mut ScorpionAttackTimers,
            Option<&HitAnimationTracker>,
        ),
        Without<crate::combat::MarkedForDeath>,
    >,
    time: Res<Time>,
    game: GameParam,
    mut events: MessageWriter<RangedAttackEvent>,
) {
    for (
        entity,
        mob,
        enemy_attack,
        mut state,
        tail_cfg,
        follow_speed,
        mut anim,
        mut current_tag,
        mut facing_dir,
        mut timers,
        hit_tracker,
    ) in attacks.iter_mut()
    {
        if mob != &Mob::Scorpion {
            continue;
        }
        let my_pos = global_transforms.get(entity).unwrap().translation();
        let player_pos = global_transforms
            .get(game.game.player)
            .unwrap()
            .translation();
        let delta = (player_pos - my_pos).truncate();

        match state.phase {
            TailPhase::Prep => {
                if state.phase_timer.elapsed() == std::time::Duration::ZERO {
                    commands.entity(entity).remove::<ScorpionQueuedAttack>();
                }
                if state.phase_timer.elapsed_secs() == 0. {
                    refresh_scorpion_facing(
                        delta,
                        &mut state.facing,
                        &mut facing_dir,
                        &mut anim,
                        &mut current_tag,
                        |f| f.prep_tail(),
                        local_transforms.get_mut(entity).ok(),
                        hit_tracker,
                    );
                }
                state.phase_timer.tick(time.delta());
                if state.phase_timer.is_finished() {
                    state.phase = TailPhase::Loop;
                    state.phase_timer =
                        Timer::from_seconds(tail_cfg.loop_duration, TimerMode::Once);
                    refresh_scorpion_facing(
                        delta,
                        &mut state.facing,
                        &mut facing_dir,
                        &mut anim,
                        &mut current_tag,
                        |f| f.loop_tail(),
                        local_transforms.get_mut(entity).ok(),
                        hit_tracker,
                    );
                }
            }
            TailPhase::Loop => {
                state.phase_timer.tick(time.delta());
                if state.phase_timer.is_finished() {
                    state.phase = TailPhase::Wave;
                    state.phase_timer = Timer::from_seconds(state.wave_duration, TimerMode::Once);
                    state.fired_this_wave = false;
                    refresh_scorpion_facing(
                        delta,
                        &mut state.facing,
                        &mut facing_dir,
                        &mut anim,
                        &mut current_tag,
                        |f| f.attack_tail(),
                        local_transforms.get_mut(entity).ok(),
                        hit_tracker,
                    );
                    commands.entity(entity).insert(MobIsAttacking(mob.clone()));
                }
            }
            TailPhase::Wave => {
                if !state.fired_this_wave {
                    state.fired_this_wave = true;
                    let dir = delta.normalize_or_zero();
                    let base_angle = dir.y.atan2(dir.x);
                    let n = state.projectiles_per_wave.max(1);
                    for i in 0..n {
                        let t = if n == 1 {
                            0.0
                        } else {
                            (i as f32 / (n - 1) as f32) * 2.0 - 1.0
                        };
                        let angle = base_angle + t * state.cone_half_angle;
                        let proj_dir = Vec2::new(angle.cos(), angle.sin());
                        events.write(RangedAttackEvent {
                            projectile: Projectile::ScorpionProjectile,
                            direction: proj_dir,
                            from_entity: Some(entity),
                            from_enemy: true,
                            is_followup_proj: false,
                            mana_cost: None,
                            mana_cost_heirloom: None,
                            dmg_override: Some(enemy_attack.0),
                            pos_override: None,
                            spawn_delay: 0.,
                        });
                    }
                }
                state.phase_timer.tick(time.delta());
                if state.phase_timer.is_finished() {
                    state.waves_left = state.waves_left.saturating_sub(1);
                    if state.waves_left == 0 {
                        timers.attack_cooldown =
                            Timer::from_seconds(tail_cfg.cooldown, TimerMode::Once);
                        commands
                            .entity(entity)
                            .remove::<TailAttackState>()
                            .remove::<MobIsAttacking>()
                            .insert(FollowState {
                                target: game.game.player,
                                curr_delta: None,
                                curr_path: None,
                                speed: follow_speed.0,
                            })
                            .insert(EnemyAttackCooldown(state.cooldown_timer.clone()));
                        set_anim_tag(&mut anim, &mut current_tag, state.facing.walk_tag());
                    } else {
                        state.phase = TailPhase::Loop;
                        state.phase_timer = Timer::from_seconds(0.2, TimerMode::Once);
                        refresh_scorpion_facing(
                            delta,
                            &mut state.facing,
                            &mut facing_dir,
                            &mut anim,
                            &mut current_tag,
                            |f| f.loop_tail(),
                            local_transforms.get_mut(entity).ok(),
                            hit_tracker,
                        );
                    }
                }
            }
        }
    }
}

// ---------- Passive tornado spawner ----------
pub fn tick_tornado_timer(
    mut commands: Commands,
    mut timers: Query<
        (
            Entity,
            &Mob,
            &GlobalTransform,
            &mut ScorpionTornadoTimer,
            &ScorpionTornadoAttack,
        ),
        Without<crate::combat::MarkedForDeath>,
    >,
    asset_server: Res<AssetServer>,
    game: GameParam,
    time: Res<Time>,
) {
    for (_entity, mob, tf, mut t, cfg) in timers.iter_mut() {
        if mob != &Mob::Scorpion {
            continue;
        }
        t.timer.tick(time.delta());
        if !t.timer.just_finished() {
            continue;
        }
        let my_pos = tf.translation().truncate();
        let player_pos = game.player().position.truncate();
        let to_player = (player_pos - my_pos).normalize_or_zero();
        let spawn_pos = my_pos + to_player * cfg.spawn_distance;
        let dir = to_player;
        let spawns = t.spawns_per_tick.max(1);
        for i in 0..spawns {
            let spread = if spawns > 1 {
                (i as f32 - (spawns as f32 - 1.) / 2.) * 0.35
            } else {
                0.
            };
            let spread_dir = Vec2::from_angle(dir.y.atan2(dir.x) + spread);
            spawn_desert_tornado(
                &mut commands,
                &asset_server,
                spawn_pos,
                spread_dir,
                cfg.tornado_speed,
                cfg.tornado_duration,
            );
        }
    }
}

// ---------- Death handling ----------
pub fn handle_scorpion_death(
    mut commands: Commands,
    mut death: Query<(Entity, &mut AseAnimation, &Mob, &mut ScorpionCurrentTag), With<DeathState>>,
    era_manager: Res<EraManager>,
    mut boss_kill_tracker: ResMut<BossKillTracker>,
    era_timer: Res<EraTimer>,
    mut mob_spawning_paused: ResMut<MobSpawningPaused>,
    item_drop_query: Query<(Entity, &ItemStack), (With<ItemDrop>, Without<BeingPulledToPlayer>)>,
    inv: Query<&Inventory, With<Player>>,
    pets: Query<(), With<Pet>>,
    proto: ProtoParam,
) {
    for (entity, mut anim, mob, mut current_tag) in death.iter_mut() {
        if mob != &Mob::Scorpion {
            continue;
        }
        // Snap to a stationary walk tag (scorpion has no dedicated death animation).
        set_anim_tag(&mut anim, &mut current_tag, WALK_SOUTH);

        boss_kill_tracker.mark_boss_killed(era_manager.current_era.clone());
        if (era_manager.current_era == Era::Main || era_manager.current_era == Era::Second)
            && era_timer.remaining_seconds > 0.0
        {
            mob_spawning_paused.paused = true;
        }

        pull_all_eligible_ground_items_to_player(
            &mut commands,
            &item_drop_query,
            &inv,
            &pets,
            &proto,
        );

        commands.entity(entity).despawn();
    }
}
