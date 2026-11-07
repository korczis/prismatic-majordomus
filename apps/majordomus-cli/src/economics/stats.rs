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
    crate::order::numeric(&mut v);
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
    crate::order::numeric(&mut medians);
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
    // A resample is `n` draws of an index into `v`, and `v` is in order, so the resample in
    // order is `v` at its drawn indices in order: how often each index was drawn gives the
    // two order statistics `quantile(_, 0.5)` reads, without sorting the resample. Sorting it
    // was most of `generate`'s economics on 2026-10-15. The same draws and the same
    // arithmetic give the same median to the bit (`counting_is_sorting`, below).
    let h = (n - 1) as f64 * 0.5;
    let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
    let mut drawn = vec![0usize; n];
    for _ in 0..resamples {
        drawn.fill(0);
        for _ in 0..n {
            drawn[rng.below(n as u64) as usize] += 1;
        }
        let (a, b) = order_statistics(&v, &drawn, lo, hi);
        medians.push(a + (h - lo as f64) * (b - a));
    }
    percentile_interval(medians, level_bp, resamples, seed, INDEPENDENT_METHOD)
}

/// The `lo`-th and `hi`-th values (from zero, `lo <= hi`) of the resample that drew index `i`
/// of the ordered `v` `drawn[i]` times: where the running count of draws first passes each.
fn order_statistics(v: &[f64], drawn: &[usize], lo: usize, hi: usize) -> (f64, f64) {
    let mut seen = 0;
    let mut low = None;
    for (value, &times) in v.iter().zip(drawn) {
        seen += times;
        if low.is_none() && seen > lo {
            low = Some(*value);
        }
        if seen > hi {
            return (low.unwrap_or(*value), *value);
        }
    }
    // `drawn` sums to `v.len()`, which is more than `hi`: the loop has already returned
    let last = v.last().copied().unwrap_or(f64::NAN);
    (low.unwrap_or(last), last)
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
    crate::order::numeric_samples(&mut c);
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
        crate::order::numeric(&mut pool);
        medians.push(quantile(&pool, 0.5)?);
    }
    percentile_interval(medians, level_bp, resamples, seed, CLUSTER_METHOD)
}

/// Which way of a paired difference (treatment minus control) is the better one, as the
/// methodology declares it for a metric: a higher accepted share is better, fewer tokens are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Better {
    /// A positive difference favours the treatment.
    Higher,
    /// A negative difference favours the treatment.
    Lower,
}

/// The one verdict a paired comparison comes to, whether it was decided by the exact test
/// ([`test_verdict`]) or by an interval ([`interval_verdict`]): the treatment is better, it
/// is worse, or the sample cannot tell. A sample too small to reject anything is
/// `Inconclusive`, never a verdict in either direction.
///
/// ```
/// use majordomus_cli::economics::stats::{paired_sign_flip_test, test_verdict, Better, Verdict};
/// // twenty tasks; five moved up, the rest did not move: p = 2/32, not below 5%
/// let mut tasks: Vec<Vec<f64>> = vec![vec![0.0]; 15];
/// tasks.extend(vec![vec![1.0]; 5]);
/// let t = paired_sign_flip_test(&tasks, 1000, 7, 5).unwrap();
/// assert_eq!(t.p_value, 0.0625);
/// assert_eq!(test_verdict(&t, 9500, Better::Higher), Verdict::Inconclusive);
/// // a sixth task moves up: p = 2/64 rejects, and the direction decides the verdict
/// tasks[0] = vec![1.0];
/// let t = paired_sign_flip_test(&tasks, 1000, 7, 5).unwrap();
/// assert_eq!(t.p_value, 0.03125);
/// assert_eq!(test_verdict(&t, 9500, Better::Higher), Verdict::Favourable);
/// assert_eq!(test_verdict(&t, 9500, Better::Lower), Verdict::Unfavourable);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The difference is significant and points the declared better way.
    Favourable,
    /// The difference is significant and points the other way.
    Unfavourable,
    /// The sample does not reject a zero difference.
    Inconclusive,
}

/// The verdict a difference of the given sign comes to when it is significant.
fn directed(difference: f64, better: Better) -> Verdict {
    match (difference > 0.0, difference < 0.0, better) {
        (true, _, Better::Higher) | (_, true, Better::Lower) => Verdict::Favourable,
        (true, _, Better::Lower) | (_, true, Better::Higher) => Verdict::Unfavourable,
        _ => Verdict::Inconclusive,
    }
}

/// The largest number of tasks that moved whose sign flips are enumerated rather than
/// sampled: 2^20 sums, about a million, is still instant.
pub const EXACT_MAX_TASKS: usize = 20;

/// What [`paired_sign_flip_test`] writes into [`PairedTest::method`] when it enumerated.
pub const EXACT_SIGN_FLIP_METHOD: &str = "exact two-sided sign-flip permutation test over tasks of the mean per-task paired difference: every assignment of signs to the tasks that moved is enumerated";

/// What [`paired_sign_flip_test`] writes into [`PairedTest::method`] when it sampled.
pub const SAMPLED_SIGN_FLIP_METHOD: &str = "two-sided sign-flip permutation test over tasks of the mean per-task paired difference: seeded random sign assignments, p = (1 + as extreme) / (1 + resamples)";

/// What [`minimum_detectable_difference`] writes into its `method`.
pub const MDE_METHOD: &str = "minimum detectable difference of the exact sign-flip test: the fewest tasks that, all moving one way by a whole accepted outcome with every other task unmoved, reject at the level, as a share of the tasks";

/// What [`bootstrap_mean_difference_clustered`] writes into [`EconomicsInterval::method`].
pub const CLUSTER_MEAN_DIFFERENCE_METHOD: &str = "percentile bootstrap of the mean paired difference, cluster bootstrap over tasks: each resample draws tasks with replacement and averages the drawn tasks' mean differences";

/// The result of [`paired_sign_flip_test`].
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PairedTest {
    /// The tasks (clusters) that held at least one finite difference.
    pub tasks: usize,
    /// The tasks whose mean difference is not zero: the only ones a sign flip changes.
    pub moved: usize,
    /// The mean over tasks of each task's mean difference.
    pub mean_difference: f64,
    /// The two-sided p-value of a zero mean difference.
    pub p_value: f64,
    /// Whether every sign assignment was enumerated.
    pub exact: bool,
    /// How many sign assignments the p-value counts over.
    pub permutations: u64,
    /// The seed of the sampled test; unused, but recorded, by the exact one.
    pub seed: u64,
    /// How the p-value was computed.
    pub method: String,
}

/// Each task's mean finite difference, tasks without one dropped, in ascending order, so
/// that nothing downstream depends on the order the tasks or their values were listed in.
fn task_means(clusters: &[Vec<f64>]) -> (Vec<f64>, usize) {
    let mut n = 0;
    let mut means: Vec<f64> = clusters
        .iter()
        .map(|v| sorted(v.as_slice()))
        .filter(|v| !v.is_empty())
        .map(|v| {
            n += v.len();
            v.iter().sum::<f64>() / v.len() as f64
        })
        .collect();
    crate::order::numeric(&mut means);
    (means, n)
}

/// The exact task-level test of a binary paired outcome: each cluster holds one task's
/// paired differences (treatment minus control, `1.0`, `0.0` or `-1.0` per repetition for an
/// accepted outcome), each task is reduced to its mean difference, and the statistic is the
/// mean of those. Under no effect each task's difference is as likely negated, so the
/// p-value is the share of sign assignments whose absolute sum is at least the observed one.
///
/// A task that did not move contributes the same under either sign, so only the tasks that
/// moved are flipped; with at most [`EXACT_MAX_TASKS`] of them every assignment is
/// enumerated and the test is exact, above that `resamples` seeded assignments are drawn.
/// Unlike a percentile bootstrap over twenty mostly-zero clusters, it does not under-cover.
///
/// Deterministic for a seed and independent of the order of tasks and values; non-finite
/// values are dropped; `None` below `min_n` differences in total, with fewer than two tasks,
/// or when it would have to sample and `resamples` is zero.
///
/// ```
/// use majordomus_cli::economics::stats::{paired_sign_flip_test, EXACT_SIGN_FLIP_METHOD};
/// let tasks = vec![vec![1.0, 0.0], vec![1.0], vec![0.0, 0.0], vec![1.0, 1.0], vec![-1.0]];
/// let t = paired_sign_flip_test(&tasks, 1000, 7, 5).unwrap();
/// let shuffled = vec![vec![1.0, 1.0], vec![-1.0], vec![0.0, 1.0], vec![0.0, 0.0], vec![1.0]];
/// assert_eq!(t, paired_sign_flip_test(&shuffled, 1000, 7, 5).unwrap(), "order does not matter");
/// // moved tasks 0.5, 1, 1, -1: |sum| >= 1.5 in 10 of the 16 sign assignments
/// assert_eq!((t.moved, t.p_value, t.exact), (4, 0.625, true));
/// assert_eq!(t.method, EXACT_SIGN_FLIP_METHOD);
/// let one_task = vec![vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0]];
/// assert!(paired_sign_flip_test(&one_task, 1000, 7, 5).is_none(), "one task is no test");
/// ```
pub fn paired_sign_flip_test(
    clusters: &[Vec<f64>],
    resamples: u32,
    seed: u64,
    min_n: usize,
) -> Option<PairedTest> {
    let (means, n) = task_means(clusters);
    let k = means.len();
    if n < min_n.max(2) || k < 2 {
        return None;
    }
    let mean_difference = means.iter().sum::<f64>() / k as f64;
    let moved: Vec<f64> = means.iter().copied().filter(|d| *d != 0.0).collect();
    let m = moved.len();
    let observed = moved.iter().sum::<f64>().abs();
    // the sums are of the same few values in another sign, so an equal sum may differ in
    // the last bits: anything within a billionth of the total magnitude counts as equal
    let tolerance = 1e-9 * moved.iter().map(|d| d.abs()).sum::<f64>();
    let as_extreme = |signs: &mut dyn FnMut(usize) -> bool| {
        let s: f64 = (0..m)
            .map(|i| if signs(i) { -moved[i] } else { moved[i] })
            .sum();
        s.abs() >= observed - tolerance
    };
    let (p_value, exact, permutations, method) = if m <= EXACT_MAX_TASKS {
        let total = 1u64 << m;
        let hits = (0..total)
            .filter(|&mask| as_extreme(&mut |i| (mask >> i) & 1 == 1))
            .count() as u64;
        (
            hits as f64 / total as f64,
            true,
            total,
            EXACT_SIGN_FLIP_METHOD,
        )
    } else {
        if resamples == 0 {
            return None;
        }
        let mut rng = SplitMix64(seed);
        let hits = (0..resamples)
            .filter(|_| as_extreme(&mut |_| rng.next() >> 63 == 1))
            .count() as u64;
        (
            (1 + hits) as f64 / (1 + resamples as u64) as f64,
            false,
            resamples as u64,
            SAMPLED_SIGN_FLIP_METHOD,
        )
    };
    Some(PairedTest {
        tasks: k,
        moved: m,
        mean_difference: round(mean_difference),
        p_value,
        exact,
        permutations,
        seed,
        method: method.into(),
    })
}

/// The verdict of a [`PairedTest`] at `level_bp`: `Inconclusive` unless the p-value is at
/// most one minus the level, and then favourable or unfavourable by the sign of the mean
/// difference against the declared better direction (see [`Verdict`]).
pub fn test_verdict(test: &PairedTest, level_bp: u32, better: Better) -> Verdict {
    let alpha = 1.0 - level_bp as f64 / 10_000.0;
    if test.p_value <= alpha {
        directed(test.mean_difference, better)
    } else {
        Verdict::Inconclusive
    }
}

/// The verdict of an interval of a paired difference: favourable or unfavourable by
/// direction when it excludes zero, `Inconclusive` when it holds zero.
///
/// ```
/// use majordomus_cli::economics::stats::{interval_verdict, Better, Verdict};
/// use majordomus_cli::economics::model::EconomicsInterval;
/// let i = EconomicsInterval {
///     level_bp: 9500, low: 0.1, high: 0.3, method: "m".into(), resamples: 1, seed: 1,
/// };
/// assert_eq!(interval_verdict(&i, Better::Higher), Verdict::Favourable);
/// assert_eq!(interval_verdict(&i, Better::Lower), Verdict::Unfavourable);
/// let across = EconomicsInterval { low: -0.1, ..i };
/// assert_eq!(interval_verdict(&across, Better::Higher), Verdict::Inconclusive);
/// ```
pub fn interval_verdict(interval: &EconomicsInterval, better: Better) -> Verdict {
    if interval.low > 0.0 {
        directed(interval.low, better)
    } else if interval.high < 0.0 {
        directed(interval.high, better)
    } else {
        Verdict::Inconclusive
    }
}

/// The smallest difference the exact test can detect, and how it was derived.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MinimumDetectableDifference {
    /// The tasks of the comparison.
    pub tasks: usize,
    /// The confidence level, in basis points.
    pub level_bp: u32,
    /// The fewest tasks that must move, all one way, for the test to reject.
    pub tasks_needed: usize,
    /// `tasks_needed / tasks`: the mean accepted-share difference that takes.
    pub difference: f64,
    /// How it was derived.
    pub method: String,
}

/// The minimum detectable difference of [`paired_sign_flip_test`] for `tasks` tasks at
/// `level_bp`: when `j` tasks move one way and the rest do not, the exact p-value is
/// `2 / 2^j`, so the test rejects only from the smallest `j` with `2 / 2^j` at most one minus
/// the level, and a smaller mean accepted-share difference than `j / tasks` (one accepted
/// outcome per task) can never be called. `None` with fewer than two tasks, or when even every
/// task moving one way would not reject: such a comparison can only be inconclusive.
///
/// ```
/// use majordomus_cli::economics::stats::{minimum_detectable_difference, MDE_METHOD};
/// // the methodology's level is 95%: 2/32 does not reject, 2/64 does
/// let mde = minimum_detectable_difference(20, 9500).unwrap();
/// assert_eq!((mde.tasks_needed, mde.difference), (6, 0.3));
/// assert_eq!(mde.method, MDE_METHOD);
/// assert!(minimum_detectable_difference(5, 9500).is_none(), "2/32 never rejects at 5%");
/// assert!(minimum_detectable_difference(1, 9500).is_none());
/// ```
pub fn minimum_detectable_difference(
    tasks: usize,
    level_bp: u32,
) -> Option<MinimumDetectableDifference> {
    if tasks < 2 {
        return None;
    }
    let alpha = 1.0 - level_bp as f64 / 10_000.0;
    let needed = (1..=tasks.min(1_000)).find(|&j| 2f64.powi(1 - j as i32) <= alpha)?;
    Some(MinimumDetectableDifference {
        tasks,
        level_bp,
        tasks_needed: needed,
        difference: round(needed as f64 / tasks as f64),
        method: MDE_METHOD.into(),
    })
}

/// A percentile bootstrap interval of the mean paired difference of a continuous outcome
/// (tokens, seconds) over tasks: each task is reduced to its mean difference, each resample
/// draws as many tasks as there are with replacement, and the statistic is the mean of the
/// drawn tasks' means, so every task weighs the same however often it was repeated, as in
/// [`paired_sign_flip_test`]. Read with [`interval_verdict`].
///
/// Deterministic for a seed and independent of the order of tasks and values; non-finite
/// values are dropped; `None` below `min_n` differences in total, with fewer than two tasks,
/// or with no resamples, as [`bootstrap_median_clustered`].
///
/// ```
/// use majordomus_cli::economics::stats::{
///     bootstrap_mean_difference_clustered, CLUSTER_MEAN_DIFFERENCE_METHOD,
/// };
/// let tasks = vec![vec![-120.0, -100.0], vec![-300.0], vec![-50.0, -70.0], vec![10.0, -30.0]];
/// let a = bootstrap_mean_difference_clustered(&tasks, 9500, 2000, 7, 5).unwrap();
/// let shuffled = vec![vec![-30.0, 10.0], vec![-300.0], vec![-70.0, -50.0], vec![-100.0, -120.0]];
/// assert_eq!(a, bootstrap_mean_difference_clustered(&shuffled, 9500, 2000, 7, 5).unwrap());
/// assert!(a.low <= -115.0 && -115.0 <= a.high, "the mean of task means is -115");
/// assert_eq!(a.method, CLUSTER_MEAN_DIFFERENCE_METHOD);
/// let one_task = vec![vec![-1.0, -2.0, -3.0, -4.0, -5.0, -6.0]];
/// assert!(bootstrap_mean_difference_clustered(&one_task, 9500, 2000, 7, 5).is_none());
/// ```
pub fn bootstrap_mean_difference_clustered(
    clusters: &[Vec<f64>],
    level_bp: u32,
    resamples: u32,
    seed: u64,
    min_n: usize,
) -> Option<EconomicsInterval> {
    let (means, n) = task_means(clusters);
    let k = means.len();
    if n < min_n.max(2) || k < 2 || resamples == 0 {
        return None;
    }
    let mut rng = SplitMix64(seed);
    let estimates = (0..resamples)
        .map(|_| {
            (0..k)
                .map(|_| means[rng.below(k as u64) as usize])
                .sum::<f64>()
                / k as f64
        })
        .collect();
    percentile_interval(
        estimates,
        level_bp,
        resamples,
        seed,
        CLUSTER_MEAN_DIFFERENCE_METHOD,
    )
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

    /// Counting the draws answers what sorting each resample answered, to the bit: the
    /// interval is compared with the one the sorting implementation computed, over odd and
    /// even sizes, repeated values, both zeros, negative values and many seeds.
    #[test]
    fn counting_is_sorting() {
        fn by_sorting(
            values: &[f64],
            level_bp: u32,
            resamples: u32,
            seed: u64,
        ) -> EconomicsInterval {
            let v = sorted(values);
            let n = v.len();
            let mut rng = SplitMix64(seed);
            let mut medians = Vec::with_capacity(resamples as usize);
            let mut sample = vec![0.0; n];
            for _ in 0..resamples {
                for slot in sample.iter_mut() {
                    *slot = v[rng.below(n as u64) as usize];
                }
                crate::order::numeric(&mut sample);
                medians.push(quantile(&sample, 0.5).unwrap());
            }
            percentile_interval(medians, level_bp, resamples, seed, INDEPENDENT_METHOD).unwrap()
        }
        let mut sets: Vec<Vec<f64>> = (2..=13)
            .map(|n| (0..n).map(|i| ((i * 7 % 5) as f64 - 2.0) * 0.1).collect())
            .collect();
        sets.push(vec![0.0, -0.0, 0.0, -0.0, 1.0]);
        sets.push(vec![0.25; 9]);
        sets.push((0..40).map(|i| (i as f64 * 0.37).sin()).collect());
        sets.push(vec![f64::NAN, 3.0, -1.5, f64::INFINITY, 2.0, 2.0]);
        for v in &sets {
            for seed in 0..25 {
                let counted = bootstrap_median(v, 9500, 300, seed, 2).unwrap();
                let sorted = by_sorting(v, 9500, 300, seed);
                assert_eq!(
                    counted.low.to_bits(),
                    sorted.low.to_bits(),
                    "{v:?} seed {seed}"
                );
                assert_eq!(
                    counted.high.to_bits(),
                    sorted.high.to_bits(),
                    "{v:?} seed {seed}"
                );
                assert_eq!(counted, sorted, "{v:?} seed {seed}");
            }
        }
    }

    /// Draws that never pass `hi` — which the bootstrap cannot produce, since its draws sum
    /// to the sample size — still answer: the high statistic is the last value, the low one
    /// whatever was passed, or the last value too, and an empty sample is not a number.
    #[test]
    fn draws_that_never_pass_the_high_rank_answer_with_the_last_value() {
        let v = [1.0, 2.0, 3.0];
        // one draw of the middle value passes rank 0 but never rank 1
        assert_eq!(order_statistics(&v, &[0, 1, 0], 0, 1), (2.0, 3.0));
        // no draw at all passes neither rank
        assert_eq!(order_statistics(&v, &[0, 0, 0], 0, 1), (3.0, 3.0));
        let (low, high) = order_statistics(&[], &[], 0, 0);
        assert!(low.is_nan() && high.is_nan(), "{low} {high}");
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

    /// The exact sign-flip test against p-values computed by hand: the sign test of tasks
    /// that all moved one way, a mixed set, fractional task means, order independence, the
    /// enumeration limit, the seeded sample above it, and every refusal.
    #[test]
    fn test_exact_paired_test() {
        let one = |d: f64| vec![d];
        // six of twenty tasks up, the rest unmoved: 2 of 2^6 assignments as extreme
        let mut tasks: Vec<Vec<f64>> = (0..14).map(|_| one(0.0)).collect();
        tasks.extend((0..6).map(|_| one(1.0)));
        let t = paired_sign_flip_test(&tasks, 100, 1, 5).unwrap();
        assert_eq!(
            (t.tasks, t.moved, t.exact, t.permutations),
            (20, 6, true, 64)
        );
        assert_eq!(t.p_value, 2.0 / 64.0);
        assert_eq!(t.mean_difference, 0.3);
        assert_eq!(t.method, EXACT_SIGN_FLIP_METHOD);
        // five up and one down: |sum| >= 4 when 0, 1, 5 or 6 of six are up, 14 of 64
        let mixed = vec![one(1.0), one(1.0), one(1.0), one(1.0), one(1.0), one(-1.0)];
        assert_eq!(
            paired_sign_flip_test(&mixed, 100, 1, 5).unwrap().p_value,
            14.0 / 64.0
        );
        // task means 0.5, 1, 1: only the all-same signs reach |sum| 2.5, 2 of 8
        let fractional = vec![vec![1.0, 0.0], one(1.0), vec![1.0, 1.0]];
        assert_eq!(
            paired_sign_flip_test(&fractional, 100, 1, 4)
                .unwrap()
                .p_value,
            0.25
        );
        // nothing moved: every assignment is as extreme as zero
        let flat: Vec<Vec<f64>> = (0..8).map(|_| one(0.0)).collect();
        let f = paired_sign_flip_test(&flat, 100, 1, 5).unwrap();
        assert_eq!((f.p_value, f.moved, f.mean_difference), (1.0, 0, 0.0));
        // order of tasks and of values, and non-finite values, change nothing
        let a = vec![
            vec![1.0, 0.0, f64::NAN],
            one(-1.0),
            vec![0.0, 1.0, 1.0],
            one(1.0),
        ];
        let b = vec![
            one(1.0),
            vec![1.0, 0.0, 1.0],
            one(-1.0),
            vec![f64::INFINITY, 0.0, 1.0],
        ];
        assert_eq!(
            paired_sign_flip_test(&a, 100, 1, 5),
            paired_sign_flip_test(&b, 100, 1, 5)
        );
        // twenty moved tasks are still enumerated; twenty-two are sampled from the seed
        let twenty: Vec<Vec<f64>> = (0..EXACT_MAX_TASKS).map(|_| one(1.0)).collect();
        let e = paired_sign_flip_test(&twenty, 100, 1, 5).unwrap();
        assert_eq!((e.exact, e.p_value), (true, 2.0 / (1u64 << 20) as f64));
        let many: Vec<Vec<f64>> = (0..22).map(|_| one(1.0)).collect();
        let s = paired_sign_flip_test(&many, 999, 3, 5).unwrap();
        assert_eq!((s.exact, s.permutations, s.seed), (false, 999, 3));
        assert_eq!(s.method, SAMPLED_SIGN_FLIP_METHOD);
        assert!(s.p_value <= 2.0 / 1000.0, "{}", s.p_value);
        assert_eq!(Some(s), paired_sign_flip_test(&many, 999, 3, 5));
        assert!(
            paired_sign_flip_test(&many, 0, 3, 5).is_none(),
            "nothing to sample"
        );
        // refusals: one task, too few differences, empty and non-finite tasks
        assert!(paired_sign_flip_test(&[vec![1.0; 9]], 100, 1, 5).is_none());
        assert!(paired_sign_flip_test(&[one(1.0), one(1.0)], 100, 1, 5).is_none());
        let hollow = vec![vec![], vec![1.0, 1.0, 1.0, 1.0, 1.0], vec![f64::NAN]];
        assert!(paired_sign_flip_test(&hollow, 100, 1, 2).is_none());
        // the minimum detectable difference at the methodology's level and others
        let mde = minimum_detectable_difference(20, 9500).unwrap();
        assert_eq!((mde.tasks_needed, mde.difference), (6, 0.3));
        assert_eq!(
            minimum_detectable_difference(40, 9500).unwrap().difference,
            0.15
        );
        assert_eq!(
            minimum_detectable_difference(20, 9900)
                .unwrap()
                .tasks_needed,
            8
        );
        assert!(minimum_detectable_difference(20, 10_000).is_none());
        assert!(minimum_detectable_difference(5, 9500).is_none());
        // the MDE is where the test first rejects: six tasks reject, five do not
        let at = |j: usize| {
            let mut t: Vec<Vec<f64>> = (0..20 - j).map(|_| one(0.0)).collect();
            t.extend((0..j).map(|_| one(1.0)));
            test_verdict(
                &paired_sign_flip_test(&t, 100, 1, 5).unwrap(),
                9500,
                Better::Higher,
            )
        };
        assert_eq!((at(5), at(6)), (Verdict::Inconclusive, Verdict::Favourable));
    }

    /// One verdict from either decision: the exact test by its p-value and direction, an
    /// interval by whether it excludes zero, and the clustered mean-difference interval
    /// that feeds it computed, refused and named.
    #[test]
    fn test_interval_verdict() {
        let i = |low: f64, high: f64| EconomicsInterval {
            level_bp: 9500,
            low,
            high,
            method: CLUSTER_MEAN_DIFFERENCE_METHOD.into(),
            resamples: 1,
            seed: 1,
        };
        assert_eq!(
            interval_verdict(&i(0.1, 0.2), Better::Higher),
            Verdict::Favourable
        );
        assert_eq!(
            interval_verdict(&i(0.1, 0.2), Better::Lower),
            Verdict::Unfavourable
        );
        assert_eq!(
            interval_verdict(&i(-0.2, -0.1), Better::Lower),
            Verdict::Favourable
        );
        assert_eq!(
            interval_verdict(&i(-0.2, -0.1), Better::Higher),
            Verdict::Unfavourable
        );
        assert_eq!(
            interval_verdict(&i(-0.1, 0.1), Better::Higher),
            Verdict::Inconclusive
        );
        assert_eq!(
            interval_verdict(&i(0.0, 0.1), Better::Higher),
            Verdict::Inconclusive
        );
        assert_eq!(
            interval_verdict(&i(0.0, 0.0), Better::Lower),
            Verdict::Inconclusive
        );
        let t = |p_value: f64, mean_difference: f64| PairedTest {
            tasks: 20,
            moved: 6,
            mean_difference,
            p_value,
            exact: true,
            permutations: 64,
            seed: 0,
            method: EXACT_SIGN_FLIP_METHOD.into(),
        };
        assert_eq!(
            test_verdict(&t(0.05, -0.3), 9500, Better::Lower),
            Verdict::Favourable
        );
        assert_eq!(
            test_verdict(&t(0.05, -0.3), 9500, Better::Higher),
            Verdict::Unfavourable
        );
        assert_eq!(
            test_verdict(&t(0.051, -0.3), 9500, Better::Lower),
            Verdict::Inconclusive
        );
        assert_eq!(
            test_verdict(&t(0.03, 0.3), 9900, Better::Higher),
            Verdict::Inconclusive
        );
        assert_eq!(
            test_verdict(&t(0.0, 0.0), 9500, Better::Higher),
            Verdict::Inconclusive
        );
        // the clustered mean difference: every task weighs the same, so tasks of two
        // identical values each make the bootstrap of the task means a known point
        let same = vec![vec![-0.2, -0.2], vec![-0.2, -0.2, -0.2], vec![-0.2]];
        let p = bootstrap_mean_difference_clustered(&same, 9500, 500, 3, 5).unwrap();
        assert_eq!((p.low, p.high), (-0.2, -0.2));
        assert_eq!(interval_verdict(&p, Better::Lower), Verdict::Favourable);
        // a task repeated many times does not outweigh the others: means -1 and 1 give 0
        let lopsided = vec![vec![-1.0; 9], vec![1.0]];
        let l = bootstrap_mean_difference_clustered(&lopsided, 9500, 2000, 3, 5).unwrap();
        assert!(l.low < 0.0 && 0.0 < l.high, "{l:?}");
        assert_eq!(interval_verdict(&l, Better::Lower), Verdict::Inconclusive);
        assert_eq!(l.method, CLUSTER_MEAN_DIFFERENCE_METHOD);
        let c: Vec<Vec<f64>> = (0..8)
            .map(|t| {
                (0..3)
                    .map(|r| (t as f64 * 0.37 + r as f64 * 0.01).sin())
                    .collect()
            })
            .collect();
        let a = bootstrap_mean_difference_clustered(&c, 9500, 1000, 1, 5).unwrap();
        let mut reversed: Vec<Vec<f64>> = c
            .iter()
            .map(|v| v.iter().rev().copied().collect())
            .collect();
        reversed.reverse();
        assert_eq!(
            Some(a.clone()),
            bootstrap_mean_difference_clustered(&reversed, 9500, 1000, 1, 5)
        );
        assert!(Some(a) != bootstrap_mean_difference_clustered(&c, 9500, 1000, 2, 5));
        assert!(bootstrap_mean_difference_clustered(&[vec![1.0; 9]], 9500, 500, 3, 5).is_none());
        assert!(bootstrap_mean_difference_clustered(&same, 9500, 0, 3, 5).is_none());
        assert!(bootstrap_mean_difference_clustered(&same, 9500, 500, 3, 7).is_none());
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
