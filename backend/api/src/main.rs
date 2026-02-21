mod routes;
mod handlers;
mod error;
mod state;
mod rate_limit;
mod aggregation;
mod auth;
mod auth_handlers;
mod cache;
mod metrics_handler;
mod metrics;
mod resource_handlers;
mod resource_tracking;

use anyhow::Result;
use axum::Router;
use axum::http::{header, HeaderValue, Method};
use dotenv::dotenv;
use prometheus::Registry;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::state::AppState;
use crate::rate_limit::RateLimitState;

#[tokio::main]
async fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    // Run migrations
    sqlx::migrate!("../../database/migrations")
        .run(&pool)
        .await?;

    tracing::info!("Database connected and migrations applied");

    // Create app state
    let registry = Registry::new();
    if let Err(e) = crate::metrics::register_all(&registry) {
        tracing::error!("Failed to register metrics: {}", e);
    }
    let state = AppState::new(pool, registry);
    let _rate_limit_state = RateLimitState::from_env();

    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://localhost:3000"),
            HeaderValue::from_static("https://soroban-registry.vercel.app"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION]);

    // Build router
    let app = Router::new()
        .merge(routes::contract_routes())
        .merge(routes::publisher_routes())
        .merge(routes::health_routes())
        .fallback(handlers::route_not_found)
        .layer(CorsLayer::permissive())
        .layer(cors)
        .with_state(state);

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], 3001));
    tracing::info!("API server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
