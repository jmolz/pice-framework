# Evaluation System

## Purpose

The Evaluate phase grades an implementation against its plan's contract using
context-isolated AI evaluators. Evaluators see only three things: the contract JSON,
the git diff, and the project's CLAUDE.md. They never see the planning conversation,
implementation session, or any other context.

This isolation eliminates self-evaluation bias. The evaluator cannot rationalize a
shortcut because it does not know why the shortcut was taken. It sees the contract
criteria, the code that was produced, and the project's coding standards. Nothing else.

## Dual-Model Adversarial Evaluation

PICE uses evaluators from different model families because different models have
different blind spots. A single model reviewing its own work (or work produced by the
same model family) tends to share the same assumptions, miss the same edge cases, and
overlook the same design flaws.

Dual-model evaluation addresses this with two distinct roles:

### Contract Grading (Claude)

The primary evaluator is Claude. It performs formal, per-criterion scoring against the
contract:

- Reads each criterion's name, threshold, and validation command
- Examines the git diff for evidence of implementation
- Assigns a score (1-10) to each criterion
- Provides a brief justification for each score

Claude's output is structured: an array of `{ criterion, score, justification }`
objects. The Rust core consumes this structure directly.

### Design Challenge (GPT-5.4 / Codex)

The adversarial evaluator is a model from a different family -- GPT-5.4 via the Codex
provider by default. Its role is fundamentally different from contract grading. It
challenges the approach itself:

- Are there design tradeoffs the implementation did not consider?
- Does the code make unstated assumptions that could break under different conditions?
- Are there failure modes that the tests do not cover?
- Would a different approach have been simpler, more maintainable, or more robust?

The adversarial review does not produce pass/fail scores. It produces findings --
concerns, suggestions, and alternative approaches. These appear in the evaluation
report as "considerations" alongside the contract scores.

### Why Different Model Families

A model trained on different data with different objectives brings genuinely independent
judgment. It may flag issues that the primary evaluator consistently overlooks, or
question assumptions it takes for granted. Formal contract grading paired with
open-ended design critique from a different model family produces more thorough
evaluation than either model alone.

## Context Isolation

Evaluator sessions receive a controlled, minimal context:

| Input | Description |
|-------|-------------|
| Contract JSON | The criteria, thresholds, and validation commands from the plan |
| Git diff | The actual code changes produced by implementation |
| CLAUDE.md | The project's coding standards, patterns, and conventions |

Evaluators do not receive:

- The planning conversation or rationale
- The implementation session's chat history
- The developer's verbal instructions
- Previous evaluation results
- Any explanation of why the code looks the way it does

This is enforced at the protocol level. The `evaluate/create` JSON-RPC method accepts
only `contract`, `diff`, and `claude_md` parameters. There is no mechanism for passing
additional context even if a provider wanted to.

## Tier System

The evaluation tier is declared in the contract and determines the evaluator
configuration:

### Tier 1: Single Evaluator

For bug fixes, configuration changes, and simple refactors. One Claude evaluator
session performs contract grading only -- no adversarial review. Fast and inexpensive.

### Tier 2: Dual-Model Parallel

For new features, integrations, and multi-file changes. This is the most common tier.
One Claude evaluator for contract grading and one Codex evaluator for adversarial
design review run in parallel via `tokio::join!`. Results are synthesized into a
unified report. If Codex fails, evaluation falls back to Tier 1 with a warning.

### Tier 3: Full Team

For architectural changes, new subsystems, and core refactors. Maximum rigor. A Claude
agent team runs four specialized evaluators in parallel:

- **Contract evaluator** -- scores each criterion (same as Tier 1/2)
- **Convention auditor** -- checks adherence to CLAUDE.md patterns
- **Regression hunter** -- looks for unintended side effects in the diff
- **Edge case breaker** -- probes for boundary conditions and failure modes

Additionally, a Codex evaluator runs at maximum reasoning depth (`xhigh` effort). All
five evaluators execute in parallel and results are synthesized into a comprehensive
report.

## Contract Enforcement

The Rust core does not trust any provider's pass/fail verdict. After collecting scores
from evaluators, the core recomputes the result independently:

1. For each criterion, check whether the evaluator's score meets the criterion's
   threshold
2. If every criterion passes, the contract passes
3. If any criterion fails, the contract fails
4. The exit code reflects the result: 0 for pass, 2 for contract failure

This separation of scoring and enforcement is intentional. Providers produce scores.
The core enforces thresholds. No provider can declare a contract "passed" if the
numbers do not support it.

Example evaluation output:

```
Contract: API Rate Limiting (Tier 2)

  Tests pass                    8/7  PASS
  Rate limit middleware exists  9/7  PASS
  No lint warnings              8/8  PASS

Contract: PASS (3/3 criteria met)

Adversarial Review (GPT-5.4):
  - Consider: Token bucket refill rate is hardcoded; configuration would
    improve flexibility
  - Consider: No test for concurrent request handling under rate limit

Exit code: 0
```

## Graceful Degradation

Evaluation must not fail because an optional evaluator is unavailable. The degradation
rules are:

- **No Codex API key configured**: Tier 2/3 evaluations run as Tier 1 (Claude only)
  with a warning message
- **Codex provider times out**: The Claude evaluation result is used alone; the
  adversarial section of the report notes the timeout
- **Codex provider crashes**: Same as timeout -- Claude results are used, crash is
  logged
- **Claude provider fails**: This is a hard failure -- evaluation cannot proceed
  without contract grading. The CLI exits with code 1.

The principle is: adversarial review enhances evaluation but is not required. Contract
grading is required.

## User-Configurable Models

Evaluation models are not hardcoded. Users configure them in `.pice/config.toml`:

```toml
[evaluation]
contract_provider = "claude-code"
contract_model = "claude-opus-4-6"
adversarial_provider = "codex"
adversarial_model = "gpt-5.4"
```

This lets users swap in newer models, use cheaper models for iteration, or configure
community-built providers for alternative model families.

## Running Evaluation

```bash
pice evaluate plans/rate-limiting.md           # uses contract's declared tier
pice evaluate plans/rate-limiting.md --json    # JSON output for CI integration
```

If the contract passes, the feature is ready for `pice review` and `pice commit`. If
it fails, the developer reviews failing criteria and adversarial findings, then fixes
the issues, revises the plan, or adjusts thresholds. Failed evaluations are stored in
the metrics database for trend analysis via `pice metrics`.

## Further Reading

- [PICE Overview](overview.md) -- The full lifecycle
- [Plan Phase](plan.md) -- Where contracts are negotiated
- [Implement Phase](implement.md) -- What evaluation grades
- [Contract Format](contract.md) -- The criteria and thresholds evaluated against
