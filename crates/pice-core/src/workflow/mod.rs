//! `.pice/workflow.yaml` schema, loader, merge, validation, and trigger grammar.
//!
//! Implements PRDv2 Feature 4 — the committable evaluation pipeline. The
//! workflow file codifies which tiers run, when parallelism applies, where
//! review gates fire, how retries work, and what budgets apply. Teams commit
//! `.pice/workflow.yaml` to the repo so every team member runs the same
//! pipeline.
//!
//! ## Module map
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`schema`] | `WorkflowConfig` struct hierarchy + serde derives |
//! | [`loader`] | Embedded defaults, project/user file loading, resolve() |
//! | [`merge`] | Floor-based merge semantics (user can restrict, never relax) |
//! | [`trigger`] | Trigger expression lexer + parser + evaluator |
//! | [`validate`] | Orchestrates schema + trigger + cross-ref + model checks |
//!
//! ## Invariants
//!
//! - `schema_version` must be `"0.2"` — any other value is a hard error.
//! - User workflow overrides cannot relax project floors (see `merge`).
//! - Trigger expressions use a shared grammar; never reinvent for gates vs. overrides.
//! - All parsing uses `serde_yaml`; pice-core stays pure (no async / network / db).

pub mod loader;
pub mod merge;
pub mod schema;
pub mod trigger;
pub mod validate;

pub use schema::{
    AdaptiveAlgo, CostCapBehavior, Defaults, EvaluatePhase, ExecutePhase, LayerOverride, OnTimeout,
    PhaseConfig, Phases, RetryConfig, ReviewConfig, WorkflowConfig,
};

/// The only schema version this crate understands.
pub const SCHEMA_VERSION: &str = "0.2";
