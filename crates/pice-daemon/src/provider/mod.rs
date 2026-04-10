//! Async provider host — spawns and manages provider subprocesses over
//! stdio-based JSON-RPC. Moved here from `pice-cli/src/provider/` in T12.
//!
//! The pure path-walking lookup logic (`registry`) stays in
//! `pice-core::provider::registry` so the CLI can preview provider resolution
//! without depending on tokio.

pub mod host;
