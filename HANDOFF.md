# Handoff: PRDv2 Phase 2 (Workflow YAML + Validation) — implementation done, files partially committed

**Date:** 2026-04-14
**Branch:** `feature/phase-2-workflow-yaml-and-validation`
**Worktree:** `/Users/jacobmolz/code/m0lz.02/.worktrees/phase-2-workflow-yaml-and-validation`
**Main repo:** `/Users/jacobmolz/code/m0lz.02`
**Last Commit:** `0badd73 feat(pice-core,pice-daemon,pice-cli): add validate command and workflow.yaml loader with tier resolution` (modifications only — new files still untracked)

## Goal

Deliver PRDv2 Phase 2 Feature 4 (`.pice/workflow.yaml` + floor-based merge + `pice validate`) per `.claude/plans/phase-2-workflow-yaml-and-validation.md`. All 20 tasks implemented; full validation suite green; 1 commit in place with modifications but the new files still need staging.

## Recently Completed (This Session)

- [x] **All 20 plan tasks** — YAML schema, loader with 3-level inheritance, floor-based merge, hand-written trigger grammar (lex/parse/evaluate), validate module, `pice validate` CLI + daemon handler, 5 workflow presets, integration test, `effective_tier` plumbed into `halted_by`
- [x] **Full validation suite green** — 436 Rust tests (0 fail, +69 from 367 baseline) + 51 TS tests; `cargo fmt --check`, `cargo clippy -- -D warnings`, `pnpm lint`, `pnpm typecheck`, `pnpm build`, `cargo build --release` all ✅
- [x] **Worktree created** at `.worktrees/phase-2-workflow-yaml-and-validation` from `main@d17b258`

## In Progress / Next Steps

- [ ] **Stage and commit the untracked new files** — `git add` the following, then commit as a second Phase 2 commit:
  - `crates/pice-core/src/workflow/` (mod + schema + loader + merge + trigger + validate)
  - `crates/pice-daemon/src/handlers/validate.rs`
  - `crates/pice-daemon/tests/workflow_integration.rs`
  - `crates/pice-cli/src/commands/validate.rs`
  - `templates/pice/workflow.yaml`
  - `templates/pice/workflow-presets/{greenfield,brownfield,ci,strict,permissive}.yaml`
  - `HANDOFF.md` (this file)
- [ ] **Run `/evaluate`** against the Tier-3 contract in `.claude/plans/phase-2-workflow-yaml-and-validation.md` (14 criteria, pass_threshold 9, Claude agent team + Codex xhigh per tier-3 rules)
- [ ] **Fast-forward merge to main** and push (origin still 5 Phase-1 commits behind)
- [ ] **File-based daemon logging** — `crates/pice-daemon/src/logging.rs` still uses stderr stub; replace with `tracing_appender::rolling::daily("~/.pice/logs", "daemon.log")` (independent item, carried from previous handoff)
- [ ] **`pice daemon start` binary discovery** — still PATH-only; should prefer adjacent-to-CLI (npm install case) (carried)
- [ ] **Phase 1 Completion: Provider Wiring** — layers still record `Pending` with `model: "phase-1-pending"`. This IS a Phase 1 remediation task, NOT Phase 2 scope. Rename the old HANDOFF's "Phase 2 provider wiring" label when this hits main.
- [ ] **PRDv2 Phase 3 — Seam Verification** — next recommended plan. Reads the `seams` section of workflow.yaml (now parsed but inert); wires the 12-category seam check registry into `run_stack_loops`.

## Key Decisions

- **Framework → project = simple overlay; project → user = floor-based merge.** PRDv2 lines 903–918 only impose floor semantics on user overrides; the plan's "reuse the same function" hint conflicted with `permissive` preset (tier=1, confidence=0.85 below framework). Split into `overlay()` and `merge_with_floor()` in `workflow/merge.rs`.
- **`max_passes` is NOT floor-guarded.** PRDv2 floor table only lists `tier`, `min_confidence`, `budget_usd`, `require_review`, gate triggers. Direction isn't monotonic (more passes = more cost AND more rigor), so unconstrained.
- **Trigger evaluator fn named `evaluate_ast`, not `eval`.** Security hook blocks bare `eval`.
- **`FloorViolation` is a serializable `thiserror` enum; all violations collected.** Manual `Display` on `FloorViolations` lists each field — single error-message doesn't cut it for the UX or the test assertions.
- **Hand-written recursive-descent trigger parser.** Grammar is tiny (~6 operators + 6 identifiers + 3 literals); under 500 lines, zero new deps, better line/column diagnostics than `nom`.
- **`BTreeMap` everywhere workflow types cross serde.** Deterministic roundtrips + diff-friendly error output.

## Dead Ends (Don't Repeat These)

- **Applying `merge_with_floor` to framework→project.** Breaks every preset that loosens framework defaults. The framework is a baseline, not a floor.
- **Enforcing a direction on `max_passes`.** PRDv2 doesn't; presets need both directions (`ci` lowers, `strict` raises).
- **Inlining violation counts in `FloorViolations::Display`.** Tests and UX both want field names inline — implement `Display` manually instead of relying on `thiserror`'s `#[error(...)]` attribute.
- **(Carried from Phase 1) `use pice_core::X` inside pice-core** — use `crate::`.
- **(Carried) Direct-only dependency cascade.** Use fixed-point iteration for transitive closure.

## Files Changed

- `Cargo.toml`, `crates/pice-core/Cargo.toml` — added `serde_yaml = "0.9"` (committed in 0badd73)
- `crates/pice-core/src/lib.rs` — registered `pub mod workflow` (committed)
- `crates/pice-core/src/cli/mod.rs` — added `CommandRequest::Validate` + `ValidateRequest` + roundtrip tests (committed)
- `crates/pice-daemon/src/handlers/mod.rs` — added `pub mod validate` + dispatch arm (committed)
- `crates/pice-daemon/src/handlers/evaluate.rs` — calls `workflow::loader::resolve` and threads into `StackLoopsConfig` (committed)
- `crates/pice-daemon/src/orchestrator/stack_loops.rs` — added `workflow` field, `effective_tier_for` helper, records tier in `halted_by` (committed)
- `crates/pice-cli/src/commands/mod.rs`, `main.rs` — added `Validate` subcommand (committed)
- `crates/pice-core/src/workflow/{mod,schema,loader,merge,trigger,validate}.rs` — NEW, entire module (**untracked**)
- `crates/pice-daemon/src/handlers/validate.rs` — NEW, 5 unit tests (**untracked**)
- `crates/pice-daemon/tests/workflow_integration.rs` — NEW, 2 integration tests (**untracked**)
- `crates/pice-cli/src/commands/validate.rs` — NEW, CLI adapter (**untracked**)
- `templates/pice/workflow.yaml` — NEW, framework defaults, embedded via `include_str!` (**untracked**)
- `templates/pice/workflow-presets/*.yaml` — NEW, 5 presets (**untracked**)
- `HANDOFF.md` — this update

## Current State

- **Tests:** 436 Rust (1 ignored) + 51 TS = 487 passing, 0 failing
- **Build:** clean for both `cargo build --release` and `pnpm build`
- **Lint/Types:** `cargo fmt --check`, `cargo clippy -- -D warnings`, `pnpm lint`, `pnpm typecheck` all clean
- **Manual verification:** `./target/release/pice validate --help` prints the expected command + `--json` + `--check-models` flags
- **Git state:** 1 commit ahead of `main` with modifications; new files/module **still untracked** pending second commit

## Context for Next Session

Phase 2 ships the workflow YAML spine — parsing, validation, three-level inheritance, and observable layer-override plumbing. The orchestrator currently uses only `effective_tier` from the merged workflow; Phase 4 (Adaptive) will extend `effective_tier_for` in `stack_loops.rs:effective_tier_for` to also resolve `min_confidence`, `max_passes`, `budget_usd`, `require_review`.

**Biggest risk:** the 9 untracked new files contain ~1500 lines of work including the entire `pice-core::workflow` module. They must be committed before any merge back to main — otherwise the feature branch compiles today but won't compile from a fresh clone.

**Recommended first action:**

```bash
cd /Users/jacobmolz/code/m0lz.02/.worktrees/phase-2-workflow-yaml-and-validation
git add crates/pice-core/src/workflow \
        crates/pice-daemon/src/handlers/validate.rs \
        crates/pice-daemon/tests/workflow_integration.rs \
        crates/pice-cli/src/commands/validate.rs \
        templates/pice/workflow.yaml \
        templates/pice/workflow-presets \
        HANDOFF.md
# then /commit
```

After committing, run `/evaluate .claude/plans/phase-2-workflow-yaml-and-validation.md` for the Tier-3 adversarial review before merging to main.
