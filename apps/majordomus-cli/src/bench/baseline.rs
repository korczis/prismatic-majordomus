//! Accepted baselines and the regression policy. A baseline is a tracked result document
//! of one platform (`os-arch-buildprofile`), promoted explicitly from a local run by
//! `bench baseline update`; the policy is data in `.ai/repo/benchmarks/rust/policy.yaml`
//! (the shell tool keeps its own evidence beside it, under `.ai/repo/benchmarks/`). A
//! check compares each measurement of a run with the baseline's under the policy and
//! reports every line; a target the baseline knows and the run does not is reported as
//! stale, never silently attached to something else, and a baseline of another platform
//! is not compared at all.
//!
//! A **budget** is the other half of the policy: an absolute bound on a metric of every
//! target under a prefix (`budgets:` in the same file). A regression threshold asks whether
//! a target got slower than it was on this host; a budget asks whether it is fast enough at
//! all, which no baseline can answer, since a target that was always too slow never
//! regresses. A budget therefore needs no baseline and is held on every host.

use std::collections::BTreeMap;
use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::metadata::yaml;
use crate::repository::Repository;

use super::results::{write_atomic, CacheMode, ResultDocument};

/// Where the Rust executable's baselines and policy live, relative to the tracked half.
pub const BASELINE_DIR: &str = "benchmarks/rust";

/// The policy file's name.
pub const POLICY_FILE: &str = "policy.yaml";

/// One threshold: a relative increase allowed on a metric.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Threshold {
    /// The allowed relative increase (`0.25` is a quarter).
    pub relative: f64,
    /// The metric gates only when the run took at least this many samples; under it the
    /// comparison is reported as `SHORT` and does not fail. A p99 of twenty samples is the
    /// slowest sample, which one scheduler hiccup decides.
    #[serde(default)]
    pub minimum_samples: usize,
}

/// The regression policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Policy {
    /// Thresholds by metric (`p50`, `p95`, `p99`).
    pub regression: BTreeMap<String, Threshold>,
    /// Increases under this many microseconds are noise, whatever the ratio.
    pub minimum_absolute_us: f64,
    /// Per-target allowances, keyed by the target key as `bench coverage` prints it, then
    /// by metric (in the file, a `targets:` list of `target: <key>` items); a metric not
    /// named here keeps the general threshold. The process-cold
    /// target spawns a process and builds the index, so the machine's load decides most of
    /// its wall clock, and it gets more room than a request does.
    #[serde(default)]
    pub targets: BTreeMap<String, BTreeMap<String, Threshold>>,
    /// Absolute bounds, held with or without a baseline and on every host. The default
    /// policy has none: a bound is the repository's to declare, never this crate's.
    #[serde(default)]
    pub budgets: Vec<Budget>,
}

/// An absolute bound on one metric of every target whose key starts with a prefix.
///
/// ```
/// use majordomus_cli::bench::baseline::Budget;
/// use majordomus_cli::bench::results::CacheMode;
///
/// // "a warm screen of the UI answers within 100 ms at p95", as the policy file states it
/// let budget = Budget {
///     prefix: "system.http.live.ui.".into(),
///     cache: CacheMode::Warm,
///     metric: "p95".into(),
///     microseconds: 100_000.0,
///     minimum_samples: 20,
/// };
/// assert!("system.http.live.ui.roadmap".starts_with(&budget.prefix));
/// assert!(!"system.http.cockpit_overview".starts_with(&budget.prefix));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Budget {
    /// The targets it bounds: every key that starts with this (`system.http.live.ui.`).
    pub prefix: String,
    /// The cache mode it bounds; a measurement in another mode is not judged by it.
    pub cache: CacheMode,
    /// The metric (`p50`, `p95`, `p99`, ...).
    pub metric: String,
    /// The bound, in microseconds.
    pub microseconds: f64,
    /// The bound holds only when the run took at least this many samples; under it the
    /// line is reported as `SHORT` and does not fail.
    #[serde(default)]
    pub minimum_samples: usize,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            regression: [
                (
                    "p50".to_string(),
                    Threshold {
                        relative: 0.25,
                        minimum_samples: 0,
                    },
                ),
                (
                    "p95".to_string(),
                    Threshold {
                        relative: 0.30,
                        minimum_samples: 50,
                    },
                ),
                (
                    "p99".to_string(),
                    Threshold {
                        relative: 0.50,
                        minimum_samples: 200,
                    },
                ),
            ]
            .into_iter()
            .collect(),
            minimum_absolute_us: 1000.0,
            targets: BTreeMap::new(),
            budgets: Vec::new(),
        }
    }
}

impl Policy {
    /// The policy of a repository: `.ai/repo/benchmarks/rust/policy.yaml`, or the default when
    /// the file does not exist.
    pub fn load(repo: &Repository) -> Result<Self> {
        let path = policy_path(repo);
        if !path.exists() {
            return Ok(Policy::default());
        }
        let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
        let map = yaml::parse_mapping(&text).map_err(|reason| Error::InvalidManifest {
            path: path.clone(),
            reason,
        })?;
        let v = serde_json::Value::Object(map);
        let mut policy = Policy::default();
        if let Some(reg) = v.get("regression").and_then(|r| r.as_object()) {
            policy.regression.clear();
            for (metric, t) in reg {
                let threshold = parse_threshold(t, &path, &format!("regression.{metric}"), 0)?;
                policy.regression.insert(metric.clone(), threshold);
            }
        }
        if let Some(targets) = v.get("targets").and_then(|r| r.as_array()) {
            for (i, item) in targets.iter().enumerate() {
                let Some(fields) = item.as_object() else {
                    return Err(Error::InvalidManifest {
                        path: path.clone(),
                        reason: format!("targets[{i}] is not a mapping"),
                    });
                };
                let Some(key) = fields.get("target").and_then(|k| k.as_str()) else {
                    return Err(Error::InvalidManifest {
                        path: path.clone(),
                        reason: format!("targets[{i}] has no `target` key"),
                    });
                };
                let mut per_metric = BTreeMap::new();
                for (metric, t) in fields.iter().filter(|(k, _)| k.as_str() != "target") {
                    let inherited = policy
                        .regression
                        .get(metric)
                        .map(|b| b.minimum_samples)
                        .unwrap_or(0);
                    let threshold =
                        parse_threshold(t, &path, &format!("targets.{key}.{metric}"), inherited)?;
                    per_metric.insert(metric.clone(), threshold);
                }
                policy.targets.insert(key.to_string(), per_metric);
            }
        }
        if let Some(budgets) = v.get("budgets").and_then(|r| r.as_array()) {
            for (i, item) in budgets.iter().enumerate() {
                policy.budgets.push(parse_budget(item, &path, i)?);
            }
        }
        if let Some(m) = v.get("minimum_absolute_us") {
            policy.minimum_absolute_us = m
                .as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| m.as_f64())
                .ok_or_else(|| Error::InvalidManifest {
                    path: path.clone(),
                    reason: "minimum_absolute_us is not a number".into(),
                })?;
        }
        Ok(policy)
    }

    /// The threshold that applies to one metric of one target: the target's own when the
    /// policy names it, the general one otherwise.
    pub fn threshold_for(&self, key: &str, metric: &str) -> Option<&Threshold> {
        self.targets
            .get(key)
            .and_then(|m| m.get(metric))
            .or_else(|| self.regression.get(metric))
    }
}

/// One `{relative, minimum_samples}` mapping; `minimum_samples` falls back to `inherited`.
fn parse_threshold(
    t: &serde_json::Value,
    path: &std::path::Path,
    at: &str,
    inherited: usize,
) -> Result<Threshold> {
    let relative = t
        .get("relative")
        .and_then(|x| {
            x.as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| x.as_f64())
        })
        .ok_or_else(|| Error::InvalidManifest {
            path: path.to_path_buf(),
            reason: format!("{at}.relative is not a number"),
        })?;
    let minimum_samples = match t.get("minimum_samples") {
        None => inherited,
        Some(x) => x
            .as_str()
            .and_then(|s| s.parse::<usize>().ok())
            .or_else(|| x.as_u64().map(|n| n as usize))
            .ok_or_else(|| Error::InvalidManifest {
                path: path.to_path_buf(),
                reason: format!("{at}.minimum_samples is not a whole number"),
            })?,
    };
    Ok(Threshold {
        relative,
        minimum_samples,
    })
}

/// One item of `budgets:`: `prefix`, `cache`, `metric` and `milliseconds`, with an optional
/// `minimum_samples`. Every key but the last is required: a budget with a default written
/// here would be a bound the repository never declared.
fn parse_budget(item: &serde_json::Value, path: &std::path::Path, i: usize) -> Result<Budget> {
    let invalid = |reason: String| Error::InvalidManifest {
        path: path.to_path_buf(),
        reason,
    };
    let text = |key: &str| {
        item.get(key)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid(format!("budgets[{i}].{key} is missing")))
    };
    let prefix = text("prefix")?.to_string();
    let metric = text("metric")?.to_string();
    let cache = match text("cache")? {
        "warm" => CacheMode::Warm,
        "cold" => CacheMode::Cold,
        "uncached" => CacheMode::Uncached,
        "not_applicable" => CacheMode::NotApplicable,
        other => {
            return Err(invalid(format!(
                "budgets[{i}].cache is `{other}`, not one of warm, cold, uncached, not_applicable"
            )))
        }
    };
    let number = |key: &str| {
        item.get(key).and_then(|x| {
            x.as_str()
                .and_then(|s| s.parse::<f64>().ok())
                .or_else(|| x.as_f64())
        })
    };
    let milliseconds = number("milliseconds")
        .filter(|ms| *ms > 0.0)
        .ok_or_else(|| {
            invalid(format!(
                "budgets[{i}].milliseconds is not a positive number"
            ))
        })?;
    let minimum_samples = match item.get("minimum_samples") {
        None => 0,
        Some(_) => number("minimum_samples")
            .filter(|n| *n >= 0.0 && n.fract() == 0.0)
            .map(|n| n as usize)
            .ok_or_else(|| {
                invalid(format!(
                    "budgets[{i}].minimum_samples is not a whole number"
                ))
            })?,
    };
    Ok(Budget {
        prefix,
        cache,
        metric,
        microseconds: milliseconds * 1000.0,
        minimum_samples,
    })
}

/// `.ai/repo/benchmarks/rust/policy.yaml`.
pub fn policy_path(repo: &Repository) -> PathBuf {
    repo.root()
        .join(repo.repo_path())
        .join(BASELINE_DIR)
        .join(POLICY_FILE)
}

/// `.ai/repo/benchmarks/rust/baseline.<platform>.json`.
pub fn baseline_path(repo: &Repository, platform: &str) -> PathBuf {
    policy_path(repo)
        .parent()
        .map(|d| d.join(format!("baseline.{platform}.json")))
        .unwrap_or_default()
}

/// One line of a check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CheckLine {
    /// The target's key.
    pub key: String,
    /// The cache mode.
    pub cache_mode: CacheMode,
    /// The metric.
    pub metric: String,
    /// The baseline's value, microseconds.
    pub baseline_us: f64,
    /// The run's value, microseconds.
    pub current_us: f64,
    /// `current / baseline - 1`.
    pub delta: f64,
    /// The allowed relative increase.
    pub allowed: f64,
    /// How many samples the run took of this target and mode.
    #[serde(default)]
    pub samples: usize,
    /// How many of them are certainly at or above the run's value of the metric
    /// ([`super::Statistics::samples_at_or_above`]): the samples a `FAIL` rests on.
    #[serde(default)]
    pub agreeing: usize,
    /// `PASS`, `FAIL`, `NOISE` (the increase is under the absolute floor), `SHORT` (the
    /// run took fewer samples than the metric's `minimum_samples`), or `WARN` (over the
    /// threshold on fewer than two agreeing samples: one cold sample on a loaded runner is
    /// a reason to re-measure, not a regression, and it does not fail).
    pub verdict: String,
}

/// The fewest agreeing samples a `FAIL` needs: one sample over a threshold is a `WARN`.
pub(crate) const AGREEING_SAMPLES: usize = 2;

/// One measurement held against a budget.
///
/// ```
/// use majordomus_cli::bench::baseline::BudgetLine;
/// use majordomus_cli::bench::results::CacheMode;
///
/// let line = BudgetLine {
///     key: "system.http.live.ui.command".into(),
///     cache_mode: CacheMode::Warm,
///     metric: "p95".into(),
///     current_us: 620_000.0,
///     budget_us: 100_000.0,
///     samples: 20,
///     verdict: "OVER".into(),
/// };
/// // the line says what a reader needs to act: which screen, how slow, against what bound
/// assert!(line.current_us > line.budget_us && line.verdict == "OVER");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BudgetLine {
    /// The target's key.
    pub key: String,
    /// The cache mode.
    pub cache_mode: CacheMode,
    /// The metric.
    pub metric: String,
    /// The run's value, microseconds.
    pub current_us: f64,
    /// The bound, microseconds.
    pub budget_us: f64,
    /// How many samples the run took of this target and mode.
    #[serde(default)]
    pub samples: usize,
    /// `PASS`, `OVER` (the value is above the bound), or `SHORT` (the run took fewer
    /// samples than the budget's `minimum_samples`).
    pub verdict: String,
}

/// The outcome of a check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Check {
    /// The platform compared.
    pub platform: String,
    /// Was a baseline for this platform found?
    pub baseline_found: bool,
    /// Every comparison.
    pub lines: Vec<CheckLine>,
    /// Targets the run measured and the baseline lacks.
    pub new_targets: Vec<String>,
    /// Targets the baseline has and the run did not measure (renamed, removed, or filtered).
    pub stale_baseline_targets: Vec<String>,
    /// Did the registry fingerprint change since the baseline?
    pub registry_changed: bool,
    /// The host the baseline was recorded on (empty when unknown).
    pub baseline_host: String,
    /// The host this run measured on.
    pub current_host: String,
    /// The 1-minute load average this run was measured under (empty when unknown).
    #[serde(default)]
    pub current_load: String,
    /// The 1-minute load average the baseline was recorded under (empty when unknown).
    #[serde(default)]
    pub baseline_load: String,
    /// Was the baseline recorded on this host? When not, every line is reported and none
    /// fails: wall-clock numbers of two machines are not a regression of either.
    pub comparable: bool,
    /// Every measurement a budget of the policy bounds, with its verdict. Judged with or
    /// without a baseline, and on every host: a bound is absolute.
    #[serde(default)]
    pub budgets: Vec<BudgetLine>,
}

impl Check {
    /// Compare a run with its platform's baseline under the policy.
    pub fn compare(
        run: &ResultDocument,
        baseline: Option<&ResultDocument>,
        policy: &Policy,
    ) -> Self {
        let platform = run.provenance.platform();
        let budgets = Self::budgets(run, policy);
        let Some(base) = baseline else {
            return Check {
                platform,
                baseline_found: false,
                lines: Vec::new(),
                new_targets: run.results.iter().map(|r| r.key.clone()).collect(),
                stale_baseline_targets: Vec::new(),
                registry_changed: false,
                baseline_host: String::new(),
                current_host: run.provenance.host.clone(),
                current_load: run.provenance.load.clone(),
                baseline_load: String::new(),
                comparable: false,
                budgets,
            };
        };
        let mut lines = Vec::new();
        let mut new_targets = Vec::new();
        for r in &run.results {
            let Some(b) = base.find(&r.key, r.cache_mode) else {
                if !new_targets.contains(&r.key) {
                    new_targets.push(r.key.clone());
                }
                continue;
            };
            for metric in policy.regression.keys() {
                let Some(threshold) = policy.threshold_for(&r.key, metric) else {
                    continue;
                };
                let (Some(cur), Some(bas)) = (r.stats.metric(metric), b.stats.metric(metric))
                else {
                    continue;
                };
                let delta = if bas > 0.0 { cur / bas - 1.0 } else { 0.0 };
                let agreeing = r.stats.samples_at_or_above(metric).unwrap_or(0);
                let verdict = if r.stats.samples < threshold.minimum_samples {
                    "SHORT"
                } else if cur - bas < policy.minimum_absolute_us {
                    "NOISE"
                } else if delta > threshold.relative && agreeing < AGREEING_SAMPLES {
                    "WARN"
                } else if delta > threshold.relative {
                    "FAIL"
                } else {
                    "PASS"
                };
                lines.push(CheckLine {
                    key: r.key.clone(),
                    cache_mode: r.cache_mode,
                    metric: metric.clone(),
                    baseline_us: bas,
                    current_us: cur,
                    delta,
                    allowed: threshold.relative,
                    samples: r.stats.samples,
                    agreeing,
                    verdict: verdict.into(),
                });
            }
        }
        let measured: std::collections::BTreeSet<&str> =
            run.results.iter().map(|r| r.key.as_str()).collect();
        let mut stale: Vec<String> = base
            .results
            .iter()
            .filter(|b| !measured.contains(b.key.as_str()))
            .map(|b| b.key.clone())
            .collect();
        crate::order::canonical(&mut stale);
        stale.dedup();
        Check {
            platform,
            baseline_found: true,
            lines,
            new_targets,
            stale_baseline_targets: stale,
            registry_changed: base.provenance.registry_fingerprint
                != run.provenance.registry_fingerprint,
            baseline_host: base.provenance.host.clone(),
            current_host: run.provenance.host.clone(),
            current_load: run.provenance.load.clone(),
            baseline_load: base.provenance.load.clone(),
            comparable: run.provenance.same_host(&base.provenance),
            budgets,
        }
    }

    /// Every measurement of `run` that a budget of the policy bounds, in the run's order.
    fn budgets(run: &ResultDocument, policy: &Policy) -> Vec<BudgetLine> {
        let mut lines = Vec::new();
        for r in &run.results {
            for b in &policy.budgets {
                if r.cache_mode != b.cache || !r.key.starts_with(&b.prefix) {
                    continue;
                }
                let Some(current) = r.stats.metric(&b.metric) else {
                    continue;
                };
                let verdict = if r.stats.samples < b.minimum_samples {
                    "SHORT"
                } else if current > b.microseconds {
                    "OVER"
                } else {
                    "PASS"
                };
                lines.push(BudgetLine {
                    key: r.key.clone(),
                    cache_mode: r.cache_mode,
                    metric: b.metric.clone(),
                    current_us: current,
                    budget_us: b.microseconds,
                    samples: r.stats.samples,
                    verdict: verdict.into(),
                });
            }
        }
        lines
    }

    /// Is any measurement over its budget? Unlike a regression, this needs no comparable
    /// baseline.
    ///
    /// ```
    /// use majordomus_cli::bench::baseline::{Check, Policy};
    /// use majordomus_cli::bench::results::{Provenance, ResultDocument};
    ///
    /// // a run that measured nothing a budget bounds is over none, with or without a baseline
    /// let run = ResultDocument {
    ///     schema: majordomus_cli::bench::RESULT_SCHEMA.into(),
    ///     finished_at: "2026-10-12T00:00:00Z".into(),
    ///     profile: "ci".into(),
    ///     provenance: Provenance {
    ///         commit: None,
    ///         dirty: false,
    ///         build_profile: "debug".into(),
    ///         os: "os".into(),
    ///         arch: "arch".into(),
    ///         version: "0".into(),
    ///         registry_fingerprint: "f".into(),
    ///         host: "host/1".into(),
    ///         load: String::new(),
    ///     },
    ///     results: Vec::new(),
    /// };
    /// let check = Check::compare(&run, None, &Policy::default());
    /// assert!(check.budgets.is_empty());
    /// assert!(!check.over_budget() && !check.failed());
    /// ```
    pub fn over_budget(&self) -> bool {
        self.budgets.iter().any(|l| l.verdict == "OVER")
    }

    /// The budget section of the human report: one line per bounded measurement, then the
    /// verdict. Empty when the policy bounds nothing the run measured.
    fn render_budgets(&self) -> String {
        let mut s = String::new();
        if self.budgets.is_empty() {
            return s;
        }
        s.push_str(&format!(
            "{:<52} {:<6} {:>5} {:>12} {:>12} {:>5}  budget\n",
            "target", "cache", "metric", "current_us", "budget_us", "n"
        ));
        for l in &self.budgets {
            s.push_str(&format!(
                "{:<52} {:<6} {:>5} {:>12.1} {:>12.1} {:>5}  {}\n",
                l.key,
                format!("{:?}", l.cache_mode).to_lowercase(),
                l.metric,
                l.current_us,
                l.budget_us,
                l.samples,
                l.verdict
            ));
        }
        s.push_str(&format!(
            "bench --check: {} ({})\n",
            if self.over_budget() {
                "over budget"
            } else {
                "within budget"
            },
            self.measured_under()
        ));
        s
    }

    /// What every verdict of this check was measured under: the runner and its 1-minute
    /// load average, and the baseline's when there is one. A timing verdict is a statement
    /// about a machine as much as about a change, so the report never gives one without it.
    ///
    /// ```
    /// use majordomus_cli::bench::baseline::{Check, Policy};
    /// use majordomus_cli::bench::results::{Provenance, ResultDocument};
    ///
    /// let run = ResultDocument {
    ///     schema: majordomus_cli::bench::RESULT_SCHEMA.into(),
    ///     finished_at: "2026-10-16T00:00:00Z".into(),
    ///     profile: "ci".into(),
    ///     provenance: Provenance {
    ///         commit: None,
    ///         dirty: false,
    ///         build_profile: "debug".into(),
    ///         os: "os".into(),
    ///         arch: "arch".into(),
    ///         version: "0".into(),
    ///         registry_fingerprint: "f".into(),
    ///         host: "runner/4".into(),
    ///         load: "31.70".into(),
    ///     },
    ///     results: Vec::new(),
    /// };
    /// let check = Check::compare(&run, None, &Policy::default());
    /// assert_eq!(check.measured_under(), "runner runner/4, load 31.70");
    /// ```
    pub fn measured_under(&self) -> String {
        let or = |v: &str, absent: &str| {
            if v.is_empty() {
                absent.to_string()
            } else {
                v.to_string()
            }
        };
        let mut s = format!(
            "runner {}, load {}",
            or(&self.current_host, "unidentified"),
            or(&self.current_load, "unknown")
        );
        if self.baseline_found {
            s.push_str(&format!(
                "; baseline recorded on {}, load {}",
                or(&self.baseline_host, "an unidentified host"),
                or(&self.baseline_load, "unknown")
            ));
        }
        s
    }

    /// Anything over its budget, or any `FAIL` on a comparable baseline? A baseline from
    /// another host never fails a run: its lines are reported only. A budget is absolute,
    /// and fails on any host.
    pub fn failed(&self) -> bool {
        self.over_budget() || (self.comparable && self.lines.iter().any(|l| l.verdict == "FAIL"))
    }

    /// The human report.
    pub fn render(&self) -> String {
        let mut s = self.render_budgets();
        if !self.baseline_found {
            s.push_str(&format!(
                "no baseline for {}: nothing compared; `majordomus bench baseline update` records one ({})\n",
                self.platform,
                self.measured_under()
            ));
            return s;
        }
        s.push_str(&format!(
            "{:<52} {:<6} {:>5} {:>12} {:>12} {:>8} {:>8} {:>9}  verdict\n",
            "target", "cache", "metric", "baseline_us", "current_us", "delta", "allowed", "agree/n"
        ));
        for l in &self.lines {
            s.push_str(&format!(
                "{:<52} {:<6} {:>5} {:>12.1} {:>12.1} {:>+7.0}% {:>+7.0}% {:>9}  {}\n",
                l.key,
                format!("{:?}", l.cache_mode).to_lowercase(),
                l.metric,
                l.baseline_us,
                l.current_us,
                l.delta * 100.0,
                l.allowed * 100.0,
                format!("{}/{}", l.agreeing, l.samples),
                l.verdict
            ));
        }
        let warned = self.lines.iter().filter(|l| l.verdict == "WARN").count();
        if warned > 0 {
            s.push_str(&format!(
                "WARN   {warned} metric(s) over the threshold on fewer than {AGREEING_SAMPLES} agreeing samples: re-measure with more samples; not a regression until they agree\n"
            ));
        }
        for k in &self.new_targets {
            s.push_str(&format!("NEW    {k} (not in the baseline)\n"));
        }
        for k in &self.stale_baseline_targets {
            s.push_str(&format!(
                "STALE  {k} (in the baseline, not measured; renamed, removed or filtered)\n"
            ));
        }
        if self.registry_changed {
            s.push_str("NOTE   the registry fingerprint differs from the baseline's: the repository or the descriptors changed\n");
        }
        if !self.comparable {
            let recorded = if self.baseline_host.is_empty() {
                "an unidentified host".to_string()
            } else {
                self.baseline_host.clone()
            };
            s.push_str(&format!(
                "NOTE   baseline recorded on {recorded}; this host is {}; reporting only, nothing fails\n",
                self.current_host
            ));
            s.push_str(&format!(
                "bench --check: reporting only (the baseline is from another host; {})\n",
                self.measured_under()
            ));
            return s;
        }
        s.push_str(&format!(
            "bench --check: {} ({})\n",
            if self.failed() {
                "regression(s) found"
            } else {
                "within policy"
            },
            self.measured_under()
        ));
        s
    }
}

/// Read a baseline, when there is one for the platform.
pub fn load_baseline(repo: &Repository, platform: &str) -> Result<Option<ResultDocument>> {
    let path = baseline_path(repo, platform);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let doc: ResultDocument = serde_json::from_str(&text).map_err(|e| Error::InvalidManifest {
        path,
        reason: format!("not a benchmark result document: {e}"),
    })?;
    Ok(Some(doc))
}

/// Promote a run to the baseline of its platform. The tracked copy carries no
/// machine-local path; it is the run's document as it is.
pub fn write_baseline(repo: &Repository, run: &ResultDocument) -> Result<PathBuf> {
    let path = baseline_path(repo, &run.provenance.platform());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    write_atomic(&path, &run.render())?;
    Ok(path)
}
