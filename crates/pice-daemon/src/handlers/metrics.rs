//! `pice metrics` handler — show evaluation metrics and cost data.

use anyhow::{Context, Result};
use pice_core::cli::{CommandResponse, MetricsRequest};
use serde::Serialize;
use serde_json::json;

use crate::metrics;
use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

pub async fn run(
    req: MetricsRequest,
    ctx: &DaemonContext,
    _sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    let project_root = ctx.project_root();

    // Open the metrics DB — for a reporting command, errors ARE surfaced
    let db = match metrics::open_metrics_db(project_root)? {
        Some(db) => db,
        None => {
            // No DB yet — return zeros
            if req.json {
                return Ok(CommandResponse::Json {
                    value: json!({
                        "total_evaluations": 0,
                        "total_loops": 0,
                        "pass_rate": 0.0,
                        "avg_score": 0.0,
                    }),
                });
            }
            return Ok(CommandResponse::Text {
                content: "No metrics database found. Run `pice init` first.\n".to_string(),
            });
        }
    };

    if req.csv {
        let csv = format_csv(&db)?;
        return Ok(CommandResponse::Text { content: csv });
    }

    let report = aggregate(&db)?;

    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::to_value(&report).context("failed to serialize metrics report")?,
        })
    } else {
        Ok(CommandResponse::Text {
            content: format_table(&report),
        })
    }
}

// ─── Aggregation logic (ported from pice-cli/src/metrics/aggregator.rs) ────

#[derive(Debug, Clone, Serialize)]
struct MetricsReport {
    total_evaluations: u64,
    total_loops: u64,
    pass_rate: f64,
    avg_score: f64,
    last_30_days: TrendData,
    tier_distribution: TierDistribution,
    top_failing_criteria: Vec<FailingCriterion>,
}

#[derive(Debug, Clone, Serialize)]
struct TrendData {
    evaluations: u64,
    distinct_plans: u64,
    pass_rate: f64,
    avg_score: f64,
}

#[derive(Debug, Clone, Serialize)]
struct TierDistribution {
    tier1: u64,
    tier2: u64,
    tier3: u64,
}

#[derive(Debug, Clone, Serialize)]
struct FailingCriterion {
    name: String,
    fail_count: u64,
    total_count: u64,
}

fn aggregate(db: &metrics::db::MetricsDb) -> Result<MetricsReport> {
    let conn = db.conn();

    let total_evaluations: u64 = conn
        .query_row("SELECT COUNT(*) FROM evaluations", [], |row| row.get(0))
        .context("failed to count evaluations")?;

    let total_loops: u64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT plan_path) FROM evaluations",
            [],
            |row| row.get(0),
        )
        .context("failed to count loops")?;

    let pass_rate = if total_evaluations > 0 {
        let passed: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evaluations WHERE passed = 1",
                [],
                |row| row.get(0),
            )
            .context("failed to count passed")?;
        (passed as f64 / total_evaluations as f64) * 100.0
    } else {
        0.0
    };

    let avg_score: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CAST(score AS REAL)), 0.0) FROM criteria_scores",
            [],
            |row| row.get(0),
        )
        .context("failed to compute avg score")?;

    let last_30_days = aggregate_trend(conn, 30)?;
    let tier_distribution = aggregate_tiers(conn)?;
    let top_failing_criteria = aggregate_failing_criteria(conn, 5)?;

    Ok(MetricsReport {
        total_evaluations,
        total_loops,
        pass_rate,
        avg_score,
        last_30_days,
        tier_distribution,
        top_failing_criteria,
    })
}

fn aggregate_trend(conn: &rusqlite::Connection, days: i64) -> Result<TrendData> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(days);
    let cutoff_str = cutoff.to_rfc3339();

    let evaluations: u64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evaluations WHERE timestamp >= ?1",
            rusqlite::params![cutoff_str],
            |row| row.get(0),
        )
        .context("failed to count recent evaluations")?;

    let pass_rate = if evaluations > 0 {
        let passed: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evaluations WHERE passed = 1 AND timestamp >= ?1",
                rusqlite::params![cutoff_str],
                |row| row.get(0),
            )
            .context("failed to count recent passed")?;
        (passed as f64 / evaluations as f64) * 100.0
    } else {
        0.0
    };

    let distinct_plans: u64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT plan_path) FROM evaluations WHERE timestamp >= ?1",
            rusqlite::params![cutoff_str],
            |row| row.get(0),
        )
        .context("failed to count recent plans")?;

    let avg_score: f64 = conn
        .query_row(
            "SELECT COALESCE(AVG(CAST(cs.score AS REAL)), 0.0) FROM criteria_scores cs JOIN evaluations e ON cs.evaluation_id = e.id WHERE e.timestamp >= ?1",
            rusqlite::params![cutoff_str],
            |row| row.get(0),
        )
        .context("failed to compute recent avg score")?;

    Ok(TrendData {
        evaluations,
        distinct_plans,
        pass_rate,
        avg_score,
    })
}

fn aggregate_tiers(conn: &rusqlite::Connection) -> Result<TierDistribution> {
    let count_tier = |tier: u8| -> Result<u64> {
        conn.query_row(
            "SELECT COUNT(*) FROM evaluations WHERE tier = ?1",
            rusqlite::params![tier],
            |row| row.get(0),
        )
        .with_context(|| format!("failed to count tier {tier}"))
    };
    Ok(TierDistribution {
        tier1: count_tier(1)?,
        tier2: count_tier(2)?,
        tier3: count_tier(3)?,
    })
}

fn aggregate_failing_criteria(
    conn: &rusqlite::Connection,
    limit: usize,
) -> Result<Vec<FailingCriterion>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, SUM(CASE WHEN passed = 0 THEN 1 ELSE 0 END) as fail_count, COUNT(*) as total_count
         FROM criteria_scores GROUP BY name HAVING fail_count > 0 ORDER BY fail_count DESC LIMIT ?1",
        )
        .context("failed to prepare failing criteria query")?;

    let criteria = stmt
        .query_map(rusqlite::params![limit as i64], |row| {
            Ok(FailingCriterion {
                name: row.get(0)?,
                fail_count: row.get(1)?,
                total_count: row.get(2)?,
            })
        })
        .context("failed to query failing criteria")?
        .filter_map(|r| r.ok())
        .collect();
    Ok(criteria)
}

fn format_table(report: &MetricsReport) -> String {
    let mut out = String::new();
    out.push_str("PICE Metrics\n");
    out.push_str("═══════════════════════════════════════\n\n");
    out.push_str(&format!(
        "Total evaluations:   {:>5}\n",
        report.total_evaluations
    ));
    out.push_str(&format!("Total PICE loops:    {:>5}\n", report.total_loops));
    out.push_str(&format!(
        "Pass rate:           {:>4.1}%\n",
        report.pass_rate
    ));
    out.push_str(&format!(
        "Average score:       {:>4.1}/10\n",
        report.avg_score
    ));
    out.push_str(&format!(
        "\nLast 30 days:\n  Evaluations:       {:>5}\n  Pass rate:         {:>4.1}%\n  Average score:     {:>4.1}/10\n",
        report.last_30_days.evaluations, report.last_30_days.pass_rate, report.last_30_days.avg_score
    ));
    out.push_str(&format!(
        "\nTier distribution:\n  Tier 1:            {:>5}\n  Tier 2:            {:>5}\n  Tier 3:            {:>5}\n",
        report.tier_distribution.tier1, report.tier_distribution.tier2, report.tier_distribution.tier3
    ));
    if !report.top_failing_criteria.is_empty() {
        out.push_str("\nMost common failures:\n");
        for c in &report.top_failing_criteria {
            out.push_str(&format!(
                "  {:<20} {}/{} failures\n",
                c.name, c.fail_count, c.total_count
            ));
        }
    }
    out
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        format!("\"{s}\"")
    }
}

fn format_csv(db: &metrics::db::MetricsDb) -> Result<String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT e.id, e.plan_path, e.feature_name, e.tier, e.passed,
                COALESCE(AVG(CAST(cs.score AS REAL)), 0.0) as avg_score, e.timestamp
         FROM evaluations e LEFT JOIN criteria_scores cs ON cs.evaluation_id = e.id
         GROUP BY e.id ORDER BY e.timestamp ASC",
        )
        .context("failed to prepare CSV export query")?;

    let mut csv = String::from("id,plan_path,feature_name,tier,passed,avg_score,timestamp\n");
    let rows = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let plan_path: String = row.get(1)?;
            let feature_name: String = row.get(2)?;
            let tier: u8 = row.get(3)?;
            let passed: i32 = row.get(4)?;
            let avg_score: f64 = row.get(5)?;
            let timestamp: String = row.get(6)?;
            Ok(format!(
                "{id},{},{},{tier},{},{avg_score:.1},{timestamp}",
                csv_escape(&plan_path),
                csv_escape(&feature_name),
                if passed != 0 { "true" } else { "false" }
            ))
        })
        .context("failed to query evaluations for CSV")?;

    for row in rows {
        csv.push_str(&row.context("failed to format CSV row")?);
        csv.push('\n');
    }
    Ok(csv)
}
