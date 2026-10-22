//! The arithmetic of the economics subsystem, and nothing else: the reduction formula,
//! order statistics, a seeded bootstrap (over independent values, or over clusters when
//! the values are repetitions of the same task), the paired with/without comparison (an
//! exact test over tasks, its minimum detectable difference, an interval of a mean paired
//! difference, and the one [`Verdict`] either becomes), and the two ways a number is printed.
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

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    let c = resampleable_clusters(clusters, min_n, resamples)?;
    let (n, k) = (c.iter().map(Vec::len).sum::<usize>(), c.len());
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

/// The clusters a cluster bootstrap draws from, in an order that depends on their values
/// alone: non-finite values dropped, each cluster sorted, empty clusters dropped, the
/// clusters sorted. `None` below `min_n` values in total, with fewer than two clusters, or
/// with no resamples.
fn resampleable_clusters(
    clusters: &[Vec<f64>],
    min_n: usize,
    resamples: u32,
) -> Option<Vec<Vec<f64>>> {
    let mut c: Vec<Vec<f64>> = clusters
        .iter()
        .map(|v| sorted(v.as_slice()))
        .filter(|v| !v.is_empty())
        .collect();
    crate::order::numeric_samples(&mut c);
    let n: usize = c.iter().map(Vec::len).sum();
    (n >= min_n.max(2) && c.len() >= 2 && resamples > 0).then_some(c)
}

/// What [`bootstrap_mean_difference_clustered`] writes into [`EconomicsInterval::method`].
pub const CLUSTER_MEAN_METHOD: &str = "percentile bootstrap of the mean paired difference, cluster bootstrap over tasks: each resample draws tasks with replacement and pools every paired difference of each drawn task";

/// A percentile bootstrap interval of the mean paired difference of a continuous outcome
/// (wall time, tokens, spend): each cluster holds one task's paired differences, treatment
/// minus control, one per repetition; each resample draws as many tasks as there are, with
/// replacement, and takes the mean of every difference of the drawn tasks.
///
/// Whole tasks are resampled for the reason [`bootstrap_median_clustered`] gives. The
/// mean, not the median, because a with/without difference is judged by its expectation:
/// a treatment that saves a little on most tasks and costs a great deal on one is not
/// favourable, and a median would say it was. An interval that excludes zero becomes a
/// direction through [`Verdict::of_interval`].
///
/// Deterministic for a seed and independent of the order of the clusters and of the
/// values within them; `None` below `min_n` differences in total, with fewer than two
/// clusters, or with no resamples.
///
/// ```
/// use majordomus_cli::economics::stats::{bootstrap_mean_difference_clustered, CLUSTER_MEAN_METHOD};
/// let tasks = vec![vec![-120.0, -100.0], vec![-40.0, -60.0, -50.0], vec![10.0, -10.0]];
/// let a = bootstrap_mean_difference_clustered(&tasks, 9500, 2000, 7, 5).unwrap();
/// let shuffled = vec![vec![-10.0, 10.0], vec![-100.0, -120.0], vec![-50.0, -60.0, -40.0]];
/// assert_eq!(a, bootstrap_mean_difference_clustered(&shuffled, 9500, 2000, 7, 5).unwrap());
/// assert!(a.low <= -52.857 && -52.857 <= a.high, "the pooled mean lies inside: {a:?}");
/// assert_eq!(a.method, CLUSTER_MEAN_METHOD);
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
    let c = resampleable_clusters(clusters, min_n, resamples)?;
    let k = c.len();
    let mut rng = SplitMix64(seed);
    let mut means = Vec::with_capacity(resamples as usize);
    for _ in 0..resamples {
        let (mut sum, mut count) = (0.0, 0usize);
        for _ in 0..k {
            let task = &c[rng.below(k as u64) as usize];
            sum += task.iter().sum::<f64>();
            count += task.len();
        }
        means.push(sum / count as f64);
    }
    percentile_interval(means, level_bp, resamples, seed, CLUSTER_MEAN_METHOD)
}

/// The most tasks [`paired_sign_test`] enumerates every sign assignment of: `2^20`
/// assignments, about a million sums of twenty differences. Above it the test draws them.
pub const EXACT_TASKS: usize = 20;

/// What [`paired_sign_test`] writes into [`PairedTest::method`] when it enumerated every
/// sign assignment.
pub const EXACT_SIGN_FLIP_METHOD: &str = "exact sign-flip permutation test over tasks of the mean per-task difference in share, two-sided: every assignment of signs to the tasks' differences enumerated";

/// What [`paired_sign_test`] writes into [`PairedTest::method`] when there were more than
/// [`EXACT_TASKS`] tasks and it drew sign assignments instead.
pub const SAMPLED_SIGN_FLIP_METHOD: &str = "sign-flip permutation test over tasks of the mean per-task difference in share, two-sided: seeded random assignments of signs, p = (1 + assignments at least as extreme) / (1 + assignments drawn)";

/// How far apart two sums of the same differences may be and still be the same sum: the
/// observed sum and a sign assignment's sum add the same terms in different orders, so an
/// assignment exactly as extreme as the observation can differ from it in the last bits.
const SUM_TOLERANCE: f64 = 1e-9;

/// The outcome of [`paired_sign_test`]: the observed difference, its p-value and how it
/// was computed. Whether it rejects is [`PairedTest::rejects`]; what that means for the
/// product is [`Verdict::of_test`].
///
/// ```
/// use majordomus_cli::economics::stats::paired_sign_test;
/// let tasks: Vec<(Vec<f64>, Vec<f64>)> = (0..6).map(|_| (vec![0.0], vec![1.0])).collect();
/// let t = paired_sign_test(&tasks, 9500, 0, 7, 5).unwrap();
/// let json = serde_json::to_value(&t).unwrap();
/// assert_eq!((json["tasks"].clone(), json["p_value"].clone()), (6.into(), (2.0 / 64.0).into()));
/// assert!(json["seed"].is_null(), "an exact test draws nothing, so it has no seed");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PairedTest {
    /// Confidence in basis points: the test rejects at a p-value of at most
    /// `1 - level_bp / 10_000`.
    pub level_bp: u32,
    /// The tasks compared: the effective sample, whatever the repetitions.
    pub tasks: usize,
    /// The mean over tasks of the treatment's share minus the control's.
    pub difference: f64,
    /// The two-sided p-value.
    pub p_value: f64,
    /// Whether every sign assignment was enumerated rather than drawn.
    pub exact: bool,
    /// The sign assignments enumerated or drawn.
    pub assignments: u64,
    /// The seed of the draws; absent for an exact test, which draws nothing.
    pub seed: Option<u64>,
    /// The test, in words.
    pub method: String,
}

impl PairedTest {
    /// Whether the p-value is at most `1 - level_bp / 10_000`: a p-value of exactly the
    /// level's 5 % rejects at 95 %, anything above it does not.
    ///
    /// ```
    /// use majordomus_cli::economics::stats::{PairedTest, EXACT_SIGN_FLIP_METHOD};
    /// let t = PairedTest {
    ///     level_bp: 9500, tasks: 20, difference: 0.2, p_value: 0.05, exact: true,
    ///     assignments: 1 << 20, seed: None, method: EXACT_SIGN_FLIP_METHOD.into(),
    /// };
    /// assert!(t.rejects());
    /// assert!(!PairedTest { p_value: 0.0501, ..t.clone() }.rejects());
    /// assert!(!PairedTest { level_bp: 9900, ..t }.rejects(), "0.05 is not 1 %");
    /// ```
    pub fn rejects(&self) -> bool {
        self.p_value <= significance(self.level_bp)
    }
}

/// `1 - level_bp / 10_000`, computed from whole basis points so that `9500` is exactly the
/// `0.05` a methodology declares, not `1 - 0.95`.
fn significance(level_bp: u32) -> f64 {
    10_000u32.saturating_sub(level_bp) as f64 / 10_000.0
}

/// The mean of the finite values, summed in order so that it does not depend on the order
/// they were listed in; `None` when there are none.
fn mean(values: &[f64]) -> Option<f64> {
    let v = sorted(values);
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

/// The exact task-level test of a binary paired outcome: did the treatment change the
/// share of accepted runs? Each task is `(control, treatment)`, the outcomes of its runs
/// in each arm, `1.0` accepted and `0.0` not. A task's difference is its treatment share
/// minus its control share, and the statistic is the mean of those differences over tasks.
///
/// Under the hypothesis that the treatment changes nothing, which arm a task's runs fell in
/// is arbitrary, so each task's difference is as likely to have the opposite sign. The
/// p-value is the share of the `2^tasks` sign assignments whose mean is at least as far
/// from zero as the observed one. With at most [`EXACT_TASKS`] tasks every assignment is
/// enumerated and the p-value is exact; above that `draws` assignments are drawn from
/// `seed`. Tasks, not runs, are the sample: twenty tasks run four times each are twenty
/// observations, never eighty.
///
/// This is the test rather than a percentile bootstrap because the differences of a binary
/// outcome over twenty tasks are mostly zero, and a percentile interval over so few
/// distinct values covers less than it says.
///
/// Non-finite outcomes are dropped and a task with an empty arm is dropped. Independent of
/// the order of the tasks and of the runs within them. `None` below `min_n` tasks or with
/// fewer than two, and with more than [`EXACT_TASKS`] tasks and no draws.
///
/// ```
/// use majordomus_cli::economics::stats::{paired_sign_test, EXACT_SIGN_FLIP_METHOD};
/// // eight tasks; with Majordomus every task accepts more of its four runs
/// let tasks: Vec<(Vec<f64>, Vec<f64>)> = (0..8)
///     .map(|t| (vec![1.0, 0.0, 0.0, if t % 2 == 0 { 1.0 } else { 0.0 }], vec![1.0, 1.0, 1.0, 1.0]))
///     .collect();
/// let test = paired_sign_test(&tasks, 9500, 0, 7, 5).unwrap();
/// assert_eq!((test.tasks, test.difference), (8, 0.625));
/// assert_eq!(test.p_value, 2.0 / 256.0, "only the observed signs and their mirror are as extreme");
/// assert!(test.exact && test.rejects() && test.seed.is_none());
/// assert_eq!(test.method, EXACT_SIGN_FLIP_METHOD);
/// let mut reordered = tasks.clone();
/// reordered.reverse();
/// reordered.iter_mut().for_each(|(c, _)| c.reverse());
/// assert_eq!(paired_sign_test(&reordered, 9500, 0, 7, 5).unwrap(), test);
/// assert!(paired_sign_test(&tasks[..1], 9500, 0, 7, 1).is_none(), "one task is no comparison");
/// ```
pub fn paired_sign_test(
    tasks: &[(Vec<f64>, Vec<f64>)],
    level_bp: u32,
    draws: u32,
    seed: u64,
    min_n: usize,
) -> Option<PairedTest> {
    let differences: Vec<f64> = tasks
        .iter()
        .filter_map(|(control, treatment)| Some(mean(treatment)? - mean(control)?))
        .collect();
    let d = sorted(&differences);
    let k = d.len();
    if k < min_n.max(2) {
        return None;
    }
    let observed: f64 = d.iter().sum();
    let bar = observed.abs() - SUM_TOLERANCE;
    let (assignments, exact, p_value) = if k <= EXACT_TASKS {
        let all = 1u64 << k;
        let extreme = (0..all)
            .filter(|&negated| signed_sum(&d, |i| (negated >> i) & 1 == 1).abs() >= bar)
            .count();
        (all, true, extreme as f64 / all as f64)
    } else {
        if draws == 0 {
            return None;
        }
        let mut rng = SplitMix64(seed);
        let mut word = 0;
        let extreme = (0..draws)
            .filter(|_| {
                let sum = signed_sum(&d, |i| {
                    if i % 64 == 0 {
                        word = rng.next();
                    }
                    (word >> (i % 64)) & 1 == 1
                });
                sum.abs() >= bar
            })
            .count();
        let p = (1 + extreme) as f64 / (1 + draws as usize) as f64;
        (u64::from(draws), false, p)
    };
    Some(PairedTest {
        level_bp,
        tasks: k,
        difference: round(observed / k as f64),
        p_value: round(p_value),
        exact,
        assignments,
        seed: (!exact).then_some(seed),
        method: if exact {
            EXACT_SIGN_FLIP_METHOD
        } else {
            SAMPLED_SIGN_FLIP_METHOD
        }
        .into(),
    })
}

/// The sum of the differences with the `i`-th negated where `negated(i)` says so, the
/// differences visited in order.
fn signed_sum(d: &[f64], mut negated: impl FnMut(usize) -> bool) -> f64 {
    d.iter()
        .enumerate()
        .map(|(i, x)| if negated(i) { -x } else { *x })
        .sum()
}

/// What [`minimum_detectable_difference`] writes into [`MinimumDetectable::method`].
pub const NORMAL_MDE_METHOD: &str = "minimum detectable difference by the normal approximation, (z(1 - alpha/2) + z(power)) * sd / sqrt(tasks), where the exact sign-flip test over that many tasks can reach the level at all";

/// The smallest mean per-task difference a paired comparison over `tasks` tasks detects
/// at its level with the given power: what a registration must state before the suite
/// runs, so that an inconclusive result can be read as "smaller than this, if anything".
///
/// ```
/// use majordomus_cli::economics::stats::minimum_detectable_difference;
/// let m = minimum_detectable_difference(20, 9500, 8000, 0.5).unwrap();
/// let json = serde_json::to_value(&m).unwrap();
/// assert_eq!((json["tasks"].clone(), json["power_bp"].clone()), (20.into(), 8000.into()));
/// assert!(m.difference > 0.0 && m.difference < 0.5, "{m:?}");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MinimumDetectable {
    /// The tasks the comparison will have.
    pub tasks: usize,
    /// Confidence in basis points, as the test's.
    pub level_bp: u32,
    /// Power in basis points: `8000` detects a difference of this size four times in five.
    pub power_bp: u32,
    /// The standard deviation of the per-task differences the registration assumes. A
    /// difference of two shares lies in `[-1, 1]`, so `1.0` is the bound no data can exceed.
    pub sd: f64,
    /// The minimum detectable difference, in the units of `sd`.
    pub difference: f64,
    /// How it was computed, in words.
    pub method: String,
}

/// The minimum detectable difference of a paired comparison over `tasks` tasks at
/// `level_bp`, with power `power_bp`, when the per-task differences have standard
/// deviation `sd`: `(z(1 - alpha/2) + z(power)) * sd / sqrt(tasks)`.
///
/// `None` with fewer than two tasks, a level or power outside `(0, 10_000)` basis points, an
/// `sd` that is negative or not a number, or when the exact sign-flip test over `tasks`
/// tasks cannot reach the level at all: its smallest p-value is `2 / 2^tasks`, so five
/// tasks can never reject at 5 % however large the difference.
///
/// ```
/// use majordomus_cli::economics::stats::{minimum_detectable_difference, NORMAL_MDE_METHOD};
/// // twenty tasks at the methodology's 95 %, power 80 %, the bound sd of a share difference
/// let m = minimum_detectable_difference(20, 9500, 8000, 1.0).unwrap();
/// assert!((m.difference - 0.6265).abs() < 1e-3, "{m:?}");
/// assert_eq!(m.method, NORMAL_MDE_METHOD);
/// // with a planning sd of 0.3 the same twenty tasks detect a smaller difference
/// assert!(minimum_detectable_difference(20, 9500, 8000, 0.3).unwrap().difference < 0.19);
/// assert!(minimum_detectable_difference(5, 9500, 8000, 1.0).is_none(), "2/32 > 0.05");
/// ```
pub fn minimum_detectable_difference(
    tasks: usize,
    level_bp: u32,
    power_bp: u32,
    sd: f64,
) -> Option<MinimumDetectable> {
    let alpha = significance(level_bp);
    let open = |bp: u32| (1..10_000).contains(&bp);
    // the exact test's smallest p-value is that of the observed signs and their mirror
    let reachable = tasks >= 64 || 2.0 / (1u64 << tasks) as f64 <= alpha;
    let valid = tasks >= 2 && open(level_bp) && open(power_bp) && sd >= 0.0 && sd.is_finite();
    if !(valid && reachable) {
        return None;
    }
    let z = normal_quantile(1.0 - alpha / 2.0) + normal_quantile(power_bp as f64 / 10_000.0);
    Some(MinimumDetectable {
        tasks,
        level_bp,
        power_bp,
        sd,
        difference: round(z * sd / (tasks as f64).sqrt()),
        method: NORMAL_MDE_METHOD.into(),
    })
}

/// The standard normal quantile, by Acklam's rational approximation (relative error below
/// `1.2e-9` over `(0, 1)`): enough for a minimum detectable difference, and the same on
/// every machine because it is a fixed sequence of arithmetic.
fn normal_quantile(p: f64) -> f64 {
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let tail = |q: f64| {
        (((((C[0] * q + C[1]) * q + C[2]) * q + C[3]) * q + C[4]) * q + C[5])
            / ((((D[0] * q + D[1]) * q + D[2]) * q + D[3]) * q + 1.0)
    };
    const LOW: f64 = 0.02425;
    if p < LOW {
        tail((-2.0 * p.ln()).sqrt())
    } else if p <= 1.0 - LOW {
        let q = p - 0.5;
        let r = q * q;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * q
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    } else {
        -tail((-2.0 * (1.0 - p).ln()).sqrt())
    }
}

/// Which direction of a paired difference (treatment minus control) is the better one: a
/// higher accepted share is better, lower wall time, tokens or spend are. Declared with the
/// outcome, never inferred from the data.
///
/// ```
/// use majordomus_cli::economics::model::EconomicsInterval;
/// use majordomus_cli::economics::stats::{Better, Verdict, CLUSTER_MEAN_METHOD};
/// assert_eq!(serde_json::to_value(Better::Lower).unwrap(), "lower");
/// let b: Better = serde_json::from_value("higher".into()).unwrap();
/// // the same interval of a difference is good news or bad news by the declared direction
/// let i = EconomicsInterval {
///     level_bp: 9500, low: 0.1, high: 0.3, method: CLUSTER_MEAN_METHOD.into(),
///     resamples: 2000, seed: 7,
/// };
/// assert_eq!(Verdict::of_interval(&i, b), Verdict::Favourable);
/// assert_eq!(Verdict::of_interval(&i, Better::Lower), Verdict::Unfavourable);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Better {
    /// A positive difference is better.
    Higher,
    /// A negative difference is better.
    Lower,
}

/// What a paired comparison says about the treatment, and the only words it may say it
/// with: the test or the interval decided for it, decided against it, or did not decide.
/// An underpowered comparison is `inconclusive`, never "no difference".
///
/// ```
/// use majordomus_cli::economics::stats::{paired_sign_test, Better, Verdict};
/// let tasks = |better: f64| -> Vec<(Vec<f64>, Vec<f64>)> {
///     (0..8).map(|_| (vec![0.5], vec![0.5 + better])).collect()
/// };
/// let up = paired_sign_test(&tasks(0.25), 9500, 0, 7, 5).unwrap();
/// let down = paired_sign_test(&tasks(-0.25), 9500, 0, 7, 5).unwrap();
/// assert!(up.rejects() && down.rejects());
/// assert_eq!(Verdict::of_test(&up, Better::Higher), Verdict::Favourable);
/// assert_eq!(Verdict::of_test(&down, Better::Higher), Verdict::Unfavourable);
/// assert_eq!(Verdict::of_test(&down, Better::Lower), Verdict::Favourable);
/// // half the tasks up, half down: the test does not reject, whatever the direction
/// let mixed: Vec<(Vec<f64>, Vec<f64>)> =
///     (0..8).map(|t| (vec![0.5], vec![if t % 2 == 0 { 0.75 } else { 0.25 }])).collect();
/// let none = paired_sign_test(&mixed, 9500, 0, 7, 5).unwrap();
/// assert!(!none.rejects());
/// assert_eq!(Verdict::of_test(&none, Better::Higher), Verdict::Inconclusive);
/// assert_eq!(serde_json::to_value(Verdict::Inconclusive).unwrap(), "inconclusive");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The difference is in the better direction, and the comparison decided it.
    Favourable,
    /// The difference is in the worse direction, and the comparison decided it.
    Unfavourable,
    /// The comparison did not decide: the test did not reject, or the interval holds zero.
    Inconclusive,
}

impl Verdict {
    /// The verdict of a test: its direction when it rejects, `Inconclusive` when it does not.
    ///
    /// ```
    /// use majordomus_cli::economics::stats::{paired_sign_test, Better, Verdict};
    /// // fewer accepted runs with the treatment on every one of seven tasks
    /// let worse: Vec<(Vec<f64>, Vec<f64>)> =
    ///     (0..7).map(|_| (vec![1.0, 1.0], vec![1.0, 0.0])).collect();
    /// let t = paired_sign_test(&worse, 9500, 0, 7, 5).unwrap();
    /// assert_eq!(Verdict::of_test(&t, Better::Higher), Verdict::Unfavourable);
    /// // the same direction over three tasks cannot reach 5 %: 2/8 is the smallest p-value
    /// let t3 = paired_sign_test(&worse[..3], 9500, 0, 7, 2).unwrap();
    /// assert_eq!(Verdict::of_test(&t3, Better::Higher), Verdict::Inconclusive);
    /// ```
    pub fn of_test(test: &PairedTest, better: Better) -> Verdict {
        if test.rejects() {
            Verdict::of_direction(test.difference, better)
        } else {
            Verdict::Inconclusive
        }
    }

    /// The verdict of an interval of a paired difference: its direction when it lies wholly
    /// on one side of zero, `Inconclusive` when it holds zero or is not a number.
    ///
    /// ```
    /// use majordomus_cli::economics::model::EconomicsInterval;
    /// use majordomus_cli::economics::stats::{Better, Verdict, CLUSTER_MEAN_METHOD};
    /// let i = EconomicsInterval {
    ///     level_bp: 9500, low: -80.0, high: -12.0, method: CLUSTER_MEAN_METHOD.into(),
    ///     resamples: 2000, seed: 7,
    /// };
    /// assert_eq!(Verdict::of_interval(&i, Better::Lower), Verdict::Favourable);
    /// assert_eq!(Verdict::of_interval(&i, Better::Higher), Verdict::Unfavourable);
    /// let zero = EconomicsInterval { low: -80.0, high: 0.0, ..i };
    /// assert_eq!(Verdict::of_interval(&zero, Better::Lower), Verdict::Inconclusive);
    /// ```
    pub fn of_interval(interval: &EconomicsInterval, better: Better) -> Verdict {
        if interval.low > 0.0 {
            Verdict::of_direction(1.0, better)
        } else if interval.high < 0.0 {
            Verdict::of_direction(-1.0, better)
        } else {
            Verdict::Inconclusive
        }
    }

    fn of_direction(difference: f64, better: Better) -> Verdict {
        match (difference > 0.0, difference < 0.0, better) {
            (true, _, Better::Higher) | (_, true, Better::Lower) => Verdict::Favourable,
            (true, _, Better::Lower) | (_, true, Better::Higher) => Verdict::Unfavourable,
            _ => Verdict::Inconclusive,
        }
    }
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

    fn shares(differences: &[f64]) -> Vec<(Vec<f64>, Vec<f64>)> {
        differences
            .iter()
            .map(|d| (vec![0.5], vec![0.5 + d]))
            .collect()
    }

    /// The p-value counted by hand: the differences 0.5, -0.25 and 0.25 have eight sign
    /// assignments whose sums are 1, 0.5, 0.5, 0, 0, -0.5, -0.5 and -1; six of them are at
    /// least as far from zero as the observed 0.5.
    #[test]
    fn the_exact_test_counts_every_assignment_at_least_as_extreme() {
        let t = paired_sign_test(&shares(&[0.5, -0.25, 0.25]), 9500, 0, 1, 2).unwrap();
        assert_eq!((t.tasks, t.assignments, t.exact), (3, 8, true));
        assert_eq!(t.p_value, 0.75);
        assert!((t.difference - 0.5 / 3.0).abs() < 1e-12);
        assert!(!t.rejects());
    }

    #[test]
    fn the_exact_test_takes_tasks_not_runs_as_its_sample() {
        // three tasks run four times each are three observations: 2/8 cannot reach 5 %
        let tasks: Vec<(Vec<f64>, Vec<f64>)> =
            (0..3).map(|_| (vec![0.0; 4], vec![1.0; 4])).collect();
        let t = paired_sign_test(&tasks, 9500, 0, 1, 2).unwrap();
        assert_eq!((t.tasks, t.p_value, t.difference), (3, 0.25, 1.0));
        assert_eq!(Verdict::of_test(&t, Better::Higher), Verdict::Inconclusive);
    }

    #[test]
    fn the_exact_test_drops_what_is_not_a_paired_task() {
        let tasks = vec![
            (vec![0.0, f64::NAN], vec![1.0]),
            (vec![], vec![1.0]),
            (vec![0.0], vec![f64::INFINITY]),
            (vec![0.0], vec![1.0]),
            (vec![1.0, 0.0], vec![1.0, 1.0]),
        ];
        let t = paired_sign_test(&tasks, 9500, 0, 1, 2).unwrap();
        assert_eq!(t.tasks, 3, "an empty or non-finite arm is no task");
        assert_eq!(t.difference, round((1.0 + 1.0 + 0.5) / 3.0));
        assert!(paired_sign_test(&tasks[..3], 9500, 0, 1, 2).is_none());
    }

    #[test]
    fn identical_arms_never_reject() {
        let t = paired_sign_test(&shares(&[0.0; 12]), 9500, 0, 1, 5).unwrap();
        assert_eq!((t.p_value, t.difference), (1.0, 0.0));
        assert_eq!(Verdict::of_test(&t, Better::Lower), Verdict::Inconclusive);
    }

    #[test]
    fn above_the_exact_limit_the_test_draws_from_its_seed() {
        let up = shares(&[0.25; EXACT_TASKS + 4]);
        assert!(
            paired_sign_test(&up, 9500, 0, 1, 5).is_none(),
            "no draws, no test"
        );
        let a = paired_sign_test(&up, 9500, 4000, 1, 5).unwrap();
        assert_eq!(a, paired_sign_test(&up, 9500, 4000, 1, 5).unwrap());
        assert!(!a.exact && a.seed == Some(1) && a.assignments == 4000);
        assert_eq!(a.method, SAMPLED_SIGN_FLIP_METHOD);
        assert!(a.p_value <= 2.0 / 4001.0, "{a:?}");
        assert_eq!(Verdict::of_test(&a, Better::Higher), Verdict::Favourable);
        let mixed: Vec<f64> = (0..EXACT_TASKS + 4)
            .map(|t| if t % 2 == 0 { 0.25 } else { -0.25 })
            .collect();
        let m = paired_sign_test(&shares(&mixed), 9500, 4000, 1, 5).unwrap();
        assert_eq!(
            m.p_value, 1.0,
            "every assignment is as far from zero as zero"
        );
        // seventy tasks need two words of signs; the draws still decide
        let many = paired_sign_test(&shares(&[-0.5; 70]), 9500, 500, 9, 5).unwrap();
        assert_eq!(Verdict::of_test(&many, Better::Lower), Verdict::Favourable);
    }

    #[test]
    fn the_normal_quantile_matches_the_tables() {
        for (p, z) in [
            (0.5, 0.0),
            (0.8, 0.841_621_233_6),
            (0.975, 1.959_963_985),
            (0.995, 2.575_829_304),
            (0.01, -2.326_347_874),
        ] {
            assert!((normal_quantile(p) - z).abs() < 1e-8, "z({p})");
            assert!((normal_quantile(1.0 - p) + z).abs() < 1e-8, "z(1 - {p})");
        }
    }

    #[test]
    fn the_minimum_detectable_difference_refuses_what_it_cannot_promise() {
        assert!(minimum_detectable_difference(1, 9500, 8000, 1.0).is_none());
        assert!(minimum_detectable_difference(20, 0, 8000, 1.0).is_none());
        assert!(minimum_detectable_difference(20, 9500, 10_000, 1.0).is_none());
        assert!(minimum_detectable_difference(20, 9500, 8000, f64::NAN).is_none());
        assert!(minimum_detectable_difference(20, 9500, 8000, -0.1).is_none());
        assert!(
            minimum_detectable_difference(6, 9500, 8000, 1.0).is_some(),
            "2/64 <= 5 %"
        );
        assert!(minimum_detectable_difference(100, 9500, 8000, 1.0).is_some());
        let (a, b) = (
            minimum_detectable_difference(20, 9500, 8000, 0.5).unwrap(),
            minimum_detectable_difference(80, 9500, 8000, 0.5).unwrap(),
        );
        assert!(
            (a.difference / b.difference - 2.0).abs() < 1e-9,
            "four times the tasks halve it"
        );
    }

    #[test]
    fn an_interval_decides_only_when_it_excludes_zero() {
        let i = |low: f64, high: f64| EconomicsInterval {
            level_bp: 9500,
            low,
            high,
            method: CLUSTER_MEAN_METHOD.into(),
            resamples: 1,
            seed: 1,
        };
        assert_eq!(
            Verdict::of_interval(&i(0.1, 0.4), Better::Higher),
            Verdict::Favourable
        );
        assert_eq!(
            Verdict::of_interval(&i(0.1, 0.4), Better::Lower),
            Verdict::Unfavourable
        );
        assert_eq!(
            Verdict::of_interval(&i(-0.4, -0.1), Better::Lower),
            Verdict::Favourable
        );
        assert_eq!(
            Verdict::of_interval(&i(0.0, 0.4), Better::Higher),
            Verdict::Inconclusive
        );
        assert_eq!(
            Verdict::of_interval(&i(-0.4, 0.0), Better::Lower),
            Verdict::Inconclusive
        );
        assert_eq!(
            Verdict::of_interval(&i(f64::NAN, f64::NAN), Better::Higher),
            Verdict::Inconclusive
        );
    }

    #[test]
    fn the_mean_interval_follows_a_shift_of_every_difference() {
        let c: Vec<Vec<f64>> = (0..8)
            .map(|t| (0..3).map(|r| (t as f64 * 0.37 + r as f64).sin()).collect())
            .collect();
        let shifted: Vec<Vec<f64>> = c
            .iter()
            .map(|v| v.iter().map(|x| x + 10.0).collect())
            .collect();
        let a = bootstrap_mean_difference_clustered(&c, 9500, 1000, 3, 5).unwrap();
        let b = bootstrap_mean_difference_clustered(&shifted, 9500, 1000, 3, 5).unwrap();
        assert!((b.low - a.low - 10.0).abs() < 1e-9 && (b.high - a.high - 10.0).abs() < 1e-9);
        assert!(bootstrap_mean_difference_clustered(&c, 9500, 0, 3, 5).is_none());
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
            fn task_order_never_changes_the_paired_test(
                tasks in prop::collection::vec(
                    (prop::collection::vec(0u8..2, 1..4), prop::collection::vec(0u8..2, 1..4)),
                    2..12,
                ),
                rotate in 0usize..12,
            ) {
                let as_shares = |t: &[(Vec<u8>, Vec<u8>)]| -> Vec<(Vec<f64>, Vec<f64>)> {
                    t.iter()
                        .map(|(c, x)| (
                            c.iter().map(|&v| v as f64).collect(),
                            x.iter().map(|&v| v as f64).collect(),
                        ))
                        .collect()
                };
                let a = paired_sign_test(&as_shares(&tasks), 9500, 0, 1, 2);
                let mut shuffled: Vec<(Vec<u8>, Vec<u8>)> = tasks
                    .iter()
                    .map(|(c, x)| { let (mut c, mut x) = (c.clone(), x.clone()); c.reverse(); x.reverse(); (c, x) })
                    .collect();
                shuffled.reverse();
                let r = rotate % shuffled.len();
                shuffled.rotate_left(r);
                prop_assert_eq!(&a, &paired_sign_test(&as_shares(&shuffled), 9500, 0, 1, 2));
                let t = a.unwrap();
                prop_assert!(0.0 < t.p_value && t.p_value <= 1.0);
                prop_assert!(-1.0 <= t.difference && t.difference <= 1.0);
            }

            #[test]
            fn cluster_order_never_changes_the_mean_interval(
                clusters in prop::collection::vec(prop::collection::vec(-3.0f64..1.0, 1..6), 2..10),
                seed in 0u64..1000,
                rotate in 0usize..10,
            ) {
                let a = bootstrap_mean_difference_clustered(&clusters, 9500, 200, seed, 5);
                let mut shuffled: Vec<Vec<f64>> = clusters
                    .iter()
                    .map(|c| { let mut c = c.clone(); c.reverse(); c })
                    .collect();
                shuffled.reverse();
                let r = rotate % shuffled.len();
                shuffled.rotate_left(r);
                prop_assert_eq!(&a, &bootstrap_mean_difference_clustered(&shuffled, 9500, 200, seed, 5));
                if let Some(i) = a {
                    let all: Vec<f64> = clusters.iter().flatten().copied().collect();
                    let d = distribution(&all).unwrap();
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
