use std::{f32::consts::PI, time::Duration};

use crate::{
    animations::{player_sprite::PlayerAnimation, AttackEvent, DoneAnimation},
    attributes::Attack,
    colors::BLACK,
    combat_helpers::spawn_temp_collider,
    enemy::Mob,
    get_active_skill_keybind,
    inputs::{CursorPos, FacingDirection, MovementVector},
    ui::damage_numbers::{spawn_text, DodgeEvent},
    world::TILE_SIZE,
    AttackTimer, EnemyDeathEvent, GameParam, HitEvent,
};
use bevy::{prelude::*, sprite::Anchor};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group, KinematicCharacterController};

use super::{ActiveSkillUsedEvent, Player, PlayerSkills, Skill};

aseprite!(pub Combo, "textures/effects/Combo.aseprite");

#[derive(Debug, Component)]
pub struct SprintState {
    pub startup_timer: Timer,
    pub sprint_duration_timer: Timer,
    pub sprint_cooldown_timer: Timer,
    pub speed_bonus: f32,
}
#[derive(Debug, Component)]
pub struct LungeState {
    pub lunge_cooldown_timer: Timer,
    pub lunge_duration: Timer,
    pub lunge_speed: f32,
}

#[derive(Debug, Component)]
pub struct Sprinting;
pub fn handle_toggle_sprinting(
    mut sprint_query: Query<
        (Entity, &mut SprintState, &PlayerSkills, Option<&Sprinting>),
        With<SprintState>,
    >,
    key_inputs: Res<Input<KeyCode>>,
    mut commands: Commands,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
) {
    for (e, sprint_state, skills, was_sprinting) in sprint_query.iter_mut() {
        if let Some(sprint_slot) = skills.has_active_skill(Skill::Sprint) {
            if key_inputs.just_pressed(get_active_skill_keybind(sprint_slot))
                && sprint_state.sprint_cooldown_timer.finished()
            {
                commands.entity(e).insert(Sprinting);
            } else if key_inputs.just_released(get_active_skill_keybind(sprint_slot)) {
                if was_sprinting.is_some() {
                    active_skill_event.send(ActiveSkillUsedEvent {
                        slot: sprint_slot,
                        cooldown: sprint_state.sprint_cooldown_timer.duration().as_secs_f32(),
                    });
                    commands.entity(e).remove::<Sprinting>();
                }
            }
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
    mut commands: Commands,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
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

            if sprint
                .sprint_duration_timer
                .tick(time.delta())
                .just_finished()
            {
                active_skill_event.send(ActiveSkillUsedEvent {
                    slot: skills.has_active_skill(Skill::Sprint).unwrap(),
                    cooldown: sprint.sprint_cooldown_timer.duration().as_secs_f32(),
                });
                commands.entity(e).remove::<Sprinting>();
            }

            kcc.translation = Some(Vec2::new(mv.0.x, mv.0.y));
        }
    }
}
pub fn handle_lunge(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut LungeState,
        &mut KinematicCharacterController,
        &mut MovementVector,
        &PlayerSkills,
        &FacingDirection,
        &Attack,
    )>,
    key_inputs: Res<Input<KeyCode>>,
    mut commands: Commands,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
) {
    for (e, mut lunge_state, mut kcc, mut mv, skills, dir, dmg) in query.iter_mut() {
        if let Some(lunge_slot) = skills.has_active_skill(Skill::SprintLunge) {
            if key_inputs.just_pressed(get_active_skill_keybind(lunge_slot))
                && lunge_state.lunge_cooldown_timer.finished()
            {
                lunge_state.lunge_cooldown_timer.reset();
                active_skill_event.send(ActiveSkillUsedEvent {
                    slot: lunge_slot,
                    cooldown: lunge_state.lunge_cooldown_timer.duration().as_secs_f32(),
                });

                // LUNGE
                commands.entity(e).insert(PlayerAnimation::Lunge);
                let angle = match dir {
                    FacingDirection::Up => 0.,
                    FacingDirection::Down => 0.,
                    FacingDirection::Left => PI / 2.,
                    FacingDirection::Right => PI / 2.,
                };
                let lunge_e = spawn_temp_collider(
                    &mut commands,
                    Transform::from_translation(Vec3::new(0., 0., 0.))
                        .with_rotation(Quat::from_rotation_z(angle)),
                    0.5,
                    dmg.0,
                    Collider::cuboid(9., 1.5 * TILE_SIZE.x),
                );
                commands.entity(lunge_e).set_parent(e);

                lunge_state.lunge_duration.tick(time.delta());
                lunge_state.lunge_cooldown_timer.tick(time.delta());
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
                } else {
                    commands
                        .entity(e)
                        .insert(CollisionGroups::new(Group::ALL, Group::ALL));
                    kcc.filter_groups = Some(CollisionGroups::new(Group::ALL, Group::ALL));

                    mv.0 = mv.0 * 0.;
                }

                if lunge_state.lunge_duration.finished() {
                    lunge_state.lunge_duration.reset();
                    commands.entity(e).insert(PlayerAnimation::Walk);
                }

                kcc.translation = Some(Vec2::new(mv.0.x, mv.0.y));
            }
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
        sprint.sprint_cooldown_timer.tick(time.delta());
        if anim.is_sprinting() {
            commands.entity(e).insert(PlayerAnimation::Walk);
        }
    }
}

pub fn handle_lunge_cooldown(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut LungeState,
        &PlayerAnimation,
        &AsepriteAnimation,
    )>,
    mut commands: Commands,
) {
    for (e, mut lunge_state, anim, aseprite_anim) in query.iter_mut() {
        lunge_state.lunge_cooldown_timer.tick(time.delta());
        if anim.is_lunging() && aseprite_anim.just_finished() {
            lunge_state.lunge_duration.reset();
            commands.entity(e).insert(PlayerAnimation::Walk);
        }
    }
}

pub fn handle_enemy_death_sprint_reset(
    mut enemy_death_events: EventReader<EnemyDeathEvent>,
    mut lunge_query: Query<&mut LungeState>,
    skills: Query<&PlayerSkills>,
    mut active_skill_event: EventWriter<ActiveSkillUsedEvent>,
) {
    for _ in enemy_death_events.iter() {
        if skills.single().has(Skill::SprintKillReset) {
            if let Some(lunge_slot) = skills.single().has_active_skill(Skill::SprintLunge) {
                for mut sprint in lunge_query.iter_mut() {
                    active_skill_event.send(ActiveSkillUsedEvent {
                        slot: lunge_slot,
                        cooldown: 0.,
                    });
                    sprint.lunge_cooldown_timer.tick(Duration::from_secs(99));
                }
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

#[derive(Debug, Component)]
pub struct ComboCounter {
    pub counter: u32,
    pub reset_timer: Timer,
}

#[derive(Debug, Component)]
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
    if !skills.has(Skill::DaggerCombo) {
        return;
    }

    let mut combo_increment = 0;
    for hit in hits.iter() {
        if mobs.get(hit.hit_entity).is_ok() {
            combo_increment += 1;
        }
    }
    for mut c in combo.iter_mut() {
        if combo_increment > 0 {
            c.counter += combo_increment;
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
