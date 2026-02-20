use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use shared::{
    Contract, ContractSearchParams, ContractVersion, PaginatedResponse, PaginatedVersionResponse,
    PublishRequest, Publisher, VersionPaginationParams, VerifyRequest,
};
use uuid::Uuid;

use crate::state::AppState;

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}

/// Get registry statistics
pub async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let total_contracts: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM contracts")
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let verified_contracts: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM contracts WHERE is_verified = true")
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total_publishers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM publishers")
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({
        "total_contracts": total_contracts,
        "verified_contracts": verified_contracts,
        "total_publishers": total_publishers,
    })))
}

/// List and search contracts
pub async fn list_contracts(
    State(state): State<AppState>,
    Query(params): Query<ContractSearchParams>,
) -> Result<Json<PaginatedResponse<Contract>>, StatusCode> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;

    // Build dynamic query based on filters
    let mut query = String::from("SELECT * FROM contracts WHERE 1=1");
    let mut count_query = String::from("SELECT COUNT(*) FROM contracts WHERE 1=1");

    if let Some(ref q) = params.query {
        let search_clause = format!(" AND (name ILIKE '%{}%' OR description ILIKE '%{}%')", q, q);
        query.push_str(&search_clause);
        count_query.push_str(&search_clause);
    }

    if let Some(verified) = params.verified_only {
        if verified {
            query.push_str(" AND is_verified = true");
            count_query.push_str(" AND is_verified = true");
        }
    }

    if let Some(ref category) = params.category {
        let category_clause = format!(" AND category = '{}'", category);
        query.push_str(&category_clause);
        count_query.push_str(&category_clause);
    }

    query.push_str(&format!(
        " ORDER BY created_at DESC LIMIT {} OFFSET {}",
        page_size, offset
    ));

    let contracts: Vec<Contract> = sqlx::query_as(&query)
        .fetch_all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let total: i64 = sqlx::query_scalar(&count_query)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(PaginatedResponse::new(
        contracts, total, page, page_size,
    )))
}

/// Get a specific contract by ID
pub async fn get_contract(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Contract>, StatusCode> {
    let contract: Contract = sqlx::query_as("SELECT * FROM contracts WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(contract))
}

// NEW — paginated version
pub async fn get_contract_versions(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(params): Query<VersionPaginationParams>,
) -> Result<Json<PaginatedVersionResponse>, (StatusCode, Json<serde_json::Value>)> {

    // ── Validate params ──────────────────────────────────
    if params.limit == 0 || params.limit > 100 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "INVALID_LIMIT",
                "message": "limit must be between 1 and 100"
            })),
        ));
    }
    if params.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "INVALID_OFFSET",
                "message": "offset must be 0 or greater"
            })),
        ));
    }

    // ── Check contract exists → 404 if not ───────────────
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM contracts WHERE id = $1)"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": "DB_ERROR", "message": "Database error"}))
    ))?;

    if !exists {
        return Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "NOT_FOUND",
                "message": format!("Contract {} not found", id)
            })),
        ));
    }

    // ── Fetch total count ────────────────────────────────
    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM contract_versions WHERE contract_id = $1"
    )
    .bind(id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": "DB_ERROR", "message": "Database error"}))
    ))?;

    // ── Fetch page ───────────────────────────────────────
    let versions: Vec<ContractVersion> = sqlx::query_as(
        "SELECT * FROM contract_versions
         WHERE contract_id = $1
         ORDER BY created_at DESC
         LIMIT $2 OFFSET $3"
    )
    .bind(id)
    .bind(params.limit)
    .bind(params.offset)
    .fetch_all(&state.db)
    .await
    .map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({"error": "DB_ERROR", "message": "Database error"}))
    ))?;

    Ok(Json(PaginatedVersionResponse {
        items: versions,
        total,
        limit: params.limit,
        offset: params.offset,
    }))
}
/// Publish a new contract
pub async fn publish_contract(
    State(state): State<AppState>,
    Json(req): Json<PublishRequest>,
) -> Result<Json<Contract>, StatusCode> {
    // First, ensure publisher exists or create one
    let publisher: Publisher = sqlx::query_as(
        "INSERT INTO publishers (stellar_address) VALUES ($1)
         ON CONFLICT (stellar_address) DO UPDATE SET stellar_address = EXCLUDED.stellar_address
         RETURNING *",
    )
    .bind(&req.publisher_address)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // TODO: Fetch WASM hash from Stellar network
    let wasm_hash = "placeholder_hash".to_string();

    // Insert contract
    let contract: Contract = sqlx::query_as(
        "INSERT INTO contracts (contract_id, wasm_hash, name, description, publisher_id, network, category, tags)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING *"
    )
        .bind(&req.contract_id)
        .bind(&wasm_hash)
        .bind(&req.name)
        .bind(&req.description)
        .bind(publisher.id)
        .bind(&req.network)
        .bind(&req.category)
        .bind(&req.tags)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(contract))
}

/// Verify a contract
pub async fn verify_contract(
    State(_state): State<AppState>,
    Json(_req): Json<VerifyRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // TODO: Implement verification logic
    Ok(Json(serde_json::json!({
        "status": "pending",
        "message": "Verification started"
    })))
}

/// Create a publisher
pub async fn create_publisher(
    State(state): State<AppState>,
    Json(publisher): Json<Publisher>,
) -> Result<Json<Publisher>, StatusCode> {
    let created: Publisher = sqlx::query_as(
        "INSERT INTO publishers (stellar_address, username, email, github_url, website)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING *",
    )
    .bind(&publisher.stellar_address)
    .bind(&publisher.username)
    .bind(&publisher.email)
    .bind(&publisher.github_url)
    .bind(&publisher.website)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(created))
}

/// Get publisher by ID
pub async fn get_publisher(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Publisher>, StatusCode> {
    let publisher: Publisher = sqlx::query_as("SELECT * FROM publishers WHERE id = $1")
        .bind(id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(publisher))
}

/// Get all contracts by a publisher
pub async fn get_publisher_contracts(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Contract>>, StatusCode> {
    let contracts: Vec<Contract> =
        sqlx::query_as("SELECT * FROM contracts WHERE publisher_id = $1 ORDER BY created_at DESC")
            .bind(id)
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(contracts))
}
