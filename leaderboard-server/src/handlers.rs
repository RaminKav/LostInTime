use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::PgPool;

use crate::{
    db,
    models::{ErrorResponse, LeaderboardResponse, SubmitScoreRequest, SubmitScoreResponse},
};

/// Query parameters for fetching leaderboard
#[derive(Debug, Deserialize)]
pub struct LeaderboardQuery {
    #[serde(default = "default_limit")]
    pub limit: i32,
    pub class: Option<String>,
}

fn default_limit() -> i32 {
    5
}

/// POST /api/leaderboard/submit
/// Submit a new score to the leaderboard
pub async fn submit_score(
    State(pool): State<PgPool>,
    Json(request): Json<SubmitScoreRequest>,
) -> Result<Json<SubmitScoreResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate input
    if request.player_name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Player name cannot be empty".to_string(),
            }),
        ));
    }

    if request.score < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Score cannot be negative".to_string(),
            }),
        ));
    }

    // Submit score to database
    match db::submit_score(&pool, &request).await {
        Ok(_) => {
            // Get rank for this score
            let rank = db::get_rank_for_score(&pool, request.score).await.ok();

            // Check if personal best
            let is_personal_best = db::is_personal_best(&pool, &request.user_id, request.score)
                .await
                .unwrap_or(false);

            Ok(Json(SubmitScoreResponse {
                success: true,
                rank,
                is_personal_best,
                message: "Score submitted successfully".to_string(),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to submit score: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to submit score".to_string(),
                }),
            ))
        }
    }
}

/// GET /api/leaderboard/top
/// Fetch top N scores from the leaderboard
pub async fn fetch_leaderboard(
    State(pool): State<PgPool>,
    Query(params): Query<LeaderboardQuery>,
) -> Result<Json<LeaderboardResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Limit to max 100 to prevent abuse
    let limit = params.limit.min(100).max(1);

    match db::fetch_top_scores(&pool, limit, params.class).await {
        Ok(entries) => Ok(Json(LeaderboardResponse {
            entries,
            last_updated: Utc::now(),
        })),
        Err(e) => {
            tracing::error!("Failed to fetch leaderboard: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to fetch leaderboard".to_string(),
                }),
            ))
        }
    }
}

/// GET /health
/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}

