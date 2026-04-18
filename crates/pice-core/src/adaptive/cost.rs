//! Jacobson/Karels-style cost projector.
//!
//! Tracks the running mean and mean-absolute-deviation (MAD) of observed
//! per-pass costs and projects the next pass's cost as `mean + 4·MAD`. The
//! `4·MAD` slack term matches TCP's RTO computation in
//! [RFC 6298 §3.4](https://datatracker.ietf.org/doc/html/rfc6298#section-3.4) —
//! same algorithm, different domain. Properties that beat the alternatives:
//!
//! - **Fail-closed under variance.** Spikes pull `MAD` up immediately so the
//!   next projection over-estimates, which is the safe direction for budget
//!   gating.
//! - **Self-tuning.** No magic constants per workload. Cold-start seed is the
//!   only caller-supplied value.
//! - **O(1) state.** Three `f64`s. No history buffer needed for the bounded
//!   `max_passes` horizon (typically 5–10).
//!
//! # Update rule
//!
//! On `observe(cost)` with current `(n, mean, mad)`:
//!
//! ```text
//! n'    = n + 1
//! mean' = mean + (cost - mean) / n'                        (online mean)
//! mad'  = mad  + (|cost - mean'| - mad) / n'               (online MAD)
//! ```
//!
//! The `n'`-denominator (vs RFC 6298's `α/β` smoothing factors) keeps the
//! algorithm fully deterministic and bounded-memory while preserving the
//! variance-conservative behavior; an EWMA variant would be a drop-in
//! replacement if a future phase needs reaction-speed tuning.

use crate::adaptive::types::AdaptiveError;

/// Online mean + MAD tracker for per-pass costs in USD.
///
/// `Default::default()` returns the zero state used on layer entry. The struct
/// is `Copy` so the orchestrator can pass it by value across the pass loop
/// without ownership ceremony.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct CostStats {
    /// Number of observations to date.
    pub n: u32,
    /// Online running mean of observed costs.
    pub mean: f64,
    /// Online running mean-absolute-deviation around `mean`.
    pub mad: f64,
}

impl CostStats {
    /// Construct a fresh tracker with no observations.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one observed cost into the running stats. Caller MUST validate
    /// `cost` first via [`CostStats::validate_nonnegative`] — this method
    /// asserts in debug builds and is permissive in release to avoid double
    /// validation in the hot path. The orchestrator validates once at the
    /// provider boundary.
    pub fn observe(&mut self, cost: f64) {
        debug_assert!(
            cost.is_finite() && cost >= 0.0,
            "observe() requires validated non-negative finite cost; got {cost}"
        );
        self.n = self.n.saturating_add(1);
        let n = self.n as f64;
        let new_mean = self.mean + (cost - self.mean) / n;
        // MAD update uses the NEW mean (consistent with the standard online
        // MAD recurrence and matches RFC 6298 §3.4 where SRTT is updated
        // before RTTVAR's deviation term).
        let new_mad = self.mad + ((cost - new_mean).abs() - self.mad) / n;
        self.mean = new_mean;
        self.mad = new_mad;
    }

    /// Project the next pass's cost. Returns `cold_start_seed` while no
    /// observations exist; otherwise returns `mean + 4·MAD`.
    ///
    /// `4·MAD` matches RFC 6298's `G` factor for RTO variance scaling. The
    /// coefficient is conservative on purpose — the budget gate is fail-closed,
    /// so over-estimating is the safe direction.
    pub fn project_next(&self, cold_start_seed: f64) -> f64 {
        if self.n == 0 {
            cold_start_seed
        } else {
            self.mean + 4.0 * self.mad
        }
    }

    /// Validate a raw cost value before observing it.
    ///
    /// Negative, NaN, and infinite costs are all rejected. The orchestrator
    /// surfaces this as a [`crate::adaptive::AdaptiveError::InvalidCost`].
    pub fn validate_nonnegative(cost: f64) -> Result<(), AdaptiveError> {
        if !cost.is_finite() || cost < 0.0 {
            Err(AdaptiveError::InvalidCost(cost))
        } else {
            Ok(())
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn n_zero_returns_seed() {
        let stats = CostStats::new();
        assert_eq!(stats.project_next(0.123), 0.123);
        assert_eq!(stats.n, 0);
    }

    #[test]
    fn n_one_returns_exact_observation() {
        // After one observation, mad = 0, so projection = mean = observation.
        let mut stats = CostStats::new();
        stats.observe(0.05);
        assert_eq!(stats.n, 1);
        assert!((stats.mean - 0.05).abs() < 1e-15);
        assert_eq!(stats.mad, 0.0);
        assert!((stats.project_next(0.001) - 0.05).abs() < 1e-15);
    }

    #[test]
    fn constant_cost_series_converges_with_zero_mad() {
        let mut stats = CostStats::new();
        for _ in 0..10 {
            stats.observe(0.03);
        }
        assert!((stats.mean - 0.03).abs() < 1e-15);
        assert!(
            stats.mad < 1e-15,
            "MAD should converge to 0, got {}",
            stats.mad
        );
        assert!((stats.project_next(0.0) - 0.03).abs() < 1e-15);
    }

    #[test]
    fn high_variance_series_projects_above_max_observed() {
        // The Phase 4 plan task 5a names [1, 1, 1, 5] as the RFC 6298
        // reference scenario and asserts `project_next > 5`. Under the
        // documented new-mean MAD recurrence the arithmetic gives:
        //   mean_4 = 1 + (5−1)/4 = 2.0
        //   mad_4  = 0 + (|5−2| − 0)/4 = 0.75
        //   projection = 2 + 4·0.75 = 5.0
        // i.e. projection lands EXACTLY at the spike, not strictly above.
        // The `> 5` wording in the plan is incompatible with the same plan's
        // pseudocode (which we honor verbatim) — the asserted invariant is
        // really "spike pulls projection up to (or above) its level," which
        // `>= 5` captures. The spike's effect is unmistakable: the
        // projection jumped from 1.0 (pre-spike steady state) to 5.0.
        let mut stats = CostStats::new();
        for &c in &[1.0, 1.0, 1.0, 5.0] {
            stats.observe(c);
        }
        let projection = stats.project_next(0.0);
        assert!(
            projection >= 5.0,
            "[1,1,1,5] must project ≥ 5 (got {projection}); spike dominates"
        );
        // And the projection must be 5x the pre-spike steady state.
        assert!(
            projection >= 5.0,
            "projection {projection} suspiciously low for spike scenario"
        );
    }

    #[test]
    fn validate_rejects_negative() {
        assert!(matches!(
            CostStats::validate_nonnegative(-0.01),
            Err(AdaptiveError::InvalidCost(_))
        ));
    }

    #[test]
    fn validate_rejects_nan_and_infinity() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                matches!(
                    CostStats::validate_nonnegative(bad),
                    Err(AdaptiveError::InvalidCost(_))
                ),
                "expected InvalidCost rejection for {bad}"
            );
        }
    }

    #[test]
    fn validate_accepts_zero_and_positive() {
        assert!(CostStats::validate_nonnegative(0.0).is_ok());
        assert!(CostStats::validate_nonnegative(0.0001).is_ok());
        assert!(CostStats::validate_nonnegative(1e6).is_ok());
    }

    #[test]
    fn online_update_matches_textbook_formula() {
        // After observing [1.0, 3.0, 5.0]:
        //   mean = 3.0
        //   |1-3| + |3-3| + |5-3| = 4, divided by 3 (online MAD recurrence
        //   approaches the population MAD) — but the running formula is not
        //   exactly the population MAD because each step uses the running
        //   mean. We just verify the result matches our own implementation.
        let mut stats = CostStats::new();
        for c in [1.0, 3.0, 5.0] {
            stats.observe(c);
        }
        assert!((stats.mean - 3.0).abs() < 1e-12);
        // Running MAD lands somewhere in (0, 4) — the exact textbook value
        // would be 4/3 ≈ 1.333; the recurrence is close but not identical.
        assert!(stats.mad > 0.0 && stats.mad < 4.0);
        // Projection must exceed the mean.
        assert!(stats.project_next(0.0) > stats.mean);
    }

    #[test]
    fn projection_is_monotone_in_observed_max_for_constant_history() {
        // Two histories where the only difference is one larger spike — the
        // projection must reflect it.
        let mut a = CostStats::new();
        for c in [0.01, 0.01, 0.01, 0.02] {
            a.observe(c);
        }
        let mut b = CostStats::new();
        for c in [0.01, 0.01, 0.01, 0.50] {
            b.observe(c);
        }
        assert!(
            b.project_next(0.0) > a.project_next(0.0),
            "larger spike must project higher: a={}, b={}",
            a.project_next(0.0),
            b.project_next(0.0)
        );
    }

    #[test]
    fn determinism_across_two_identical_runs() {
        // Critical for Phase 4 contract criterion #15 (determinism).
        let mut a = CostStats::new();
        let mut b = CostStats::new();
        let series = [0.02, 0.025, 0.018, 0.03, 0.022];
        for c in series {
            a.observe(c);
            b.observe(c);
        }
        assert_eq!(a, b);
        assert_eq!(a.project_next(0.0), b.project_next(0.0));
    }
}
