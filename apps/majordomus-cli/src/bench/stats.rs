//! Latency statistics of one target: the samples reduced to the numbers a baseline is
//! compared on. Durations are nanoseconds on a monotonic clock, reported in
//! microseconds.

use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The statistics of one set of samples, in microseconds.
///
/// A reduction of whatever samples it was given and no measurement of its own: the numbers
/// mean something only together with the run's provenance, which is why a result document
/// carries both. `samples` is part of the value for that reason — a percentile of twenty
/// samples is one scheduler hiccup, and the regression policy refuses to gate on it.
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::bench::Statistics;
/// let reduced = Statistics::of(&[
///     Duration::from_micros(10),
///     Duration::from_micros(30),
///     Duration::from_micros(20),
/// ]);
/// // order does not matter: the samples are sorted before anything is read off them
/// assert_eq!(reduced.samples, 3);
/// assert_eq!((reduced.min_us, reduced.max_us), (10.0, 30.0));
/// assert_eq!(reduced.p50_us, 20.0);
/// // an empty set is not a fast one: it is a set with nothing in it
/// let nothing = Statistics::of(&[]);
/// assert_eq!(nothing.samples, 0);
/// assert_eq!(nothing.p99_us, 0.0);
/// ```
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

    /// One metric by name (`p50`, `p95`, `p99`, `mean`, `max`, `min`, `p90`).
    ///
    /// The regression policy is data — it names its metrics as strings in a YAML file — so
    /// something has to turn a name into a number. `None` for a name this type does not
    /// have is what makes a policy naming an unknown metric skip that comparison instead of
    /// failing a run on a typo.
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::bench::Statistics;
    /// let stats = Statistics::of(&(1..=100).map(Duration::from_micros).collect::<Vec<_>>());
    /// assert_eq!(stats.metric("p50"), Some(stats.p50_us));
    /// assert_eq!(stats.metric("max"), Some(stats.max_us));
    /// // a metric nobody defined is not silently something else
    /// assert_eq!(stats.metric("p999"), None);
    /// assert_eq!(stats.metric(""), None);
    /// ```
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
