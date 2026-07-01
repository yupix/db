use axum::{Router, routing::get};
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod auth;
mod config;
mod db;
mod error;
mod metrics;
mod orchestrator;
mod scheduler;
mod state;
mod util;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,backend=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    tracing::info!("Connecting to database...");

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Running migrations...");
    sqlx::migrate!("./migrations").run(&pool).await?;

    let docker = bollard::Docker::connect_with_local_defaults()?;
    tracing::info!("Docker connection established");

    let state = Arc::new(AppState {
        db: pool,
        docker,
        config: config.clone(),
    });

    // Start background metrics collection (Docker stats → time-series + rollup).
    metrics::spawn(state.clone());
    tracing::info!("Metrics collector started");

    // Start the scheduled-backup + retention-pruning loop.
    scheduler::spawn(state.clone());
    tracing::info!("Backup scheduler started");

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_credentials(true)
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
        ])
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ]);

    let app = Router::new()
        .route("/api/health", get(api::health::health))
        .nest("/api/auth", api::auth::router())
        .nest("/api/organizations", api::organizations::router())
        .nest("/api/projects", api::projects::router())
        .route(
            "/api/projects/{id}/query",
            get(api::query::query_ws_handler),
        )
        .layer(cors)
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Server listening on {}", addr);

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
