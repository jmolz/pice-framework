//! `pice benchmark` handler — before/after workflow effectiveness comparison.

use anyhow::Result;
use pice_core::cli::{BenchmarkRequest, CommandResponse};
use serde_json::json;

use crate::metrics;
use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

pub async fn run(
    req: BenchmarkRequest,
    ctx: &DaemonContext,
    _sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    let project_root = ctx.project_root();

    // Git stats
    let total_commits = count_git_commits(project_root);
    let coverage_pct = get_coverage_pct(project_root);

    // Metrics stats
    let (total_evaluations, pass_rate, avg_score) =
        if let Ok(Some(db)) = metrics::open_metrics_db(project_root) {
            let conn = db.conn();
            let total: u64 = conn
                .query_row("SELECT COUNT(*) FROM evaluations", [], |row| row.get(0))
                .unwrap_or(0);
            let passed: u64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM evaluations WHERE passed = 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let avg: f64 = conn
                .query_row(
                    "SELECT COALESCE(AVG(CAST(score AS REAL)), 0.0) FROM criteria_scores",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);
            let rate = if total > 0 {
                (passed as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            (total, rate, avg)
        } else {
            (0, 0.0, 0.0)
        };

    if req.json {
        Ok(CommandResponse::Json {
            value: json!({
                "total_commits": total_commits,
                "coverage_pct": coverage_pct,
                "total_evaluations": total_evaluations,
                "pass_rate": pass_rate,
                "avg_score": avg_score,
            }),
        })
    } else {
        let mut output = String::new();
        output.push_str("PICE Benchmark\n");
        output.push_str("═══════════════════════════════════════\n\n");
        output.push_str(&format!("Total commits:       {:>5}\n", total_commits));
        output.push_str(&format!("Test coverage:       {:>4.1}%\n", coverage_pct));
        output.push_str(&format!(
            "Total evaluations:   {:>5}\n",
            total_evaluations
        ));
        output.push_str(&format!("Pass rate:           {:>4.1}%\n", pass_rate));
        output.push_str(&format!("Average score:       {:>4.1}/10\n", avg_score));
        Ok(CommandResponse::Text { content: output })
    }
}

fn count_git_commits(project_root: &std::path::Path) -> u64 {
    std::process::Command::new("git")
        .args(["rev-list", "--count", "HEAD"])
        .current_dir(project_root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok())
        .unwrap_or(0)
}

fn get_coverage_pct(project_root: &std::path::Path) -> f64 {
    // Check for coverage reports in common locations
    let coverage_files = [
        "coverage/lcov.info",
        "target/coverage/lcov.info",
        ".coverage",
    ];
    for file in &coverage_files {
        let path = project_root.join(file);
        if path.exists() {
            // For lcov.info, parse line coverage
            if file.ends_with("lcov.info") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let mut hit = 0u64;
                    let mut total = 0u64;
                    for line in content.lines() {
                        if let Some(rest) = line.strip_prefix("DA:") {
                            total += 1;
                            if let Some(count_str) = rest.split(',').nth(1) {
                                if count_str.trim().parse::<u64>().unwrap_or(0) > 0 {
                                    hit += 1;
                                }
                            }
                        }
                    }
                    if total > 0 {
                        return (hit as f64 / total as f64) * 100.0;
                    }
                }
            }
        }
    }
    0.0
}
