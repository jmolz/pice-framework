# Handoff: Phase 0 Daemon Refactor — T15+T16 Complete, Ready for T17

**Date:** 2026-04-12
**Branch:** `v0.2/phase-0-daemon` (worktree at `.worktrees/phase-0-daemon/`)
**Last Commit:** `cbe85a8 feat(pice-daemon): add Windows named pipe transport (T16)`

## Goal

Execute Phase 0 of the PICE v0.2 refactor — split `pice-cli` into three crates (`pice-cli` thin adapter + `pice-daemon` orchestrator + `pice-core` shared logic) so Stack Loops, workflow YAML, and adaptive algorithms (v0.2 Phase 1+) have a foundation. **Tier 3 contract**, 16 criteria. See `.claude/plans/phase-0-daemon-foundation.md`.

## Recently Completed (This Session)

- [x] **T15: Unix socket transport** — `server/unix.rs` with `UnixSocketListener` (bind + stale-socket recovery + 0600 perms) and `UnixConnection` (newline-delimited JSON framing over `tokio::net::UnixStream`). 3 unit tests (bind/accept roundtrip with 0600 check, live-daemon conflict, malformed-frame parse error). (`188b2ac`)
- [x] **Stale-socket integration test split** — `tests/server_unix_stale_socket.rs` moved to its own integration test binary to dodge a macOS fd-inheritance race where sibling tests in `prompt::builders` spawn `git` subprocesses. The race is structural (concurrent fork + freshly-bound socket fd) and only surfaces on macOS. See dead-ends below. (`188b2ac`)
- [x] **CI matrix for Rust platform coverage** — New `rust-platform-coverage` job with `[macos-latest, windows-latest]` matrix. `fail-fast: false`. Runs `clippy` + `test` only (no pnpm, no fmt, no release build — those are platform-neutral and already run in the existing Linux job). (`6349484`)
- [x] **Windows cross-compile toolchain** — `brew install mingw-w64` + `rustup target add x86_64-pc-windows-gnu`. `cargo check --target x86_64-pc-windows-gnu --workspace --all-targets` clean.
- [x] **Framing extraction** — `server/framing.rs` with `JsonLineFramed<R, W>` generic over any `AsyncRead + AsyncWrite` pair. Extracted from `unix.rs` so both transports share the same framing (EOF, parse, buffer reuse, embedded-newline debug_assert). Zero behavior change on Unix. (`6203621`)
- [x] **T16: Windows named pipe transport** — `server/windows.rs` with `WindowsPipeListener` (bind with `first_pipe_instance(true)` for exclusive ownership) and `WindowsPipeConnection` (wraps `JsonLineFramed` over `tokio::io::split` halves of `NamedPipeServer`). 3 unit tests matching T15's shape (roundtrip, conflict, malformed). All gated `#[cfg(windows)]` — type-checked via cross-compile, runtime-validated only on Windows CI. (`cbe85a8`)

## In Progress / Next Steps

- [ ] **T17: Auth token** — generate 32 random bytes, hex-encode, write to `~/.pice/daemon.token` with 0600 perms, rotate on every daemon start. Reject requests missing/mismatched token with JSON-RPC error `-32002`. File: `crates/pice-daemon/src/server/auth.rs`.
- [ ] T18: RPC router wiring (transport + auth → handlers dispatch). File: `crates/pice-daemon/src/server/router.rs`.
- [ ] T19: 11 per-command handlers. Plan says: do `init.rs` + `execute.rs` manually (trivial + complex streaming exemplars), then dispatch subagents for the rest. Handlers MUST use `CommandResponse::Text { content: "…".to_string() }` (struct-variant, not newtype).
- [ ] T20: `PICE_DAEMON_INLINE=1` bypass in `pice_daemon::inline::run_command`.
- [ ] T21: Lifecycle (startup, signal handling, graceful shutdown budget of 10s).
- [ ] T22: CLI adapter refactor + auto-start (100ms health-check timeout, 2s bind wait).
- [ ] T23–T32: `pice daemon` subcommand, integration tests, CI/NPM updates, Tier 3 `/evaluate` of the whole phase.

## Key Decisions

- **Worktree isolation**: `.worktrees/phase-0-daemon/` — `main` stays shippable, rollback is `git reset --hard` in the worktree with zero main-branch blast radius.
- **`StreamSink` = `Arc<dyn StreamSink>` (aliased as `SharedSink`)** — not `&dyn` or a generic. Forced by `NotificationHandler = Box<dyn Fn + Send>` being `'static`.
- **Git rename preservation pattern**: move to a fresh path (e.g. `orchestrator/core.rs`), rewrite the pre-existing stub as a glue `mod.rs`.
- **T14 facade re-exports over call-site rewrites**: avoids double-churn (T14 move + T19 CLI-to-RPC rewrite).
- **`CommandResponse` struct variants (not newtype)**: serde's `#[serde(tag = "type")]` cannot serialize tagged newtypes wrapping primitives.
- **No stale-pipe recovery on Windows**: named pipes are refcounted kernel objects, not files on disk. `PermissionDenied` from `first_pipe_instance(true)` means a live process holds the handle — there is no "dead corpse" case. A probe via `ClientOptions::open` would consume the next-server slot and leave a doomed connection. See `server/windows.rs` module docs for the full rationale.
- **No `DaemonTransport` trait yet**: deferred to T18 where the router actually needs to consume both transports. Premature async traits in Rust are painful (bounds verbosity, GATs). The two impls intentionally mirror each other's API shape so the trait falls out naturally.
- **`JsonLineFramed` is transport-generic, splits stay platform-specific**: `unix.rs` uses `UnixStream::into_split` (lock-free, platform-specific). `windows.rs` uses `tokio::io::split` (generic, BiLock). Both satisfy `JsonLineFramed<R: AsyncRead, W: AsyncWrite>`.
- **Stale-socket test in integration binary**: moved out of `server::unix` unit tests to avoid macOS fd-inheritance race with `prompt::builders`' git subprocess spawning. See dead-ends below.

## Dead Ends (Don't Repeat These)

*(Preserved from prior sessions plus new entries)*

- **`use pice_core::X` inside pice-core itself** — `error[E0433]`. Use `crate::`.
- **Newtype variants in `#[serde(tag = "type")]` enums wrapping primitives** — use struct variants.
- **Batching `Edit` calls without prior `Read`** — the Edit tool requires each target file to be Read first.
- **`git mv` into an existing stub file** — loses rename detection. Move to fresh path + glue `mod.rs`.
- **`git checkout main` from a worktree** — fatal, `main` is already checked out elsewhere.
- **`#[cfg(test)]` on cross-crate test helpers** — invisible from downstream crate. Remove the gate.
- **`cargo clippy --workspace` without `--all-targets`** — silently ignores test-code warnings.
- **Process-global env tests without a mutex guard** — `set_var`/`remove_var` races. Use `Mutex<()>`.
- **Fighting `rustfmt` import ordering** — let `cargo fmt --all` run; don't hand-craft order.
- **macOS fd-inheritance race in stale-socket tests** — under parallel unit-test execution, sibling tests that `std::process::Command::new("git")` can inherit a freshly-bound socket fd before `Drop` runs, making the kernel treat a stale socket as live. Fix: move the stale-socket test to its own integration test binary (`tests/server_unix_stale_socket.rs`). The binary runs in its own process with no sibling forks. **Do not inline it back into `server::unix`.**
- **`#[derive(Debug)]` missing on listener types** — `Result::expect_err` requires `T: Debug` to format "expected error, got Ok(...)". Hit on both `UnixSocketListener` (T15) and `WindowsPipeListener` (T16). Always derive Debug on types used in test assertions.
- **Probing a named pipe you own via `ClientOptions::open`** — the probe pairs with your own next-server slot, leaving a doomed connection that immediately EOFs on the next `accept()`. Named pipes don't need probing; `first_pipe_instance(true)` + `PermissionDenied` is the entire conflict-detection story.
- **Phase plan spec said Windows stale pipes need probe-and-retry like Unix** — the spec was wrong. Named pipes are kernel objects; the kernel reclaims the name when the last handle closes (process death included). There is no stale-corpse case. The `PermissionDenied` error IS the liveness proof.

## Files Changed (This Session)

- `crates/pice-daemon/src/server/mod.rs` — registers `framing`, `#[cfg(unix)] pub mod unix`, `#[cfg(windows)] pub mod windows`
- `crates/pice-daemon/src/server/framing.rs` — NEW: `JsonLineFramed<R, W>` transport-generic framing
- `crates/pice-daemon/src/server/unix.rs` — NEW: `UnixSocketListener`, `UnixConnection`, 3 unit tests
- `crates/pice-daemon/src/server/windows.rs` — NEW: `WindowsPipeListener`, `WindowsPipeConnection`, 3 unit tests (Windows-only)
- `crates/pice-daemon/tests/server_unix_stale_socket.rs` — NEW: integration test isolating fd-race-sensitive stale-socket recovery
- `.github/workflows/ci.yml` — NEW job `rust-platform-coverage` with `[macos-latest, windows-latest]` matrix

## Current State

- **Tests:** **203 Rust** (12 binaries) + **49 TypeScript** — +4 from T14 baseline (3 Unix socket unit tests + 1 Unix stale-socket integration test; 3 Windows tests exist but only execute on Windows CI).
- **Build:** `cargo build --release` clean; both `pice` and `pice-daemon` binaries compile.
- **Cross-compile:** `cargo check --target x86_64-pc-windows-gnu --workspace --all-targets` clean (Windows transport + tests type-check).
- **Lint/Types:** `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm lint`, `pnpm typecheck` all clean.
- **pice-core purity:** `cargo tree -p pice-core -e normal | grep -E '(tokio|reqwest|rusqlite|hyper)'` empty ✅
- **Phase 0 progress:** **16/32 tasks complete** + CI matrix + cross-compile toolchain ready.

## Context for Next Session

T17 is the auth token module (`server/auth.rs`). Spec at line 640 of `.claude/plans/phase-0-daemon-foundation.md`. Key requirements:

1. Generate 32 random bytes, hex-encode → 64-char bearer token string
2. Write to `~/.pice/daemon.token` with 0600 perms (Unix) or default ACL (Windows)
3. Rotate on every daemon start (overwrite existing token file)
4. Validate incoming requests against the stored token
5. Reject mismatched/missing tokens with JSON-RPC error code `-32002`
6. **Never log the token. Never send it to providers. Never pass as process argument.**

T17 consumes the transport from T15/T16 (the auth layer sits between `accept()` and the router in T18). The token file path should come from `pice-core::transport::SocketPath` or a sibling config path resolver.

**Recommended first action:**
```bash
cd /Users/jacobmolz/code/pice-framework/.worktrees/phase-0-daemon
git log --oneline main..HEAD                    # expect 12 commits ending at cbe85a8
cargo test --workspace 2>&1 | grep "test result: ok" | awk '{s+=$4} END {print s}'  # expect: 203
# Then read, in order:
# 1. .claude/plans/phase-0-daemon-foundation.md (Task 17 section, line ~640)
# 2. .claude/rules/daemon.md (auth token section — 0600 perms, rotation, error code)
# 3. crates/pice-core/src/protocol/mod.rs (DaemonError codes — -32002 is AUTH_FAILED)
# 4. crates/pice-daemon/src/server/mod.rs (where auth.rs will be registered)
# Then implement: generate → write → validate → test roundtrip with bad token.
```
