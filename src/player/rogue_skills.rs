use std::{f32::consts::PI, time::Duration};

use crate::{
    animations::{player_sprite::PlayerAnimation, AttackEvent, DoneAnimation},
    attributes::{attribute_helpers::skill_power_multiplier, Attack, CurrentMana, SkillPower},
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    colors::BLACK,
    combat_helpers::spawn_temp_collider,
    cursor::CursorPos,
    ecs_helpers::{safe_set_parent, SafeHierarchyExt},
    enemy::Mob,
    inputs::{skill_aim_direction, FacingDirection, MovementVector},
    item::projectile::Projectile,
    ui::{
        damage_numbers::{spawn_text, DodgeEvent},
        game_fonts::FLOATING_TEXT,
    },
    world::{y_sort::YSort, TILE_SIZE},
    AttackTimer, EnemyDeathEvent, GameParam, HitEvent, PLAYER_MOVE_SPEED,
};
use bevy::{prelude::*, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group, KinematicCharacterController};

use super::{
    melee_skills::{spawn_delayed_heirloom_cast, DelayedCastType, HEIRLOOM_EXTRA_CAST_DELAY},
    skills::active_skill_scaling::{attack_damage_multiplier, SPRINT_LUNGE},
    ActiveSkill, ActiveSkillUsedEvent, Heirloom, Player, PlayerSkills,
};

aseprite!(pub Combo, "textures/effects/Combo.aseprite");

/// Player sprint active-skill state. Lives on the player only while the sprint
/// skill is running, so it churns on every use — `SparseSet` avoids moving the
/// player between its "base" and "+SprintState" archetypes every activation.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct SprintState {
    pub startup_timer: Timer,
    pub sprint_duration_timer: Timer,
    pub speed_bonus: f32,
}

/// Player lunge active-skill state; same churn profile as `SprintState`.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct LungeState {
    pub lunge_duration: Timer,
    pub lunge_speed: f32,
}

/// Direction of the active dash plus a counter of tracer shadows already spawned
/// this lunge. Stored as a separate component (rather than fields on
/// [`LungeState`]) because `handle_active_skill_event` re-inserts `LungeState`
/// at the end of the activation frame, which would otherwise clobber the
/// direction captured by `handle_lunge` and leave subsequent tracers oriented
/// incorrectly.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct LungeDashInfo {
    pub direction: Vec2,
    pub tracers_spawned: u8,
}

/// World-space "ghost" sprite left behind during a lunge. Fades out then despawns.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct LungeShadow {
    pub timer: Timer,
}

/// Source art for [`LungeShadow`] points straight down.
const LUNGE_SHADOW_DEFAULT_DIR: Vec2 = Vec2::NEG_Y;
/// How long a tracer stays on-screen before being despawned.
const LUNGE_SHADOW_LIFETIME_SECS: f32 = 0.2;
/// Alpha the tracer starts at. Fades linearly to 0 over its lifetime.
const LUNGE_SHADOW_START_ALPHA: f32 = 1.;

pub(crate) fn spawn_lunge_shadow(
    commands: &mut Commands,
    asset_server: &AssetServer,
    world_pos: Vec3,
    direction: Vec2,
) {
    let angle = if direction.length_squared() > 0.0 {
        LUNGE_SHADOW_DEFAULT_DIR.angle_between(direction)
    } else {
        0.0
    };
    commands
        .spawn(SpriteBundle {
            texture: asset_server.load("textures/player/PlayerLungeShadow.png"),
            sprite: Sprite {
                color: Color::rgba(1.0, 1.0, 1.0, LUNGE_SHADOW_START_ALPHA),
                ..Default::default()
            },
            transform: Transform::from_translation(Vec3::new(world_pos.x, world_pos.y, 0.9))
                .with_rotation(Quat::from_rotation_z(angle)),
            ..Default::default()
        })
        .insert(LungeShadow {
            timer: Timer::from_seconds(LUNGE_SHADOW_LIFETIME_SECS, TimerMode::Once),
        })
        .insert(YSort(-0.01))
        .insert(Name::new("LungeShadow"));
}

pub fn tick_lunge_shadows(
    time: Res<Time>,
    mut query: Query<(Entity, &mut LungeShadow, &mut Sprite)>,
    mut commands: Commands,
) {
    for (e, mut shadow, mut sprite) in query.iter_mut() {
        shadow.timer.tick(time.delta());
        let remaining = 1.0 - shadow.timer.percent();
        sprite
            .color
            .set_a(LUNGE_SHADOW_START_ALPHA * remaining.max(0.0));
        if shadow.timer.finished() {
            commands.entity(e).despawn_recursive();
        }
    }
}

/// Marker present only while the player is actively sprinting. Toggled on/off
/// on every sprint activation, so `SparseSet` prevents the player moving
/// between archetype variants each time.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct Sprinting;
// Sprint is now activated via ActiveSkillUsedEvent in skill_heirlooms.rs
// This function is kept for compatibility but no longer handles toggle/release
pub fn handle_toggle_sprinting(
    mut sprint_query: Query<
        (Entity, &mut SprintState, &PlayerSkills, Option<&Sprinting>),
        With<SprintState>,
    >,
    _key_inputs: Res<Input<KeyCode>>,
    _commands: Commands,
) {
    // Sprint is now a button press ability, activated via ActiveSkillUsedEvent
    // No longer handles toggle/release logic
    for (_e, _sprint_state, _skills, _was_sprinting) in sprint_query.iter_mut() {
        // Sprint activation is handled in skill_heirlooms.rs::handle_active_skill_event
    }
}
pub fn handle_sprint_timer(
    time: Res<Time>,
    mut query: Query<
        (
            Entity,
            &mut SprintState,
            &mut KinematicCharacterController,
            &mut MovementVector,
            &PlayerAnimation,
            &PlayerSkills,
            Option<&AttackTimer>,
        ),
        With<Sprinting>,
    >,
    mouse_inputs: Res<Input<MouseButton>>,
    game: GameParam,
    mut attack_event: EventWriter<AttackEvent>,
    cursor_pos: Res<CursorPos>,
    mut commands: Commands,
) {
    for (e, mut sprint, mut kcc, mut mv, anim, skills, attack_cooldown_option) in query.iter_mut() {
        if !sprint.startup_timer.finished() {
            sprint.startup_timer.tick(time.delta());
            // Don't reset cooldown timer during startup - let it tick normally
        } else {
            if anim != &PlayerAnimation::Run && !anim.is_one_time_anim() {
                commands.entity(e).insert(PlayerAnimation::Run);
            }
            let speed_bonus_skill = skills.has(Heirloom::SprintFaster);
            mv.0 = mv.0 * (sprint.speed_bonus + if speed_bonus_skill { 0.2 } else { 0. });
            let player_pos = game.player().position;

            let direction =
                (cursor_pos.world_coords.truncate() - player_pos.truncate()).normalize_or_zero();
            if mouse_inputs.pressed(MouseButton::Left)
                && !anim.is_an_attack()
                && attack_cooldown_option.is_none()
            {
                commands
                    .entity(e)
                    .insert(PlayerAnimation::RunAttack2)
                    .insert(crate::animations::player_sprite::AttackAnimationTimer(
                        Timer::from_seconds(1.0, TimerMode::Once),
                    ));
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: false,
                });
            }

            if sprint
                .sprint_duration_timer
                .tick(time.delta())
                .just_finished()
            {
                commands.entity(e).remove::<Sprinting>();
            }

            kcc.translation = Some(Vec2::new(mv.0.x, mv.0.y));
        }
    }
}
pub fn handle_lunge(
    time: Res<Time>,
    mut active_skill_events: EventReader<ActiveSkillUsedEvent>,
    mut query: Query<(
        Entity,
        &mut LungeState,
        &mut KinematicCharacterController,
        &mut MovementVector,
        &PlayerSkills,
        &FacingDirection,
        &Attack,
        &OwnedBlessings,
        &mut CurrentMana,
        &SkillPower,
        &GlobalTransform,
        Option<&mut LungeDashInfo>,
    )>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    projectile_size: Query<&crate::attributes::ProjectileSize, With<Player>>,
    mut trigger_counts: ResMut<crate::player::skills::HeirloomTriggerCounts>,
    aim: Res<crate::aim::AimState>,
) {
    let activated_slots: Vec<usize> = active_skill_events.iter().map(|ev| ev.slot).collect();

    for (
        e,
        mut lunge_state,
        mut kcc,
        mut mv,
        skills,
        dir,
        dmg,
        blessings,
        mut current_mana,
        skill_power,
        global_transform,
        dash_info_opt,
    ) in query.iter_mut()
    {
        let lunge_slot = skills.has_active_skill(ActiveSkill::SprintLunge);
        let should_activate = lunge_slot
            .map(|slot| activated_slots.contains(&slot))
            .unwrap_or(false);

        if should_activate {
            // Cooldown and gating are handled by dispatch_active_skill_events.
            // Execute lunge mechanics without directly resetting the cooldown timer.
            commands.entity(e).insert(PlayerAnimation::Lunge);
            commands.spawn(SoundSpawner::new(AudioSoundEffect::Lunge, 0.2));

            // Prefer movement input when actively moving; otherwise aim (right stick /
            // mouseless mode) or sprite facing — same rules as teleport / roll.
            let dash_direction =
                skill_aim_direction(mv.0, aim.facing_dir, dir.get_dir_vec());
            // Tracer #1 fires immediately at the activation position. Subsequent tracers
            // are gated on `LungeDashInfo` which lives in its own component so it isn't
            // wiped by the `LungeState` re-insert that happens later this frame in
            // `handle_active_skill_event`.
            spawn_lunge_shadow(
                &mut commands,
                &asset_server,
                global_transform.translation(),
                dash_direction,
            );
            commands.entity(e).insert(LungeDashInfo {
                direction: dash_direction,
                tracers_spawned: 1,
            });

            let angle = match dir {
                FacingDirection::Up => 0.,
                FacingDirection::Down => 0.,
                FacingDirection::Left => PI / 2.,
                FacingDirection::Right => PI / 2.,
            };
            let skill_power_mult =
                skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());
            let lunge_e = spawn_temp_collider(
                &mut commands,
                Transform::from_translation(Vec3::new(0., 0., 0.))
                    .with_rotation(Quat::from_rotation_z(angle)),
                0.5,
                (dmg.0 as f32 * skill_power_mult * attack_damage_multiplier(SPRINT_LUNGE)) as i32,
                Collider::cuboid(9., 1.5 * TILE_SIZE.x),
                Projectile::SprintLunge,
            );
            safe_set_parent(&mut commands, lunge_e, e);

            {
                let echo_count = skills.get_count(Heirloom::SkillEcho);
                let mana_cost = Heirloom::SkillEcho.get_mana_cost();
                let echo_dmg = (dmg.0 as f32 * 1.) as i32;
                let size_mult = projectile_size
                    .get_single()
                    .map(|s| s.get_multiplier())
                    .unwrap_or(1.0);

                for i in 0..echo_count {
                    if current_mana.0 < mana_cost {
                        break;
                    }
                    current_mana.0 -= mana_cost;
                    trigger_counts.record_mana(Heirloom::SkillEcho, mana_cost);

                    if i == 0 {
                        crate::player::melee_skills::spawn_echo_hitbox(
                            &mut commands,
                            &asset_server,
                            e,
                            echo_dmg,
                            size_mult,
                        );
                    } else {
                        spawn_delayed_heirloom_cast(
                            &mut commands,
                            HEIRLOOM_EXTRA_CAST_DELAY * i as f32,
                            DelayedCastType::Echo {
                                player: e,
                                dmg: echo_dmg,
                                size_multiplier: size_mult,
                            },
                        );
                    }
                }
            }

            lunge_state.lunge_duration.tick(time.delta());
            mv.0 = mv.0 * 0.;
        } else if lunge_state.lunge_duration.percent() != 0. {
            lunge_state.lunge_duration.tick(time.delta());
            let percent = lunge_state.lunge_duration.percent();
            if let Some(mut dash_info) = dash_info_opt {
                if percent >= 0.40 && dash_info.tracers_spawned < 2 {
                    spawn_lunge_shadow(
                        &mut commands,
                        &asset_server,
                        global_transform.translation(),
                        dash_info.direction,
                    );
                    dash_info.tracers_spawned = 2;
                }
                if percent >= 0.80 && dash_info.tracers_spawned < 3 {
                    spawn_lunge_shadow(
                        &mut commands,
                        &asset_server,
                        global_transform.translation(),
                        dash_info.direction,
                    );
                    dash_info.tracers_spawned = 3;
                }
            }
            if lunge_state.lunge_duration.percent() >= 0.20
                && lunge_state.lunge_duration.percent() <= 0.45
            {
                commands
                    .entity(e)
                    .insert(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
                kcc.filter_groups = Some(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
                // Lunge distance must stay constant: take only the direction
                // from `mv` and rebuild magnitude from the base move speed,
                // so Speed stat / hunger / consumable buffs don't scale it.
                let lunge_dir = mv.0.normalize_or_zero();
                mv.0 =
                    lunge_dir * PLAYER_MOVE_SPEED * time.delta_seconds() * lunge_state.lunge_speed;
            } else if lunge_state.lunge_duration.percent() < 0.20 {
                mv.0 = mv.0 * 0.;
            } else {
                commands
                    .entity(e)
                    .insert(CollisionGroups::new(Group::ALL, Group::ALL));
                kcc.filter_groups = Some(CollisionGroups::new(Group::ALL, Group::ALL));
            }

            if lunge_state.lunge_duration.finished() {
                lunge_state.lunge_duration.reset();
                commands.entity(e).insert(PlayerAnimation::Walk);
                commands.entity(e).remove::<LungeDashInfo>();
            }

            kcc.translation = Some(Vec2::new(mv.0.x, mv.0.y));
        }
    }
}

pub fn handle_sprinting_cooldown(
    mut query: Query<(Entity, &mut SprintState, &PlayerAnimation), Without<Sprinting>>,
    mut commands: Commands,
) {
    for (e, mut sprint, anim) in query.iter_mut() {
        sprint.startup_timer.reset();
        sprint.sprint_duration_timer.reset();
        // Cooldown is now ticked in tick_skill_cooldowns, so we don't tick it here
        // This prevents double-ticking and ensures cooldown ticks even while sprinting
        if anim.is_sprinting() {
            commands.entity(e).insert(PlayerAnimation::Walk);
        }
    }
}

pub fn handle_lunge_cooldown(
    mut query: Query<(
        Entity,
        &mut LungeState,
        &PlayerAnimation,
        &AsepriteAnimation,
    )>,
    mut commands: Commands,
) {
    for (e, mut lunge_state, anim, aseprite_anim) in query.iter_mut() {
        if anim.is_lunging() && aseprite_anim.just_finished() {
            lunge_state.lunge_duration.reset();
            commands.entity(e).insert(PlayerAnimation::Walk);
            commands.entity(e).remove::<LungeDashInfo>();
        }
    }
}

pub fn handle_enemy_death_sprint_reset(
    mut enemy_death_events: EventReader<EnemyDeathEvent>,
    mut class_slots: Query<&mut crate::player::skills::ClassSkillSlots, With<Player>>,
    skills: Query<&PlayerSkills>,
) {
    for _ in enemy_death_events.iter() {
        let skillz = skills.single();
        if skillz.has(Heirloom::SprintKillReset) {
            if let Some(lunge_slot) = skillz.has_active_skill(ActiveSkill::SprintLunge) {
                if lunge_slot < 4 {
                    if let Ok(mut slots) = class_slots.get_single_mut() {
                        slots.0[lunge_slot]
                            .cooldown_timer
                            .tick(Duration::from_secs_f32(99.0));
                    }
                }
            }
        }
    }
}

pub fn handle_dodge_crit(dodges: EventReader<DodgeEvent>, mut game: GameParam) {
    if dodges.is_empty() {
        return;
    }
    if game.has_skill(Heirloom::DodgeCrit) {
        game.player_mut().next_hit_crit = true;
    }
}

/// Combo-counter state on the player, only present while the combo-using
/// class/skill is equipped. Stored `SparseSet` to keep the player in a single
/// archetype across combo (un)equips.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct ComboCounter {
    pub counter: u32,
    pub reset_timer: Timer,
}

/// Short-lived marker on the combo-animation entity. Cleaned up when the
/// animation finishes, so stored `SparseSet` to avoid archetype churn.
#[derive(Debug, Component)]
#[component(storage = "SparseSet")]
pub struct ComboAnim;

pub fn handle_add_combo_counter(
    mut commands: Commands,
    mut hits: EventReader<HitEvent>,
    mobs: Query<&Mob>,
    mut combo: Query<&mut ComboCounter>,
    old_combo_anims: Query<(Entity, Option<&TextureAtlasSprite>), With<ComboAnim>>,
    player: Query<(Entity, &PlayerSkills), With<Player>>,
    asset_server: Res<AssetServer>,
) {
    let (player_e, skills) = player.single();
    if !skills.has(Heirloom::DaggerCombo) {
        return;
    }

    // Only count direct attacks (no status/heirloom damage like poison)
    let mut combo_increment = 0;
    for hit in hits.iter() {
        if hit.from_heirloom_effect.is_none() && mobs.get(hit.hit_entity).is_ok() {
            combo_increment += 1;
        }
    }
    let combo_cap = (500 * skills.get_count(Heirloom::DaggerCombo).max(1)) as u32;
    for mut c in combo.iter_mut() {
        if combo_increment > 0 {
            c.counter = (c.counter + combo_increment).min(combo_cap);
            c.reset_timer.reset();
            for (e, anim) in old_combo_anims.iter() {
                if anim.is_some() {
                    commands.entity(e).despawn_recursive();
                } else {
                    commands.entity(e).insert(DoneAnimation);
                }
            }
            let text = spawn_text(
                &mut commands,
                &asset_server,
                Vec3::new(0., -1., 1.),
                BLACK,
                format!("{}", c.counter),
                Anchor::Center,
                FLOATING_TEXT,
                0,
            );
            let count = old_combo_anims.iter().count() as f32;
            commands
                .spawn(AsepriteBundle {
                    aseprite: asset_server.load(Combo::PATH),
                    animation: AsepriteAnimation::from(Combo::tags::COMBO),
                    transform: Transform::from_translation(Vec3::new(0., 20., count + 1.)),
                    ..Default::default()
                })
                .insert(VisibilityBundle::default())
                .insert(ComboAnim)
                .safe_add_child(text)
                .safe_set_parent(player_e);
        }
    }
}

pub fn tick_combo_counter(
    time: Res<Time>,
    mut combo: Query<&mut ComboCounter>,
    old_combo_anims: Query<Entity, With<ComboAnim>>,
    mut commands: Commands,
) {
    for mut c in combo.iter_mut() {
        c.reset_timer.tick(time.delta());
        if c.reset_timer.finished() {
            c.counter = 0;
            c.reset_timer.reset();
            for c in old_combo_anims.iter() {
                commands.entity(c).despawn_recursive();
            }
        }
    }
}

pub fn pause_combo_anim_when_done(mut combo: Query<&mut AsepriteAnimation, With<ComboAnim>>) {
    for mut anim in combo.iter_mut() {
        if anim.just_finished() {
            anim.pause();
        }
    }
}

// ---------------------------------------------------------------------------
// Recall / Shadow Step (Rogue) — retrace sampled positions from the last
// [`RECALL_REWIND_SECONDS`] window at a fixed world speed along the polyline.
// ---------------------------------------------------------------------------
//
// [`PositionHistory`] stores world `Vec2` samples oldest-first. On cast we
// walk **current → newest sample → … → second-oldest → snapped oldest tile
// center** so the player visually follows the path they actually took, in
// reverse. The ring buffer is not cleared on cast so chained Shadow Steps can
// reuse recent trail data. Dash duration is `polyline_length / RECALL_DASH_SPEED_PX_PER_SEC`
// (clamped). Damage uses one short-lived line collider per polyline edge.
use crate::item::projectile::RangedAttackEvent;
use crate::player::skill_heirlooms::Stealthed;
use crate::player::skills::active_skill_scaling::{
    RECALL, RECALL_DASH_DURATION_MAX_SECS, RECALL_DASH_DURATION_MIN_SECS,
    RECALL_DASH_SPEED_PX_PER_SEC, RECALL_HISTORY_CAPACITY, RECALL_HITBOX_HALF_WIDTH,
    RECALL_HITBOX_SECONDS, RECALL_LANDING_STEALTH_SECS, RECALL_MIN_SAMPLES,
    RECALL_SAMPLE_INTERVAL_SECS, RECALL_SHADOW_INTERVAL_ARC_PX,
};
use crate::player::skills::StealthState;

/// Ring buffer of recent player world positions, sampled at a fixed cadence.
///
/// Stored `SparseSet` because it's only present on the player while a skill
/// that depends on it (currently `Recall`) is equipped, so toggling it on/off
/// must not churn the player's archetype.
#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct PositionHistory {
    /// Oldest sample at index 0; pushed at the back. Capacity is bounded so
    /// the underlying `Vec` never grows past `RECALL_HISTORY_CAPACITY`.
    pub samples: Vec<Vec2>,
    /// Ticks every frame, fires every `RECALL_SAMPLE_INTERVAL_SECS` to record.
    pub sample_timer: Timer,
}

impl PositionHistory {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(RECALL_HISTORY_CAPACITY),
            sample_timer: Timer::from_seconds(RECALL_SAMPLE_INTERVAL_SECS, TimerMode::Repeating),
        }
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

pub fn tick_position_history(
    time: Res<Time>,
    mut q: Query<(&GlobalTransform, &mut PositionHistory), With<Player>>,
) {
    let Ok((tf, mut hist)) = q.get_single_mut() else {
        return;
    };
    hist.sample_timer.tick(time.delta());
    if !hist.sample_timer.just_finished() {
        return;
    }
    let pos = tf.translation().truncate();
    if hist.samples.len() >= RECALL_HISTORY_CAPACITY {
        // Cheap on a 12-element Vec; keeps the buffer at exactly capacity so
        // `samples[0]` is always the oldest in-window sample.
        hist.samples.remove(0);
    }
    hist.samples.push(pos);
}

/// Clears the recall history after any forced player teleport so we don't
/// rewind across dimension changes or scripted moves.
pub fn clear_position_history_on_move(
    mut move_events: bevy::ecs::event::EventReader<super::MovePlayerEvent>,
    mut q: Query<&mut PositionHistory, With<Player>>,
) {
    if move_events.is_empty() {
        return;
    }
    move_events.clear();
    if let Ok(mut hist) = q.get_single_mut() {
        hist.clear();
    }
}

fn polyline_length(path: &[Vec2]) -> f32 {
    path.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// Arc-length `s` (pixels) along `path`, clamped to the polyline length.
fn position_along_polyline(path: &[Vec2], mut s: f32) -> Vec2 {
    if path.is_empty() {
        return Vec2::ZERO;
    }
    if path.len() == 1 {
        return path[0];
    }
    let max_s = polyline_length(path);
    s = s.min(max_s);
    for w in path.windows(2) {
        let a = w[0];
        let b = w[1];
        let len = a.distance(b);
        if len < 1e-4 {
            continue;
        }
        if s <= len {
            let t = (s / len).min(1.0);
            return a + (b - a) * t;
        }
        s -= len;
    }
    *path.last().unwrap()
}

/// Forward tangent on the retrace polyline at arc length `s` (for shadow orientation).
fn recall_shadow_tangent(path: &[Vec2], s: f32) -> Vec2 {
    let max_s = polyline_length(path);
    let eps = 6.0_f32;
    let s0 = (s - eps).max(0.);
    let s1 = (s + eps).min(max_s);
    let p0 = position_along_polyline(path, s0);
    let p1 = position_along_polyline(path, s1);
    let d = p1 - p0;
    if d.length_squared() > 1e-4 {
        d.normalize()
    } else {
        LUNGE_SHADOW_DEFAULT_DIR
    }
}

/// Drives the player along [`RecallDashState::path`] at [`RECALL_DASH_SPEED_PX_PER_SEC`].
/// Stored `SparseSet` because it's only present during the dash.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct RecallDashState {
    /// World polyline: cast position → newest sample → … → second-oldest → snapped end.
    pub path: Vec<Vec2>,
    pub path_length: f32,
    /// Distance covered along the polyline (arc length, pixels).
    pub traveled: f32,
    /// Next arc length at which to spawn a lunge shadow (moves forward with the dash).
    pub next_shadow_arclength: f32,
    pub timer: Timer,
}

pub fn handle_recall(
    mut events: bevy::ecs::event::EventReader<ActiveSkillUsedEvent>,
    mut q: Query<
        (
            Entity,
            &GlobalTransform,
            &PlayerSkills,
            &mut PositionHistory,
            &Attack,
            &SkillPower,
            &OwnedBlessings,
        ),
        With<Player>,
    >,
    game: crate::GameParam,
    proto_param: crate::proto::proto_param::ProtoParam,
    mut ranged_attack_events: bevy::ecs::event::EventWriter<RangedAttackEvent>,
    mut commands: Commands,
) {
    let Ok((player_e, tf, skills, mut hist, atk, skill_power, blessings)) = q.get_single_mut()
    else {
        return;
    };
    let Some(slot) = skills.has_active_skill(ActiveSkill::Recall) else {
        return;
    };
    let should_activate = events.iter().any(|ev| ev.slot == slot);
    if !should_activate {
        return;
    }

    if hist.samples.len() < RECALL_MIN_SAMPLES {
        return;
    }

    let samples = hist.samples.clone();
    let Some(&oldest_raw) = samples.first() else {
        return;
    };

    let from = tf.translation().truncate();

    // Validate destination tile for the oldest sample (water/wall/tree).
    let intended_tile = crate::world::world_helpers::world_pos_to_tile_pos(oldest_raw);
    let Some(dest_tile) = crate::player::mage_skills::resolve_teleport_destination_tile(
        intended_tile,
        from,
        &game,
        &proto_param,
    ) else {
        return;
    };
    let dest = crate::world::world_helpers::tile_pos_to_world_pos(dest_tile, false)
        + Vec2::new(TILE_SIZE.x * 0.5, TILE_SIZE.y * 0.5);

    // Retrace: current → newest recorded → … → second-oldest → snapped oldest.
    let mut path: Vec<Vec2> = Vec::with_capacity(samples.len() + 1);
    path.push(from);
    for &p in samples.iter().rev().take(samples.len().saturating_sub(1)) {
        path.push(p);
    }
    path.push(dest);

    let path_length = polyline_length(&path);
    if path_length < 4.0 {
        return;
    }

    let dash_duration = (path_length / RECALL_DASH_SPEED_PX_PER_SEC)
        .clamp(RECALL_DASH_DURATION_MIN_SECS, RECALL_DASH_DURATION_MAX_SECS);

    let power_mult = skill_power_multiplier(skill_power, blessings.get_skill_power_bonus());
    let dmg = (atk.0 as f32 * power_mult * attack_damage_multiplier(RECALL)) as i32;

    // One line hitbox per edge so damage matches the actual retrace path.
    let hit_life = (dash_duration + RECALL_HITBOX_SECONDS).max(RECALL_HITBOX_SECONDS);
    for w in path.windows(2) {
        let a = w[0];
        let b = w[1];
        let d = b - a;
        let seg_len = d.length();
        if seg_len < 1.0 {
            continue;
        }
        let mid = a + d * 0.5;
        let angle = f32::atan2(d.y, d.x);
        let half_length = (seg_len * 0.5).max(RECALL_HITBOX_HALF_WIDTH);
        let hitbox = spawn_temp_collider(
            &mut commands,
            Transform::from_translation(Vec3::new(mid.x, mid.y, 0.))
                .with_rotation(Quat::from_rotation_z(angle)),
            hit_life,
            dmg,
            Collider::cuboid(half_length, RECALL_HITBOX_HALF_WIDTH),
            Projectile::Recall,
        );
    }

    commands.spawn(SoundSpawner::new(AudioSoundEffect::Teleport, 0.1));

    ranged_attack_events.send(RangedAttackEvent {
        projectile: Projectile::Smoke,
        direction: Vec2::ZERO,
        mana_cost: None,
        mana_cost_heirloom: None,
        from_enemy: false,
        from_entity: Some(player_e),
        is_followup_proj: false,
        dmg_override: Some(0),
        pos_override: Some(from),
        spawn_delay: 0.0,
    });

    commands
        .entity(player_e)
        .insert(crate::player::skills::PhasingThroughEnemies::new(
            dash_duration + 0.02,
        ));

    commands.entity(player_e).insert(RecallDashState {
        path,
        path_length,
        traveled: 0.,
        next_shadow_arclength: RECALL_SHADOW_INTERVAL_ARC_PX,
        timer: Timer::from_seconds(dash_duration, TimerMode::Once),
    });
}

/// Fixed-speed motion along [`RecallDashState::path`]. On the last tick, grants
/// unbreakable landing stealth and destination smoke (see [`handle_recall`] for
/// origin smoke).
pub fn tick_recall_dash(
    time: Res<Time>,
    asset_server: Res<AssetServer>,
    mut q: Query<
        (
            Entity,
            &mut RecallDashState,
            &mut KinematicCharacterController,
            &mut MovementVector,
        ),
        With<Player>,
    >,
    mut ranged_attack_events: bevy::ecs::event::EventWriter<RangedAttackEvent>,
    mut commands: Commands,
) {
    let Ok((player_e, mut dash, mut kcc, mut mv)) = q.get_single_mut() else {
        return;
    };

    // [`PhasingThroughEnemies`] only swaps the player to [`Group::GROUP_2`],
    // which still intersects world/static colliders that use [`Group::ALL`].
    // For Recall we need true no-clip through trees/walls for the dash line,
    // matching the pattern used on enemies (`NONE` / `NONE` on the controller).
    // `manage_ability_phasing` runs earlier in the schedule; we override here
    // every dash frame. After the dash ends we remove this state and leave
    // `NONE` for the rest of the frame; the next frame phasing restores
    // `GROUP_2` for the short tail before [`PhasingThroughEnemies`] expires.
    commands
        .entity(player_e)
        .insert(CollisionGroups::new(Group::NONE, Group::NONE));
    kcc.filter_groups = Some(CollisionGroups::new(Group::NONE, Group::NONE));

    dash.timer.tick(time.delta());

    let speed = RECALL_DASH_SPEED_PX_PER_SEC;
    let t0 = dash.traveled;
    let t1 = (t0 + speed * time.delta_seconds()).min(dash.path_length);
    let pos0 = position_along_polyline(&dash.path, t0);
    let pos1 = position_along_polyline(&dash.path, t1);
    dash.traveled = t1;

    // Trail shadows spaced by arc length as we move — avoids spawning them all at cast.
    while dash.next_shadow_arclength <= dash.traveled + 0.5 {
        let s = dash.next_shadow_arclength.min(dash.path_length);
        let p = position_along_polyline(&dash.path, s);
        let dir = recall_shadow_tangent(&dash.path, s);
        spawn_lunge_shadow(&mut commands, &asset_server, p.extend(0.9), dir);
        dash.next_shadow_arclength += RECALL_SHADOW_INTERVAL_ARC_PX;
        if dash.next_shadow_arclength > dash.path_length + RECALL_SHADOW_INTERVAL_ARC_PX {
            break;
        }
    }

    let step = pos1 - pos0;

    mv.0 = Vec2::ZERO;
    kcc.translation = Some(step);

    let finished = dash.timer.finished() || dash.traveled >= dash.path_length - 0.25;

    if finished {
        commands.entity(player_e).remove::<RecallDashState>();

        commands
            .entity(player_e)
            .insert(StealthState {
                duration: Timer::from_seconds(RECALL_LANDING_STEALTH_SECS, TimerMode::Once),
                unbreakable: true,
            })
            .insert(Stealthed);

        ranged_attack_events.send(RangedAttackEvent {
            projectile: Projectile::Smoke,
            direction: Vec2::ZERO,
            mana_cost: None,
            mana_cost_heirloom: None,
            from_enemy: false,
            from_entity: Some(player_e),
            is_followup_proj: false,
            dmg_override: Some(0),
            pos_override: None,
            spawn_delay: 0.0,
        });
    }
}
