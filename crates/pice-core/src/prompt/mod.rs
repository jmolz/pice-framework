//! Pure prompt helpers — file reads and git subprocess calls that do not
//! depend on the provider orchestrator.
//!
//! The context-assembly builders (`build_plan_prompt`, etc.) stay with the
//! orchestrator in `pice-daemon::prompt`.

pub mod helpers;
