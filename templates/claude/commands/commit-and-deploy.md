---
description: Standard workflow for building, testing, committing and deploying Alpaka changes
---

# Commit and Deploy Workflow

## Pre-Commit Validation

Run these checks in order. Fix failures before proceeding.

### 1. Build

```bash
pnpm build
```

### 2. Check maxDuration limits

```bash
grep -Ern 'maxDuration[[:space:]]*=[[:space:]]*(80[1-9]|8[1-9][0-9]|9[0-9][0-9]|[0-9]{4,})' --include='*.ts' app/api/ && echo "ERROR: maxDuration exceeds 800s Vercel Pro limit" || echo "OK: all maxDuration values within limit"
```

### 3. Pipeline pattern validation (required when touching pipeline/job code)

```bash
./scripts/validate-pipeline-patterns.sh
```

### 4. Regression tests (REQUIRED for any potentially breaking change)

```bash
pnpm test -- __tests__/regression/
```

### 5. RON tests (required when touching ron/**)

```bash
cd ron && python3 -m pytest __tests__/test_financial_extraction.py __tests__/test_title_generation.py -v
```

### 6. RON Docker build (required when touching ron/**)

```bash
docker build -f ron/Dockerfile .
```

### 7. Full unit tests

```bash
pnpm test
```

## Documentation Updates (REQUIRED)

Before committing:

1. **TASK.md**: Mark completed tasks `[x]`, add discovered issues, update dates
2. **Help/Knowledge/Security docs**: If product behavior changed, review for accuracy, clarity, structure, terminology, parity, and metadata
3. **README.md**: Update if commands, env vars, architecture, or setup changed
4. **ron/README.md**: Update if RON endpoints, models, config, or architecture changed

## Commit by Feature (CRITICAL)

**Do NOT create one giant commit.** Group changes by feature/purpose.

```bash
# Review everything
git status

# Stage and commit by logical group
git add components/projects/*.tsx
git commit -m "feat(ui): enhance project engagements tab"

git add app/api/projects/[id]/engagements/route.ts
git commit -m "feat(api): include AI summary in engagements response"

git add lib/db/schema*.ts lib/db/migrations/*.sql
git commit -m "feat(db): add communication schema"

# Docs last, separately
git add TASK.md README.md ron/README.md
git commit -m "docs: update task tracking and documentation"
```

**Commit tags:** `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`

**Include scope:** `feat(ui)`, `fix(api)`, `refactor(db)`, `feat(ron)`

If any AI layer files changed (CLAUDE.md, .claude/rules/, .claude/commands/), add a `Context:` section to the commit body.

## Deploy

```bash
git push origin main
```

**Frontend (Vercel):** Auto-deploys in ~2-5 minutes.
**RON Backend (RunPod):** Auto-deploys in ~15-20 minutes if `ron/**` or `shared/**` changed.

## Verify

```bash
git status
# Expected: "nothing to commit, working tree clean"
```

If RON deploy fails: check GitHub Actions logs, verify `docker build -f ron/Dockerfile .` locally, check RunPod console.
