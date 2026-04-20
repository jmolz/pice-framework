//! Shared `CommandRequest` / `CommandResponse` enums — the serialization
//! boundary between `pice-cli` and `pice-daemon`.
//!
//! The CLI parses clap args, converts them to a `CommandRequest` via the
//! `From<XxxArgs>` impls defined in each command module, and sends the request
//! to the daemon as the `params` of a `cli/dispatch` RPC. The daemon dispatches
//! based on the enum variant. Both sides depend on the SAME enum here —
//! divergence is a bug (see `.claude/rules/rust-core.md` "Crate boundary rule").
//!
//! ## Mirroring rule
//!
//! Every variant of this enum corresponds 1:1 with a variant of the clap
//! `Commands` enum in `pice-cli/src/main.rs`, EXCEPT:
//! - `Completions` — handled entirely at the CLI layer (clap_complete),
//!   never crosses the socket.
//! - `Daemon` (added in T24) — manages the daemon process itself, handled
//!   at the CLI layer.
//!
//! Every request struct mirrors the corresponding `XxxArgs` struct from
//! `pice-cli/src/commands/*.rs`. When a field is added to the clap args, the
//! corresponding field must be added here too, otherwise the CLI can't
//! communicate the new option to the daemon.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A command request from the CLI adapter to the daemon.
///
/// Serialized into the `params` of a `cli/dispatch` daemon RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum CommandRequest {
    Init(InitRequest),
    Prime(PrimeRequest),
    Plan(PlanRequest),
    Execute(ExecuteRequest),
    Evaluate(EvaluateRequest),
    Review(ReviewRequest),
    Commit(CommitRequest),
    Handoff(HandoffRequest),
    Status(StatusRequest),
    Metrics(MetricsRequest),
    Benchmark(BenchmarkRequest),
    Layers(LayersRequest),
    Validate(ValidateRequest),
    /// Phase 6: list pending review gates or record a reviewer decision.
    /// Subcommand-dispatched so the CLI binds `pice review-gate --list`
    /// and `pice review-gate --gate-id … --decision …` to different
    /// fields without requiring two RPC method names.
    ReviewGate(ReviewGateRequest),
    /// Phase 6: export the `gate_decisions` audit trail (CSV / JSON).
    /// First subcommand is `Gates`; additional audit surfaces (e.g.,
    /// `Seams`) can extend the enum without a new RPC variant.
    Audit(AuditRequest),
    // NOTE: Completions is handled entirely by clap at the CLI layer.
    // NOTE: Daemon subcommand (start/stop/etc.) is also CLI-only.
}

/// The final result of a dispatched command, sent via `cli/stream-done`.
///
/// Uses struct variants (not newtype) for `Json` and `Text` because serde's
/// internally-tagged enum representation cannot serialize a tagged newtype
/// variant containing a primitive. Struct variants serialize as objects with
/// the tag and fields coexisting.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CommandResponse {
    /// The command produced machine-readable JSON output (--json mode).
    Json { value: serde_json::Value },
    /// The command produced human-readable text output.
    Text { content: String },
    /// The command succeeded with no user-visible payload.
    Empty,
    /// The command failed and the CLI should exit with the given code.
    ///
    /// `message` is human-readable text routed to stderr. For structured
    /// JSON-on-failure (the `--json` error path) use [`CommandResponse::ExitJson`]
    /// instead — mixing the two via string sniffing is ambiguous and fragile.
    Exit { code: i32, message: String },
    /// The command failed in `--json` mode and the CLI should emit the
    /// structured payload on stdout before exiting with the given code.
    ///
    /// Distinct from `Exit` so the renderer does not need to guess whether a
    /// message is JSON or plain text. Used by `pice validate --json` on
    /// validation failure so CI pipelines (`pice validate --json && deploy`)
    /// fail closed while still receiving a parseable error report on stdout.
    ExitJson { code: i32, value: serde_json::Value },
}

/// Stable discriminant strings carried in the `value.status` field of an
/// `ExitJson` payload. Promoted from raw `json!` literals (Phase 3 round-4
/// adversarial review fix) so a typo at the call site fails to compile and
/// CLI integration tests can pin the wire string against the same constants
/// the handler emits. Serialized via the kebab-case rename so the wire
/// shape (`"plan-not-found"`) is unchanged.
///
/// Add a new variant here when introducing a new structured failure path —
/// callers MUST pattern-match exhaustively on this enum, not on the wire
/// strings. Each variant must have a CLI binary integration test that
/// asserts the exact serialized value (see
/// `crates/pice-cli/tests/evaluate_integration.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExitJsonStatus {
    /// `pice evaluate <plan> --json` — plan file does not exist on disk.
    PlanNotFound,
    /// `pice evaluate <plan> --json` — plan file exists but failed to parse.
    PlanParseFailed,
    /// `pice evaluate <plan> --json` — plan parsed but has no `## Contract` section.
    NoContractSection,
    /// `pice evaluate <plan> --json` — workflow.yaml has validation errors
    /// (bad triggers, unknown layer overrides, unknown seam boundaries, etc.).
    WorkflowValidationFailed,
    /// `pice evaluate <plan> --json` — the merged seams map (layers.toml +
    /// workflow.yaml) violates the project floor (e.g. user empty-listed a
    /// boundary the project requires).
    SeamFloorViolation,
    /// `pice evaluate <plan> --json` — the floor-merged seams map fails the
    /// registry validator (unknown check id or applies_to mismatch in a
    /// boundary declared by layers.toml).
    MergedSeamValidationFailed,
    /// `pice evaluate <plan> --json` — evaluation ran to completion but at
    /// least one layer finished in `Failed` status (SPRT reject, ADTS
    /// exhaustion, or a failed seam check). Phase 4 contract criterion #11
    /// (CLI exit-code routing) locks this wire form.
    EvaluationFailed,
    /// `pice evaluate <plan> --json` — the evaluation loop completed but
    /// persisting the result (final `evaluations` summary UPDATE or a
    /// `pass_events` insert) failed. Phase 4.1 Pass-6 Codex High #4 fix:
    /// this was previously swallowed into a `warn!` log and the handler
    /// returned success, producing a manifest that looked green while the
    /// DB carried placeholder/NULL summary fields. We now route it through
    /// the same typed-discriminant path as other structured failures so
    /// dashboards can distinguish "evaluation failed on contract grading"
    /// from "evaluation succeeded but metrics didn't land" — both have
    /// operator-observable consequences but very different remediations.
    MetricsPersistFailed,

    /// Phase 6 review gates: a gate was rejected with no retries remaining
    /// (or the `on_timeout: reject` branch fired on an expired gate). The
    /// layer is `Failed` and the overall manifest is `Failed` — exit 2,
    /// treated like a contract failure because the reviewer explicitly
    /// declined the change.
    ReviewGateRejected,

    /// Phase 6 review gates: a gate with `on_timeout: reject` expired
    /// without a decision and the reconciler fired the timeout action.
    /// Distinct from `ReviewGateRejected` so dashboards can surface
    /// timeout rates separately from manual-reject rates. Exit 2.
    ReviewGateTimeout,

    /// Phase 6 review gates: a concurrent `pice review-gate --decision`
    /// call raced another reviewer — the second caller's SQLite write
    /// hit the `gate_decisions.gate_id` UNIQUE constraint, or the gate
    /// had already transitioned out of `Pending` before the handler
    /// acquired the manifest locks. Exit 1 (operator-actionable; the
    /// first reviewer's decision is the source of truth).
    ReviewGateConflict,

    /// Phase 6 review gates: `pice evaluate` ran to a gate boundary in
    /// a non-TTY (CI / `--json`) context. The pending gates are reported
    /// on stdout and the process exits with **3** so shell loops can
    /// distinguish "work not done, needs reviewer action" from exit 1
    /// (failure) / exit 2 (rejected). New exit code — extends the
    /// existing 0/1/2 surface without overlap.
    ReviewGatePending,

    /// Phase 6 review gates: `pice review-gate` invoked without the
    /// flag combination needed to identify a decision target (e.g.,
    /// neither `--list` nor `--gate-id` supplied; or `--gate-id` with
    /// no `--decision` and stdin is not a TTY). Exit 1.
    MissingDecision,
}

impl ExitJsonStatus {
    /// Wire prefix carried in the per-layer `LayerResult.halted_by` string
    /// when a mid-loop `pass_events` insert fails inside the adaptive
    /// orchestrator. Routing in `build_adaptive_layer_result` and the
    /// `evaluate` handler both check this prefix to map the halt to
    /// `LayerStatus::Pending` (operational, not contract failure) and to
    /// surface via `ExitJsonStatus::MetricsPersistFailed` (exit 1, not
    /// `EvaluationFailed` exit 2). Centralized here so a future rename
    /// updates ONE site and both consumers pick it up automatically —
    /// closes Pass-11.1 W2 (duplicated routing logic).
    pub const METRICS_PERSIST_FAILED_PREFIX: &'static str = "metrics_persist_failed:";

    /// Phase 6 review gates: `halted_by` prefix for a layer that was
    /// rejected at a review gate with no retries remaining. Emitted by
    /// the `ReviewGate::Decide` handler; consumers map to exit code 2
    /// (`LayerStatus::Failed`, `ManifestStatus::Failed`).
    pub const HALTED_GATE_REJECTED: &'static str = "gate_rejected";

    /// Phase 6 review gates: `halted_by` prefix for a layer that timed
    /// out at a review gate with `on_timeout: reject`. Emitted by the
    /// `GateReconciler` and the `gate/decide` timeout prelude. Maps to
    /// exit code 2 alongside [`Self::HALTED_GATE_REJECTED`].
    pub const HALTED_GATE_TIMEOUT_REJECT: &'static str = "gate_timeout_reject";

    /// True if `halted_by` represents a review-gate halt (either manual
    /// reject-without-retries or timeout_reject). Used by both the
    /// orchestrator's halt router and the CLI's exit-code mapper. Flat
    /// underscore convention matches `sprt_*` — a future switch to
    /// prefix-family (`gate_rejected:manual` / `gate_rejected:timeout`)
    /// would only touch this module.
    pub fn is_gate_halt(halted_by: &str) -> bool {
        halted_by == Self::HALTED_GATE_REJECTED || halted_by == Self::HALTED_GATE_TIMEOUT_REJECT
    }

    /// Wire prefix carried in the per-layer `LayerResult.halted_by` string
    /// when a parallel cohort task is cancelled (via
    /// `CancellationToken::cancel()`) before, during, or after provider
    /// evaluation. Phase 5 emits three concrete sub-variants via
    /// [`CancelledReason`]:
    ///
    /// - `"cancelled:pre_spawn"` — cancelled before `tokio::spawn` of the task
    /// - `"cancelled:in_flight"` — observed by the task after it started
    /// - `"cancelled:join_aborted"` — `JoinSet::abort_all()` killed the task
    ///
    /// Centralized so future phases (e.g. Phase 5.5 daemon-shutdown
    /// integration, where `cancelled:*` values may become routing signals
    /// for exit-code mapping) update ONE site and every consumer picks it
    /// up automatically — the same silent-divergence prevention pattern as
    /// [`Self::METRICS_PERSIST_FAILED_PREFIX`].
    pub const CANCELLED_PREFIX: &'static str = "cancelled:";

    /// Returns the serialized wire string. Used by tests so the assertion
    /// runs against the same enum the handler emits — no risk of typo drift
    /// between handler call site and test fixture.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PlanNotFound => "plan-not-found",
            Self::PlanParseFailed => "plan-parse-failed",
            Self::NoContractSection => "no-contract-section",
            Self::WorkflowValidationFailed => "workflow-validation-failed",
            Self::SeamFloorViolation => "seam-floor-violation",
            Self::MergedSeamValidationFailed => "merged-seam-validation-failed",
            Self::EvaluationFailed => "evaluation-failed",
            Self::MetricsPersistFailed => "metrics-persist-failed",
            Self::ReviewGateRejected => "review-gate-rejected",
            Self::ReviewGateTimeout => "review-gate-timeout",
            Self::ReviewGateConflict => "review-gate-conflict",
            Self::ReviewGatePending => "review-gate-pending",
            Self::MissingDecision => "missing-decision",
        }
    }

    /// Conventional process exit code for a given structured status.
    ///
    /// | Family | Exit | Semantics |
    /// |--------|------|-----------|
    /// | contract failure (reviewer reject, grading fail) | 2 | the change does not meet the bar |
    /// | operational failure (parse, validation, persistence, conflict, missing decision) | 1 | tooling / config / race |
    /// | `ReviewGatePending` | 3 | work paused pending human decision (Phase 6) |
    ///
    /// Centralizing the mapping here — instead of hardcoding `code: 1|2`
    /// at each `ExitJson` construction site — lets a future release retire
    /// an exit code with a single edit instead of N handler touches.
    pub fn exit_code(&self) -> i32 {
        match self {
            // Contract/reviewer-level rejection family.
            Self::EvaluationFailed
            | Self::NoContractSection
            | Self::ReviewGateRejected
            | Self::ReviewGateTimeout => 2,
            // Work-paused-waiting-for-human-review family (Phase 6).
            Self::ReviewGatePending => 3,
            // Operational failure family (everything else).
            Self::PlanNotFound
            | Self::PlanParseFailed
            | Self::WorkflowValidationFailed
            | Self::SeamFloorViolation
            | Self::MergedSeamValidationFailed
            | Self::MetricsPersistFailed
            | Self::ReviewGateConflict
            | Self::MissingDecision => 1,
        }
    }

    /// True if `halted_by` represents a mid-loop metrics persistence
    /// failure. Both the layer-status mapper in `pice-daemon` AND the
    /// `evaluate` handler call this helper — never re-implement the prefix
    /// check inline (Pass-11.1 W2: drift between two `starts_with` call
    /// sites would silently misroute the exit code).
    pub fn is_metrics_persist_failed(halted_by: &str) -> bool {
        halted_by.starts_with(Self::METRICS_PERSIST_FAILED_PREFIX)
    }

    /// True if `halted_by` represents a parallel-cohort cancellation (any
    /// of the three Phase-5 sub-variants in [`CancelledReason`]). Every
    /// consumer — integration tests today, daemon-shutdown routing in
    /// Phase 5.5 — calls this helper; the inline literal is not
    /// re-typed anywhere. Same pattern as `is_metrics_persist_failed`.
    pub fn is_cancelled(halted_by: &str) -> bool {
        halted_by.starts_with(Self::CANCELLED_PREFIX)
    }
}

/// Typed sub-variant of a `cancelled:*` `halted_by` string. Pairs with
/// [`ExitJsonStatus::CANCELLED_PREFIX`] so call sites never re-type the
/// literal. Three variants are pinned by the Phase-5 cohort-parallelism
/// integration tests; adding a fourth requires updating `as_str` AND the
/// `CANCELLED_PREFIX`-const-agrees-with-helper parity test.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelledReason {
    /// The task was cancelled before `tokio::spawn` got to run it.
    PreSpawn,
    /// The task observed cancellation after spawn.
    InFlight,
    /// `JoinSet::abort_all()` dropped the task's future; synthesized
    /// during the post-drain walk over layers that never produced a
    /// `LayerOutcome`.
    JoinAborted,
}

impl CancelledReason {
    /// Returns the full `halted_by` wire string
    /// (`"cancelled:<reason>"`). Callers always use this — the prefix
    /// is never concatenated inline.
    pub fn as_halted_by(&self) -> String {
        format!("{}{}", ExitJsonStatus::CANCELLED_PREFIX, self.suffix())
    }

    /// Just the reason tail after the `:` (used by the parity test).
    pub fn suffix(&self) -> &'static str {
        match self {
            Self::PreSpawn => "pre_spawn",
            Self::InFlight => "in_flight",
            Self::JoinAborted => "join_aborted",
        }
    }
}

// ─── Request structs ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitRequest {
    pub force: bool,
    #[serde(default)]
    pub upgrade: bool,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrimeRequest {
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanRequest {
    pub description: String,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteRequest {
    pub plan_path: PathBuf,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateRequest {
    pub plan_path: PathBuf,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitRequest {
    pub message: Option<String>,
    pub dry_run: bool,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffRequest {
    pub output: Option<PathBuf>,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusRequest {
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsRequest {
    pub json: bool,
    pub csv: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRequest {
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayersRequest {
    pub subcommand: LayersSubcommand,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum LayersSubcommand {
    Detect { write: bool, force: bool },
    List,
    Check,
    Graph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateRequest {
    pub json: bool,
    #[serde(default)]
    pub check_models: bool,
}

// ─── Phase 6: review-gate + audit request / response DTOs ───────────────────

/// Top-level wire struct for `pice review-gate` commands. Mirrors the
/// [`LayersRequest`] pattern: the subcommand discriminates list vs
/// decide so one RPC variant serves both CLI entry points.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewGateRequest {
    pub subcommand: ReviewGateSubcommand,
    pub json: bool,
}

/// `action`-tagged review-gate subcommand enum. `list` returns every
/// pending gate (optionally filtered to a feature); `decide` records a
/// reviewer's approve/reject/skip against a specific gate id.
///
/// `reviewer` is the caller's resolved username (`$USER` / `$USERNAME`
/// on the CLI side, never read from process env by the daemon) so the
/// audit trail attributes every decision to a human or a CI bot name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ReviewGateSubcommand {
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feature_id: Option<String>,
    },
    Decide {
        gate_id: String,
        decision: crate::gate::GateDecision,
        reviewer: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// Response DTO for `ReviewGateSubcommand::List`. A cross-feature
/// snapshot so `pice review-gate --list` can enumerate every gate
/// blocking the user without per-feature RPC round-trips.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateListResponse {
    pub gates: Vec<GateListEntry>,
}

/// Flattened view of a pending gate for the list RPC. Distinct from
/// [`crate::layers::manifest::GateEntry`] because the list surfaces
/// CROSS-feature data (it includes `feature_id`) while the manifest
/// gate is always scoped to its owning feature.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GateListEntry {
    pub id: String,
    pub feature_id: String,
    pub layer: String,
    pub trigger_expression: String,
    pub requested_at: String,
    pub timeout_at: String,
    pub reject_attempts_remaining: u32,
}

/// Response DTO for `ReviewGateSubcommand::Decide`. Includes the
/// remaining `pending_gates` on the feature so a TTY-driven prompt
/// loop on the CLI side can surface "2 of 3 gates decided" without an
/// extra `List` round-trip — this closes the Claude Cycle-2 multi-gate
/// race finding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GateDecideResponse {
    /// The audit-decision string (one of `approve`, `reject`, `skip`,
    /// `timeout_reject`, `timeout_approve`, `timeout_skip`). Distinct
    /// from the requested `decision` because a timeout prelude may
    /// have fired first — the response returns the outcome that
    /// actually landed.
    pub decision: String,
    pub layer_status: crate::layers::manifest::LayerStatus,
    pub manifest_status: crate::layers::manifest::ManifestStatus,
    pub reject_attempts_remaining: u32,
    /// Remaining gates on the same feature that still need a decision
    /// after this one. Empty vec → the CLI loop can now re-invoke
    /// `pice evaluate` to resume the cohort loop.
    pub pending_gates: Vec<crate::layers::manifest::GateEntry>,
    /// SQLite `gate_decisions.id` of the audit row inserted by this
    /// decision. Operationally useful for linking dashboard events to
    /// the audit trail without a SELECT scan.
    pub audit_id: i64,
}

/// Top-level wire struct for `pice audit`. First subcommand is
/// `Gates`; future audit surfaces (seam findings, cost events) add
/// variants without needing a new RPC method.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuditRequest {
    pub subcommand: AuditSubcommand,
    pub json: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case", deny_unknown_fields)]
pub enum AuditSubcommand {
    Gates {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        feature_id: Option<String>,
        /// RFC3339 lower bound on `requested_at`. Stored as a string
        /// (not `DateTime<Utc>`) so the CLI can pass `--since 2026-04-20T00:00:00Z`
        /// directly without parsing.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<String>,
        /// CSV vs JSON output — orthogonal to the `json` field on
        /// [`AuditRequest`] because `--csv` and `--json` are mutually
        /// exclusive human/machine format knobs, not "human vs RPC"
        /// shapes. Both flags suppress human-friendly `println!`.
        #[serde(default)]
        csv: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_request_roundtrip() {
        let req = CommandRequest::Init(InitRequest {
            force: true,
            upgrade: false,
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        // Tag-based serialization: {"command":"init",...}
        assert!(wire.contains("\"command\":\"init\""));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Init(r) => {
                assert!(r.force);
                assert!(!r.upgrade);
                assert!(!r.json);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn init_request_upgrade_roundtrip() {
        let req = CommandRequest::Init(InitRequest {
            force: false,
            upgrade: true,
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Init(r) => {
                assert!(r.upgrade);
                assert!(!r.force);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn init_request_upgrade_defaults_false() {
        // Backwards compat: old JSON without "upgrade" field should default to false
        let json = r#"{"command":"init","force":false,"json":false}"#;
        let parsed: CommandRequest = serde_json::from_str(json).unwrap();
        match parsed {
            CommandRequest::Init(r) => assert!(!r.upgrade),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn plan_request_with_description_roundtrip() {
        let req = CommandRequest::Plan(PlanRequest {
            description: "add auth".to_string(),
            json: true,
        });
        let wire = serde_json::to_string(&req).unwrap();
        assert!(wire.contains("\"command\":\"plan\""));
        assert!(wire.contains("\"description\":\"add auth\""));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Plan(r) => {
                assert_eq!(r.description, "add auth");
                assert!(r.json);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn execute_request_with_path_roundtrip() {
        let req = CommandRequest::Execute(ExecuteRequest {
            plan_path: PathBuf::from(".claude/plans/auth.md"),
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Execute(r) => {
                assert_eq!(r.plan_path, PathBuf::from(".claude/plans/auth.md"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn commit_request_with_optional_message_none() {
        let req = CommandRequest::Commit(CommitRequest {
            message: None,
            dry_run: true,
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Commit(r) => {
                assert!(r.message.is_none());
                assert!(r.dry_run);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn commit_request_with_optional_message_some() {
        let req = CommandRequest::Commit(CommitRequest {
            message: Some("fix(bug): resolve race".to_string()),
            dry_run: false,
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Commit(r) => {
                assert_eq!(r.message.as_deref(), Some("fix(bug): resolve race"));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn handoff_request_with_optional_path() {
        let req = CommandRequest::Handoff(HandoffRequest {
            output: Some(PathBuf::from("HANDOFF.md")),
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Handoff(r) => {
                assert_eq!(r.output, Some(PathBuf::from("HANDOFF.md")));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn metrics_request_with_csv_flag() {
        let req = CommandRequest::Metrics(MetricsRequest {
            json: false,
            csv: true,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Metrics(r) => {
                assert!(!r.json);
                assert!(r.csv);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn layers_request_roundtrip() {
        let req = CommandRequest::Layers(LayersRequest {
            subcommand: LayersSubcommand::Detect {
                write: true,
                force: false,
            },
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        assert!(wire.contains("\"command\":\"layers\""));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Layers(r) => {
                assert!(!r.json);
                match r.subcommand {
                    LayersSubcommand::Detect { write, force } => {
                        assert!(write);
                        assert!(!force);
                    }
                    _ => panic!("wrong subcommand"),
                }
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn validate_request_roundtrip() {
        let req = CommandRequest::Validate(ValidateRequest {
            json: true,
            check_models: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        assert!(wire.contains("\"command\":\"validate\""));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Validate(r) => {
                assert!(r.json);
                assert!(!r.check_models);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn validate_request_check_models_defaults_false() {
        // Backwards compat: old JSON without check_models defaults to false.
        let json = r#"{"command":"validate","json":false}"#;
        let parsed: CommandRequest = serde_json::from_str(json).unwrap();
        match parsed {
            CommandRequest::Validate(r) => assert!(!r.check_models),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn layers_subcommand_list_roundtrip() {
        let req = CommandRequest::Layers(LayersRequest {
            subcommand: LayersSubcommand::List,
            json: true,
        });
        let wire = serde_json::to_string(&req).unwrap();
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Layers(r) => {
                assert!(r.json);
                matches!(r.subcommand, LayersSubcommand::List);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn command_request_kebab_case_tag() {
        // Verify every variant uses kebab-case tags matching the clap command names.
        for (req, expected_tag) in [
            (CommandRequest::Prime(PrimeRequest { json: false }), "prime"),
            (
                CommandRequest::Review(ReviewRequest { json: false }),
                "review",
            ),
            (
                CommandRequest::Status(StatusRequest { json: false }),
                "status",
            ),
            (
                CommandRequest::Benchmark(BenchmarkRequest { json: false }),
                "benchmark",
            ),
            (
                CommandRequest::Evaluate(EvaluateRequest {
                    plan_path: PathBuf::from("plan.md"),
                    json: false,
                }),
                "evaluate",
            ),
            (
                CommandRequest::Layers(LayersRequest {
                    subcommand: LayersSubcommand::Graph,
                    json: false,
                }),
                "layers",
            ),
        ] {
            let wire = serde_json::to_string(&req).unwrap();
            assert!(
                wire.contains(&format!("\"command\":\"{expected_tag}\"")),
                "variant should serialize with tag {expected_tag}, got: {wire}"
            );
        }
    }

    // ─── CommandResponse tests ───────────────────────────────────────────────

    #[test]
    fn command_response_json_roundtrip() {
        let resp = CommandResponse::Json {
            value: serde_json::json!({"plans": 3, "tier": 2}),
        };
        let wire = serde_json::to_string(&resp).unwrap();
        assert!(wire.contains("\"type\":\"json\""));
        let parsed: CommandResponse = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandResponse::Json { value } => {
                assert_eq!(value, serde_json::json!({"plans": 3, "tier": 2}));
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn command_response_text_roundtrip() {
        let resp = CommandResponse::Text {
            content: "done".to_string(),
        };
        let wire = serde_json::to_string(&resp).unwrap();
        assert!(wire.contains("\"type\":\"text\""));
        let parsed: CommandResponse = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandResponse::Text { content } => assert_eq!(content, "done"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn command_response_empty_roundtrip() {
        let resp = CommandResponse::Empty;
        let wire = serde_json::to_string(&resp).unwrap();
        assert!(wire.contains("\"type\":\"empty\""));
        let parsed: CommandResponse = serde_json::from_str(&wire).unwrap();
        matches!(parsed, CommandResponse::Empty);
    }

    #[test]
    fn command_response_exit_json_roundtrip() {
        // JSON-mode failure path: exit nonzero AND carry a structured payload
        // that the renderer writes to stdout. Catches the old string-sniffing
        // ambiguity where a plain-text `Exit` message that happened to parse
        // as JSON would be misrouted.
        let resp = CommandResponse::ExitJson {
            code: 1,
            value: serde_json::json!({"ok": false, "errors": ["bad"]}),
        };
        let wire = serde_json::to_string(&resp).unwrap();
        assert!(wire.contains("\"type\":\"exit-json\""));
        let parsed: CommandResponse = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandResponse::ExitJson { code, value } => {
                assert_eq!(code, 1);
                assert_eq!(value["ok"], false);
                assert_eq!(value["errors"][0], "bad");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn command_response_exit_with_reason() {
        // Evaluation failed (contract criteria not met) — exit 2 per CLI conventions.
        let resp = CommandResponse::Exit {
            code: 2,
            message: "contract criteria not met".to_string(),
        };
        let wire = serde_json::to_string(&resp).unwrap();
        assert!(wire.contains("\"type\":\"exit\""));
        let parsed: CommandResponse = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandResponse::Exit { code, message } => {
                assert_eq!(code, 2);
                assert_eq!(message, "contract criteria not met");
            }
            _ => panic!("wrong variant"),
        }
    }

    /// Pass-11.1 W2 fix: lock the `metrics_persist_failed:` prefix
    /// constant against the helper. Both `build_adaptive_layer_result` in
    /// the daemon AND the `evaluate` handler call
    /// `ExitJsonStatus::is_metrics_persist_failed(...)`; if the constant
    /// changes without the helper following, both sites silently
    /// misroute. This test fails on drift.
    #[test]
    fn metrics_persist_failed_prefix_helper_agrees_with_constant() {
        let happy = format!(
            "{}{}",
            ExitJsonStatus::METRICS_PERSIST_FAILED_PREFIX,
            "simulated SQLite I/O error on call 2"
        );
        assert!(ExitJsonStatus::is_metrics_persist_failed(&happy));
        // Must be unambiguous against the existing `runtime_error:` namespace
        // — Pass-11 chose a non-overlapping prefix on purpose.
        assert!(!ExitJsonStatus::is_metrics_persist_failed(
            "runtime_error:metrics_persist_failed:legacy"
        ));
        assert!(!ExitJsonStatus::is_metrics_persist_failed(""));
        assert!(!ExitJsonStatus::is_metrics_persist_failed("sprt_rejected"));
        // Empty body after the prefix is still a valid match — error
        // strings can be empty in pathological cases.
        assert!(ExitJsonStatus::is_metrics_persist_failed(
            ExitJsonStatus::METRICS_PERSIST_FAILED_PREFIX
        ));
    }

    /// Phase 5 cohort-parallelism: lock the `cancelled:` prefix constant
    /// against the helper AND the typed `CancelledReason` enum. Three
    /// production call sites in `stack_loops.rs` construct
    /// `halted_by` via `CancelledReason::as_halted_by()`; integration
    /// tests consume via `ExitJsonStatus::is_cancelled(...)`. A refactor
    /// that updates one without the other must fail loudly — this test
    /// catches that drift.
    #[test]
    fn cancelled_prefix_helper_and_reason_enum_agree() {
        // Every typed reason produces a `halted_by` string that the
        // helper accepts.
        for reason in [
            CancelledReason::PreSpawn,
            CancelledReason::InFlight,
            CancelledReason::JoinAborted,
        ] {
            let halted_by = reason.as_halted_by();
            assert!(
                halted_by.starts_with(ExitJsonStatus::CANCELLED_PREFIX),
                "{halted_by} must start with CANCELLED_PREFIX"
            );
            assert!(ExitJsonStatus::is_cancelled(&halted_by));
        }
        // Negative cases: disjoint prefixes must not match.
        assert!(!ExitJsonStatus::is_cancelled(
            ExitJsonStatus::METRICS_PERSIST_FAILED_PREFIX
        ));
        assert!(!ExitJsonStatus::is_cancelled("runtime_error:provider"));
        assert!(!ExitJsonStatus::is_cancelled(""));
        // Bare prefix (empty reason tail) is still cancellation — the
        // post-drain synthesis path writes it in pathological races.
        assert!(ExitJsonStatus::is_cancelled(
            ExitJsonStatus::CANCELLED_PREFIX
        ));
    }

    /// Phase 3 round-5 adversarial review fix: lock `ExitJsonStatus::as_str()`
    /// to the serde `rename_all = "kebab-case"` output. The handler emits via
    /// `as_str()` directly (bypassing serde), so the two paths can silently
    /// drift. This test fails on mismatch, forcing future variant renames to
    /// update BOTH the serde derive AND the `as_str()` match arm.
    #[test]
    fn exit_json_status_as_str_matches_serde_kebab_case() {
        let all_variants = [
            ExitJsonStatus::PlanNotFound,
            ExitJsonStatus::PlanParseFailed,
            ExitJsonStatus::NoContractSection,
            ExitJsonStatus::WorkflowValidationFailed,
            ExitJsonStatus::SeamFloorViolation,
            ExitJsonStatus::MergedSeamValidationFailed,
            ExitJsonStatus::EvaluationFailed,
            ExitJsonStatus::MetricsPersistFailed,
            ExitJsonStatus::ReviewGateRejected,
            ExitJsonStatus::ReviewGateTimeout,
            ExitJsonStatus::ReviewGateConflict,
            ExitJsonStatus::ReviewGatePending,
            ExitJsonStatus::MissingDecision,
        ];
        for variant in &all_variants {
            let serde_output = serde_json::to_string(variant).unwrap();
            let expected = format!("\"{}\"", variant.as_str());
            assert_eq!(
                serde_output, expected,
                "ExitJsonStatus::{variant:?} — serde output {serde_output} != as_str() {expected}; \
                 update the as_str() match arm or the serde rename to stay in sync"
            );
        }
    }

    /// Phase 6 Task 3: lock the exit-code mapping so a rename or new
    /// variant can't silently misroute. Exit 3 is NEW in Phase 6 —
    /// reserved for `ReviewGatePending` and nothing else.
    #[test]
    fn exit_code_family_mapping_is_stable() {
        // Contract/reviewer-reject family → 2
        assert_eq!(ExitJsonStatus::EvaluationFailed.exit_code(), 2);
        assert_eq!(ExitJsonStatus::NoContractSection.exit_code(), 2);
        assert_eq!(ExitJsonStatus::ReviewGateRejected.exit_code(), 2);
        assert_eq!(ExitJsonStatus::ReviewGateTimeout.exit_code(), 2);
        // Pause-for-review family → 3 (Phase 6 new exit code)
        assert_eq!(ExitJsonStatus::ReviewGatePending.exit_code(), 3);
        // Operational family → 1
        assert_eq!(ExitJsonStatus::PlanNotFound.exit_code(), 1);
        assert_eq!(ExitJsonStatus::ReviewGateConflict.exit_code(), 1);
        assert_eq!(ExitJsonStatus::MissingDecision.exit_code(), 1);
        assert_eq!(ExitJsonStatus::MetricsPersistFailed.exit_code(), 1);
        // Exhaustive sweep: every variant's exit code is one of {1, 2, 3}.
        for v in [
            ExitJsonStatus::PlanNotFound,
            ExitJsonStatus::PlanParseFailed,
            ExitJsonStatus::NoContractSection,
            ExitJsonStatus::WorkflowValidationFailed,
            ExitJsonStatus::SeamFloorViolation,
            ExitJsonStatus::MergedSeamValidationFailed,
            ExitJsonStatus::EvaluationFailed,
            ExitJsonStatus::MetricsPersistFailed,
            ExitJsonStatus::ReviewGateRejected,
            ExitJsonStatus::ReviewGateTimeout,
            ExitJsonStatus::ReviewGateConflict,
            ExitJsonStatus::ReviewGatePending,
            ExitJsonStatus::MissingDecision,
        ] {
            let code = v.exit_code();
            assert!(
                (1..=3).contains(&code),
                "{v:?} returned exit code {code} outside {{1, 2, 3}} — \
                 extending the surface requires explicit CLI conventions update"
            );
        }
    }

    // ── Phase 6: review-gate + audit RPC roundtrips ──────────────────

    #[test]
    fn review_gate_list_request_roundtrip() {
        let req = CommandRequest::ReviewGate(ReviewGateRequest {
            subcommand: ReviewGateSubcommand::List {
                feature_id: Some("feat-abc".to_string()),
            },
            json: true,
        });
        let wire = serde_json::to_string(&req).unwrap();
        assert!(wire.contains("\"command\":\"review-gate\""));
        assert!(wire.contains("\"action\":\"list\""));
        assert!(wire.contains("\"feature_id\":\"feat-abc\""));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::ReviewGate(r) => {
                assert!(r.json);
                match r.subcommand {
                    ReviewGateSubcommand::List { feature_id } => {
                        assert_eq!(feature_id.as_deref(), Some("feat-abc"));
                    }
                    _ => panic!("expected List subcommand"),
                }
            }
            _ => panic!("expected ReviewGate variant"),
        }
    }

    #[test]
    fn review_gate_decide_request_roundtrip() {
        let req = CommandRequest::ReviewGate(ReviewGateRequest {
            subcommand: ReviewGateSubcommand::Decide {
                gate_id: "feat:infra:01".to_string(),
                decision: crate::gate::GateDecision::Reject,
                reviewer: "jacob".to_string(),
                reason: Some("blocked on staging deploy".to_string()),
            },
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        assert!(wire.contains("\"action\":\"decide\""));
        assert!(wire.contains("\"decision\":\"reject\""));
        assert!(wire.contains("\"reviewer\":\"jacob\""));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::ReviewGate(r) => match r.subcommand {
                ReviewGateSubcommand::Decide {
                    gate_id,
                    decision,
                    reviewer,
                    reason,
                } => {
                    assert_eq!(gate_id, "feat:infra:01");
                    assert_eq!(decision, crate::gate::GateDecision::Reject);
                    assert_eq!(reviewer, "jacob");
                    assert_eq!(reason.as_deref(), Some("blocked on staging deploy"));
                }
                _ => panic!("expected Decide"),
            },
            _ => panic!("expected ReviewGate"),
        }
    }

    #[test]
    fn audit_gates_request_roundtrip() {
        let req = CommandRequest::Audit(AuditRequest {
            subcommand: AuditSubcommand::Gates {
                feature_id: None,
                since: Some("2026-04-01T00:00:00Z".to_string()),
                csv: true,
            },
            json: false,
        });
        let wire = serde_json::to_string(&req).unwrap();
        assert!(wire.contains("\"command\":\"audit\""));
        assert!(wire.contains("\"action\":\"gates\""));
        assert!(wire.contains("\"csv\":true"));
        let parsed: CommandRequest = serde_json::from_str(&wire).unwrap();
        match parsed {
            CommandRequest::Audit(r) => match r.subcommand {
                AuditSubcommand::Gates {
                    feature_id,
                    since,
                    csv,
                } => {
                    assert!(feature_id.is_none());
                    assert_eq!(since.as_deref(), Some("2026-04-01T00:00:00Z"));
                    assert!(csv);
                }
            },
            _ => panic!("expected Audit"),
        }
    }

    #[test]
    fn gate_list_response_roundtrip() {
        let resp = GateListResponse {
            gates: vec![GateListEntry {
                id: "feat:infra:01".to_string(),
                feature_id: "feat".to_string(),
                layer: "infra".to_string(),
                trigger_expression: "always".to_string(),
                requested_at: "2026-04-20T00:00:00Z".to_string(),
                timeout_at: "2026-04-21T00:00:00Z".to_string(),
                reject_attempts_remaining: 1,
            }],
        };
        let wire = serde_json::to_string(&resp).unwrap();
        let back: GateListResponse = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn gate_decide_response_roundtrip() {
        use crate::layers::manifest::{LayerStatus, ManifestStatus};
        let resp = GateDecideResponse {
            decision: "approve".to_string(),
            layer_status: LayerStatus::Passed,
            manifest_status: ManifestStatus::InProgress,
            reject_attempts_remaining: 2,
            pending_gates: vec![],
            audit_id: 42,
        };
        let wire = serde_json::to_string(&resp).unwrap();
        assert!(wire.contains("\"layer_status\":\"passed\""));
        assert!(wire.contains("\"manifest_status\":\"in-progress\""));
        let back: GateDecideResponse = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, resp);
    }

    #[test]
    fn phase_6_request_dtos_deny_unknown_fields() {
        // All new DTOs carry `deny_unknown_fields`. Exercise the
        // rejection so a rename like `gate_id` → `gateId` can't
        // silently no-op in a user's stale CLI call.
        let bad = r#"{"subcommand":{"action":"list","bogusField":1},"json":false}"#;
        let err = serde_json::from_str::<ReviewGateRequest>(bad).unwrap_err();
        assert!(
            err.to_string().contains("bogusField") || err.to_string().contains("unknown field")
        );
    }

    /// Phase 6 Task 3: lock the gate-halt prefix constants against the
    /// `is_gate_halt` predicate. Flat-underscore convention (matching
    /// `sprt_*`) is the agreed style; refactoring to a prefix family
    /// (`gate_rejected:*`) in a later phase must update ONE file.
    #[test]
    fn gate_halt_prefixes_agree_with_is_gate_halt() {
        assert!(ExitJsonStatus::is_gate_halt(
            ExitJsonStatus::HALTED_GATE_REJECTED
        ));
        assert!(ExitJsonStatus::is_gate_halt(
            ExitJsonStatus::HALTED_GATE_TIMEOUT_REJECT
        ));
        // Adjacent halt families must not be misrouted through the gate
        // predicate — e.g., a future `gate_approved` halt (there is
        // none) must not match, nor a `sprt_rejected`.
        assert!(!ExitJsonStatus::is_gate_halt("sprt_rejected"));
        assert!(!ExitJsonStatus::is_gate_halt("gate_approved"));
        assert!(!ExitJsonStatus::is_gate_halt(""));
        assert!(!ExitJsonStatus::is_gate_halt(
            ExitJsonStatus::METRICS_PERSIST_FAILED_PREFIX
        ));
    }
}
