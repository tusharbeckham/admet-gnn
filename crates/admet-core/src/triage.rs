//! Ranking and triage: collapsing twelve predictions into one number.
//!
//! Manual chapter 14. The desirability curves and [`top_k`] are **fully
//! implemented**; only [`triage_score`] waits on the concrete prediction type
//! from `admet-infer`.
//!
//! # The product is the ordering
//!
//! A chemist submits a library and wants the most promising compounds first. That
//! requires collapsing twelve endpoint predictions into one comparable number —
//! which means deciding, in code, what "promising" means. That decision is domain
//! knowledge, not arithmetic, and it is the part of this module worth defending in
//! a viva.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Per-category weights for the composite score.
///
/// Toxicity outweighs everything by default: a cardiac liability is
/// disqualifying, whereas mediocre absorption is a formulation problem. These are
/// **product decisions, not tuned parameters** — do not fit them to data, and do
/// state them in the report so a reader can disagree with the weighting rather
/// than guess at it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weights {
    /// Caco-2, HIA, P-gp, bioavailability.
    pub absorption: f32,
    /// BBB, plasma protein binding, volume of distribution.
    pub distribution: f32,
    /// CYP3A4, CYP2D6 inhibition.
    pub metabolism: f32,
    /// Half-life, hepatocyte clearance.
    pub excretion: f32,
    /// hERG.
    pub toxicity: f32,
}

impl Default for Weights {
    fn default() -> Self {
        Self {
            absorption: 1.0,
            distribution: 0.8,
            metabolism: 1.0,
            excretion: 0.8,
            toxicity: 2.0,
        }
    }
}

/// Desirability for "higher is better", ramping linearly between two points.
///
/// Below `low` scores 0, above `high` scores 1, linear in between. A ramp rather
/// than a step because a hard threshold makes the ranking discontinuous: two
/// compounds either side of an arbitrary cut-off would be ordered as though they
/// were qualitatively different when they differ by 0.01.
///
/// ```
/// use admet_core::triage::desirability_higher_better;
/// assert_eq!(desirability_higher_better(-6.0, -5.5, -4.5), 0.0);
/// assert_eq!(desirability_higher_better(-4.0, -5.5, -4.5), 1.0);
/// assert_eq!(desirability_higher_better(-5.0, -5.5, -4.5), 0.5);
/// ```
pub fn desirability_higher_better(value: f32, low: f32, high: f32) -> f32 {
    if !value.is_finite() || high <= low {
        return 0.0;
    }
    ((value - low) / (high - low)).clamp(0.0, 1.0)
}

/// Desirability for "lower is better".
pub fn desirability_lower_better(value: f32, low: f32, high: f32) -> f32 {
    1.0 - desirability_higher_better(value, low, high)
}

/// Desirability of a probability, given which outcome is wanted.
///
/// `want_positive == true` means a high probability is good (intestinal
/// absorption); `false` means it is bad (hERG blockade, P-gp substrate).
pub fn desirability_binary(probability: f32, want_positive: bool) -> f32 {
    if !probability.is_finite() {
        return 0.0;
    }
    let p = probability.clamp(0.0, 1.0);
    if want_positive {
        p
    } else {
        1.0 - p
    }
}

/// Desirability of landing inside a window, peaking at its centre.
///
/// The right shape for half-life: too short means dosing four times a day, too
/// long means accumulation toxicity. Neither "higher" nor "lower" is better, and
/// modelling it as either would rank a 200-hour half-life as excellent.
///
/// ```
/// use admet_core::triage::desirability_window;
/// assert_eq!(desirability_window(14.0, 4.0, 24.0), 1.0);  // centre
/// assert_eq!(desirability_window(2.0, 4.0, 24.0), 0.0);   // too short
/// assert_eq!(desirability_window(48.0, 4.0, 24.0), 0.0);  // too long
/// ```
pub fn desirability_window(value: f32, low: f32, high: f32) -> f32 {
    if !value.is_finite() || high <= low || value < low || value > high {
        return 0.0;
    }
    let centre = (low + high) / 2.0;
    let half_width = (high - low) / 2.0;
    (1.0 - (value - centre).abs() / half_width).clamp(0.0, 1.0)
}

/// Weighted **geometric** mean of per-endpoint desirabilities.
///
/// # Why geometric and not arithmetic
///
/// This is the single most important line of domain reasoning in the module.
///
/// An arithmetic mean lets one excellent score compensate for a fatal one. In
/// drug discovery that is exactly wrong: **cardiac toxicity is disqualifying no
/// matter how well a compound is absorbed.** The geometric mean drives the product
/// toward zero as any single factor approaches zero, so a likely hERG blocker
/// cannot be rescued by eleven good values.
///
/// Computed in log space to avoid underflow across twelve factors, with terms
/// floored at `1e-6` so a single zero yields a very small score rather than
/// `-inf`.
///
/// Returns `None` for an empty or zero-weighted term list — no score is honest
/// there, and `0.0` would be indistinguishable from "scored, and terrible".
pub fn weighted_geometric_mean(terms: &[(f32, f32)]) -> Option<f32> {
    let weight_sum: f32 = terms.iter().map(|(_, w)| *w).sum();
    if terms.is_empty() || weight_sum <= 0.0 {
        return None;
    }
    //  A zero desirability is ABSORBING, checked before the logs. This is not a
    //  special case bolted on -- it is what the geometric mean of a set
    //  containing zero actually is, and it is the whole reason this function is
    //  a geometric mean rather than an arithmetic one.
    //
    //  Relying on the `max(1e-6)` floor below to approximate it does not work.
    //  With eleven perfect endpoints and one fatal one at weight 2, the floor
    //  yields exp(2 * ln(1e-6) / 13) = 0.119 -- which ranks a likely hERG
    //  blocker as a middling candidate instead of rejecting it. The floor's job
    //  is only to keep `ln` finite for small-but-survivable values; it must not
    //  be load-bearing for disqualification.
    if terms.iter().any(|&(d, w)| w > 0.0 && d <= 0.0) {
        return Some(0.0);
    }
    let log_sum: f32 = terms.iter().map(|(d, w)| w * d.max(1e-6).ln()).sum();
    Some((log_sum / weight_sum).exp())
}

/// A scored candidate, ready for ranking.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    /// The 27-character InChIKey. Also the tie-breaker — see [`Scored::cmp`].
    pub inchikey: String,
    /// Composite triage score in `[0, 1]`.
    pub score: f32,
    /// Row index in the submitted batch, so results map back to the user's file.
    pub row_index: usize,
}

impl Eq for Scored {}

impl Ord for Scored {
    /// Order by score, then by InChIKey, with NaN sinking to the bottom.
    ///
    /// # Why the tie-break is not optional
    ///
    /// Floating-point scores tie more often than you expect, especially after
    /// rounding for display. Without a deterministic tie-break, heap order leaks
    /// into the output: re-running the same job reorders the top 100, a user
    /// reports it as a bug, and you lose an afternoon. Breaking on InChIKey costs
    /// one string comparison on the rare equal-score path.
    ///
    /// # Why NaN is handled explicitly
    ///
    /// A NaN score means an endpoint failed to predict. The obvious
    /// `partial_cmp().unwrap()` would panic and take down a 50,000-row batch job
    /// because one molecule misbehaved. Sorting NaN below everything means a
    /// broken prediction sinks out of the top-k instead — visible in the output,
    /// harmless to the run.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.score.is_nan(), other.score.is_nan()) {
            // Two failures still need a stable relative order.
            (true, true) => self.inchikey.cmp(&other.inchikey),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            // Neither is NaN, so partial_cmp always yields Some. Equal scores
            // fall through to the deterministic tie-break.
            (false, false) => self
                .score
                .partial_cmp(&other.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| self.inchikey.cmp(&other.inchikey)),
        }
    }
}

impl PartialOrd for Scored {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// The best `k` items from a stream, using a bounded min-heap.
///
/// # Why not just sort
///
/// | Algorithm | Complexity | n=50k, k=100 | Trade-off |
/// |---|---|---|---|
/// | Full sort | `O(n log n)` | ~780k ops | Simple; wastes 99.8% of the work |
/// | **Bounded min-heap** | `O(n log k)` | ~332k ops | Streaming-friendly, `O(k)` memory |
/// | Quickselect | `O(n)` avg | ~50k ops | Fastest, but needs all data in memory and reorders it |
///
/// The min-heap is chosen for the middle column as much as the second: it works
/// on a stream, so a batch job never needs the full result set resident. At
/// 50,000 rows that is the difference between 100 items in memory and 50,000.
///
/// The heap root is the **weakest** kept item, so deciding whether a candidate
/// belongs is one `O(1)` comparison. Only the rare improvement costs `O(log k)`,
/// and on realistic data most candidates are rejected immediately.
///
/// Output is sorted best-first.
///
/// ```
/// use admet_core::triage::{top_k, Scored};
/// let items: Vec<Scored> = (0..100)
///     .map(|i| Scored { inchikey: format!("KEY{i:03}"), score: i as f32 / 100.0, row_index: i })
///     .collect();
/// let best = top_k(items.into_iter(), 3);
/// assert_eq!(best.len(), 3);
/// assert_eq!(best[0].score, 0.99);
/// assert!(best[0].score >= best[1].score);
/// ```
pub fn top_k(stream: impl Iterator<Item = Scored>, k: usize) -> Vec<Scored> {
    if k == 0 {
        return Vec::new();
    }
    // Reverse turns the max-heap into a min-heap, so the root is the weakest
    // item kept -- which is precisely the one a new candidate must beat.
    let mut heap: BinaryHeap<std::cmp::Reverse<Scored>> = BinaryHeap::with_capacity(k + 1);
    for item in stream {
        if heap.len() < k {
            heap.push(std::cmp::Reverse(item));
        } else if let Some(std::cmp::Reverse(weakest)) = heap.peek() {
            if item > *weakest {
                heap.pop();
                heap.push(std::cmp::Reverse(item));
            }
        }
    }
    let mut out: Vec<Scored> = heap.into_iter().map(|std::cmp::Reverse(s)| s).collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out
}

/// Composite triage score for one molecule's twelve predictions.
///
/// Waits on the concrete prediction struct, which lives in `admet-infer`
/// (Increment 2). The building blocks it will compose —
/// [`desirability_higher_better`], [`desirability_binary`],
/// [`desirability_window`] and [`weighted_geometric_mean`] — are all implemented
/// and tested, so this is assembly rather than design.
///
/// The mapping it must apply, from manual Listing 14.1:
///
/// | Endpoint | Curve | Rationale |
/// |---|---|---|
/// | `caco2` | higher better, −5.5 → −4.5 | log cm/s permeability |
/// | `hia` | binary, want positive | absorption is required |
/// | `pgp` | binary, want negative | efflux substrate is bad |
/// | `herg` | binary, want negative | **disqualifying** |
/// | `half_life` | window, 4 → 24 h | once-daily dosing |
///
/// # Errors
/// [`crate::CoreError::NotImplemented`] until Increment 4.
pub fn triage_score(predictions: &[f32], weights: &Weights) -> crate::Result<f32> {
    let _ = (predictions, weights);
    Err(crate::CoreError::NotImplemented("triage::triage_score"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scored(key: &str, score: f32) -> Scored {
        Scored {
            inchikey: key.to_string(),
            score,
            row_index: 0,
        }
    }

    #[test]
    fn ramps_clamp_at_both_ends() {
        assert_eq!(desirability_higher_better(-6.0, -5.5, -4.5), 0.0);
        assert_eq!(desirability_higher_better(-5.5, -5.5, -4.5), 0.0);
        assert_eq!(desirability_higher_better(-5.0, -5.5, -4.5), 0.5);
        assert_eq!(desirability_higher_better(-4.5, -5.5, -4.5), 1.0);
        assert_eq!(desirability_higher_better(0.0, -5.5, -4.5), 1.0);
    }

    #[test]
    fn lower_better_is_the_exact_complement() {
        for v in [-6.0f32, -5.5, -5.0, -4.5, 0.0] {
            let hi = desirability_higher_better(v, -5.5, -4.5);
            let lo = desirability_lower_better(v, -5.5, -4.5);
            assert!((hi + lo - 1.0).abs() < 1e-6, "at {v}: {hi} + {lo} != 1");
        }
    }

    #[test]
    fn binary_desirability_respects_the_wanted_outcome() {
        assert_eq!(desirability_binary(0.9, true), 0.9);
        assert!((desirability_binary(0.9, false) - 0.1).abs() < 1e-6);
        // Out-of-range probabilities clamp rather than producing scores >1.
        assert_eq!(desirability_binary(1.5, true), 1.0);
        assert_eq!(desirability_binary(-0.2, true), 0.0);
    }

    #[test]
    fn window_peaks_at_the_centre_and_zeroes_outside() {
        assert_eq!(desirability_window(14.0, 4.0, 24.0), 1.0);
        assert_eq!(desirability_window(4.0, 4.0, 24.0), 0.0);
        assert_eq!(desirability_window(24.0, 4.0, 24.0), 0.0);
        assert_eq!(desirability_window(2.0, 4.0, 24.0), 0.0);
        assert_eq!(desirability_window(100.0, 4.0, 24.0), 0.0);
        assert!((desirability_window(9.0, 4.0, 24.0) - 0.5).abs() < 1e-6);
    }

    /// NaN reaches these functions when an endpoint fails to predict. Returning
    /// 0.0 keeps the score finite; propagating NaN poisons the whole ranking.
    #[test]
    fn non_finite_inputs_score_zero_rather_than_propagating() {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(desirability_higher_better(bad, 0.0, 1.0), 0.0);
            assert_eq!(desirability_binary(bad, true), 0.0);
            assert_eq!(desirability_window(bad, 0.0, 1.0), 0.0);
        }
    }

    #[test]
    fn degenerate_ranges_score_zero_instead_of_dividing_by_zero() {
        assert_eq!(desirability_higher_better(5.0, 3.0, 3.0), 0.0);
        assert_eq!(desirability_window(5.0, 3.0, 3.0), 0.0);
    }

    /// The load-bearing property of the whole module: one fatal value sinks the
    /// compound. If this test ever fails, the score has silently become an
    /// arithmetic mean and the tool is recommending cardiotoxic compounds.
    #[test]
    fn one_disqualifying_value_sinks_the_geometric_mean() {
        let excellent = [(1.0f32, 1.0f32); 11];
        let mut with_one_fatal = excellent.to_vec();
        with_one_fatal.push((0.0, 2.0)); // a likely hERG blocker

        let all_good = weighted_geometric_mean(&excellent).unwrap();
        let one_fatal = weighted_geometric_mean(&with_one_fatal).unwrap();

        assert!((all_good - 1.0).abs() < 1e-5);
        assert!(
            one_fatal < 0.02,
            "geometric mean must collapse, got {one_fatal}"
        );

        // The contrast that justifies the choice: an arithmetic mean would have
        // rated this compound as highly promising.
        let arithmetic: f32 =
            with_one_fatal.iter().map(|(d, _)| d).sum::<f32>() / with_one_fatal.len() as f32;
        assert!(
            arithmetic > 0.9,
            "arithmetic mean would have said {arithmetic}"
        );
    }

    #[test]
    fn geometric_mean_of_equal_terms_is_that_term() {
        let terms = [(0.5f32, 1.0f32); 6];
        assert!((weighted_geometric_mean(&terms).unwrap() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn geometric_mean_declines_no_score_when_it_has_none() {
        assert_eq!(weighted_geometric_mean(&[]), None);
        assert_eq!(weighted_geometric_mean(&[(0.9, 0.0), (0.8, 0.0)]), None);
    }

    #[test]
    fn weights_shift_the_result_toward_the_heavier_term() {
        let unweighted = weighted_geometric_mean(&[(0.9, 1.0), (0.1, 1.0)]).unwrap();
        let toxicity_heavy = weighted_geometric_mean(&[(0.9, 1.0), (0.1, 5.0)]).unwrap();
        assert!(
            toxicity_heavy < unweighted,
            "weighting the bad term must lower the score: {toxicity_heavy} vs {unweighted}"
        );
    }

    #[test]
    fn top_k_returns_the_k_best_sorted_descending() {
        let items: Vec<Scored> = (0..1000)
            .map(|i| scored(&format!("KEY{i:04}"), i as f32 / 1000.0))
            .collect();
        let best = top_k(items.into_iter(), 10);

        assert_eq!(best.len(), 10);
        assert!((best[0].score - 0.999).abs() < 1e-6);
        assert!((best[9].score - 0.990).abs() < 1e-6);
        for w in best.windows(2) {
            assert!(w[0].score >= w[1].score, "output must be sorted best-first");
        }
    }

    #[test]
    fn top_k_handles_k_larger_than_the_stream() {
        let best = top_k(vec![scored("A", 0.5), scored("B", 0.9)].into_iter(), 100);
        assert_eq!(best.len(), 2);
        assert_eq!(best[0].inchikey, "B");
    }

    #[test]
    fn top_k_of_zero_or_empty_is_empty() {
        assert!(top_k(vec![scored("A", 0.5)].into_iter(), 0).is_empty());
        assert!(top_k(std::iter::empty(), 10).is_empty());
    }

    /// Re-running the same job must produce the same order. Without the
    /// InChIKey tie-break, heap order leaks into the output and identical
    /// scores shuffle between runs.
    #[test]
    fn equal_scores_break_deterministically_on_inchikey() {
        let make = || {
            vec![
                scored("CCCCCCCCCCCCCC-UHFFFAOYSA-N", 0.5),
                scored("AAAAAAAAAAAAAA-UHFFFAOYSA-N", 0.5),
                scored("BBBBBBBBBBBBBB-UHFFFAOYSA-N", 0.5),
            ]
        };
        let first = top_k(make().into_iter(), 3);
        for _ in 0..20 {
            let again = top_k(make().into_iter(), 3);
            assert_eq!(
                first.iter().map(|s| &s.inchikey).collect::<Vec<_>>(),
                again.iter().map(|s| &s.inchikey).collect::<Vec<_>>(),
                "identical input must produce identical order"
            );
        }
        // Descending by key, since scores are equal.
        assert!(first[0].inchikey.starts_with('C'));
        assert!(first[2].inchikey.starts_with('A'));
    }

    /// A NaN score means an endpoint failed. It must sink, not panic -- a
    /// `partial_cmp().unwrap()` in the comparator would take down a 50,000-row
    /// batch job because one molecule misbehaved.
    #[test]
    fn nan_scores_sink_without_panicking() {
        let items = vec![
            scored("GOOD", 0.9),
            scored("NAN0", f32::NAN),
            scored("MEH0", 0.4),
        ];
        let best = top_k(items.into_iter(), 3);
        assert_eq!(best.len(), 3);
        assert_eq!(best[0].inchikey, "GOOD");
        assert!(best[2].score.is_nan(), "NaN must end up last");
    }

    #[test]
    fn default_weights_prioritise_toxicity() {
        let w = Weights::default();
        assert!(w.toxicity > w.absorption);
        assert!(w.toxicity > w.distribution);
        assert!(w.toxicity > w.metabolism);
        assert!(w.toxicity > w.excretion);
    }
}
