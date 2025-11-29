mod db;
mod handlers;
mod models;

use axum::{
    routing::{get, post},
    Router,
};
use shuttle_axum::ShuttleAxum;
use sqlx::PgPool;
use tower_http::cors::{Any, CorsLayer};

#[shuttle_runtime::main]
async fn main(
    #[shuttle_shared_db::Postgres] pool: PgPool,
) -> ShuttleAxum {
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    tracing::info!("Migrations complete");

    // Configure CORS to allow requests from the game client
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build application router
    let app = Router::new()
        .route("/health", get(handlers::health_check))
        .route("/api/leaderboard/submit", post(handlers::submit_score))
        .route("/api/leaderboard/top", get(handlers::fetch_leaderboard))
        .layer(cors)
        .with_state(pool);

    Ok(app.into())
}

