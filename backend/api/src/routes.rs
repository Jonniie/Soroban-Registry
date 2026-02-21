use axum::{
    routing::{get, post},
    Router,
};

use crate::{auth_handlers, handlers, metrics_handler, resource_handlers, state::AppState};

pub fn observability_routes() -> Router<AppState> {
    Router::new().route("/metrics", get(metrics_handler::metrics_endpoint))
}

pub fn contract_routes() -> Router<AppState> {
    Router::new()
        .route("/api/contracts", get(handlers::list_contracts))
        // .route("/api/contracts/graph", get(handlers::get_contract_graph))
        .route("/api/contracts", post(handlers::publish_contract))
        .route("/api/contracts/:id", get(handlers::get_contract))
        .route("/api/contracts/:id/versions", get(handlers::get_contract_versions))
        .route("/api/contracts/verify", post(handlers::verify_contract))
}

pub fn publisher_routes() -> Router<AppState> {
    Router::new()
        .route("/api/publishers", post(handlers::create_publisher))
        .route("/api/publishers/:id", get(handlers::get_publisher))
        .route(
            "/api/publishers/:id/contracts",
            get(handlers::get_publisher_contracts),
        )
}

pub fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(handlers::health_check))
        .route("/api/stats", get(handlers::get_stats))
}

pub fn auth_routes() -> Router<AppState> {
    Router::new()
        .route("/api/auth/challenge", get(auth_handlers::get_challenge))
        .route("/api/auth/verify", post(auth_handlers::verify_challenge))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/api/contracts", post(handlers::publish_contract))
        .route("/api/contracts/:id/verify", post(handlers::verify_contract))
}

pub fn resource_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/contracts/:id/resources",
            get(resource_handlers::get_contract_resources),
        )
}

// Placeholder routes for modules that are not yet implemented or being refactored
pub fn migration_routes() -> Router<AppState> { Router::new() }
pub fn canary_routes() -> Router<AppState> { Router::new() }
pub fn ab_test_routes() -> Router<AppState> { Router::new() }
pub fn performance_routes() -> Router<AppState> { Router::new() }
