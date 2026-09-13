//! Accepted baselines and the regression policy. A baseline is a tracked result document
//! of one platform (`os-arch-buildprofile`), promoted explicitly from a local run by
//! `bench baseline update`; the policy is data in `.ai/repo/benchmarks/rust/policy.yaml`
//! (the shell tool keeps its own evidence beside it, under `.ai/repo/benchmarks/`). A
//! check compares each measurement of a run with the baseline's under the policy and
//! reports every line; a target the baseline knows and the run does not is reported as
//! stale, never silently attached to something else, and a baseline of another platform
//! is not compared at all.
//!
//! The policy being data is what makes a per-target allowance possible without a branch in
//! the comparison: the process-cold target spawns a process and builds an index, so its
//! wall clock is mostly the machine's load, and `policy.yaml` gives it room by naming it
//! rather than by anyone writing `if key == ...` here.
//!
//! ```
//! use std::collections::BTreeMap;
//! use std::time::Duration;
//! use majordomus_cli::bench::baseline::{Check, Policy, Threshold};
//! use majordomus_cli::bench::results::CacheMode;
//! use majordomus_cli::bench::{
//!     BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA,
//! };
//! # fn run(key: &str, us: u64) -> ResultDocument {
//! #     ResultDocument {
//! #         schema: RESULT_SCHEMA.into(),
//! #         finished_at: "2026-01-01T00:00:00Z".into(),
//! #         profile: "ci".into(),
//! #         provenance: serde_json::from_value(serde_json::json!({
//! #             "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
//! #             "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
//! #         })).unwrap(),
//! #         results: vec![BenchmarkResult {
//! #             key: key.into(),
//! #             kind: TargetKind::System { target: SystemTarget::McpProcessCold },
//! #             cache_mode: CacheMode::NotApplicable,
//! #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
//! #             handler_invocations: None,
//! #         }],
//! #     }
//! # }
//! let cold = SystemTarget::McpProcessCold.key();
//!
//! // A general quarter, and three times that for the one target whose clock is the
//! // machine's mood. Both come out of the file; neither is written in the comparison.
//! let policy = Policy {
//!     regression: [("p50".to_string(), Threshold { relative: 0.25, minimum_samples: 0 })]
//!         .into_iter()
//!         .collect(),
//!     minimum_absolute_us: 1000.0,
//!     targets: [(
//!         cold.to_string(),
//!         [("p50".to_string(), Threshold { relative: 0.75, minimum_samples: 0 })]
//!             .into_iter()
//!             .collect::<BTreeMap<_, _>>(),
//!     )]
//!     .into_iter()
//!     .collect(),
//! };
//!
//! // Half again as slow: over the general threshold, inside this target's own.
//! let check = Check::compare(&run(cold, 150_000), Some(&run(cold, 100_000)), &policy);
//! assert_eq!(check.lines.len(), 1);
//! assert_eq!(check.lines[0].verdict, "PASS");
//! assert_eq!(check.lines[0].allowed, 0.75, "the target's allowance, not the general one");
//! assert!(!check.failed());
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
/// Two fields, because a relative threshold on its own fails twice. It is too tight at the
/// bottom — a quarter of a 40 µs target is 10 µs, which is scheduler noise, and
/// `minimum_absolute_us` is what stops that from being a regression. And it is meaningless
/// on a tail measured from too few samples — a p99 of twenty samples *is* the slowest
/// sample, so `minimum_samples` reports the comparison as `SHORT` instead of gating on one
/// hiccup.
///
/// ```
/// use majordomus_cli::bench::baseline::Threshold;
///
/// let quarter = Threshold { relative: 0.25, minimum_samples: 0 };
/// // The same threshold buys 10 µs on a fast target and a whole millisecond on a slow one.
/// assert_eq!(40.0 * quarter.relative, 10.0);
/// assert_eq!(4_000.0 * quarter.relative, 1_000.0);
///
/// // A tail metric asks for a run long enough to have one.
/// let tail = Threshold { relative: 0.5, minimum_samples: 200 };
/// assert!(tail.minimum_samples > quarter.minimum_samples);
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

/// The regression policy: which metrics gate, by how much, and where the noise floor is.
///
/// Its default is the policy, not a fallback: a repository with no `policy.yaml` is
/// checked under exactly these numbers, and the file exists to give one target more room,
/// not to switch checking on. The three metrics are ordered the way confidence in them is
/// — p50 gates from the first sample, p95 needs fifty, p99 needs two hundred — so a quick
/// run gates on the median and reports the tails as `SHORT` rather than pretending to
/// measure them.
///
/// ```
/// use majordomus_cli::bench::baseline::Policy;
///
/// let policy = Policy::default();
/// assert_eq!(policy.regression["p50"].relative, 0.25);
/// assert_eq!(policy.regression["p50"].minimum_samples, 0, "the median gates at once");
/// assert_eq!(policy.regression["p99"].minimum_samples, 200, "the tail needs a long run");
/// assert!(policy.regression["p99"].relative > policy.regression["p50"].relative);
/// assert_eq!(policy.minimum_absolute_us, 1000.0);
/// assert!(policy.targets.is_empty(), "no target is excused until a file says so");
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
    /// A `regression:` mapping in the file *replaces* the default metrics rather than
    /// merging into them, so a policy that names only `p50` gates only on the median and
    /// the tails are not silently still in force. A per-target entry is the other way
    /// round: it overrides the metrics it names and inherits `minimum_samples` from the
    /// general threshold it overrides, because a target's allowance is about that target's
    /// variance, not about how long the run was.
    ///
    /// ```
    /// use majordomus_cli::bench::baseline::Policy;
    /// use majordomus_cli::Repository;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// std::fs::create_dir_all(dir.path().join(".ai/repo/benchmarks/rust")).unwrap();
    /// std::fs::write(dir.path().join(".ai/manifest.yaml"),
    ///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
    /// let repo = Repository::discover(dir.path()).unwrap();
    ///
    /// // No file: the default is the policy, not an absence of one.
    /// assert_eq!(Policy::load(&repo).unwrap(), Policy::default());
    ///
    /// std::fs::write(dir.path().join(".ai/repo/benchmarks/rust/policy.yaml"),
    ///     "regression:\n  p50:\n    relative: 0.20\nminimum_absolute_us: 250\ntargets:\n  - target: system.mcp.process_cold\n    p50:\n      relative: 1.0\n").unwrap();
    /// let policy = Policy::load(&repo).unwrap();
    ///
    /// assert_eq!(policy.regression.len(), 1, "the file replaces the metrics, it does not add to them");
    /// assert_eq!(policy.regression["p50"].relative, 0.20);
    /// assert_eq!(policy.minimum_absolute_us, 250.0);
    /// assert_eq!(policy.targets["system.mcp.process_cold"]["p50"].relative, 1.0);
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
    ///
    /// The fall-through is per *metric*, not per target: a target that is given room on
    /// `p50` is still held to the general `p99`, so an allowance is as narrow as it was
    /// written. A metric no threshold covers answers `None`, and the comparison for it is
    /// skipped rather than run against a zero.
    ///
    /// ```
    /// use std::collections::BTreeMap;
    /// use majordomus_cli::bench::baseline::{Policy, Threshold};
    ///
    /// let mut policy = Policy::default();
    /// policy.targets.insert(
    ///     "system.mcp.process_cold".to_string(),
    ///     [("p50".to_string(), Threshold { relative: 1.0, minimum_samples: 0 })]
    ///         .into_iter()
    ///         .collect::<BTreeMap<_, _>>(),
    /// );
    ///
    /// assert_eq!(policy.threshold_for("system.mcp.process_cold", "p50").unwrap().relative, 1.0);
    /// assert_eq!(
    ///     policy.threshold_for("system.mcp.process_cold", "p99").unwrap().relative,
    ///     policy.regression["p99"].relative,
    ///     "the allowance was written for the median only"
    /// );
    /// assert_eq!(policy.threshold_for("system.mcp.ping", "p50").unwrap().relative, 0.25);
    /// assert!(policy.threshold_for("system.mcp.ping", "p75").is_none());
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

/// Where this repository keeps the regression policy: `<repository>/.ai/repo/benchmarks/rust/policy.yaml`.
///
/// Under the *tracked* half, because the policy is a decision the repository made and
/// everyone measuring it must be held to the same one; a policy in the local half would
/// let each machine set its own thresholds and call the result a check. The `rust/`
/// segment is there because the shell tool keeps its own evidence beside it.
///
/// ```
/// use majordomus_cli::bench::baseline::policy_path;
/// use majordomus_cli::Repository;
///
/// let dir = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
/// std::fs::write(dir.path().join(".ai/manifest.yaml"),
///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
/// let repo = Repository::discover(dir.path()).unwrap();
///
/// assert!(policy_path(&repo).ends_with(".ai/repo/benchmarks/rust/policy.yaml"));
/// assert!(policy_path(&repo).starts_with(repo.root()));
/// ```
pub fn policy_path(repo: &Repository) -> PathBuf {
    repo.root()
        .join(repo.repo_path())
        .join(BASELINE_DIR)
        .join(POLICY_FILE)
}

/// Where the accepted baseline of one platform lives:
/// `<repository>/.ai/repo/benchmarks/rust/baseline.<platform>.json`.
///
/// The platform is in the *file name* rather than inside the document, so several
/// platforms keep baselines side by side, a run reads only its own, and a machine that has
/// never recorded one simply finds no file — which is reported as "no baseline", not as a
/// comparison against somebody else's hardware.
///
/// ```
/// use majordomus_cli::bench::baseline::baseline_path;
/// use majordomus_cli::Repository;
///
/// let dir = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
/// std::fs::write(dir.path().join(".ai/manifest.yaml"),
///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
/// let repo = Repository::discover(dir.path()).unwrap();
///
/// let linux = baseline_path(&repo, "linux-x86_64-release");
/// let mac = baseline_path(&repo, "macos-aarch64-release");
/// assert!(linux.ends_with("baseline.linux-x86_64-release.json"));
/// assert_ne!(linux, mac, "two platforms, two files, neither overwriting the other");
/// assert_eq!(linux.parent(), mac.parent());
/// ```
pub fn baseline_path(repo: &Repository, platform: &str) -> PathBuf {
    policy_path(repo)
        .parent()
        .map(|d| d.join(format!("baseline.{platform}.json")))
        .unwrap_or_default()
}

/// One line of a check: one metric of one target under one cache mode, with both numbers
/// and the verdict.
///
/// The line carries `baseline_us`, `current_us` and the `allowed` ratio beside the
/// verdict, so a reader can recompute the decision instead of trusting it — and so a
/// `NOISE` on a fast target and a `PASS` on a slow one are told apart by looking, not by
/// rerunning. Four verdicts exist because there are four outcomes worth distinguishing:
/// within policy, over it, over it by an amount too small to mean anything, and measured
/// from too few samples to judge.
///
/// ```
/// use majordomus_cli::bench::baseline::CheckLine;
///
/// let line: CheckLine = serde_json::from_value(serde_json::json!({
///     "key": "system.http.openapi", "cache_mode": "not_applicable", "metric": "p50",
///     "baseline_us": 400.0, "current_us": 600.0, "delta": 0.5, "allowed": 0.25,
///     "verdict": "FAIL"
/// }))
/// .unwrap();
///
/// // The verdict is recomputable from the line, which is why the numbers are kept.
/// assert_eq!(line.current_us / line.baseline_us - 1.0, line.delta);
/// assert!(line.delta > line.allowed);
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

/// The outcome of a check: every comparison, plus everything that could not be compared
/// and why.
///
/// Most of this struct is about the cases where there is nothing to compare, because those
/// are the cases a check gets wrong. A target the baseline lacks is `new_targets`; a target
/// the baseline has and the run did not measure is `stale_baseline_targets`; a different
/// `registry_fingerprint` says the set of targets itself moved; and `comparable` says
/// whether the baseline was recorded on this host at all. None of them is an error, and all
/// of them are printed — an unreportable comparison that says nothing is how a regression
/// gets through.
///
/// ```
/// # use std::time::Duration;
/// use majordomus_cli::bench::baseline::{Check, Policy};
/// # use majordomus_cli::bench::results::CacheMode;
/// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA};
/// # fn doc(entries: &[(&str, u64)]) -> ResultDocument {
/// #     ResultDocument {
/// #         schema: RESULT_SCHEMA.into(),
/// #         finished_at: "2026-01-01T00:00:00Z".into(),
/// #         profile: "ci".into(),
/// #         provenance: serde_json::from_value(serde_json::json!({
/// #             "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
/// #             "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
/// #         })).unwrap(),
/// #         results: entries.iter().map(|(key, us)| BenchmarkResult {
/// #             key: (*key).into(),
/// #             kind: TargetKind::System { target: SystemTarget::McpPing },
/// #             cache_mode: CacheMode::NotApplicable,
/// #             stats: Statistics::of(&vec![Duration::from_micros(*us); 60]),
/// #             handler_invocations: None,
/// #         }).collect(),
/// #     }
/// # }
/// // The first run on a machine: there is no baseline, so nothing is compared and
/// // everything measured is reported as new.
/// let run = doc(&[("system.mcp.ping", 90), ("system.http.openapi", 400)]);
/// let check = Check::compare(&run, None, &Policy::default());
///
/// assert!(!check.baseline_found);
/// assert!(check.lines.is_empty(), "nothing to compare against");
/// assert_eq!(check.new_targets.len(), 2);
/// assert!(!check.comparable);
/// assert!(!check.failed(), "a missing baseline is not a regression");
/// assert_eq!(check.platform, "linux-x86_64-release");
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
    /// A comparison is made only where both sides have the same key *and* the same cache
    /// mode. Everything else is named rather than joined to something near it: a key only
    /// the run has is `NEW`, a key only the baseline has is `STALE`, and a renamed target
    /// therefore shows up as one of each instead of as a regression against a target that
    /// no longer exists.
    ///
    /// ```
    /// # use std::collections::BTreeMap;
    /// # use std::time::Duration;
    /// use majordomus_cli::bench::baseline::{Check, Policy, Threshold};
    /// # use majordomus_cli::bench::results::CacheMode;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA};
    /// # fn doc(entries: &[(&str, u64)]) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-01-01T00:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: serde_json::from_value(serde_json::json!({
    /// #             "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    /// #             "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
    /// #         })).unwrap(),
    /// #         results: entries.iter().map(|(key, us)| BenchmarkResult {
    /// #             key: (*key).into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(*us); 60]),
    /// #             handler_invocations: None,
    /// #         }).collect(),
    /// #     }
    /// # }
    /// let policy = Policy {
    ///     regression: [("p50".to_string(), Threshold { relative: 0.25, minimum_samples: 0 })]
    ///         .into_iter()
    ///         .collect(),
    ///     minimum_absolute_us: 1000.0,
    ///     targets: BTreeMap::new(),
    /// };
    ///
    /// // The baseline knows `tools_list`; this run renamed it and added nothing else.
    /// let baseline = doc(&[("system.mcp.ping", 4_000), ("system.mcp.tools_list", 9_000)]);
    /// let run = doc(&[("system.mcp.ping", 6_000), ("system.mcp.list_tools", 9_000)]);
    /// let check = Check::compare(&run, Some(&baseline), &policy);
    ///
    /// // One key is on both sides, so exactly one comparison was made.
    /// assert_eq!(check.lines.len(), 1);
    /// assert_eq!(check.lines[0].key, "system.mcp.ping");
    /// assert_eq!(check.lines[0].verdict, "FAIL", "+50% on a 4 ms target, allowed 25%");
    ///
    /// // The rename is two facts, not a regression against the old name.
    /// assert_eq!(check.new_targets, ["system.mcp.list_tools"]);
    /// assert_eq!(check.stale_baseline_targets, ["system.mcp.tools_list"]);
    /// assert!(check.comparable, "same host, so the comparison counts");
    /// assert!(check.failed());
    /// ```
    ///
    /// The two floors keep a comparison from meaning more than it can. An increase smaller
    /// than the policy's absolute floor is `NOISE` however large the ratio, and a metric
    /// measured from fewer samples than its threshold asks for is `SHORT`:
    ///
    /// ```
    /// # use std::collections::BTreeMap;
    /// # use std::time::Duration;
    /// use majordomus_cli::bench::baseline::{Check, Policy, Threshold};
    /// # use majordomus_cli::bench::results::CacheMode;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA};
    /// # fn doc(us: u64, samples: usize) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-01-01T00:00:00Z".into(),
    /// #         profile: "quick".into(),
    /// #         provenance: serde_json::from_value(serde_json::json!({
    /// #             "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    /// #             "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
    /// #         })).unwrap(),
    /// #         results: vec![BenchmarkResult {
    /// #             key: "system.mcp.ping".into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(us); samples]),
    /// #             handler_invocations: None,
    /// #         }],
    /// #     }
    /// # }
    /// let threshold = Threshold { relative: 0.25, minimum_samples: 0 };
    /// let policy = Policy {
    ///     regression: [("p50".to_string(), threshold)].into_iter().collect(),
    ///     minimum_absolute_us: 1000.0,
    ///     targets: BTreeMap::new(),
    /// };
    ///
    /// // Four times slower, and 30 µs: under the absolute floor, so it is noise.
    /// let noisy = Check::compare(&doc(40, 60), Some(&doc(10, 60)), &policy);
    /// assert_eq!(noisy.lines[0].delta, 3.0);
    /// assert_eq!(noisy.lines[0].verdict, "NOISE");
    /// assert!(!noisy.failed());
    ///
    /// // The same ratio on a target slow enough for it to mean something.
    /// let real = Check::compare(&doc(40_000, 60), Some(&doc(10_000, 60)), &policy);
    /// assert_eq!(real.lines[0].verdict, "FAIL");
    ///
    /// // A tail metric asked for 200 samples and got 60: reported, not judged.
    /// let tail = Policy {
    ///     regression: [("p50".to_string(), Threshold { relative: 0.25, minimum_samples: 200 })]
    ///         .into_iter()
    ///         .collect(),
    ///     ..policy
    /// };
    /// let short = Check::compare(&doc(40_000, 60), Some(&doc(10_000, 60)), &tail);
    /// assert_eq!(short.lines[0].verdict, "SHORT");
    /// assert!(!short.failed());
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
            comparable: run.provenance.same_host(&base.provenance),
        }
    }

    /// Any `FAIL` on a comparable baseline? A baseline from another host never fails a
    /// run: its lines are reported only.
    ///
    /// The lines are identical either way — the same comparisons, the same `FAIL`
    /// verdicts, printed either way — and only the verdict of the *run* changes. That is
    /// the distinction the whole host fingerprint exists for: a laptop's baseline read on a
    /// shared CI runner produces regressions on every line, and failing on them would teach
    /// everyone to ignore the check.
    ///
    /// ```
    /// # use std::collections::BTreeMap;
    /// # use std::time::Duration;
    /// use majordomus_cli::bench::baseline::{Check, Policy, Threshold};
    /// # use majordomus_cli::bench::results::CacheMode;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA};
    /// # fn doc(host: &str, us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-01-01T00:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: serde_json::from_value(serde_json::json!({
    /// #             "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    /// #             "version": "0.0.0", "registry_fingerprint": "f", "host": host
    /// #         })).unwrap(),
    /// #         results: vec![BenchmarkResult {
    /// #             key: "system.mcp.ping".into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(us); 60]),
    /// #             handler_invocations: None,
    /// #         }],
    /// #     }
    /// # }
    /// let policy = Policy {
    ///     regression: [("p50".to_string(), Threshold { relative: 0.25, minimum_samples: 0 })]
    ///         .into_iter()
    ///         .collect(),
    ///     minimum_absolute_us: 1000.0,
    ///     targets: BTreeMap::new(),
    /// };
    /// let run = doc("a-cpu/8", 6_000);
    ///
    /// let own = Check::compare(&run, Some(&doc("a-cpu/8", 4_000)), &policy);
    /// let borrowed = Check::compare(&run, Some(&doc("another-cpu/64", 4_000)), &policy);
    ///
    /// // The same regression is found both times, and reported both times.
    /// assert_eq!(own.lines[0].verdict, "FAIL");
    /// assert_eq!(borrowed.lines[0].verdict, "FAIL");
    ///
    /// // Only one of them is this machine's to fail on.
    /// assert!(own.failed());
    /// assert!(!borrowed.failed(), "another host's numbers are evidence, not a verdict");
    /// assert!(!borrowed.comparable);
    /// ```
    pub fn failed(&self) -> bool {
        self.comparable && self.lines.iter().any(|l| l.verdict == "FAIL")
    }

    /// The human report: a table of every comparison, then the things that were not
    /// compared, then one line saying what the run means.
    ///
    /// The last line is the report's verdict in words, and it is different for each way a
    /// check can end — no baseline at all, a baseline from another host, within policy, or
    /// regressions found — so a reader who scrolls to the bottom of a CI log is never left
    /// to infer which of the four happened from an absence of output.
    ///
    /// ```
    /// use majordomus_cli::bench::baseline::{Check, Policy};
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::results::CacheMode;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA};
    /// # fn doc(host: &str) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-01-01T00:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: serde_json::from_value(serde_json::json!({
    /// #             "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    /// #             "version": "0.0.0", "registry_fingerprint": "f", "host": host
    /// #         })).unwrap(),
    /// #         results: vec![BenchmarkResult {
    /// #             key: "system.mcp.ping".into(),
    /// #             kind: TargetKind::System { target: SystemTarget::McpPing },
    /// #             cache_mode: CacheMode::NotApplicable,
    /// #             stats: Statistics::of(&vec![Duration::from_micros(90); 60]),
    /// #             handler_invocations: None,
    /// #         }],
    /// #     }
    /// # }
    /// // Nothing to compare: the report says so and names the command that fixes it.
    /// let first_run = Check::compare(&doc("a-cpu/8"), None, &Policy::default()).render();
    /// assert!(first_run.contains("no baseline for linux-x86_64-release"));
    /// assert!(first_run.contains("majordomus bench baseline update"));
    ///
    /// // A baseline from elsewhere: the lines are printed and the run is not judged.
    /// let borrowed =
    ///     Check::compare(&doc("a-cpu/8"), Some(&doc("another-cpu/64")), &Policy::default()).render();
    /// assert!(borrowed.contains("baseline recorded on another-cpu/64"));
    /// assert!(borrowed.ends_with("bench --check: reporting only (the baseline is from another host)\n"));
    ///
    /// // This machine's own baseline, nothing over the thresholds.
    /// let clean =
    ///     Check::compare(&doc("a-cpu/8"), Some(&doc("a-cpu/8")), &Policy::default()).render();
    /// assert!(clean.ends_with("bench --check: within policy\n"));
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

/// Read a baseline, when there is one for the platform.
///
/// The absence of a file is `Ok(None)` and a file that is not a result document is an
/// error, which is the distinction that matters: a machine that has never recorded a
/// baseline is an ordinary state a check reports, and a corrupt or truncated baseline is
/// not something to silently treat as one.
///
/// ```
/// use majordomus_cli::bench::baseline::{baseline_path, load_baseline};
/// use majordomus_cli::bench::{ResultDocument, RESULT_SCHEMA};
/// use majordomus_cli::Repository;
///
/// let dir = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(dir.path().join(".ai/repo/benchmarks/rust")).unwrap();
/// std::fs::write(dir.path().join(".ai/manifest.yaml"),
///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
/// let repo = Repository::discover(dir.path()).unwrap();
///
/// // Never recorded here: an ordinary answer, not an error.
/// assert!(load_baseline(&repo, "linux-x86_64-release").unwrap().is_none());
///
/// let document = ResultDocument {
///     schema: RESULT_SCHEMA.into(),
///     finished_at: "2026-01-01T00:00:00Z".into(),
///     profile: "ci".into(),
///     provenance: serde_json::from_value(serde_json::json!({
///         "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
///         "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
///     }))
///     .unwrap(),
///     results: Vec::new(),
/// };
/// std::fs::write(baseline_path(&repo, "linux-x86_64-release"), document.render()).unwrap();
/// assert_eq!(load_baseline(&repo, "linux-x86_64-release").unwrap(), Some(document));
///
/// // Half a file is refused rather than read as an empty baseline.
/// std::fs::write(baseline_path(&repo, "linux-x86_64-release"), "{\"schema\":").unwrap();
/// assert!(load_baseline(&repo, "linux-x86_64-release").is_err());
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
/// The platform is taken from the run's own provenance rather than from an argument, so a
/// promotion cannot be filed under a platform it was not measured on. Promotion is the only
/// way a baseline changes and it is always explicit — nothing here updates a baseline
/// because a run was faster, which is what keeps a ratchet from quietly following a slow
/// afternoon downwards.
///
/// ```
/// use majordomus_cli::bench::baseline::{load_baseline, write_baseline};
/// use majordomus_cli::bench::{ResultDocument, RESULT_SCHEMA};
/// use majordomus_cli::Repository;
///
/// let dir = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
/// std::fs::write(dir.path().join(".ai/manifest.yaml"),
///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
/// let repo = Repository::discover(dir.path()).unwrap();
///
/// let run = ResultDocument {
///     schema: RESULT_SCHEMA.into(),
///     finished_at: "2026-01-01T00:00:00Z".into(),
///     profile: "full".into(),
///     provenance: serde_json::from_value(serde_json::json!({
///         "dirty": false, "build_profile": "release", "os": "macos", "arch": "aarch64",
///         "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/12"
///     }))
///     .unwrap(),
///     results: Vec::new(),
/// };
///
/// let path = write_baseline(&repo, &run).unwrap();
/// assert!(path.ends_with("baseline.macos-aarch64-release.json"), "filed under what measured it");
/// assert_eq!(load_baseline(&repo, "macos-aarch64-release").unwrap(), Some(run));
/// assert!(load_baseline(&repo, "linux-x86_64-release").unwrap().is_none(), "and under nothing else");
/// ```
pub fn write_baseline(repo: &Repository, run: &ResultDocument) -> Result<PathBuf> {
    let path = baseline_path(repo, &run.provenance.platform());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    write_atomic(&path, &run.render())?;
    Ok(path)
}
