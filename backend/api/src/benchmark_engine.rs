// api/src/benchmark_engine.rs
// Core benchmarking engine: runs iterations, computes statistics, detects regressions.
// In production this calls the actual Soroban CLI/RPC; here we simulate with
// realistic timing so the full plumbing works end-to-end.

use std::time::{Duration, Instant};

/// Raw timing result from one iteration
#[derive(Debug, Clone)]
pub struct IterationResult {
    pub execution_time_ms: f64,
    pub cpu_instructions: Option<i64>,
    pub memory_bytes: Option<i64>,
}

/// Aggregated statistics from N iterations
#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    pub min_ms: f64,
    pub max_ms: f64,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub stddev_ms: f64,
}

impl BenchmarkStats {
    /// Compute stats from a sorted list of timings
    pub fn compute(mut timings: Vec<f64>) -> Self {
        assert!(
            !timings.is_empty(),
            "Cannot compute stats from empty timings"
        );
        timings.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let n = timings.len();
        let min_ms = timings[0];
        let max_ms = timings[n - 1];
        let avg_ms = timings.iter().sum::<f64>() / n as f64;
        let p95_ms = timings[(n as f64 * 0.95) as usize].min(max_ms);
        let p99_ms = timings[(n as f64 * 0.99) as usize].min(max_ms);

        let variance = timings.iter().map(|t| (t - avg_ms).powi(2)).sum::<f64>() / n as f64;
        let stddev_ms = variance.sqrt();

        BenchmarkStats {
            min_ms,
            max_ms,
            avg_ms,
            p95_ms,
            p99_ms,
            stddev_ms,
        }
    }

    /// Coefficient of variation — lower is more consistent
    pub fn cv(&self) -> f64 {
        if self.avg_ms == 0.0 {
            0.0
        } else {
            self.stddev_ms / self.avg_ms
        }
    }

    /// True if variance is acceptably low (<15% CV)
    pub fn is_consistent(&self) -> bool {
        self.cv() < 0.15
    }
}

/// Variance-stabilised timing: runs a warmup then measures
pub struct BenchmarkRunner {
    pub method: String,
    pub iterations: usize,
    pub warmup_iterations: usize,
}

impl BenchmarkRunner {
    pub fn new(method: String, iterations: usize) -> Self {
        // Warmup = 10% of iterations, min 5, max 20
        let warmup = (iterations / 10).clamp(5, 20);
        Self {
            method,
            iterations,
            warmup_iterations: warmup,
        }
    }

    /// Execute the benchmark. Returns (individual results, stats).
    ///
    /// In production, replace `simulate_invocation` with actual Soroban CLI calls
    /// via `tokio::process::Command` or the Horizon/RPC SDK.
    pub fn run(&self) -> (Vec<IterationResult>, BenchmarkStats) {
        // Warmup — discard results
        for _ in 0..self.warmup_iterations {
            let _ = self.simulate_invocation();
        }

        // Measured iterations
        let mut results = Vec::with_capacity(self.iterations);
        for _ in 0..self.iterations {
            results.push(self.simulate_invocation());
        }

        let timings: Vec<f64> = results.iter().map(|r| r.execution_time_ms).collect();
        let stats = BenchmarkStats::compute(timings);

        (results, stats)
    }

    /// Simulate a Soroban contract invocation.
    ///
    /// Replace with:
    ///   tokio::process::Command::new("soroban")
    ///     .args(["contract", "invoke", "--id", &contract_id, "--", &self.method, ...])
    ///     .output().await
    ///
    /// Then parse execution_time from the `--cost` output or measure wall time.
    fn simulate_invocation(&self) -> IterationResult {
        let base_ms = match self.method.as_str() {
            "transfer" => 12.5,
            "swap" => 18.3,
            "initialize" => 8.1,
            "mint" => 14.2,
            "burn" => 11.7,
            _ => 10.0,
        };

        // Add realistic jitter (±8% with occasional outliers)
        let jitter_pct = (rand_f64() - 0.5) * 0.16;
        let outlier = if rand_f64() < 0.03 {
            rand_f64() * 0.40
        } else {
            0.0
        };
        let time_ms = base_ms * (1.0 + jitter_pct + outlier);

        let start = Instant::now();
        std::thread::sleep(Duration::from_micros((time_ms * 100.0) as u64));
        let _elapsed = start.elapsed();

        IterationResult {
            execution_time_ms: time_ms,
            cpu_instructions: Some((time_ms * 45_000.0) as i64),
            memory_bytes: Some(128 * 1024 + (rand_f64() * 32.0 * 1024.0) as i64),
        }
    }
}

/// Check if current benchmark is a regression vs baseline.
/// Returns (is_regression, regression_pct)
pub fn check_regression(baseline_p95: f64, current_p95: f64, threshold_pct: f64) -> (bool, f64) {
    if baseline_p95 == 0.0 {
        return (false, 0.0);
    }
    let delta_pct = ((current_p95 - baseline_p95) / baseline_p95) * 100.0;
    (delta_pct > threshold_pct, delta_pct)
}

/// Minimal LCG pseudo-random (avoids the `rand` crate dependency)
fn rand_f64() -> f64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    let lcg = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    (lcg >> 33) as f64 / u32::MAX as f64
}

/// CLI output formatter — matches the spec: min/max/avg/p95
pub fn format_cli_output(
    contract_id: &str,
    method: &str,
    iterations: usize,
    stats: &BenchmarkStats,
    alert: Option<&str>,
) -> String {
    let consistency = if stats.is_consistent() {
        "consistent"
    } else {
        " high variance"
    };
    let mut out = String::new();
    out.push_str(&format!(
        "\n╔══ Soroban Registry Benchmark ══════════════════════════╗\n"
    ));
    out.push_str(&format!("  Contract : {}\n", contract_id));
    out.push_str(&format!("  Method   : {}()\n", method));
    out.push_str(&format!(
        "  Runs     : {} iterations + warmup\n",
        iterations
    ));
    out.push_str(&format!(
        "╠══ Timing (ms) ══════════════════════════════════════════╣\n"
    ));
    out.push_str(&format!("  Min      : {:>8.3} ms\n", stats.min_ms));
    out.push_str(&format!("  Max      : {:>8.3} ms\n", stats.max_ms));
    out.push_str(&format!("  Avg      : {:>8.3} ms\n", stats.avg_ms));
    out.push_str(&format!("  p95      : {:>8.3} ms\n", stats.p95_ms));
    out.push_str(&format!("  p99      : {:>8.3} ms\n", stats.p99_ms));
    out.push_str(&format!(
        "  StdDev   : {:>8.3} ms  ({})\n",
        stats.stddev_ms, consistency
    ));
    if let Some(alert_msg) = alert {
        out.push_str(&format!(
            "╠══  REGRESSION ALERT ══════════════════════════════════╣\n"
        ));
        out.push_str(&format!("  {}\n", alert_msg));
    }
    out.push_str(&format!(
        "╚═════════════════════════════════════════════════════════╝\n"
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_are_correct() {
        let timings = vec![10.0, 20.0, 15.0, 12.0, 18.0, 11.0, 30.0, 14.0, 16.0, 13.0];
        let stats = BenchmarkStats::compute(timings);
        assert_eq!(stats.min_ms, 10.0);
        assert_eq!(stats.max_ms, 30.0);
        assert!((stats.avg_ms - 15.9).abs() < 0.1);
        assert!(stats.p95_ms >= stats.p99_ms || stats.p99_ms == stats.max_ms);
    }

    #[test]
    fn regression_detection_works() {
        let (is_reg, pct) = check_regression(10.0, 11.5, 10.0);
        assert!(is_reg);
        assert!((pct - 15.0).abs() < 0.1);

        let (is_reg, _) = check_regression(10.0, 10.5, 10.0);
        assert!(!is_reg); // 5% increase < 10% threshold
    }

    #[test]
    fn consistency_check() {
        // Tight distribution — should be consistent
        let tight: Vec<f64> = (0..100).map(|i| 10.0 + (i % 5) as f64 * 0.1).collect();
        let stats = BenchmarkStats::compute(tight);
        assert!(stats.is_consistent());
    }
}
