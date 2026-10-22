//! The arithmetic of the economics subsystem, and nothing else: the reduction formula,
//! order statistics, a seeded bootstrap (over independent values, or over clusters when
//! the values are repetitions of the same task), the paired comparison of two arms (an
//! exact task-level test for a binary outcome, a task-clustered interval for a continuous
//! one, and the one [`Verdict`] either reaches), and the two ways a number is printed.
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

/// What [`bootstrap_mean_difference_clustered`] writes into [`EconomicsInterval::method`].
pub const PAIRED_MEAN_CLUSTER_METHOD: &str = "percentile bootstrap of the mean paired difference, cluster bootstrap over tasks: each resample draws tasks with replacement and pools every paired difference of each drawn task";

/// What [`sign_flip_test`] writes into [`PairedTest::method`] when it enumerated every
/// assignment of signs.
pub const SIGN_FLIP_EXACT_METHOD: &str = "exact sign-flip permutation test over tasks of the mean per-task difference in accepted share, two-sided, every assignment of signs enumerated";

/// What [`sign_flip_test`] writes into [`PairedTest::method`] when there were too many tasks
/// to enumerate and the signs were drawn instead.
pub const SIGN_FLIP_SAMPLED_METHOD: &str = "sign-flip permutation test over tasks of the mean per-task difference in accepted share, two-sided, seeded random assignments of signs, p = (1 + as extreme) / (1 + resamples)";

/// The most tasks with a non-zero difference [`sign_flip_test`] enumerates exhaustively:
/// 2^20 assignments. Above it the assignments are drawn, and the method says so.
pub const SIGN_FLIP_EXACT_MAX: usize = 20;

/// Which way a paired difference (treatment minus control) is better: a higher accepted
/// share, or a lower cost. Declared by whoever states the outcome, never inferred from the
/// data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Better {
    /// A positive difference favours the treatment.
    Higher,
    /// A negative difference favours the treatment.
    Lower,
}

/// The one verdict a paired comparison reaches, whichever calculation decided it.
///
/// `Inconclusive` is a result and not a missing one: it is what an evidence base too small
/// to tell the arms apart must say, and nothing is allowed to round it into a direction.
///
/// ```
/// use majordomus_cli::economics::stats::{Better, Verdict};
/// assert_eq!(Verdict::of(true, 0.3, Better::Higher), Verdict::Favourable);
/// assert_eq!(Verdict::of(true, 0.3, Better::Lower), Verdict::Unfavourable);
/// assert_eq!(Verdict::of(false, 0.3, Better::Higher), Verdict::Inconclusive);
/// assert_eq!(Verdict::of(true, 0.0, Better::Higher), Verdict::Inconclusive);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The difference is decided, and it goes the declared better way.
    Favourable,
    /// The difference is decided, and it goes the other way.
    Unfavourable,
    /// The evidence does not decide the direction.
    Inconclusive,
}

impl Verdict {
    /// The verdict of a difference that a calculation did or did not `decide`: a decided
    /// difference is favourable or unfavourable by its sign against `better`; an undecided
    /// one, or one of exactly zero, is inconclusive.
    pub fn of(decided: bool, difference: f64, better: Better) -> Verdict {
        if !decided || !difference.is_finite() || difference == 0.0 {
            return Verdict::Inconclusive;
        }
        match (difference > 0.0, better) {
            (true, Better::Higher) | (false, Better::Lower) => Verdict::Favourable,
            _ => Verdict::Unfavourable,
        }
    }

    /// The verdict of [`sign_flip_test`]: a test that does not reject is inconclusive, one
    /// that rejects is favourable or unfavourable by the direction of its mean difference.
    ///
    /// ```
    /// use majordomus_cli::economics::stats::{sign_flip_test, Better, Verdict};
    /// // eight tasks, each one paired run: the treatment accepted where the control did not
    /// // on every task. 2 / 2^8 is below 0.05, so the test rejects.
    /// let all = vec![vec![(false, true)]; 8];
    /// let t = sign_flip_test(&all, 9500, 0, 1, 2).unwrap();
    /// assert!(t.rejected);
    /// assert_eq!(Verdict::of_test(&t, Better::Higher), Verdict::Favourable);
    /// assert_eq!(Verdict::of_test(&t, Better::Lower), Verdict::Unfavourable);
    /// // four tasks for the treatment and three against it decide nothing
    /// let mixed: Vec<Vec<(bool, bool)>> = (0..7).map(|i| vec![(i >= 4, i < 4)]).collect();
    /// let t = sign_flip_test(&mixed, 9500, 0, 1, 2).unwrap();
    /// assert!(!t.rejected);
    /// assert_eq!(Verdict::of_test(&t, Better::Higher), Verdict::Inconclusive);
    /// ```
    pub fn of_test(test: &PairedTest, better: Better) -> Verdict {
        Verdict::of(test.rejected, test.mean_difference, better)
    }

    /// The verdict of an interval of a paired difference: one that lies wholly above or
    /// wholly below zero decides the direction, one that contains zero does not.
    ///
    /// ```
    /// use majordomus_cli::economics::stats::{bootstrap_mean_difference_clustered, Better, Verdict};
    /// let cheaper = vec![vec![-0.2, -0.25], vec![-0.3], vec![-0.1, -0.15], vec![-0.22]];
    /// let i = bootstrap_mean_difference_clustered(&cheaper, 9500, 2000, 7, 4).unwrap();
    /// assert_eq!(Verdict::of_interval(&i, Better::Lower), Verdict::Favourable);
    /// let either = vec![vec![-0.2], vec![0.3], vec![-0.1], vec![0.15]];
    /// let i = bootstrap_mean_difference_clustered(&either, 9500, 2000, 7, 4).unwrap();
    /// assert_eq!(Verdict::of_interval(&i, Better::Lower), Verdict::Inconclusive);
    /// ```
    pub fn of_interval(interval: &EconomicsInterval, better: Better) -> Verdict {
        if interval.low > 0.0 {
            Verdict::of(true, interval.low, better)
        } else if interval.high < 0.0 {
            Verdict::of(true, interval.high, better)
        } else {
            Verdict::Inconclusive
        }
    }
}

/// What [`sign_flip_test`] found: the mean per-task difference, its two-sided p-value,
/// whether that rejects at the level, and how it was computed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PairedTest {
    /// Tasks with at least one paired run: the clusters the test permutes over.
    pub tasks: usize,
    /// Paired runs across those tasks.
    pub pairs: usize,
    /// The mean over tasks of each task's accepted share under treatment minus under control.
    pub mean_difference: f64,
    /// The two-sided p-value of that mean under the null that each task's difference is as
    /// likely negative as positive.
    pub p_value: f64,
    /// The confidence level in basis points; the test rejects at `1 - level`.
    pub level_bp: u32,
    /// Whether `p_value` is at or below `1 - level`.
    pub rejected: bool,
    /// Whether every assignment of signs was enumerated, rather than drawn.
    pub exact: bool,
    /// How many assignments were drawn; 0 when `exact`.
    pub resamples: u32,
    /// The seed the drawn assignments came from.
    pub seed: u64,
    /// Which calculation this is, in words.
    pub method: String,
}

/// An exact task-level test of a binary paired outcome: each task contributes the mean
/// difference in accepted share between its treatment and control runs (`(control,
/// treatment)` per paired run, `true` for accepted), and the statistic is the mean of those
/// per-task differences. Under the null each task's difference is as likely to have the
/// other sign, so the p-value is the share of the 2^k assignments of signs whose mean is at
/// least as far from zero as the observed one, two-sided.
///
/// The task is the unit, not the run: repetitions of one task share its fixture and its
/// difficulty, and a percentile bootstrap over twenty clusters of mostly-zero differences
/// covers less than it claims. A task whose difference is zero is the same under every
/// sign, so only the others are enumerated; with more than [`SIGN_FLIP_EXACT_MAX`] of them,
/// `resamples` seeded assignments are drawn instead and the method says so.
///
/// Deterministic for a seed and independent of the order of tasks and of runs within a
/// task. `None` below `min_n` paired runs in total, or with fewer than two tasks.
///
/// ```
/// use majordomus_cli::economics::stats::{sign_flip_test, SIGN_FLIP_EXACT_METHOD};
/// let tasks = vec![
///     vec![(false, true), (false, true)],
///     vec![(true, true)],
///     vec![(false, true)],
///     vec![(true, false), (false, false)],
/// ];
/// let t = sign_flip_test(&tasks, 9500, 0, 1, 2).unwrap();
/// let reversed: Vec<Vec<(bool, bool)>> =
///     tasks.iter().rev().map(|v| v.iter().rev().copied().collect()).collect();
/// assert_eq!(t, sign_flip_test(&reversed, 9500, 0, 1, 2).unwrap(), "order does not matter");
/// assert_eq!((t.tasks, t.pairs, t.exact), (4, 6, true));
/// assert_eq!(t.method, SIGN_FLIP_EXACT_METHOD);
/// assert!(sign_flip_test(&tasks[..1], 9500, 0, 1, 1).is_none(), "one task is no comparison");
/// assert!(sign_flip_test(&tasks, 9500, 0, 1, 7).is_none(), "below min_n");
/// ```
pub fn sign_flip_test(
    tasks: &[Vec<(bool, bool)>],
    level_bp: u32,
    resamples: u32,
    seed: u64,
    min_n: usize,
) -> Option<PairedTest> {
    let clusters: Vec<&Vec<(bool, bool)>> = tasks.iter().filter(|t| !t.is_empty()).collect();
    let pairs: usize = clusters.iter().map(|t| t.len()).sum();
    let k = clusters.len();
    if pairs < min_n.max(2) || k < 2 {
        return None;
    }
    let mut diffs: Vec<f64> = clusters
        .iter()
        .map(|t| {
            let share = |pick: fn(&(bool, bool)) -> bool| {
                t.iter().filter(|p| pick(p)).count() as f64 / t.len() as f64
            };
            share(|p| p.1) - share(|p| p.0)
        })
        .collect();
    crate::order::numeric(&mut diffs);
    let mean = diffs.iter().sum::<f64>() / k as f64;
    let moving: Vec<f64> = diffs.iter().copied().filter(|d| *d != 0.0).collect();
    let observed = moving.iter().sum::<f64>().abs();
    // the same sum reached by another route differs in its last bits; it is as extreme
    let as_extreme = |sum: f64| sum.abs() >= observed - 1e-9 * observed.max(1.0);
    let (p_value, exact, drawn) = if moving.len() <= SIGN_FLIP_EXACT_MAX {
        // every assignment, in Gray-code order: one sign changes per step
        let m = moving.len();
        let mut signs = vec![1.0f64; m];
        let mut sum: f64 = moving.iter().sum();
        let mut extreme: u64 = u64::from(as_extreme(sum));
        for step in 1u64..(1u64 << m) {
            let i = step.trailing_zeros() as usize;
            sum -= 2.0 * signs[i] * moving[i];
            signs[i] = -signs[i];
            extreme += u64::from(as_extreme(sum));
        }
        (extreme as f64 / (1u64 << m) as f64, true, 0)
    } else {
        if resamples == 0 {
            return None;
        }
        let mut rng = SplitMix64(seed);
        let mut extreme: u64 = 0;
        for _ in 0..resamples {
            let sum: f64 = moving
                .iter()
                .map(|d| if rng.next() & 1 == 0 { *d } else { -*d })
                .sum();
            extreme += u64::from(as_extreme(sum));
        }
        (
            (1 + extreme) as f64 / (1 + u64::from(resamples)) as f64,
            false,
            resamples,
        )
    };
    let p_value = round(p_value.min(1.0));
    Some(PairedTest {
        tasks: k,
        pairs,
        mean_difference: round(mean),
        p_value,
        level_bp,
        rejected: rejects(p_value, level_bp),
        exact,
        resamples: drawn,
        seed,
        method: if exact {
            SIGN_FLIP_EXACT_METHOD
        } else {
            SIGN_FLIP_SAMPLED_METHOD
        }
        .into(),
    })
}

/// Whether a two-sided p-value rejects at `level_bp`: at or below `1 - level`, compared in
/// basis points so that `0.05` against `9500` is not decided by the last bit of a float.
fn rejects(p_value: f64, level_bp: u32) -> bool {
    let alpha_bp = 10_000u32.saturating_sub(level_bp.min(10_000)) as f64;
    p_value * 10_000.0 <= alpha_bp + 1e-9
}

/// The smallest mean per-task difference in accepted share that [`sign_flip_test`] can
/// declare at all with `tasks` tasks of one paired run each, at `level_bp`; `None` when no
/// outcome of that many tasks could reject.
///
/// With one run per arm a task's difference is -1, 0 or +1, and the most extreme outcome of
/// `d` deciding tasks — all `d` the same way — has the two-sided p-value 2 / 2^d. So the
/// floor is the smallest `d` with 2 / 2^d at or below `1 - level`, over `tasks`: on fewer
/// agreeing tasks every result is inconclusive, whatever the data. More deciding tasks in
/// both directions never need fewer net agreeing ones (`the_floor_is_the_unanimous_one`,
/// below, checks every outcome). It is a floor, not a power calculation: it says what the
/// sample cannot show, not how likely it is to show what is there.
///
/// ```
/// use majordomus_cli::economics::stats::minimum_detectable_difference;
/// // the methodology's 95 %: six of twenty tasks must agree, net, before anything is decided
/// assert_eq!(minimum_detectable_difference(20, 9500), Some(0.3));
/// assert_eq!(minimum_detectable_difference(5, 9500), None, "2 / 2^5 is above 0.05");
/// ```
pub fn minimum_detectable_difference(tasks: usize, level_bp: u32) -> Option<f64> {
    let d = (1..=tasks.min(1024)).find(|d| rejects(2.0 / 2f64.powi(*d as i32), level_bp))?;
    Some(round(d as f64 / tasks as f64))
}

/// A percentile bootstrap interval of the mean paired difference over tasks: each value is
/// one paired run's difference (treatment minus control), grouped by task; each resample
/// draws as many tasks as there are, with replacement, pools every difference of each drawn
/// task and takes the mean of the pool. For a continuous outcome, where the per-task
/// differences are not mostly zero and the bootstrap's percentile covers what it claims; a
/// binary one goes to [`sign_flip_test`].
///
/// Deterministic for a seed and independent of order, exactly as
/// [`bootstrap_median_clustered`] is: non-finite values dropped, each task sorted, empty
/// tasks dropped, tasks sorted before any draw. `None` below `min_n` values in total, with
/// fewer than two tasks, or with no resamples.
///
/// ```
/// use majordomus_cli::economics::stats::{bootstrap_mean_difference_clustered, PAIRED_MEAN_CLUSTER_METHOD};
/// let tasks = vec![vec![0.1, 0.12], vec![0.3, f64::NAN], vec![0.2, 0.22]];
/// let a = bootstrap_mean_difference_clustered(&tasks, 9500, 2000, 7, 4).unwrap();
/// let shuffled = vec![vec![0.22, 0.2], vec![0.3], vec![0.12, 0.1]];
/// assert_eq!(a, bootstrap_mean_difference_clustered(&shuffled, 9500, 2000, 7, 4).unwrap());
/// assert!(a.low > 0.0 && a.low <= a.high);
/// assert_eq!(a.method, PAIRED_MEAN_CLUSTER_METHOD);
/// assert!(bootstrap_mean_difference_clustered(&[vec![0.1, 0.2, 0.3, 0.4]], 9500, 2000, 7, 4).is_none());
/// ```
pub fn bootstrap_mean_difference_clustered(
    tasks: &[Vec<f64>],
    level_bp: u32,
    resamples: u32,
    seed: u64,
    min_n: usize,
) -> Option<EconomicsInterval> {
    let mut c: Vec<Vec<f64>> = tasks
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
    let sums: Vec<(f64, usize)> = c.iter().map(|v| (v.iter().sum(), v.len())).collect();
    let mut rng = SplitMix64(seed);
    let mut means = Vec::with_capacity(resamples as usize);
    for _ in 0..resamples {
        let (mut total, mut count) = (0.0, 0usize);
        for _ in 0..k {
            let (s, len) = sums[rng.below(k as u64) as usize];
            total += s;
            count += len;
        }
        means.push(total / count as f64);
    }
    percentile_interval(means, level_bp, resamples, seed, PAIRED_MEAN_CLUSTER_METHOD)
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

    /// The two-sided p-value of `net` among `m` tasks whose differences are each -1 or +1:
    /// the share of the 2^m sign assignments whose sum is at least as far from zero.
    fn sign_test_p(m: u32, net: i64) -> f64 {
        let choose =
            |n: u32, k: u32| (0..k).fold(1u128, |c, i| c * u128::from(n - i) / u128::from(i + 1));
        let count: u128 = (0..=m)
            .filter(|j| (2 * i64::from(*j) - i64::from(m)).abs() >= net.abs())
            .map(|j| choose(m, j))
            .sum();
        count as f64 / (1u128 << m) as f64
    }

    /// `up` tasks where only the treatment was accepted, `down` where only the control was,
    /// `same` where both arms agreed: one paired run each.
    fn binary_tasks(up: usize, down: usize, same: usize) -> Vec<Vec<(bool, bool)>> {
        let mut t = vec![vec![(false, true)]; up];
        t.extend(vec![vec![(true, false)]; down]);
        t.extend((0..same).map(|i| vec![(i % 2 == 0, i % 2 == 0)]));
        t
    }

    #[test]
    fn test_exact_paired_test() {
        // six of twenty tasks agree and the rest tie: 2 / 2^6 rejects at 95 %, five do not
        let six = sign_flip_test(&binary_tasks(6, 0, 14), 9500, 0, 1, 2).unwrap();
        assert!(six.exact && six.rejected, "{six:?}");
        assert_eq!((six.p_value, six.mean_difference), (2.0 / 64.0, 0.3));
        let five = sign_flip_test(&binary_tasks(5, 0, 15), 9500, 0, 1, 2).unwrap();
        assert!(!five.rejected && five.p_value == 2.0 / 32.0, "{five:?}");
        assert_eq!(
            Verdict::of_test(&five, Better::Higher),
            Verdict::Inconclusive
        );
        // the floor and the test agree: twenty tasks need six net
        assert_eq!(
            minimum_detectable_difference(20, 9500),
            Some(six.mean_difference)
        );
        // every split of up to twelve deciding tasks is the binomial sign test, exactly
        for m in 1..=12u32 {
            for up in 0..=m {
                let t = sign_flip_test(
                    &binary_tasks(up as usize, (m - up) as usize, 3),
                    9500,
                    0,
                    1,
                    2,
                )
                .unwrap();
                let net = i64::from(up) - i64::from(m - up);
                assert_eq!(t.p_value, round(sign_test_p(m, net)), "m={m} up={up}");
                assert_eq!(
                    t.rejected,
                    sign_test_p(m, net) <= 0.05 + 1e-12,
                    "m={m} up={up}"
                );
            }
        }
        // fractional differences from repeated runs: the per-task mean, not the pooled runs
        let repeated = vec![
            vec![(false, true), (false, false)],
            vec![(false, true), (true, true)],
            vec![(true, false), (true, true)],
        ];
        let t = sign_flip_test(&repeated, 9500, 0, 1, 2).unwrap();
        assert_eq!(
            (t.tasks, t.pairs, t.mean_difference),
            (3, 6, round(0.5 / 3.0))
        );
        // past the enumeration bound the signs are drawn, reproducibly, near the exact value
        let many = binary_tasks(21, 9, 4);
        let a = sign_flip_test(&many, 9500, 20_000, 3, 2).unwrap();
        assert_eq!(a, sign_flip_test(&many, 9500, 20_000, 3, 2).unwrap());
        assert!(!a.exact && a.method == SIGN_FLIP_SAMPLED_METHOD && a.resamples == 20_000);
        assert!(
            (a.p_value - sign_test_p(30, 12)).abs() < 0.01,
            "{} vs {}",
            a.p_value,
            sign_test_p(30, 12)
        );
        assert!(
            sign_flip_test(&many, 9500, 0, 3, 2).is_none(),
            "nothing to draw with"
        );
        // nothing below two tasks, or below min_n runs
        assert!(sign_flip_test(&binary_tasks(1, 0, 0), 9500, 0, 1, 1).is_none());
        assert!(sign_flip_test(&[vec![], vec![(false, true)]], 9500, 0, 1, 1).is_none());
    }

    #[test]
    fn the_floor_is_the_unanimous_one() {
        for level_bp in [9000, 9500, 9750, 9900] {
            let alpha = 1.0 - f64::from(level_bp) / 10_000.0;
            for tasks in 1..=30u32 {
                let least_net = (1..=tasks)
                    .flat_map(|m| (0..=m).map(move |up| (m, 2 * i64::from(up) - i64::from(m))))
                    .filter(|(m, net)| sign_test_p(*m, *net) <= alpha + 1e-12)
                    .map(|(_, net)| net.unsigned_abs())
                    .min();
                assert_eq!(
                    minimum_detectable_difference(tasks as usize, level_bp),
                    least_net.map(|n| round(n as f64 / f64::from(tasks))),
                    "tasks={tasks} level={level_bp}"
                );
            }
        }
    }

    #[test]
    fn test_interval_verdict() {
        let cheaper = vec![
            vec![-0.2, -0.25],
            vec![-0.3],
            vec![-0.1, -0.15],
            vec![-0.22],
        ];
        let i = bootstrap_mean_difference_clustered(&cheaper, 9500, 2000, 7, 4).unwrap();
        assert!(i.high < 0.0, "{i:?}");
        assert_eq!(Verdict::of_interval(&i, Better::Lower), Verdict::Favourable);
        assert_eq!(
            Verdict::of_interval(&i, Better::Higher),
            Verdict::Unfavourable
        );
        let either = vec![vec![-0.2], vec![0.3], vec![-0.1], vec![0.15]];
        let j = bootstrap_mean_difference_clustered(&either, 9500, 2000, 7, 4).unwrap();
        assert!(j.low < 0.0 && 0.0 < j.high, "{j:?}");
        assert_eq!(
            Verdict::of_interval(&j, Better::Lower),
            Verdict::Inconclusive
        );
        // reproducible from its seed, and a seed is part of the answer
        assert_eq!(
            i,
            bootstrap_mean_difference_clustered(&cheaper, 9500, 2000, 7, 4).unwrap()
        );
        assert_eq!(
            bootstrap_mean_difference_clustered(&cheaper, 9500, 2000, 8, 4)
                .unwrap()
                .seed,
            8
        );
        // tasks that differ widen it past the interval of the same values as one per task
        let spread = vec![
            vec![0.0, 0.0, 0.0],
            vec![0.6, 0.6, 0.6],
            vec![0.3, 0.3, 0.3],
        ];
        let pooled: Vec<Vec<f64>> = spread.concat().into_iter().map(|v| vec![v]).collect();
        let by_task = bootstrap_mean_difference_clustered(&spread, 9500, 4000, 7, 4).unwrap();
        let by_run = bootstrap_mean_difference_clustered(&pooled, 9500, 4000, 7, 4).unwrap();
        assert!(
            by_task.high - by_task.low > by_run.high - by_run.low,
            "{by_task:?} {by_run:?}"
        );
        // fewer than two tasks, too few values, or no resamples: no interval
        assert!(
            bootstrap_mean_difference_clustered(&[vec![0.1, 0.2, 0.3, 0.4]], 9500, 2000, 7, 4)
                .is_none()
        );
        assert!(
            bootstrap_mean_difference_clustered(&[vec![0.1], vec![0.2]], 9500, 2000, 7, 4)
                .is_none()
        );
        assert!(bootstrap_mean_difference_clustered(&cheaper, 9500, 0, 7, 4).is_none());
        // a verdict is serialized the way a surface will name it
        assert_eq!(
            serde_json::to_value(Verdict::Inconclusive).unwrap(),
            "inconclusive"
        );
        assert_eq!(serde_json::to_value(Better::Lower).unwrap(), "lower");
    }

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
