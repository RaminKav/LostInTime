use std::{f32::consts::PI, time::Duration};

use crate::{
    animations::{player_sprite::PlayerAnimation, AttackEvent, DoneAnimation},
    attributes::{attribute_helpers::skill_power_multiplier, Attack, CurrentMana, SkillPower},
    audio::{AudioSoundEffect, SoundSpawner},
    blessings::OwnedBlessings,
    colors::BLACK,
    combat_helpers::spawn_temp_collider,
    cursor::CursorPos,
    enemy::Mob,
    inputs::{FacingDirection, MovementVector},
    item::projectile::Projectile,
    ui::damage_numbers::{spawn_text, DodgeEvent},
    world::TILE_SIZE,
    AttackTimer, EnemyDeathEvent, GameParam, HitEvent, InputMappings,
};
use bevy::{prelude::*, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group, KinematicCharacterController};

use super::{ActiveSkill, ActiveSkillUsedEvent, Heirloom, Player, PlayerSkills};

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
    )>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    projectile_size: Query<&crate::attributes::ProjectileSize, With<Player>>,
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
                (dmg.0 as f32 * skill_power_mult * 0.85) as i32,
                Collider::cuboid(9., 1.5 * TILE_SIZE.x),
                Projectile::None,
            );
            commands.entity(lunge_e).set_parent(e);

            if skills.has(Heirloom::SkillEcho) {
                let mana_cost = Heirloom::SkillEcho.get_mana_cost();
                if current_mana.0 >= mana_cost {
                    current_mana.0 -= mana_cost;

                    let echo_dmg = (dmg.0 as f32 * 1.) as i32;
                    let size_mult = projectile_size
                        .get_single()
                        .map(|s| s.get_multiplier())
                        .unwrap_or(1.0);
                    crate::player::melee_skills::spawn_echo_hitbox(
                        &mut commands,
                        &asset_server,
                        e,
                        echo_dmg,
                        size_mult,
                    );
                }
            }

            lunge_state.lunge_duration.tick(time.delta());
            mv.0 = mv.0 * 0.;
        } else if lunge_state.lunge_duration.percent() != 0. {
            lunge_state.lunge_duration.tick(time.delta());
            if lunge_state.lunge_duration.percent() >= 0.20
                && lunge_state.lunge_duration.percent() <= 0.45
            {
                commands
                    .entity(e)
                    .insert(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
                kcc.filter_groups = Some(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
                mv.0 = mv.0 * lunge_state.lunge_speed;
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
                1.,
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
                .add_child(text)
                .set_parent(player_e);
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
