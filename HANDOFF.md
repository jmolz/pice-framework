# Handoff: MVP Complete — Pre-Launch Polish

**Date:** 2026-04-03
**Branch:** main
**Last Commit:** 9282b6e docs(readme): add test count badge — 217 tests passing

## Goal

Prepare the PICE CLI repo for public launch (Hacker News Show HN, Monday/Tuesday). All 5 PRD phases are complete and pushed to `jmolz/pice-framework` on GitHub.

## Recently Completed (This Session)

- [x] Extracted shared `send_batch()` telemetry function — eliminated duplicated HTTP logic between telemetry.rs and evaluate.rs
- [x] Updated rules: CLAUDE.md test baseline 167→168, metrics.md HTTP Sending subsection
- [x] Pushed to GitHub remote `jmolz/pice-framework` (merged LICENSE from remote)
- [x] Fixed CI: added `pnpm install && pnpm build` before `cargo test` (stub provider needs compiled JS), moved `pnpm build` before `pnpm typecheck` (workspace references)
- [x] Fixed README: PICE phases corrected (Plan, Implement, Contract-Evaluate — not four separate phases), badge URLs fixed for `jmolz/pice-framework`, removed unpublished crates.io/npm badges, added 217-tests-passing badge

## Current State

- **Tests:** 168 Rust + 49 TypeScript = 217, all passing locally
- **Build:** clean (debug + release)
- **Lint/Types:** 0 clippy warnings, 0 TS errors, clean formatting
- **CI:** Fix pushed (d33d257), awaiting green — verify before launch
- **Remote:** `jmolz/pice-framework` on GitHub, MIT licensed

## In Progress / Next Steps (Pre-Launch)

- [ ] Verify CI is green after the fix push — check GitHub Actions
- [ ] Add GitHub topics: `ai`, `cli`, `rust`, `developer-tools`, `code-quality`, `evaluation` (repo Settings > General)
- [ ] Create v0.1.0 release tag — `git tag v0.1.0 && git push origin v0.1.0` (gives a Releases sidebar entry; full release workflow needs NPM_TOKEN secret)
- [ ] Add FAQ section to README — pre-empt HN questions: "why not aider/cursor", "why Rust+TS", "telemetry concerns", "does this improve quality"
- [ ] Add example output to README — screenshot or code block of `pice evaluate` showing dual-model report with scores
- [ ] Enable GitHub Discussions
- [ ] Create issue templates (bug report, feature request, provider request)
- [ ] Post Show HN — target Monday or Tuesday morning ET for best traction

## Key Decisions

- **PICE = 3 phases, not 4**: Plan (includes contract negotiation), Implement, Contract-Evaluate. The acronym maps P-I-C-E but Contract-Evaluate is one phase.
- **reqwest with rustls-tls**: Pure-Rust TLS avoids OpenSSL dependency for cross-compilation.
- **Fire-and-forget telemetry**: `tokio::spawn` in evaluate.rs; unsent events retry next invocation via SQLite queue.
- **NPM optionalDependencies pattern**: Same as Biome/Turbo/esbuild for binary distribution.

## Context for Next Session

MVP is complete and on GitHub. CI fix is pushed but needs verification. The main work before launch is repo polish: topics, release tag, FAQ, example output, and issue templates. None of this is code — it's presentation and community readiness.

**Recommended first action:** Check CI status at `https://github.com/jmolz/pice-framework/actions`, then work through the pre-launch checklist above.
