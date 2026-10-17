//! Latency statistics of one target: the samples reduced to the numbers a baseline is
//! compared on. Durations are nanoseconds on a monotonic clock, reported in
//! microseconds.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The statistics of one set of samples, in microseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Statistics {
    /// How many samples.
    pub samples: usize,
    /// The smallest.
    pub min_us: f64,
    /// The median.
    pub p50_us: f64,
    /// The 90th percentile.
    pub p90_us: f64,
    /// The 95th percentile.
    pub p95_us: f64,
    /// The 99th percentile.
    pub p99_us: f64,
    /// The largest.
    pub max_us: f64,
    /// The mean.
    pub mean_us: f64,
    /// The population standard deviation.
    pub stddev_us: f64,
}

impl Statistics {
    /// Reduce samples. An empty set is all zeros with `samples: 0`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::bench::Statistics;
    /// let s = Statistics::of(&(1..=100).map(Duration::from_micros).collect::<Vec<_>>());
    /// assert_eq!(s.samples, 100);
    /// assert_eq!(s.min_us, 1.0);
    /// assert_eq!(s.max_us, 100.0);
    /// assert_eq!(s.p50_us, 50.0);
    /// assert_eq!(s.p99_us, 99.0);
    /// assert!((s.mean_us - 50.5).abs() < 1e-9);
    /// ```
    pub fn of(samples: &[Duration]) -> Self {
        if samples.is_empty() {
            return Statistics {
                samples: 0,
                min_us: 0.0,
                p50_us: 0.0,
                p90_us: 0.0,
                p95_us: 0.0,
                p99_us: 0.0,
                max_us: 0.0,
                mean_us: 0.0,
                stddev_us: 0.0,
            };
        }
        let mut us: Vec<f64> = samples
            .iter()
            .map(|d| d.as_nanos() as f64 / 1000.0)
            .collect();
        // Ascending by value, and deliberately not the canonical order: the percentiles
        // below index into this vector by rank, so the sequence is the statistic's own
        // definition and nothing renders it.
        us.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = us.len();
        let pct = |p: f64| -> f64 {
            // nearest-rank: the smallest value below which p percent of the samples fall
            let rank = ((p / 100.0) * n as f64).ceil() as usize;
            us[rank.clamp(1, n) - 1]
        };
        let mean = us.iter().sum::<f64>() / n as f64;
        let var = us.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / n as f64;
        Statistics {
            samples: n,
            min_us: us[0],
            p50_us: pct(50.0),
            p90_us: pct(90.0),
            p95_us: pct(95.0),
            p99_us: pct(99.0),
            max_us: us[n - 1],
            mean_us: mean,
            stddev_us: var.sqrt(),
        }
    }

    /// How many samples are certainly at or above a metric's value: the samples a verdict
    /// on that metric rests on. A nearest-rank percentile of rank `r` has `n - r + 1` samples
    /// at or above it; the maximum has one, the minimum all of them; the mean is an
    /// aggregate of every sample and one outlier can carry it, so it counts as one. `None`
    /// for a metric this does not know.
    ///
    /// A metric over its threshold is a regression only when at least two samples agree:
    /// the median of one sample, or the p99 of twenty, is the single slowest sample, which
    /// one scheduler hiccup on a loaded runner decides.
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::bench::Statistics;
    /// let one = Statistics::of(&[Duration::from_millis(23)]);
    /// assert_eq!(one.samples_at_or_above("p50"), Some(1));
    /// let five = Statistics::of(&vec![Duration::from_millis(9); 5]);
    /// assert_eq!(five.samples_at_or_above("p50"), Some(3));
    /// assert_eq!(five.samples_at_or_above("p99"), Some(1));
    /// assert_eq!(five.samples_at_or_above("min"), Some(5));
    /// assert_eq!(five.samples_at_or_above("mean"), Some(1));
    /// assert_eq!(five.samples_at_or_above("p42"), None);
    /// ```
    pub fn samples_at_or_above(&self, metric: &str) -> Option<usize> {
        let n = self.samples;
        if n == 0 {
            return self.metric(metric).map(|_| 0);
        }
        let above_rank = |p: f64| {
            let rank = ((p / 100.0) * n as f64).ceil() as usize;
            n - rank.clamp(1, n) + 1
        };
        Some(match metric {
            "min" => n,
            "p50" => above_rank(50.0),
            "p90" => above_rank(90.0),
            "p95" => above_rank(95.0),
            "p99" => above_rank(99.0),
            "max" | "mean" => 1,
            _ => return None,
        })
    }

    /// One metric by name (`p50`, `p95`, `p99`, `mean`, `max`, `min`, `p90`).
    pub fn metric(&self, name: &str) -> Option<f64> {
        Some(match name {
            "min" => self.min_us,
            "p50" => self.p50_us,
            "p90" => self.p90_us,
            "p95" => self.p95_us,
            "p99" => self.p99_us,
            "max" => self.max_us,
            "mean" => self.mean_us,
            _ => return None,
        })
    }
}
