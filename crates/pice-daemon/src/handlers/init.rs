//! `pice init` handler — scaffold `.claude/` and `.pice/` directories.
//!
//! Phase 0 stub: returns a placeholder response. The full body port from
//! `pice-cli/src/commands/init.rs` requires `extract_templates` to move to
//! `pice-core` (pure logic, no async) — tracked for a subsequent task.

use anyhow::Result;
use pice_core::cli::{CommandResponse, InitRequest};

use crate::orchestrator::StreamSink;
use crate::server::router::DaemonContext;

/// Initialize a project with PICE scaffolding.
///
/// Full implementation will:
/// 1. Extract templates to `.claude/` and `.pice/`
/// 2. Validate the scaffolded config
/// 3. Initialize the metrics database
/// 4. Return created/skipped file counts
#[allow(clippy::unused_async)]
pub async fn run(
    req: InitRequest,
    _ctx: &DaemonContext,
    sink: &dyn StreamSink,
) -> Result<CommandResponse> {
    sink.send_chunk("init handler: scaffolding not yet ported to daemon\n");
    if req.json {
        Ok(CommandResponse::Json {
            value: serde_json::json!({"status": "stub", "command": "init"}),
        })
    } else {
        Ok(CommandResponse::Text {
            content: "init: handler not yet ported from pice-cli (Phase 0 stub)".to_string(),
        })
    }
}
