---
paths:
  - "templates/**"
  - "crates/pice-daemon/src/templates/**"
---

# Template & Scaffolding Rules

## Ownership

- `templates/claude/` — files scaffolded by `pice init` into `.claude/`
- `templates/pice/` — files scaffolded by `pice init` into `.pice/`
- `crates/pice-daemon/src/templates/mod.rs` — `rust-embed` embedding + extraction logic
- The **daemon** owns template extraction (init handler). The CLI delegates via adapter.

## Template Drift (CRITICAL)

`templates/claude/` and the root `.claude/` can drift when methodology improvements (threshold changes, worktree awareness, workflow updates) are made to the root `.claude/` commands without syncing back to `templates/`.

**Rule: when you update a file in `.claude/commands/` or `.claude/templates/`, check if the same file exists in `templates/claude/` and sync the change.**

Files that must stay in sync:

| Root | Template |
|------|----------|
| `.claude/commands/evaluate.md` | `templates/claude/commands/evaluate.md` |
| `.claude/commands/execute.md` | `templates/claude/commands/execute.md` |
| `.claude/commands/plan-feature.md` | `templates/claude/commands/plan-feature.md` |
| `.claude/commands/commit-and-deploy.md` | `templates/claude/commands/commit-and-deploy.md` |
| `.claude/commands/empty-redeploy.md` | `templates/claude/commands/empty-redeploy.md` |
| `.claude/commands/review.md` | `templates/claude/commands/review.md` |
| `.claude/commands/handoff.md` | `templates/claude/commands/handoff.md` |
| `.claude/commands/prime.md` | `templates/claude/commands/prime.md` |
| `.claude/templates/plan-template.md` | `templates/claude/templates/plan-template.md` |

Files that exist only in root (project-specific, not scaffolded):
- `.claude/PRD.md`, `.claude/rules/*.md`, `.claude/docs/*.md`, `.claude/plans/*.md`, `.claude/settings.local.json`, `.claude/skills/`

## Build-Time Embedding

- Templates are embedded via `rust-embed` at compile time. Changes to `templates/` require `cargo build` to take effect.
- The `rust-embed` derive is in `pice-daemon/src/templates/mod.rs`.
- Existing tests verify template files are embedded and extractable. They check file existence, not content.

## Per-Crate .claude/ Artifacts

Running `pice init` inside a crate subdirectory (e.g., during testing) creates a `.claude/` directory there. These are test artifacts, not tracked code:
- `crates/*/.claude/` is gitignored
- If you see per-crate `.claude/` directories, delete them — they are stale copies of the templates
