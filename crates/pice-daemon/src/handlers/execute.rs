//! `pice execute` handler — run a plan through the provider.
//!
//! This is the streaming exemplar handler. The full implementation will:
//! 1. Load and parse the plan file
//! 2. Build the execute prompt via `prompt::build_execute_prompt`
//! 3. Start a provider session via `ProviderOrchestrator`
//! 4. Stream chunks to `sink` during execution
//! 5. Record metrics events (execute_started, execute_completed)

use anyhow::Result;
use pice_core::cli::{CommandResponse, ExecuteRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

#[allow(clippy::unused_async)]
pub async fn run(
    req: ExecuteRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk(&format!(
        "execute handler: not yet ported to daemon (plan: {})\n",
        req.plan_path.display()
    ));
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "execute"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "execute: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
