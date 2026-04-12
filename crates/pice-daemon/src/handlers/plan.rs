//! `pice plan` handler — create a plan for a feature description.

use anyhow::Result;
use pice_core::cli::{CommandResponse, PlanRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: PlanRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk(&format!(
        "plan handler: not yet ported to daemon (description: {})\n",
        req.description
    ));
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "plan"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "plan: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
