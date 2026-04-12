//! `pice review` handler — AI code review of current changes.

use anyhow::Result;
use pice_core::cli::{CommandResponse, ReviewRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: ReviewRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("review handler: not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "review"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "review: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
