//! Adversarial Divergence-Triggered Scaling (ADTS).
//!
//! When Claude and Codex score the SAME pass differently by more than
//! [`AdtsConfig::divergence_threshold`], escalate compute on the next pass.
//! Three within-run levels (PRDv2:1126–1128 reading):
//!
//! | escalations_used | Verdict on divergence              | Effect on next pass |
//! |------------------|------------------------------------|---------------------|
//! | 0                | `ScheduleExtraPassFreshContext`    | Re-run with `fresh_context = true` |
//! | 1                | `ScheduleExtraPassElevatedEffort`  | Re-run with `effort = xhigh` (or equiv) |
//! | ≥ max            | `EscalationExhausted`              | Halt, layer Failed |
//!
//! Mid-run formal tier re-issuance (the literal PRDv2 reading) is explicitly
//! rejected per the Phase 4 plan Q&A locks (lines 9–11): tier is the committed
//! contract depth decided at plan time. ADTS escalates *compute*, not *identity*.

use crate::adaptive::types::{AdaptiveError, AdtsConfig, AdtsVerdict};

/// Per-pass paired score (claude_score, codex_score) on the 0–10 scale.
///
/// Type alias kept narrow to make the public API self-documenting without
/// introducing a new struct (which would need its own serde + Phase 7 protocol
/// extension to little benefit). The orchestrator pairs scores by `pass_index`
/// before calling.
pub type PairedScore = (f64, f64);

/// Inspect a sequence of paired scores and decide whether to escalate.
///
/// `escalations_used` is the count the orchestrator has applied so far in the
/// current layer's loop. The function does not mutate it — the caller bumps
/// it after acting on `ScheduleExtraPass*`.
///
/// Empty input returns [`AdtsVerdict::Continue`] (nothing to compare yet).
pub fn run_adts(
    paired_passes: &[PairedScore],
    escalations_used: u32,
    cfg: &AdtsConfig,
) -> Result<AdtsVerdict, AdaptiveError> {
    // Threshold validation — guards a NaN config silently classifying
    // every pair as non-divergent.
    if !(cfg.divergence_threshold.is_finite() && (0.0..=10.0).contains(&cfg.divergence_threshold)) {
        return Err(AdaptiveError::InvalidDivergenceThreshold(
            cfg.divergence_threshold,
        ));
    }

    // Validate every pair — finite and in [0, 10]. We check ALL pairs (not
    // just the latest) so a stale bad input surfaces deterministically.
    for &(claude, codex) in paired_passes {
        if !is_valid_score(claude) || !is_valid_score(codex) {
            return Err(AdaptiveError::InvalidAdtsScore { claude, codex });
        }
    }

    // No paired data → nothing to compare.
    let Some(&(claude, codex)) = paired_passes.last() else {
        return Ok(AdtsVerdict::Continue);
    };

    let divergence = (claude - codex).abs();
    if divergence <= cfg.divergence_threshold {
        return Ok(AdtsVerdict::Continue);
    }

    // Diverged. If we've used (or exceeded) the budget, exhaust immediately —
    // covers `max_divergence_escalations = 0` "always exhaust on divergence".
    if escalations_used >= cfg.max_divergence_escalations {
        return Ok(AdtsVerdict::EscalationExhausted);
    }

    // Escalation ladder — only two distinct compute escalations exist. If the
    // user configured `max > 2`, levels beyond 2 collapse to Exhausted because
    // there is no further compute lever to pull within the run.
    let verdict = match escalations_used {
        0 => AdtsVerdict::ScheduleExtraPassFreshContext,
        1 => AdtsVerdict::ScheduleExtraPassElevatedEffort,
        _ => AdtsVerdict::EscalationExhausted,
    };
    Ok(verdict)
}

fn is_valid_score(s: f64) -> bool {
    s.is_finite() && (0.0..=10.0).contains(&s)
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converged_scores_continue() {
        let cfg = AdtsConfig::default(); // divergence=2.0, max=2
                                         // |9.0 - 8.5| = 0.5 ≤ 2.0
        let verdict = run_adts(&[(9.0, 8.5)], 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::Continue);
    }

    #[test]
    fn divergence_at_threshold_does_not_escalate() {
        let cfg = AdtsConfig::default();
        // |9.0 - 7.0| = 2.0 == threshold (boundary is inclusive)
        let verdict = run_adts(&[(9.0, 7.0)], 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::Continue);
    }

    #[test]
    fn first_divergence_schedules_fresh_context() {
        let cfg = AdtsConfig::default();
        // |9.0 - 3.0| = 6.0 > 2.0
        let verdict = run_adts(&[(9.0, 3.0)], 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::ScheduleExtraPassFreshContext);
    }

    #[test]
    fn second_divergence_schedules_elevated_effort() {
        let cfg = AdtsConfig::default();
        // First and second pass both diverged; orchestrator already consumed
        // one escalation for Level 1.
        let history = vec![(9.0, 3.0), (9.0, 3.0)];
        let verdict = run_adts(&history, 1, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::ScheduleExtraPassElevatedEffort);
    }

    #[test]
    fn third_divergence_at_max_two_exhausts() {
        let cfg = AdtsConfig::default(); // max = 2
        let history = vec![(9.0, 3.0), (9.0, 3.0), (9.0, 3.0)];
        let verdict = run_adts(&history, 2, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::EscalationExhausted);
    }

    #[test]
    fn max_escalations_zero_always_exhausts_on_divergence() {
        let cfg = AdtsConfig {
            divergence_threshold: 2.0,
            max_divergence_escalations: 0,
        };
        // Diverged on the very first pass — nothing to spend, exhaust immediately.
        let verdict = run_adts(&[(9.0, 3.0)], 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::EscalationExhausted);

        // But convergence still continues — max = 0 only matters on divergence.
        let verdict = run_adts(&[(8.0, 8.5)], 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::Continue);
    }

    #[test]
    fn empty_paired_passes_continues() {
        let cfg = AdtsConfig::default();
        let verdict = run_adts(&[], 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::Continue);
    }

    #[test]
    fn nan_score_rejected() {
        let cfg = AdtsConfig::default();
        match run_adts(&[(f64::NAN, 5.0)], 0, &cfg) {
            Err(AdaptiveError::InvalidAdtsScore { claude, codex }) => {
                assert!(claude.is_nan());
                assert_eq!(codex, 5.0);
            }
            other => panic!("expected InvalidAdtsScore, got {other:?}"),
        }
        match run_adts(&[(5.0, f64::NAN)], 0, &cfg) {
            Err(AdaptiveError::InvalidAdtsScore { claude, codex }) => {
                assert_eq!(claude, 5.0);
                assert!(codex.is_nan());
            }
            other => panic!("expected InvalidAdtsScore, got {other:?}"),
        }
    }

    #[test]
    fn out_of_range_score_rejected() {
        let cfg = AdtsConfig::default();
        for bad in [-0.1f64, 10.1, 100.0, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                run_adts(&[(bad, 5.0)], 0, &cfg),
                Err(AdaptiveError::InvalidAdtsScore { .. })
            ));
        }
    }

    #[test]
    fn invalid_divergence_threshold_rejected() {
        for bad in [-0.1f64, 10.1, f64::NAN, f64::INFINITY] {
            let cfg = AdtsConfig {
                divergence_threshold: bad,
                max_divergence_escalations: 2,
            };
            assert!(matches!(
                run_adts(&[(8.0, 5.0)], 0, &cfg),
                Err(AdaptiveError::InvalidDivergenceThreshold(_))
            ));
        }
    }

    #[test]
    fn divergence_uses_only_latest_pair() {
        // Earlier pair was divergent, latest is converged — should Continue.
        let cfg = AdtsConfig::default();
        let history = vec![(9.0, 3.0), (8.0, 7.5)];
        let verdict = run_adts(&history, 0, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::Continue);
    }

    #[test]
    fn max_above_two_collapses_to_exhausted_after_level_two() {
        // If a user sets max = 5, there is no Level 3..5 compute lever; we
        // exhaust at escalations_used = 2 anyway. Documented behavior.
        let cfg = AdtsConfig {
            divergence_threshold: 2.0,
            max_divergence_escalations: 5,
        };
        let history = vec![(9.0, 3.0); 3];
        let verdict = run_adts(&history, 2, &cfg).unwrap();
        assert_eq!(verdict, AdtsVerdict::EscalationExhausted);
    }
}
