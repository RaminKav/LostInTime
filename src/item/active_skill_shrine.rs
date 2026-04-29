use bevy::prelude::*;
use bevy_aseprite::{anim::AsepriteAnimation, aseprite, AsepriteBundle};

use crate::{
    assets::Graphics,
    item::object_actions::ObjectAction,
    player::skills::{
        ActiveSkill, ActiveSkillChoiceState, HeirloomRarity, PlayerSkills,
    },
    ui::{
        key_input_guide::InteractionGuideTrigger,
        minimap::UpdateMiniMapEvent,
        UIState,
    },
    world::TileMapPosition,
    GameParam,
};
use rand::seq::IteratorRandom;
use strum::IntoEnumIterator;

use super::WorldObject;

/// Per-shrine state; only present on an un-consumed active-skill shrine and
/// removed on use. `SparseSet` for the same reason as the other shrine state
/// components.
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ActiveSkillShrineState {
    pub is_used: bool,
    pub tile_pos: TileMapPosition,
}

/// Same filter as the shrine interaction: exclude these and anything the player already has.
pub fn roll_active_skill_shrine_offer_skills(
    player_skills: Option<&PlayerSkills>,
) -> Vec<ActiveSkill> {
    let mut rng = rand::thread_rng();
    let player_current_skills = player_skills
        .map(|skills| {
            let mut current = Vec::new();
            for slot in [
                &skills.active_skill_slot_0,
                &skills.active_skill_slot_1,
                &skills.active_skill_slot_2,
                &skills.active_skill_slot_3,
                &skills.active_skill_slot_4,
            ] {
                if let Some(s) = slot {
                    current.push(s.active_skill);
                }
            }
            current
        })
        .unwrap_or_default();

    let mut available_skills: Vec<ActiveSkill> = ActiveSkill::iter()
        .filter(|skill| {
            *skill != ActiveSkill::Parry
                && *skill != ActiveSkill::Sprint
                && *skill != ActiveSkill::LaserBeam
                && !player_current_skills.contains(skill)
        })
        .collect();

    let mut chosen_skills = Vec::new();
    for _ in 0..2 {
        if available_skills.is_empty() {
            break;
        }
        let chosen = *available_skills.iter().choose(&mut rng).unwrap();
        chosen_skills.push(chosen);
        available_skills.retain(|s| *s != chosen);
    }
    chosen_skills
}

pub fn skill_choices_from_offer_skills(skills: &[ActiveSkill]) -> Vec<ActiveSkillChoiceState> {
    skills
        .iter()
        .map(|skill| ActiveSkillChoiceState::new(*skill, HeirloomRarity::Common))
        .collect()
}

/// Resource to hold the active skill choices from a shrine interaction
#[derive(Resource, Clone, Debug)]
pub struct ActiveSkillShrineSelection {
    pub skill_choices: Vec<ActiveSkillChoiceState>,
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
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
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
            game.world_obj_cache
                .active_skill_shrine_offers
                .remove(&shrine.tile_pos);

            // Update minimap to reflect the shrine is now "Done"
            minimap_event.send(UpdateMiniMapEvent {
                pos: Some(shrine.tile_pos),
                new_tile: Some(WorldObject::ActiveSkillShrineDone),
            });
        }
    }
}

fn restore_active_skill_shrine_interactivity(commands: &mut Commands, shrine_entity: Entity) {
    commands
        .entity(shrine_entity)
        .remove::<ActiveSkillShrineState>()
        .insert(ObjectAction::ActiveSkillShrine)
        .insert(InteractionGuideTrigger {
            key: Some("F".to_string()),
            text: Some("Get Skill".to_string()),
            activation_distance: 32.,
            icon_stack: None,
        });
}

/// When gameplay UI is closed, clear an abandoned active skill shrine flow and make the
/// shrine interactable again. The shrine is only consumed in `handle_active_skill_shrine_completion`
/// after a successful swap (`is_used` in `handle_active_skill_shrine_overwrite_interaction`).
///
/// Only reacts in `UIState::Closed` so we never restore during the transition from the skill
/// list to the slot-overwrite screen (`ActiveSkillShrine` → `ActiveSkills`).
pub fn handle_active_skill_shrine_esc(
    shrine_selection: Option<Res<ActiveSkillShrineSelection>>,
    shrine_overwrite: Option<Res<ActiveSkillShrineOverwrite>>,
    mut commands: Commands,
    curr_ui_state: Res<State<UIState>>,
) {
    if curr_ui_state.0 != UIState::Closed {
        return;
    }
    if let Some(selection) = shrine_selection.as_ref() {
        restore_active_skill_shrine_interactivity(&mut commands, selection.shrine_entity);
        commands.remove_resource::<ActiveSkillShrineSelection>();
    }
    if let Some(overwrite) = shrine_overwrite.as_ref() {
        restore_active_skill_shrine_interactivity(&mut commands, overwrite.shrine_entity);
        commands.remove_resource::<ActiveSkillShrineOverwrite>();
    }
}
