//! Accepted baselines and the regression policy. A baseline is a tracked result document
//! of one platform (`os-arch-buildprofile`), promoted explicitly from a local run by
//! `bench baseline update`; the policy is data in `.ai/repo/benchmarks/rust/policy.yaml`
//! (the shell tool keeps its own evidence beside it, under `.ai/repo/benchmarks/`). A
//! check compares each measurement of a run with the baseline's under the policy and
//! reports every line; a target the baseline knows and the run does not is reported as
//! stale, never silently attached to something else, and a baseline of another platform
//! is not compared at all.
//!
//! The comparison is arithmetic over two documents and a policy, with no measurement in it:
//! a metric's increase is judged against a relative threshold, a floor in microseconds that
//! calls small increases noise, and a minimum number of samples under which a metric is
//! reported and cannot fail.
//!
//! ```
//! # use std::time::Duration;
//! # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
//! #     TargetKind, RESULT_SCHEMA};
//! # use majordomus_cli::bench::results::{CacheMode, Provenance};
//! # fn document(host: &str, us: u64) -> ResultDocument {
//! #     ResultDocument {
//! #         schema: RESULT_SCHEMA.into(),
//! #         finished_at: "2026-09-10T12:00:00Z".into(),
//! #         profile: "ci".into(),
//! #         provenance: Provenance {
//! #             commit: None, dirty: false, build_profile: "release".into(),
//! #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
//! #             registry_fingerprint: "abc".into(), host: host.into(),
//! #         },
//! #         results: vec![BenchmarkResult {
//! #             key: SystemTarget::McpPing.key().into(),
//! #             kind: TargetKind::System { target: SystemTarget::McpPing },
//! #             cache_mode: CacheMode::NotApplicable,
//! #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
//! #             handler_invocations: None,
//! #         }],
//! #     }
//! # }
//! use majordomus_cli::bench::baseline::{Check, Policy};
//! let policy = Policy::default();
//! let baseline = document("Example CPU/8", 1_000);
//! let run = document("Example CPU/8", 4_000);
//!
//! let check = Check::compare(&run, Some(&baseline), &policy);
//! assert!(check.baseline_found && check.comparable);
//! assert!(check.failed(), "four times the baseline is past every threshold");
//! // the p99 is reported and does not gate: the profile took too few samples for it
//! let p99 = check.lines.iter().find(|l| l.metric == "p99").unwrap();
//! assert_eq!(p99.verdict, "SHORT");
//!
//! // a baseline recorded on another machine is reported against, never failed against
//! let elsewhere = Check::compare(&run, Some(&document("Another CPU/64", 1_000)), &policy);
//! assert!(!elsewhere.comparable);
//! assert!(!elsewhere.failed());
//! ```

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
///
/// Relative rather than absolute, because an allowance in microseconds would have to be
/// rewritten for every machine and every target. The second field is what keeps a small
/// profile from producing false regressions: a percentile computed over fewer samples than
/// this is reported as `SHORT` and cannot fail a run, because a p99 of twenty samples is
/// the slowest sample and one scheduler hiccup decides it.
///
/// ```
/// use majordomus_cli::bench::baseline::{Policy, Threshold};
/// let policy = Policy::default();
/// // the higher the percentile, the noisier it is, so the more samples it demands
/// let p50: &Threshold = policy.threshold_for("anything", "p50").unwrap();
/// let p99: &Threshold = policy.threshold_for("anything", "p99").unwrap();
/// assert!(p99.relative > p50.relative);
/// assert!(p99.minimum_samples > p50.minimum_samples);
/// assert_eq!(p50.minimum_samples, 0, "a median is meaningful over any run");
/// ```
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

/// The regression policy: what counts as a regression, as data.
///
/// It lives in `.ai/repo/benchmarks/rust/policy.yaml` rather than in this file, because
/// deciding that a target may be slower is a judgement somebody makes and should be able to
/// make in a reviewable diff. Three things are adjustable and each answers a different
/// failure: a relative threshold per metric, an absolute floor under which an increase is
/// noise whatever the ratio, and per-target allowances for the targets whose wall clock is
/// mostly the machine's — spawning a process, building an index.
///
/// ```
/// use majordomus_cli::bench::baseline::{Policy, Threshold};
/// let mut policy = Policy::default();
/// // the general thresholds apply to every target that has no allowance of its own
/// assert_eq!(policy.threshold_for("system.mcp.ping", "p50").unwrap().relative, 0.25);
/// // and a target that spawns a process is given room, by name
/// policy.targets.insert(
///     "system.mcp.process_cold".to_string(),
///     [("p50".to_string(), Threshold { relative: 1.0, minimum_samples: 0 })]
///         .into_iter()
///         .collect(),
/// );
/// assert_eq!(policy.threshold_for("system.mcp.process_cold", "p50").unwrap().relative, 1.0);
/// assert!(policy.minimum_absolute_us > 0.0, "an increase can be too small to mean anything");
/// ```
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
        }
    }
}

impl Policy {
    /// The policy of a repository: `.ai/repo/benchmarks/rust/policy.yaml`, or the default when
    /// the file does not exist.
    ///
    /// Absent is not an error — a repository that has never tuned its thresholds gets the
    /// defaults — but present and malformed is: a policy that does not parse would
    /// otherwise silently become the default, which is the one outcome nobody asked for.
    /// The file is read key by key, so a policy that names only `minimum_absolute_us` keeps
    /// the default thresholds rather than clearing them.
    ///
    /// ```no_run
    /// use majordomus_cli::bench::baseline::Policy;
    /// use majordomus_cli::repository::Repository;
    /// // compiled and not run: it reads a real checkout's policy file
    /// fn read(repo: &Repository) {
    ///     let policy = Policy::load(repo).expect("the policy parses, or says why not");
    ///     assert!(policy.threshold_for("anything", "p50").is_some());
    /// }
    /// ```
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
    /// The threshold that governs one metric of one target: its own allowance, or the
    /// general one.
    ///
    /// Per-target first and the general threshold as the fallback, so an allowance is
    /// always a widening of a rule that already exists rather than a rule of its own. A
    /// metric neither names is `None`, and a comparison of it is skipped — which is what
    /// keeps a policy naming a metric this executable does not compute from failing a run.
    ///
    /// ```
    /// use majordomus_cli::bench::baseline::{Policy, Threshold};
    /// let mut policy = Policy::default();
    /// let general = policy.threshold_for("system.mcp.ping", "p50").unwrap().relative;
    ///
    /// policy.targets.insert(
    ///     "system.mcp.process_cold".to_string(),
    ///     [("p50".to_string(), Threshold { relative: 1.5, minimum_samples: 0 })]
    ///         .into_iter()
    ///         .collect(),
    /// );
    /// // the named target gets its own allowance; everything else keeps the general one
    /// assert_eq!(policy.threshold_for("system.mcp.process_cold", "p50").unwrap().relative, 1.5);
    /// assert_eq!(policy.threshold_for("system.mcp.ping", "p50").unwrap().relative, general);
    /// // an allowance for one metric does not silence the others on that target
    /// assert!(policy.threshold_for("system.mcp.process_cold", "p95").is_some());
    /// // and a metric nobody declared governs nothing
    /// assert!(policy.threshold_for("system.mcp.ping", "p999").is_none());
    /// ```
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

/// Where a repository's regression policy lives: `.ai/repo/benchmarks/rust/policy.yaml`.
///
/// Under the repository's *tracked* half, because the policy is a decision under review,
/// not machine-local state. It is derived from the repository rather than hard-coded so
/// that a checkout which keeps its AI layer somewhere else is still answered correctly.
///
/// ```no_run
/// use majordomus_cli::bench::baseline::policy_path;
/// use majordomus_cli::repository::Repository;
/// // compiled and not run: the path is derived from a real checkout
/// fn locate(repo: &Repository) {
///     let path = policy_path(repo);
///     assert!(path.ends_with("benchmarks/rust/policy.yaml"), "{}", path.display());
///     assert!(path.starts_with(repo.root()));
/// }
/// ```
pub fn policy_path(repo: &Repository) -> PathBuf {
    repo.root()
        .join(repo.repo_path())
        .join(BASELINE_DIR)
        .join(POLICY_FILE)
}

/// Where the accepted baseline of one platform lives:
/// `.ai/repo/benchmarks/rust/baseline.<platform>.json`.
///
/// One file per platform, beside the policy, and the platform is in the name rather than in
/// the document's body — so a macOS release baseline and a Linux debug one can be committed
/// together and neither can be read as the other's.
///
/// ```no_run
/// use majordomus_cli::bench::baseline::baseline_path;
/// use majordomus_cli::repository::Repository;
/// // compiled and not run: the path is derived from a real checkout
/// fn locate(repo: &Repository) {
///     let path = baseline_path(repo, "macos-aarch64-release");
///     assert!(path.ends_with("baseline.macos-aarch64-release.json"), "{}", path.display());
///     // it sits beside the policy that governs the comparison
///     assert_eq!(
///         path.parent(),
///         majordomus_cli::bench::baseline::policy_path(repo).parent(),
///     );
/// }
/// ```
pub fn baseline_path(repo: &Repository, platform: &str) -> PathBuf {
    policy_path(repo)
        .parent()
        .map(|d| d.join(format!("baseline.{platform}.json")))
        .unwrap_or_default()
}

/// One line of a check: one metric of one measurement, judged.
///
/// It carries both numbers and the allowance as well as the verdict, so a report can be
/// read without re-deriving anything and a person can see *how close* something is rather
/// than only whether it passed. The four verdicts are distinct on purpose: `NOISE` and
/// `SHORT` are both "not failed", and they are not failed for different reasons — one is
/// too small an increase to mean anything, the other too few samples to judge.
///
/// ```
/// # use std::time::Duration;
/// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
/// #     TargetKind, RESULT_SCHEMA};
/// # use majordomus_cli::bench::results::{CacheMode, Provenance};
/// # fn document(host: &str, us: u64) -> ResultDocument {
/// #     ResultDocument {
/// #         schema: RESULT_SCHEMA.into(),
/// #         finished_at: "2026-09-10T12:00:00Z".into(),
/// #         profile: "ci".into(),
/// #         provenance: Provenance {
/// #             commit: None, dirty: false, build_profile: "release".into(),
/// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
/// #             registry_fingerprint: "abc".into(), host: host.into(),
/// #         },
/// #         results: vec![BenchmarkResult {
/// #             key: SystemTarget::McpPing.key().into(),
/// #             kind: TargetKind::System { target: SystemTarget::McpPing },
/// #             cache_mode: CacheMode::NotApplicable,
/// #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
/// #             handler_invocations: None,
/// #         }],
/// #     }
/// # }
/// use majordomus_cli::bench::baseline::{Check, CheckLine, Policy};
/// let check = Check::compare(&document("Example CPU/8", 4_000),
///                            Some(&document("Example CPU/8", 1_000)), &Policy::default());
/// let line: &CheckLine = check.lines.iter().find(|l| l.metric == "p50").unwrap();
/// assert_eq!(line.key, SystemTarget::McpPing.key());
/// assert_eq!((line.baseline_us, line.current_us), (1_000.0, 4_000.0));
/// // the delta is what it is measured against the allowance, both on the line
/// assert!((line.delta - 3.0).abs() < 1e-9);
/// assert_eq!(line.verdict, "FAIL");
/// ```
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
    /// `PASS`, `FAIL`, `NOISE` (the increase is under the absolute floor), or `SHORT` (the
    /// run took fewer samples than the metric's `minimum_samples`).
    pub verdict: String,
}

/// The outcome of a check: every comparison, and everything that could not be compared.
///
/// The fields that are not comparisons are the interesting ones. A target the run measured
/// and the baseline lacks is *new*; one the baseline has and the run did not measure is
/// *stale* — renamed, removed or filtered — and is reported rather than quietly attached to
/// something else. `registry_changed` says the repository is no longer the one the baseline
/// was recorded against, and `comparable` says the two hosts are the same machine, without
/// which nothing is allowed to fail.
///
/// ```
/// # use std::time::Duration;
/// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
/// #     TargetKind, RESULT_SCHEMA};
/// # use majordomus_cli::bench::results::{CacheMode, Provenance};
/// # fn document(host: &str, us: u64) -> ResultDocument {
/// #     ResultDocument {
/// #         schema: RESULT_SCHEMA.into(),
/// #         finished_at: "2026-09-10T12:00:00Z".into(),
/// #         profile: "ci".into(),
/// #         provenance: Provenance {
/// #             commit: None, dirty: false, build_profile: "release".into(),
/// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
/// #             registry_fingerprint: "abc".into(), host: host.into(),
/// #         },
/// #         results: vec![BenchmarkResult {
/// #             key: SystemTarget::McpPing.key().into(),
/// #             kind: TargetKind::System { target: SystemTarget::McpPing },
/// #             cache_mode: CacheMode::NotApplicable,
/// #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
/// #             handler_invocations: None,
/// #         }],
/// #     }
/// # }
/// use majordomus_cli::bench::baseline::{Check, Policy};
/// // no baseline at all: nothing is compared, and everything measured is new
/// let run = document("Example CPU/8", 1_000);
/// let first = Check::compare(&run, None, &Policy::default());
/// assert!(!first.baseline_found);
/// assert!(first.lines.is_empty());
/// assert_eq!(first.new_targets, vec![SystemTarget::McpPing.key().to_string()]);
/// assert!(!first.failed(), "there is nothing to have regressed against");
///
/// // a baseline whose target the run did not measure is stale, not a comparison
/// let mut baseline = document("Example CPU/8", 1_000);
/// baseline.results[0].key = "system.mcp.tools_list".into();
/// let check = Check::compare(&run, Some(&baseline), &Policy::default());
/// assert_eq!(check.stale_baseline_targets, vec!["system.mcp.tools_list".to_string()]);
/// assert_eq!(check.new_targets, vec![SystemTarget::McpPing.key().to_string()]);
/// ```
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
    /// Was the baseline recorded on this host? When not, every line is reported and none
    /// fails: wall-clock numbers of two machines are not a regression of either.
    pub comparable: bool,
}

impl Check {
    /// Compare a run with its platform's baseline under the policy.
    ///
    /// Every metric of every measurement the two have in common is judged, and the verdict
    /// is one of four: `FAIL` when the relative increase is past the allowance, `NOISE`
    /// when the absolute increase is under the policy's floor whatever the ratio, `SHORT`
    /// when the run took fewer samples than the metric demands, and `PASS`. Nothing is
    /// dropped — a comparison that could not be made is reported as a new or a stale target
    /// — because a check that silently compared fewer things each release would go green by
    /// forgetting.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(host: &str, us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: host.into(),
    /// #         },
    /// #         results: vec![BenchmarkResult {
    /// #             key: SystemTarget::McpPing.key().into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
    /// #             handler_invocations: None,
    /// #         }],
    /// #     }
    /// # }
    /// use majordomus_cli::bench::baseline::{Check, Policy};
    /// let policy = Policy::default();
    /// let baseline = document("Example CPU/8", 1_000);
    ///
    /// // a run that is barely slower is not a regression: the increase is under the floor
    /// let quiet = Check::compare(&document("Example CPU/8", 1_500), Some(&baseline), &policy);
    /// assert!(quiet.lines.iter().all(|l| l.verdict != "FAIL"), "{:?}", quiet.lines);
    /// assert!(!quiet.failed());
    ///
    /// // four times as slow is past both the floor and the allowance
    /// let loud = Check::compare(&document("Example CPU/8", 4_000), Some(&baseline), &policy);
    /// assert_eq!(loud.lines.iter().find(|l| l.metric == "p50").unwrap().verdict, "FAIL");
    /// assert!(loud.failed());
    /// ```
    pub fn compare(
        run: &ResultDocument,
        baseline: Option<&ResultDocument>,
        policy: &Policy,
    ) -> Self {
        let platform = run.provenance.platform();
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
                comparable: false,
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
                let verdict = if r.stats.samples < threshold.minimum_samples {
                    "SHORT"
                } else if cur - bas < policy.minimum_absolute_us {
                    "NOISE"
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
        stale.sort();
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
            comparable: run.provenance.same_host(&base.provenance),
        }
    }

    /// Any `FAIL` on a comparable baseline? A baseline from another host never fails a
    /// run: its lines are reported only.
    ///
    /// The one question a gate asks, and the host condition is why it is not simply "does
    /// any line say FAIL": wall-clock numbers from two machines are not a regression of
    /// either, and a check that failed on them would make the gate a measurement of which
    /// runner CI happened to schedule.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(host: &str, us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: host.into(),
    /// #         },
    /// #         results: vec![BenchmarkResult {
    /// #             key: SystemTarget::McpPing.key().into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
    /// #             handler_invocations: None,
    /// #         }],
    /// #     }
    /// # }
    /// use majordomus_cli::bench::baseline::{Check, Policy};
    /// let policy = Policy::default();
    /// let slow = document("Example CPU/8", 4_000);
    ///
    /// // the same machine: a real regression, and it fails
    /// let here = Check::compare(&slow, Some(&document("Example CPU/8", 1_000)), &policy);
    /// assert!(here.failed());
    ///
    /// // the same numbers against a baseline from elsewhere: reported, and nothing fails
    /// let elsewhere = Check::compare(&slow, Some(&document("Another CPU/64", 1_000)), &policy);
    /// assert!(elsewhere.lines.iter().any(|l| l.verdict == "FAIL"));
    /// assert!(!elsewhere.failed(), "two machines are not a regression of either");
    /// ```
    pub fn failed(&self) -> bool {
        self.comparable && self.lines.iter().any(|l| l.verdict == "FAIL")
    }

    /// The report a person reads: every comparison as a row, then what could not be
    /// compared, then the verdict.
    ///
    /// Every line is printed, passes included, because the useful question when a benchmark
    /// moves is usually "what else moved?" — and the last line always says what the check
    /// concluded, so a log can be read from the bottom. A check with no baseline says how to
    /// record one instead of printing an empty table.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(host: &str, us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: host.into(),
    /// #         },
    /// #         results: vec![BenchmarkResult {
    /// #             key: SystemTarget::McpPing.key().into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
    /// #             handler_invocations: None,
    /// #         }],
    /// #     }
    /// # }
    /// use majordomus_cli::bench::baseline::{Check, Policy};
    /// let policy = Policy::default();
    /// // nothing to compare against: the report says what would record one
    /// let first = Check::compare(&document("Example CPU/8", 1_000), None, &policy);
    /// assert!(first.render().contains("bench baseline update"), "{}", first.render());
    ///
    /// // and a check against another machine says it is reporting only
    /// let elsewhere = Check::compare(&document("Example CPU/8", 4_000),
    ///     Some(&document("Another CPU/64", 1_000)), &policy);
    /// let report = elsewhere.render();
    /// assert!(report.contains("reporting only"), "{report}");
    /// assert!(report.contains("Another CPU/64"), "{report}");
    /// ```
    pub fn render(&self) -> String {
        let mut s = String::new();
        if !self.baseline_found {
            s.push_str(&format!(
                "no baseline for {}: nothing compared; `majordomus bench baseline update` records one\n",
                self.platform
            ));
            return s;
        }
        s.push_str(&format!(
            "{:<52} {:<6} {:>5} {:>12} {:>12} {:>8} {:>8}  verdict\n",
            "target", "cache", "metric", "baseline_us", "current_us", "delta", "allowed"
        ));
        for l in &self.lines {
            s.push_str(&format!(
                "{:<52} {:<6} {:>5} {:>12.1} {:>12.1} {:>+7.0}% {:>+7.0}%  {}\n",
                l.key,
                format!("{:?}", l.cache_mode).to_lowercase(),
                l.metric,
                l.baseline_us,
                l.current_us,
                l.delta * 100.0,
                l.allowed * 100.0,
                l.verdict
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
            s.push_str("bench --check: reporting only (the baseline is from another host)\n");
            return s;
        }
        s.push_str(if self.failed() {
            "bench --check: regression(s) found\n"
        } else {
            "bench --check: within policy\n"
        });
        s
    }
}

/// Read the accepted baseline of a platform, when there is one.
///
/// Absent is `Ok(None)` and not an error: a platform nobody has recorded a baseline for is
/// the ordinary state of a new machine, and the check reports that it compared nothing. A
/// file that is there and is not a result document *is* an error — a baseline that failed
/// to parse must never be read as "no baseline", which would turn a broken file into a
/// silently passing check.
///
/// ```no_run
/// use majordomus_cli::bench::baseline::load_baseline;
/// use majordomus_cli::repository::Repository;
/// // compiled and not run: it reads a real checkout's tracked baseline
/// fn read(repo: &Repository) {
///     assert!(load_baseline(repo, "no-such-platform").unwrap().is_none());
///     if let Some(baseline) = load_baseline(repo, "macos-aarch64-release").unwrap() {
///         assert_eq!(baseline.provenance.platform(), "macos-aarch64-release");
///     }
/// }
/// ```
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
///
/// A deliberate act, and the only way a baseline changes: nothing here promotes a run
/// because it looked good. The document is copied unaltered, so the accepted baseline
/// carries the host it was recorded on — which is what later lets a check tell a comparison
/// it may fail on from one it may only report. It is written atomically, into the file the
/// run's own platform names.
///
/// ```no_run
/// use majordomus_cli::bench::baseline::{baseline_path, write_baseline};
/// use majordomus_cli::bench::ResultDocument;
/// use majordomus_cli::repository::Repository;
/// // compiled and not run: it writes into a real checkout's tracked tree
/// fn promote(repo: &Repository, run: &ResultDocument) {
///     let path = write_baseline(repo, run).expect("the tracked half is writable");
///     // the platform of the run decides the file, so a baseline cannot be misfiled
///     assert_eq!(path, baseline_path(repo, &run.provenance.platform()));
/// }
/// ```
pub fn write_baseline(repo: &Repository, run: &ResultDocument) -> Result<PathBuf> {
    let path = baseline_path(repo, &run.provenance.platform());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    write_atomic(&path, &run.render())?;
    Ok(path)
}
