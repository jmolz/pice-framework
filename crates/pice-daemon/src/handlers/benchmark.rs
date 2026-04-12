//! `pice benchmark` handler — run evaluation benchmark suite.

use anyhow::Result;
use pice_core::cli::{BenchmarkRequest, CommandResponse};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: BenchmarkRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("benchmark handler: not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "benchmark"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "benchmark: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
