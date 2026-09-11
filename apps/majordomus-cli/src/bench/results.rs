//! The result document: a versioned record of one benchmark run, with the provenance
//! that makes it comparable (commit, dirty state, build profile, platform, registry
//! fingerprint, profile) and, per target, the statistics of each cache mode measured.
//! Local runs are written under the checkout's local half; the accepted baseline is a
//! tracked file of the same shape, minus the machine-local fields, promoted explicitly.
//!
//! The lifecycle is: reduce samples into statistics, attach the provenance that makes them
//! comparable, and write the document atomically. Everything here is data — nothing in this
//! module measures anything.
//!
//! ```
//! use std::time::Duration;
//! use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
//!     TargetKind, RESULT_SCHEMA};
//! use majordomus_cli::bench::results::{CacheMode, Provenance};
//! let run = ResultDocument {
//!     schema: RESULT_SCHEMA.into(),
//!     finished_at: "2026-09-10T12:00:00Z".into(),
//!     profile: "ci".into(),
//!     provenance: Provenance {
//!         commit: Some("08e4bb2".into()), dirty: false, build_profile: "release".into(),
//!         os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
//!         registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
//!     },
//!     results: vec![BenchmarkResult {
//!         key: SystemTarget::McpPing.key().into(),
//!         kind: TargetKind::System { target: SystemTarget::McpPing },
//!         cache_mode: CacheMode::NotApplicable,
//!         stats: Statistics::of(&vec![Duration::from_micros(500); 60]),
//!         handler_invocations: None,
//!     }],
//! };
//!
//! // a baseline is filed by platform, and compared only within one
//! assert_eq!(run.provenance.platform(), "macos-aarch64-release");
//! // a measurement is identified by its target key and its cache mode together
//! let found = run
//!     .find(SystemTarget::McpPing.key(), CacheMode::NotApplicable)
//!     .expect("the run measured it");
//! assert_eq!(found.stats.samples, 60);
//! // and the document round-trips, because a baseline is read back as one of these
//! let text = run.render();
//! assert_eq!(serde_json::from_str::<ResultDocument>(&text).unwrap(), run);
//! ```

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
///
/// Part of a measurement's identity and not a note beside it: a capability with a cache is
/// measured twice, and comparing a cold run against a warm baseline would report a
/// regression that is only a difference of question. `Uncached` and `NotApplicable` are
/// distinct for the same reason — the first is a capability that declares no cache, the
/// second is not a capability call at all.
///
/// ```
/// use majordomus_cli::bench::results::CacheMode;
/// // four distinct words, because a result is looked up by key *and* mode
/// let modes = [CacheMode::Uncached, CacheMode::Cold, CacheMode::Warm,
///              CacheMode::NotApplicable];
/// let mut words: Vec<String> = modes
///     .iter()
///     .map(|m| serde_json::to_value(m).unwrap().to_string())
///     .collect();
/// words.sort();
/// words.dedup();
/// assert_eq!(words.len(), modes.len());
/// assert_eq!(serde_json::to_value(CacheMode::NotApplicable).unwrap(), "not_applicable");
/// ```
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

/// One target's measurement, in one cache mode.
///
/// The identity is the key *and* the mode, which is why a document holds several of these
/// per target rather than one with several sets of numbers. `kind` is carried beside the
/// key so that a result read years later still says what was measured, even if the
/// capability that produced it is gone. `handler_invocations` is the honesty check on a
/// cached measurement: a warm mode that ran the handler as many times as it took samples
/// was not warm.
///
/// ```
/// # use std::time::Duration;
/// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
/// #     TargetKind, RESULT_SCHEMA};
/// # use majordomus_cli::bench::results::{CacheMode, Provenance};
/// # fn document(us: u64) -> ResultDocument {
/// #     ResultDocument {
/// #         schema: RESULT_SCHEMA.into(),
/// #         finished_at: "2026-09-10T12:00:00Z".into(),
/// #         profile: "ci".into(),
/// #         provenance: Provenance {
/// #             commit: None, dirty: false, build_profile: "release".into(),
/// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
/// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
/// let run = document(1_000);
/// let measured = &run.results[0];
/// // a result is found by what identifies it, and by nothing looser
/// assert_eq!(measured.key, SystemTarget::McpPing.key());
/// assert_eq!(measured.cache_mode, CacheMode::NotApplicable);
/// assert_eq!(measured.stats.samples, 60);
/// // a transport's own operation is nobody's handler, so nothing was counted
/// assert!(measured.handler_invocations.is_none());
/// ```
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

/// The provenance a run carries: everything that decides whether two runs may be compared.
///
/// A wall-clock number means nothing on its own. The commit and the dirty flag say what was
/// measured, the build profile says how it was compiled — a debug build is a different
/// program — the registry fingerprint says whether the repository still holds the same
/// work, and the host says whether the two machines are the same machine. Without this a
/// benchmark result is a number with no claim attached to it.
///
/// ```
/// # use std::time::Duration;
/// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
/// #     TargetKind, RESULT_SCHEMA};
/// # use majordomus_cli::bench::results::{CacheMode, Provenance};
/// # fn document(us: u64) -> ResultDocument {
/// #     ResultDocument {
/// #         schema: RESULT_SCHEMA.into(),
/// #         finished_at: "2026-09-10T12:00:00Z".into(),
/// #         profile: "ci".into(),
/// #         provenance: Provenance {
/// #             commit: None, dirty: false, build_profile: "release".into(),
/// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
/// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
/// let run = document(1_000);
/// // the platform is what a baseline is filed under
/// assert_eq!(run.provenance.platform(), "macos-aarch64-release");
/// // and the host is what decides comparability, separately
/// assert!(run.provenance.same_host(&document(2_000).provenance));
/// ```
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
    /// The host that measured: the CPU's brand string and its logical core count
    /// (`Apple M5 Pro/18`). Wall-clock numbers are comparable only between runs on the
    /// same host; a baseline recorded elsewhere is reported against, never failed
    /// against. Empty when the host could not be identified (older baselines).
    #[serde(default)]
    pub host: String,
}

impl Provenance {
    /// The provenance of this process measuring this repository.
    ///
    /// Everything is observed rather than passed in: the commit and the dirty flag from
    /// git, the build profile from `cfg!(debug_assertions)`, the platform and the host from
    /// the machine. A repository git cannot answer for is recorded as *dirty* with no
    /// commit — the safe reading, because a run that cannot say what it measured must not
    /// look like a clean one.
    ///
    /// ```no_run
    /// use majordomus_cli::bench::results::Provenance;
    /// use majordomus_cli::repository::Repository;
    /// // compiled and not run: it inspects a real checkout with git
    /// fn record(repo: &Repository) {
    ///     let provenance = Provenance::of(repo, "abc123");
    ///     assert_eq!(provenance.registry_fingerprint, "abc123");
    ///     assert!(provenance.platform().ends_with(&provenance.build_profile));
    /// }
    /// ```
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
            host: host_fingerprint(),
            version: crate::VERSION.into(),
            registry_fingerprint: registry_fingerprint.into(),
        }
    }

    /// The platform key baselines are kept by: `os-arch-buildprofile`.
    ///
    /// Coarse on purpose. It is the *filing* key — one accepted baseline per platform — and
    /// it deliberately says nothing about the machine, so that a debug build and a release
    /// build of the same commit can never be compared with each other. Whether two runs are
    /// comparable at all is a finer question, and [`Provenance::same_host`] answers it.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
    /// let run = document(1_000);
    /// assert_eq!(run.provenance.platform(), "macos-aarch64-release");
    /// // the build profile is part of the key, so two builds never share a baseline
    /// let mut debug = run.provenance.clone();
    /// debug.build_profile = "debug".into();
    /// assert_ne!(debug.platform(), run.provenance.platform());
    /// ```
    pub fn platform(&self) -> String {
        format!("{}-{}-{}", self.os, self.arch, self.build_profile)
    }

    /// Were these two measured on the same host? Unknown hosts never match.
    ///
    /// This is what decides whether a comparison may *fail* a run: numbers from two
    /// machines are not a regression of either, so a baseline recorded elsewhere is
    /// reported against and never failed against. An empty host — an older baseline,
    /// recorded before the field existed — matches nothing, including another empty one,
    /// because "unknown" is not evidence of sameness.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
    /// let mine = document(1_000).provenance;
    /// assert!(mine.same_host(&document(2_000).provenance));
    ///
    /// let mut elsewhere = mine.clone();
    /// elsewhere.host = "Another CPU/64".into();
    /// assert!(!mine.same_host(&elsewhere));
    ///
    /// // two unidentified hosts are not the same host
    /// let mut unknown = mine.clone();
    /// unknown.host = String::new();
    /// assert!(!unknown.same_host(&unknown.clone()));
    /// ```
    pub fn same_host(&self, other: &Provenance) -> bool {
        !self.host.is_empty() && self.host == other.host
    }
}

/// The CPU brand string and the logical core count, `<brand>/<cores>`; the brand alone
/// when the count is unknown, the os and arch when neither is (an unidentified host
/// still gets a stable, if coarse, name, and two of them never compare as equal only
/// when the string is empty, which this function does not return).
///
/// It is what [`Provenance::same_host`] compares, so it has to be stable across runs on
/// one machine and different between machines of different speed — hence the core count
/// beside the brand. It is never empty, which matters: an empty host means "recorded
/// before this field existed" and is treated as unknown.
///
/// ```no_run
/// use majordomus_cli::bench::results::host_fingerprint;
/// // compiled and not run: on macOS it asks the system for the CPU's brand string
/// let host = host_fingerprint();
/// assert!(!host.is_empty(), "an unidentified host still gets a coarse name");
/// assert_eq!(host, host_fingerprint(), "stable across calls on one machine");
/// ```
pub fn host_fingerprint() -> String {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get().to_string())
        .unwrap_or_default();
    let brand = cpu_brand()
        .unwrap_or_else(|| format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH));
    if cores.is_empty() {
        brand
    } else {
        format!("{brand}/{cores}")
    }
}

/// The CPU brand string: `sysctl machdep.cpu.brand_string` on macOS, the first
/// `model name` of `/proc/cpuinfo` on Linux, nothing elsewhere.
fn cpu_brand() -> Option<String> {
    if cfg!(target_os = "macos") {
        let out = std::process::Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        return (!s.is_empty()).then_some(s);
    }
    if cfg!(target_os = "linux") {
        let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
        return text
            .lines()
            .find(|l| l.starts_with("model name"))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string())
            .filter(|v| !v.is_empty());
    }
    None
}

/// One run: the provenance that makes it comparable, and every measurement it took.
///
/// The same shape serves two purposes, which is why there is one type: a local run written
/// under the checkout's local half, and the accepted baseline tracked in the repository.
/// Promotion is therefore a copy rather than a translation, and a baseline cannot drift
/// into a shape a run cannot be compared with.
///
/// `finished_at` is observed evidence and not a generated field — it is when the run
/// ended, which is also what names the local file.
///
/// ```
/// # use std::time::Duration;
/// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
/// #     TargetKind, RESULT_SCHEMA};
/// # use majordomus_cli::bench::results::{CacheMode, Provenance};
/// # fn document(us: u64) -> ResultDocument {
/// #     ResultDocument {
/// #         schema: RESULT_SCHEMA.into(),
/// #         finished_at: "2026-09-10T12:00:00Z".into(),
/// #         profile: "ci".into(),
/// #         provenance: Provenance {
/// #             commit: None, dirty: false, build_profile: "release".into(),
/// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
/// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
/// let run = document(1_000);
/// assert_eq!(run.schema, RESULT_SCHEMA);
/// // a document is read back exactly as it was written
/// let text = run.render();
/// let back: ResultDocument = serde_json::from_str(&text).unwrap();
/// assert_eq!(back, run);
/// // and a measurement is looked up by the pair that identifies it
/// assert!(run.find(SystemTarget::McpPing.key(), CacheMode::NotApplicable).is_some());
/// ```
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
    ///
    /// The checkout's *local* half, because a local run is machine-local evidence and not
    /// something to commit: only an explicit promotion puts a document into the tracked
    /// tree. The name carries the timestamp and the profile, so runs accumulate beside each
    /// other instead of overwriting one another, and it is written atomically — a reader
    /// never sees half a document.
    ///
    /// ```no_run
    /// use majordomus_cli::bench::ResultDocument;
    /// use majordomus_cli::repository::Repository;
    /// // compiled and not run: it writes into a real checkout
    /// fn record(repo: &Repository, run: &ResultDocument) {
    ///     let path = run.write_local(repo).expect("the local half is writable");
    ///     assert!(path.to_string_lossy().ends_with(".json"));
    ///     assert!(path.to_string_lossy().contains(&run.profile));
    /// }
    /// ```
    pub fn write_local(&self, repo: &Repository) -> Result<PathBuf> {
        let dir = repo.root().join(repo.local_path()).join(LOCAL_DIR);
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        let stamp = self.finished_at.replace([':', '-'], "");
        let path = dir.join(format!("{stamp}-{}.json", self.profile));
        write_atomic(&path, &self.render())?;
        Ok(path)
    }

    /// Pretty JSON with a trailing newline.
    ///
    /// Pretty because a baseline is a tracked file read in diffs: a regression that changes
    /// one target should be one hunk. The trailing newline is the same reason.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
    /// let text = document(1_000).render();
    /// assert!(text.ends_with("\n"), "a committed file ends with a newline");
    /// assert!(text.contains("\n  \"schema\""), "one field per line, for a diff");
    /// // and it is the document itself, not a summary of it
    /// let back: ResultDocument = serde_json::from_str(&text).unwrap();
    /// assert_eq!(back, document(1_000));
    /// ```
    pub fn render(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }

    /// One result by key and cache mode: the pair that identifies a measurement.
    ///
    /// Both halves are required, and that is the point — this is what a baseline comparison
    /// looks a run's measurement up with, and matching on the key alone would compare a
    /// cold measurement against a warm baseline. `None` is how a target the baseline knows
    /// and the run did not measure becomes a reported stale line rather than a comparison
    /// against something else.
    ///
    /// ```
    /// # use std::time::Duration;
    /// # use majordomus_cli::bench::{BenchmarkResult, ResultDocument, Statistics, SystemTarget,
    /// #     TargetKind, RESULT_SCHEMA};
    /// # use majordomus_cli::bench::results::{CacheMode, Provenance};
    /// # fn document(us: u64) -> ResultDocument {
    /// #     ResultDocument {
    /// #         schema: RESULT_SCHEMA.into(),
    /// #         finished_at: "2026-09-10T12:00:00Z".into(),
    /// #         profile: "ci".into(),
    /// #         provenance: Provenance {
    /// #             commit: None, dirty: false, build_profile: "release".into(),
    /// #             os: "macos".into(), arch: "aarch64".into(), version: "0.5.0".into(),
    /// #             registry_fingerprint: "abc".into(), host: "Example CPU/8".into(),
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
    /// let run = document(1_000);
    /// assert!(run.find(SystemTarget::McpPing.key(), CacheMode::NotApplicable).is_some());
    /// // the same key in a mode it was not measured in is not a near miss
    /// assert!(run.find(SystemTarget::McpPing.key(), CacheMode::Warm).is_none());
    /// assert!(run.find("no.such.target", CacheMode::NotApplicable).is_none());
    /// ```
    pub fn find(&self, key: &str, mode: CacheMode) -> Option<&BenchmarkResult> {
        self.results
            .iter()
            .find(|r| r.key == key && r.cache_mode == mode)
    }
}

/// Write a file through a temporary neighbour and a rename, so that a reader never sees
/// half a file.
///
/// Both a local result and an accepted baseline go through here. The neighbour is in the
/// same directory on purpose — a rename is only atomic within a filesystem — and it is the
/// reason an interrupted run leaves either the previous document or the new one, never a
/// truncated file that fails to parse the next time somebody compares against it.
///
/// ```
/// use majordomus_cli::bench::results::write_atomic;
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("baseline.macos-aarch64-release.json");
/// write_atomic(&path, "{\"schema\":\"x\"}\n").unwrap();
/// assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"schema\":\"x\"}\n");
/// // it replaces, and it leaves no temporary neighbour behind
/// write_atomic(&path, "{\"schema\":\"y\"}\n").unwrap();
/// assert_eq!(std::fs::read_to_string(&path).unwrap(), "{\"schema\":\"y\"}\n");
/// assert!(!path.with_extension("json.tmp").exists());
/// ```
pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}
