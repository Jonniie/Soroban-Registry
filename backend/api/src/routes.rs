use axum::{
    routing::{get, post, put},
    Router,
};

use crate::{handlers, state::AppState};

pub fn contract_routes() -> Router<AppState> {
    Router::new()
        .route("/api/contracts", get(handlers::list_contracts))
        .route("/api/contracts", post(handlers::publish_contract))
        .route("/api/contracts/:id", get(handlers::get_contract))
        .route("/api/contracts/:id/abi", get(handlers::get_contract_abi))
        .route("/api/contracts/:id/versions", get(handlers::get_contract_versions))
        .route(
            "/api/contracts/:id/versions",
            get(handlers::get_contract_versions),
        )
        .route(
            "/api/contracts/:id/analytics",
            get(handlers::get_contract_analytics),
        )
        .route("/api/contracts/verify", post(handlers::verify_contract))
        .route("/api/contracts/:id/deployments/status", get(handlers::get_deployment_status))
        .route("/api/deployments/green", post(handlers::deploy_green))
        .route("/api/deployments/switch", post(handlers::switch_deployment))
        .route("/api/deployments/:contract_id/rollback", post(handlers::rollback_deployment))
        .route("/api/deployments/health", post(handlers::report_health_check))
        .route("/api/contracts/:id/state/:key", get(handlers::get_contract_state).post(handlers::update_contract_state))
}

/// Publisher-related routes
pub fn publisher_routes() -> Router<AppState> {
    Router::new()
        .route("/api/publishers", post(handlers::create_publisher))
        .route("/api/publishers/:id", get(handlers::get_publisher))
        .route(
            "/api/publishers/:id/contracts",
            get(handlers::get_publisher_contracts),
        )
}

/// Health check routes
pub fn health_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(handlers::health_check))
        .route("/api/stats", get(handlers::get_stats))
        .route("/api/cache/stats", get(handlers::get_cache_stats))
}

/// Migration-related routes
pub fn migration_routes() -> Router<AppState> {
    Router::new()
        .route("/api/migrations", post(handlers::migrations::create_migration).get(handlers::migrations::get_migrations))
        .route("/api/migrations/:id", put(handlers::migrations::update_migration).get(handlers::migrations::get_migration))
}
