// api/src/benchmark_routes.rs
// Benchmark route definitions.
// Merge into the main Axum router alongside existing routes.

use axum::{
    routing::{get, post},
    Router,
};

use crate::{benchmark_handlers, state::AppState};

/// All contract benchmarking routes.
///
/// Add to main.rs router setup:
///
/// ```rust
/// .merge(benchmark_routes::benchmark_routes())
/// ```
pub fn benchmark_routes() -> Router<AppState> {
    Router::new()
        // ── Run a new benchmark ────────────────────────────────────────────
        // CLI equivalent: soroban-registry benchmark {id} --method=transfer --iterations=100
        .route(
            "/api/contracts/:id/benchmarks",
            post(benchmark_handlers::run_benchmark).get(benchmark_handlers::list_benchmarks),
        )
        // ── Dashboard summary (latest per method + active alerts) ──────────
        .route(
            "/api/contracts/:id/benchmarks/summary",
            get(benchmark_handlers::get_benchmark_summary),
        )
        // ── Performance trend for charting ─────────────────────────────────
        // ?method=transfer  returns time-series of p95/avg
        .route(
            "/api/contracts/:id/benchmarks/trend",
            get(benchmark_handlers::get_benchmark_trend),
        )
        // ── Single benchmark detail with run-level data ────────────────────
        .route(
            "/api/contracts/:id/benchmarks/:benchmark_id",
            get(benchmark_handlers::get_benchmark),
        )
        // ── CLI-formatted plaintext output ─────────────────────────────────
        .route(
            "/api/contracts/:id/benchmarks/:benchmark_id/cli-output",
            get(benchmark_handlers::get_cli_output),
        )
        // ── Resolve a performance alert ────────────────────────────────────
        .route(
            "/api/contracts/:id/benchmarks/alerts/:alert_id/resolve",
            post(benchmark_handlers::resolve_alert),
        )
}
