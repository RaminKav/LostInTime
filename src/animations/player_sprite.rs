// idle
// walk
// run (run stop)
// roll
// parry (parry success)
// attack (2 forms)
// sprint
// bowbasic

//when direction changes, we need to assign a new animation handle
//
use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite};

use crate::inputs::FacingDirection;

aseprite!(pub PlayerAseprite, "textures/player/player.aseprite");

#[derive(Component, Eq, PartialEq, Debug)]
pub enum PlayerAnimation {
    Idle,
    Walk,
    Run,
    Roll,
    Parry,
    Attack,
    Sprint,
    Bow,
    Lunge,
    RunAttack,
    RunAttack1,
    RunAttack2,
    Teleport,
}
impl PlayerAnimation {
    pub fn get_str(&self, dir: FacingDirection) -> String {
        let dir_str = dir.get_anim_dir_str();
        match self {
            PlayerAnimation::Idle => format!("Idle{}", dir_str),
            PlayerAnimation::Walk => format!("Walk{}", dir_str),
            PlayerAnimation::Run => format!("Run{}", dir_str),
            PlayerAnimation::Roll => format!("Roll{}", dir_str),
            PlayerAnimation::Parry => format!("Parry{}", dir_str),
            PlayerAnimation::Attack => format!("Attack2{}", dir_str),
            PlayerAnimation::Sprint => format!("Sprint{}", dir_str),
            PlayerAnimation::Bow => format!("Bow{}", dir_str),
            PlayerAnimation::Lunge => format!("Lunge{}", dir_str),
            PlayerAnimation::RunAttack => format!("RunAttack{}", dir_str),
            PlayerAnimation::RunAttack1 => format!("RunAttack1{}", dir_str),
            PlayerAnimation::RunAttack2 => format!("RunAttack2{}", dir_str),
            PlayerAnimation::Teleport => format!("Teleport{}", dir_str),
        }
    }
    pub fn is_dir_locked(&self) -> bool {
        match self {
            PlayerAnimation::Roll => true,
            PlayerAnimation::Parry => true,
            PlayerAnimation::Attack => true,
            PlayerAnimation::Sprint => true,
            PlayerAnimation::Lunge => true,
            PlayerAnimation::Bow => true,
            PlayerAnimation::RunAttack => true,
            PlayerAnimation::RunAttack1 => true,
            PlayerAnimation::RunAttack2 => true,
            _ => false,
        }
    }
    pub fn is_attacking(&self) -> bool {
        self == &PlayerAnimation::Attack
    }
    pub fn is_movement_restricting(&self) -> bool {
        self == &PlayerAnimation::Attack || self == &PlayerAnimation::Bow
    }
    pub fn is_run_attacking(&self) -> bool {
        self == &PlayerAnimation::RunAttack
            || self == &PlayerAnimation::RunAttack1
            || self == &PlayerAnimation::RunAttack2
    }
    pub fn is_sprinting(&self) -> bool {
        self == &PlayerAnimation::Run
    }
    pub fn is_lunging(&self) -> bool {
        self == &PlayerAnimation::Lunge
    }
    pub fn is_rolling(&self) -> bool {
        self == &PlayerAnimation::Roll
    }
    pub fn is_walking(&self) -> bool {
        self == &PlayerAnimation::Walk
    }
    pub fn is_idling(&self) -> bool {
        self == &PlayerAnimation::Idle
    }
    pub fn is_one_time_anim(&self) -> bool {
        match self {
            PlayerAnimation::Roll => true,
            PlayerAnimation::Parry => true,
            PlayerAnimation::Attack => true,
            PlayerAnimation::Bow => true,
            PlayerAnimation::Lunge => true,
            PlayerAnimation::RunAttack => true,
            PlayerAnimation::RunAttack1 => true,
            PlayerAnimation::RunAttack2 => true,
            PlayerAnimation::Teleport => true,
            _ => false,
        }
    }
    pub fn is_an_attack(&self) -> bool {
        match self {
            PlayerAnimation::Attack => true,
            PlayerAnimation::Lunge => true,
            PlayerAnimation::RunAttack => true,
            PlayerAnimation::RunAttack1 => true,
            PlayerAnimation::RunAttack2 => true,
            _ => false,
        }
    }
}

#[derive(Component)]
pub struct PlayerAnimationState {
    pub prev_dir: FacingDirection,
}
impl PlayerAnimationState {
    pub fn new() -> Self {
        Self {
            prev_dir: FacingDirection::Down,
        }
    }
}

pub fn handle_anim_change_when_player_dir_changes(
    mut new_dir_query: Query<
        (
            &FacingDirection,
            &PlayerAnimation,
            &mut AsepriteAnimation,
            &mut PlayerAnimationState,
            &mut TextureAtlasSprite,
        ),
        Changed<FacingDirection>,
    >,
) {
    for (new_dir, curr_anim, mut prev_anim_state, mut prev_dir, mut sprite) in
        new_dir_query.iter_mut()
    {
        if curr_anim.is_dir_locked() {
            continue;
        }

        match new_dir {
            FacingDirection::Up => {
                *prev_anim_state = AsepriteAnimation::from(curr_anim.get_str(new_dir.clone()));

                sprite.flip_x = false;
            }
            FacingDirection::Down => {
                *prev_anim_state = AsepriteAnimation::from(curr_anim.get_str(new_dir.clone()));
                sprite.flip_x = false;
            }
            FacingDirection::Left | FacingDirection::Right => {
                *prev_anim_state = AsepriteAnimation::from(curr_anim.get_str(new_dir.clone()));
                if new_dir == &FacingDirection::Left {
                    sprite.flip_x = true;
                } else {
                    sprite.flip_x = false;
                }
            }
        }

        prev_dir.prev_dir = new_dir.clone();
    }
}

pub fn handle_player_animation_change(
    mut query: Query<
        (
            &PlayerAnimation,
            &mut AsepriteAnimation,
            &mut PlayerAnimationState,
            &FacingDirection,
        ),
        Changed<PlayerAnimation>,
    >,
) {
    for (curr_anim, mut aseprite_anim, mut prev_dir, dir) in query.iter_mut() {
        *aseprite_anim = AsepriteAnimation::from(curr_anim.get_str(dir.clone()));
        prev_dir.prev_dir = dir.clone();
    }
}

pub fn cleanup_one_time_animations(
    mut query: Query<(
        Entity,
        &PlayerAnimation,
        &AsepriteAnimation,
        &PlayerAnimationState,
    )>,
    mut commands: Commands,
) {
    for (e, curr_anim, anim_state, dir) in query.iter_mut() {
        if curr_anim.is_one_time_anim() && anim_state.just_finished() {
            commands.entity(e).insert(PlayerAnimation::Idle);
        }
    }
}
