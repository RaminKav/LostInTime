use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    assets::Graphics, item::object_actions::ObjectAction,
    ui::key_input_guide::InteractionGuideTrigger,
};

use super::WorldObject;

#[derive(Component)]
pub struct ActiveSkillShrineState {
    pub is_used: bool,
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
) {
    for (e, shrine) in shrines.iter() {
        if shrine.is_used {
            commands
                .entity(e)
                .insert(WorldObject::ActiveSkillShrineDone)
                .remove::<ObjectAction>()
                .remove::<InteractionGuideTrigger>()
                .remove::<ActiveSkillShrineState>();
        }
    }
}
