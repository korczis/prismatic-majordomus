//! The arithmetic of the economics subsystem, and nothing else: the reduction formula,
//! order statistics, a seeded bootstrap (over independent values, or over clusters when
//! the values are repetitions of the same task), and the two ways a number is printed.
//! Every surface that shows a number gets it from here through [`super::summarize`];
//! nothing downstream recomputes one, and nothing downstream formats a percentage its own
//! way.
//!
//! Two rules are enforced by the types rather than by care:
//!
//! - A reduction against a zero control is `None`, never zero and never infinity: there is
//!   nothing to reduce, so there is no ratio.
//! - A negative reduction is kept. Treatment costing more than control is a result.
//!
//! ```
//! use majordomus_cli::economics::stats::{reduction, distribution, percent_text};
//! assert_eq!(reduction(100, 50), Some(0.5));
//! assert_eq!(reduction(100, 120), Some(-0.2));
//! assert_eq!(reduction(0, 10), None);
//! let d = distribution(&[0.1, 0.3, 0.2]).unwrap();
//! assert_eq!(d.median, 0.2);
//! assert_eq!(percent_text(reduction(100, 120).unwrap()), "-20.0%");
//! ```

use super::model::{EconomicsDistribution, EconomicsInterval};

/// `1 - treatment / control`: the share of the control's quantity the treatment did not
/// spend. Negative when the treatment spent more; `None` when the control spent nothing,
/// because a ratio against zero is not a number anyone should be shown.
///
/// ```
/// use majordomus_cli::economics::stats::reduction;
/// assert_eq!(reduction(100, 100), Some(0.0));
/// assert_eq!(reduction(100, 0), Some(1.0));
/// assert_eq!(reduction(100, 120), Some(-0.2));
/// assert_eq!(reduction(0, 0), None);
/// ```
pub fn reduction(control: u64, treatment: u64) -> Option<f64> {
    if control == 0 {
        return None;
    }
    Some(round(1.0 - treatment as f64 / control as f64))
}

/// The `q` quantile of sorted values, linear between the two nearest ranks (Hyndman and
/// Fan's type 7, the default of R and NumPy). `None` for no values.
///
/// ```
/// use majordomus_cli::economics::stats::quantile;
/// assert_eq!(quantile(&[1.0, 2.0, 3.0, 4.0], 0.5), Some(2.5));
/// assert_eq!(quantile(&[7.0], 0.95), Some(7.0));
/// assert_eq!(quantile(&[], 0.5), None);
/// ```
pub fn quantile(sorted: &[f64], q: f64) -> Option<f64> {
    match sorted.len() {
        0 => None,
        1 => Some(sorted[0]),
        n => {
            let h = (n - 1) as f64 * q.clamp(0.0, 1.0);
            let lo = h.floor() as usize;
            let hi = h.ceil() as usize;
            Some(sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo]))
        }
    }
}

fn sorted(values: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v
}

/// The distribution of a set of values: order statistics, mean and sample standard
/// deviation. Independent of the order the values arrive in. `None` for no values.
///
/// ```
/// use majordomus_cli::economics::stats::distribution;
/// let d = distribution(&[0.32, 0.51, 0.41]).unwrap();
/// assert_eq!((d.n, d.median, d.min, d.max), (3, 0.41, 0.32, 0.51));
/// assert!(distribution(&[0.5]).unwrap().sd.is_none(), "one value has no spread");
/// assert!(distribution(&[]).is_none());
/// ```
pub fn distribution(values: &[f64]) -> Option<EconomicsDistribution> {
    let v = sorted(values);
    let n = v.len();
    if n == 0 {
        return None;
    }
    let mean = v.iter().sum::<f64>() / n as f64;
    let sd = (n > 1)
        .then(|| (v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt());
    Some(EconomicsDistribution {
        n,
        mean: round(mean),
        median: round(quantile(&v, 0.5)?),
        p25: round(quantile(&v, 0.25)?),
        p75: round(quantile(&v, 0.75)?),
        p95: round(quantile(&v, 0.95)?),
        min: round(v[0]),
        max: round(v[n - 1]),
        sd: sd.map(round),
    })
}

/// Twelve decimal places: enough for any ratio this subsystem computes, few enough that the
/// same arithmetic on two machines prints the same JSON.
pub(crate) fn round(x: f64) -> f64 {
    (x * 1e12).round() / 1e12
}

/// SplitMix64: a small, fully specified generator, so that the interval a seed produces is
/// the same on every machine and in every version of every dependency.
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` by rejection, so that no index is favoured.
    fn below(&mut self, n: u64) -> u64 {
        let zone = u64::MAX - u64::MAX % n;
        loop {
            let x = self.next();
            if x < zone {
                return x % n;
            }
        }
    }
}

/// The percentile interval of a set of bootstrap medians at `level_bp`.
fn percentile_interval(
    mut medians: Vec<f64>,
    level_bp: u32,
    resamples: u32,
    seed: u64,
    method: &str,
) -> Option<EconomicsInterval> {
    medians.sort_by(|a, b| a.total_cmp(b));
    let tail = (1.0 - level_bp as f64 / 10_000.0) / 2.0;
    Some(EconomicsInterval {
        level_bp,
        low: round(quantile(&medians, tail)?),
        high: round(quantile(&medians, 1.0 - tail)?),
        method: method.into(),
        resamples,
        seed,
    })
}

/// What [`bootstrap_median`] writes into [`EconomicsInterval::method`].
pub const INDEPENDENT_METHOD: &str =
    "percentile bootstrap of the median, each value resampled independently with replacement";

/// What [`bootstrap_median_clustered`] writes into [`EconomicsInterval::method`].
pub const CLUSTER_METHOD: &str = "percentile bootstrap of the median, cluster bootstrap over tasks: each resample draws tasks with replacement and pools every value of each drawn task";

/// A percentile bootstrap interval of the median: resample the values with replacement
/// `resamples` times, take each resample's median, and report the quantiles of those
/// medians at the requested confidence. The values are sorted first, so the interval
/// depends on the set of values and the seed, never on the order they were listed in.
/// `None` below `min_n` values: an interval over two pairs would be precision nobody has.
///
/// Resampling each value on its own assumes the values are independent. That holds for
/// the context suite's seeds, each a separate compilation; it does not hold for the
/// repetitions of one task, which is what [`bootstrap_median_clustered`] is for.
///
/// ```
/// use majordomus_cli::economics::stats::{bootstrap_median, INDEPENDENT_METHOD};
/// let v = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6];
/// let a = bootstrap_median(&v, 9500, 2000, 7, 5).unwrap();
/// let b = bootstrap_median(&[0.6, 0.5, 0.4, 0.3, 0.2, 0.1], 9500, 2000, 7, 5).unwrap();
/// assert_eq!(a, b, "order does not matter");
/// assert!(a.low <= 0.35 && 0.35 <= a.high);
/// assert_eq!(a.method, INDEPENDENT_METHOD);
/// assert!(bootstrap_median(&v[..3], 9500, 2000, 7, 5).is_none());
/// ```
pub fn bootstrap_median(
    values: &[f64],
    level_bp: u32,
    resamples: u32,
    seed: u64,
    min_n: usize,
) -> Option<EconomicsInterval> {
    let v = sorted(values);
    let n = v.len();
    if n < min_n.max(2) || resamples == 0 {
        return None;
    }
    let mut rng = SplitMix64(seed);
    let mut medians = Vec::with_capacity(resamples as usize);
    let mut sample = vec![0.0; n];
    for _ in 0..resamples {
        for slot in sample.iter_mut() {
            *slot = v[rng.below(n as u64) as usize];
        }
        sample.sort_by(|a, b| a.total_cmp(b));
        medians.push(quantile(&sample, 0.5)?);
    }
    percentile_interval(medians, level_bp, resamples, seed, INDEPENDENT_METHOD)
}

/// A percentile bootstrap interval of the median over clustered values: each resample
/// draws as many clusters as there are, with replacement, pools every value of each drawn
/// cluster, and takes the median of the pool.
///
/// The repetitions of one task share its fixture, its prompt and its difficulty, so they
/// are not independent observations: resampling them one by one would treat four runs of
/// one easy task as four tasks and report an interval narrower than the evidence allows.
/// Resampling whole clusters (tasks) keeps what varies between tasks in the interval.
///
/// Deterministic: non-finite values are dropped, each cluster is sorted, empty clusters
/// are dropped, and the clusters are sorted by their sorted values before any draw, so the
/// interval depends on the clusters and the seed, never on the order they were listed in.
/// `None` below `min_n` values in total, with fewer than two clusters (a single task has
/// no between-task variation to resample), or with no resamples.
///
/// ```
/// use majordomus_cli::economics::stats::{bootstrap_median_clustered, CLUSTER_METHOD};
/// let tasks = vec![vec![0.1, 0.12], vec![0.3, 0.31, 0.29], vec![0.5, 0.52]];
/// let a = bootstrap_median_clustered(&tasks, 9500, 2000, 7, 5).unwrap();
/// let shuffled = vec![vec![0.52, 0.5], vec![0.1, 0.12], vec![0.29, 0.31, 0.3]];
/// assert_eq!(a, bootstrap_median_clustered(&shuffled, 9500, 2000, 7, 5).unwrap());
/// assert!(a.low <= 0.3 && 0.3 <= a.high);
/// assert_eq!(a.method, CLUSTER_METHOD);
/// let one_task = vec![vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6]];
/// assert!(bootstrap_median_clustered(&one_task, 9500, 2000, 7, 5).is_none());
/// ```
pub fn bootstrap_median_clustered(
    clusters: &[Vec<f64>],
    level_bp: u32,
    resamples: u32,
    seed: u64,
    min_n: usize,
) -> Option<EconomicsInterval> {
    let mut c: Vec<Vec<f64>> = clusters
        .iter()
        .map(|v| sorted(v.as_slice()))
        .filter(|v| !v.is_empty())
        .collect();
    c.sort_by(|a, b| {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| x.total_cmp(y))
            .find(|o| o.is_ne())
            .unwrap_or_else(|| a.len().cmp(&b.len()))
    });
    let n: usize = c.iter().map(Vec::len).sum();
    let k = c.len();
    if n < min_n.max(2) || k < 2 || resamples == 0 {
        return None;
    }
    let mut rng = SplitMix64(seed);
    let mut medians = Vec::with_capacity(resamples as usize);
    let mut pool = Vec::with_capacity(n * 2);
    for _ in 0..resamples {
        pool.clear();
        for _ in 0..k {
            pool.extend_from_slice(&c[rng.below(k as u64) as usize]);
        }
        pool.sort_by(|a, b| a.total_cmp(b));
        medians.push(quantile(&pool, 0.5)?);
    }
    percentile_interval(medians, level_bp, resamples, seed, CLUSTER_METHOD)
}

/// A confidence level in basis points as a person reads it: `9500` is `95%`, `9750` is
/// `97.5%`. Whole percentages carry no decimals and a fraction carries only the digits it
/// needs, so a level is never rounded into one the methodology did not declare.
///
/// ```
/// use majordomus_cli::economics::stats::level_text;
/// assert_eq!(level_text(9500), "95%");
/// assert_eq!(level_text(9750), "97.5%");
/// assert_eq!(level_text(9999), "99.99%");
/// assert_eq!(level_text(9000), "90%");
/// ```
pub fn level_text(level_bp: u32) -> String {
    let (whole, rest) = (level_bp / 100, level_bp % 100);
    if rest == 0 {
        format!("{whole}%")
    } else {
        let digits = format!("{rest:02}");
        format!("{whole}.{}%", digits.trim_end_matches('0'))
    }
}

/// A ratio as a signed percentage with one decimal: `0.2` is `+20.0%`, `-0.05` is `-5.0%`.
/// Anything that rounds to zero at one decimal prints `0.0%` with no sign, so a surface
/// never shows `-0.0%` for a result that is, to the precision shown, no change. A value
/// that is not a number prints `n/a` rather than a figure.
///
/// ```
/// use majordomus_cli::economics::stats::percent_text;
/// assert_eq!(percent_text(0.2), "+20.0%");
/// assert_eq!(percent_text(-0.05), "-5.0%");
/// assert_eq!(percent_text(-0.0001), "0.0%", "never -0.0%");
/// assert_eq!(percent_text(0.0), "0.0%");
/// assert_eq!(percent_text(f64::NAN), "n/a");
/// ```
pub fn percent_text(ratio: f64) -> String {
    if !ratio.is_finite() {
        return "n/a".into();
    }
    let x = ratio * 100.0;
    let magnitude = format!("{:.1}", x.abs());
    if magnitude.bytes().all(|b| b == b'0' || b == b'.') {
        "0.0%".into()
    } else if x < 0.0 {
        format!("-{magnitude}%")
    } else {
        format!("+{magnitude}%")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_formula_is_the_one_the_methodology_states() {
        assert_eq!(reduction(100, 50), Some(0.5));
        assert_eq!(reduction(100, 100), Some(0.0));
        assert_eq!(reduction(100, 120), Some(-0.2));
        assert_eq!(reduction(100_000, 55_000), Some(0.45));
    }

    #[test]
    fn a_zero_control_has_no_reduction_rather_than_an_infinite_one() {
        assert_eq!(reduction(0, 0), None);
        assert_eq!(reduction(0, 1), None);
    }

    #[test]
    fn a_regression_is_never_clamped() {
        let r = reduction(1_000, 3_000).unwrap();
        assert!(
            (r + 2.0).abs() < 1e-12,
            "a treatment three times the control is -200 %, got {r}"
        );
    }

    #[test]
    fn quantiles_interpolate_linearly() {
        let v = [10.0, 20.0, 30.0, 40.0, 50.0];
        assert_eq!(quantile(&v, 0.0), Some(10.0));
        assert_eq!(quantile(&v, 0.25), Some(20.0));
        assert_eq!(quantile(&v, 0.5), Some(30.0));
        assert!((quantile(&v, 0.95).unwrap() - 48.0).abs() < 1e-9);
        assert_eq!(quantile(&v, 1.0), Some(50.0));
    }

    #[test]
    fn a_distribution_drops_nothing_finite_and_nothing_else() {
        let d = distribution(&[f64::NAN, 1.0, f64::INFINITY, 3.0]).unwrap();
        assert_eq!(d.n, 2, "non-finite values are not measurements");
        assert_eq!(d.mean, 2.0);
    }

    #[test]
    fn the_bootstrap_is_reproducible_from_its_seed_and_moves_with_it() {
        let v: Vec<f64> = (0..40).map(|i| (i as f64 * 0.37).sin()).collect();
        let a = bootstrap_median(&v, 9500, 1000, 1, 5).unwrap();
        let b = bootstrap_median(&v, 9500, 1000, 1, 5).unwrap();
        let c = bootstrap_median(&v, 9500, 1000, 2, 5).unwrap();
        assert_eq!(a, b);
        assert!(a != c, "a different seed is a different resampling");
        assert!(a.low <= a.high);
    }

    #[test]
    fn the_bootstrap_of_identical_values_is_a_point() {
        let i = bootstrap_median(&[0.25; 8], 9500, 500, 3, 5).unwrap();
        assert_eq!((i.low, i.high), (0.25, 0.25));
    }

    #[test]
    fn the_cluster_bootstrap_needs_two_clusters_and_enough_values() {
        let many = vec![vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7]];
        assert!(
            bootstrap_median_clustered(&many, 9500, 500, 3, 5).is_none(),
            "one task"
        );
        let few = vec![vec![0.1, 0.2], vec![0.3]];
        assert!(
            bootstrap_median_clustered(&few, 9500, 500, 3, 5).is_none(),
            "three values"
        );
        let empty_dropped = vec![vec![], vec![0.1, 0.2, 0.3], vec![f64::NAN]];
        assert!(
            bootstrap_median_clustered(&empty_dropped, 9500, 500, 3, 2).is_none(),
            "an empty or non-finite cluster is not a second task"
        );
        let ok = vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5]];
        assert!(
            bootstrap_median_clustered(&ok, 9500, 0, 3, 5).is_none(),
            "no resamples"
        );
        let i = bootstrap_median_clustered(&ok, 9500, 500, 3, 5).unwrap();
        assert!(i.low <= i.high);
        assert_eq!(i.method, CLUSTER_METHOD);
    }

    #[test]
    fn the_cluster_bootstrap_is_reproducible_from_its_seed() {
        let c: Vec<Vec<f64>> = (0..8)
            .map(|t| {
                (0..4)
                    .map(|r| (t as f64 * 0.37 + r as f64 * 0.01).sin())
                    .collect()
            })
            .collect();
        let a = bootstrap_median_clustered(&c, 9500, 1000, 1, 5).unwrap();
        assert_eq!(a, bootstrap_median_clustered(&c, 9500, 1000, 1, 5).unwrap());
        assert!(a != bootstrap_median_clustered(&c, 9500, 1000, 2, 5).unwrap());
    }

    #[test]
    fn tasks_that_differ_widen_the_interval_past_the_pooled_one() {
        // six tasks, each repeated five times with nearly identical results: the pooled
        // bootstrap sees thirty independent values, the cluster bootstrap sees six tasks
        let clusters: Vec<Vec<f64>> = (0..6)
            .map(|t| (0..5).map(|r| t as f64 * 0.1 + r as f64 * 0.001).collect())
            .collect();
        let pooled: Vec<f64> = clusters.iter().flatten().copied().collect();
        let p = bootstrap_median(&pooled, 9500, 4000, 5, 5).unwrap();
        let c = bootstrap_median_clustered(&clusters, 9500, 4000, 5, 5).unwrap();
        assert!(
            c.high - c.low >= p.high - p.low,
            "cluster {c:?} against pooled {p:?}"
        );
    }

    #[test]
    fn a_level_is_printed_with_the_decimals_it_has() {
        assert_eq!(level_text(9500), "95%");
        assert_eq!(level_text(9750), "97.5%");
        assert_eq!(level_text(9905), "99.05%");
        assert_eq!(level_text(10_000), "100%");
        assert_eq!(level_text(50), "0.5%");
    }

    #[test]
    fn a_percentage_that_rounds_to_zero_has_no_sign() {
        assert_eq!(percent_text(-0.0), "0.0%");
        assert_eq!(percent_text(-0.00049), "0.0%");
        assert_eq!(percent_text(0.00049), "0.0%");
        assert_eq!(percent_text(-0.0006), "-0.1%");
        assert_eq!(percent_text(1.5), "+150.0%");
        assert_eq!(percent_text(f64::INFINITY), "n/a");
    }

    mod invariants {
        use super::super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn identical_sides_reduce_by_nothing(c in 1u64..10_000_000) {
                prop_assert_eq!(reduction(c, c), Some(0.0));
            }

            #[test]
            fn the_sign_follows_which_side_spent_more(c in 1u64..10_000_000, t in 0u64..10_000_000) {
                let r = reduction(c, t).unwrap();
                if t < c { prop_assert!(r > 0.0); }
                if t > c { prop_assert!(r < 0.0); }
                if t == c { prop_assert_eq!(r, 0.0); }
            }

            #[test]
            fn order_never_changes_an_aggregate(mut v in prop::collection::vec(-3.0f64..1.0, 5..40), seed in 0u64..1000) {
                let a = (distribution(&v), bootstrap_median(&v, 9500, 200, seed, 5));
                v.reverse();
                let b = (distribution(&v), bootstrap_median(&v, 9500, 200, seed, 5));
                prop_assert_eq!(a, b);
            }

            #[test]
            fn cluster_order_never_changes_the_cluster_interval(
                clusters in prop::collection::vec(prop::collection::vec(-3.0f64..1.0, 1..6), 2..10),
                seed in 0u64..1000,
                rotate in 0usize..10,
            ) {
                let a = bootstrap_median_clustered(&clusters, 9500, 200, seed, 5);
                let mut shuffled: Vec<Vec<f64>> = clusters
                    .iter()
                    .map(|c| { let mut c = c.clone(); c.reverse(); c })
                    .collect();
                shuffled.reverse();
                let r = rotate % shuffled.len();
                shuffled.rotate_left(r);
                prop_assert_eq!(a, bootstrap_median_clustered(&shuffled, 9500, 200, seed, 5));
            }

            #[test]
            fn the_cluster_interval_lies_within_the_values(
                clusters in prop::collection::vec(prop::collection::vec(-3.0f64..1.0, 1..6), 2..10),
            ) {
                let all: Vec<f64> = clusters.iter().flatten().copied().collect();
                let d = distribution(&all).unwrap();
                if let Some(i) = bootstrap_median_clustered(&clusters, 9500, 200, 11, 5) {
                    prop_assert!(d.min - 1e-9 <= i.low && i.low <= i.high && i.high <= d.max + 1e-9);
                }
            }

            #[test]
            fn a_percentage_never_prints_negative_zero(x in -0.01f64..0.01) {
                let s = percent_text(x);
                prop_assert!(!s.starts_with("-0.0%") && s != "+0.0%", "{}", s);
            }

            #[test]
            fn the_median_lies_within_its_interval_bounds(v in prop::collection::vec(-3.0f64..1.0, 5..40)) {
                let d = distribution(&v).unwrap();
                prop_assert!(d.min <= d.median && d.median <= d.max);
                let i = bootstrap_median(&v, 9500, 300, 11, 5).unwrap();
                prop_assert!(d.min - 1e-9 <= i.low && i.high <= d.max + 1e-9);
            }
        }
    }
}
