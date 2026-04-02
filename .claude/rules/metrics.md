---
paths:
  - "crates/pice-cli/src/metrics/**"
---

# Metrics & Telemetry Rules

## SQLite Schema

- Use `rusqlite` with WAL mode for concurrent read access
- Schema versioning via a `schema_version` table — check and migrate on startup
- Tables: `evaluations`, `criteria_scores`, `loop_events`, `telemetry_queue`
- All timestamps are UTC ISO 8601

## Passive Collection

Metrics are collected automatically by every orchestration command. No user action required:
- `pice plan` → records plan creation event
- `pice execute` → records execution start/complete
- `pice evaluate` → records per-criterion scores, pass/fail, tier, models used
- `pice commit` → records commit event linked to the plan

## Telemetry

- **Opt-in only.** Default is `false`. Set via `pice init` prompt or `.pice/config.toml`.
- **Anonymized.** No code, file paths, project names, or user identifiers.
- **Transparent.** Every payload is logged to `.pice/telemetry-log.jsonl` before sending.
- **Inspectable.** `pice telemetry show` displays recent payloads.
- Telemetry endpoint failures are silent (logged to debug, never user-facing errors).

## Aggregation Queries

`pice metrics` runs aggregation queries against the local SQLite DB:
- Total loops, pass rate, average score, trend (last 30 days)
- Output as terminal table (default), JSON (`--json`), or CSV (`--csv`)
