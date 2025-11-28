use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Request body for submitting a score
#[derive(Debug, Deserialize, Serialize)]
pub struct SubmitScoreRequest {
    pub user_id: String,
    pub player_name: String,
    pub score: i32,
    pub class: String,
    pub chaos_level: i32,
    pub mobs_killed: i32,
    pub objs_destroyed: i32,
}

/// Response after submitting a score
#[derive(Debug, Serialize)]
pub struct SubmitScoreResponse {
    pub success: bool,
    pub rank: Option<i64>,
    pub is_personal_best: bool,
    pub message: String,
}

/// A leaderboard entry from the database
#[derive(Debug, FromRow, Serialize)]
pub struct LeaderboardEntry {
    pub rank: Option<i64>,
    pub player_name: String,
    pub score: i32,
    pub class: String,
    pub chaos_level: i32,
    pub submitted_at: DateTime<Utc>,
}

/// Response for fetching top scores
#[derive(Debug, Serialize)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
    pub last_updated: DateTime<Utc>,
}

/// Player statistics
#[derive(Debug, FromRow, Serialize)]
pub struct PlayerStats {
    pub user_id: String,
    pub best_score: i32,
    pub current_rank: Option<i64>,
    pub total_runs: i32,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

