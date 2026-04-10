//! Per-command async handlers — one module per CommandRequest variant.
//!
//! Populated by T19: init, prime, plan, execute, evaluate, review, commit,
//! handoff, status, metrics, benchmark. The handler signature is
//!
//! ```ignore
//! pub async fn run(
//!     req: XxxRequest,
//!     ctx: &DaemonContext,
//!     sink: &dyn StreamSink,
//! ) -> anyhow::Result<CommandResponse>
//! ```
//!
//! Handlers move the body from `pice-cli/src/commands/{cmd}.rs` with two
//! changes: call `sink.send_chunk(..)` instead of `output::print_chunk(..)`,
//! and return a `CommandResponse` instead of printing the final payload.
//!
//! `Completions` is NOT a handler (handled entirely by clap_complete at the
//! CLI layer). The `Daemon` subcommand (T24) is also CLI-only.
