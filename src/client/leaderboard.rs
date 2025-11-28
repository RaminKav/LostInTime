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
    GameState,
};

/// Configuration for leaderboard server
const LEADERBOARD_API_URL: &str = "http://localhost:3000/api/leaderboard";

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
        cache.is_loading = true;

        let limit = event.limit;
        let class_filter = event.class_filter.clone();

        info!("Fetching leaderboard (limit: {})", limit);

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
pub fn poll_submit_tasks(mut commands: Commands, mut tasks: Query<(Entity, &mut SubmitScoreTask)>) {
    for (entity, mut task) in &mut tasks {
        if let Some(result) = future::block_on(future::poll_once(&mut task.0)) {
            match result {
                Ok(response) => {
                    info!(
                        "Score submitted! Rank: {:?}, Personal Best: {}",
                        response.rank, response.is_personal_best
                    );
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
    mut has_fetched: Local<bool>,
) {
    // Only fetch once per menu entry
    if !*has_fetched {
        *has_fetched = true;
        events.send(FetchLeaderboardEvent {
            limit: 5,
            class_filter: None,
        });
    }
}

/// Reset the fetch flag when exiting main menu
pub fn reset_fetch_flag_on_menu_exit(mut has_fetched: Local<bool>) {
    *has_fetched = false;
}

/// Auto-submit score on game over
pub fn auto_submit_score_on_game_over(
    mut game_over_events: EventReader<crate::client::GameOverEvent>,
    mut submit_events: EventWriter<SubmitScoreEvent>,
    run_score: Res<RunScore>,
    game_data: Res<crate::client::GameData>,
    chaos: Res<crate::chaos::ChaosTracker>,
    class: Res<PlayerClass>,
    graphics: Res<Graphics>,
) {
    for _ in game_over_events.iter() {
        // Get player name from game data or use default
        let player_name = game_data
            .player_name
            .clone()
            .unwrap_or_else(|| "Player".to_string());
        let class_name = graphics.get_class_data(class.class.clone()).name.clone();
        submit_events.send(SubmitScoreEvent {
            user_id: game_data.user_id.to_string(),
            player_name: player_name.clone(),
            score: run_score.score as i32,
            class: class_name.clone(),
            chaos_level: chaos.get_chaos().trunc() as i32,
            mobs_killed: run_score.mobs_killed as i32,
            objs_destroyed: run_score.objs_destroyed as i32,
        });
    }
}

/// Plugin to register leaderboard systems
pub struct LeaderboardPlugin;

impl Plugin for LeaderboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<SubmitScoreEvent>()
            .add_event::<FetchLeaderboardEvent>()
            .init_resource::<LeaderboardCache>()
            .add_systems((
                handle_submit_score_event,
                handle_fetch_leaderboard_event,
                poll_fetch_tasks.run_if(has_fetch_tasks),
                poll_submit_tasks.run_if(has_submit_tasks),
            ))
            .add_system(auto_submit_score_on_game_over.in_set(OnUpdate(GameState::Main)))
            .add_system(auto_fetch_leaderboard_on_menu.run_if(in_state(GameState::MainMenu)))
            .add_system(reset_fetch_flag_on_menu_exit.in_schedule(OnExit(GameState::MainMenu)));
    }
}
