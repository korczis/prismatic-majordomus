//! The result document: a versioned record of one benchmark run, with the provenance
//! that makes it comparable (commit, dirty state, build profile, platform, registry
//! fingerprint, profile) and, per target, the statistics of each cache mode measured.
//! Local runs are written under the checkout's local half; the accepted baseline is a
//! tracked file of the same shape, minus the machine-local fields, promoted explicitly.
//!
//! One shape for both halves is what makes a check possible at all: a baseline is a run
//! that was promoted, so comparing them is looking up a key in a document of the same
//! type, never translating between two.
//!
//! ```
//! use std::time::Duration;
//! use majordomus_cli::bench::results::CacheMode;
//! use majordomus_cli::bench::{
//!     BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA,
//! };
//!
//! let samples: Vec<Duration> = (100..=180).map(Duration::from_micros).collect();
//! let document = ResultDocument {
//!     schema: RESULT_SCHEMA.into(),
//!     finished_at: "2026-01-01T00:00:00Z".into(),
//!     profile: "quick".into(),
//!     provenance: serde_json::from_value(serde_json::json!({
//!         "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
//!         "version": "0.0.0", "registry_fingerprint": "abc", "host": "a-cpu/8"
//!     }))
//!     .unwrap(),
//!     results: vec![BenchmarkResult {
//!         key: SystemTarget::McpPing.key().into(),
//!         kind: TargetKind::System { target: SystemTarget::McpPing },
//!         cache_mode: CacheMode::NotApplicable,
//!         stats: Statistics::of(&samples),
//!         handler_invocations: None,
//!     }],
//! };
//!
//! // Rendered and read back: what `bench baseline update` promotes is what was measured.
//! let promoted: ResultDocument = serde_json::from_str(&document.render()).unwrap();
//! assert_eq!(promoted, document);
//! assert_eq!(
//!     promoted.find(SystemTarget::McpPing.key(), CacheMode::NotApplicable).unwrap().stats.samples,
//!     81
//! );
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
/// It is part of a measurement's identity, not a note about it: a cached capability is
/// measured twice, cold and warm, and the two numbers answer different questions — what
/// the handler costs, and what a repeat costs. A check looks a baseline line up by key
/// *and* mode, so a cold run can never be compared against a warm baseline.
///
/// `Uncached` and `NotApplicable` say two different things, and the difference is worth
/// keeping: the first is a capability that declared no cache, the second is not a
/// capability call at all.
///
/// ```
/// use majordomus_cli::bench::results::CacheMode;
/// assert_ne!(CacheMode::Cold, CacheMode::Warm, "one key, two measurements");
/// assert_eq!(serde_json::to_value(CacheMode::NotApplicable).unwrap(), "not_applicable");
/// assert_eq!(serde_json::to_value(CacheMode::Uncached).unwrap(), "uncached");
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

/// One target's measurement: what was timed, under which cache mode, and the statistics.
///
/// `handler_invocations` is the one field that is evidence about the *benchmark* rather
/// than about the code: on the direct transport the executor's counter says how many times
/// the handler actually ran during the samples, so a warm measurement that claims a cache
/// hit can be checked against the number of calls that reached the handler. A warm run of
/// twenty samples that invoked the handler twenty times measured no cache.
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::bench::results::CacheMode;
/// use majordomus_cli::bench::{BenchmarkResult, Statistics, TargetKind, Transport};
/// use majordomus_cli::capability::CachePolicy;
///
/// let warm = BenchmarkResult {
///     key: "context.resolve|direct|root".into(),
///     kind: TargetKind::Capability {
///         id: "context.resolve".into(),
///         module: "context".into(),
///         transport: Transport::Direct,
///         case: "root".into(),
///         input: serde_json::json!({ "path": "." }),
///         cache: CachePolicy::Process { max_entries: 64, ttl_seconds: None },
///         tool: None,
///         route: None,
///     },
///     cache_mode: CacheMode::Warm,
///     stats: Statistics::of(&(1..=20).map(Duration::from_micros).collect::<Vec<_>>()),
///     handler_invocations: Some(1),
/// };
/// assert_eq!(warm.stats.samples, 20);
/// assert_eq!(warm.handler_invocations, Some(1), "twenty samples, one handler call: a cache");
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

/// The provenance a run carries: everything that has to match before two numbers may be
/// compared.
///
/// A wall-clock measurement means nothing on its own. Every field here is a reason a
/// comparison could be wrong rather than a fact about the code: a debug build is not a
/// release one, an ARM laptop is not an x86 runner, a dirty tree is not a commit, and a
/// registry whose fingerprint changed is not the same set of targets. The check reads them
/// and says so — it reports a fingerprint change, refuses to fail on another host's
/// baseline, and keeps baselines in separate files per [`platform`](Provenance::platform).
///
/// ```
/// use majordomus_cli::bench::results::Provenance;
///
/// let recorded: Provenance = serde_json::from_value(serde_json::json!({
///     "commit": "0f2a1c", "dirty": true, "build_profile": "debug",
///     "os": "macos", "arch": "aarch64", "version": "0.6.0",
///     "registry_fingerprint": "9d1", "host": "a-cpu/12"
/// }))
/// .unwrap();
/// assert_eq!(recorded.platform(), "macos-aarch64-debug");
/// assert!(recorded.dirty, "measured against uncommitted work, and it says so");
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
    /// The provenance of this process against this repository: the build it was compiled
    /// as, the host it is running on, and what git says about the tree.
    ///
    /// Nothing here is asked of the caller, because every one of these is a thing a caller
    /// would get wrong or leave out. When git cannot answer — the directory is not a
    /// checkout, or the command is unavailable — the run is recorded as having no commit
    /// *and* as dirty: an unknown tree is treated as the worse of the two possibilities, so
    /// a baseline is never promoted from a state nobody can return to.
    ///
    /// ```
    /// use majordomus_cli::bench::results::Provenance;
    /// use majordomus_cli::Repository;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
    /// std::fs::write(dir.path().join(".ai/manifest.yaml"),
    ///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
    /// let repo = Repository::discover(dir.path()).unwrap();
    ///
    /// let provenance = Provenance::of(&repo, "fingerprint-of-the-registry");
    /// assert_eq!(provenance.registry_fingerprint, "fingerprint-of-the-registry");
    /// assert_eq!(provenance.os, std::env::consts::OS);
    /// assert!(!provenance.host.is_empty(), "the host is always named, however coarsely");
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

    /// The platform key baselines are kept by.
    ///
    /// It names the file — `baseline.<platform>.json` — so two platforms keep two baselines
    /// side by side and neither overwrites the other. The build profile is part of it
    /// because a debug build is a different program: comparing it with a release baseline
    /// would report a regression of several hundred percent on every line.
    ///
    /// ```
    /// use majordomus_cli::bench::results::Provenance;
    /// # fn p(os: &str, arch: &str, profile: &str) -> Provenance {
    /// #     serde_json::from_value(serde_json::json!({
    /// #         "dirty": false, "build_profile": profile, "os": os, "arch": arch,
    /// #         "version": "0.0.0", "registry_fingerprint": "f", "host": "h/1"
    /// #     })).unwrap()
    /// # }
    /// assert_eq!(p("linux", "x86_64", "release").platform(), "linux-x86_64-release");
    /// assert_ne!(
    ///     p("linux", "x86_64", "debug").platform(),
    ///     p("linux", "x86_64", "release").platform(),
    ///     "one machine, two programs, two baselines"
    /// );
    /// ```
    pub fn platform(&self) -> String {
        format!("{}-{}-{}", self.os, self.arch, self.build_profile)
    }

    /// Were these two measured on the same host? Unknown hosts never match.
    ///
    /// The asymmetry is deliberate: an empty host is not equal to another empty host, so a
    /// pair of baselines from before the field existed compares as *not* comparable rather
    /// than as a match. Being wrong here fails a pull request on somebody else's slower
    /// runner, so the unknown case is resolved towards reporting rather than gating.
    ///
    /// ```
    /// use majordomus_cli::bench::results::Provenance;
    /// # fn host(host: &str) -> Provenance {
    /// #     serde_json::from_value(serde_json::json!({
    /// #         "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    /// #         "version": "0.0.0", "registry_fingerprint": "f", "host": host
    /// #     })).unwrap()
    /// # }
    /// assert!(host("a-cpu/8").same_host(&host("a-cpu/8")));
    /// assert!(!host("a-cpu/8").same_host(&host("a-cpu/16")), "same chip, twice the cores");
    /// assert!(!host("").same_host(&host("")), "two unknowns are not one machine");
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
/// The core count is in the name because it is in the measurement: the same chip in a
/// four-core container and on an eighteen-core laptop schedules a benchmark differently,
/// and a baseline recorded on one is not a baseline for the other.
///
/// ```
/// use majordomus_cli::bench::results::host_fingerprint;
/// let host = host_fingerprint();
/// assert!(!host.is_empty(), "never empty: an empty host is what `same_host` reads as unknown");
/// assert_eq!(host, host_fingerprint(), "stable within a process");
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

/// One run: when it finished, what it was, where it was measured, and every measurement.
///
/// It is the same document whether it sits in the checkout's local half as this morning's
/// run or in the tracked half as the accepted baseline — promotion copies it, it is not
/// converted. `results` is flat and keyed by target *and* cache mode rather than nested,
/// because that is how a check reads it: one lookup per comparison, and a key the baseline
/// no longer has is an absence a check can name rather than a branch it takes.
///
/// ```
/// use std::time::Duration;
/// use majordomus_cli::bench::results::CacheMode;
/// use majordomus_cli::bench::{
///     BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA,
/// };
/// # fn result(key: &str, mode: CacheMode, us: u64) -> BenchmarkResult {
/// #     BenchmarkResult {
/// #         key: key.into(),
/// #         kind: TargetKind::System { target: SystemTarget::McpPing },
/// #         cache_mode: mode,
/// #         stats: Statistics::of(&[Duration::from_micros(us)]),
/// #         handler_invocations: None,
/// #     }
/// # }
/// let document = ResultDocument {
///     schema: RESULT_SCHEMA.into(),
///     finished_at: "2026-01-01T00:00:00Z".into(),
///     profile: "quick".into(),
///     provenance: serde_json::from_value(serde_json::json!({
///         "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
///         "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
///     }))
///     .unwrap(),
///     results: vec![
///         result("plan.show|direct|one", CacheMode::Cold, 400),
///         result("plan.show|direct|one", CacheMode::Warm, 12),
///     ],
/// };
///
/// // One key, two measurements: the mode is part of the identity, not a label on it.
/// assert_eq!(document.results.len(), 2);
/// assert_eq!(document.find("plan.show|direct|one", CacheMode::Warm).unwrap().stats.max_us, 12.0);
/// assert_eq!(document.schema, "majordomus/benchmark-result/v1");
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
    /// Local, and never the tracked half: a run is one machine's observation, and the file
    /// name carries the two things that distinguish two of them — when it finished and
    /// which profile it used — so runs accumulate instead of overwriting each other. The
    /// timestamp loses its punctuation because the name is a file name, not a date; it
    /// keeps its order.
    ///
    /// ```
    /// use majordomus_cli::bench::{ResultDocument, RESULT_SCHEMA};
    /// use majordomus_cli::Repository;
    ///
    /// let dir = tempfile::tempdir().unwrap();
    /// std::fs::create_dir_all(dir.path().join(".ai/repo")).unwrap();
    /// std::fs::write(dir.path().join(".ai/manifest.yaml"),
    ///     "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n").unwrap();
    /// let repo = Repository::discover(dir.path()).unwrap();
    ///
    /// let document = ResultDocument {
    ///     schema: RESULT_SCHEMA.into(),
    ///     finished_at: "2026-01-01T09:30:00Z".into(),
    ///     profile: "quick".into(),
    ///     provenance: serde_json::from_value(serde_json::json!({
    ///         "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    ///         "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
    ///     }))
    ///     .unwrap(),
    ///     results: Vec::new(),
    /// };
    ///
    /// let path = document.write_local(&repo).unwrap();
    /// assert!(path.ends_with(".ai/local/benchmarks/20260101T093000Z-quick.json"));
    /// let read_back: ResultDocument =
    ///     serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    /// assert_eq!(read_back, document);
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
    /// Pretty and newline-terminated because a baseline is a tracked file that people read
    /// in diffs: one measurement per set of lines, so a promotion shows which targets moved
    /// and by how much rather than one changed line thousands of characters wide.
    ///
    /// ```
    /// use majordomus_cli::bench::{ResultDocument, RESULT_SCHEMA};
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
    ///
    /// let text = document.render();
    /// assert!(text.ends_with("}\n"), "a tracked file ends with a newline");
    /// assert!(text.contains("\n  \"profile\": \"ci\""), "indented, so a diff is readable");
    /// assert_eq!(serde_json::from_str::<ResultDocument>(&text).unwrap(), document);
    /// ```
    pub fn render(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).unwrap_or_default();
        s.push('\n');
        s
    }

    /// One result by key and cache mode; `None` when this run did not measure it.
    ///
    /// Both halves of the lookup matter. A check asks the baseline for the run's key *and*
    /// mode, so a cold measurement never finds a warm baseline to compare itself with; and
    /// the `None` is what a check turns into a `NEW` or `STALE` line, which is the whole
    /// reason a missing target is reported instead of quietly skipped.
    ///
    /// ```
    /// use std::time::Duration;
    /// use majordomus_cli::bench::results::CacheMode;
    /// use majordomus_cli::bench::{
    ///     BenchmarkResult, ResultDocument, Statistics, SystemTarget, TargetKind, RESULT_SCHEMA,
    /// };
    ///
    /// let document = ResultDocument {
    ///     schema: RESULT_SCHEMA.into(),
    ///     finished_at: "2026-01-01T00:00:00Z".into(),
    ///     profile: "quick".into(),
    ///     provenance: serde_json::from_value(serde_json::json!({
    ///         "dirty": false, "build_profile": "release", "os": "linux", "arch": "x86_64",
    ///         "version": "0.0.0", "registry_fingerprint": "f", "host": "a-cpu/8"
    ///     }))
    ///     .unwrap(),
    ///     results: vec![BenchmarkResult {
    ///         key: SystemTarget::McpPing.key().into(),
    ///         kind: TargetKind::System { target: SystemTarget::McpPing },
    ///         cache_mode: CacheMode::NotApplicable,
    ///         stats: Statistics::of(&[Duration::from_micros(90)]),
    ///         handler_invocations: None,
    ///     }],
    /// };
    ///
    /// assert!(document.find(SystemTarget::McpPing.key(), CacheMode::NotApplicable).is_some());
    /// assert!(
    ///     document.find(SystemTarget::McpPing.key(), CacheMode::Warm).is_none(),
    ///     "the same key under another mode is another measurement, not this one"
    /// );
    /// assert!(document.find("system.http.openapi", CacheMode::NotApplicable).is_none());
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
/// The neighbour is in the same directory on purpose: a rename is atomic only within a
/// file system, so a temporary file in the system's temp directory would degrade to a copy
/// and lose the property this exists for. A baseline is read by CI while it is being
/// promoted, and half a JSON document is a parse error nobody can reproduce.
///
/// ```
/// use majordomus_cli::bench::results::write_atomic;
///
/// let dir = tempfile::tempdir().unwrap();
/// let path = dir.path().join("baseline.linux-x86_64-release.json");
/// write_atomic(&path, "{\"schema\":\"majordomus/benchmark-result/v1\"}\n").unwrap();
/// assert!(std::fs::read_to_string(&path).unwrap().ends_with("\n"));
///
/// // The neighbour it wrote through is gone; only the finished file is left behind.
/// let left: Vec<_> = std::fs::read_dir(dir.path())
///     .unwrap()
///     .map(|e| e.unwrap().file_name())
///     .collect();
/// assert_eq!(left.len(), 1, "no .tmp neighbour survives a successful write");
/// ```
pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, content).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| Error::io(path, e))
}
