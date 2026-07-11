use bevy::prelude::*;

use crate::{
    item::object_actions::ObjectAction,
    player::{
        skills::{
            get_disabled_skills, ActiveSkill, ActiveSkillChoiceState, HeirloomRarity, PlayerSkills,
            SkillClass,
        },
        unlocks::{UnlockUpgrades, UnlockedSkills},
    },
    ui::{key_input_guide::InteractionGuideTrigger, minimap::UpdateMiniMapEvent, UIState},
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

/// Number of distinct skills offered at each active skill shrine.
pub const ACTIVE_SKILL_SHRINE_OFFER_COUNT: usize = 2;

/// Same filter as the shrine interaction: exclude these and anything the player already has.
pub fn roll_active_skill_shrine_offer_skills(
    player_skills: Option<&PlayerSkills>,
) -> Vec<ActiveSkill> {
    refresh_active_skill_shrine_offer_skills(&[], player_skills)
}

/// Roll a fresh offer, excluding the previous shrine choices so rerolls swap both skills.
pub fn reroll_active_skill_shrine_offer_skills(
    previous_offer: &[ActiveSkill],
    player_skills: Option<&PlayerSkills>,
) -> Vec<ActiveSkill> {
    refresh_active_skill_shrine_offer_skills_impl(&[], player_skills, previous_offer)
}

/// Re-validate a cached shrine offer against the player's current active skills:
/// drop any entries the player has acquired since the offer was rolled and top
/// the result up to [`ACTIVE_SKILL_SHRINE_OFFER_COUNT`] distinct choices with
/// fresh rolls. Used at interaction time so a previously rolled offer can't hand
/// out duplicates of skills the player picked up between world-gen and visiting
/// the shrine.
pub fn refresh_active_skill_shrine_offer_skills(
    cached_offer: &[ActiveSkill],
    player_skills: Option<&PlayerSkills>,
) -> Vec<ActiveSkill> {
    refresh_active_skill_shrine_offer_skills_impl(cached_offer, player_skills, &[])
}

fn refresh_active_skill_shrine_offer_skills_impl(
    cached_offer: &[ActiveSkill],
    player_skills: Option<&PlayerSkills>,
    also_exclude: &[ActiveSkill],
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

    // Keep cached entries the player doesn't already own, dropping any
    // duplicates that may have crept in.
    let mut chosen_skills: Vec<ActiveSkill> = Vec::new();
    for skill in cached_offer.iter().copied() {
        if !player_current_skills.contains(&skill) && !chosen_skills.contains(&skill) {
            chosen_skills.push(skill);
        }
    }

    let mut available_skills: Vec<ActiveSkill> = ActiveSkill::iter()
        .filter(|skill| {
            !get_disabled_skills().contains(skill)
                && *skill != ActiveSkill::LaserBeam
                && !skill.is_movement_skill()
                && !player_current_skills.contains(skill)
                && !chosen_skills.contains(skill)
                && !also_exclude.contains(skill)
        })
        .collect();

    while chosen_skills.len() < ACTIVE_SKILL_SHRINE_OFFER_COUNT {
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

/// Shrine assignment targets: slots 1–2 only. Slot 2 requires a Time Fragment
/// purchase (or `bypass_unlocks`). Slot 0 (movement) is never assignable.
pub fn shrine_assignable_slots(
    class: &SkillClass,
    unlocked: &UnlockedSkills,
    unlock_upgrades: &UnlockUpgrades,
    bypass_unlocks: bool,
) -> Vec<usize> {
    let mut slots = vec![1];
    if bypass_unlocks || unlocked.is_unlocked(class, 2, unlock_upgrades) {
        slots.push(2);
    }
    slots
}

pub enum ShrineAssignAction {
    AutoFill(usize),
    ShowSlotPicker,
}

/// After the player picks a shrine skill, auto-fill the first empty unlocked slot
/// (slot 1 / LMB before slot 2) or show the slot picker when both are full.
pub fn shrine_assign_action(
    skills: &PlayerSkills,
    class: &SkillClass,
    unlocked: &UnlockedSkills,
    unlock_upgrades: &UnlockUpgrades,
    bypass_unlocks: bool,
) -> ShrineAssignAction {
    let assignable = shrine_assignable_slots(class, unlocked, unlock_upgrades, bypass_unlocks);
    let empty_slots: Vec<usize> = assignable
        .iter()
        .copied()
        .filter(|&slot| skills.get_active_skill_in_slot(slot).is_none())
        .collect();

    if let Some(&slot) = empty_slots.first() {
        ShrineAssignAction::AutoFill(slot)
    } else {
        ShrineAssignAction::ShowSlotPicker
    }
}

/// Write a shrine skill into the given player slot index (1 or 2).
pub fn assign_shrine_skill_to_slot(skills: &mut PlayerSkills, slot: usize, skill: ActiveSkillChoiceState) {
    match slot {
        1 => skills.active_skill_slot_1 = Some(skill),
        2 => skills.active_skill_slot_2 = Some(skill),
        _ => (),
    }
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

pub fn handle_active_skill_shrine_completion(
    shrines: Query<(Entity, &ActiveSkillShrineState)>,
    mut commands: Commands,
    mut game: GameParam,
    mut minimap_event: EventWriter<UpdateMiniMapEvent>,
) {
    for (e, shrine) in shrines.iter() {
        if shrine.is_used {
            commands
                .entity(e)
                .insert(WorldObject::ActiveSkillShrineDone)
                .remove::<ObjectAction>()
                .remove::<InteractionGuideTrigger>()
                .remove::<ActiveSkillShrineState>();

            game.add_object_to_chunk_cache(shrine.tile_pos, WorldObject::ActiveSkillShrineDone);
            game.world_obj_cache
                .active_skill_shrine_offers
                .remove(&shrine.tile_pos);

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
