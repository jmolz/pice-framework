//! `pice audit gates` handler — export the `gate_decisions` audit
//! table as CSV or JSON.
//!
//! `--csv` and `--json` are orthogonal output-format knobs:
//! - `--csv`      → human-importable spreadsheet format on stdout.
//! - `--json`     → single JSON object `{"decisions": [...]}` on stdout.
//! - default      → human table on stdout.
//!
//! Both `--csv` and `--json` suppress the human-friendly header line
//! so the output stream is pipe-clean (matching the rule applied to
//! every other `--json` handler in the daemon).

use anyhow::Result;
use pice_core::cli::{AuditRequest, AuditSubcommand, CommandResponse};

use crate::metrics::{aggregator, db::MetricsDb, store::GateDecisionsFilter};
use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

/// Compute the metrics DB path for this project and open a read-only
/// handle. Handlers that only query (like this one) don't need the
/// write-side `Arc<Mutex<MetricsDb>>` the adaptive loop uses — a fresh
/// `MetricsDb::open` per invocation is fine (SQLite handles concurrent
/// readers cleanly under WAL mode, set by the daemon's `init()`).
fn open_db(ctx: &DaemonContext) -> Result<MetricsDb> {
    let db_path = ctx.project_root().join(".pice").join("metrics.db");
    MetricsDb::open(&db_path)
}

/// Return `true` iff `.pice/metrics.db` is present on disk. Used by
/// the audit handler to distinguish "uninitialized project" (fresh-repo
/// absence — return empty result) from "DB exists but open/migrate
/// failed" (corruption, permission error, schema mismatch — surface the
/// error so operators can repair the system). Phase 6 Codex review #4.
fn metrics_db_exists(ctx: &DaemonContext) -> bool {
    ctx.project_root()
        .join(".pice")
        .join("metrics.db")
        .is_file()
}

pub async fn run(
    req: AuditRequest,
    ctx: &DaemonContext,
    _sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    match req.subcommand {
        AuditSubcommand::Gates {
            feature_id,
            since,
            csv,
        } => run_gates(ctx, req.json, feature_id, since, csv).await,
    }
}

async fn run_gates(
    ctx: &DaemonContext,
    json_mode: bool,
    feature_id: Option<String>,
    since: Option<String>,
    csv_mode: bool,
) -> Result<CommandResponse> {
    // Phase 6 Codex review finding #4: only treat a *missing* DB file as
    // the "fresh-repo, no audit yet" signal. If the file exists but
    // `MetricsDb::open` fails (corruption, permission, schema mismatch,
    // locked WAL), surface the error — swallowing it as empty history
    // masks data loss. `MetricsDb::open` runs v1→v4 migrations on open,
    // so an open failure is a genuine problem for the operator.
    if !metrics_db_exists(ctx) {
        tracing::debug!("metrics DB absent; returning empty audit result");
        return empty_response(json_mode, csv_mode);
    }
    let db = open_db(ctx).map_err(|e| {
        tracing::error!(error = %e, "audit: failed to open existing metrics DB");
        e.context("failed to open metrics.db for audit query")
    })?;

    let filter = GateDecisionsFilter {
        feature_id,
        since,
        limit: None,
    };

    if csv_mode {
        let csv = aggregator::format_gate_decisions_csv(&db, &filter)?;
        return Ok(CommandResponse::Text { content: csv });
    }
    if json_mode {
        let arr = aggregator::format_gate_decisions_json(&db, &filter)?;
        return Ok(CommandResponse::Json {
            value: serde_json::json!({ "decisions": arr }),
        });
    }

    // Human-readable default: render as a simple table.
    let rows = crate::metrics::store::query_gate_decisions(&db, &filter)?;
    if rows.is_empty() {
        return Ok(CommandResponse::Text {
            content: "No gate decisions recorded.\n".to_string(),
        });
    }
    let mut out = String::from("Gate decisions:\n\n");
    out.push_str(&format!(
        "{:<6} {:<24} {:<18} {:<14} {:<10} {:<12}\n",
        "id", "gate_id", "feature", "layer", "decision", "reviewer"
    ));
    out.push_str(&"-".repeat(90));
    out.push('\n');
    for r in rows {
        out.push_str(&format!(
            "{:<6} {:<24} {:<18} {:<14} {:<10} {:<12}\n",
            r.id,
            truncate(&r.gate_id, 24),
            truncate(&r.feature_id, 18),
            truncate(&r.layer, 14),
            r.decision,
            r.reviewer.as_deref().unwrap_or("-"),
        ));
    }
    Ok(CommandResponse::Text { content: out })
}

fn empty_response(json_mode: bool, csv_mode: bool) -> Result<CommandResponse> {
    if csv_mode {
        return Ok(CommandResponse::Text {
            content: "id,gate_id,feature_id,layer,trigger_expression,decision,reviewer,reason,\
                      requested_at,decided_at,elapsed_seconds\n"
                .to_string(),
        });
    }
    if json_mode {
        return Ok(CommandResponse::Json {
            value: serde_json::json!({ "decisions": [] }),
        });
    }
    Ok(CommandResponse::Text {
        content: "No gate decisions recorded.\n".to_string(),
    })
}

/// Truncate a string to `max` chars for the human table, padding is
/// handled by `{:<n}` at format time.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut taken: String = s.chars().take(max.saturating_sub(1)).collect();
        taken.push('…');
        taken
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::store::{insert_gate_decision, GateDecisionRow};
    use pice_core::cli::AuditRequest;

    /// Seed two gate_decisions rows into a context-local metrics DB.
    fn seed_db(ctx: &DaemonContext) {
        let db = open_db(ctx).unwrap();
        insert_gate_decision(
            &db,
            &GateDecisionRow {
                gate_id: "feat:infra:01",
                feature_id: "feat",
                layer: "infra",
                trigger_expression: "layer == infra",
                decision: "approve",
                reviewer: Some("jacob"),
                reason: None,
                requested_at: "2026-04-20T00:00:00Z",
                decided_at: "2026-04-20T00:05:00Z",
                elapsed_seconds: 300,
            },
        )
        .unwrap();
        insert_gate_decision(
            &db,
            &GateDecisionRow {
                gate_id: "feat2:deploy:01",
                feature_id: "feat2",
                layer: "deploy",
                trigger_expression: "tier >= 3",
                decision: "reject",
                reviewer: Some("alice"),
                reason: Some("needs perf review"),
                requested_at: "2026-04-20T02:00:00Z",
                decided_at: "2026-04-20T02:10:00Z",
                elapsed_seconds: 600,
            },
        )
        .unwrap();
    }

    /// Set up a `DaemonContext` rooted at a temp dir so each test has
    /// its own metrics DB.
    fn test_ctx(tmp: &tempfile::TempDir) -> DaemonContext {
        std::fs::create_dir_all(tmp.path().join(".pice")).unwrap();
        DaemonContext::new_for_test_with_root("tok", tmp.path().to_path_buf())
    }

    #[tokio::test]
    async fn empty_db_returns_empty_csv_header() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(&tmp);
        let req = AuditRequest {
            subcommand: AuditSubcommand::Gates {
                feature_id: None,
                since: None,
                csv: true,
            },
            json: false,
        };
        let resp = run(req, &ctx, &crate::orchestrator::NullSink)
            .await
            .unwrap();
        match resp {
            CommandResponse::Text { content } => {
                assert!(content.starts_with("id,gate_id,"));
                // Only the header line — no data rows.
                let lines: Vec<&str> = content.lines().collect();
                assert_eq!(lines.len(), 1);
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_db_returns_empty_json_array() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(&tmp);
        let req = AuditRequest {
            subcommand: AuditSubcommand::Gates {
                feature_id: None,
                since: None,
                csv: false,
            },
            json: true,
        };
        let resp = run(req, &ctx, &crate::orchestrator::NullSink)
            .await
            .unwrap();
        match resp {
            CommandResponse::Json { value } => {
                assert_eq!(value["decisions"], serde_json::json!([]));
            }
            other => panic!("expected Json, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn populated_db_csv_single_row_fetch() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(&tmp);
        // Create DB by opening once (init() runs v1..v4 migrations),
        // then seed.
        let _ = open_db(&ctx).unwrap();
        seed_db(&ctx);

        let req = AuditRequest {
            subcommand: AuditSubcommand::Gates {
                feature_id: Some("feat".to_string()),
                since: None,
                csv: true,
            },
            json: false,
        };
        let resp = run(req, &ctx, &crate::orchestrator::NullSink)
            .await
            .unwrap();
        match resp {
            CommandResponse::Text { content } => {
                let lines: Vec<&str> = content.lines().collect();
                // Header + one filtered row (feat only, not feat2).
                assert_eq!(lines.len(), 2, "got CSV: {content}");
                assert!(content.contains("feat:infra:01"));
                assert!(!content.contains("feat2:deploy:01"));
            }
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn populated_db_json_shape_is_decisions_array() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(&tmp);
        let _ = open_db(&ctx).unwrap();
        seed_db(&ctx);

        let req = AuditRequest {
            subcommand: AuditSubcommand::Gates {
                feature_id: None,
                since: None,
                csv: false,
            },
            json: true,
        };
        let resp = run(req, &ctx, &crate::orchestrator::NullSink)
            .await
            .unwrap();
        match resp {
            CommandResponse::Json { value } => {
                let arr = value["decisions"].as_array().expect("decisions array");
                assert_eq!(arr.len(), 2);
                // Rows come back ordered by requested_at ASC (query
                // helper enforces it). feat:infra:01 came first.
                assert_eq!(arr[0]["gate_id"], "feat:infra:01");
                assert_eq!(arr[1]["gate_id"], "feat2:deploy:01");
            }
            other => panic!("expected Json, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn corrupt_db_file_surfaces_as_error_not_empty_result() {
        // Phase 6 Codex review finding #4: if `.pice/metrics.db` exists
        // but is corrupt (or otherwise un-openable), audit must surface
        // the failure rather than hand back an empty CSV — an empty
        // result there would mask data loss.
        let tmp = tempfile::tempdir().unwrap();
        let ctx = test_ctx(&tmp);
        let db_path = tmp.path().join(".pice").join("metrics.db");
        // Write non-SQLite bytes so the `MetricsDb::open` migration
        // step fails on header check.
        std::fs::write(&db_path, b"this is not a sqlite database file").unwrap();

        let req = AuditRequest {
            subcommand: AuditSubcommand::Gates {
                feature_id: None,
                since: None,
                csv: false,
            },
            json: false,
        };
        let result = run(req, &ctx, &crate::orchestrator::NullSink).await;
        assert!(
            result.is_err(),
            "corrupt DB must surface as handler error, got: {result:?}"
        );
    }

    #[test]
    fn truncate_keeps_short_strings_untouched() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_uses_ellipsis_for_long_strings() {
        assert_eq!(truncate("hello world", 6), "hello…");
    }
}
