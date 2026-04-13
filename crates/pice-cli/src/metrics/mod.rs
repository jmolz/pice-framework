//! Metrics module — CLI-side facade.
//!
//! In T14, the write side (`db`, `store`, `telemetry`) moved to
//! `pice-daemon::metrics`. Read-only aggregation also moved to
//! `pice-daemon::metrics::aggregator` during Phase 1 handler porting.
//!
//! The `pub use` re-exports below let existing callers keep using
//! `crate::metrics::{db, store, telemetry, open_metrics_db, ...}` unchanged
//! while the underlying code lives in pice-daemon.

// v0.1 re-exports — only resolve_metrics_db_path and db remain used (by
// init::run_in tests). The rest become dead after T23 rewrites commands to
// go through the adapter. Will be fully removed when daemon handlers are ported.
#[allow(unused_imports)]
pub use pice_daemon::metrics::{
    db, normalize_plan_path, open_metrics_db, resolve_metrics_db_path, store, telemetry,
};
