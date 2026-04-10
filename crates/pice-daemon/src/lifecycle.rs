//! Daemon lifecycle — startup, signal handling, graceful shutdown.
//!
//! Populated by T21. This T11 stub provides a no-op `run()` so the daemon
//! binary compiles and `main.rs` can call into it. T21 replaces the body
//! with the full event loop:
//!
//! 1. Load config, resolve socket path
//! 2. Generate auth token, write to ~/.pice/daemon.token (0600 perms)
//! 3. Bind socket (with stale-cleanup retry)
//! 4. Spawn accept loop with `tokio::spawn` per connection
//! 5. Listen for SIGTERM/SIGINT/CTRL-C via `tokio::signal`
//! 6. On shutdown: stop accepting, drain in-flight RPCs, shut down providers,
//!    flush metrics WAL, remove socket file, exit 0
//!
//! See `.claude/rules/daemon.md` "Graceful shutdown" for the 10s budget rule.

/// Run the daemon event loop. Blocks until the daemon shuts down.
///
/// T11 stub: returns immediately with Ok. T21 replaces with the real loop.
pub async fn run() -> anyhow::Result<()> {
    tracing::info!("pice-daemon stub lifecycle::run invoked (T11 placeholder)");
    Ok(())
}
