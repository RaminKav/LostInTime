use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    assets::Graphics, item::object_actions::ObjectAction,
    ui::key_input_guide::InteractionGuideTrigger, world::TileMapPosition, GameParam,
};

use super::WorldObject;

#[derive(Component)]
pub struct ActiveSkillShrineState {
    pub is_used: bool,
    pub tile_pos: TileMapPosition,
}

use crate::player::skills::ActiveSkillChoiceState;

/// Resource to hold the active skill choice from a shrine interaction
#[derive(Resource, Clone, Debug)]
pub struct ActiveSkillShrineSelection {
    pub skill_choice: ActiveSkillChoiceState,
    pub shrine_entity: Entity,
}

/// Resource to hold a skill waiting to replace an existing active skill
#[derive(Resource, Clone, Debug)]
pub struct ActiveSkillShrineOverwrite {
    pub skill_choice: ActiveSkillChoiceState,
    pub shrine_entity: Entity,
}

aseprite!(pub ActiveSkillSprite, "textures/miner.ase");

pub fn add_active_skill_shrine_visuals_on_spawn(
    mut commands: Commands,
    new_shrines: Query<
        (Entity, &WorldObject, &Transform),
        Or<(Added<WorldObject>, Changed<WorldObject>)>,
    >,
    graphics: Res<Graphics>,
) {
    for (e, obj, t) in new_shrines.iter() {
        if obj == &WorldObject::ActiveSkillShrine {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(ActiveSkillSprite::tags::IDLE),
                    aseprite: graphics.active_skill_shrine.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("ACTIVE_SKILL_SHRINE"));
        } else if obj == &WorldObject::ActiveSkillShrineDone {
            commands
                .entity(e)
                .insert(AsepriteBundle {
                    transform: *t,
                    animation: AsepriteAnimation::from(ActiveSkillSprite::tags::DONE),
                    aseprite: graphics.active_skill_shrine.as_ref().unwrap().clone(),
                    ..default()
                })
                .insert(Name::new("ACTIVE_SKILL_SHRINE_DONE"));
        }
    }
}

pub fn handle_active_skill_shrine_completion(
    shrines: Query<(Entity, &ActiveSkillShrineState)>,
    mut commands: Commands,
    mut game: GameParam,
) {
    for (e, shrine) in shrines.iter() {
        if shrine.is_used {
            // Update animation to the "Done" variant
            commands
                .entity(e)
                .insert(WorldObject::ActiveSkillShrineDone)
                .insert(AsepriteAnimation::from(ActiveSkillSprite::tags::DONE))
                .remove::<ObjectAction>()
                .remove::<InteractionGuideTrigger>()
                .remove::<ActiveSkillShrineState>();

            // Update the world object cache so the shrine stays "Done" when chunk respawns
            game.add_object_to_chunk_cache(shrine.tile_pos, WorldObject::ActiveSkillShrineDone);
        }
    }
}

/// Handle ESC closing the active skill shrine UI - mark shrine as done
pub fn handle_active_skill_shrine_esc(
    shrine_selection: Option<Res<ActiveSkillShrineSelection>>,
    mut shrine_query: Query<&mut ActiveSkillShrineState>,
    mut commands: Commands,
    curr_ui_state: Res<State<crate::ui::UIState>>,
) {
    // If we have a selection resource but we're not in the ActiveSkillShrine UI state,
    // it means the player ESC'd without making a choice
    if curr_ui_state.0 != crate::ui::UIState::ActiveSkillShrine {
        if let Some(selection) = shrine_selection {
            if let Ok(mut shrine_state) = shrine_query.get_mut(selection.shrine_entity) {
                shrine_state.is_used = true;
            }
            commands.remove_resource::<ActiveSkillShrineSelection>();
        }
    }
}
