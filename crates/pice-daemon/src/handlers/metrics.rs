//! `pice metrics` handler — show evaluation metrics and cost data.

use anyhow::Result;
use pice_core::cli::{CommandResponse, MetricsRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: MetricsRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("metrics handler: not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "metrics"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "metrics: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
