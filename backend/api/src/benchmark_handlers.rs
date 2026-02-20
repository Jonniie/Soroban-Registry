// api/src/benchmark_handlers.rs
// Axum handlers for contract benchmarking.
// Follows the same patterns as audit_handlers.rs.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    benchmark_engine::{check_regression, format_cli_output, BenchmarkRunner, BenchmarkStats},
    state::AppState,
};
use shared::models::{
    BenchmarkComparison, BenchmarkRecord, BenchmarkResponse, BenchmarkRun, BenchmarkStatus,
    BenchmarkTrendPoint, ContractBenchmarkSummary, PerformanceAlert, RunBenchmarkRequest,
};

// ─────────────────────────────────────────────────────────
// POST /api/contracts/:id/benchmarks
// Runs N iterations of a method and persists results.
// ─────────────────────────────────────────────────────────
pub async fn run_benchmark(
    State(state): State<AppState>,
    Path(contract_id): Path<Uuid>,
    Json(req): Json<RunBenchmarkRequest>,
) -> Result<Json<BenchmarkResponse>, StatusCode> {
    // Validate contract exists
    let (contract_name,): (String,) = sqlx::query_as("SELECT name FROM contracts WHERE id = $1")
        .bind(contract_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let iterations = req.iterations.clamp(1, 1000) as usize;
    let version = req.version.as_deref().unwrap_or("unknown");

    // Create pending record
    let benchmark: BenchmarkRecord = sqlx::query_as(
        r#"INSERT INTO benchmark_records
               (contract_id, contract_version, method_name, iterations, args_json, status)
           VALUES ($1, $2, $3, $4, $5, 'pending')
           RETURNING *"#,
    )
    .bind(contract_id)
    .bind(version)
    .bind(&req.method)
    .bind(req.iterations)
    .bind(&req.args_json)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Mark as running
    sqlx::query("UPDATE benchmark_records SET status = 'running' WHERE id = $1")
        .bind(benchmark.id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // --- Run the benchmark (blocking; move to spawn_blocking in production) ---
    let runner = BenchmarkRunner::new(req.method.clone(), iterations);
    let (raw_results, stats) = runner.run();

    // Persist individual runs
    for (i, result) in raw_results.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO benchmark_runs
                   (benchmark_id, iteration, execution_time_ms, cpu_instructions, memory_bytes)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(benchmark.id)
        .bind(i as i32 + 1)
        .bind(result.execution_time_ms)
        .bind(result.cpu_instructions)
        .bind(result.memory_bytes)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Update record with computed stats
    let benchmark: BenchmarkRecord = sqlx::query_as(
        r#"UPDATE benchmark_records
           SET status        = 'completed',
               min_ms        = $1,
               max_ms        = $2,
               avg_ms        = $3,
               p95_ms        = $4,
               p99_ms        = $5,
               stddev_ms     = $6,
               completed_at  = NOW()
           WHERE id = $7
           RETURNING *"#,
    )
    .bind(stats.min_ms)
    .bind(stats.max_ms)
    .bind(stats.avg_ms)
    .bind(stats.p95_ms)
    .bind(stats.p99_ms)
    .bind(stats.stddev_ms)
    .bind(benchmark.id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Compare vs previous baseline for same method
    let maybe_previous: Option<BenchmarkRecord> = sqlx::query_as(
        r#"SELECT * FROM benchmark_records
           WHERE contract_id = $1
             AND method_name = $2
             AND status = 'completed'
             AND id != $3
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .bind(contract_id)
    .bind(&req.method)
    .bind(benchmark.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (comparison, alert) = if let Some(prev) = &maybe_previous {
        let comp = BenchmarkComparison {
            previous_benchmark_id: prev.id,
            previous_version: prev.contract_version.clone(),
            previous_p95_ms: prev.p95_ms,
            current_p95_ms: benchmark.p95_ms,
            delta_ms: benchmark.p95_ms - prev.p95_ms,
            delta_pct: ((benchmark.p95_ms - prev.p95_ms) / prev.p95_ms) * 100.0,
            is_regression: false, // set below
        };

        let (is_regression, regression_pct) =
            check_regression(prev.p95_ms, benchmark.p95_ms, req.alert_threshold_pct);

        let comp = BenchmarkComparison {
            is_regression,
            ..comp
        };

        let maybe_alert = if is_regression {
            let alert: PerformanceAlert = sqlx::query_as(
                r#"INSERT INTO performance_alerts
                       (contract_id, method_name, baseline_benchmark_id, current_benchmark_id,
                        baseline_p95_ms, current_p95_ms, regression_pct, alert_threshold_pct)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                   RETURNING *"#,
            )
            .bind(contract_id)
            .bind(&req.method)
            .bind(prev.id)
            .bind(benchmark.id)
            .bind(prev.p95_ms)
            .bind(benchmark.p95_ms)
            .bind(regression_pct)
            .bind(req.alert_threshold_pct)
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            tracing::warn!(
                contract_id = %contract_id,
                method = %req.method,
                regression_pct = %regression_pct,
                "Performance regression detected"
            );
            Some(alert)
        } else {
            None
        };

        (Some(comp), maybe_alert)
    } else {
        (None, None)
    };

    // Fetch persisted runs for response
    let runs: Vec<BenchmarkRun> =
        sqlx::query_as("SELECT * FROM benchmark_runs WHERE benchmark_id = $1 ORDER BY iteration")
            .bind(benchmark.id)
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!(
        benchmark_id = %benchmark.id,
        method = %req.method,
        p95_ms = %benchmark.p95_ms,
        "Benchmark completed"
    );

    Ok(Json(BenchmarkResponse {
        benchmark,
        runs,
        alert,
        comparison,
    }))
}

// ─────────────────────────────────────────────────────────
// GET /api/contracts/:id/benchmarks
// List all benchmarks for a contract.
// ─────────────────────────────────────────────────────────
pub async fn list_benchmarks(
    State(state): State<AppState>,
    Path(contract_id): Path<Uuid>,
    Query(params): Query<ListBenchmarksParams>,
) -> Result<Json<Vec<BenchmarkRecord>>, StatusCode> {
    let limit = params.limit.unwrap_or(20).clamp(1, 100);
    let method_filter = params.method.as_deref().unwrap_or("%");

    let records: Vec<BenchmarkRecord> = sqlx::query_as(
        r#"SELECT * FROM benchmark_records
           WHERE contract_id = $1
             AND method_name LIKE $2
           ORDER BY created_at DESC
           LIMIT $3"#,
    )
    .bind(contract_id)
    .bind(method_filter)
    .bind(limit as i64)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(records))
}

#[derive(Debug, Deserialize)]
pub struct ListBenchmarksParams {
    pub method: Option<String>,
    pub limit: Option<usize>,
}

// ─────────────────────────────────────────────────────────
// GET /api/contracts/:id/benchmarks/:benchmark_id
// Get a single benchmark with all run data.
// ─────────────────────────────────────────────────────────
pub async fn get_benchmark(
    State(state): State<AppState>,
    Path((contract_id, benchmark_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<BenchmarkResponse>, StatusCode> {
    let benchmark: BenchmarkRecord =
        sqlx::query_as("SELECT * FROM benchmark_records WHERE id = $1 AND contract_id = $2")
            .bind(benchmark_id)
            .bind(contract_id)
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

    let runs: Vec<BenchmarkRun> =
        sqlx::query_as("SELECT * FROM benchmark_runs WHERE benchmark_id = $1 ORDER BY iteration")
            .bind(benchmark_id)
            .fetch_all(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let alert: Option<PerformanceAlert> =
        sqlx::query_as("SELECT * FROM performance_alerts WHERE current_benchmark_id = $1 LIMIT 1")
            .bind(benchmark_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(BenchmarkResponse {
        benchmark,
        runs,
        alert,
        comparison: None,
    }))
}

// ─────────────────────────────────────────────────────────
// GET /api/contracts/:id/benchmarks/trend?method=transfer
// Returns time-series data for the dashboard chart.
// ─────────────────────────────────────────────────────────
pub async fn get_benchmark_trend(
    State(state): State<AppState>,
    Path(contract_id): Path<Uuid>,
    Query(params): Query<TrendParams>,
) -> Result<Json<Vec<BenchmarkTrendPoint>>, StatusCode> {
    let method = params.method.as_deref().unwrap_or("%");

    let trend: Vec<BenchmarkTrendPoint> = sqlx::query_as(
        r#"SELECT
               id AS benchmark_id,
               contract_version AS version,
               created_at,
               p95_ms,
               avg_ms,
               min_ms,
               max_ms
           FROM benchmark_records
           WHERE contract_id = $1
             AND method_name LIKE $2
             AND status = 'completed'
           ORDER BY created_at ASC
           LIMIT 200"#,
    )
    .bind(contract_id)
    .bind(method)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(trend))
}

#[derive(Debug, Deserialize)]
pub struct TrendParams {
    pub method: Option<String>,
}

// ─────────────────────────────────────────────────────────
// GET /api/contracts/:id/benchmarks/summary
// Dashboard summary: methods benchmarked, latest results, active alerts.
// ─────────────────────────────────────────────────────────
pub async fn get_benchmark_summary(
    State(state): State<AppState>,
    Path(contract_id): Path<Uuid>,
) -> Result<Json<ContractBenchmarkSummary>, StatusCode> {
    let (total_benchmarks,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM benchmark_records WHERE contract_id = $1 AND status = 'completed'",
    )
    .bind(contract_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let methods: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT method_name FROM benchmark_records WHERE contract_id = $1 AND status = 'completed' ORDER BY method_name",
    )
    .bind(contract_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let latest_benchmarks: Vec<BenchmarkRecord> = sqlx::query_as(
        r#"SELECT DISTINCT ON (method_name) *
           FROM benchmark_records
           WHERE contract_id = $1 AND status = 'completed'
           ORDER BY method_name, created_at DESC"#,
    )
    .bind(contract_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let active_alerts: Vec<PerformanceAlert> = sqlx::query_as(
        "SELECT * FROM performance_alerts WHERE contract_id = $1 AND resolved = false ORDER BY created_at DESC",
    )
    .bind(contract_id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ContractBenchmarkSummary {
        contract_id,
        total_benchmarks,
        methods_benchmarked: methods.into_iter().map(|(m,)| m).collect(),
        latest_benchmarks,
        active_alerts,
    }))
}

// ─────────────────────────────────────────────────────────
// POST /api/contracts/:id/benchmarks/alerts/:alert_id/resolve
// ─────────────────────────────────────────────────────────
pub async fn resolve_alert(
    State(state): State<AppState>,
    Path((contract_id, alert_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, StatusCode> {
    let rows = sqlx::query(
        "UPDATE performance_alerts SET resolved = true WHERE id = $1 AND contract_id = $2",
    )
    .bind(alert_id)
    .bind(contract_id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .rows_affected();

    if rows == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

// ─────────────────────────────────────────────────────────
// GET /api/contracts/:id/benchmarks/cli-output/:benchmark_id
// Returns the CLI-style formatted output for a completed benchmark.
// ─────────────────────────────────────────────────────────
pub async fn get_cli_output(
    State(state): State<AppState>,
    Path((contract_id, benchmark_id)): Path<(Uuid, Uuid)>,
) -> Result<String, StatusCode> {
    let benchmark: BenchmarkRecord =
        sqlx::query_as("SELECT * FROM benchmark_records WHERE id = $1 AND contract_id = $2")
            .bind(benchmark_id)
            .bind(contract_id)
            .fetch_one(&state.db)
            .await
            .map_err(|_| StatusCode::NOT_FOUND)?;

    if benchmark.status != BenchmarkStatus::Completed {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let stats = BenchmarkStats {
        min_ms: benchmark.min_ms,
        max_ms: benchmark.max_ms,
        avg_ms: benchmark.avg_ms,
        p95_ms: benchmark.p95_ms,
        p99_ms: benchmark.p99_ms,
        stddev_ms: benchmark.stddev_ms,
    };

    let alert_msg: Option<String> = sqlx::query_scalar(
        r#"SELECT CONCAT('p95 increased ', ROUND(regression_pct::numeric, 1), '% (', 
                         ROUND(baseline_p95_ms::numeric, 2), 'ms → ',
                         ROUND(current_p95_ms::numeric, 2), 'ms)')
           FROM performance_alerts WHERE current_benchmark_id = $1 LIMIT 1"#,
    )
    .bind(benchmark_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .flatten();

    Ok(format_cli_output(
        &contract_id.to_string(),
        &benchmark.method_name,
        benchmark.iterations as usize,
        &stats,
        alert_msg.as_deref(),
    ))
}
