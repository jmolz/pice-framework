//! Inline mode — runs a `CommandRequest` in-process without a socket.
//!
//! Used by:
//! - `pice-cli` when `PICE_DAEMON_INLINE=1` is set (regression-safe escape
//!   hatch for diagnosing daemon-related failures).
//! - Integration tests that want to exercise the handler chain without
//!   spawning a separate daemon subprocess.
//!
//! Populated by T20. This T11 stub declares the entry point so the CLI
//! adapter (T22) can depend on it, but the actual dispatch logic is a
//! placeholder until the handlers exist.
