//! Accepted baselines and the regression policy. A baseline is a tracked result document
//! of one platform (`os-arch-buildprofile`), promoted explicitly from a local run by
//! `bench baseline update`; the policy is data in `.ai/repo/benchmarks/rust/policy.yaml`
//! (the shell tool keeps its own evidence beside it, under `.ai/repo/benchmarks/`). A
//! check compares each measurement of a run with the baseline's under the policy and
//! reports every line; a target the baseline knows and the run does not is reported as
//! stale, never silently attached to something else, and a baseline of another platform
//! is not compared at all.

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
    /// `PASS`, `FAIL`, `NOISE` (the increase is under the absolute floor), or `SHORT` (the
    /// run took fewer samples than the metric's `minimum_samples`).
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
    /// Was the baseline recorded on this host? When not, every line is reported and none
    /// fails: wall-clock numbers of two machines are not a regression of either.
    pub comparable: bool,
}

impl Check {
    /// Compare a run with its platform's baseline under the policy.
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
    pub fn failed(&self) -> bool {
        self.comparable && self.lines.iter().any(|l| l.verdict == "FAIL")
    }

    /// The human report.
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
