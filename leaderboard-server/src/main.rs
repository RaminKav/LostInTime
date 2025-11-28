mod db;
mod handlers;
mod models;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "leaderboard_server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables from .env file (if present)
    dotenv::dotenv().ok();

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/leaderboard".to_string());

    tracing::info!("Connecting to database...");

    // Create database connection pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .connect(&database_url)
        .await?;

    tracing::info!("Running migrations...");

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

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

    // Get port from environment or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Starting server on {}", addr);

    // Start server
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

