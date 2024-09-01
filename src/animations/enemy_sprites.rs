// 0 - Idle animation
// 1 - Walk animation
// 2 - Attack animation
// 3 - Hit animation
// 4 - Death animation
// L, U, R, D -> 0, 1, 2, 3

use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use crate::{assets::Graphics, enemy::Mob, inputs::FacingDirection, player::Player, GameParam};

use super::AnimationTimer;

#[derive(Component, Schematic, Reflect, FromReflect, Eq, PartialEq, Debug, Default)]
#[reflect(Schematic, Default)]
pub enum EnemyAnimationState {
    Idle,
    #[default]
    Walk,
    Attack,
    Hit,
    Death,
    Dash,
}
#[derive(Component, Schematic, Reflect, FromReflect, Eq, PartialEq, Debug, Default)]
#[reflect(Schematic, Default)]
pub struct LeftFacingSideProfile;

#[derive(Component, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct CharacterAnimationSpriteSheetData {
    pub animation_frames: Vec<u8>,
    pub anim_offset: usize,
}
impl CharacterAnimationSpriteSheetData {
    pub fn get_starting_frame_for_animation(&self, animation: &EnemyAnimationState) -> usize {
        let max_frames = *self.animation_frames.iter().max().unwrap() as f32;
        match animation {
            EnemyAnimationState::Idle => 0,
            EnemyAnimationState::Walk => (max_frames * 1.) as usize,
            EnemyAnimationState::Hit | EnemyAnimationState::Dash => (max_frames * 2.) as usize,
            EnemyAnimationState::Death => (max_frames * 3.) as usize,
            EnemyAnimationState::Attack => (max_frames * 4.) as usize,
        }
    }
    pub fn is_done_current_animation(&self, index: usize) -> bool {
        let max_frames = *self.animation_frames.iter().max().unwrap() as f32;
        let current_frame = index as f32;
        let current_animation = self.anim_offset as f32;
        let current_animation_frames = self.animation_frames[current_animation as usize] as f32;
        current_frame >= current_animation_frames + max_frames * (current_animation) - 1.
    }
    pub fn get_anim_state(&self) -> EnemyAnimationState {
        match self.anim_offset {
            0 => EnemyAnimationState::Idle,
            1 => EnemyAnimationState::Walk,
            2 => EnemyAnimationState::Hit,
            3 => EnemyAnimationState::Death,
            4 => EnemyAnimationState::Attack,
            _ => EnemyAnimationState::Attack,
        }
    }
}

pub fn change_anim_offset_when_character_action_state_changes(
    mut query: Query<
        (
            Entity,
            &mut CharacterAnimationSpriteSheetData,
            &EnemyAnimationState,
            &mut TextureAtlas,
        ),
        Changed<EnemyAnimationState>,
    >,
    game: GameParam,
) {
    for (e, mut sprite_sheet_data, state, mut sprite) in query.iter_mut() {
        let is_player = game.game.player == e;
        let max_frames = *sprite_sheet_data.animation_frames.iter().max().unwrap() as f32;
        match state {
            EnemyAnimationState::Idle => {
                sprite_sheet_data.anim_offset = 0;
            }
            EnemyAnimationState::Walk => {
                sprite_sheet_data.anim_offset = 1;
                sprite.index = (max_frames * 1.) as usize;
            }
            EnemyAnimationState::Hit | EnemyAnimationState::Dash => {
                sprite_sheet_data.anim_offset = 2;
                sprite.index = (max_frames * 2.) as usize;
            }
            EnemyAnimationState::Death => {
                sprite_sheet_data.anim_offset = 3;
                sprite.index = (max_frames * 3.) as usize;
            }
            EnemyAnimationState::Attack => {
                if is_player {
                    if let Some(main_hand) = game.player().main_hand_slot {
                        if main_hand.item_stack.obj_type.is_weapon() {
                            sprite_sheet_data.anim_offset =
                                4 + main_hand.get_attack_anim_offset() as usize;
                            sprite.index =
                                (max_frames * (4. + main_hand.get_attack_anim_offset())) as usize;
                        }
                    }
                } else {
                    sprite_sheet_data.anim_offset = 4;
                    sprite.index = (max_frames * 4.) as usize;
                }
            }
        }
    }
}
pub fn change_character_anim_direction(
    mut mob_query: Query<
        (
            &FacingDirection,
            &mut TextureAtlas,
            &mut Handle<TextureAtlas>,
            Option<&Mob>,
            Option<&LeftFacingSideProfile>,
        ),
        Changed<FacingDirection>,
    >,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    _asset_server: Res<AssetServer>,
    graphics: Res<Graphics>,
) {
    for (
        facing_direction,
        mut sprite,
        texture_atlas_handle,
        mob_option,
        left_side_profile_option,
    ) in mob_query.iter_mut()
    {
        let texture_atlas = texture_atlases.get_mut(&texture_atlas_handle).unwrap();

        match facing_direction {
            FacingDirection::Left => {
                texture_atlas.texture = if let Some(mob) = mob_option {
                    graphics
                        .mob_spritesheets
                        .as_ref()
                        .unwrap()
                        .get(mob)
                        .unwrap()[0]
                        .clone()
                } else {
                    graphics.player_spritesheets.as_ref().unwrap()[0].clone()
                };
                sprite.flip_x = left_side_profile_option.is_none();
            }
            FacingDirection::Up => {
                texture_atlas.texture = if let Some(mob) = mob_option {
                    graphics
                        .mob_spritesheets
                        .as_ref()
                        .unwrap()
                        .get(mob)
                        .unwrap()[1]
                        .clone()
                } else {
                    graphics.player_spritesheets.as_ref().unwrap()[1].clone()
                };
            }
            FacingDirection::Right => {
                texture_atlas.texture = if let Some(mob) = mob_option {
                    graphics
                        .mob_spritesheets
                        .as_ref()
                        .unwrap()
                        .get(mob)
                        .unwrap()[0]
                        .clone()
                } else {
                    graphics.player_spritesheets.as_ref().unwrap()[0].clone()
                };
                sprite.flip_x = left_side_profile_option.is_some();
            }
            FacingDirection::Down => {
                texture_atlas.texture = if let Some(mob) = mob_option {
                    graphics
                        .mob_spritesheets
                        .as_ref()
                        .unwrap()
                        .get(mob)
                        .unwrap()[2]
                        .clone()
                } else {
                    graphics.player_spritesheets.as_ref().unwrap()[2].clone()
                };
            }
        }
    }
}

pub fn animate_character_spritesheet_animations(
    time: Res<Time>,
    mut query: Query<(
        Entity,
        &mut AnimationTimer,
        &CharacterAnimationSpriteSheetData,
        &mut TextureAtlas,
        Option<&Player>,
    )>,
) {
    for (_e, mut timer, sprite_sheet_data, mut sprite, is_player) in &mut query {
        let mult = if is_player.is_some()
            && sprite_sheet_data.get_anim_state() == EnemyAnimationState::Attack
        {
            3.
        } else if is_player.is_none()
            && sprite_sheet_data.get_anim_state() == EnemyAnimationState::Idle
        {
            0.5
        } else {
            1.
        };
        timer.tick(time.delta().mul_f32(mult));
        if timer.just_finished() {
            let max_frames = *sprite_sheet_data.animation_frames.iter().max().unwrap() as f32;
            let frames =
                (sprite_sheet_data.animation_frames[sprite_sheet_data.anim_offset]) as usize;
            sprite.index =
                ((sprite.index + 1 - max_frames as usize * sprite_sheet_data.anim_offset) % frames)
                    + max_frames as usize * sprite_sheet_data.anim_offset;
            timer.reset();
        }
    }
}
