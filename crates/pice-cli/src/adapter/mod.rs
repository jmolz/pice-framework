//! CLI adapter — routes `CommandRequest` to the daemon (socket) or inline mode.
//!
//! This is the CLI's single entry point for executing commands. Every clap
//! command handler calls `adapter::dispatch()`, which decides whether to run
//! the command in-process (inline mode) or forward it over the daemon socket.
//!
//! ## Dispatch paths
//!
//! 1. **Inline** (`PICE_DAEMON_INLINE=1`): calls `pice_daemon::inline::run_command`
//!    directly, bypassing the socket and auth layers. Used for diagnosis and CI.
//! 2. **Socket** (default): connects to the daemon, auto-starting it if needed,
//!    sends a `cli/dispatch` RPC, reads the response.
//!
//! ## Phase 0 status
//!
//! Both paths are wired and tested. Handlers return Phase 0 stubs. The daemon
//! auto-start spawns `pice-daemon` as a detached child process and polls the
//! socket for readiness.
//!
//! T23 rewrites all 11 command dispatchers to call [`dispatch`]; until then,
//! the existing v0.1 command handlers remain wired in `main.rs`.

// T23 rewrites command dispatchers to call adapter::dispatch(). Until then,
// nothing in main.rs calls into this module, so all items appear dead.
#![allow(dead_code)]

pub mod autostart;
pub mod inline;
pub mod transport;

use anyhow::Result;
use pice_core::cli::{CommandRequest, CommandResponse};

/// Dispatch a `CommandRequest` to the daemon or inline handler.
///
/// Checks `PICE_DAEMON_INLINE` env var to decide the path:
/// - Set: inline mode (in-process, no socket)
/// - Unset: socket mode (connect to daemon, auto-start if needed)
pub async fn dispatch(req: CommandRequest) -> Result<CommandResponse> {
    if std::env::var("PICE_DAEMON_INLINE").is_ok() {
        return inline::dispatch_inline(req).await;
    }

    let mut client = autostart::ensure_daemon_running().await?;
    client.dispatch(req).await
}
