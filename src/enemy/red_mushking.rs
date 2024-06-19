use crate::{
    custom_commands::CommandsExt, enemy::spawn_helpers::can_spawn_mob_here, item::LootTable,
    player::levels::ExperienceReward, GameParam,
};
use bevy::prelude::*;
use bevy_proto::prelude::ProtoCommands;
use bevy_rapier2d::{
    control::KinematicCharacterController,
    geometry::{Collider, Sensor},
};
use rand::Rng;
use seldom_state::{prelude::StateMachine, trigger::BoolTrigger};

use crate::{
    ai::{EnemyAttackCooldown, FollowState, LeapAttackState},
    attributes::{Attack, CurrentHealth, MaxHealth},
    collisions::DamagesWorldObjects,
    proto::proto_param::ProtoParam,
    Game, PLAYER_MOVE_SPEED,
};
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
    game: Res<Game>,
) {
    for (e, mob, transform, follow_speed, leap_attack) in spawn_events.iter() {
        if mob != &Mob::RedMushking {
            continue;
        }
        let mut e_cmds = commands.entity(e);
        let mut animation = AsepriteAnimation::from(RedMushking::tags::IDLE);
        animation.play();
        e_cmds
            .insert(AsepriteBundle {
                aseprite: asset_server.load(RedMushking::PATH),
                animation,
                transform: *transform,
                ..Default::default()
            })
            .insert(FollowState {
                target: game.player,
                speed: follow_speed.0,
            })
            .insert(DamagesWorldObjects)
            .insert(HealthThreshold(1.))
            .insert(AttackCollider(None));
        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .trans::<FollowState>(
                HealthTrigger(0.65),
                SummonAttackState {
                    num_summons_left: 8,
                    timer: Timer::from_seconds(0.2, TimerMode::Repeating),
                },
            )
            .trans::<FollowState>(
                JumpTimer,
                LeapAttackState {
                    target: game.player,
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
            );

        e_cmds.insert(state_machine);
    }
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct JumpAttackState;

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct SummonAttackState {
    num_summons_left: usize,
    timer: Timer,
}

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
        if frame < 14 || frame > 28 {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::ATTACK_HOP);
        } else if frame == 23 {
            // BEGIN DMGING
            commands.entity(entity).insert(MobIsAttacking);
            if att_collider.0.is_none() {
                let hitbox = commands
                    .spawn((
                        TransformBundle::default(),
                        attack.clone(),
                        leap_attack.clone(),
                        Collider::capsule(Vec2::new(-17., -15.), Vec2::new(17., -15.), 20.),
                        MobIsAttacking,
                        Sensor,
                        DamagesWorldObjects,
                    ))
                    .set_parent(entity)
                    .id();
                att_collider.0 = Some(hitbox);
            }
        }
        // Get the positions of the attacker and target
        let target_translation =
            transforms.get(leap_attack.target).unwrap().translation + Vec3::new(0., 18., 0.);
        let attack_transform = transforms.get_mut(entity).unwrap();
        let attack_translation = attack_transform.translation;
        if frame == 16 {
            if leap_attack.dir.is_none() {
                let delta = (target_translation - attack_translation).clamp(
                    Vec3::splat(-MAX_JUMP_DISTANCE),
                    Vec3::splat(MAX_JUMP_DISTANCE),
                );
                leap_attack.dir = Some(delta.truncate());
            }
        }

        // BEGIN MOVING
        if frame >= 18 && frame <= 23 {
            // println!("      begin moveing {:?}", time.delta_seconds());
            kcc.translation = Some((leap_attack.dir.unwrap() * time.delta_seconds()) * 10. / 6.);
        }
        // END LEAP ATTACK
        if frame == 28 {
            // println!("              end leap attack");
            commands
                .entity(entity)
                .insert(FollowState {
                    target: leap_attack.target,
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
        if frame < 29 || frame > 42 {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::START_SUMMON);
        } else if frame == 33 {
            *anim_state = AsepriteAnimation::from(RedMushking::tags::SUMMONING);
        } else if frame >= 33 && frame <= 39 {
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

pub fn new_follow(
    mut transforms: Query<&mut Transform>,
    mut follows: Query<(
        Entity,
        &FollowState,
        Option<&EnemyAttackCooldown>,
        &mut AsepriteAnimation,
        &mut KinematicCharacterController,
    )>,
    time: Res<Time>,
) {
    for (entity, follow, att_cooldown, mut anim, mut mover) in follows.iter_mut() {
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

        if anim.current_frame() < 6 || anim.current_frame() > 13 {
            *anim = AsepriteAnimation::from(RedMushking::tags::WALK);
        }
    }
}

#[derive(Clone, Copy, Reflect)]
pub struct JumpTimer;

impl BoolTrigger for JumpTimer {
    type Param<'w, 's> = (Query<'w, 's, &'static EnemyAttackCooldown>, Res<'w, Time>);

    fn trigger(&self, entity: Entity, (attack_cooldown, time): Self::Param<'_, '_>) -> bool {
        if let Ok(_) = attack_cooldown.get(entity) {
            return false;
        }
        return true;
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
