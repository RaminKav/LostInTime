use crate::{
    assets::Graphics,
    collisions::DamagesWorldObjects,
    combat::combat_helpers::{spawn_deferred_aseprite_collider, DeferredComponent},
    enemy::{FollowSpeed, Mob},
    item::projectile::Projectile,
    player::Player,
    GameParam,
};
use bevy::prelude::*;
use bevy::sprite::{ColorMaterial, MaterialMesh2dBundle};
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, Aseprite, AsepriteBundle};
use bevy_rapier2d::prelude::{Collider, CollisionGroups, Group};
use rand::Rng;
use seldom_state::{
    prelude::StateMachine,
    trigger::{BoolTrigger, Trigger},
};

use super::{
    red_mushking::{BossAttackPreview, DeathState},
    FollowState,
};

aseprite!(pub StoneGolem, "textures/stonegolem/StoneGolem.ase");
aseprite!(pub StonePillar, "textures/stonegolem/StonePillar.ase");

// Constants
const SPIKE_ATTACK_COUNT: usize = 5;
const SPIKE_ATTACK_INTERVAL: f32 = 0.15;
const SPIKE_WARNING_DELAY: f32 = 0.65; // Time between warning and damage
const SPIKE_DAMAGE: i32 = 20;
const SPIKE_DURATION: f32 = 10.0; // How long the spike hitbox lasts

#[derive(Component)]
pub struct SpikeAttackTimer {
    pub random_timer: Timer,
}

#[derive(Clone, Component, Reflect)]
#[component(storage = "SparseSet")]
pub struct SpikeAttackState {
    pub num_spikes_left: usize,
    pub attack_timer: Timer, // Controls interval between spikes
    pub current_target_pos: Option<Vec2>,
    pub preview_entity: Option<Entity>,
    pub active_spike_delay_timer: Option<Timer>, // For the currently spawning spike delay
}

#[derive(Clone, Copy, Reflect)]
pub struct SpikeAttackTimerTrigger;

impl BoolTrigger for SpikeAttackTimerTrigger {
    type Param<'w, 's> = Query<'w, 's, &'static SpikeAttackTimer>;

    fn trigger(&self, entity: Entity, query: Self::Param<'_, '_>) -> bool {
        if let Ok(timer) = query.get(entity) {
            if timer.random_timer.finished() {
                // info!("SpikeAttackTimerTrigger triggered for entity {:?}", entity);
                return true;
            }
        }
        false
    }
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

        // Initial animation - assume WalkFront is default or use IDLE if available
        let mut animation = AsepriteAnimation::from("WalkFront");
        animation.play();

        let mut rng = rand::thread_rng();
        let initial_timer = rng.gen_range(3.0..5.0);
        info!("NEW STONE GOLEM @@@@@@@");
        e_cmds
            .insert(AsepriteBundle {
                aseprite: asset_server.load(StoneGolem::PATH),
                animation,
                transform: *transform,
                ..Default::default()
            })
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
            });

        let state_machine = StateMachine::default()
            .set_trans_logging(false)
            .with_state::<DeathState>() // Add DeathState so state machine is always valid
            .trans::<FollowState>(
                SpikeAttackTimerTrigger,
                SpikeAttackState {
                    num_spikes_left: SPIKE_ATTACK_COUNT,
                    attack_timer: Timer::from_seconds(0.0, TimerMode::Once), // Start immediately
                    current_target_pos: None,
                    preview_entity: None,
                    active_spike_delay_timer: None,
                },
            )
            .trans::<SpikeAttackState>(
                Trigger::not(SpikeAttackTimerTrigger), // Placeholder, logic handles return
                FollowState {
                    target: game.game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: follow_speed.0,
                },
            );

        // Note: We handle transition back to FollowState manually in handle_spike_attack
        // The above transition is just to satisfy type checking if needed,
        // but actually we will remove SpikeAttackState and insert FollowState manually.

        e_cmds.insert(state_machine);
    }
}

pub fn tick_spike_attack_timer(
    mut timers: Query<&mut SpikeAttackTimer, Without<crate::combat::MarkedForDeath>>,
    time: Res<Time>,
) {
    for mut timer in timers.iter_mut() {
        timer.random_timer.tick(time.delta());
    }
}

/// Helper function to spawn a golem spike hitbox with animation
pub fn spawn_golem_spike_hitbox(
    commands: &mut Commands,
    stone_pillar_asset: Handle<Aseprite>,
    pos: Vec3,
    dmg: i32,
    golem_entity: Entity,
) {
    // Create default animation
    let anim = AsepriteAnimation::default();

    // Spawn at a higher Z to ensure visibility
    let transform = Transform::from_translation(pos + Vec3::new(0., 32., 995.0));

    // Queue deferred spawn - actual entity will be created in PreUpdate
    spawn_deferred_aseprite_collider(
        commands,
        transform,
        SPIKE_DURATION,
        dmg,
        Collider::capsule(Vec2::new(0., -32.), Vec2::new(0., -32.), 16.),
        stone_pillar_asset,
        anim,
        false,
        Projectile::GolemSpike,
        vec![DeferredComponent::EnemyProjectile {
            entity: golem_entity,
            mob: Mob::StoneGolem,
        }],
        None, // No parent
    );
}

pub fn handle_spike_attack(
    mut commands: Commands,
    mut attacks: Query<
        (Entity, &mut SpikeAttackState, &mut AsepriteAnimation),
        Without<crate::combat::MarkedForDeath>,
    >,
    mut timers: Query<&mut SpikeAttackTimer>,
    player_query: Query<&Transform, With<Player>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    graphics: Res<Graphics>,
    time: Res<Time>,
    game: GameParam,
    follow_speed_query: Query<&FollowSpeed>,
) {
    for (entity, mut state, mut anim) in attacks.iter_mut() {
        // Set animation to Attack
        let frame = anim.current_frame();
        // Assuming Attack frames are 30-44.
        let is_attacking = (30..=44).contains(&frame);
        if !is_attacking {
            *anim = AsepriteAnimation::from("Attack");
            if anim.is_paused() {
                anim.play();
            }
        }

        state.attack_timer.tick(time.delta());

        if state.active_spike_delay_timer.is_none()
            && state.num_spikes_left > 0
            && state.attack_timer.finished()
        {
            // Start new spike sequence
            if let Ok(player_txfm) = player_query.get_single() {
                info!(
                    "StoneGolem starting spike sequence. Spikes left: {}",
                    state.num_spikes_left
                );
                let target_pos = player_txfm.translation.truncate();
                state.current_target_pos = Some(target_pos);

                // Spawn Warning
                let preview_entity = commands
                    .spawn((
                        MaterialMesh2dBundle {
                            mesh: meshes
                                .add(bevy::prelude::shape::Circle::new(16.).into())
                                .into(),
                            material: materials
                                .add(ColorMaterial::from(Color::rgba(1.0, 0.0, 0.0, 0.3))),
                            transform: Transform {
                                translation: target_pos.extend(990.0),
                                ..default()
                            },
                            ..default()
                        },
                        BossAttackPreview,
                    ))
                    .id();
                state.preview_entity = Some(preview_entity);

                // Set delay timer for actual damage
                state.active_spike_delay_timer =
                    Some(Timer::from_seconds(SPIKE_WARNING_DELAY, TimerMode::Once));
                info!(
                    "StoneGolem preparing spike attack. Spikes left: {}",
                    state.num_spikes_left
                );
            }
        }

        // Handle active spike delay
        if let Some(timer) = &mut state.active_spike_delay_timer {
            timer.tick(time.delta());
            if timer.finished() {
                // Spawn Spike Hitbox
                if let Some(target_pos) = state.current_target_pos {
                    info!("StoneGolem spawning spike at {:?}", target_pos);
                    spawn_golem_spike_hitbox(
                        &mut commands,
                        graphics.stone_pillar_ase.as_ref().unwrap().clone(),
                        target_pos.extend(0.0),
                        SPIKE_DAMAGE,
                        entity,
                    );
                }

                // Cleanup Preview
                if let Some(preview_entity) = state.preview_entity {
                    commands.entity(preview_entity).despawn_recursive();
                    state.preview_entity = None;
                }

                state.num_spikes_left -= 1;
                state.active_spike_delay_timer = None;

                // Reset attack timer for next spike
                state.attack_timer = Timer::from_seconds(SPIKE_ATTACK_INTERVAL, TimerMode::Once);
            }
        }

        // Check completion
        if state.num_spikes_left == 0 && state.active_spike_delay_timer.is_none() {
            info!("StoneGolem spike attack complete. Returning to Follow.");
            // Transition back to Follow
            let follow_speed = follow_speed_query.get(entity).map(|f| f.0).unwrap_or(0.65);

            // Safety check: ensure entity still exists before modifying state machine
            if let Some(mut entity_commands) = commands.get_entity(entity) {
                entity_commands
                    .remove::<SpikeAttackState>()
                    .insert(FollowState {
                        target: game.game.player,
                        curr_delta: None,
                        curr_path: None,
                        speed: follow_speed,
                    });
            }

            // Reset Timer
            if let Ok(mut timer) = timers.get_mut(entity) {
                let mut rng = rand::thread_rng();
                timer.random_timer = Timer::from_seconds(rng.gen_range(3.0..5.0), TimerMode::Once);
            }

            // Reset Animation to Walk
            *anim = AsepriteAnimation::from("WalkFront");
        }
    }
}

pub fn update_stone_golem_walk_animation(
    mut query: Query<
        (Entity, &mut Transform, &mut AsepriteAnimation, &Mob),
        (With<FollowState>, Without<crate::combat::MarkedForDeath>),
    >,
    player_query: Query<&Transform, (With<Player>, Without<FollowState>)>,
) {
    if let Ok(player_transform) = player_query.get_single() {
        let player_pos = player_transform.translation.truncate();

        for (_entity, mut transform, mut anim, mob) in query.iter_mut() {
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

            // Determine desired animation based on direction with hysteresis
            let current_frame = anim.current_frame();
            let is_walk_front = (10..=29).contains(&current_frame);
            let is_walk_back = (45..=61).contains(&current_frame);
            let is_walk_side = (62..=77).contains(&current_frame);

            // 10% buffer to prevent rapid switching between Side and Vertical
            let desired_anim = if abs_x > abs_y * 1.1 {
                "WalkSide"
            } else if abs_y > abs_x * 1.1 {
                if delta.y > 0. {
                    "WalkBack"
                } else {
                    "WalkFront"
                }
            } else {
                // In the buffer zone, prefer keeping current valid animation
                if is_walk_side {
                    "WalkSide"
                } else if is_walk_back {
                    "WalkBack"
                } else if is_walk_front {
                    "WalkFront"
                } else {
                    // Fallback if current animation is not a walk (e.g. just finished attack)
                    if abs_x > abs_y {
                        "WalkSide"
                    } else if delta.y > 0. {
                        "WalkBack"
                    } else {
                        "WalkFront"
                    }
                }
            };

            // Apply change if needed
            let needs_change = match desired_anim {
                "WalkFront" => !is_walk_front,
                "WalkBack" => !is_walk_back,
                "WalkSide" => !is_walk_side,
                _ => false,
            };

            if needs_change {
                *anim = AsepriteAnimation::from(desired_anim);
                if anim.is_paused() {
                    anim.play();
                }
            }

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
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}
