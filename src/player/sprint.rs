use std::time::Duration;

use crate::{
    animations::{enemy_sprites::EnemyAnimationState, AttackEvent},
    inputs::{CursorPos, MovementVector},
    EnemyDeathEvent, GameParam,
};
use bevy::prelude::*;
use bevy_rapier2d::prelude::KinematicCharacterController;
#[derive(Debug, Component)]
pub struct SprintUpgrade {
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
    mut sprint_query: Query<(Entity, &mut SprintUpgrade), With<SprintUpgrade>>,
    key_inputs: Res<Input<KeyCode>>,
    mut commands: Commands,
) {
    for (e, mut sprint_state) in sprint_query.iter_mut() {
        if key_inputs.just_pressed(KeyCode::Space) && sprint_state.sprint_cooldown_timer.finished()
        {
            commands.entity(e).insert(Sprinting);
            sprint_state.sprint_cooldown_timer.reset();
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
            &mut SprintUpgrade,
            &mut KinematicCharacterController,
            &mut MovementVector,
        ),
        With<Sprinting>,
    >,
    mouse_inputs: Res<Input<MouseButton>>,
    mut game: GameParam,
    mut attack_event: EventWriter<AttackEvent>,
    cursor_pos: Res<CursorPos>,
    key_inputs: Res<Input<KeyCode>>,
    mut commands: Commands,
) {
    for (e, mut sprint, mut kcc, mut mv) in query.iter_mut() {
        if !sprint.startup_timer.finished() {
            sprint.startup_timer.tick(time.delta());
        } else {
            let player_state = game.player_mut();
            player_state.is_sprinting = true;
            mv.0 = mv.0 * sprint.speed_bonus;

            if (mouse_inputs.just_pressed(MouseButton::Right)
                || key_inputs.just_pressed(KeyCode::Slash))
                && sprint.sprint_cooldown_timer.percent() == 0.
            {
                // LUNGE
                let player_pos = game.player().position;

                let direction = (cursor_pos.world_coords.truncate() - player_pos.truncate())
                    .normalize_or_zero();
                commands.entity(e).insert(EnemyAnimationState::Attack);
                attack_event.send(AttackEvent {
                    direction,
                    ignore_cooldown: true,
                });

                sprint.lunge_duration.tick(time.delta());
                sprint.sprint_cooldown_timer.tick(time.delta());
                mv.0 = mv.0 * 0.;
                game.player_mut().is_lunging = true;
            } else if sprint.lunge_duration.percent() != 0. {
                sprint.lunge_duration.tick(time.delta());
                if sprint.lunge_duration.percent() >= 0.20
                    && sprint.lunge_duration.percent() <= 0.45
                {
                    mv.0 = mv.0 * sprint.lunge_speed;
                } else {
                    mv.0 = mv.0 * 0.;
                }
            }

            if sprint.lunge_duration.finished() {
                sprint.lunge_duration.reset();
                commands.entity(e).remove::<Sprinting>();
                game.player_mut().is_lunging = false;
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
    mut query: Query<&mut SprintUpgrade, Without<Sprinting>>,
    mut game: GameParam,
) {
    for mut sprint in query.iter_mut() {
        sprint.startup_timer.reset();
        sprint.sprint_duration_timer.reset();
        sprint.lunge_duration.reset();
        sprint.sprint_cooldown_timer.tick(time.delta());
        let player_state = game.player_mut();
        player_state.is_sprinting = false;
        player_state.is_lunging = false;
    }
}

pub fn handle_enemy_death_sprint_reset(
    enemy_death_events: EventReader<EnemyDeathEvent>,
    mut sprint_query: Query<&mut SprintUpgrade>,
) {
    if !enemy_death_events.is_empty() {
        for mut sprint in sprint_query.iter_mut() {
            sprint.sprint_cooldown_timer.tick(Duration::from_secs(99));
        }
    }
}
