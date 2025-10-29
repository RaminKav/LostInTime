use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};
use rand::Rng;
use seldom_state::{prelude::StateMachine, trigger::Trigger};

use crate::{
    ai::LineOfSight, enemy::Mob, inputs::FacingDirection, player::Player, Game, Pet, PetState,
};

aseprite!(pub SlimePetSprite, "textures/pets/slime-pet.ase");
aseprite!(pub FairyPetSprite, "textures/pets/fairy-pet.ase");

pub mod tags {
    pub const IDLE: &str = "IDLE";
    pub const WALK: &str = "WALK";
    pub const ATTACK: &str = "ATTACK";
}

#[derive(Component, Clone, Reflect)]
#[component(storage = "SparseSet")]
pub struct PetIdleState {
    pub walk_timer: Timer,
    pub locked_in_idle_timer: Timer,
    pub direction: FacingDirection,
    pub speed: f32,
    pub is_stopped: bool,
}

#[derive(Component, Clone, Reflect)]
#[component(storage = "SparseSet")]
pub struct PetFollowState {
    pub target: Entity,
    pub curr_delta: Option<Vec2>,
    pub curr_path: Option<Vec<Vec2>>,
    pub speed: f32,
}

#[derive(Component, Clone, Reflect)]
#[component(storage = "SparseSet")]
pub struct PetAttackState {
    pub target: Entity,
    pub attack_timer: Timer,
    pub cooldown_timer: Timer,
}

impl Default for PetIdleState {
    fn default() -> Self {
        Self {
            walk_timer: Timer::from_seconds(2., TimerMode::Repeating),
            locked_in_idle_timer: Timer::from_seconds(1., TimerMode::Once),
            direction: FacingDirection::new_rand_dir(rand::thread_rng()),
            speed: 15.,
            is_stopped: false,
        }
    }
}

pub fn handle_new_pet_state_machine(
    mut commands: Commands,
    spawn_events: Query<(Entity, &Transform, &PetState, &Pet), Added<crate::pets::state::PetState>>,
    asset_server: Res<AssetServer>,
    game: Res<Game>,
) {
    for (e, transform, pet_state, pet) in spawn_events.iter() {
        let mut e_cmds = commands.entity(e);

        // Use Aseprite animation system
        let mut animation = AsepriteAnimation::from(pet.get_idle_anim());
        animation.pause();
        e_cmds
            .insert(AsepriteBundle {
                aseprite: asset_server.load(pet.get_aseprite_path()),
                animation,
                transform: *transform,
                ..Default::default()
            })
            .insert(PetIdleState::default());

        // Create the state machine
        let state_machine = StateMachine::default()
            .set_trans_logging(true)
            .with_state::<PetFollowState>()
            .with_state::<PetIdleState>()
            .trans::<PetAttackState>(
                Trigger::not(LineOfSight {
                    target: game.player,
                    range: 16. * 8.,
                }),
                PetFollowState {
                    target: game.player,
                    curr_delta: None,
                    curr_path: None,
                    speed: pet_state.follow_speed * 2.,
                },
            );

        e_cmds.insert(state_machine);
    }
}

pub fn handle_pet_idle_state(
    mut pets: Query<
        (
            Entity,
            &mut Transform,
            &mut PetIdleState,
            &mut AsepriteAnimation,
            &mut PetState,
            &Pet,
        ),
        With<PetIdleState>,
    >,
    time: Res<Time>,
    mut commands: Commands,
    players: Query<(Entity, &GlobalTransform), With<Player>>,
    mob_txfms: Query<&Transform, (With<Mob>, Without<PetIdleState>)>,
) {
    let (player_entity, player_transform) = players.single();
    for (entity, mut transform, mut idle_state, mut animation, mut pet_state, pet) in
        pets.iter_mut()
    {
        idle_state.walk_timer.tick(time.delta());
        idle_state.locked_in_idle_timer.tick(time.delta());

        // Set animation to idle
        if animation.current_frame() < 7 || animation.is_paused() {
            *animation = AsepriteAnimation::from(pet.get_idle_anim());
        }

        let pet_txfm = transform.translation;
        let player_txfm = player_transform.translation();
        let distance_to_player = pet_txfm.distance(player_txfm);

        // Check if we should transition to follow state
        if let Some(target_e) = pet_state.current_target {
            let target_txfm = match mob_txfms.get(target_e) {
                Ok(t) => t.translation,
                Err(_) => {
                    // Target no longer exists
                    pet_state.current_target = None;
                    continue;
                }
            };
            let distance_to_target = pet_txfm.distance(target_txfm);
            if distance_to_target > pet_state.min_target_distance
                && idle_state.locked_in_idle_timer.finished()
            {
                commands
                    .entity(entity)
                    .remove::<PetIdleState>()
                    .insert(PetFollowState {
                        target: pet_state.current_target.unwrap(),
                        curr_delta: None,
                        curr_path: None,
                        speed: pet_state.follow_speed,
                    });
                pet_state.is_following_player = false;
            }
            continue;
        }

        // If too far from player, follow player
        if distance_to_player > pet_state.max_distance_from_player
            && idle_state.locked_in_idle_timer.finished()
        {
            commands
                .entity(entity)
                .remove::<PetIdleState>()
                .insert(PetFollowState {
                    target: player_entity,
                    curr_delta: None,
                    curr_path: None,
                    speed: pet_state.follow_speed * 2.,
                });
            pet_state.is_following_player = true;
            continue;
        }

        // We're close to player and have no target, do random idle movement
        if idle_state.walk_timer.finished() && !idle_state.is_stopped {
            idle_state.direction = FacingDirection::new_rand_dir(rand::thread_rng());
            idle_state.walk_timer =
                Timer::from_seconds(rand::thread_rng().gen_range(1.0..3.0), TimerMode::Repeating);
        }

        // Move in the current direction
        if !idle_state.is_stopped {
            let direction = idle_state.direction.get_dir_vec();
            let movement = direction * idle_state.speed * time.delta_seconds();
            transform.translation += movement.extend(0.0);
        }
    }
}

pub fn handle_pet_follow_state(
    mut pets: Query<
        (
            Entity,
            &mut Transform,
            &PetFollowState,
            &mut AsepriteAnimation,
            &crate::pets::state::PetState,
            &Pet,
        ),
        With<PetFollowState>,
    >,
    targets: Query<&Transform, Without<PetFollowState>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (entity, mut transform, follow_state, mut animation, pet_state, pet) in pets.iter_mut() {
        if animation.current_frame() > 7 || animation.is_paused() {
            *animation = AsepriteAnimation::from(pet.get_walk_anim());
        }

        // Check if we should return to idle (no target or target is too close)
        if pet_state.current_target.is_none() && !pet_state.is_following_player {
            // No target, return to idle
            commands
                .entity(entity)
                .remove::<PetFollowState>()
                .insert(PetIdleState::default());
            continue;
        }

        if let Ok(target_transform) = targets.get(follow_state.target) {
            let distance_from_target = transform.translation.distance(target_transform.translation);

            // If too close to target, return to idle
            if distance_from_target < pet_state.min_target_distance {
                commands
                    .entity(entity)
                    .remove::<PetFollowState>()
                    .insert(PetIdleState::default());
                continue;
            }

            // Move towards target
            let direction = (target_transform.translation - transform.translation).normalize();
            let movement = direction * follow_state.speed * time.delta_seconds();
            transform.translation += movement;
        } else {
            // Target no longer exists, return to idle
            commands
                .entity(entity)
                .remove::<PetFollowState>()
                .insert(PetIdleState::default());
        }
    }
}
