//! The result document: a versioned record of one benchmark run, with the provenance
//! that makes it comparable (commit, dirty state, build profile, platform, registry
//! fingerprint, profile) and, per target, the statistics of each cache mode measured.
//! Local runs are written under the checkout's local half; the accepted baseline is a
//! tracked file of the same shape, minus the machine-local fields, promoted explicitly.

use std::path::{Path, PathBuf};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::repository::Repository;

use super::projection::TargetKind;
use super::stats::Statistics;

/// The schema of a result document.
pub const RESULT_SCHEMA: &str = "majordomus/benchmark-result/v1";

/// Where local results go, relative to the checkout-local half.
pub const LOCAL_DIR: &str = "benchmarks";

/// How the cache stood when the samples were taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CacheMode {
    /// The capability declares no cache.
    Uncached,
    /// The cache was cleared before every sample: the handler ran each time.
    Cold,
    /// The same input repeated: answered from the cache.
    Warm,
    /// Not a capability call.
    NotApplicable,
}

/// One target's measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BenchmarkResult {
    /// The target's key.
    pub key: String,
    /// What was measured.
    pub kind: TargetKind,
    /// The cache mode.
    pub cache_mode: CacheMode,
    /// The statistics.
    pub stats: Statistics,
    /// Handler invocations during the samples, when known (direct transport).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handler_invocations: Option<u64>,
}

/// The provenance a run carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Provenance {
    /// Full commit id of HEAD, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Was the work tree dirty?
    pub dirty: bool,
    /// `debug` or `release`.
    pub build_profile: String,
    /// `std::env::consts::OS`.
    pub os: String,
    /// `std::env::consts::ARCH`.
    pub arch: String,
    /// The executable's version.
    pub version: String,
    /// The registry fingerprint of the repository measured.
    pub registry_fingerprint: String,
}

impl Provenance {
    /// This process, this repository.
    pub fn of(repo: &Repository, registry_fingerprint: &str) -> Self {
        let git = crate::git::inspect(repo.root());
        let (commit, dirty) = match git {
            crate::git::GitState::Available(info) => (info.head, info.working_tree == "dirty"),
            crate::git::GitState::Unavailable { .. } => (None, true),
        };
        Provenance {
            commit,
            dirty,
            build_profile: if cfg!(debug_assertions) {
                "debug".into()
            } else {
                "release".into()
            },
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            version: crate::VERSION.into(),
            registry_fingerprint: registry_fingerprint.into(),
        }
    }

    /// The platform key baselines are kept by.
    pub fn platform(&self) -> String {
        format!("{}-{}-{}", self.os, self.arch, self.build_profile)
    }
}

/// One run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResultDocument {
    /// `majordomus/benchmark-result/v1`.
    pub schema: String,
    /// When the run finished, RFC 3339 UTC. Observed evidence, not a generated artifact.
    pub finished_at: String,
    /// The profile the run used.
    pub profile: String,
    /// Provenance.
    pub provenance: Provenance,
    /// Every measurement, by key then cache mode.
    pub results: Vec<BenchmarkResult>,
}

impl ResultDocument {
    /// Write under `<local>/benchmarks/<utc>-<profile>.json`; returns the path.
    pub fn write_local(&self, repo: &Repository) -> Result<PathBuf> {
        let dir = repo.root().join(repo.local_path()).join(LOCAL_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        let stamp = self.finished_at.replace([':', '-'], "");
        let path = dir.join(format!("{stamp}-{}.json", self.profile));
        write_atomic(&path, &self.render())?;
        Ok(path)
    }

    /// Pretty JSON with a trailing newline.
    pub fn render(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }

    /// One result by key and cache mode.
    pub fn find(&self, key: &str, mode: CacheMode) -> Option<&BenchmarkResult> {
        self.results
            .iter()
            .find(|r| r.key == key && r.cache_mode == mode)
    }
}

/// Write a file through a temporary neighbour and a rename, so that a reader never sees
/// half a file.
pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}
