---
paths:
  - "crates/pice-cli/src/metrics/**"
  - "crates/pice-cli/src/commands/metrics.rs"
  - "crates/pice-cli/src/commands/benchmark.rs"
---

# Metrics & Telemetry Rules

## SQLite Schema

- Use `rusqlite` with WAL mode for concurrent read access
- Schema versioning via a `schema_version` table — check and migrate on startup
- Tables: `evaluations`, `criteria_scores`, `loop_events`, `telemetry_queue`
- All timestamps are UTC ISO 8601 (RFC 3339)
- Multi-table inserts (e.g., evaluation + criteria_scores) must use `BEGIN TRANSACTION` / `COMMIT` / `ROLLBACK` to prevent orphaned rows

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
