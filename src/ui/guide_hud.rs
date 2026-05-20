use bevy::prelude::*;

use crate::{
    colors::DARK_WOOD_BROWN,
    player::Player,
    ui::global_text_message::GlobalTextMessageEvent,
    world::{
        dimension::EraManager, portal::BossKillTracker, world_helpers::tile_pos_to_world_pos,
        TILE_SIZE,
    },
    GameParam,
};

#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub enum GoalState {
    FindBossShrine,
    DefeatBoss,
    ReturnToPortal,
}

#[derive(Component)]
pub struct GoalText;

const GOAL_DISTANCE_THRESHOLD: f32 = 8. * TILE_SIZE.x; // 8 tiles

pub fn init_goal_state(mut commands: Commands) {
    commands.insert_resource(GoalState::FindBossShrine);
}

pub fn handle_goal_state_updates(
    mut goal_state: ResMut<GoalState>,
    player_query: Query<&GlobalTransform, With<Player>>,
    game: GameParam,
    boss_kill_tracker: Option<Res<BossKillTracker>>,
    portal_query: Query<&GlobalTransform, With<crate::world::portal::TimePortal>>,
    mut global_text_events: EventWriter<GlobalTextMessageEvent>,
) {
    let player_t = match player_query.get_single() {
        Ok(t) => t,
        Err(_) => return,
    };
    let player_pos = player_t.translation().truncate();

    match *goal_state {
        GoalState::FindBossShrine => {
            // Check if player is within 8 tiles of boss shrine
            if let Some(shrine_pos_tile) = game
                .world_obj_cache
                .unique_objs
                .get(&crate::item::WorldObject::BossShrine)
            {
                let shrine_pos = tile_pos_to_world_pos(*shrine_pos_tile, false);
                let distance = player_pos.distance(shrine_pos);
                if distance <= GOAL_DISTANCE_THRESHOLD {
                    *goal_state = GoalState::DefeatBoss;
                }
            }
        }
        GoalState::DefeatBoss => {
            // Check if boss is defeated for current era
            if let Some(tracker) = boss_kill_tracker.as_ref() {
                if tracker.is_boss_killed(&game.era.current_era) {
                    *goal_state = GoalState::ReturnToPortal;
                    global_text_events.send(GlobalTextMessageEvent::new(
                        "Return to the portal...",
                        DARK_WOOD_BROWN,
                    ));
                }
            }
        }
        GoalState::ReturnToPortal => {
            // Check if player is within 8 tiles of portal
            if let Ok(portal_t) = portal_query.get_single() {
                let portal_pos = portal_t.translation().truncate();
                let distance = player_pos.distance(portal_pos);
                if distance <= GOAL_DISTANCE_THRESHOLD {
                    // Portal reached, goal cycle will reset when era changes
                }
            }
        }
    }
}

pub fn handle_goal_reset_on_era_change(
    mut goal_state: ResMut<GoalState>,
    era_manager: Res<EraManager>,
    mut prev_era: Local<Option<crate::world::dimension::Era>>,
) {
    let current_era = &era_manager.current_era;

    // Reset goal when era changes (but not on first initialization)
    if let Some(prev) = prev_era.as_ref() {
        if prev != current_era {
            *goal_state = GoalState::FindBossShrine;
        }
    }
    *prev_era = Some(current_era.clone());
}

/// Compact progress HUD omits objective text; despawn any legacy `GoalText` entities.
pub fn display_goal_text(
    mut commands: Commands,
    goal_text_query: Query<Entity, With<GoalText>>,
) {
    for entity in goal_text_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}
