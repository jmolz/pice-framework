//! `pice commit` handler — AI-generated commit message.

use anyhow::Result;
use pice_core::cli::{CommandResponse, CommitRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: CommitRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("commit handler: not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "commit"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "commit: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
