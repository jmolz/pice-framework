# Handoff: v0.2 Post-Merge Cleanup Complete — Ready for Next Phase

**Date:** 2026-04-13
**Branch:** `main`
**Last Commit:** `424cbc7 fix(pice-core): support Windows path separators in project tree test`

## Goal

Clean up v0.2 main branch after Phase 0 + Phase 1 merge. All handlers ported, templates synced, docs updated, rules created. Branch is ready for the next feature phase.

## Recently Completed (This Session)

- [x] **Template sync** — 9 `templates/claude/` files synced with root `.claude/` (threshold 7→8, worktree awareness, PICE workflows)
- [x] **46 stale artifacts deleted** — per-crate `.claude/` test dirs + reference plan removed; `crates/*/.claude/` gitignored
- [x] **Test counts updated** — README badge (320), CONTRIBUTING.md (271), commit-and-deploy.md (271)
- [x] **Windows `.await` bug fixed** — `lifecycle.rs:169` missing `.await` on async `WindowsPipeListener::bind()`
- [x] **Created `.claude/rules/templates.md`** — template drift prevention rule with sync table (UNCOMMITTED)

## In Progress / Next Steps

- [ ] **Commit pending rule changes** — 3 files: `.claude/rules/templates.md` (new), `CLAUDE.md` (tech stack update), `.claude/rules/rust-core.md` (v0.2 marked current)
- [ ] **File-based daemon logging** — `logging.rs` uses stderr stub. Replace with `tracing_appender::rolling::daily("~/.pice/logs", "daemon.log")`
- [ ] **`pice daemon start` binary discovery** — PATH lookup only. Should check adjacent to CLI binary first (npm install case)
- [ ] **PRDv2 Phase 1: Layer Detection** — next major feature. Read `PRDv2.md` and `.claude/rules/stack-loops.md`

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
- **Uncommitted:** 3 files from `/create-rules` (rule file + 2 doc updates)

## Context for Next Session

Phase 0 + Phase 1 are complete and merged. The branch is clean except for 3 uncommitted doc/rule files. Commit those first, then choose: minor cleanup (daemon logging, binary discovery) or start PRDv2 Phase 1 (layer detection). The HANDOFF's "In Progress" items are ordered by size — smallest first.

**Recommended first action:**
```bash
git add .claude/rules/templates.md .claude/rules/rust-core.md CLAUDE.md && git commit -m "docs(rules): add template drift prevention rule and update v0.2 references"
```
