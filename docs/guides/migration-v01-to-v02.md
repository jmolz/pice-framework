# Migration Guide: PICE v0.1 to v0.2

## What changed

### Architecture

v0.2 introduces a **headless daemon + CLI adapter split**. The Rust core becomes a long-running `pice-daemon` process; `pice` becomes a thin CLI adapter that communicates with the daemon over a Unix socket (macOS/Linux) or named pipe (Windows). This is transparent to users — the CLI auto-starts the daemon on first command.

### Layer-aware evaluation

v0.1 evaluates features as monolithic units — one contract, one diff, one evaluation loop. v0.2 introduces **Stack Loops**: per-layer evaluation where a feature is PASS only when every layer passes. This catches the cross-layer blind spots that v0.1 misses (missing env vars, broken deploys, schema drift).

## How to upgrade

### Quick upgrade

```bash
pice init --upgrade
```

This generates:
- `.pice/layers.toml` — detected layer configuration for your project
- `.pice/contracts/` — default contract templates for each layer (backend, database, api, frontend, infrastructure, deployment, observability)

Review and commit both files.

### Manual upgrade

1. Run `pice layers detect` to see what PICE detects
2. Run `pice layers detect --write` to write `.pice/layers.toml`
3. Edit the file to match your project's actual architecture
4. Run `pice layers check` to verify coverage
5. Run `pice layers graph` to visualize dependencies
6. Commit `.pice/layers.toml`

## New files

| File | Purpose |
|------|---------|
| `.pice/layers.toml` | Layer definitions: paths, dependencies, always-run flags |
| `.pice/contracts/*.toml` | Per-layer evaluation contracts (criteria, thresholds) |

## Backwards compatibility

**Without `.pice/layers.toml`**, v0.2 falls back to v0.1 single-loop evaluation with a warning. All existing commands work unchanged. You can adopt layers incrementally.

**v0.1 providers still work.** The provider protocol extensions (layer fields on `session/create`) are optional. Providers that don't declare `layerAware: true` are driven in single-layer fallback mode.

## New commands

| Command | Purpose |
|---------|---------|
| `pice layers detect` | Run detection, print proposed layers.toml |
| `pice layers detect --write` | Write detected layers to `.pice/layers.toml` |
| `pice layers list` | Show current layer configuration |
| `pice layers check` | Report files not matched by any layer |
| `pice layers graph` | ASCII diagram of layer dependencies |
| `pice init --upgrade` | Generate layers.toml + contracts for v0.1 projects |

## Layer detection

PICE uses a six-level heuristic stack to detect layers:

1. **Manifest files** — `package.json`, `Cargo.toml`, `pyproject.toml`, etc.
2. **Directory patterns** — `terraform/`, `deploy/`, `pages/`, etc.
3. **Framework signals** — Next.js, Express, FastAPI, Rails, SvelteKit
4. **Config files** — Dockerfile, docker-compose.yml, CI configs
5. **Import graph** — (stubbed in v0.2, full analysis in v0.3)
6. **Override file** — `.pice/layers.toml` always wins

## What to review after upgrade

1. **Layer paths**: Verify each layer's `paths` globs match your actual file layout
2. **Dependencies**: Check `depends_on` reflects your real dependency graph
3. **Always-run layers**: Infrastructure, deployment, and observability are `always_run = true` by default — they evaluate on every change regardless of scope
4. **Contracts**: Customize `.pice/contracts/*.toml` criteria for your project's specific quality standards
