//! SQLite metrics writer + telemetry HTTP sender. Moved here from
//! `pice-cli/src/metrics/` in T14.
//!
//! The daemon owns ALL writes to the metrics database. The CLI reads from
//! the same database for reporting (`pice metrics`, `pice status`) but never
//! writes directly. Non-fatal recording pattern per `.claude/rules/metrics.md`
//! — write failures log at `warn` and continue.
//!
//! Note that `pice-cli::metrics::aggregator` (read-only queries) stays in
//! `pice-cli` per the phase-0 plan; T14 imports types from here.
