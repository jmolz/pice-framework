//! `pice metrics` handler — show evaluation metrics and cost data.

use anyhow::Result;
use pice_core::cli::{CommandResponse, MetricsRequest};
use serde_json::json;

use crate::metrics;
use crate::metrics::aggregator;
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
        let csv = aggregator::format_csv(&db)?;
        return Ok(CommandResponse::Text { content: csv });
    }

    let report = aggregator::aggregate(&db)?;

    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::to_value(&report)
                .map_err(|e| anyhow::anyhow!("failed to serialize metrics report: {e}"))?,
        })
    } else {
        Ok(CommandResponse::Text {
            content: aggregator::format_table(&report),
        })
    }
}
