# Pre-Launch Repo Polish — Design Spec

Prepare the PICE CLI repo for public launch (Show HN, Monday/Tuesday). All work is presentation and community infrastructure — no CLI code changes.

## Scope

### In Scope

1. **README: Example output section** — Static code block of `pice evaluate` Tier 2 report after Quick Start
2. **README: FAQ section** — Four entries pre-empting HN questions, before Contributing
3. **GitHub topics** — `ai`, `cli`, `rust`, `typescript`, `developer-tools`, `code-quality`, `evaluation`, `ai-coding`, `workflow-orchestration`
4. **v0.1.0 release** — Tag + GitHub release with auto-generated notes (no binary uploads)
5. **GitHub Discussions** — Enable with default categories
6. **Issue templates** — YAML form-based: Bug Report, Feature Request, Provider Request + config.yml linking Discussions
7. **CI verification** — Already green, final confirmation during execution

### Out of Scope

- FUNDING.yml (declined)
- Screenshots/asciinema (static code block chosen)
- CLI code changes
- NPM publish or binary release workflow
- Binary uploads to the release

## Design Decisions

### Example Output

A fenced code block showing a realistic Tier 2 `pice evaluate` report using the exact Unicode box-drawing format from `crates/pice-cli/src/engine/output.rs`. Shows:
- Header: "Evaluation Report — Tier 2"
- 4 criteria with passing scores (8/7, 9/7, 8/8, 7/7)
- Adversarial review section with 2 design challenges
- Overall PASS result with summary

Placed as `## Example` between Quick Start and Commands.

### FAQ Content

Four Q&A entries in `## FAQ` section placed before Contributing:
1. **"Why not aider/cursor/copilot?"** — PICE is the orchestration layer, not a replacement. Works with any tool via provider protocol.
2. **"Why Rust + TypeScript?"** — Rust for fast CLI core. TS because AI SDKs (Claude, OpenAI) are JavaScript-first.
3. **"Is the telemetry sketchy?"** — Opt-in, off by default. Data inspectable locally in `.pice/telemetry-log.jsonl` before anything leaves your machine.
4. **"Does this actually improve code quality?"** — That's what the metrics engine measures. Data over vibes.

### Issue Templates (YAML Form-Based)

Three templates in `.github/ISSUE_TEMPLATE/`:

**bug-report.yml**: OS (dropdown), PICE version (text, required), steps to reproduce (textarea, required), expected behavior (textarea, required), actual behavior (textarea, required), provider (dropdown: claude-code/codex/other)

**feature-request.yml**: Description (textarea, required), use case (textarea, required), alternatives considered (textarea)

**provider-request.yml**: AI tool name (text, required), SDK/API docs link (text), workflow commands needed (checkboxes: plan/execute/evaluate/review/commit), additional context (textarea)

**config.yml**: Adds "Ask a Question" link pointing to GitHub Discussions Q&A category.

### GitHub Configuration

All done via `gh` CLI:
- `gh repo edit --add-topic` for each topic
- `gh release create v0.1.0 --generate-notes` for the release
- `gh api` to enable Discussions
