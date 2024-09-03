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

#[derive(Clone)]
pub struct PlayerAnimationFrameSpec {
    pub idle_start: usize,
    pub idle_end: usize,
    pub walk_start: usize,
    pub walk_end: usize,
    pub run_start: usize,
    pub run_end: usize,
    pub roll_start: usize,
    pub roll_end: usize,
    pub parry_start: usize,
    pub parry_end: usize,
    pub parry_success_start: usize,
    pub parry_success_end: usize,
    pub attack_start: usize,
    pub attack_end: usize,
    pub run_attack_start: usize,
    pub run_attack_end: usize,
    pub run_attack1_start: usize,
    pub run_attack1_end: usize,
    pub run_attack2_start: usize,
    pub run_attack2_end: usize,
    pub sprint_start: usize,
    pub sprint_end: usize,
    pub bow_start: usize,
    pub bow_end: usize,
    pub lunge_start: usize,
    pub lunge_end: usize,
}

impl PlayerAnimationFrameSpec {
    pub fn get_starting_frame_for_animation(&self, animation: &PlayerAnimation) -> usize {
        match animation {
            PlayerAnimation::Idle => self.idle_start,
            PlayerAnimation::Walk => self.walk_start,
            PlayerAnimation::Run => self.run_start,
            PlayerAnimation::Roll => self.roll_start,
            PlayerAnimation::Parry => self.parry_start,
            PlayerAnimation::Attack => self.attack_start,
            PlayerAnimation::Sprint => self.sprint_start,
            PlayerAnimation::Bow => self.bow_start,
            PlayerAnimation::Lunge => self.lunge_start,
            PlayerAnimation::RunAttack => self.run_attack_start,
            PlayerAnimation::RunAttack1 => self.run_attack1_start,
            PlayerAnimation::RunAttack2 => self.run_attack2_start,
        }
    }
    pub fn get_ending_frame_for_animation(&self, animation: &PlayerAnimation) -> usize {
        match animation {
            PlayerAnimation::Idle => self.idle_end,
            PlayerAnimation::Walk => self.walk_end,
            PlayerAnimation::Run => self.run_end,
            PlayerAnimation::Roll => self.roll_end,
            PlayerAnimation::Parry => self.parry_end,
            PlayerAnimation::Attack => self.attack_end,
            PlayerAnimation::Sprint => self.sprint_end,
            PlayerAnimation::Bow => self.bow_end,
            PlayerAnimation::Lunge => self.lunge_end,
            PlayerAnimation::RunAttack => self.run_attack_end,
            PlayerAnimation::RunAttack1 => self.run_attack1_end,
            PlayerAnimation::RunAttack2 => self.run_attack2_end,
        }
    }
}

#[derive(Resource, Clone)]
pub struct PlayerDirectionalAnimation {
    pub back: PlayerAnimationFrameSpec,
    pub front: PlayerAnimationFrameSpec,
    pub side: PlayerAnimationFrameSpec,
}
impl PlayerDirectionalAnimation {
    pub fn get_curr_anim_last_frame(
        &self,
        animation: &PlayerAnimation,
        dir: FacingDirection,
    ) -> usize {
        match dir {
            FacingDirection::Up => self.back.get_ending_frame_for_animation(animation),
            FacingDirection::Down => self.front.get_ending_frame_for_animation(animation),
            FacingDirection::Left | FacingDirection::Right => {
                self.side.get_ending_frame_for_animation(animation)
            }
        }
    }
}
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

pub fn setup_player_spec_resource(mut commands: Commands) {
    let back_offset = 129;
    let side_offset = 246;
    commands.insert_resource(PlayerDirectionalAnimation {
        back: PlayerAnimationFrameSpec {
            idle_start: 0 + back_offset,
            idle_end: 5 + back_offset,
            walk_start: 6 + back_offset,
            walk_end: 11 + back_offset,
            run_start: 12 + back_offset,
            run_end: 17 + back_offset,
            roll_start: 45 + back_offset,
            roll_end: 54 + back_offset,
            parry_start: 22 + back_offset,
            parry_end: 25 + back_offset,
            parry_success_start: 26 + back_offset,
            parry_success_end: 28 + back_offset,
            attack_start: 38 + back_offset,
            attack_end: 44 + back_offset,
            sprint_start: 55 + back_offset,
            sprint_end: 58 + back_offset,
            run_attack_start: 62 + back_offset,
            run_attack_end: 72 + back_offset,
            run_attack1_start: 62 + back_offset,
            run_attack1_end: 66 + back_offset,
            run_attack2_start: 67 + back_offset,
            run_attack2_end: 72 + back_offset,
            bow_start: 97 + back_offset,
            bow_end: 106 + back_offset,
            lunge_start: 108 + back_offset,
            lunge_end: 117 + back_offset,
        },
        front: PlayerAnimationFrameSpec {
            idle_start: 0,
            idle_end: 5,
            walk_start: 6,
            walk_end: 11,
            run_start: 12,
            run_end: 17,
            roll_start: 50,
            roll_end: 59,
            parry_start: 102,
            parry_end: 105,
            parry_success_start: 106,
            parry_success_end: 108,
            attack_start: 43,
            attack_end: 49,
            sprint_start: 60,
            sprint_end: 63,
            run_attack_start: 77,
            run_attack_end: 87,
            run_attack1_start: 77,
            run_attack1_end: 81,
            run_attack2_start: 82,
            run_attack2_end: 87,
            bow_start: 109,
            bow_end: 118,
            lunge_start: 119,
            lunge_end: 128,
        },
        side: PlayerAnimationFrameSpec {
            idle_start: 0 + side_offset,
            idle_end: 5 + side_offset,
            walk_start: 6 + side_offset,
            walk_end: 11 + side_offset,
            run_start: 12 + side_offset,
            run_end: 17 + side_offset,
            roll_start: 22 + side_offset,
            roll_end: 31 + side_offset,
            parry_start: 32 + side_offset,
            parry_end: 35 + side_offset,
            parry_success_start: 36 + side_offset,
            parry_success_end: 38 + side_offset,
            attack_start: 48 + side_offset,
            attack_end: 54 + side_offset,
            run_attack_start: 71 + side_offset,
            run_attack_end: 82 + side_offset,
            run_attack1_start: 71 + side_offset,
            run_attack1_end: 75 + side_offset,
            run_attack2_start: 76 + side_offset,
            run_attack2_end: 82 + side_offset,
            sprint_start: 83 + side_offset,
            sprint_end: 86 + side_offset,
            bow_start: 97 + side_offset,
            bow_end: 106 + side_offset,
            lunge_start: 107 + side_offset,
            lunge_end: 116 + side_offset,
        },
    })
}

pub fn cleanup_one_time_animations(
    mut query: Query<(
        Entity,
        &PlayerAnimation,
        &AsepriteAnimation,
        &PlayerAnimationState,
    )>,
    mut commands: Commands,
    anim_spec: Res<PlayerDirectionalAnimation>,
) {
    for (e, curr_anim, anim_state, dir) in query.iter_mut() {
        if curr_anim.is_one_time_anim()
            && anim_state.current_frame()
                == anim_spec.get_curr_anim_last_frame(curr_anim, dir.prev_dir.clone())
        {
            commands.entity(e).insert(PlayerAnimation::Idle);
        }
    }
}
