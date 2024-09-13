use std::time::Duration;

use crate::{
    animations::{player_sprite::PlayerAnimation, AttackEvent},
    inputs::{CursorPos, MovementVector},
    ui::damage_numbers::DodgeEvent,
    AttackTimer, EnemyDeathEvent, GameParam,
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::{CollisionGroups, Group, KinematicCharacterController};

use super::{PlayerSkills, Skill};
#[derive(Debug, Component)]
pub struct SprintState {
    pub startup_timer: Timer,
    pub sprint_duration_timer: Timer,
    pub sprint_cooldown_timer: Timer,
    pub lunge_duration: Timer,
    pub speed_bonus: f32,
    pub lunge_speed: f32,
}

#[derive(Debug, Component)]
pub struct Sprinting;
pub fn handle_toggle_sprinting(
    mut sprint_query: Query<(Entity, &mut SprintState), With<SprintState>>,
    key_inputs: Res<Input<KeyCode>>,
    mut commands: Commands,
) {
    for (e, sprint_state) in sprint_query.iter_mut() {
        if key_inputs.just_pressed(KeyCode::Space) && sprint_state.sprint_cooldown_timer.finished()
        {
            commands.entity(e).insert(Sprinting);
        } else if key_inputs.just_released(KeyCode::Space) {
            commands.entity(e).remove::<Sprinting>();
        }
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
    key_inputs: Res<Input<KeyCode>>,
    mut commands: Commands,
) {
    for (e, mut sprint, mut kcc, mut mv, anim, skills, attack_cooldown_option) in query.iter_mut() {
        if !sprint.startup_timer.finished() {
            sprint.startup_timer.tick(time.delta());
            sprint.sprint_cooldown_timer.reset();
        } else {
            if anim != &PlayerAnimation::Run && !anim.is_one_time_anim() {
                commands.entity(e).insert(PlayerAnimation::Run);
            }
            let speed_bonus_skill = skills.has(Skill::SprintFaster);
            let lunge_skill = skills.has(Skill::SprintLunge);
            mv.0 = mv.0 * (sprint.speed_bonus + if speed_bonus_skill { 0.2 } else { 0. });
            let player_pos = game.player().position;

            let direction =
                (cursor_pos.world_coords.truncate() - player_pos.truncate()).normalize_or_zero();
            if mouse_inputs.pressed(MouseButton::Left)
                && !anim.is_an_attack()
                && attack_cooldown_option.is_none()
            {
                commands.entity(e).insert(PlayerAnimation::RunAttack2);
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: false,
                });
            }
            if lunge_skill
                && (mouse_inputs.pressed(MouseButton::Right) || key_inputs.pressed(KeyCode::Slash))
                && sprint.sprint_cooldown_timer.percent() == 0.
            {
                // LUNGE
                commands.entity(e).insert(PlayerAnimation::Lunge);
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: true,
                });

                sprint.lunge_duration.tick(time.delta());
                sprint.sprint_cooldown_timer.tick(time.delta());
                mv.0 = mv.0 * 0.;
            } else if sprint.lunge_duration.percent() != 0. {
                sprint.lunge_duration.tick(time.delta());
                if sprint.lunge_duration.percent() >= 0.20
                    && sprint.lunge_duration.percent() <= 0.45
                {
                    commands
                        .entity(e)
                        .insert(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
                    kcc.filter_groups = Some(CollisionGroups::new(Group::GROUP_2, Group::GROUP_2));
                    mv.0 = mv.0 * sprint.lunge_speed;
                } else {
                    commands
                        .entity(e)
                        .insert(CollisionGroups::new(Group::ALL, Group::ALL));
                    kcc.filter_groups = Some(CollisionGroups::new(Group::ALL, Group::ALL));

                    mv.0 = mv.0 * 0.;
                }
            }

            if sprint.lunge_duration.finished() {
                sprint.lunge_duration.reset();
                commands.entity(e).remove::<Sprinting>();
                commands.entity(e).insert(PlayerAnimation::Walk);
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

pub fn handle_sprinting_cooldown(
    time: Res<Time>,
    mut query: Query<(Entity, &mut SprintState, &PlayerAnimation), Without<Sprinting>>,
    mut commands: Commands,
) {
    for (e, mut sprint, anim) in query.iter_mut() {
        sprint.startup_timer.reset();
        sprint.sprint_duration_timer.reset();
        sprint.lunge_duration.reset();
        sprint.sprint_cooldown_timer.tick(time.delta());
        if anim.is_sprinting() || anim.is_lunging() {
            commands.entity(e).insert(PlayerAnimation::Walk);
        }
    }
}

pub fn handle_enemy_death_sprint_reset(
    enemy_death_events: EventReader<EnemyDeathEvent>,
    mut sprint_query: Query<&mut SprintState>,
    skills: Query<&PlayerSkills>,
) {
    if !enemy_death_events.is_empty() {
        if skills.single().has(Skill::SprintKillReset) {
            for mut sprint in sprint_query.iter_mut() {
                sprint.sprint_cooldown_timer.tick(Duration::from_secs(99));
            }
        }
    }
}

pub fn handle_dodge_crit(dodges: EventReader<DodgeEvent>, mut game: GameParam) {
    if dodges.is_empty() {
        return;
    }
    if game.has_skill(Skill::DodgeCrit) {
        game.player_mut().next_hit_crit = true;
    }
}
