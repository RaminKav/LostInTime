use bevy::{prelude::*, render::view::RenderLayers};

use crate::{
    player::Player,
    world::{
        dimension::EraManager, portal::BossKillTracker, world_helpers::tile_pos_to_world_pos,
        TILE_SIZE,
    },
    GameParam, ScreenResolution, GAME_HEIGHT,
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

pub fn display_goal_text(
    mut commands: Commands,
    goal_state: Res<GoalState>,
    asset_server: Res<AssetServer>,
    goal_text_query: Query<Entity, With<GoalText>>,
    res: Res<ScreenResolution>,
) {
    // Update text when goal state changes or if no text exists yet
    if !goal_state.is_changed() && !goal_text_query.is_empty() {
        return;
    }

    // Remove old goal text
    for entity in goal_text_query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    // Get goal text
    let goal_text = match *goal_state {
        GoalState::FindBossShrine => "Find the Boss Shrine",
        GoalState::DefeatBoss => "Defeat the Boss",
        GoalState::ReturnToPortal => "Return To Portal",
    };

    // Spawn new goal text
    commands
        .spawn(Text2dBundle {
            text: Text::from_section(
                goal_text,
                TextStyle {
                    font: asset_server.load("fonts/4x5.ttf"),
                    font_size: 5.0,
                    color: Color::WHITE,
                },
            )
            .with_alignment(TextAlignment::Center),
            text_anchor: bevy::sprite::Anchor::CenterLeft,
            transform: Transform::from_translation(Vec3::new(
                -res.game_width / 2. + 5.5,
                (GAME_HEIGHT - 15.) / 2. - 50.,
                3.,
            )),
            ..Default::default()
        })
        .insert(GoalText)
        .insert(RenderLayers::from_layers(&[3]))
        .insert(Name::new("Goal Text"));
}
