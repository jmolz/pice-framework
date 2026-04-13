# Handoff: v0.2 Architecture Complete — Ready for Layer Detection

**Date:** 2026-04-13
**Branch:** `main`
**Last Commit:** `408a948 fix(pice-core): skip project tree test on Windows (uses Unix find)`

## Goal

v0.2 architectural foundation (daemon split + handler porting) is complete and merged. Branch is clean, all tests passing. Ready for v0.2 user-visible features.

## Completed (Architecture Phases)

- [x] **Phase 0: Daemon Foundation** — 3-crate split (`pice-cli`, `pice-daemon`, `pice-core`), socket transport, inline mode, auto-start
- [x] **Phase 1: Handler Porting** — all 11 handlers ported with real logic, streaming, dual-model eval
- [x] **Post-merge cleanup** — template sync, stale artifact removal, test count updates, Windows fixes, rules created
- [x] **Rule/doc commits** — `caaacea` committed rule changes; `408a948` committed test fix

## Next Steps

- [ ] **File-based daemon logging** — `logging.rs` uses stderr stub. Replace with `tracing_appender::rolling::daily("~/.pice/logs", "daemon.log")`
- [ ] **`pice daemon start` binary discovery** — PATH lookup only. Should check adjacent to CLI binary first (npm install case)
- [ ] **PRDv2 Feature Phase 1: Layer Detection** — first v0.2 user-visible feature. Read `PRDv2.md` §MVP Scope + `.claude/rules/stack-loops.md`

## Key Decisions

- **`StreamSink` = `Arc<dyn StreamSink>`** — forced by `NotificationHandler = Box<dyn Fn + Send>` being `'static`
- **`CommandResponse` struct variants (not newtype)** — serde `#[serde(tag = "type")]` can't serialize tagged newtypes wrapping primitives
- **Template sync is manual** — root `.claude/` and `templates/claude/` must be kept in sync; `.claude/rules/templates.md` has the sync table

## Dead Ends (Don't Repeat These)

- **`use pice_core::X` inside pice-core** — use `crate::`.
- **Newtype variants in `#[serde(tag = "type")]` enums** — use struct variants.
- **macOS fd-inheritance race in stale-socket tests** — keep in separate integration test binary.
- **Per-crate `.claude/` dirs** — hooks recreate them. Gitignored now, delete on sight.

## Current State

- **Tests:** 271 Rust (1 ignored) + 49 TS = 320 total, 0 failures
- **Build:** clean (both `pice` and `pice-daemon` binaries)
- **Lint/Types:** fmt, clippy, eslint, tsc all clean
- **Uncommitted:** none
- **Unpushed:** none

## Context for Next Session

Architecture phases are done. Choose: minor cleanup (daemon logging, binary discovery) or start PRDv2 Feature Phase 1 — Layer Detection. The next-steps items are ordered by size — smallest first.
