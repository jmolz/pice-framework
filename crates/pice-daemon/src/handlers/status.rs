//! `pice status` handler — show project state and recent evaluations.

use anyhow::Result;
use pice_core::cli::{CommandResponse, StatusRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: StatusRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("status handler: not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "status"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "status: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
