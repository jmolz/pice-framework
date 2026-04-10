//! `pice-daemon` binary entry point.
//!
//! Deliberately thin: initialize logging, then hand control to
//! `pice_daemon::lifecycle::run`, which owns the event loop, signal handling,
//! and graceful shutdown budget (see `.claude/rules/daemon.md`).
//!
//! Populated in T11 as a stub. T21 wires up the full lifecycle.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pice_daemon::logging::init()?;
    pice_daemon::lifecycle::run().await
}
