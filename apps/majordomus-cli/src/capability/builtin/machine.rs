//! The `machine` module: the build output this repository's builds leave on the machine,
//! swept on a schedule rather than only when a build happens to start (I2881).
//!
//! Every sweep this repository had ran inside a build: `lib/rust_bin.sh` removed idle private
//! build directories and stale incremental sessions when `bin/majordomus` or `scripts/derive`
//! built, and `scripts/reap-orphans --targets --reclaim` ran below a free-space mark. A machine
//! whose builds come from somewhere else — a derive offload, a remote coverage run, a session
//! that called cargo itself — never swept at all. On 2026-10-18 lundra reached 0 bytes free
//! twice in one morning with 87 GB in `~/.cargo/build-shared/majordomus` (48 GB of it
//! `llvm-cov-target`) and 19 GB in `~/.cargo/build-private`, and nothing on the machine was
//! going to remove any of it. The owner ordered disk space solved systemically on every
//! machine.
//!
//! Two capabilities:
//!
//! - `machine.sweep` classifies what cargo left under every cargo cache home of this machine —
//!   whole private build directories, a coverage run's `llvm-cov-target` inside a shared build
//!   directory, and idle incremental sessions — and, when asked to reclaim, removes the ones
//!   nothing is using. A check by default, because the alternative deletes files.
//! - `machine.schedule` reports, installs or removes the per-user scheduler entry that runs the
//!   sweep every fifteen minutes: a launchd agent on macOS, a systemd `--user` timer on Linux.
//!
//! The sweep fails closed. A directory whose age cannot be read is `unmeasurable`; one written
//! within the idle threshold is `recent`; one a running process names is `in_use`; one whose
//! cargo lock cannot be taken is `held`; only the rest are `reclaimable`, and each is asked
//! again immediately before it is renamed out of cargo's reach and removed. A process table
//! that cannot be read keeps everything.
//!
//! ```
//! use majordomus_cli::capability::builtin::machine::module;
//! let m = module();
//! let ids: Vec<&str> = m.capabilities.iter().map(|c| c.capability.id.as_str()).collect();
//! assert_eq!(ids, ["machine.sweep", "machine.schedule"]);
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::capability::benchmark::{BenchmarkCases, CaseContext, NamedCase};
use crate::capability::handler::{CapabilityError, Context};
use crate::capability::model::{CapabilityKind, CliExposure};
use crate::capability::model::{Exposure, Stability};
use crate::capability::module::ModuleDescriptor;
use crate::{capability, module};

use super::{mcp, post};

/// Minutes nothing in a private build directory or a coverage target may have been written
/// before the sweep reclaims it, unless the call names another threshold.
pub const DEFAULT_IDLE_MINUTES: u64 = 120;

/// Minutes an incremental session must have been idle before it is reclaimed: rustc writes a
/// live session continuously, and a session idle this long belongs to no running compilation.
pub const INCREMENTAL_IDLE_MINUTES: u64 = 30;

/// Below this many GiB free on a cargo cache home's volume, the sweep is below the floor and
/// reclaims at [`INCREMENTAL_IDLE_MINUTES`] for everything: a machine about to stop is worth a
/// cold build.
pub const DEFAULT_FLOOR_GIB: u64 = 30;

/// How often the scheduler runs the sweep, in minutes.
pub const SCHEDULE_EVERY_MINUTES: u64 = 15;

/// The launchd label and the systemd unit's stem.
pub const SCHEDULE_LABEL: &str = "dev.majordomus.sweep";
const SYSTEMD_STEM: &str = "majordomus-sweep";

fn yes() -> bool {
    true
}

// ---------------------------------------------------------------- the sweep's vocabulary

/// What a candidate is: a whole private build directory, a coverage run's target inside a
/// shared build directory, or one incremental session — each with its own idle threshold and
/// its own way of being held by a build.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::BuildSweepKind;
/// let k: BuildSweepKind = serde_json::from_str("\"coverage_target\"").unwrap();
/// assert_eq!(k, BuildSweepKind::CoverageTarget);
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BuildSweepKind {
    /// A whole directory under `<cargo home>/build-private/`: one worktree's own build, made
    /// when the shared build directory was busy.
    PrivateBuildDir,
    /// A coverage run's `llvm-cov-target` inside a shared build directory; every coverage run
    /// removes it when it starts, so an idle one is only space.
    CoverageTarget,
    /// One `s-*` session of a profile's `incremental/` directory in a shared build directory.
    IncrementalSession,
}

/// What the sweep decided about a candidate.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::BuildSweepVerdict;
/// assert_eq!(serde_json::to_string(&BuildSweepVerdict::InUse).unwrap(), "\"in_use\"");
/// ```
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum BuildSweepVerdict {
    /// Idle past the threshold, named by no process and held by no build.
    Reclaimable,
    /// Written within the idle threshold.
    Recent,
    /// A running process names it, or the process table could not be read.
    InUse,
    /// Its cargo lock is held by a build, or the lock could not be opened.
    Held,
    /// Its age could not be read, so it is never a candidate.
    Unmeasurable,
}

/// One candidate and what was done with it: its path, what it is, the verdict, how long it has
/// been idle, its size, and whether this call removed it.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::{BuildSweepItem, BuildSweepVerdict};
/// let i: BuildSweepItem = serde_json::from_value(serde_json::json!({
///     "path": "/h/build-private/a", "kind": "private_build_dir", "verdict": "recent",
///     "idle_minutes": 5, "bytes": 10, "removed": false
/// })).unwrap();
/// assert_eq!(i.verdict, BuildSweepVerdict::Recent);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuildSweepItem {
    /// The directory.
    pub path: String,
    /// What it is.
    pub kind: BuildSweepKind,
    /// What the sweep decided.
    pub verdict: BuildSweepVerdict,
    /// Minutes since anything in it was written; absent when that could not be read.
    pub idle_minutes: Option<u64>,
    /// Its size in bytes, as far as it could be read.
    pub bytes: u64,
    /// Whether this call removed it.
    pub removed: bool,
}

/// The input of `machine.sweep`: whether it is a check, the idle threshold, the free-space
/// floor, and the cargo cache homes to sweep when not the machine's own.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::BuildSweepInput;
/// let i: BuildSweepInput = serde_json::from_str("{}").unwrap();
/// assert!(i.check, "a check unless asked to reclaim");
/// assert!(i.cargo_homes.is_empty());
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildSweepInput {
    /// Classify and report, and remove nothing. The default, because the alternative deletes.
    #[serde(default = "yes")]
    pub check: bool,
    /// Minutes a private build directory or a coverage target must have been idle (default
    /// 120). Below the floor the sweep uses 30 whatever this says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_minutes: Option<u64>,
    /// GiB free below which the sweep is below the floor (default 30).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_gib: Option<u64>,
    /// The cargo cache homes to sweep; by default `$CARGO_HOME`, `~/.cargo` and every
    /// `~/.asdf/installs/rust/*` that exists.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cargo_homes: Vec<String>,
}

impl BenchmarkCases for BuildSweepInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // Every case is a check: a case is executed against this machine by `majordomus
        // bench`, and a case that reclaimed would delete the machine's build caches.
        vec![
            NamedCase::new(
                "check-a-named-home",
                BuildSweepInput {
                    check: true,
                    cargo_homes: vec!["/nonexistent/cargo-home".into()],
                    ..BuildSweepInput::default()
                },
            ),
            NamedCase::new(
                "check-below-a-high-floor",
                BuildSweepInput {
                    check: true,
                    idle_minutes: Some(DEFAULT_IDLE_MINUTES),
                    floor_gib: Some(1_000_000),
                    cargo_homes: vec!["/nonexistent/cargo-home".into()],
                },
            ),
        ]
    }
}

/// What one sweep found and did: the homes it covered, the free space before and after,
/// whether it ran below the floor and at which threshold, every candidate, and the bytes it
/// removed.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::BuildSweepReport;
/// let r: BuildSweepReport = serde_json::from_value(serde_json::json!({
///     "check": true, "homes": [], "free_before_bytes": null, "free_after_bytes": null,
///     "below_floor": false, "idle_minutes": 120, "items": [], "reclaimed_bytes": 0
/// })).unwrap();
/// assert!(r.check && r.items.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BuildSweepReport {
    /// Whether this was a check (nothing removed).
    pub check: bool,
    /// The cargo cache homes swept.
    pub homes: Vec<String>,
    /// The fewest bytes free on any swept home's volume before the sweep, when readable.
    pub free_before_bytes: Option<u64>,
    /// The same after the sweep.
    pub free_after_bytes: Option<u64>,
    /// Whether the volume was below the floor, so everything was swept at 30 minutes.
    pub below_floor: bool,
    /// The idle threshold applied to private build directories and coverage targets.
    pub idle_minutes: u64,
    /// Every candidate, in canonical order.
    pub items: Vec<BuildSweepItem>,
    /// Bytes this call removed.
    pub reclaimed_bytes: u64,
}

// ---------------------------------------------------------------- the sweep

/// The thresholds one sweep applies: how long a private build directory or a coverage target,
/// and separately an incremental session, must have gone unwritten before it can be a
/// candidate at all. Below the floor the first threshold drops to the second.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::SweepPolicy;
/// let normal = SweepPolicy::new(120, false);
/// let pressed = SweepPolicy::new(120, true);
/// assert!(pressed.idle < normal.idle);
/// assert_eq!(pressed.incremental_idle, normal.incremental_idle);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepPolicy {
    /// Seconds for private build directories and coverage targets.
    pub idle: Duration,
    /// Seconds for incremental sessions.
    pub incremental_idle: Duration,
}

impl SweepPolicy {
    /// The policy for a threshold in minutes, below the floor or not.
    ///
    /// ```
    /// use majordomus_cli::capability::builtin::machine::SweepPolicy;
    /// let p = SweepPolicy::new(120, false);
    /// assert_eq!(p.idle.as_secs(), 7200);
    /// assert_eq!(SweepPolicy::new(120, true).idle.as_secs(), 1800, "below the floor");
    /// ```
    pub fn new(idle_minutes: u64, below_floor: bool) -> SweepPolicy {
        let minutes = if below_floor {
            idle_minutes.min(INCREMENTAL_IDLE_MINUTES)
        } else {
            idle_minutes
        };
        SweepPolicy {
            idle: Duration::from_secs(minutes * 60),
            incremental_idle: Duration::from_secs(INCREMENTAL_IDLE_MINUTES * 60),
        }
    }
}

/// The cargo cache homes of this machine: an explicit `CARGO_HOME`, `~/.cargo`, and every
/// toolchain home asdf keeps, each that exists, once, in canonical order.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::cargo_homes;
/// let home = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(home.path().join(".cargo")).unwrap();
/// std::fs::create_dir_all(home.path().join(".asdf/installs/rust/1.90.0")).unwrap();
/// let found = cargo_homes(Some(home.path()), None);
/// assert_eq!(found.len(), 2);
/// ```
pub fn cargo_homes(home: Option<&Path>, cargo_home: Option<&Path>) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf| {
        if let Ok(c) = p.canonicalize() {
            if c.is_dir() && !found.contains(&c) {
                found.push(c);
            }
        }
    };
    if let Some(c) = cargo_home {
        push(c.to_path_buf());
    }
    if let Some(h) = home {
        push(h.join(".cargo"));
        for d in subdirs(&h.join(".asdf/installs/rust")) {
            push(d);
        }
    }
    let mut names: Vec<String> = found.iter().map(|p| p.display().to_string()).collect();
    crate::order::canonical_strings(&mut names);
    names.into_iter().map(PathBuf::from).collect()
}

/// The real directories directly under `dir`, never a symlink; none when it cannot be read.
fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = rd
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    let mut names: Vec<String> = out.iter().map(|p| p.display().to_string()).collect();
    crate::order::canonical_strings(&mut names);
    out = names.into_iter().map(PathBuf::from).collect();
    out
}

/// What cargo left under one cargo cache home that the sweep considers.
fn candidates(home: &Path) -> Vec<(PathBuf, BuildSweepKind)> {
    let mut out = Vec::new();
    for d in subdirs(&home.join("build-private")) {
        out.push((d, BuildSweepKind::PrivateBuildDir));
    }
    for shared in subdirs(&home.join("build-shared")) {
        let cov = shared.join("llvm-cov-target");
        if cov.is_dir() && !is_symlink(&cov) {
            out.push((cov, BuildSweepKind::CoverageTarget));
        }
        for profile in subdirs(&shared) {
            if profile.ends_with("llvm-cov-target") {
                continue;
            }
            for krate in subdirs(&profile.join("incremental")) {
                for session in subdirs(&krate) {
                    let named = session.file_name().and_then(|n| n.to_str());
                    if named.is_some_and(|n| n.starts_with("s-")) {
                        out.push((session, BuildSweepKind::IncrementalSession));
                    }
                }
            }
        }
    }
    out
}

fn is_symlink(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink())
}

/// The newest modification time of anything under `p` and its total size; `None` when any
/// entry could not be read, because a partial walk could miss the one recent file.
fn newest_and_size(p: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::symlink_metadata(p).ok()?;
    let mut newest = meta.modified().ok()?;
    let mut size = meta.len();
    if meta.is_dir() {
        for entry in std::fs::read_dir(p).ok()? {
            let (t, s) = newest_and_size(&entry.ok()?.path())?;
            newest = newest.max(t);
            size += s;
        }
    }
    Some((newest, size))
}

/// Whether a build holds `p`: any cargo or rustc lock file at or just below it that cannot be
/// locked exclusively at once. A lock file that cannot be opened counts as held.
fn held(p: &Path, kind: BuildSweepKind) -> bool {
    let mut locks: Vec<PathBuf> = Vec::new();
    match kind {
        BuildSweepKind::IncrementalSession => {
            // rustc works in `s-<time>-<random>-working` and finalizes it as
            // `s-<time>-<random>-<svh>`, locking it through `s-<time>-<random>.lock`
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if name.ends_with("-working") {
                return true;
            }
            let stem = name.rsplit_once('-').map_or(name, |(s, _)| s);
            locks.push(p.with_file_name(format!("{stem}.lock")));
        }
        BuildSweepKind::PrivateBuildDir | BuildSweepKind::CoverageTarget => {
            let mut dirs = vec![p.to_path_buf()];
            dirs.extend(subdirs(p));
            for d in dirs {
                for name in [".cargo-lock", ".cargo-build-lock"] {
                    locks.push(d.join(name));
                }
            }
        }
    }
    locks.iter().filter(|l| l.exists()).any(|l| !lockable(l))
}

/// Whether an exclusive lock on `path` can be taken without waiting; released at once.
fn lockable(path: &Path) -> bool {
    use std::os::fd::AsRawFd;
    let Ok(f) = std::fs::OpenOptions::new().read(true).open(path) else {
        return false;
    };
    // SAFETY: flock on a file descriptor this function owns for the duration of the call.
    let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    rc == 0
}

/// Whether a running process names `p`; with no process table, every path is in use. A
/// coverage target is also in use while any coverage run is.
fn in_use(p: &Path, kind: BuildSweepKind, processes: Option<&[String]>) -> bool {
    let Some(lines) = processes else {
        return true;
    };
    let me = p.display().to_string();
    lines.iter().any(|l| {
        l.contains(&me) || (kind == BuildSweepKind::CoverageTarget && l.contains("llvm-cov"))
    })
}

/// The verdict on one candidate at `now`.
fn judge(
    p: &Path,
    kind: BuildSweepKind,
    policy: &SweepPolicy,
    processes: Option<&[String]>,
    now: SystemTime,
) -> (BuildSweepVerdict, Option<u64>, u64) {
    let Some((newest, bytes)) = newest_and_size(p) else {
        return (BuildSweepVerdict::Unmeasurable, None, 0);
    };
    let idle = now.duration_since(newest).unwrap_or(Duration::ZERO);
    let threshold = match kind {
        BuildSweepKind::IncrementalSession => policy.incremental_idle,
        BuildSweepKind::PrivateBuildDir | BuildSweepKind::CoverageTarget => policy.idle,
    };
    let verdict = if idle < threshold {
        BuildSweepVerdict::Recent
    } else if in_use(p, kind, processes) {
        BuildSweepVerdict::InUse
    } else if held(p, kind) {
        BuildSweepVerdict::Held
    } else {
        BuildSweepVerdict::Reclaimable
    };
    (verdict, Some(idle.as_secs() / 60), bytes)
}

/// Move `p` out of cargo's reach, then remove it: a build that starts in between finds no
/// directory and makes a fresh one, never half of an old one.
fn remove(p: &Path) -> bool {
    let Some(name) = p.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let aside = p.with_file_name(format!(".{name}.swept-{}", std::process::id()));
    std::fs::rename(p, &aside).is_ok() && std::fs::remove_dir_all(&aside).is_ok()
}

/// One sweep over `homes`: classify every candidate and, unless `check`, remove the ones that
/// are still reclaimable when asked a second time. Pure but for the filesystem: the process
/// table and the clock are the caller's.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::{sweep, SweepPolicy};
/// let home = tempfile::tempdir().unwrap();
/// std::fs::create_dir_all(home.path().join("build-private/a1/debug")).unwrap();
/// let later = std::time::SystemTime::now() + std::time::Duration::from_secs(3 * 3600);
/// let items = sweep(&[home.path().to_path_buf()], &SweepPolicy::new(120, false), Some(&[]), later, false);
/// assert!(items[0].removed, "idle for three hours, named and held by nothing");
/// ```
pub fn sweep(
    homes: &[PathBuf],
    policy: &SweepPolicy,
    processes: Option<&[String]>,
    now: SystemTime,
    check: bool,
) -> Vec<BuildSweepItem> {
    let mut items = Vec::new();
    for home in homes {
        let mut found = candidates(home);
        // a coverage target swept whole takes its sessions with it
        let mut gone: Vec<PathBuf> = Vec::new();
        for (path, kind) in found.drain(..) {
            if gone.iter().any(|g| path.starts_with(g)) {
                continue;
            }
            let (verdict, idle_minutes, bytes) = judge(&path, kind, policy, processes, now);
            let mut removed = false;
            if !check && verdict == BuildSweepVerdict::Reclaimable {
                // asked again: a build may have started since the first look
                let (again, ..) = judge(&path, kind, policy, processes, now);
                removed = again == BuildSweepVerdict::Reclaimable && remove(&path);
                if removed {
                    gone.push(path.clone());
                }
            }
            items.push(BuildSweepItem {
                path: path.display().to_string(),
                kind,
                verdict,
                idle_minutes,
                bytes,
                removed,
            });
        }
    }
    // homes in canonical order, and each home walked by canonical name: the order is the walk's
    items
}

/// Bytes free to an unprivileged user on the volume holding `p`.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::free_bytes;
/// assert!(free_bytes(std::path::Path::new("/")).is_some());
/// assert!(free_bytes(std::path::Path::new("/no/such/volume/here")).is_none());
/// ```
// the field types are u32 on one platform and u64 on another, so the conversion is a no-op on
// one of them
#[allow(clippy::useless_conversion)]
pub fn free_bytes(p: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
    // SAFETY: statvfs writes into a zeroed struct this function owns.
    let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c.as_ptr(), &mut s) };
    (rc == 0).then(|| u64::from(s.f_bavail).saturating_mul(u64::from(s.f_frsize)))
}

/// The command lines of every other running process; `None` when the table cannot be read.
fn process_table() -> Option<Vec<String>> {
    let out = std::process::Command::new("ps")
        .args(["-axww", "-o", "pid=,args="])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| own_excluded(&String::from_utf8_lossy(&out.stdout)))
}

/// The process lines minus this process's own: the sweep's own command line names the homes.
fn own_excluded(table: &str) -> Vec<String> {
    let me = std::process::id().to_string();
    table
        .lines()
        .map(str::trim)
        .filter(|l| l.split_whitespace().next() != Some(me.as_str()))
        .map(str::to_string)
        .collect()
}

/// The fewest free bytes over the volumes of `homes`, when any could be read.
fn least_free(homes: &[PathBuf]) -> Option<u64> {
    homes.iter().filter_map(|h| free_bytes(h)).min()
}

/// The sweep with every input made explicit: what `machine.sweep` runs.
pub(crate) fn run_sweep(
    input: &BuildSweepInput,
    homes: Vec<PathBuf>,
    processes: Option<&[String]>,
    now: SystemTime,
) -> BuildSweepReport {
    let floor = input
        .floor_gib
        .unwrap_or(DEFAULT_FLOOR_GIB)
        .saturating_mul(1 << 30);
    let free_before = least_free(&homes);
    let below_floor = free_before.is_some_and(|f| f < floor);
    let idle_minutes = input.idle_minutes.unwrap_or(DEFAULT_IDLE_MINUTES);
    let policy = SweepPolicy::new(idle_minutes, below_floor);
    let items = sweep(&homes, &policy, processes, now, input.check);
    let reclaimed_bytes = items.iter().filter(|i| i.removed).map(|i| i.bytes).sum();
    BuildSweepReport {
        check: input.check,
        homes: homes.iter().map(|h| h.display().to_string()).collect(),
        free_before_bytes: free_before,
        free_after_bytes: least_free(&homes),
        below_floor,
        idle_minutes: policy.idle.as_secs() / 60,
        items,
        reclaimed_bytes,
    }
}

/// The homes a sweep covers: the ones named, or else every cargo cache home `HOME` and
/// `CARGO_HOME` lead to.
fn homes_for(
    input: &BuildSweepInput,
    home: Option<PathBuf>,
    cargo_home: Option<PathBuf>,
) -> Vec<PathBuf> {
    if input.cargo_homes.is_empty() {
        cargo_homes(home.as_deref(), cargo_home.as_deref())
    } else {
        input.cargo_homes.iter().map(PathBuf::from).collect()
    }
}

fn machine_sweep(_: &Context, input: BuildSweepInput) -> Result<BuildSweepReport, CapabilityError> {
    let homes = homes_for(
        &input,
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os("CARGO_HOME").map(PathBuf::from),
    );
    let processes = process_table();
    Ok(run_sweep(
        &input,
        homes,
        processes.as_deref(),
        SystemTime::now(),
    ))
}

// ---------------------------------------------------------------- the schedule

/// What `machine.schedule` is asked to do.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::SweepScheduleAction;
/// assert_eq!(SweepScheduleAction::default(), SweepScheduleAction::Status);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SweepScheduleAction {
    /// Report whether the scheduler entry is there, and change nothing.
    #[default]
    Status,
    /// Write the scheduler entry and, unless told not to, load it.
    Install,
    /// Unload the scheduler entry and remove its files.
    Remove,
}

/// The input of `machine.schedule`: whether to report, install or remove the per-user
/// scheduler entry, the floor the scheduled sweep runs with, the program it starts, and
/// whether launchctl or systemctl is asked to load or unload it.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::{SweepScheduleAction, SweepScheduleInput};
/// let i: SweepScheduleInput = serde_json::from_str("{}").unwrap();
/// assert_eq!(i.action, SweepScheduleAction::Status);
/// assert!(i.activate);
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SweepScheduleInput {
    /// `status` (the default), `install` or `remove`.
    #[serde(default)]
    pub action: SweepScheduleAction,
    /// The floor the scheduled sweep runs with, in GiB (default 30).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_gib: Option<u64>,
    /// The executable the scheduler starts; by default the `majordomus` on PATH, so that an
    /// upgrade does not leave the entry pointing at a removed version.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub program: Option<String>,
    /// Load or unload the entry with launchctl or systemctl after writing it (default true).
    #[serde(default = "yes")]
    pub activate: bool,
}

impl BenchmarkCases for SweepScheduleInput {
    fn benchmark_cases(_: &CaseContext<'_>) -> Vec<NamedCase<Self>> {
        // status only: a case is executed on the machine running the benchmark
        vec![NamedCase::new(
            "status",
            SweepScheduleInput {
                action: SweepScheduleAction::Status,
                floor_gib: Some(DEFAULT_FLOOR_GIB),
                program: Some("majordomus".into()),
                activate: false,
            },
        )]
    }
}

/// What `machine.schedule` found or did: the scheduler this platform has, whether every file
/// of the entry is there after the call, those files, the program the entry starts, how
/// often it runs, and each scheduler command that was run with its outcome.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::SweepScheduleReport;
/// let r: SweepScheduleReport = serde_json::from_value(serde_json::json!({
///     "scheduler": "systemd", "installed": false, "files": [], "program": "majordomus",
///     "every_minutes": 15, "activation": []
/// })).unwrap();
/// assert!(!r.installed && r.activation.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SweepScheduleReport {
    /// `launchd`, `systemd`, or `none` on a platform with neither.
    pub scheduler: String,
    /// Whether every file of the entry is there after this call.
    pub installed: bool,
    /// The entry's files.
    pub files: Vec<String>,
    /// The executable the entry starts.
    pub program: String,
    /// Minutes between two sweeps.
    pub every_minutes: u64,
    /// Each scheduler command run, with its outcome.
    pub activation: Vec<String>,
}

/// Which scheduler a platform has, by the operating system's name as Rust spells it: launchd
/// on macOS, systemd on Linux, and none elsewhere, where the entry is refused rather than
/// written somewhere nothing reads.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::scheduler_for;
/// assert_eq!(scheduler_for("macos"), "launchd");
/// assert_eq!(scheduler_for("linux"), "systemd");
/// assert_eq!(scheduler_for("windows"), "none");
/// ```
pub fn scheduler_for(os: &str) -> &'static str {
    match os {
        "macos" => "launchd",
        "linux" => "systemd",
        _ => "none",
    }
}

/// The files of the scheduler entry, with their contents.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::entry_files;
/// use std::path::Path;
/// let f = entry_files("systemd", Path::new("/h"), "/bin/majordomus", Path::new("/r"), 30);
/// assert_eq!(f.len(), 2);
/// assert!(f[1].1.contains("OnUnitActiveSec=15min"));
/// assert!(entry_files("none", Path::new("/h"), "m", Path::new("/r"), 30).is_empty());
/// ```
pub fn entry_files(
    scheduler: &str,
    home: &Path,
    program: &str,
    workdir: &Path,
    floor_gib: u64,
) -> Vec<(PathBuf, String)> {
    let args = [
        "machine",
        "sweep",
        "--reclaim",
        "--floor-gib",
        &floor_gib.to_string(),
    ]
    .map(String::from);
    match scheduler {
        "launchd" => {
            let mut argv = format!("    <string>{}</string>\n", xml(program));
            for a in &args {
                argv.push_str(&format!("    <string>{}</string>\n", xml(a)));
            }
            let log = home.join("Library/Logs/majordomus-sweep.log");
            let plist = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
                 <plist version=\"1.0\">\n<dict>\n\
                 \x20 <key>Label</key><string>{SCHEDULE_LABEL}</string>\n\
                 \x20 <key>ProgramArguments</key>\n  <array>\n{argv}  </array>\n\
                 \x20 <key>WorkingDirectory</key><string>{}</string>\n\
                 \x20 <key>StartInterval</key><integer>{}</integer>\n\
                 \x20 <key>RunAtLoad</key><true/>\n\
                 \x20 <key>Nice</key><integer>10</integer>\n\
                 \x20 <key>StandardOutPath</key><string>{}</string>\n\
                 \x20 <key>StandardErrorPath</key><string>{}</string>\n\
                 </dict>\n</plist>\n",
                xml(&workdir.display().to_string()),
                SCHEDULE_EVERY_MINUTES * 60,
                xml(&log.display().to_string()),
                xml(&log.display().to_string()),
            );
            vec![(
                home.join("Library/LaunchAgents")
                    .join(format!("{SCHEDULE_LABEL}.plist")),
                plist,
            )]
        }
        "systemd" => {
            let dir = home.join(".config/systemd/user");
            let exec = std::iter::once(program.to_string())
                .chain(args)
                .map(|a| systemd_word(&a))
                .collect::<Vec<_>>()
                .join(" ");
            let service = format!(
                "[Unit]\nDescription=Sweep the build output this machine's majordomus builds left (I2881)\n\n\
                 [Service]\nType=oneshot\nWorkingDirectory={}\nExecStart={exec}\nNice=10\n",
                workdir.display()
            );
            let timer = format!(
                "[Unit]\nDescription=Run {SYSTEMD_STEM}.service every {SCHEDULE_EVERY_MINUTES} minutes\n\n\
                 [Timer]\nOnBootSec=5min\nOnUnitActiveSec={SCHEDULE_EVERY_MINUTES}min\nPersistent=true\n\n\
                 [Install]\nWantedBy=timers.target\n"
            );
            vec![
                (dir.join(format!("{SYSTEMD_STEM}.service")), service),
                (dir.join(format!("{SYSTEMD_STEM}.timer")), timer),
            ]
        }
        _ => Vec::new(),
    }
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn systemd_word(s: &str) -> String {
    if s.chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\\')
    {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

/// The scheduler commands that load or unload the entry.
fn activation_commands(scheduler: &str, files: &[PathBuf], load: bool) -> Vec<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    match (scheduler, files.first()) {
        ("launchd", Some(plist)) => {
            let p = plist.display().to_string();
            let mut cmds = vec![s(&["launchctl", "unload", &p])];
            if load {
                cmds.push(s(&["launchctl", "load", "-w", &p]));
            }
            cmds
        }
        ("systemd", Some(_)) => {
            let timer = format!("{SYSTEMD_STEM}.timer");
            if load {
                vec![
                    s(&["systemctl", "--user", "daemon-reload"]),
                    s(&["systemctl", "--user", "enable", "--now", &timer]),
                ]
            } else {
                vec![s(&["systemctl", "--user", "disable", "--now", &timer])]
            }
        }
        _ => Vec::new(),
    }
}

/// Run one scheduler command; the outcome as one line.
fn run_command(argv: &[String]) -> String {
    let outcome = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .output();
    let said = match outcome {
        Ok(o) if o.status.success() => "ok".to_string(),
        Ok(o) => format!("exit {}", o.status.code().unwrap_or(-1)),
        Err(e) => format!("not run: {e}"),
    };
    format!("{}: {said}", argv.join(" "))
}

/// The schedule with every input made explicit: what `machine.schedule` runs. `runner` runs
/// one scheduler command and says how it went.
pub(crate) fn run_schedule(
    input: &SweepScheduleInput,
    scheduler: &str,
    home: &Path,
    program: &str,
    workdir: &Path,
    runner: &mut dyn FnMut(&[String]) -> String,
) -> Result<SweepScheduleReport, CapabilityError> {
    let floor = input.floor_gib.unwrap_or(DEFAULT_FLOOR_GIB);
    let files = entry_files(scheduler, home, program, workdir, floor);
    let paths: Vec<PathBuf> = files.iter().map(|(p, _)| p.clone()).collect();
    let mut activation = Vec::new();
    match input.action {
        SweepScheduleAction::Status => {}
        SweepScheduleAction::Install => {
            if files.is_empty() {
                return Err(CapabilityError::Refused(
                    "this platform has no scheduler the sweep knows (launchd or systemd); run \
                     `majordomus machine sweep --reclaim` from the machine's own scheduler"
                        .into(),
                ));
            }
            for (path, text) in &files {
                let dir = path.parent().unwrap_or(home);
                std::fs::create_dir_all(dir)
                    .and_then(|()| std::fs::write(path, text))
                    .map_err(|e| {
                        CapabilityError::Refused(format!(
                            "{} could not be written: {e}",
                            path.display()
                        ))
                    })?;
            }
            if input.activate {
                for cmd in activation_commands(scheduler, &paths, true) {
                    activation.push(runner(&cmd));
                }
            }
        }
        SweepScheduleAction::Remove => {
            if input.activate {
                for cmd in activation_commands(scheduler, &paths, false) {
                    activation.push(runner(&cmd));
                }
            }
            for p in &paths {
                if p.exists() {
                    std::fs::remove_file(p).map_err(|e| {
                        CapabilityError::Refused(format!(
                            "{} could not be removed: {e}",
                            p.display()
                        ))
                    })?;
                }
            }
        }
    }
    Ok(SweepScheduleReport {
        scheduler: scheduler.to_string(),
        installed: !paths.is_empty() && paths.iter().all(|p| p.exists()),
        files: paths.iter().map(|p| p.display().to_string()).collect(),
        program: program.to_string(),
        every_minutes: SCHEDULE_EVERY_MINUTES,
        activation,
    })
}

/// The `majordomus` a scheduler should start: the first on `path`, else `fallback`.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::program_on_path;
/// let dir = tempfile::tempdir().unwrap();
/// assert_eq!(program_on_path(Some(dir.path().as_os_str()), "/x/majordomus"), "/x/majordomus");
/// std::fs::write(dir.path().join("majordomus"), "").unwrap();
/// assert!(program_on_path(Some(dir.path().as_os_str()), "/x").ends_with("majordomus"));
/// ```
pub fn program_on_path(path: Option<&std::ffi::OsStr>, fallback: &str) -> String {
    path.into_iter()
        .flat_map(std::env::split_paths)
        .map(|d| d.join("majordomus"))
        .find(|p| p.is_file())
        .map_or_else(|| fallback.to_string(), |p| p.display().to_string())
}

fn machine_schedule(
    ctx: &Context,
    input: SweepScheduleInput,
) -> Result<SweepScheduleReport, CapabilityError> {
    let home = std::env::var_os("HOME").map(PathBuf::from).ok_or_else(|| {
        CapabilityError::Refused("HOME is not set, so there is no user to schedule for".into())
    })?;
    let fallback = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "majordomus".into());
    let program = input
        .program
        .clone()
        .unwrap_or_else(|| program_on_path(std::env::var_os("PATH").as_deref(), &fallback));
    let workdir = PathBuf::from(&ctx.index.repository.root);
    let scheduler = scheduler_for(std::env::consts::OS);
    run_schedule(
        &input,
        scheduler,
        &home,
        &program,
        &workdir,
        &mut run_command,
    )
}

// ---------------------------------------------------------------- the module

fn cli(words: &[&str]) -> Option<CliExposure> {
    Some(CliExposure {
        path: words.iter().map(|w| w.to_string()).collect(),
    })
}

/// The module descriptor: the sweep and its schedule.
///
/// ```
/// use majordomus_cli::capability::builtin::machine::module;
/// let m = module();
/// assert_eq!(m.capabilities.len(), 2);
/// assert!(m.capabilities.iter().all(|c| c.capability.execution.needs_confirmation()));
/// ```
pub fn module() -> ModuleDescriptor {
    module! {
        id: "machine",
        title: "This machine's build output",
        description: "What this repository's builds leave on the machine they run on — private build directories, coverage targets, incremental sessions under every cargo cache home — classified and, when asked, reclaimed; and the per-user scheduler entry that runs that sweep every fifteen minutes on every machine, whether or not a build of this repository ever starts there (I2881).",
        stability: Stability::BehaviorallyVerified,
        capabilities: [
            capability! {
                id: "machine.sweep",
                kind: CapabilityKind::Command,
                title: "Classify the build output under every cargo cache home of this machine, and reclaim what nothing uses",
                description: "Walks every cargo cache home of this machine ($CARGO_HOME, ~/.cargo and each ~/.asdf/installs/rust/*, or the homes named): every directory under build-private/, every shared build directory's llvm-cov-target, and every incremental session. Each gets a verdict: unmeasurable when its age cannot be read, recent when written within the idle threshold (120 minutes, 30 for sessions), in_use when a running process names it (any coverage run, for a coverage target; every path when the process table cannot be read), held when its cargo lock cannot be taken, and otherwise reclaimable. A check by default; asked to reclaim, it asks each reclaimable directory again, renames it out of cargo's reach and removes it. Below the floor (30 GiB free by default) everything is swept at 30 minutes. Answers with the free space before and after, every candidate and the bytes reclaimed.",
                input: BuildSweepInput,
                output: BuildSweepReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_machine_sweep"),
                    http: post("/api/v1/machine/sweep"),
                    cli: cli(&["machine", "sweep"]),
                },
                tags: ["machine", "disk", "build", "maintenance"],
                handler: machine_sweep,
            }
            // It removes files, so every client asks first; the effect model names the
            // repository, and the build output it removes is this repository's builds'.
            .writes_repository(),
            capability! {
                id: "machine.schedule",
                kind: CapabilityKind::Command,
                title: "Report, install or remove the scheduler entry that sweeps this machine every fifteen minutes",
                description: "The per-user scheduler entry that runs `majordomus machine sweep --reclaim` every fifteen minutes from this checkout: a launchd agent (~/Library/LaunchAgents/dev.majordomus.sweep.plist) on macOS, a systemd --user service and timer (~/.config/systemd/user/majordomus-sweep.*) on Linux. `status` reports whether its files are there; `install` writes them and loads them with launchctl or systemctl, each command's outcome reported; `remove` unloads and deletes them. The entry starts the `majordomus` on PATH unless another program is named, so an upgrade does not leave it pointing at a removed version. Refused on a platform with neither scheduler.",
                input: SweepScheduleInput,
                output: SweepScheduleReport,
                stability: Stability::BehaviorallyVerified,
                exposure: Exposure {
                    mcp: mcp("majordomus_machine_schedule"),
                    http: post("/api/v1/machine/schedule"),
                    cli: cli(&["machine", "schedule"]),
                },
                tags: ["machine", "disk", "schedule", "maintenance"],
                handler: machine_schedule,
            }
            .writes_repository(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file at `rel` under `root`, written and dated `age` ago.
    fn aged(root: &Path, rel: &str, age: Duration) -> PathBuf {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, "x").unwrap();
        let t = SystemTime::now() - age;
        let f = std::fs::File::options().write(true).open(&p).unwrap();
        f.set_modified(t).unwrap();
        p
    }

    /// Date a directory and everything in it `age` ago.
    fn age_tree(p: &Path, age: Duration) {
        let t = SystemTime::now() - age;
        if p.is_dir() {
            for e in std::fs::read_dir(p).unwrap() {
                age_tree(&e.unwrap().path(), age);
            }
        }
        let f = std::fs::File::open(p).unwrap();
        f.set_modified(t).unwrap();
    }

    const H: Duration = Duration::from_secs(3600);

    /// The four verdicts a directory can earn, and that only a reclaimable one is removed:
    /// an idle private build directory goes, a recent one stays, one a process names stays,
    /// one whose cargo lock a build holds stays.
    #[test]
    fn only_an_idle_unnamed_unheld_build_directory_is_reclaimed() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path().canonicalize().unwrap();
        aged(&h, "build-private/idle/debug/deps/a", 3 * H);
        aged(
            &h,
            "build-private/recent/debug/deps/a",
            Duration::from_secs(60),
        );
        aged(&h, "build-private/named/debug/deps/a", 3 * H);
        aged(&h, "build-private/locked/debug/.cargo-lock", 3 * H);
        for d in ["idle", "named", "locked"] {
            age_tree(&h.join("build-private").join(d), 3 * H);
        }
        let lock = std::fs::File::open(h.join("build-private/locked/debug/.cargo-lock")).unwrap();
        // SAFETY: an exclusive flock the test holds for its duration.
        use std::os::fd::AsRawFd;
        assert_eq!(unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) }, 0);
        let ps = vec![format!(
            "4242 cargo build --target-dir {}",
            h.join("build-private/named").display()
        )];
        let report = run_sweep(
            &BuildSweepInput {
                check: false,
                idle_minutes: Some(120),
                floor_gib: Some(0),
                cargo_homes: Vec::new(),
            },
            vec![h.clone()],
            Some(ps.as_slice()),
            SystemTime::now(),
        );
        let verdict = |name: &str| {
            report
                .items
                .iter()
                .find(|i| i.path.ends_with(name))
                .map(|i| (i.verdict, i.removed))
                .unwrap()
        };
        assert_eq!(verdict("idle"), (BuildSweepVerdict::Reclaimable, true));
        assert_eq!(verdict("recent"), (BuildSweepVerdict::Recent, false));
        assert_eq!(verdict("named"), (BuildSweepVerdict::InUse, false));
        assert_eq!(verdict("locked"), (BuildSweepVerdict::Held, false));
        assert!(!h.join("build-private/idle").exists());
        assert!(h.join("build-private/recent").exists());
        assert!(report.reclaimed_bytes > 0);
        assert!(!report.below_floor);
        assert_eq!(report.idle_minutes, 120);
        drop(lock);
    }

    /// A check removes nothing; with no process table everything is in use; a coverage target
    /// is in use while any coverage run is, and swept whole with its sessions when idle;
    /// an idle incremental session goes and a recent one stays.
    #[test]
    fn a_check_removes_nothing_and_a_coverage_target_waits_for_every_coverage_run() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path().canonicalize().unwrap();
        aged(
            &h,
            "build-shared/m/llvm-cov-target/debug/incremental/c-1/s-old/x",
            3 * H,
        );
        age_tree(&h.join("build-shared/m/llvm-cov-target"), 3 * H);
        aged(&h, "build-shared/m/debug/incremental/c-1/s-old/x", 2 * H);
        age_tree(&h.join("build-shared/m/debug/incremental/c-1/s-old"), 2 * H);
        aged(
            &h,
            "build-shared/m/debug/incremental/c-1/s-new/x",
            Duration::from_secs(60),
        );
        aged(
            &h,
            "build-shared/m/debug/incremental/c-1/not-a-session/x",
            3 * H,
        );
        let homes = vec![h.clone()];
        let p = SweepPolicy::new(120, false);
        let now = SystemTime::now();

        let checked = sweep(&homes, &p, Some(&[]), now, true);
        assert!(checked.iter().all(|i| !i.removed));
        assert_eq!(
            checked.len(),
            3,
            "the coverage target and two sessions: {checked:?}"
        );

        let blind = sweep(&homes, &p, None, now, false);
        assert!(blind
            .iter()
            .filter(|i| i.verdict != BuildSweepVerdict::Recent)
            .all(|i| i.verdict == BuildSweepVerdict::InUse && !i.removed));

        let covering = vec!["77 cargo llvm-cov --all-targets".to_string()];
        let busy = sweep(&homes, &p, Some(covering.as_slice()), now, true);
        let cov = busy
            .iter()
            .find(|i| i.kind == BuildSweepKind::CoverageTarget)
            .unwrap();
        assert_eq!(cov.verdict, BuildSweepVerdict::InUse);

        let swept = sweep(&homes, &p, Some(&[]), now, false);
        let by_kind = |k: BuildSweepKind, removed: bool| {
            swept
                .iter()
                .filter(|i| i.kind == k && i.removed == removed)
                .count()
        };
        assert_eq!(by_kind(BuildSweepKind::CoverageTarget, true), 1);
        assert_eq!(
            by_kind(BuildSweepKind::IncrementalSession, true),
            1,
            "s-old"
        );
        assert_eq!(
            by_kind(BuildSweepKind::IncrementalSession, false),
            1,
            "s-new"
        );
        assert!(!h.join("build-shared/m/llvm-cov-target").exists());
        assert!(h
            .join("build-shared/m/debug/incremental/c-1/s-new")
            .exists());
        assert!(h
            .join("build-shared/m/debug/incremental/c-1/not-a-session")
            .exists());
    }

    /// Below the floor the threshold drops to thirty minutes, so a directory idle for forty
    /// minutes goes that would have stayed above it; a home that does not exist yields
    /// nothing and no free space.
    #[test]
    fn below_the_floor_the_sweep_reclaims_at_thirty_minutes() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path().canonicalize().unwrap();
        aged(&h, "build-private/forty/x", Duration::from_secs(40 * 60));
        age_tree(&h.join("build-private/forty"), Duration::from_secs(40 * 60));
        let input = |floor| BuildSweepInput {
            check: true,
            idle_minutes: None,
            floor_gib: Some(floor),
            cargo_homes: Vec::new(),
        };
        let above = run_sweep(&input(0), vec![h.clone()], Some(&[]), SystemTime::now());
        assert_eq!(above.items[0].verdict, BuildSweepVerdict::Recent);
        assert_eq!(above.idle_minutes, DEFAULT_IDLE_MINUTES);
        let below = run_sweep(
            &input(u64::MAX >> 31),
            vec![h],
            Some(&[]),
            SystemTime::now(),
        );
        assert!(below.below_floor);
        assert_eq!(below.idle_minutes, INCREMENTAL_IDLE_MINUTES);
        assert_eq!(below.items[0].verdict, BuildSweepVerdict::Reclaimable);
        let none = run_sweep(
            &input(30),
            vec![PathBuf::from("/no/such/home")],
            Some(&[]),
            SystemTime::now(),
        );
        assert!(none.items.is_empty() && none.free_before_bytes.is_none());
    }

    /// A directory whose contents cannot be read is unmeasurable and kept; the lock of a
    /// session that cannot be opened counts as held; the sweep's own process line is not
    /// read as a user of anything.
    #[test]
    fn what_cannot_be_read_is_kept() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let h = home.path().canonicalize().unwrap();
        aged(&h, "build-private/sealed/inner/x", 3 * H);
        let inner = h.join("build-private/sealed/inner");
        std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o000)).unwrap();
        let (v, idle, _) = judge(
            &h.join("build-private/sealed"),
            BuildSweepKind::PrivateBuildDir,
            &SweepPolicy::new(120, false),
            Some(&[]),
            SystemTime::now(),
        );
        std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!((v, idle), (BuildSweepVerdict::Unmeasurable, None));

        let session = h.join("build-shared/m/debug/incremental/c/s-1-svh");
        let lock = h.join("build-shared/m/debug/incremental/c/s-1.lock");
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(&lock, "").unwrap();
        std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert!(
            held(&session, BuildSweepKind::IncrementalSession),
            "a lock that cannot be opened"
        );
        std::fs::set_permissions(&lock, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!held(&session, BuildSweepKind::IncrementalSession));
        let working = h.join("build-shared/m/debug/incremental/c/s-2-working");
        assert!(
            held(&working, BuildSweepKind::IncrementalSession),
            "a session rustc is writing"
        );

        let me = std::process::id();
        let table = format!(
            "{me} majordomus machine sweep {}\n  1 /sbin/launchd\n",
            h.display()
        );
        assert_eq!(own_excluded(&table), ["1 /sbin/launchd"]);
        assert!(
            !remove(Path::new("/")),
            "a path with no name is never removed"
        );
    }

    /// The homes a machine has, and the table a real `ps` gives.
    #[test]
    fn the_homes_and_the_process_table_are_this_machines() {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        std::fs::create_dir_all(h.join(".cargo")).unwrap();
        std::fs::create_dir_all(h.join(".asdf/installs/rust/1.98.1")).unwrap();
        std::fs::create_dir_all(h.join("explicit")).unwrap();
        let found = cargo_homes(Some(h), Some(&h.join("explicit")));
        assert_eq!(found.len(), 3);
        assert_eq!(
            cargo_homes(Some(h), Some(&h.join(".cargo"))).len(),
            2,
            "once each"
        );
        assert!(cargo_homes(None, None).is_empty());
        let table = process_table().expect("ps answers here");
        let me = std::process::id().to_string();
        assert!(table
            .iter()
            .all(|l| l.split_whitespace().next() != Some(me.as_str())));
    }

    /// Install writes the entry and loads it, status sees it, remove unloads and deletes it,
    /// each scheduler command reported; without activation nothing is run; a platform with no
    /// scheduler is refused; and the capability's handler answers status for this machine.
    #[test]
    fn the_schedule_is_installed_reported_and_removed() {
        for scheduler in ["launchd", "systemd"] {
            let home = tempfile::tempdir().unwrap();
            let mut ran: Vec<String> = Vec::new();
            let mut runner = |c: &[String]| {
                ran.push(c.join(" "));
                format!("{}: ok", c.join(" "))
            };
            let input = |action, activate| SweepScheduleInput {
                action,
                floor_gib: Some(25),
                program: None,
                activate,
            };
            let w = Path::new("/repo with space");
            let installed = run_schedule(
                &input(SweepScheduleAction::Install, true),
                scheduler,
                home.path(),
                "/opt/majordomus",
                w,
                &mut runner,
            )
            .unwrap();
            assert!(installed.installed, "{scheduler}");
            assert_eq!(installed.activation.len(), 2, "{scheduler}");
            let text = std::fs::read_to_string(&installed.files[0]).unwrap();
            assert!(text.contains("--reclaim") && text.contains("25"), "{text}");
            let status = run_schedule(
                &input(SweepScheduleAction::Status, true),
                scheduler,
                home.path(),
                "/opt/majordomus",
                w,
                &mut runner,
            )
            .unwrap();
            assert!(status.installed && status.activation.is_empty());
            let removed = run_schedule(
                &input(SweepScheduleAction::Remove, true),
                scheduler,
                home.path(),
                "/opt/majordomus",
                w,
                &mut runner,
            )
            .unwrap();
            assert!(!removed.installed && !removed.activation.is_empty());
            let quiet = run_schedule(
                &input(SweepScheduleAction::Install, false),
                scheduler,
                home.path(),
                "/opt/majordomus",
                w,
                &mut runner,
            )
            .unwrap();
            assert!(quiet.installed && quiet.activation.is_empty());
            let quiet_gone = run_schedule(
                &input(SweepScheduleAction::Remove, false),
                scheduler,
                home.path(),
                "/opt/majordomus",
                w,
                &mut runner,
            )
            .unwrap();
            assert!(!quiet_gone.installed && quiet_gone.activation.is_empty());
            assert!(!ran.is_empty());
        }
        let home = tempfile::tempdir().unwrap();
        let refused = run_schedule(
            &SweepScheduleInput {
                action: SweepScheduleAction::Install,
                ..SweepScheduleInput::default()
            },
            "none",
            home.path(),
            "m",
            Path::new("/r"),
            &mut |_| String::new(),
        );
        assert!(matches!(refused, Err(CapabilityError::Refused(_))));
        // a file in the way of the entry's directory refuses the install, and a directory in
        // the way of a file to remove refuses the removal
        std::fs::write(home.path().join("Library"), "").unwrap();
        let blocked = run_schedule(
            &SweepScheduleInput {
                action: SweepScheduleAction::Install,
                ..SweepScheduleInput::default()
            },
            "launchd",
            home.path(),
            "m",
            Path::new("/r"),
            &mut |_| String::new(),
        );
        assert!(matches!(blocked, Err(CapabilityError::Refused(_))));
        let dir_home = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(
            dir_home
                .path()
                .join(".config/systemd/user/majordomus-sweep.service/x"),
        )
        .unwrap();
        let stuck = run_schedule(
            &SweepScheduleInput {
                action: SweepScheduleAction::Remove,
                activate: false,
                ..SweepScheduleInput::default()
            },
            "systemd",
            dir_home.path(),
            "m",
            Path::new("/r"),
            &mut |_| String::new(),
        );
        assert!(matches!(stuck, Err(CapabilityError::Refused(_))));
        assert!(activation_commands("launchd", &[], true).is_empty());
        assert_eq!(systemd_word("a b\"c\\"), "\"a b\\\"c\\\\\"");
        assert_eq!(xml("<a&b>"), "&lt;a&amp;b&gt;");
        assert!(run_command(&["true".to_string()]).ends_with(": ok"));
        assert!(run_command(&["false".to_string()]).ends_with(": exit 1"));
        assert!(run_command(&["/no/such/program".to_string()]).contains("not run"));
    }

    /// Through the registry: the sweep checks this machine and the schedule reports status,
    /// each answering in its declared shape.
    #[test]
    fn the_capabilities_answer_through_the_registry() {
        let repo = crate::synthetic::SyntheticRepository::new(crate::synthetic::Shape::default())
            .expect("a synthetic repository");
        let ctx = repo.context().expect("a context");
        let empty = tempfile::tempdir().unwrap();
        let v = ctx
            .execute(
                "machine.sweep",
                serde_json::json!({"cargo_homes": [empty.path()]}),
            )
            .expect("a sweep report");
        assert_eq!(v["check"], true);
        assert!(v["items"].as_array().unwrap().is_empty());
        // the homes HOME and CARGO_HOME lead to, without walking this machine's
        let fake = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(fake.path().join(".cargo")).unwrap();
        let found = homes_for(&BuildSweepInput::default(), Some(fake.path().into()), None);
        assert_eq!(found, [fake.path().join(".cargo").canonicalize().unwrap()]);
        let s = ctx
            .execute(
                "machine.schedule",
                serde_json::json!({"program": "/opt/majordomus"}),
            )
            .expect("a status");
        assert_eq!(s["program"], "/opt/majordomus");
        let s = ctx
            .execute("machine.schedule", serde_json::json!({}))
            .expect("a status");
        assert!(s["program"].as_str().unwrap().contains("majordomus"));
    }
}
