//! `pice evaluate` handler — grade contract criteria with dual-model evaluation.

use anyhow::Result;
use pice_core::cli::{CommandResponse, EvaluateRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: EvaluateRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk(&format!(
        "evaluate handler: not yet ported to daemon (plan: {})\n",
        req.plan_path.display()
    ));
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "evaluate"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "evaluate: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
