use crate::models::{LeaderboardEntry, SubmitScoreRequest};
use sqlx::{PgPool, Row};

/// Submit a score to the leaderboard
pub async fn submit_score(pool: &PgPool, request: &SubmitScoreRequest) -> Result<(), sqlx::Error> {
    // Insert the score
    sqlx::query(
        r#"
        INSERT INTO leaderboard (user_id, player_name, score, class, chaos_level, mobs_killed, objs_destroyed)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (user_id, score, submitted_at) DO NOTHING
        "#
    )
    .bind(&request.user_id)
    .bind(&request.player_name)
    .bind(request.score)
    .bind(&request.class)
    .bind(request.chaos_level)
    .bind(request.mobs_killed)
    .bind(request.objs_destroyed)
    .execute(pool)
    .await?;

    // Update user profile
    sqlx::query(
        r#"
        INSERT INTO users (user_id, last_known_name, total_runs, best_score, last_seen)
        VALUES ($1, $2, 1, $3, NOW())
        ON CONFLICT (user_id) DO UPDATE
        SET last_known_name = EXCLUDED.last_known_name,
            total_runs = users.total_runs + 1,
            best_score = GREATEST(users.best_score, EXCLUDED.best_score),
            last_seen = NOW()
        "#,
    )
    .bind(&request.user_id)
    .bind(&request.player_name)
    .bind(request.score)
    .execute(pool)
    .await?;

    Ok(())
}

/// Get rank for a specific score
pub async fn get_rank_for_score(pool: &PgPool, score: i32) -> Result<i64, sqlx::Error> {
    let result = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT (COUNT(*) + 1)::BIGINT as rank
        FROM leaderboard
        WHERE score > $1
        "#,
    )
    .bind(score)
    .fetch_one(pool)
    .await?;

    Ok(result)
}

/// Check if this is the user's personal best
pub async fn is_personal_best(
    pool: &PgPool,
    user_id: &str,
    score: i32,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        SELECT best_score
        FROM users
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(match result {
        Some(row) => {
            let best: i32 = row.get(0);
            score >= best
        }
        None => true, // First score is always personal best
    })
}

/// Fetch top N scores from the leaderboard
pub async fn fetch_top_scores(
    pool: &PgPool,
    limit: i32,
    class_filter: Option<String>,
) -> Result<Vec<LeaderboardEntry>, sqlx::Error> {
    let query = if let Some(class) = class_filter {
        sqlx::query_as::<_, LeaderboardEntry>(
            r#"
            SELECT 
                ROW_NUMBER() OVER (ORDER BY score DESC, submitted_at ASC)::BIGINT as rank,
                player_name,
                score,
                class,
                chaos_level,
                submitted_at
            FROM leaderboard
            WHERE class = $1
            ORDER BY score DESC, submitted_at ASC
            LIMIT $2
            "#,
        )
        .bind(class)
        .bind(limit)
    } else {
        sqlx::query_as::<_, LeaderboardEntry>(
            r#"
            SELECT 
                ROW_NUMBER() OVER (ORDER BY score DESC, submitted_at ASC)::BIGINT as rank,
                player_name,
                score,
                class,
                chaos_level,
                submitted_at
            FROM leaderboard
            ORDER BY score DESC, submitted_at ASC
            LIMIT $1
            "#,
        )
        .bind(limit)
    };

    query.fetch_all(pool).await
}

/// Get player's best score and current rank
pub async fn fetch_player_stats(
    pool: &PgPool,
    user_id: &str,
) -> Result<Option<(i32, i64, i32)>, sqlx::Error> {
    let result = sqlx::query(
        r#"
        SELECT 
            u.best_score,
            (SELECT (COUNT(*) + 1)::BIGINT FROM leaderboard WHERE score > u.best_score) as rank,
            u.total_runs
        FROM users u
        WHERE u.user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|row| (row.get(0), row.get(1), row.get(2))))
}
