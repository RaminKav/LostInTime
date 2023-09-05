// 0 - Idle animation
// 1 - Walk animation
// 2 - Attack animation
// 3 - Hit animation
// 4 - Death animation
// L, U, R, D -> 0, 1, 2, 3

use bevy::prelude::*;
use bevy_proto::prelude::{ReflectSchematic, Schematic};

use crate::{enemy::Mob, inputs::FacingDirection};

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
}
#[derive(Component, Schematic, Reflect, FromReflect, Eq, PartialEq, Debug, Default)]
#[reflect(Schematic, Default)]
pub struct LeftFacingSideProfile;

#[derive(Component, Clone, Schematic, Reflect, FromReflect)]
#[reflect(Schematic)]
pub struct CharacterAnimationSpriteSheetData {
    pub animation_frames: Vec<u8>,
    pub valid_directions: Vec<u8>,
    pub anim_offset: usize,
}
impl CharacterAnimationSpriteSheetData {
    pub fn get_starting_frame_for_animation(&self, animation: &EnemyAnimationState) -> usize {
        let max_frames = *self.animation_frames.iter().max().unwrap() as f32;
        match animation {
            EnemyAnimationState::Idle => 0,
            EnemyAnimationState::Walk => (max_frames * 1.) as usize,
            EnemyAnimationState::Hit => (max_frames * 2.) as usize,
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
}

pub fn change_anim_offset_when_character_action_state_changes(
    mut query: Query<
        (
            &mut CharacterAnimationSpriteSheetData,
            &EnemyAnimationState,
            &mut TextureAtlasSprite,
        ),
        Changed<EnemyAnimationState>,
    >,
) {
    for (mut sprite_sheet_data, state, mut sprite) in query.iter_mut() {
        let max_frames = *sprite_sheet_data.animation_frames.iter().max().unwrap() as f32;
        match state {
            EnemyAnimationState::Idle => {
                sprite_sheet_data.anim_offset = 0;
            }
            EnemyAnimationState::Walk => {
                sprite_sheet_data.anim_offset = 1;
                sprite.index = (max_frames * 1.) as usize;
            }
            EnemyAnimationState::Hit => {
                sprite_sheet_data.anim_offset = 2;
                sprite.index = (max_frames * 2.) as usize;
            }
            EnemyAnimationState::Death => {
                sprite_sheet_data.anim_offset = 3;
                sprite.index = (max_frames * 3.) as usize;
            }
            EnemyAnimationState::Attack => {
                sprite_sheet_data.anim_offset = 4;
                sprite.index = (max_frames * 4.) as usize;
            }
        }
    }
}
pub fn change_character_anim_direction(
    mut mob_query: Query<
        (
            &FacingDirection,
            &mut TextureAtlasSprite,
            &mut Handle<TextureAtlas>,
            &Mob,
            Option<&LeftFacingSideProfile>,
        ),
        With<Mob>,
    >,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    asset_server: Res<AssetServer>,
) {
    for (facing_direction, mut sprite, texture_atlas_handle, mob, left_side_profile_option) in
        mob_query.iter_mut()
    {
        let mut texture_atlas = texture_atlases.get_mut(&texture_atlas_handle).unwrap();

        match facing_direction {
            FacingDirection::Left => {
                texture_atlas.texture = asset_server.load(format!(
                    "textures/{}/{}_{}.png",
                    mob.to_string().to_lowercase(),
                    mob.to_string().to_lowercase(),
                    "side"
                ));
                sprite.flip_x = left_side_profile_option.is_none();
            }
            FacingDirection::Up => {
                texture_atlas.texture = asset_server.load(format!(
                    "textures/{}/{}_{}.png",
                    mob.to_string().to_lowercase(),
                    mob.to_string().to_lowercase(),
                    "up"
                ));
            }
            FacingDirection::Right => {
                texture_atlas.texture = asset_server.load(format!(
                    "textures/{}/{}_{}.png",
                    mob.to_string().to_lowercase(),
                    mob.to_string().to_lowercase(),
                    "side"
                ));
                sprite.flip_x = left_side_profile_option.is_some();
            }
            FacingDirection::Down => {
                texture_atlas.texture = asset_server.load(format!(
                    "textures/{}/{}_{}.png",
                    mob.to_string().to_lowercase(),
                    mob.to_string().to_lowercase(),
                    "down"
                ));
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
        &mut TextureAtlasSprite,
    )>,
) {
    for (_e, mut timer, sprite_sheet_data, mut sprite) in &mut query {
        timer.tick(time.delta());
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
