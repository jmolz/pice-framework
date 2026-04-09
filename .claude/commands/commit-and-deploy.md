---
description: Standard workflow for building, testing, committing and pushing PICE framework changes
---

# Commit and Deploy Workflow

## Pre-Commit Validation

Run these checks in order. Fix failures before proceeding.

### 1. Rust checks

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

### 2. TypeScript checks

```bash
pnpm lint
pnpm typecheck
pnpm test
```

### 3. Full builds

```bash
cargo build --release
pnpm build
```

**Expected baseline:** 168 Rust tests, 49 TypeScript tests, 0 lint errors, 0 warnings, clean release build.

## Commit by Feature (CRITICAL)

**Do NOT create one giant commit.** Group changes by feature/purpose.

```bash
# Review everything
git status

# Stage and commit by logical group — examples:
git add crates/pice-cli/src/engine/*.rs
git commit -m "feat(engine): add session capture support"

git add packages/provider-claude-code/src/*.ts
git commit -m "feat(provider): implement streaming notifications"

git add templates/
git commit -m "chore(templates): update init scaffolding"

# Docs last, separately
git add docs/ README.md CONTRIBUTING.md
git commit -m "docs: update architecture diagrams"

# Plans separately
git add .claude/plans/
git commit -m "docs(plans): add feature plan"
```

**Commit tags:** `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`

**Include scope:** `feat(engine)`, `fix(provider)`, `refactor(protocol)`, `docs(readme)`

If any AI layer files changed (CLAUDE.md, .claude/rules/, .claude/commands/), add a `Context:` section to the commit body.

## Push

```bash
git push origin main
```

CI runs automatically via GitHub Actions (`.github/workflows/ci.yml`).

## Verify

```bash
git status
# Expected: "nothing to commit, working tree clean"
```

Check CI status:

```bash
gh run list --limit 1
```
