use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task},
};
use chrono::{DateTime, Utc};
use futures_lite::future;
use serde::{Deserialize, Serialize};

use crate::{
    assets::Graphics,
    player::{score::RunScore, skills::PlayerClass},
    ui::CheatSettings,
    GameState,
};

/// Configuration for leaderboard server
/// Uses localhost in debug builds, production URL in release builds
/// Configuration for leaderboard server
#[cfg(debug_assertions)]
const LEADERBOARD_API_URL: &str =
    "https://lost-in-time-leaderboard-production.up.railway.app/api/leaderboard";

#[cfg(not(debug_assertions))]
const LEADERBOARD_API_URL: &str =
    "https://lost-in-time-leaderboard-production.up.railway.app/api/leaderboard";

// Alternative: Use environment variable with fallback
// const LEADERBOARD_API_URL: &str = env!("LEADERBOARD_URL", "http://localhost:3000/api/leaderboard");

/// Request to submit a score
#[derive(Debug, Serialize)]
pub struct SubmitScoreRequest {
    pub user_id: String,
    pub player_name: String,
    pub score: i32,
    pub class: String,
    pub chaos_level: i32,
    pub mobs_killed: i32,
    pub objs_destroyed: i32,
}

/// Response from submitting a score
#[derive(Debug, Deserialize)]
pub struct SubmitScoreResponse {
    pub success: bool,
    pub rank: Option<i64>,
    pub is_personal_best: bool,
    pub message: String,
}

/// A single entry in the leaderboard
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LeaderboardEntry {
    pub rank: Option<i64>,
    pub player_name: String,
    pub score: i32,
    pub class: String,
    pub chaos_level: i32,
    pub submitted_at: DateTime<Utc>,
}

/// Response from fetching the leaderboard
#[derive(Debug, Deserialize)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
    pub last_updated: DateTime<Utc>,
}

/// Cached leaderboard data
#[derive(Resource, Debug, Default, Clone)]
pub struct LeaderboardCache {
    pub entries: Vec<LeaderboardEntry>,
    pub last_updated: Option<DateTime<Utc>>,
    pub is_loading: bool,
    pub last_error: Option<String>,
}

/// Resource to track the last submitted score and rank for display on game over screen
#[derive(Resource, Default)]
pub struct LastSubmittedScore {
    pub score: i32,
    pub rank: Option<i64>,
    pub is_personal_best: bool,
}

/// Event to trigger score submission
#[derive(Debug)]
pub struct SubmitScoreEvent {
    pub user_id: String,
    pub player_name: String,
    pub score: i32,
    pub class: String,
    pub chaos_level: i32,
    pub mobs_killed: i32,
    pub objs_destroyed: i32,
}

/// Event to trigger leaderboard fetch
#[derive(Debug)]
pub struct FetchLeaderboardEvent {
    pub limit: i32,
    pub class_filter: Option<String>,
}

/// Component for tracking leaderboard fetch task
#[derive(Component)]
pub struct FetchLeaderboardTask(Task<Result<LeaderboardResponse, String>>);

/// Component for tracking score submit task  
#[derive(Component)]
pub struct SubmitScoreTask(Task<Result<SubmitScoreResponse, String>>);

/// Submit a score to the leaderboard (blocking, runs in separate thread)
pub fn submit_score_blocking(request: SubmitScoreRequest) -> Result<SubmitScoreResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .post(format!("{}/submit", LEADERBOARD_API_URL))
        .json(&request)
        .send()
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if response.status().is_success() {
        response
            .json::<SubmitScoreResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    } else {
        Err(format!("Server error: {}", response.status()))
    }
}

/// Fetch top scores from the leaderboard (blocking, runs in separate thread)
pub fn fetch_leaderboard_blocking(
    limit: i32,
    class_filter: Option<String>,
) -> Result<LeaderboardResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let mut url = format!("{}/top?limit={}", LEADERBOARD_API_URL, limit);
    if let Some(class) = class_filter {
        url.push_str(&format!("&class={}", class));
    }

    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to send request: {}", e))?;

    if response.status().is_success() {
        response
            .json::<LeaderboardResponse>()
            .map_err(|e| format!("Failed to parse response: {}", e))
    } else {
        Err(format!("Server error: {}", response.status()))
    }
}

/// System to handle score submission events (spawns async task)
pub fn handle_submit_score_event(
    mut commands: Commands,
    mut events: EventReader<SubmitScoreEvent>,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for event in events.iter() {
        let request = SubmitScoreRequest {
            user_id: event.user_id.clone(),
            player_name: event.player_name.clone(),
            score: event.score,
            class: event.class.clone(),
            chaos_level: event.chaos_level,
            mobs_killed: event.mobs_killed,
            objs_destroyed: event.objs_destroyed,
        };

        info!("Submitting score to leaderboard: {}", event.score);

        // Spawn async task
        let task = thread_pool.spawn(async move { submit_score_blocking(request) });

        commands.spawn(SubmitScoreTask(task));
    }
}

/// System to handle leaderboard fetch events (spawns async task)
pub fn handle_fetch_leaderboard_event(
    mut commands: Commands,
    mut events: EventReader<FetchLeaderboardEvent>,
    mut cache: ResMut<LeaderboardCache>,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for event in events.iter() {
        info!("=== HANDLE_FETCH_LEADERBOARD_EVENT processing event ===");
        cache.is_loading = true;

        let limit = event.limit;
        let class_filter = event.class_filter.clone();

        info!(
            "Fetching leaderboard (limit: {}), spawning async task",
            limit
        );

        // Spawn async task
        let task =
            thread_pool.spawn(async move { fetch_leaderboard_blocking(limit, class_filter) });

        commands.spawn(FetchLeaderboardTask(task));
    }
}

/// Run condition: only run poll system if there are tasks
fn has_fetch_tasks(tasks: Query<(), With<FetchLeaderboardTask>>) -> bool {
    !tasks.is_empty()
}

/// Run condition: only run poll system if there are tasks
fn has_submit_tasks(tasks: Query<(), With<SubmitScoreTask>>) -> bool {
    !tasks.is_empty()
}

/// System to poll fetch tasks and update cache  
pub fn poll_fetch_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut FetchLeaderboardTask)>,
    mut cache: ResMut<LeaderboardCache>,
) {
    for (entity, mut task) in &mut tasks {
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            match result {
                Ok(response) => {
                    cache.entries = response.entries;
                    cache.last_updated = Some(response.last_updated);
                    cache.is_loading = false;
                    cache.last_error = None;
                    info!(
                        "Leaderboard fetched successfully: {} entries",
                        cache.entries.len()
                    );
                }
                Err(e) => {
                    cache.is_loading = false;
                    cache.last_error = Some(e.clone());
                    warn!("Failed to fetch leaderboard: {}", e);
                }
            }

            if let Some(mut entity_cmd) = commands.get_entity(entity) {
                entity_cmd.despawn();
            }
        }
    }
}

/// System to poll submit tasks
pub fn poll_submit_tasks(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut SubmitScoreTask)>,
    mut last_submitted: ResMut<LastSubmittedScore>,
) {
    for (entity, mut task) in &mut tasks {
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            match result {
                Ok(response) => {
                    info!(
                        "Score submitted! Rank: {:?}, Personal Best: {}",
                        response.rank, response.is_personal_best
                    );
                    // Store the rank for display on game over screen
                    last_submitted.rank = response.rank;
                    last_submitted.is_personal_best = response.is_personal_best;
                }
                Err(e) => {
                    warn!("Failed to submit score: {}", e);
                }
            }

            if let Some(mut entity_cmd) = commands.get_entity(entity) {
                entity_cmd.despawn();
            }
        }
    }
}

/// Auto-fetch leaderboard when entering main menu
pub fn auto_fetch_leaderboard_on_menu(
    mut events: EventWriter<FetchLeaderboardEvent>,
    mut cache: ResMut<LeaderboardCache>,
) {
    info!("=== AUTO_FETCH_LEADERBOARD_ON_MENU CALLED ===");
    info!("Setting cache.is_loading = true");

    // Mark as loading immediately so UI shows loading state
    cache.is_loading = true;

    // Fetch every time we enter the main menu
    events.send(FetchLeaderboardEvent {
        limit: 10,
        class_filter: None,
    });

    info!("Sent FetchLeaderboardEvent");
}

/// Auto-submit score on game over
pub fn auto_submit_score_on_game_over(
    mut game_over_events: EventReader<crate::client::GameOverEvent>,
    mut submit_events: EventWriter<SubmitScoreEvent>,
    run_score: Res<RunScore>,
    game_data: Res<crate::client::GameData>,
    chaos: Res<crate::chaos::ChaosTracker>,
    infinite_mode: Res<crate::night::InfiniteMode>,
    class: Res<PlayerClass>,
    graphics: Res<Graphics>,
    mut last_submitted: ResMut<LastSubmittedScore>,
    cheat_settings: Option<Res<CheatSettings>>,
) {
    let dev_mode = cheat_settings.map(|c| c.dev_mode).unwrap_or(false);
    for _ in game_over_events.iter() {
        // Get player name from game data or use default
        let player_name = game_data
            .player_name
            .clone()
            .unwrap_or_else(|| "Player".to_string());

        info!(
            "Submitting score for player: {} (from GameData resource)",
            player_name
        );

        let class_name = graphics.get_class_data(class.class.clone()).name.clone();

        let score = run_score.score as i32;

        // Include both global chaos and infinite mode chaos bonus
        let total_chaos = chaos.get_chaos() + infinite_mode.get_chaos_bonus();

        // Store the score being submitted (rank will be updated when response arrives)
        last_submitted.score = score;
        last_submitted.rank = None; // Clear previous rank
        last_submitted.is_personal_best = false;
        if !dev_mode {
            submit_events.send(SubmitScoreEvent {
                user_id: game_data.user_id.to_string(),
                player_name: player_name.clone(),
                score,
                class: class_name.clone(),
                chaos_level: total_chaos.trunc() as i32,
                mobs_killed: run_score.mobs_killed as i32,
                objs_destroyed: run_score.objs_destroyed as i32,
            });
        }
    }
}

/// Plugin to register leaderboard systems
pub struct LeaderboardPlugin;

impl Plugin for LeaderboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SubmitScoreEvent>()
            .add_event::<FetchLeaderboardEvent>()
            .init_resource::<LeaderboardCache>()
            .init_resource::<LastSubmittedScore>()
            .add_systems((
                handle_submit_score_event,
                handle_fetch_leaderboard_event,
                poll_fetch_tasks.run_if(has_fetch_tasks),
                poll_submit_tasks.run_if(has_submit_tasks),
            ))
            .add_system(auto_submit_score_on_game_over.in_set(OnUpdate(GameState::Main)))
            .add_system(auto_fetch_leaderboard_on_menu.in_schedule(OnEnter(GameState::MainMenu)));
    }
}
