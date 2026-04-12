//! `pice prime` handler — orient AI on the codebase.

use anyhow::Result;
use pice_core::cli::{CommandResponse, PrimeRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: PrimeRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("prime handler: not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "prime"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "prime: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
