//! `mesh build-cache ensure`: a dependency install or a build directory restored before the
//! command that would make it, and saved after (I3952, ADR 0157 decisions 8, 9, 12 and 13).
//!
//! **The levels, nearest first.** An install whose key is already in place — the marker file
//! [`MARKER`] inside its target names it — is skipped. Otherwise the machine's local level
//! (`~/.cache/majordomus/build-cache/<key>.tar.gz` beside `<key>.sha256`) is asked, then the
//! store this node declares, when its own probe finds it healthy, [`select`](super::select)
//! chooses it and a credential file is present. A hit is restored only after its SHA-256
//! matches the digest recorded when it was saved, and only after every entry is judged safe
//! ([`super::archive`]). A miss runs the command and, when it succeeds, saves what it left to
//! the local level and to the store.
//!
//! **A failure is a miss.** A corrupt archive, a refused entry, a store that is down, slow or
//! refuses the credential, a disk under the build's free-space floor: each is said in the
//! answer's notes and the command runs exactly as it would have without the cache
//! (rule `project.cache-is-invisible`). `MAJORDOMUS_BUILD_CACHE=off` bypasses every level.
//!
//! **Per kind.** `npm-modules`: a hit does not run the command, because `npm ci` deletes
//! `node_modules` first. `mix-deps`: a hit runs it anyway, `mix deps.get` then being a quick
//! check. `cargo-build-dir`: the command is the build, and always runs; the archive is
//! restored only into a cold build directory, under cargo's own lock, and saved without
//! incremental state and only below one PUT's size.
//!
//! **Bounded.** The local level is held under the policy's `build_cache.local_max_gib`
//! (oldest first) after every save, and `scripts/reap-orphans` measures and reduces it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::archive::{self, Scratch};
use super::key::{self, KeyReport, Kind, KindKey};
use super::s3::{Credential, Endpoint, Store};
use super::{local_offer, read_node_file, select, FileError, Policy, PROBE_BUDGET};

/// The file inside a target that names the key its content was made under.
pub const MARKER: &str = ".majordomus-build-cache-key";

/// The switch that bypasses every level: `MAJORDOMUS_BUILD_CACHE=off`.
pub const SWITCH: &str = "MAJORDOMUS_BUILD_CACHE";

/// The largest archive saved: a single S3 PUT carries at most 5 GiB, and this leaves room.
pub const MAX_PUT_BYTES: u64 = 4608 * 1024 * 1024;

/// The local level's bound when the policy declares none, in GiB.
pub const DEFAULT_LOCAL_MAX_GIB: u64 = 20;

/// The free-space floor when `MAJORDOMUS_MIN_FREE_MB` says nothing: lib/rust_bin.sh's.
const DEFAULT_MIN_FREE_MB: u64 = 5120;

/// The asking node, as the selection knows it: only this node's own store is asked until a
/// signed offer (I2671) makes another node's trustworthy (ADR 0157 decision 10).
const OWN_NODE: &str = "this-node";

/// How long one read or write of a store transfer may stall.
const STORE_BUDGET: Duration = Duration::from_secs(30);

/// The region a SigV4 signature names; MinIO's default.
const REGION: &str = "us-east-1";

const GIB: u64 = 1024 * 1024 * 1024;

/// Where the target's content came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// `MAJORDOMUS_BUILD_CACHE=off`: no level was asked, the command ran.
    Bypassed,
    /// No key could be derived (a toolchain did not answer): the command ran uncached.
    Uncached,
    /// The marker already named this key: nothing was done.
    InPlace,
    /// Restored from this machine's local level.
    Local,
    /// Restored from the store.
    Store,
    /// Every level missed: the command made it.
    Command,
    /// Nothing was restored and no command was given.
    Nothing,
}

/// A level an archive was saved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    /// `~/.cache/majordomus/build-cache`.
    Local,
    /// The selected store.
    Store,
}

/// The command, and how it ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct CommandRun {
    /// What ran.
    pub argv: Vec<String>,
    /// Its exit code; absent when it could not start or a signal ended it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
}

/// The local level, measured after this run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LocalLevel {
    /// Where it is.
    pub dir: String,
    /// What it holds, in bytes.
    pub bytes: u64,
    /// What it may hold.
    pub bound_bytes: u64,
    /// Archives removed by this run to stay under the bound, oldest first.
    pub pruned: u64,
}

/// What `mesh build-cache ensure` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(rename = "BuildCacheEnsureReport")]
pub struct EnsureReport {
    /// What is cached.
    pub kind: Kind,
    /// The directory holding the lockfile, relative to the repository.
    pub dir: String,
    /// Where the content lives.
    pub target: String,
    /// The key, when one could be derived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    /// Where the content came from.
    pub source: Source,
    /// The command, when it ran.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandRun>,
    /// The levels a new archive was saved to.
    pub saved: Vec<Level>,
    /// Every miss, refusal and skip, in the order they happened.
    pub notes: Vec<String>,
    /// The local level after this run.
    pub local: LocalLevel,
}

/// What a run reads from the machine: where the levels are, and the bounds.
#[derive(Debug, Clone)]
pub(crate) struct Machine {
    pub(crate) home: Option<PathBuf>,
    pub(crate) cache_dir: PathBuf,
    pub(crate) node_file: PathBuf,
    pub(crate) credentials: PathBuf,
    pub(crate) cargo_home: PathBuf,
    pub(crate) bypass: bool,
    pub(crate) min_free_mb: u64,
    pub(crate) local_max_bytes: u64,
    pub(crate) max_put_bytes: u64,
}

/// `${XDG_CACHE_HOME:-$HOME/.cache}/majordomus/build-cache`, as `scripts/reap-orphans` reads it.
pub(crate) fn cache_dir(home: Option<&Path>, xdg_cache: Option<&str>) -> PathBuf {
    let base = match (xdg_cache, home) {
        (Some(x), _) if !x.is_empty() => PathBuf::from(x),
        (_, Some(h)) => h.join(".cache"),
        _ => PathBuf::from(".cache"),
    };
    base.join("majordomus").join("build-cache")
}

impl Machine {
    /// This machine, as the environment describes it, with the policy's bound.
    pub(crate) fn from_env(local_max_gib: Option<u64>) -> Machine {
        Machine::from_env_with(local_max_gib, |n| std::env::var(n).ok())
    }

    /// [`Machine::from_env`] over any lookup of environment values, so that the reading is
    /// testable without touching the process's environment.
    fn from_env_with(local_max_gib: Option<u64>, var: impl Fn(&str) -> Option<String>) -> Machine {
        let home = var("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
        let config = home
            .clone()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config");
        let min_free_mb = var("MAJORDOMUS_MIN_FREE_MB")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_MIN_FREE_MB);
        Machine {
            cache_dir: cache_dir(home.as_deref(), var("XDG_CACHE_HOME").as_deref()),
            node_file: config.join("majordomus/build-cache.json"),
            credentials: config.join("mesh-build-cache/credentials"),
            cargo_home: key::cargo_home(var("CARGO_HOME").as_deref(), home.as_deref()),
            bypass: var(SWITCH).is_some_and(|v| v.trim().eq_ignore_ascii_case("off")),
            min_free_mb,
            local_max_bytes: local_max_gib
                .unwrap_or(DEFAULT_LOCAL_MAX_GIB)
                .saturating_mul(GIB),
            max_put_bytes: MAX_PUT_BYTES,
            home,
        }
    }

    fn entry(&self, key: &str) -> (PathBuf, PathBuf) {
        (
            self.cache_dir.join(format!("{key}.tar.gz")),
            self.cache_dir.join(format!("{key}.sha256")),
        )
    }
}

/// The instance of `kind` a run is about: the one in `dir` when named, else the only one, else
/// the one in the directory the command was started from.
pub(crate) fn resolve(
    report: &KeyReport,
    kind: Kind,
    dir: Option<&str>,
    cwd: Option<&str>,
) -> Result<KindKey, String> {
    let normal = |d: &str| {
        let d = d.trim_end_matches('/');
        if d.is_empty() || d == "." {
            ".".to_string()
        } else {
            d.trim_start_matches("./").to_string()
        }
    };
    let of_kind: Vec<&KindKey> = report.kinds.iter().filter(|k| k.kind == kind).collect();
    let lockfile = kind.lockfile();
    if let Some(d) = dir {
        let d = normal(d);
        return of_kind
            .iter()
            .find(|k| k.dir == d)
            .map(|k| (*k).clone())
            .ok_or_else(|| format!("the repository tracks no {lockfile} in {d}"));
    }
    match of_kind.as_slice() {
        [] => Err(format!("the repository tracks no {lockfile}")),
        [one] => Ok((*one).clone()),
        many => cwd
            .map(normal)
            .and_then(|c| many.iter().find(|k| k.dir == c))
            .map(|k| (*k).clone())
            .ok_or_else(|| {
                let dirs: Vec<&str> = many.iter().map(|k| k.dir.as_str()).collect();
                format!(
                    "the repository tracks {lockfile} in {}: name one with --dir",
                    dirs.join(", ")
                )
            }),
    }
}

/// A run whose instance could not be named: no key, the directory it was asked about, and why.
pub(crate) fn unresolved(kind: Kind, dir: Option<&str>, why: String) -> KindKey {
    let dir = dir.map(|d| d.trim_end_matches('/')).unwrap_or_default();
    KindKey {
        kind,
        dir: if dir.is_empty() {
            ".".into()
        } else {
            dir.to_string()
        },
        location: String::new(),
        platform: key::machine_triple(),
        toolchain: None,
        inputs: Vec::new(),
        digest: String::new(),
        key: None,
        unavailable: Some(why),
    }
}

/// Where an instance's content lives, absolute.
pub(crate) fn target_of(root: &Path, kk: &KindKey, cargo_home: &Path) -> PathBuf {
    let dir = if kk.dir == "." {
        root.to_path_buf()
    } else {
        root.join(&kk.dir)
    };
    match kk.kind {
        Kind::NpmModules => dir.join("node_modules"),
        Kind::MixDeps => dir.join("deps"),
        Kind::CargoBuildDir => key::cargo_build_dir(root, &dir, cargo_home),
    }
}

fn read_marker(target: &Path) -> Option<String> {
    fs::read_to_string(target.join(MARKER))
        .ok()
        .map(|s| s.trim().to_string())
}

fn write_marker(target: &Path, key: &str, notes: &mut Vec<String>) {
    if let Err(e) = fs::write(target.join(MARKER), format!("{key}\n")) {
        notes.push(format!("the key could not be marked in place: {e}"));
    }
}

/// Free bytes on the filesystem holding `path` or its nearest existing ancestor.
pub(crate) fn free_bytes(path: &Path) -> Option<u64> {
    use std::os::unix::ffi::OsStrExt;
    let mut at = Some(path);
    while let Some(p) = at {
        if p.exists() {
            let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
            let mut st = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
            // SAFETY: `c` is a valid NUL-terminated path and `st` a writable statvfs, read
            // only when the call reports success.
            if unsafe { libc::statvfs(c.as_ptr(), st.as_mut_ptr()) } != 0 {
                return None;
            }
            // SAFETY: statvfs returned 0, so the struct is initialised.
            let st = unsafe { st.assume_init() };
            // 32 bits on macOS and 64 on Linux: a widening on one, a no-op on the other
            #[allow(clippy::useless_conversion)]
            return Some(u64::from(st.f_bavail).saturating_mul(u64::from(st.f_frsize)));
        }
        at = p.parent();
    }
    None
}

/// Why nothing may be written beside `path` now, or `None` when it may: the build's own
/// free-space floor (lib/rust_bin.sh, `MAJORDOMUS_MIN_FREE_MB`, 0 disables it). A volume
/// that cannot be measured is not refused, as the build's floor does not refuse it.
fn below_floor(path: &Path, floor_mb: u64) -> Option<String> {
    if floor_mb == 0 {
        return None;
    }
    let free_mb = free_bytes(path)? / (1024 * 1024);
    (free_mb < floor_mb).then(|| {
        format!(
            "{free_mb} MB free on the volume of {}, under the {floor_mb} MB floor (MAJORDOMUS_MIN_FREE_MB)",
            path.display()
        )
    })
}

fn verify(archive: &Path, sum: &Path) -> Result<(), String> {
    let recorded =
        fs::read_to_string(sum).map_err(|e| format!("its digest cannot be read: {e}"))?;
    let recorded = recorded.split_whitespace().next().unwrap_or_default();
    if recorded.len() != 64 || !recorded.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("its recorded digest is not a SHA-256".into());
    }
    let actual = archive::sha256_file(archive).map_err(|e| format!("it cannot be read: {e}"))?;
    if actual != recorded.to_ascii_lowercase() {
        return Err(format!(
            "its SHA-256 {} does not match the {} recorded when it was saved",
            &actual[..12],
            &recorded[..12.min(recorded.len())]
        ));
    }
    Ok(())
}

/// How an archive is put in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Swap the whole target (`node_modules`, `deps`).
    Replace,
    /// Fill a cold build directory under cargo's lock.
    BuildDir,
}

impl Mode {
    fn excludes(self) -> &'static [&'static str] {
        match self {
            Mode::Replace => &[MARKER],
            Mode::BuildDir => &[
                MARKER,
                "incremental",
                archive::CARGO_LOCKS[0],
                archive::CARGO_LOCKS[1],
            ],
        }
    }
}

fn unpack_and_place(archive_path: &Path, target: &Path, mode: Mode) -> Result<(), String> {
    let scratch = archive::unpack_beside(archive_path, target)?;
    match mode {
        Mode::Replace => archive::replace(scratch, target),
        Mode::BuildDir => archive::place_build_dir(scratch, target),
    }
}

fn discard(archive_path: &Path, sum: &Path) {
    let _ = fs::remove_file(archive_path);
    let _ = fs::remove_file(sum);
}

/// The store this node may use now, or why none.
pub(crate) fn select_store(m: &Machine) -> Result<Store, String> {
    let file = match read_node_file(&m.node_file) {
        Ok(f) => f,
        Err(FileError::Absent) => {
            return Err("this node declares no store: the local level alone".into())
        }
        Err(FileError::Invalid(why)) => return Err(why),
    };
    let Some(provider) = file.provider else {
        return Err("this node declares no store of its own: the local level alone".into());
    };
    let offer = local_offer(&provider, OWN_NODE, PROBE_BUDGET);
    let policy = Policy {
        own_node: Some(OWN_NODE.into()),
        ..Policy::default()
    };
    let selection = select(&[offer], &policy);
    let Some(chosen) = selection.chosen else {
        let why: Vec<String> = selection
            .passed_over
            .iter()
            .map(|p| p.why.clone())
            .collect();
        return Err(format!("{} ({})", selection.reason, why.join("; ")));
    };
    let credential = Credential::read(&m.credentials).map_err(|e| e.to_string())?;
    Ok(Store {
        endpoint: Endpoint::parse(&chosen.endpoint)?,
        bucket: chosen.bucket,
        region: REGION.into(),
        credential,
        budget: STORE_BUDGET,
    })
}

/// The store, asked for at most once per run, and only when a level needs it.
struct StoreCell {
    asked: bool,
    store: Option<Store>,
}

impl StoreCell {
    fn get(&mut self, m: &Machine, notes: &mut Vec<String>) -> Option<&Store> {
        if !self.asked {
            self.asked = true;
            match select_store(m) {
                Ok(s) => self.store = Some(s),
                Err(why) => notes.push(format!("store: {why}")),
            }
        }
        self.store.as_ref()
    }
}

/// Restore `key` into `target` from the nearest level that has it; the level, or `None`.
fn restore(
    key: &str,
    target: &Path,
    m: &Machine,
    mode: Mode,
    store: &mut StoreCell,
    notes: &mut Vec<String>,
) -> Option<Source> {
    if let Some(why) = below_floor(target, m.min_free_mb) {
        notes.push(format!("not restored: {why}"));
        return None;
    }
    let (archive_path, sum) = m.entry(key);
    if archive_path.is_file() && sum.is_file() {
        let placed = verify(&archive_path, &sum)
            .and_then(|()| unpack_and_place(&archive_path, target, mode));
        match placed {
            Ok(()) => {
                // most recently used is last to be pruned
                if let Ok(f) = fs::OpenOptions::new().write(true).open(&archive_path) {
                    let _ = f.set_modified(SystemTime::now());
                }
                return Some(Source::Local);
            }
            Err(why) => {
                notes.push(format!("local: the archive was discarded: {why}"));
                discard(&archive_path, &sum);
            }
        }
    } else {
        notes.push("local: no archive under this key".into());
    }
    let store = store.get(m, notes)?;
    match fetch(store, key, &archive_path, &sum) {
        Ok(false) => {
            notes.push("store: no archive under this key".into());
            None
        }
        Err(why) => {
            notes.push(format!("store: {why}"));
            None
        }
        Ok(true) => match unpack_and_place(&archive_path, target, mode) {
            Ok(()) => Some(Source::Store),
            Err(why) => {
                notes.push(format!("store: the archive was discarded: {why}"));
                discard(&archive_path, &sum);
                None
            }
        },
    }
}

/// Fetch `key` and its digest from the store into the local level, verified: `Ok(false)` when
/// the store has neither.
fn fetch(store: &Store, key: &str, archive_path: &Path, sum: &Path) -> Result<bool, String> {
    let parent = archive_path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let scratch = Scratch::beside(archive_path, "fetch").map_err(|e| e.to_string())?;
    let (tmp_archive, tmp_sum) = (scratch.path.join("a"), scratch.path.join("s"));
    if !store
        .get(&format!("{key}.sha256"), &tmp_sum)
        .map_err(|e| e.to_string())?
    {
        return Ok(false);
    }
    if !store
        .get(&format!("{key}.tar.gz"), &tmp_archive)
        .map_err(|e| e.to_string())?
    {
        return Ok(false);
    }
    verify(&tmp_archive, &tmp_sum).map_err(|why| format!("the archive was discarded: {why}"))?;
    // a local copy is kept, the archive first: a reader needs both
    fs::rename(&tmp_archive, archive_path).map_err(|e| e.to_string())?;
    fs::rename(&tmp_sum, sum).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Archive `target` under `key` into the local level and the store.
fn save(
    key: &str,
    target: &Path,
    m: &Machine,
    mode: Mode,
    store: &mut StoreCell,
    notes: &mut Vec<String>,
) -> (Vec<Level>, u64) {
    let mut saved = Vec::new();
    if let Some(why) = below_floor(&m.cache_dir, m.min_free_mb) {
        notes.push(format!("not saved: {why}"));
        return (saved, 0);
    }
    let (archive_path, sum) = m.entry(key);
    let made = (|| -> Result<Scratch, String> {
        fs::create_dir_all(archive_path.parent().unwrap_or(Path::new(".")))
            .map_err(|e| e.to_string())?;
        let scratch = Scratch::beside(&archive_path, "save").map_err(|e| e.to_string())?;
        let held = match mode {
            Mode::BuildDir => {
                archive::lock_profiles(target, &archive::subdirectories(target), false)?
            }
            Mode::Replace => Vec::new(),
        };
        archive::create(target, &scratch.path.join("a"), mode.excludes())?;
        drop(held);
        Ok(scratch)
    })();
    let scratch = match made {
        Ok(s) => s,
        Err(why) => {
            notes.push(format!("not saved: {why}"));
            return (saved, 0);
        }
    };
    let tmp_archive = scratch.path.join("a");
    let size = fs::metadata(&tmp_archive).map(|m| m.len()).unwrap_or(0);
    if size > m.max_put_bytes {
        notes.push(format!(
            "not saved: the archive is {size} bytes, more than one PUT carries ({} bytes)",
            m.max_put_bytes
        ));
        return (saved, 0);
    }
    let written = archive::sha256_file(&tmp_archive)
        .and_then(|digest| fs::write(scratch.path.join("s"), format!("{digest}\n")))
        .and_then(|()| fs::rename(&tmp_archive, &archive_path))
        .and_then(|()| fs::rename(scratch.path.join("s"), &sum));
    if let Err(e) = written {
        notes.push(format!("not saved: {e}"));
        discard(&archive_path, &sum);
        return (saved, 0);
    }
    saved.push(Level::Local);
    let pruned = prune(m, key);
    if let Some(store) = store.get(m, notes) {
        // the digest last: a reader that finds it finds the archive too
        let put = store
            .put(&format!("{key}.tar.gz"), &archive_path)
            .and_then(|()| store.put(&format!("{key}.sha256"), &sum));
        match put {
            Ok(()) => saved.push(Level::Store),
            Err(e) => notes.push(format!("store: not saved: {e}")),
        }
    }
    (saved, pruned)
}

/// Every archive of the local level: `(modified, archive, bytes with its digest)`, in age
/// order; a scratch directory (a name starting with `.`) is not an entry.
fn entries(dir: &Path) -> std::collections::BTreeMap<(SystemTime, PathBuf), u64> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(it) = fs::read_dir(&d) else { continue };
        for e in it.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            match e.file_type() {
                Ok(t) if t.is_dir() => stack.push(e.path()),
                Ok(t) if t.is_file() && name.ends_with(".tar.gz") => {
                    let Ok(meta) = e.metadata() else { continue };
                    let sum = e
                        .path()
                        .with_file_name(format!("{}.sha256", name.trim_end_matches(".tar.gz")));
                    let extra = fs::metadata(&sum).map(|m| m.len()).unwrap_or(0);
                    let when = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    out.insert((when, e.path()), meta.len() + extra);
                }
                _ => {}
            }
        }
    }
    out
}

/// Remove the oldest archives until the level is under its bound, never `keep`'s. The count.
pub(crate) fn prune(m: &Machine, keep: &str) -> u64 {
    let (keep_archive, _) = m.entry(keep);
    let mut total = archive::tree_bytes(&m.cache_dir);
    let mut pruned = 0;
    for ((_, path), bytes) in entries(&m.cache_dir) {
        if total <= m.local_max_bytes {
            break;
        }
        if path == keep_archive {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
        let sum = path.with_file_name(format!(
            "{}.sha256",
            name.unwrap_or_default().trim_end_matches(".tar.gz")
        ));
        discard(&path, &sum);
        total = total.saturating_sub(bytes);
        pruned += 1;
    }
    pruned
}

fn run(argv: &[String], cwd: &Path, notes: &mut Vec<String>) -> Option<CommandRun> {
    let (program, args) = argv.split_first()?;
    // the command's own output goes to stderr, so that the answer on stdout stays one value
    let out = io_stderr();
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(out)
        .status();
    let exit = match status {
        Ok(s) => s.code(),
        Err(e) => {
            notes.push(format!("{program} could not be started: {e}"));
            None
        }
    };
    Some(CommandRun {
        argv: argv.to_vec(),
        exit,
    })
}

fn io_stderr() -> Stdio {
    use std::os::fd::AsFd;
    io::stderr()
        .as_fd()
        .try_clone_to_owned()
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::inherit())
}

/// Ensure one instance: the whole flow of the module header, for a key already derived.
pub(crate) fn ensure(root: &Path, kk: &KindKey, argv: &[String], m: &Machine) -> EnsureReport {
    let target = target_of(root, kk, &m.cargo_home);
    let cwd = if kk.dir == "." {
        root.to_path_buf()
    } else {
        root.join(&kk.dir)
    };
    let mut notes = Vec::new();
    let mut store = StoreCell {
        asked: false,
        store: None,
    };
    let mut saved = Vec::new();
    let mut pruned = 0;
    let mut command = None;
    let source = if m.bypass {
        notes.push(format!("{SWITCH}=off: every level is bypassed"));
        command = run(argv, &cwd, &mut notes);
        Source::Bypassed
    } else if let Some(key) = kk.key.as_deref() {
        match kk.kind {
            Kind::CargoBuildDir => {
                let restored = if archive::is_cold(&target) {
                    let r = restore(key, &target, m, Mode::BuildDir, &mut store, &mut notes);
                    if r.is_some() {
                        write_marker(&target, key, &mut notes);
                    }
                    r
                } else {
                    notes.push("the build directory is warm: nothing is restored into it".into());
                    None
                };
                command = run(argv, &cwd, &mut notes);
                let succeeded = command.as_ref().is_none_or(|c| c.exit == Some(0));
                let already =
                    read_marker(&target).as_deref() == Some(key) || m.entry(key).0.is_file();
                if restored.is_none() && succeeded && !already && !archive::is_cold(&target) {
                    (saved, pruned) = save(key, &target, m, Mode::BuildDir, &mut store, &mut notes);
                    if !saved.is_empty() {
                        write_marker(&target, key, &mut notes);
                    }
                }
                match (restored, &command) {
                    (Some(level), _) => level,
                    (None, Some(_)) => Source::Command,
                    (None, None) => Source::Nothing,
                }
            }
            Kind::MixDeps | Kind::NpmModules => {
                if read_marker(&target).as_deref() == Some(key) {
                    notes.push("the key is already in place: the command is not run".into());
                    Source::InPlace
                } else if let Some(level) =
                    restore(key, &target, m, Mode::Replace, &mut store, &mut notes)
                {
                    write_marker(&target, key, &mut notes);
                    // `mix deps.get` on restored deps is a quick check; `npm ci` would delete
                    // what was just restored
                    if kk.kind == Kind::MixDeps {
                        command = run(argv, &cwd, &mut notes);
                    }
                    level
                } else {
                    command = run(argv, &cwd, &mut notes);
                    if command.as_ref().is_some_and(|c| c.exit == Some(0)) && target.is_dir() {
                        (saved, pruned) =
                            save(key, &target, m, Mode::Replace, &mut store, &mut notes);
                        write_marker(&target, key, &mut notes);
                    }
                    if command.is_some() {
                        Source::Command
                    } else {
                        Source::Nothing
                    }
                }
            }
        }
    } else {
        notes.push(format!(
            "no key: {}; the command runs uncached",
            kk.unavailable
                .as_deref()
                .unwrap_or("the toolchain did not answer")
        ));
        command = run(argv, &cwd, &mut notes);
        Source::Uncached
    };
    EnsureReport {
        kind: kk.kind,
        dir: kk.dir.clone(),
        target: key::display_location(&target, root, m.home.as_deref()),
        key: kk.key.clone(),
        source,
        command,
        saved,
        notes,
        local: LocalLevel {
            dir: key::display_location(&m.cache_dir, root, m.home.as_deref()),
            bytes: archive::tree_bytes(&m.cache_dir),
            bound_bytes: m.local_max_bytes,
            pruned,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        m: Machine,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        fs::create_dir_all(root.join("site")).unwrap();
        fs::write(root.join("site/package-lock.json"), "{}").unwrap();
        let home = dir.path().join("home");
        let m = Machine {
            home: Some(home.clone()),
            cache_dir: cache_dir(Some(&home), None),
            node_file: home.join(".config/majordomus/build-cache.json"),
            credentials: home.join(".config/mesh-build-cache/credentials"),
            cargo_home: home.join(".cargo"),
            bypass: false,
            min_free_mb: 0,
            local_max_bytes: 20 * GIB,
            max_put_bytes: MAX_PUT_BYTES,
        };
        Fixture { _dir: dir, root, m }
    }

    fn kk(kind: Kind, dir: &str, key: Option<&str>) -> KindKey {
        KindKey {
            kind,
            dir: dir.into(),
            location: String::new(),
            platform: "p".into(),
            toolchain: key.map(|_| "t".into()),
            inputs: vec![],
            digest: "0".repeat(32),
            key: key.map(String::from),
            unavailable: key.is_none().then(|| "node does not answer".into()),
        }
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into()]
    }

    const INSTALL: &str = "mkdir -p node_modules/a/.bin && echo one > node_modules/a/index.js \
         && ln -s ../index.js node_modules/a/.bin/a && echo ran >> ../ran.log";

    fn ran(f: &Fixture) -> usize {
        fs::read_to_string(f.root.join("ran.log"))
            .map(|s| s.lines().count())
            .unwrap_or(0)
    }

    /// Every file under `dir` but the marker, with its content (a link: its target).
    fn listing(dir: &Path) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut stack = vec![PathBuf::new()];
        while let Some(rel) = stack.pop() {
            for e in fs::read_dir(dir.join(&rel)).unwrap().flatten() {
                let r = rel.join(e.file_name());
                let t = e.file_type().unwrap();
                if e.file_name() == MARKER {
                    continue;
                }
                if t.is_dir() {
                    stack.push(r.clone());
                    out.insert(r.display().to_string(), "<dir>".into());
                } else if t.is_symlink() {
                    out.insert(
                        r.display().to_string(),
                        fs::read_link(e.path()).unwrap().display().to_string(),
                    );
                } else {
                    out.insert(
                        r.display().to_string(),
                        fs::read_to_string(e.path()).unwrap(),
                    );
                }
            }
        }
        out
    }

    #[test]
    fn a_miss_runs_and_saves_a_hit_restores_without_running_and_in_place_is_skipped() {
        let f = fixture();
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/abc"));
        let cold = ensure(&f.root, &k, &sh(INSTALL), &f.m);
        assert_eq!(cold.source, Source::Command, "{cold:?}");
        assert_eq!(cold.saved, vec![Level::Local]);
        assert_eq!(cold.command.as_ref().and_then(|c| c.exit), Some(0));
        assert!(
            cold.notes.iter().any(|n| n.contains("no store")),
            "{:?}",
            cold.notes
        );
        assert_eq!(ran(&f), 1);
        let target = f.root.join("site/node_modules");
        let want = listing(&target);
        assert_eq!(
            read_marker(&target).as_deref(),
            Some("v1/npm-modules/p/t/abc")
        );
        assert!(f
            .m
            .cache_dir
            .join("v1/npm-modules/p/t/abc.tar.gz")
            .is_file());
        assert!(cold.local.bytes > 0 && cold.local.dir.starts_with("~/"));

        let again = ensure(&f.root, &k, &sh(INSTALL), &f.m);
        assert_eq!(again.source, Source::InPlace);
        assert_eq!(ran(&f), 1);

        fs::remove_dir_all(&target).unwrap();
        let warm = ensure(&f.root, &k, &sh(INSTALL), &f.m);
        assert_eq!(warm.source, Source::Local, "{warm:?}");
        assert_eq!(warm.command, None, "npm is not run on a hit");
        assert_eq!(ran(&f), 1);
        assert_eq!(listing(&target), want);

        fs::remove_dir_all(&target).unwrap();
        let off = Machine {
            bypass: true,
            ..f.m.clone()
        };
        let bypassed = ensure(&f.root, &k, &sh(INSTALL), &off);
        assert_eq!(bypassed.source, Source::Bypassed);
        assert_eq!(ran(&f), 2);
        assert_eq!(
            listing(&target),
            want,
            "cold, warm and bypassed leave the same tree"
        );
        assert_eq!(read_marker(&target), None, "a bypassed run marks nothing");
    }

    #[test]
    fn mix_runs_its_command_on_a_hit_too() {
        let f = fixture();
        fs::write(f.root.join("site/mix.lock"), "%{}").unwrap();
        let k = kk(Kind::MixDeps, "site", Some("v1/mix-deps/any/t/d"));
        let script = "mkdir -p deps/x && echo x > deps/x/mix.exs && echo ran >> ../ran.log";
        assert_eq!(
            ensure(&f.root, &k, &sh(script), &f.m).source,
            Source::Command
        );
        fs::remove_dir_all(f.root.join("site/deps")).unwrap();
        let warm = ensure(&f.root, &k, &sh("echo ran >> ../ran.log"), &f.m);
        assert_eq!(warm.source, Source::Local);
        assert_eq!(warm.command.and_then(|c| c.exit), Some(0));
        assert_eq!(ran(&f), 2);
        assert!(f.root.join("site/deps/x/mix.exs").is_file());
    }

    #[test]
    fn a_corrupt_archive_is_discarded_and_the_command_runs() {
        let f = fixture();
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/c"));
        ensure(&f.root, &k, &sh(INSTALL), &f.m);
        let (a, s) = f.m.entry("v1/npm-modules/p/t/c");
        let mut bytes = fs::read(&a).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        fs::write(&a, bytes).unwrap();
        fs::remove_dir_all(f.root.join("site/node_modules")).unwrap();
        let r = ensure(&f.root, &k, &sh(INSTALL), &f.m);
        assert_eq!(r.source, Source::Command);
        assert!(
            r.notes.iter().any(|n| n.contains("does not match")),
            "{:?}",
            r.notes
        );
        assert_eq!(ran(&f), 2);
        // and the save that followed replaced it with a good one
        assert!(verify(&a, &s).is_ok());
        fs::write(&s, "not a digest\n").unwrap();
        assert!(verify(&a, &s).unwrap_err().contains("not a SHA-256"));
        fs::remove_file(&s).unwrap();
        assert!(verify(&a, &s).unwrap_err().contains("cannot be read"));
    }

    #[test]
    fn an_archive_with_an_escaping_link_is_refused_and_the_command_runs() {
        let f = fixture();
        let key = "v1/npm-modules/p/t/evil";
        let src = f.root.join("evil");
        fs::create_dir_all(&src).unwrap();
        std::os::unix::fs::symlink("/etc", src.join("x")).unwrap();
        let (a, s) = f.m.entry(key);
        fs::create_dir_all(a.parent().unwrap()).unwrap();
        archive::create(&src, &a, &[]).unwrap();
        fs::write(&s, archive::sha256_file(&a).unwrap()).unwrap();
        let r = ensure(
            &f.root,
            &kk(Kind::NpmModules, "site", Some(key)),
            &sh(INSTALL),
            &f.m,
        );
        assert_eq!(r.source, Source::Command);
        assert!(
            r.notes.iter().any(|n| n.contains("outside its target")),
            "{:?}",
            r.notes
        );
        assert!(!f.root.join("site/node_modules/x").exists());
    }

    #[test]
    fn a_failed_command_saves_nothing_and_no_key_runs_uncached() {
        let f = fixture();
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/f"));
        let r = ensure(&f.root, &k, &sh("mkdir node_modules; exit 3"), &f.m);
        assert_eq!(r.command.and_then(|c| c.exit), Some(3));
        assert!(r.saved.is_empty());
        let r = ensure(
            &f.root,
            &kk(Kind::NpmModules, "site", None),
            &sh("true"),
            &f.m,
        );
        assert_eq!(r.source, Source::Uncached);
        assert!(r.notes[0].contains("node does not answer"));
        let r = ensure(&f.root, &k, &["/no/such/program-mj".to_string()], &f.m);
        assert!(
            r.notes.iter().any(|n| n.contains("could not be started")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn the_floor_refuses_a_restore_and_a_save() {
        let f = fixture();
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/floor"));
        let tight = Machine {
            min_free_mb: u64::MAX / (1024 * 1024 * 2),
            ..f.m.clone()
        };
        let r = ensure(&f.root, &k, &sh(INSTALL), &tight);
        assert_eq!(r.source, Source::Command);
        assert!(
            r.notes.iter().any(|n| n.starts_with("not restored")),
            "{:?}",
            r.notes
        );
        assert!(
            r.notes.iter().any(|n| n.starts_with("not saved")),
            "{:?}",
            r.notes
        );
        assert!(r.saved.is_empty());
        assert!(below_floor(Path::new("/"), 0).is_none());
        assert!(
            free_bytes(Path::new("/no/such/dir/at/all")).is_some(),
            "an ancestor is measured"
        );
    }

    #[test]
    fn an_archive_larger_than_one_put_is_not_saved_and_says_so() {
        let f = fixture();
        let small = Machine {
            max_put_bytes: 10,
            ..f.m.clone()
        };
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/big"));
        let r = ensure(&f.root, &k, &sh(INSTALL), &small);
        assert!(r.saved.is_empty());
        assert!(
            r.notes.iter().any(|n| n.contains("more than one PUT")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn the_local_level_is_pruned_oldest_first_and_never_the_new_entry() {
        let f = fixture();
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let (a, s) = f.m.entry(&format!("v1/k/{name}"));
            fs::create_dir_all(a.parent().unwrap()).unwrap();
            fs::write(&a, vec![0u8; 1000]).unwrap();
            fs::write(&s, "x").unwrap();
            let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1000 * (i as u64 + 1));
            fs::OpenOptions::new()
                .write(true)
                .open(&a)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        let m = Machine {
            local_max_bytes: 2100,
            ..f.m.clone()
        };
        assert_eq!(prune(&m, "v1/k/old"), 1);
        assert!(m.entry("v1/k/old").0.is_file(), "the kept entry stays");
        assert!(
            !m.entry("v1/k/mid").0.exists(),
            "the oldest other goes first"
        );
        assert!(m.entry("v1/k/new").0.is_file());
        assert_eq!(prune(&m, "v1/k/old"), 0, "under the bound nothing goes");
    }

    #[test]
    fn instances_are_resolved_by_dir_alone_or_cwd() {
        let r = KeyReport {
            platform: "p".into(),
            kinds: vec![
                kk(Kind::NpmModules, "site", Some("a")),
                kk(Kind::NpmModules, "docs", Some("b")),
                kk(Kind::MixDeps, ".", Some("c")),
            ],
        };
        assert_eq!(
            resolve(&r, Kind::MixDeps, None, None)
                .unwrap()
                .key
                .as_deref(),
            Some("c")
        );
        assert_eq!(
            resolve(&r, Kind::MixDeps, Some("./"), None).unwrap().dir,
            "."
        );
        assert_eq!(
            resolve(&r, Kind::NpmModules, Some("docs/"), None)
                .unwrap()
                .dir,
            "docs"
        );
        assert_eq!(
            resolve(&r, Kind::NpmModules, None, Some("site"))
                .unwrap()
                .dir,
            "site"
        );
        assert!(resolve(&r, Kind::NpmModules, None, None)
            .unwrap_err()
            .contains("--dir"));
        assert!(resolve(&r, Kind::NpmModules, Some("x"), None)
            .unwrap_err()
            .contains("in x"));
        assert!(resolve(&r, Kind::CargoBuildDir, None, None)
            .unwrap_err()
            .contains("Cargo.lock"));
    }

    #[test]
    fn an_unresolved_instance_has_no_key_and_says_why() {
        let k = unresolved(Kind::NpmModules, Some("site/"), "no lockfile".into());
        assert_eq!((k.dir.as_str(), k.key.as_deref()), ("site", None));
        assert_eq!(unresolved(Kind::MixDeps, Some(""), "x".into()).dir, ".");
        assert_eq!(
            unresolved(Kind::MixDeps, None, "x".into())
                .unavailable
                .as_deref(),
            Some("x")
        );
    }

    #[test]
    fn the_machine_reads_its_switch_and_paths() {
        assert_eq!(
            cache_dir(Some(Path::new("/h")), None),
            PathBuf::from("/h/.cache/majordomus/build-cache")
        );
        assert_eq!(
            cache_dir(Some(Path::new("/h")), Some("/x")),
            PathBuf::from("/x/majordomus/build-cache")
        );
        assert_eq!(
            cache_dir(None, Some("")),
            PathBuf::from(".cache/majordomus/build-cache")
        );
        let m = Machine::from_env(Some(1));
        assert_eq!(m.local_max_bytes, GIB);
        assert_eq!(
            Machine::from_env(None).local_max_bytes,
            DEFAULT_LOCAL_MAX_GIB * GIB
        );
        assert!(m.node_file.ends_with(".config/majordomus/build-cache.json"));
    }

    #[test]
    fn a_cold_build_directory_is_restored_and_a_warm_one_is_built_over() {
        let f = fixture();
        fs::create_dir_all(f.root.join(".cargo")).unwrap();
        fs::write(
            f.root.join(".cargo/config.toml"),
            "[build]\nbuild-dir = \"{cargo-cache-home}/build-shared/fx\"\n",
        )
        .unwrap();
        let k = kk(Kind::CargoBuildDir, ".", Some("v1/cargo-build-dir/p/t/d"));
        let bd = f.m.cargo_home.join("build-shared/fx");
        let build = format!(
            "mkdir -p {0}/debug/deps {0}/debug/incremental/s && echo dep > {0}/debug/deps/a.rlib \
             && echo inc > {0}/debug/incremental/s/x && echo tag > {0}/CACHEDIR.TAG \
             && : > {0}/debug/.cargo-build-lock && echo ran >> ran.log",
            bd.display()
        );
        let first = ensure(&f.root, &k, &sh(&build), &f.m);
        assert_eq!(first.source, Source::Command, "{first:?}");
        assert_eq!(first.saved, vec![Level::Local]);
        assert_eq!(first.target, "~/.cargo/build-shared/fx");
        // warm: the build runs, nothing is restored, nothing saved again
        let warm = ensure(&f.root, &k, &sh(&build), &f.m);
        assert!(
            warm.notes.iter().any(|n| n.contains("warm")),
            "{:?}",
            warm.notes
        );
        assert!(warm.saved.is_empty());
        // cold on a second machine sharing the local level: restored without incremental state
        fs::remove_dir_all(&bd).unwrap();
        let restored = ensure(&f.root, &k, &[], &f.m);
        assert_eq!(restored.source, Source::Local, "{restored:?}");
        assert_eq!(
            fs::read_to_string(bd.join("debug/deps/a.rlib")).unwrap(),
            "dep\n"
        );
        assert!(!bd.join("debug/incremental").exists());
        assert!(bd.join("CACHEDIR.TAG").is_file());
        assert!(bd.join("debug/.cargo-build-lock").is_file());
        // no command, warm, already saved: nothing at all
        assert_eq!(ensure(&f.root, &k, &[], &f.m).source, Source::Nothing);
    }

    #[test]
    fn a_held_lock_refuses_to_save_a_build_directory() {
        let f = fixture();
        let k = kk(
            Kind::CargoBuildDir,
            ".",
            Some("v1/cargo-build-dir/p/t/held"),
        );
        let bd = f.root.join("target");
        fs::create_dir_all(bd.join("debug")).unwrap();
        fs::write(bd.join("debug/x.rlib"), "x").unwrap();
        let _held = archive::lock_profile(&bd.join("debug"), true)
            .unwrap()
            .unwrap();
        let r = ensure(&f.root, &k, &[], &f.m);
        assert!(r.saved.is_empty());
        assert!(
            r.notes.iter().any(|n| n.contains("holds cargo's lock")),
            "{:?}",
            r.notes
        );
    }

    /// The objects a stub store holds, by request path.
    type Objects = Arc<Mutex<BTreeMap<String, Vec<u8>>>>;

    /// A stub S3 store: HEAD, GET and PUT over a shared map, and the readiness route.
    fn stub_store() -> (String, Objects) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let objects: Objects = Arc::default();
        let shared = objects.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let (method, path) = (
                    parts.next().unwrap_or("").to_string(),
                    parts.next().unwrap_or("").to_string(),
                );
                let mut length = 0usize;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut out = stream;
                let reply = |out: &mut std::net::TcpStream, status: &str, body: &[u8]| {
                    let _ = out.write_all(
                        format!(
                            "HTTP/1.1 {status}\r\nContent-Length: {}\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    );
                    let _ = out.write_all(body);
                };
                let mut map = shared.lock().unwrap();
                match method.as_str() {
                    _ if path.starts_with("/minio/health") => reply(&mut out, "200 OK", b""),
                    "PUT" => {
                        let mut body = vec![0u8; length];
                        let _ = reader.read_exact(&mut body);
                        map.insert(path, body);
                        reply(&mut out, "200 OK", b"");
                    }
                    "GET" => match map.get(&path) {
                        Some(b) => reply(&mut out, "200 OK", &b.clone()),
                        None => reply(&mut out, "404 Not Found", b""),
                    },
                    _ => reply(&mut out, "405 Method Not Allowed", b""),
                }
            }
        });
        (url, objects)
    }

    fn declare_store(m: &Machine, url: &str) {
        fs::create_dir_all(m.node_file.parent().unwrap()).unwrap();
        fs::write(
            &m.node_file,
            format!(r#"{{"provider":{{"provider":"minio","scope":"host","endpoint":"{url}","bucket":"b"}}}}"#),
        )
        .unwrap();
        fs::create_dir_all(m.credentials.parent().unwrap()).unwrap();
        fs::write(
            &m.credentials,
            "[mesh-build-cache]\naws_access_key_id = n\naws_secret_access_key = s\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&m.credentials, fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn what_one_node_saved_to_the_store_another_restores() {
        let (url, objects) = stub_store();
        let one = fixture();
        declare_store(&one.m, &url);
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/shared"));
        let saved = ensure(&one.root, &k, &sh(INSTALL), &one.m);
        assert_eq!(saved.saved, vec![Level::Local, Level::Store], "{saved:?}");
        assert_eq!(objects.lock().unwrap().len(), 2);

        let two = fixture();
        declare_store(&two.m, &url);
        let r = ensure(&two.root, &k, &sh(INSTALL), &two.m);
        assert_eq!(r.source, Source::Store, "{r:?}");
        assert_eq!(ran(&two), 0);
        assert_eq!(
            listing(&two.root.join("site/node_modules")),
            listing(&one.root.join("site/node_modules"))
        );
        assert!(
            two.m.entry("v1/npm-modules/p/t/shared").0.is_file(),
            "a local copy is kept"
        );

        // a corrupt object in the store is a miss on a third node
        for (p, body) in objects.lock().unwrap().iter_mut() {
            if p.ends_with(".tar.gz") {
                body[10] ^= 0xff;
            }
        }
        let three = fixture();
        declare_store(&three.m, &url);
        let r = ensure(&three.root, &k, &sh(INSTALL), &three.m);
        assert_eq!(r.source, Source::Command);
        assert!(
            r.notes
                .iter()
                .any(|n| n.starts_with("store: the archive was discarded")),
            "{:?}",
            r.notes
        );

        // a key the store lacks is a plain miss
        let four = fixture();
        declare_store(&four.m, &url);
        let r = ensure(
            &four.root,
            &kk(Kind::NpmModules, "site", Some("v1/none")),
            &sh(INSTALL),
            &four.m,
        );
        assert!(
            r.notes
                .iter()
                .any(|n| n == "store: no archive under this key"),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn a_store_is_used_only_when_declared_healthy_and_credentialed() {
        let f = fixture();
        assert!(select_store(&f.m)
            .unwrap_err()
            .contains("declares no store"));
        fs::create_dir_all(f.m.node_file.parent().unwrap()).unwrap();
        fs::write(&f.m.node_file, "{}").unwrap();
        assert!(select_store(&f.m).unwrap_err().contains("of its own"));
        fs::write(&f.m.node_file, "{bad").unwrap();
        assert!(select_store(&f.m)
            .unwrap_err()
            .contains("not a build-cache file"));
        // a port nothing listens on: unhealthy, passed over with the rule
        let port = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        declare_store(&f.m, &format!("http://127.0.0.1:{port}"));
        assert!(select_store(&f.m).unwrap_err().contains("unhealthy"));
        let (url, _) = stub_store();
        declare_store(&f.m, &url);
        assert!(select_store(&f.m).is_ok());
        fs::remove_file(&f.m.credentials).unwrap();
        assert!(select_store(&f.m)
            .unwrap_err()
            .contains("no store credential"));
    }

    fn is_root() -> bool {
        // SAFETY: geteuid has no preconditions and reads the process's own credentials.
        unsafe { libc::geteuid() == 0 }
    }

    fn mode(path: &Path, bits: u32) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(bits)).unwrap();
    }

    /// A stub store answering by `(method, path suffix)` with the first matching rule, 404
    /// otherwise, and 200 on its readiness route. A body sent to it is read and dropped.
    fn scripted_store(rules: Vec<(&'static str, &'static str, u16, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() {
                    continue;
                }
                let mut parts = line.split_whitespace();
                let (method, path) = (
                    parts.next().unwrap_or("").to_string(),
                    parts.next().unwrap_or("").to_string(),
                );
                let mut length = 0usize;
                loop {
                    let mut h = String::new();
                    if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" {
                        break;
                    }
                    if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut sink = vec![0u8; length];
                let _ = reader.read_exact(&mut sink);
                let (status, body) = if path.starts_with("/minio/health") {
                    (200, Vec::new())
                } else {
                    rules
                        .iter()
                        .find(|(m, suffix, _, _)| *m == method && path.ends_with(suffix))
                        .map(|(_, _, status, body)| (*status, body.clone()))
                        .unwrap_or((404, Vec::new()))
                };
                let mut out = stream;
                let _ = out.write_all(
                    format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    )
                    .as_bytes(),
                );
                let _ = out.write_all(&body);
            }
        });
        url
    }

    /// The store a machine declared at `url`, as the run selects it.
    fn store_at(f: &Fixture, url: &str) -> Store {
        declare_store(&f.m, url);
        select_store(&f.m).unwrap()
    }

    fn cell() -> StoreCell {
        StoreCell {
            asked: false,
            store: None,
        }
    }

    /// A real archive of a one-file tree and the digest line that goes with it.
    fn good_archive(f: &Fixture) -> (Vec<u8>, Vec<u8>) {
        let src = f.root.join("good");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("f"), "f").unwrap();
        let a = f.root.join("good.tar.gz");
        archive::create(&src, &a, &[]).unwrap();
        let sum = format!("{}\n", archive::sha256_file(&a).unwrap());
        (fs::read(&a).unwrap(), sum.into_bytes())
    }

    #[test]
    fn the_machine_reads_its_floor_its_switch_and_a_missing_home_from_the_environment() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |n: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == n)
                    .map(|(_, v)| v.to_string())
            }
        };
        let m = Machine::from_env_with(
            None,
            env(&[
                ("HOME", ""),
                ("MAJORDOMUS_MIN_FREE_MB", " 7 "),
                (SWITCH, " OFF "),
            ]),
        );
        assert_eq!(m.home, None);
        assert_eq!(m.min_free_mb, 7);
        assert!(m.bypass);
        assert_eq!(
            m.node_file,
            PathBuf::from("./.config/majordomus/build-cache.json")
        );
        let unparsed = Machine::from_env_with(
            None,
            env(&[("MAJORDOMUS_MIN_FREE_MB", "lots"), (SWITCH, "on")]),
        );
        assert_eq!(
            unparsed.min_free_mb, DEFAULT_MIN_FREE_MB,
            "text is no floor"
        );
        assert!(!unparsed.bypass, "only `off` bypasses");
    }

    #[test]
    fn a_marker_that_cannot_be_written_is_said_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let mut notes = Vec::new();
        write_marker(&dir.path().join("absent"), "k", &mut notes);
        assert!(
            notes[0].starts_with("the key could not be marked in place"),
            "{notes:?}"
        );
    }

    #[test]
    fn free_space_is_unknown_for_a_path_nothing_exists_of_and_is_then_not_refused() {
        assert_eq!(free_bytes(Path::new("")), None);
        assert_eq!(free_bytes(Path::new("a\0b")), None);
        assert_eq!(below_floor(Path::new(""), 5), None);
    }

    #[test]
    fn a_digest_for_an_archive_that_is_gone_says_it_cannot_be_read() {
        let dir = tempfile::tempdir().unwrap();
        let sum = dir.path().join("a.sha256");
        fs::write(&sum, "a".repeat(64)).unwrap();
        let err = verify(&dir.path().join("a.tar.gz"), &sum).unwrap_err();
        assert!(err.starts_with("it cannot be read"), "{err}");
    }

    #[test]
    fn a_declared_store_without_a_scheme_is_probed_but_not_spoken_to() {
        let f = fixture();
        let (url, _) = stub_store();
        declare_store(&f.m, url.trim_start_matches("http://"));
        let err = select_store(&f.m).unwrap_err();
        assert!(err.contains("only http://"), "{err}");
    }

    #[test]
    fn an_archive_that_cannot_be_touched_is_still_restored() {
        if is_root() {
            return;
        }
        let f = fixture();
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/ro"));
        ensure(&f.root, &k, &sh(INSTALL), &f.m);
        let (a, _) = f.m.entry("v1/npm-modules/p/t/ro");
        mode(&a, 0o444);
        fs::remove_dir_all(f.root.join("site/node_modules")).unwrap();
        let r = ensure(&f.root, &k, &sh(INSTALL), &f.m);
        assert_eq!(r.source, Source::Local, "{r:?}");
        assert_eq!(ran(&f), 1);
        mode(&a, 0o644);
    }

    #[test]
    fn an_archive_the_store_serves_that_is_unsafe_is_discarded_and_the_command_runs() {
        let (url, objects) = stub_store();
        let f = fixture();
        declare_store(&f.m, &url);
        let key = "v1/npm-modules/p/t/evil2";
        let src = f.root.join("evil");
        fs::create_dir_all(&src).unwrap();
        std::os::unix::fs::symlink("/etc", src.join("x")).unwrap();
        let a = f.root.join("evil.tar.gz");
        archive::create(&src, &a, &[]).unwrap();
        {
            let mut map = objects.lock().unwrap();
            map.insert(format!("/b/{key}.tar.gz"), fs::read(&a).unwrap());
            map.insert(
                format!("/b/{key}.sha256"),
                format!("{}\n", archive::sha256_file(&a).unwrap()).into_bytes(),
            );
        }
        let r = ensure(
            &f.root,
            &kk(Kind::NpmModules, "site", Some(key)),
            &sh(INSTALL),
            &f.m,
        );
        assert_eq!(r.source, Source::Command, "{r:?}");
        assert!(
            r.notes
                .iter()
                .any(|n| n.starts_with("store: the archive was discarded")
                    && n.contains("outside its target")),
            "{:?}",
            r.notes
        );
        assert!(!f.m.entry(key).0.is_file() || r.saved.contains(&Level::Local));
        assert!(!f.root.join("site/node_modules/x").exists());
    }

    #[test]
    fn a_fetch_says_where_it_failed() {
        let f = fixture();
        let key = "v1/k/x";
        let (archive_bytes, sum_bytes) = good_archive(&f);
        let both = |sum_status: u16, archive_status: u16| {
            scripted_store(vec![
                ("GET", ".sha256", sum_status, sum_bytes.clone()),
                ("GET", ".tar.gz", archive_status, archive_bytes.clone()),
            ])
        };
        let (a, s) = (
            f.root.join("lvl/v1/k/x.tar.gz"),
            f.root.join("lvl/v1/k/x.sha256"),
        );

        // the local level's directory cannot be made
        let store = store_at(&f, &both(200, 200));
        let file = f.root.join("file");
        fs::write(&file, "x").unwrap();
        assert!(fetch(&store, key, &file.join("d/x.tar.gz"), &s).is_err());

        // ... or cannot be written in
        if !is_root() {
            let locked = f.root.join("locked");
            fs::create_dir(&locked).unwrap();
            mode(&locked, 0o500);
            let r = fetch(&store, key, &locked.join("x.tar.gz"), &s);
            mode(&locked, 0o700);
            assert!(r.is_err());
        }

        // the store errs on the digest, then on the archive
        let broken = store_at(&f, &scripted_store(vec![("GET", "", 500, vec![])]));
        let err = fetch(&broken, key, &a, &s).unwrap_err();
        assert!(err.contains("500"), "{err}");
        let half = store_at(
            &f,
            &scripted_store(vec![
                ("GET", ".sha256", 200, sum_bytes.clone()),
                ("GET", ".tar.gz", 500, vec![]),
            ]),
        );
        assert!(fetch(&half, key, &a, &s).unwrap_err().contains("500"));

        // a digest without its archive is a miss
        let lone = store_at(&f, &both(200, 404));
        assert_eq!(fetch(&lone, key, &a, &s), Ok(false));

        // both present, and a local entry that is in the way of either rename
        let full = store_at(&f, &both(200, 200));
        fs::create_dir_all(&a).unwrap();
        assert!(
            fetch(&full, key, &a, &s).is_err(),
            "the archive's place is a directory"
        );
        fs::remove_dir(&a).unwrap();
        fs::create_dir_all(&s).unwrap();
        assert!(
            fetch(&full, key, &a, &s).is_err(),
            "the digest's place is a directory"
        );
        fs::remove_dir(&s).unwrap();
        fs::remove_file(&a).unwrap();
        assert_eq!(fetch(&full, key, &a, &s), Ok(true));
        assert!(a.is_file() && s.is_file());
    }

    #[test]
    fn a_save_that_goes_wrong_says_so_and_saves_nothing() {
        let f = fixture();
        let key = "v1/npm-modules/p/t/s";
        let target = f.root.join("site/node_modules");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("f"), "f").unwrap();
        let attempt = |m: &Machine, target: &Path| {
            let mut notes = Vec::new();
            let r = save(key, target, m, Mode::Replace, &mut cell(), &mut notes);
            (r, notes)
        };

        // the local level cannot be made
        let file = f.root.join("file");
        fs::write(&file, "x").unwrap();
        let under_file = Machine {
            cache_dir: file.join("c"),
            ..f.m.clone()
        };
        let ((saved, pruned), notes) = attempt(&under_file, &target);
        assert!((saved.is_empty(), pruned) == (true, 0));
        assert!(notes[0].starts_with("not saved"), "{notes:?}");

        // ... or cannot be written in
        if !is_root() {
            let (a, _) = f.m.entry(key);
            let dir = a.parent().unwrap();
            fs::create_dir_all(dir).unwrap();
            mode(dir, 0o500);
            let ((saved, _), notes) = attempt(&f.m, &target);
            mode(dir, 0o700);
            assert!(saved.is_empty());
            assert!(notes[0].starts_with("not saved"), "{notes:?}");
        }

        // a target that is not there cannot be archived
        let ((saved, _), notes) = attempt(&f.m, &f.root.join("absent"));
        assert!(saved.is_empty());
        assert!(notes[0].contains("tar failed"), "{notes:?}");

        // an archive's place that is a directory cannot be filled
        let (a, _) = f.m.entry(key);
        fs::create_dir_all(&a).unwrap();
        let ((saved, _), notes) = attempt(&f.m, &target);
        assert!(saved.is_empty());
        assert!(notes[0].starts_with("not saved"), "{notes:?}");
        fs::remove_dir(&a).unwrap();
    }

    #[test]
    fn a_store_that_refuses_the_put_is_said_and_the_local_copy_stays() {
        let f = fixture();
        let url = scripted_store(vec![("PUT", "", 500, vec![])]);
        declare_store(&f.m, &url);
        let r = ensure(
            &f.root,
            &kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/put")),
            &sh(INSTALL),
            &f.m,
        );
        assert_eq!(r.saved, vec![Level::Local]);
        assert!(
            r.notes.iter().any(|n| n.starts_with("store: not saved")),
            "{:?}",
            r.notes
        );
    }

    #[test]
    fn a_local_level_that_is_not_there_has_nothing_to_prune() {
        let f = fixture();
        assert_eq!(prune(&f.m, "v1/k/none"), 0);
        assert!(entries(&f.m.cache_dir).is_empty());
    }

    #[test]
    fn an_install_without_a_command_makes_nothing_and_says_nothing_ran() {
        let f = fixture();
        let k = kk(Kind::NpmModules, "site", Some("v1/npm-modules/p/t/none"));
        let r = ensure(&f.root, &k, &[], &f.m);
        assert_eq!(r.source, Source::Nothing);
        assert_eq!(r.command, None);
        assert!(r.saved.is_empty());
    }
}
