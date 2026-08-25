use crate::aseprite_assets::AttackWarning;
use crate::aseprite_helpers::aseprite_bundle;
use bevy_aseprite_ultra::prelude::{AseAnimation, Aseprite};
// 0 - Idle animation
// 1 - Walk animation
// 2 - Attack animation
// 3 - Hit animation
// 4 - Death animation
// L, U, R, D -> 0, 1, 2, 3

use bevy::prelude::*;

use crate::{
    animations::DoneAnimation,
    assets::Graphics,
    combat_helpers::DespawnTimer,
    ecs_helpers::SafeHierarchyExt,
    enemy::{aseprite_enemy::AsepriteBasicEnemy, Mob},
    inputs::FacingDirection,
    player::melee_skills::Parried,
};

use super::AnimationTimer;

#[derive(Component, Reflect, Eq, PartialEq, Debug, Default, Clone)]
#[reflect(Default)]
pub enum EnemyAnimationState {
    Idle,
    #[default]
    Walk,
    Attack,
    Hit,
    Death,
    Dash,
}
#[derive(Component, Reflect, Eq, PartialEq, Debug, Default)]
#[reflect(Default)]
pub struct LeftFacingSideProfile;

#[derive(Component, Clone, Reflect, Debug)]
pub struct CharacterAnimationSpriteSheetData {
    pub animation_frames: Vec<u8>,
    pub anim_offset: usize,
}
impl CharacterAnimationSpriteSheetData {
    pub fn row_for_animation(animation: &EnemyAnimationState) -> usize {
        match animation {
            EnemyAnimationState::Idle => 0,
            EnemyAnimationState::Walk => 1,
            EnemyAnimationState::Hit | EnemyAnimationState::Dash => 2,
            EnemyAnimationState::Death => 3,
            EnemyAnimationState::Attack => 4,
        }
    }

    pub fn get_starting_frame_for_animation(&self, animation: &EnemyAnimationState) -> usize {
        let max_frames = *self.animation_frames.iter().max().unwrap() as f32;
        (max_frames * Self::row_for_animation(animation) as f32) as usize
    }

    /// Keeps the sheet row in sync with [`EnemyAnimationState`].
    /// Returns `true` when the row changed (caller should snap the atlas index).
    pub fn sync_offset_to_state(&mut self, animation: &EnemyAnimationState) -> bool {
        let expected = Self::row_for_animation(animation);
        if self.anim_offset != expected {
            self.anim_offset = expected;
            true
        } else {
            false
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
            &mut Sprite,
        ),
        Changed<EnemyAnimationState>,
    >,
) {
    for (mut sprite_sheet_data, state, mut sprite) in query.iter_mut() {
        // Always update the row even if the atlas isn't ready yet. Pending sprite-sheet
        // resolution used to make this system no-op on the Changed frame, leaving
        // `anim_offset` stuck on the idle row while state was already Walk.
        sprite_sheet_data.sync_offset_to_state(state);
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            atlas.index = sprite_sheet_data.get_starting_frame_for_animation(state);
        }
    }
}
/// Only runs for legacy sprite-sheet mobs. Aseprite-based mobs are excluded so their atlas is
/// not overwritten with mob_spritesheets data they don't have:
/// - [`AsepriteBasicEnemy`] for shared walk+lunge aseprite mobs
/// - [`AseAnimation`] for any other aseprite-driven mob (e.g. bosses) that may still carry
///   [`FacingDirection`] for AI; those use their own sheet from `aseprite_bundle`.
pub fn change_character_anim_direction(
    mut mob_query: Query<
        (
            &FacingDirection,
            &mut Sprite,
            &Mob,
            Option<&LeftFacingSideProfile>,
        ),
        (
            Changed<FacingDirection>,
            Without<AsepriteBasicEnemy>,
            Without<AseAnimation>,
        ),
    >,
    graphics: Res<Graphics>,
) {
    let Some(sheets) = graphics.mob_spritesheets.as_ref() else {
        return;
    };
    for (facing_direction, mut sprite, mob, left_side_profile_option) in mob_query.iter_mut() {
        let Some(handles) = sheets.get(mob) else {
            continue;
        };
        match facing_direction {
            FacingDirection::Left => {
                sprite.image = handles[0].clone();
                sprite.flip_x = left_side_profile_option.is_none();
            }
            FacingDirection::Up => {
                sprite.image = handles[1].clone();
            }
            FacingDirection::Right => {
                sprite.image = handles[0].clone();
                sprite.flip_x = left_side_profile_option.is_some();
            }
            FacingDirection::Down => {
                sprite.image = handles[2].clone();
            }
        }
    }
}

pub fn animate_character_spritesheet_animations(
    time: Res<Time>,
    mut query: Query<
        (
            &mut AnimationTimer,
            &mut CharacterAnimationSpriteSheetData,
            &mut Sprite,
            Option<&EnemyAnimationState>,
        ),
        Without<Parried>,
    >,
) {
    for (mut timer, mut sprite_sheet_data, mut sprite, anim_state) in query.iter_mut() {
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };
        // Heal the spawn race where Walk was set before the atlas existed, or pending
        // sprite apply reset the index to 0 while `anim_offset` still pointed at idle.
        if let Some(state) = anim_state {
            if sprite_sheet_data.sync_offset_to_state(state) {
                atlas.index = sprite_sheet_data.get_starting_frame_for_animation(state);
            }
        }

        timer.tick(time.delta());
        if timer.just_finished() {
            let max_frames = *sprite_sheet_data.animation_frames.iter().max().unwrap() as f32;
            let frames =
                (sprite_sheet_data.animation_frames[sprite_sheet_data.anim_offset]) as usize;
            if frames == 0 {
                continue;
            }
            let row_base = max_frames as usize * sprite_sheet_data.anim_offset;
            let local = atlas.index.saturating_sub(row_base);
            atlas.index = (local + 1) % frames + row_base;
            timer.reset();
        }
    }
}

pub fn spawn_attack_warning_aseprite(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    parent: Entity,
    duration: f32,
) -> Entity {
    let entity = commands
        .spawn((
            aseprite_bundle(
                asset_server.load(AttackWarning::PATH),
                AttackWarning::tags::WARNING,
                Transform::from_translation(pos),
                Visibility::default(),
                true,
            ),
            DespawnTimer(Timer::from_seconds(duration, TimerMode::Once)),
            DoneAnimation,
        ))
        .safe_set_parent(parent)
        .id();
    entity
}

/// Persistent UI warning icon (achievements, class unlocks). Loops until the caller despawns it.
/// Combat telegraphs should use [`spawn_attack_warning_aseprite`], which plays once and despawns.
pub fn spawn_looping_attack_warning_aseprite(
    commands: &mut Commands,
    asset_server: &AssetServer,
    pos: Vec3,
    parent: Entity,
) -> Entity {
    commands
        .spawn(aseprite_bundle(
            asset_server.load(AttackWarning::PATH),
            AttackWarning::tags::WARNING,
            Transform::from_translation(pos),
            Visibility::default(),
            false,
        ))
        .safe_set_parent(parent)
        .id()
}
