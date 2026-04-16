---
paths:
  - "crates/pice-cli/src/metrics/**"
  - "crates/pice-cli/src/commands/metrics.rs"
  - "crates/pice-cli/src/commands/benchmark.rs"
  - "crates/pice-daemon/src/state/**"
---

# Metrics & Telemetry Rules

## SQLite Schema

- Use `rusqlite` with WAL mode for concurrent read access
- Schema versioning via a `schema_version` table — check and migrate on startup
- Tables (v0.1): `evaluations`, `criteria_scores`, `loop_events`, `telemetry_queue`
- Tables (v0.2+): `gate_decisions`, `cost_events`, `seam_findings`, `layer_runs`
- Tables (v0.5+): `check_outcomes`, `model_predictions` (for predictive check selection)
- All timestamps are UTC ISO 8601 (RFC 3339)
- Multi-table inserts (e.g., evaluation + criteria_scores) must use `BEGIN TRANSACTION` / `COMMIT` / `ROLLBACK` to prevent orphaned rows
- Schema migrations are forward-only. Every new table or column gets a numbered migration. Never edit existing migration files.

## Passive Collection

Metrics are collected automatically by every orchestration command. No user action required:
- `pice plan` → records plan creation event
- `pice execute` → records execution start/complete
- `pice evaluate` → records per-criterion scores, pass/fail, tier, models used
- `pice commit` → records commit event linked to the plan

## Non-Fatal Recording Pattern

Workflow commands (evaluate, plan, execute, commit) must NEVER fail due to metrics errors. Use this pattern:

```rust
if let Ok(Some(db)) = metrics::open_metrics_db(&project_root) {
    if let Err(e) = metrics::store::record_loop_event(&db, ...) {
        tracing::warn!("failed to record event: {e}");
    }
}
```

- `open_metrics_db` returns `Ok(None)` when the DB file doesn't exist — the `if let` skips silently.
- `open_metrics_db` returns `Err(...)` for corrupt DBs — the `if let Ok(...)` skips silently.
- Recording errors use `tracing::warn!` (degraded behavior), telemetry errors use `tracing::debug!` (silent).
- **Reporting commands** (`pice metrics`, `pice benchmark`) MAY propagate DB errors — the user needs to know.

## Plan Path Normalization

Plan paths stored in metrics DB must be normalized to project-relative canonical form via `metrics::normalize_plan_path()`. This prevents history fragmentation from different path spellings (absolute, relative, `./`-prefixed).

- Always normalize before writing to `evaluations.plan_path` or `loop_events.plan_path`
- The canonical form is `.claude/plans/<filename>` — same format `pice status` uses for lookups
- Status enrichment queries use the same canonical form

## Telemetry

- **Opt-in only.** Default is `false`. Set via `pice init` prompt or `.pice/config.toml`.
- **Anonymized.** No code, file paths, project names, or user identifiers.
- **Transparent.** Every payload is logged to `.pice/telemetry-log.jsonl` before sending.
- **Inspectable.** `pice telemetry show` displays recent payloads.
- Telemetry endpoint failures are silent (logged to debug, never user-facing errors).

### HTTP Sending

Telemetry events are sent via HTTP POST using `telemetry::send_batch()` — the single implementation of the HTTP logic. Both the library/test path (`TelemetryClient::flush_inner()`) and the production path (`commands::evaluate::flush_telemetry()`) call this function.

- **Batch size:** Up to 50 pending events per flush
- **Timeout:** 10 seconds (`HTTP_TIMEOUT` constant)
- **TLS:** `reqwest` with `rustls-tls` (pure Rust, no OpenSSL dependency)
- **Production path:** `flush_telemetry()` reads pending events synchronously, then spawns a detached `tokio::spawn` for the HTTP POST so it never blocks CLI output. If the process exits before the spawn completes, unsent events stay in the SQLite queue and retry on the next `pice evaluate` invocation.
- **DB reopening:** The spawned task reopens `MetricsDb` to mark events as sent because `rusqlite::Connection` isn't `Sync` and can't cross the spawn boundary.

### Wire-Format Safety

Telemetry uses a separate `AnonymizedPayload` struct (not `TelemetryEvent` itself) as the wire format. The `anonymize()` function destructures `TelemetryEvent` exhaustively — adding a new field to `TelemetryEvent` causes a compile error, forcing an explicit decision about whether to include it in the wire format. This is a compile-time guarantee against accidental data leakage.

## Aggregation Queries

`pice metrics` runs aggregation queries against the local SQLite DB:
- Total loops, pass rate, average score, trend (last 30 days)
- Output as terminal table (default), JSON (`--json`), or CSV (`--csv`)
- CSV output uses RFC 4180 escaping (embedded quotes doubled, fields with commas/quotes/newlines wrapped)

## Init Behavior

- `pice init` creates a real SQLite DB with schema (not an empty file)
- `pice init --force` runs migrations on the existing DB — it NEVER deletes metrics history
- The DB path is resolved from `config.metrics.db_path`, not hardcoded

## v0.2+ Audit Trail

Every gate decision writes a row to `gate_decisions`:

```sql
CREATE TABLE gate_decisions (
    id INTEGER PRIMARY KEY,
    feature_id TEXT NOT NULL,
    layer TEXT NOT NULL,
    trigger_expression TEXT NOT NULL,
    decision TEXT NOT NULL,           -- approve | reject | skip | timeout_reject | timeout_approve | timeout_skip
    reviewer TEXT,                    -- $USER by default, or dashboard authenticated user
    reason TEXT,                      -- optional free text
    requested_at TEXT NOT NULL,       -- RFC 3339
    decided_at TEXT NOT NULL,
    elapsed_seconds INTEGER NOT NULL
);
```

- **Write-only from the caller's perspective.** The daemon INSERTs; nothing UPDATEs rows. A changed decision creates a new row (linked by `feature_id` + `layer`).
- **Deletion is explicit** via `pice audit prune --before DATE`. Default retention is 365 days via `[audit] retention_days` in `config.toml`.
- **Reviewer identity** is `$USER` for CLI-actioned gates. For dashboard gates, the reviewer comes from the dashboard session's token → user mapping (v0.3).
- Reporting surface: `pice audit gates [--feature F] [--since DATE]`.

## v0.2+ Cost Tracking

Every provider pass writes a row to `cost_events`:

```sql
CREATE TABLE cost_events (
    id INTEGER PRIMARY KEY,
    feature_id TEXT NOT NULL,
    layer TEXT,                   -- NULL for feature-level events
    pass_index INTEGER,           -- NULL for non-evaluation events
    provider TEXT NOT NULL,       -- "claude-code" | "codex" | ...
    model TEXT NOT NULL,
    cost_usd REAL NOT NULL,
    tokens_input INTEGER,
    tokens_output INTEGER,
    timestamp TEXT NOT NULL
);
```

- Cost is attributed to the layer and pass that incurred it.
- `pice metrics cost [--by-day|--by-feature|--by-layer]` reports aggregated spend.
- Budget enforcement reads from this table in real time — if adding the next projected pass would exceed `workflow.defaults.budget_usd`, the adaptive controller halts with `halted_by: budget`.
- The non-fatal recording pattern applies here too: a cost_events write failure must NOT abort the evaluation. Log at `warn` and continue.

## v0.2+ Layer Runs and Seam Findings

`layer_runs` captures per-layer execution summary:

```sql
CREATE TABLE layer_runs (
    id INTEGER PRIMARY KEY,
    feature_id TEXT NOT NULL,
    layer TEXT NOT NULL,
    status TEXT NOT NULL,          -- running | passed | failed | skipped | pending_review
    contract_tier INTEGER NOT NULL,
    passes INTEGER NOT NULL,
    final_confidence REAL,
    total_cost_usd REAL NOT NULL,
    halted_by TEXT,                -- sprt_confidence_reached | sprt_rejected | budget | max_passes | vec_entropy | gate_rejected
    started_at TEXT NOT NULL,
    completed_at TEXT
);
```

`seam_findings` captures per-boundary seam check results:

```sql
CREATE TABLE seam_findings (
    id INTEGER PRIMARY KEY,
    feature_id TEXT NOT NULL,
    boundary TEXT NOT NULL,         -- "backend↔database", "api↔frontend", ...
    check_id TEXT NOT NULL,         -- "schema_match", "openapi_compliance", ...
    severity TEXT NOT NULL,         -- pass | warn | fail
    category INTEGER NOT NULL,      -- 1–12 from the 12 seam failure categories
    details TEXT,
    timestamp TEXT NOT NULL
);
```

These tables are populated by the daemon. The CLI reads from them for `pice status` and `pice metrics`. Dashboard (v0.3) reads the same tables.

## v0.4+ Adaptive Evaluation

Phase 4 adds the `pass_events` table (one row per provider invocation) and five new columns on `evaluations` (post-loop adaptive summary). Schema migrations are guarded by `PRAGMA table_info` so they're idempotent across daemon restarts.

```sql
-- New columns added to existing evaluations table
ALTER TABLE evaluations ADD COLUMN passes_used         INTEGER;
ALTER TABLE evaluations ADD COLUMN halted_by           TEXT;
ALTER TABLE evaluations ADD COLUMN adaptive_algorithm  TEXT;
ALTER TABLE evaluations ADD COLUMN final_confidence    REAL;
ALTER TABLE evaluations ADD COLUMN final_total_cost_usd REAL;

-- New table: per-pass audit trail
CREATE TABLE pass_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    evaluation_id INTEGER NOT NULL,
    pass_index INTEGER NOT NULL,            -- 1-indexed within the layer's loop
    model TEXT NOT NULL,                    -- "stub-echo" | "claude-opus-4-7" | ...
    score REAL,                             -- nullable: provider may not score
    cost_usd REAL,                          -- nullable: provider may omit cost
    timestamp TEXT NOT NULL,                -- ISO 8601
    FOREIGN KEY (evaluation_id) REFERENCES evaluations(id) ON DELETE CASCADE
);
```

### Write-path invariants

- **Header insert before the loop, finalize after.** `insert_evaluation_header` writes a placeholder row with `passed=0, summary=NULL` so the adaptive loop has a valid `evaluation_id` to FK-attach pass_events to. `finalize_evaluation` rewrites `passed` and `summary` after the loop returns.
- **`pass_events` writes happen BEFORE the halt-decision check.** A budget-halted loop still records the triggering pass cost. Skipping this would silently undercount on every budget halt.
- **`update_evaluation_adaptive_summary` aggregates across layers.** `passes_used = SUM(layer.passes.len())`, `final_total_cost_usd = SUM(layer.total_cost_usd)` (so `evaluations.final_total_cost_usd` matches `SUM(pass_events.cost_usd)` per evaluation_id within 1e-9).
- **`MetricsDb` is `!Sync`.** The rusqlite Connection's prepared-statement cache uses `RefCell`. The daemon wraps it in `Arc<Mutex<MetricsDb>>` so the per-pass sink can stay `Send` across the orchestrator's `await`. No contention because the sink is the only writer during the loop.
- **`PRAGMA foreign_keys = ON` is set on every connection.** Required for `ON DELETE CASCADE` to fire when an evaluation row is deleted (e.g., during retention-policy GC).
- **Concurrent isolation.** Two concurrent evaluations on different features write to disjoint `evaluation_id` groups in the same DB. There is no shared lock between evaluations beyond per-DB serialization (Phase 4 contract criterion #17).

## v0.5 Predictive Selection Data

When v0.5 ships, `check_outcomes` captures the label needed for model training:

```sql
CREATE TABLE check_outcomes (
    id INTEGER PRIMARY KEY,
    feature_id TEXT NOT NULL,
    check_id TEXT NOT NULL,
    fired BOOLEAN NOT NULL,         -- did the check flag an issue?
    true_positive BOOLEAN,          -- was the flag a real bug? NULL until labeled
    labeled_at TEXT,
    labeled_by TEXT,                -- "developer" | "inferred"
    cost_usd REAL NOT NULL,
    timestamp TEXT NOT NULL
);
```

Labels come from `pice feedback {true-positive|false-positive}` (explicit) or inferred from post-evaluation commit patterns (heuristic). The `pice model train` command consumes this table to train the predictive check selector.
