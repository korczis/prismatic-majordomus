//! The one liveness predicate every deleting sweep asks (I4346): is anybody using this tree
//! or build directory, and if so, who and how do we know?
//!
//! Three sweeps delete on this machine and on the build host: `scripts/reap-orphans --targets`
//! (build output of worktrees), `majordomus worktree cleanup --remove` (whole worktrees) and the
//! build host's `disk-guard` (cargo build directories under people's and CI runners' homes).
//! Each used to decide liveness its own way, and on 2026-10-28 one of them read a worktree as
//! idle because liveness came from command lines and working directories alone, which a
//! session working through absolute paths never shows. It removed the only copy of the
//! executable that worktree's hooks required (ADR 0224), and every hook refused after it.
//!
//! So liveness is decided here, once, and every sweep asks: `worktree cleanup` in process,
//! `reap-orphans` and `disk-guard` through `majordomus worktree liveness <path>...`. The answer
//! for each path is `keep` or `reclaim`, and a `keep` carries every reason found.
//!
//! # What makes a path live
//!
//! A path is examined together with the work tree that holds it, when one does (the nearest
//! ancestor, itself included, that holds a `.git`). It is live when any of these holds:
//!
//! | signal                 | read from                                                       |
//! |------------------------|-----------------------------------------------------------------|
//! | `task`                 | the tree's `.ai/local/state/current.yaml`, outcome `active`      |
//! | `mesh_claim`           | a live mesh claim or session naming its issue, branch or path   |
//! | `board_claim`          | a peer attached to, or announcing on, the tree's own server      |
//! | `derive_lock`          | the machine's derive lock, whose live owner names its branch     |
//! | `queue_pid_file`       | an offload run record or lock pid file whose process works in it|
//! | `runner_job`           | a running CI runner job (`Runner.Worker`) whose home holds it    |
//! | `process_environment`  | another process's `CARGO_*_DIR` or `MAJORDOMUS_BIN` points in it |
//! | `process_directory`    | another process's working directory is inside it                |
//! | `process_arguments`    | another process names it on its command line                    |
//! | `git_activity`         | the tree's index, `HEAD` or `HEAD` reflog changed in the window |
//!
//! # Failing closed
//!
//! Not knowing is never "nobody". Every reading that could not be taken is a reason of its
//! own, `unreadable`, and keeps the path: a task record that exists but cannot be read, a git
//! file that cannot be stat'ed, a server whose lease is live but which does not answer. A
//! source that could not be read at all — the process table, the working directories, the
//! environments of this user's processes, the claims of a repository that declares a mesh, the
//! offload run records — is also named in the report's `blind` list, and a report with
//! anything there is `complete: false`: the command line exits non-zero on it, and
//! `reap-orphans` then removes nothing at all.
//!
//! One reading the kernel itself withholds: macOS hands nobody the environment of Apple's
//! platform binaries (`/bin/zsh`, `/bin/sleep`), not even their own user. Their working
//! directory and command line are still read; the processes that build — cargo, rustc, the
//! executable itself — are not platform binaries, and their environments are read in full.
//!
//! The predicate never deletes, never writes, and never refreshes an index: every reading is a
//! stat, a file read, a process-table read or a bounded HTTP GET to a loopback server.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The schema of [`LivenessReport`].
pub(crate) const SCHEMA: &str = "majordomus-worktree-liveness/v1";

/// The idle window when the caller names none: git state written within it is somebody's work
/// in progress. Thirty minutes, the age `reap-orphans` already required of a server before it
/// called one abandoned.
pub(crate) const DEFAULT_IDLE_SECONDS: u64 = 1800;

/// Where a work tree's active task record lives, relative to it.
const TASK_RECORD: &str = ".ai/local/state/current.yaml";

/// The variables of another process that say where it builds or which executable it runs.
const MAJORDOMUS_BIN: &str = "MAJORDOMUS_BIN";

/// How many processes one reason names before it counts the rest.
const NAMED: usize = 3;

/// Keep or reclaim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LivenessVerdict {
    /// Something uses it, or that could not be ruled out.
    Keep,
    /// Every source was read and none of them names it.
    Reclaim,
}

impl LivenessVerdict {
    /// The word the command line prints.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            LivenessVerdict::Keep => "keep",
            LivenessVerdict::Reclaim => "reclaim",
        }
    }
}

/// What made a path live.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LivenessSignal {
    /// An open majordomus task in the tree.
    Task,
    /// A live mesh claim or session names it.
    MeshClaim,
    /// A peer of the tree's own server is attached or announced.
    BoardClaim,
    /// The derive lock's live owner names it.
    DeriveLock,
    /// An offload run record or a pid file of the offload lock names it.
    QueuePidFile,
    /// A CI runner job is running under the home that holds it.
    RunnerJob,
    /// Another process's `CARGO_*_DIR` or `MAJORDOMUS_BIN` points into it.
    ProcessEnvironment,
    /// Another process's working directory is inside it.
    ProcessDirectory,
    /// Another process names it on its command line.
    ProcessArguments,
    /// Its index, `HEAD` or `HEAD` reflog changed within the idle window.
    GitActivity,
    /// A reading that could not be taken: not knowing keeps it.
    Unreadable,
}

impl LivenessSignal {
    /// The word the command line prints.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            LivenessSignal::Task => "task",
            LivenessSignal::MeshClaim => "mesh_claim",
            LivenessSignal::BoardClaim => "board_claim",
            LivenessSignal::DeriveLock => "derive_lock",
            LivenessSignal::QueuePidFile => "queue_pid_file",
            LivenessSignal::RunnerJob => "runner_job",
            LivenessSignal::ProcessEnvironment => "process_environment",
            LivenessSignal::ProcessDirectory => "process_directory",
            LivenessSignal::ProcessArguments => "process_arguments",
            LivenessSignal::GitActivity => "git_activity",
            LivenessSignal::Unreadable => "unreadable",
        }
    }
}

/// One reason a path is kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct LivenessReason {
    /// What kind of evidence.
    pub(crate) signal: LivenessSignal,
    /// The evidence, in words.
    pub(crate) detail: String,
}

/// The answer for one path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PathLiveness {
    /// The path as it was asked about (made absolute when it was relative).
    pub(crate) path: String,
    /// Keep or reclaim.
    pub(crate) verdict: LivenessVerdict,
    /// The work tree that holds it, when one does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tree: Option<String>,
    /// Every reason it is kept; empty exactly when the verdict is `reclaim`.
    pub(crate) reasons: Vec<LivenessReason>,
}

/// The answer of `worktree.liveness`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct LivenessReport {
    /// [`SCHEMA`].
    pub(crate) schema: String,
    /// The idle window git activity is judged against, in seconds.
    pub(crate) idle_seconds: u64,
    /// Whether every source was read. A sweep that gets `false` removes nothing.
    pub(crate) complete: bool,
    /// Every source that could not be read at all, with why.
    pub(crate) blind: Vec<String>,
    /// One answer per path, in the order asked.
    pub(crate) paths: Vec<PathLiveness>,
}

// ---------------------------------------------------------------- the readings

/// One process, as far as this process could read it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Process {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) uid: u32,
    /// Its command line, one string, separators collapsed.
    pub(crate) args: String,
    /// Its working directory, when it could be read.
    pub(crate) cwd: Option<String>,
    /// The variables of its environment this predicate reads, when it could be read.
    pub(crate) env: Option<Vec<(String, String)>>,
}

/// The machine's processes, read once for every path asked about.
#[derive(Debug, Clone)]
pub(crate) struct Machine {
    /// This process.
    pub(crate) me: u32,
    /// The process table; an error when it could not be listed.
    pub(crate) processes: Result<Vec<Process>, String>,
    /// Whether the working directories could be read.
    pub(crate) directories: Result<(), String>,
    /// Whether the environments of this user's processes could be read.
    pub(crate) environments: Result<(), String>,
}

impl Machine {
    /// This process and every ancestor of it: the callers that handed it the paths, whose own
    /// command lines name every one of them.
    fn askers(&self) -> BTreeSet<u32> {
        let mut out = BTreeSet::from([self.me]);
        let Ok(procs) = &self.processes else {
            return out;
        };
        let parent: BTreeMap<u32, u32> = procs.iter().map(|p| (p.pid, p.ppid)).collect();
        let mut at = self.me;
        while let Some(&up) = parent.get(&at) {
            if up <= 1 || !out.insert(up) {
                break;
            }
            at = up;
        }
        out
    }
}

/// Where the offload queue and the derive lock are on this machine.
#[derive(Debug, Clone)]
pub(crate) struct Places {
    /// The offload run records, one `<tag>.run` per run that lives.
    pub(crate) offload_runs: PathBuf,
    /// The offload lock directory, one pid file per process of the derive that holds it.
    pub(crate) offload_lock: PathBuf,
    /// The derive lock directory, when the environment moves it from beside the primary
    /// checkout (`MAJORDOMUS_DERIVE_LOCK`, as `lib/machine_lock.sh` reads it).
    pub(crate) derive_lock: Option<PathBuf>,
}

impl Places {
    /// The declared defaults of the offload queue, and the derive lock where the environment
    /// says it is.
    pub(crate) fn of_env() -> Places {
        let q = crate::offload::declaration::QueueConfig::default();
        Places {
            offload_runs: PathBuf::from(crate::offload::owner::runs_directory(&q.directory)),
            offload_lock: PathBuf::from(q.lock),
            derive_lock: std::env::var_os("MAJORDOMUS_DERIVE_LOCK")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        }
    }
}

/// The servers a board or the mesh is read from. A trait so that a test hands in its answers.
pub(crate) trait Servers {
    /// `GET <url><path>`, its JSON body, or why there is none.
    fn get(&self, url: &str, path: &str) -> Result<Value, String>;
}

/// The loopback servers of this machine, asked over HTTP within the probe timeout.
pub(crate) struct Http;

impl Servers for Http {
    fn get(&self, url: &str, path: &str) -> Result<Value, String> {
        let reply = crate::mcp::bridge::request(
            url,
            "GET",
            path,
            &[],
            None,
            crate::lease::timings().probe_timeout,
        )
        .map_err(|e| format!("{url} did not answer ({e})"))?;
        if reply.status != 200 {
            return Err(format!("{url} answered {}", reply.status));
        }
        serde_json::from_str(&reply.body).map_err(|e| format!("{url} answered no JSON ({e})"))
    }
}

/// The HTTP route of a capability, from the registry rather than written here a second time.
fn route(id: &str) -> Option<String> {
    crate::command_graph::load::registry().and_then(|r| {
        r.get(id)
            .and_then(|c| c.exposure.http.as_ref().map(|h| h.path.clone()))
    })
}

// ---------------------------------------------------------------- reading this machine

/// Read this machine's processes: the table from `ps`, the working directories from `lsof`,
/// and the environments of this user's processes from the kernel. Each reading that comes
/// back empty is an error, never an empty machine.
pub(crate) fn read_machine() -> Machine {
    let me = std::process::id();
    let mut processes = list_processes();
    let mut directories = Err("the process table was not read".to_string());
    let mut environments = Err("the process table was not read".to_string());
    if let Ok(procs) = processes.as_mut() {
        directories = read_directories(procs);
        environments = read_environments(procs, me);
    }
    Machine {
        me,
        processes,
        directories,
        environments,
    }
}

/// `ps` over every process: pid, parent, owner, command line.
fn list_processes() -> Result<Vec<Process>, String> {
    let out = std::process::Command::new("ps")
        .args([
            "-A", "-ww", "-o", "pid=", "-o", "ppid=", "-o", "uid=", "-o", "args=",
        ])
        .output()
        .map_err(|e| format!("ps could not be run ({e})"))?;
    let procs = parse_ps(&String::from_utf8_lossy(&out.stdout));
    if procs.is_empty() {
        return Err("ps listed no process".into());
    }
    Ok(procs)
}

/// The lines `ps -o pid= -o ppid= -o uid= -o args=` prints; a line that does not start with
/// three numbers is not a process.
fn parse_ps(text: &str) -> Vec<Process> {
    text.lines()
        .filter_map(|line| {
            let mut words = line.split_whitespace();
            let pid = words.next()?.parse().ok()?;
            let ppid = words.next()?.parse().ok()?;
            let uid = words.next()?.parse().ok()?;
            let args = collapse(&words.collect::<Vec<_>>().join(" "));
            Some(Process {
                pid,
                ppid,
                uid,
                args,
                ..Process::default()
            })
        })
        .collect()
}

/// One `lsof` for every working directory it may read.
fn read_directories(procs: &mut [Process]) -> Result<(), String> {
    let out = std::process::Command::new("lsof")
        .args(["-n", "-P", "-w", "-a", "-d", "cwd", "-Fpn"])
        .output()
        .map_err(|e| format!("lsof could not be run ({e})"))?;
    let cwds = parse_lsof(&String::from_utf8_lossy(&out.stdout));
    if cwds.is_empty() {
        return Err("lsof read no working directory".into());
    }
    for p in procs.iter_mut() {
        p.cwd = cwds.get(&p.pid).cloned();
    }
    Ok(())
}

/// `lsof -Fpn`: a `p<pid>` line, then the `n<path>` of each of its files.
fn parse_lsof(text: &str) -> BTreeMap<u32, String> {
    let mut out = BTreeMap::new();
    let mut pid = None;
    for line in text.lines() {
        if let Some(p) = line.strip_prefix('p') {
            pid = p.trim().parse::<u32>().ok();
        } else if let (Some(n), Some(at)) = (line.strip_prefix('n'), pid) {
            out.insert(at, collapse(n));
        }
    }
    out
}

/// The variables a sweep's question turns on.
fn wanted(key: &str) -> bool {
    key == MAJORDOMUS_BIN || key == "HOME" || (key.starts_with("CARGO_") && key.ends_with("_DIR"))
}

/// Keep only the variables a sweep asks about, from `KEY=value` entries.
fn variables<'a>(entries: impl Iterator<Item = &'a str>) -> Vec<(String, String)> {
    entries
        .filter_map(|e| e.split_once('='))
        .filter(|(k, _)| wanted(k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// Read the environment of every process this one may read: its user's, or all when it is
/// root. A process gone since the table was listed is no failure; a readable process whose
/// environment could not be read is, and when every one failed the reading is an error.
fn read_environments(procs: &mut [Process], me: u32) -> Result<(), String> {
    // SAFETY: geteuid has no preconditions and cannot fail.
    let uid = unsafe { libc::geteuid() };
    let (mut tried, mut read) = (0usize, 0usize);
    for p in procs.iter_mut().filter(|p| p.pid != me) {
        if uid != 0 && p.uid != uid {
            continue;
        }
        match environment_of(p.pid) {
            Some(env) => {
                read += 1;
                tried += 1;
                p.env = Some(variables(env.iter().map(String::as_str)));
            }
            None if crate::lease::alive(p.pid) => tried += 1,
            None => {}
        }
    }
    if tried > 0 && read == 0 {
        return Err(format!(
            "the environment of none of {tried} process(es) of this user could be read"
        ));
    }
    Ok(())
}

/// A process's environment, `KEY=value` each, from `/proc` on Linux.
#[cfg(target_os = "linux")]
fn environment_of(pid: u32) -> Option<Vec<String>> {
    let bytes = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    Some(nul_strings(&bytes))
}

/// A process's environment, `KEY=value` each, from `KERN_PROCARGS2` on macOS.
#[cfg(target_os = "macos")]
fn environment_of(pid: u32) -> Option<Vec<String>> {
    let pid = libc::c_int::try_from(pid).ok()?;
    let mut argmax: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
    // SAFETY: the buffer is a c_int of the size passed, and nothing is written.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            (&mut argmax as *mut libc::c_int).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 || argmax <= 0 {
        return None;
    }
    let mut buf = vec![0u8; usize::try_from(argmax).ok()?];
    let mut size = buf.len();
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    // SAFETY: the buffer is `size` bytes long, and the kernel writes at most that many.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buf.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    buf.truncate(size);
    procargs_environment(&buf)
}

/// Nowhere else is a process's environment read.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn environment_of(_pid: u32) -> Option<Vec<String>> {
    None
}

/// NUL-separated strings, the empty ones dropped.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn nul_strings(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// The environment in a `KERN_PROCARGS2` buffer: `argc` as a native 32-bit integer, the
/// executable's path, NUL padding, `argc` arguments, then the environment up to an empty
/// string or the end.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn procargs_environment(buf: &[u8]) -> Option<Vec<String>> {
    let argc = i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?);
    let argc = usize::try_from(argc).ok()?;
    let mut rest = &buf[4..];
    // the executable's path, then the padding after it
    let end = rest.iter().position(|b| *b == 0)?;
    rest = &rest[end..];
    let start = rest.iter().position(|b| *b != 0).unwrap_or(rest.len());
    rest = &rest[start..];
    let mut fields = rest.split(|b| *b == 0);
    for _ in 0..argc {
        fields.next()?;
    }
    Some(
        fields
            .take_while(|f| !f.is_empty())
            .map(|f| String::from_utf8_lossy(f).into_owned())
            .collect(),
    )
}

/// The home directory of a user, from the password database.
fn home_of_uid(uid: u32) -> Option<String> {
    let mut buf: Vec<libc::c_char> = vec![0; 16 * 1024];
    // SAFETY: an all-zero passwd is a valid value to be overwritten.
    let mut pw: libc::passwd = unsafe { std::mem::zeroed() };
    let mut out: *mut libc::passwd = std::ptr::null_mut();
    // SAFETY: every pointer is to a live buffer of the length passed.
    let rc = unsafe { libc::getpwuid_r(uid, &mut pw, buf.as_mut_ptr(), buf.len(), &mut out) };
    if rc != 0 || out.is_null() || pw.pw_dir.is_null() {
        return None;
    }
    // SAFETY: getpwuid_r succeeded, so pw_dir points to a NUL-terminated string in `buf`.
    let dir = unsafe { std::ffi::CStr::from_ptr(pw.pw_dir) };
    Some(dir.to_string_lossy().into_owned()).filter(|d| !d.is_empty())
}

// ---------------------------------------------------------------- spellings

/// Repeated separators collapsed and a trailing one dropped: the shape every reading is
/// compared in. `$TMPDIR` ends in a slash on macOS, so `.../T//wt` and `.../T/wt` are one
/// directory that a plain comparison takes for two.
fn collapse(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut slash = false;
    for c in s.chars() {
        if c == '/' {
            if !slash {
                out.push(c);
            }
            slash = true;
        } else {
            out.push(c);
            slash = false;
        }
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

/// Every spelling one directory answers to here: as given, as the file system resolves it,
/// and with or without the `/private` macOS reaches `/tmp` and `/var` through.
fn spellings(p: &Path) -> Vec<String> {
    let mut base = vec![collapse(&p.to_string_lossy())];
    if let Ok(real) = std::fs::canonicalize(p) {
        base.push(collapse(&real.to_string_lossy()));
    }
    let mut out: Vec<String> = Vec::new();
    for s in base {
        let other = if let Some(rest) = s.strip_prefix("/private/") {
            Some(format!("/{rest}"))
        } else if s.starts_with("/tmp/") || s.starts_with("/var/") {
            Some(format!("/private{s}"))
        } else {
            None
        };
        for v in std::iter::once(s).chain(other) {
            if !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// `x` is `dir` or below it.
fn inside(x: &str, dir: &str) -> bool {
    x == dir || (x.starts_with(dir) && x.as_bytes().get(dir.len()) == Some(&b'/'))
}

/// One is the other or below it.
fn meets(a: &str, b: &str) -> bool {
    inside(a, b) || inside(b, a)
}

// ---------------------------------------------------------------- the tree

/// The work tree that holds a path, and what git says about it.
#[derive(Debug, Clone, Default)]
struct Tree {
    root: PathBuf,
    gitdir: Option<PathBuf>,
    branch: Option<String>,
    /// The primary checkout of its repository.
    primary: Option<PathBuf>,
}

impl Tree {
    /// The nearest ancestor of `path`, itself included, that holds a `.git`.
    fn of(path: &Path) -> Option<Tree> {
        let root = path
            .ancestors()
            .find(|a| std::fs::symlink_metadata(a.join(".git")).is_ok())?
            .to_path_buf();
        let gitdir = super::state::git_dir_of(&root);
        let branch = gitdir.as_deref().and_then(|g| {
            let head = std::fs::read_to_string(g.join("HEAD")).ok()?;
            head.trim()
                .strip_prefix("ref: refs/heads/")
                .map(str::to_string)
        });
        let primary = gitdir.as_deref().and_then(|g| {
            let common = match std::fs::read_to_string(g.join("commondir")) {
                Ok(c) => {
                    let c = PathBuf::from(c.trim());
                    if c.is_absolute() {
                        c
                    } else {
                        g.join(c)
                    }
                }
                Err(_) => g.to_path_buf(),
            };
            let common = std::fs::canonicalize(&common).unwrap_or(common);
            common.parent().map(Path::to_path_buf)
        });
        Some(Tree {
            root,
            gitdir,
            branch,
            primary,
        })
    }

    /// The issue the branch names: `I<digits>` as a whole segment or before a `-`.
    fn issue(&self) -> Option<String> {
        issue_of_branch(self.branch.as_deref()?)
    }
}

/// The issue a branch name carries, as `I<digits>` at the start of one of its segments,
/// followed by `-` or nothing.
fn issue_of_branch(branch: &str) -> Option<String> {
    branch.split('/').find_map(|seg| {
        let head = seg.split('-').next()?;
        let digits = head.strip_prefix('I')?;
        (!digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit())).then(|| head.to_string())
    })
}

/// Whether the checkout at `root` declares a mesh that is enabled.
fn declares_mesh(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root.join(".ai/repo/mesh")) else {
        return false;
    };
    entries.flatten().any(|e| {
        let p = e.path();
        p.extension().is_some_and(|x| x == "yaml")
            && std::fs::read_to_string(&p)
                .ok()
                .and_then(|t| crate::metadata::yaml::parse_mapping(&t).ok())
                .and_then(|m| m.get("enabled").cloned())
                .is_some_and(|v| v == Value::Bool(true) || v == "true")
    })
}

/// The address of the server holding a checkout's lease, when one is alive: `Ok(None)` when
/// there is none, an error when there is one whose lease cannot be read.
fn server_of(checkout: &Path) -> Result<Option<String>, String> {
    let half = crate::capability::builtin::server::local_half(checkout);
    let path = crate::lease::lease_file(checkout, &half);
    match crate::lease::LeaseFile::read(&path) {
        crate::lease::LeaseFile::Absent => Ok(None),
        crate::lease::LeaseFile::Empty => Err(format!(
            "{} is being written: its server is starting",
            path.display()
        )),
        crate::lease::LeaseFile::Corrupt(why) => Err(format!("{} {why}", path.display())),
        crate::lease::LeaseFile::Document(d) => {
            if !crate::lease::alive(d.pid) {
                return Ok(None);
            }
            d.url.map(Some).ok_or_else(|| {
                format!(
                    "the server of {} is still binding its address",
                    checkout.display()
                )
            })
        }
    }
}

// ---------------------------------------------------------------- the mesh

/// What the mesh of one repository says, loosely read so that a server of another version
/// still answers.
#[derive(Debug, Clone, Default)]
struct Mesh {
    /// Live claims: (key, issue, scope).
    claims: Vec<(String, Option<String>, Vec<String>)>,
    /// Active sessions: (key, branch, issue).
    sessions: Vec<(String, Option<String>, Option<String>)>,
}

/// Read `mesh.state` (full view) as returned by a server.
fn mesh_of_answer(v: &Value) -> Result<Mesh, String> {
    if v["active"] != Value::Bool(true) {
        return Err(v["reason"]
            .as_str()
            .unwrap_or("cooperation is not active")
            .to_string());
    }
    let s = |v: &Value| v.as_str().map(str::to_string);
    let mut mesh = Mesh::default();
    for c in v["state"]["claims"].as_array().into_iter().flatten() {
        // released and expired claims are over; everything else is somebody's
        let state = c["state"]["state"].as_str().unwrap_or("held");
        if matches!(state, "released" | "expired") {
            continue;
        }
        let scope = c["scope"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect();
        mesh.claims
            .push((s(&c["key"]).unwrap_or_default(), s(&c["issue"]), scope));
    }
    for x in v["state"]["sessions"].as_array().into_iter().flatten() {
        if x["state"].as_str() != Some("active") {
            continue;
        }
        mesh.sessions.push((
            s(&x["key"]).unwrap_or_default(),
            s(&x["info"]["branch"]),
            s(&x["info"]["issue"]),
        ));
    }
    Ok(mesh)
}

// ---------------------------------------------------------------- the judge

/// The readings shared by every path, and the per-repository ones taken once.
struct Judge<'a> {
    machine: &'a Machine,
    servers: &'a dyn Servers,
    places: &'a Places,
    idle: Duration,
    now: SystemTime,
    askers: BTreeSet<u32>,
    mesh: BTreeMap<PathBuf, Result<Option<Mesh>, String>>,
    blind: BTreeSet<String>,
    records: Option<Result<Vec<crate::offload::owner::RunRecord>, String>>,
}

impl<'a> Judge<'a> {
    fn new(
        machine: &'a Machine,
        servers: &'a dyn Servers,
        places: &'a Places,
        idle: Duration,
        now: SystemTime,
    ) -> Self {
        Judge {
            machine,
            servers,
            places,
            idle,
            now,
            askers: machine.askers(),
            mesh: BTreeMap::new(),
            blind: BTreeSet::new(),
            records: None,
        }
    }

    fn judge(&mut self, asked: &str) -> PathLiveness {
        let path = PathBuf::from(asked);
        let mut reasons = Vec::new();
        if std::fs::symlink_metadata(&path).is_err() {
            reasons.push(unreadable("it does not exist, or cannot be stat'ed"));
            return PathLiveness {
                path: asked.to_string(),
                verdict: LivenessVerdict::Keep,
                tree: None,
                reasons,
            };
        }
        let names = spellings(&path);
        let tree = Tree::of(&path);
        self.machine_wide(&mut reasons);
        self.processes(&names, &mut reasons);
        self.queue(&names, tree.as_ref(), &mut reasons);
        self.runner(&names, &mut reasons);
        if let Some(t) = &tree {
            task(t, &mut reasons);
            self.git(t, &mut reasons);
            self.derive_lock(t, &mut reasons);
            self.board(t, &mut reasons);
            self.mesh(t, &names, &mut reasons);
        }
        PathLiveness {
            path: asked.to_string(),
            verdict: if reasons.is_empty() {
                LivenessVerdict::Reclaim
            } else {
                LivenessVerdict::Keep
            },
            tree: tree.map(|t| t.root.to_string_lossy().into_owned()),
            reasons,
        }
    }

    /// A machine-wide reading that failed keeps every path.
    fn machine_wide(&mut self, reasons: &mut Vec<LivenessReason>) {
        let m = self.machine;
        for (what, r) in [
            ("the process table", m.processes.as_ref().map(|_| ())),
            (
                "the working directories",
                m.directories.as_ref().map(|_| ()),
            ),
            (
                "the process environments",
                m.environments.as_ref().map(|_| ()),
            ),
        ] {
            if let Err(why) = r {
                self.blind.insert(format!("{what}: {why}"));
                reasons.push(unreadable(&format!("{what} could not be read: {why}")));
            }
        }
    }

    /// Another process works in it, builds into it, runs from it, or names it.
    fn processes(&self, names: &[String], reasons: &mut Vec<LivenessReason>) {
        let Ok(procs) = &self.machine.processes else {
            return;
        };
        let mut by: BTreeMap<LivenessSignal, Vec<String>> = BTreeMap::new();
        for p in procs.iter().filter(|p| p.pid != self.machine.me) {
            let who = format!("pid {} ({})", p.pid, command_word(&p.args));
            if let Some(cwd) = &p.cwd {
                if names.iter().any(|n| inside(cwd, n)) {
                    by.entry(LivenessSignal::ProcessDirectory)
                        .or_default()
                        .push(format!("{who} has its working directory inside it"));
                }
            }
            for (k, v) in p.env.iter().flatten().filter(|(k, _)| k != "HOME") {
                let value = collapse(v);
                if value == "/" || !value.starts_with('/') {
                    continue;
                }
                let values = spellings(Path::new(&value));
                if names.iter().any(|n| values.iter().any(|v| meets(v, n))) {
                    by.entry(LivenessSignal::ProcessEnvironment)
                        .or_default()
                        .push(format!("{who} has {k}={value}"));
                }
            }
            if !self.askers.contains(&p.pid) && names.iter().any(|n| p.args.contains(n.as_str())) {
                by.entry(LivenessSignal::ProcessArguments)
                    .or_default()
                    .push(format!("{who} names it on its command line"));
            }
        }
        for (signal, list) in by {
            reasons.push(LivenessReason {
                signal,
                detail: named(&list),
            });
        }
    }

    /// The offload run records whose run lives in it, and the offload lock's live pids that
    /// work in it.
    fn queue(&mut self, names: &[String], tree: Option<&Tree>, reasons: &mut Vec<LivenessReason>) {
        let mut dirs: Vec<String> = names.to_vec();
        if let Some(t) = tree {
            dirs.extend(spellings(&t.root));
        }
        let records = self
            .records
            .get_or_insert_with(|| read_records(&self.places.offload_runs))
            .clone();
        match records {
            Err(why) => {
                self.blind.insert(format!("the offload run records: {why}"));
                reasons.push(unreadable(&format!(
                    "the offload run records could not be read: {why}"
                )));
            }
            Ok(records) => {
                for r in records.iter().filter(|r| !r.cwd.is_empty()) {
                    let cwd = collapse(&r.cwd);
                    if dirs.iter().any(|d| meets(&cwd, d)) && crate::lease::alive(r.pid) {
                        reasons.push(LivenessReason {
                            signal: LivenessSignal::QueuePidFile,
                            detail: format!("offload run {} (pid {}) runs in {cwd}", r.tag, r.pid),
                        });
                    }
                }
            }
        }
        let Ok(entries) = std::fs::read_dir(&self.places.offload_lock) else {
            return;
        };
        let procs = self.machine.processes.as_ref().ok();
        for e in entries.flatten() {
            let Some(pid) = e.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
                continue;
            };
            if !crate::lease::alive(pid) {
                continue;
            }
            let Some(p) = procs.and_then(|ps| ps.iter().find(|p| p.pid == pid)) else {
                continue;
            };
            let works = p
                .cwd
                .as_deref()
                .is_some_and(|c| dirs.iter().any(|d| inside(c, d)))
                || dirs.iter().any(|d| p.args.contains(d.as_str()));
            if works {
                reasons.push(LivenessReason {
                    signal: LivenessSignal::QueuePidFile,
                    detail: format!("{} names pid {pid}, which works in it", e.path().display()),
                });
            }
        }
    }

    /// A CI runner job runs, and the home it runs under holds the path.
    fn runner(&mut self, names: &[String], reasons: &mut Vec<LivenessReason>) {
        let Ok(procs) = &self.machine.processes else {
            return;
        };
        for p in procs.iter().filter(|p| p.args.contains("Runner.Worker")) {
            let home = p
                .env
                .iter()
                .flatten()
                .find(|(k, _)| k == "HOME")
                .map(|(_, v)| collapse(v))
                .or_else(|| home_of_uid(p.uid));
            match home {
                Some(h) if h != "/" => {
                    if names.iter().any(|n| inside(n, &h)) {
                        reasons.push(LivenessReason {
                            signal: LivenessSignal::RunnerJob,
                            detail: format!(
                                "a runner job runs (pid {}) under {h}, which holds it",
                                p.pid
                            ),
                        });
                    }
                }
                _ => {
                    let why = format!("the home of runner job pid {} could not be read", p.pid);
                    self.blind.insert(why.clone());
                    reasons.push(unreadable(&why));
                }
            }
        }
    }

    /// Its index, `HEAD` or `HEAD` reflog changed within the idle window.
    fn git(&self, t: &Tree, reasons: &mut Vec<LivenessReason>) {
        let Some(gitdir) = &t.gitdir else {
            reasons.push(unreadable(&format!(
                "the git directory of {} could not be found",
                t.root.display()
            )));
            return;
        };
        for (file, required) in [("HEAD", true), ("index", false), ("logs/HEAD", false)] {
            match std::fs::metadata(gitdir.join(file)).and_then(|m| m.modified()) {
                Ok(at) => {
                    let age = self.now.duration_since(at).unwrap_or_default();
                    if age < self.idle {
                        reasons.push(LivenessReason {
                            signal: LivenessSignal::GitActivity,
                            detail: format!(
                                "its {file} changed {}s ago, within the {}s idle window",
                                age.as_secs(),
                                self.idle.as_secs()
                            ),
                        });
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound && !required => {}
                Err(e) => reasons.push(unreadable(&format!(
                    "{} could not be stat'ed ({e})",
                    gitdir.join(file).display()
                ))),
            }
        }
    }

    /// The derive lock beside the primary checkout, whose live owner names its branch.
    fn derive_lock(&self, t: &Tree, reasons: &mut Vec<LivenessReason>) {
        let lock = match (&self.places.derive_lock, &t.primary) {
            (Some(l), _) => l.clone(),
            (None, Some(p)) => match p.parent() {
                Some(home) => home.join(".derive.lock"),
                None => return,
            },
            (None, None) => return,
        };
        if !lock.is_dir() {
            return;
        }
        let owner = match std::fs::read_to_string(lock.join("owner")) {
            Ok(o) => o,
            Err(e) => {
                reasons.push(unreadable(&format!(
                    "the derive lock {} is held and its owner could not be read ({e})",
                    lock.display()
                )));
                return;
            }
        };
        let line = owner.lines().next().unwrap_or("").trim();
        let (branch, pid) = derive_owner(line);
        if pid.is_some_and(|p| !crate::lease::alive(p)) {
            return;
        }
        let root = t.root.to_string_lossy();
        let names_it = (branch.is_some() && branch.as_deref() == t.branch.as_deref())
            || line.contains(root.as_ref());
        if names_it {
            reasons.push(LivenessReason {
                signal: LivenessSignal::DeriveLock,
                detail: format!("the derive lock's owner `{line}` names it"),
            });
        }
    }

    /// A peer attached to, or announcing on, the tree's own server.
    fn board(&self, t: &Tree, reasons: &mut Vec<LivenessReason>) {
        let url = match server_of(&t.root) {
            Ok(Some(url)) => url,
            Ok(None) => return,
            Err(why) => {
                reasons.push(unreadable(&format!("its board could not be read: {why}")));
                return;
            }
        };
        let Some(path) = route("peers.list") else {
            reasons.push(unreadable("peers.list declares no HTTP route"));
            return;
        };
        let peers = match self.servers.get(&url, &format!("{path}?checkouts=this")) {
            Ok(v) => v,
            Err(why) => {
                reasons.push(unreadable(&format!("its board could not be read: {why}")));
                return;
            }
        };
        let mut seen = Vec::new();
        for p in peers["peers"].as_array().into_iter().flatten() {
            let id = p["id"].as_str().unwrap_or("?");
            let client = p["client"]["name"].as_str().unwrap_or("?");
            let claims = p["claims"].as_array().map_or(0, Vec::len);
            if p["attached"] == Value::Bool(true) {
                seen.push(format!("{id} ({client}) is attached"));
            } else if claims > 0 || !p["announcement"].is_null() {
                seen.push(format!("{id} ({client}) announced work here"));
            }
        }
        if !seen.is_empty() {
            reasons.push(LivenessReason {
                signal: LivenessSignal::BoardClaim,
                detail: format!("its board at {url}: {}", named(&seen)),
            });
        }
    }

    /// A live mesh claim or an active session of the repository's mesh names it.
    fn mesh(&mut self, t: &Tree, names: &[String], reasons: &mut Vec<LivenessReason>) {
        let key = t.primary.clone().unwrap_or_else(|| t.root.clone());
        let reading = match self.mesh.get(&key) {
            Some(r) => r.clone(),
            None => {
                let r = self.read_mesh(t);
                self.mesh.insert(key.clone(), r.clone());
                r
            }
        };
        let mesh = match reading {
            Ok(Some(m)) => m,
            Ok(None) => return,
            Err(why) => {
                let what = format!("the mesh claims of {}: {why}", key.display());
                self.blind.insert(what.clone());
                reasons.push(unreadable(&what));
                return;
            }
        };
        let issue = t.issue();
        let mut roots = spellings(&t.root);
        roots.extend(names.iter().cloned());
        let mut seen = Vec::new();
        for (claim, claim_issue, scope) in &mesh.claims {
            if issue.is_some() && claim_issue == &issue {
                seen.push(format!(
                    "claim {claim} is for {}",
                    issue.as_deref().unwrap_or("")
                ));
            } else if let Some(s) = scope
                .iter()
                .map(|s| collapse(s))
                .find(|s| s.starts_with('/') && roots.iter().any(|r| meets(s, r)))
            {
                seen.push(format!("claim {claim} covers {s}"));
            }
        }
        for (session, branch, session_issue) in &mesh.sessions {
            if t.branch.is_some() && branch == &t.branch {
                seen.push(format!(
                    "session {session} works on {}",
                    t.branch.as_deref().unwrap_or("")
                ));
            } else if issue.is_some() && session_issue == &issue {
                seen.push(format!(
                    "session {session} works on {}",
                    issue.as_deref().unwrap_or("")
                ));
            }
        }
        if !seen.is_empty() {
            reasons.push(LivenessReason {
                signal: LivenessSignal::MeshClaim,
                detail: named(&seen),
            });
        }
    }

    /// The mesh of the tree's repository: `None` when it declares none, else what the tree's
    /// own server, or the primary checkout's, says. Declared and unreadable is an error.
    fn read_mesh(&self, t: &Tree) -> Result<Option<Mesh>, String> {
        let primary = t.primary.as_deref();
        if !declares_mesh(&t.root) && !primary.is_some_and(declares_mesh) {
            return Ok(None);
        }
        let mut why = Vec::new();
        let mut checkouts = vec![t.root.as_path()];
        checkouts.extend(primary.filter(|p| *p != t.root));
        let path = route("mesh.state").ok_or("mesh.state declares no HTTP route")?;
        for c in checkouts {
            match server_of(c) {
                Ok(Some(url)) => match self
                    .servers
                    .get(&url, &format!("{path}?view=full"))
                    .and_then(|v| mesh_of_answer(&v))
                {
                    Ok(m) => return Ok(Some(m)),
                    Err(e) => why.push(e),
                },
                Ok(None) => why.push(format!("no server of {} is running", c.display())),
                Err(e) => why.push(e),
            }
        }
        Err(format!("the mesh is declared and {}", why.join("; ")))
    }
}

/// The open task of a tree.
fn task(t: &Tree, reasons: &mut Vec<LivenessReason>) {
    let path = t.root.join(TASK_RECORD);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            reasons.push(unreadable(&format!(
                "its task record {} could not be read ({e})",
                path.display()
            )));
            return;
        }
    };
    if text.trim().is_empty() {
        return;
    }
    match crate::capability::builtin::continuity::read_task(&path) {
        Some(task) if task.outcome == "active" => reasons.push(LivenessReason {
            signal: LivenessSignal::Task,
            detail: format!("task {} is open in it: {}", task.id, task.task),
        }),
        Some(_) => {}
        None => reasons.push(unreadable(&format!(
            "its task record {} is not a task record",
            path.display()
        ))),
    }
}

/// The branch and pid of a derive lock's owner line, `<who>:<branch> <pid>` or with a
/// `pid=<n>` word: the branch after the last `:` of the first word, the pid only when it
/// is a number.
fn derive_owner(line: &str) -> (Option<String>, Option<u32>) {
    let words: Vec<&str> = line.split_whitespace().collect();
    let pid = words
        .iter()
        .find_map(|w| w.strip_prefix("pid=").and_then(|p| p.parse().ok()))
        .or_else(|| {
            words
                .last()
                .and_then(|w| w.parse().ok())
                .filter(|_| words.len() > 1)
        });
    let branch = words
        .first()
        .and_then(|w| w.rsplit_once(':'))
        .map(|(_, b)| b.to_string())
        .filter(|b| !b.is_empty());
    (branch, pid)
}

/// The offload run records in a directory: none when it does not exist.
fn read_records(dir: &Path) -> Result<Vec<crate::offload::owner::RunRecord>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{} ({e})", dir.display())),
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "run") {
            continue;
        }
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{} ({e})", p.display()))?;
        out.push(
            crate::offload::owner::RunRecord::parse(&text)
                .map_err(|e| format!("{}: {e}", p.display()))?,
        );
    }
    Ok(out)
}

fn unreadable(detail: &str) -> LivenessReason {
    LivenessReason {
        signal: LivenessSignal::Unreadable,
        detail: detail.to_string(),
    }
}

/// The first word of a command line, without its directory.
fn command_word(args: &str) -> String {
    let first = args.split_whitespace().next().unwrap_or("?");
    first.rsplit('/').next().unwrap_or(first).to_string()
}

/// The first few, then how many more.
fn named(list: &[String]) -> String {
    let mut s = list
        .iter()
        .take(NAMED)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if list.len() > NAMED {
        s.push_str(&format!(" and {} more", list.len() - NAMED));
    }
    s
}

// ---------------------------------------------------------------- the answer

/// Judge every path against readings already taken. No path is deleted, and nothing is
/// written anywhere.
pub(crate) fn judge(
    paths: &[String],
    idle_seconds: u64,
    machine: &Machine,
    servers: &dyn Servers,
    places: &Places,
    now: SystemTime,
) -> LivenessReport {
    let mut j = Judge::new(
        machine,
        servers,
        places,
        Duration::from_secs(idle_seconds),
        now,
    );
    let answers: Vec<PathLiveness> = paths.iter().map(|p| j.judge(p)).collect();
    LivenessReport {
        schema: SCHEMA.into(),
        idle_seconds,
        complete: j.blind.is_empty(),
        blind: j.blind.into_iter().collect(),
        paths: answers,
    }
}

/// The idle window a caller wrote (`30m`, `2h`), or the default when it wrote none.
pub(crate) fn idle_seconds(idle: Option<&str>) -> Result<u64, String> {
    match idle {
        None => Ok(DEFAULT_IDLE_SECONDS),
        Some(text) => crate::cli::parse_duration(text).map_err(|e| format!("idle: {e}")),
    }
}

/// Make every path absolute against `base`.
pub(crate) fn absolute(paths: &[String], base: &Path) -> Vec<String> {
    paths
        .iter()
        .map(|p| {
            if Path::new(p).is_absolute() {
                p.clone()
            } else {
                base.join(p).to_string_lossy().into_owned()
            }
        })
        .collect()
}

/// Answer for `paths` from this machine as it is now. The machine is read only when there
/// is a path to judge.
pub(crate) fn answer(paths: &[String], idle_seconds: u64) -> LivenessReport {
    let machine = if paths.iter().any(|p| std::fs::symlink_metadata(p).is_ok()) {
        read_machine()
    } else {
        Machine {
            me: std::process::id(),
            processes: Ok(Vec::new()),
            directories: Ok(()),
            environments: Ok(()),
        }
    };
    judge(
        paths,
        idle_seconds,
        &machine,
        &Http,
        &Places::of_env(),
        SystemTime::now(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set in the child a test starts from this binary to hold an environment open.
    const HOLD: &str = "MJ_LIVENESS_TEST_HOLD";

    /// A server that is never there: every question about a board or the mesh fails.
    struct Nobody;
    impl Servers for Nobody {
        fn get(&self, url: &str, _: &str) -> Result<Value, String> {
            Err(format!("{url} is not there"))
        }
    }

    /// Servers that answer from a table keyed by the route's capability.
    struct Answering(Value, Value);
    impl Servers for Answering {
        fn get(&self, _: &str, path: &str) -> Result<Value, String> {
            if path.starts_with(&route("mesh.state").unwrap()) {
                Ok(self.0.clone())
            } else {
                Ok(self.1.clone())
            }
        }
    }

    fn git(cwd: &Path, args: &[&str]) {
        let out = std::process::Command::new("git")
            .args(["-c", "user.email=t@t", "-c", "user.name=t"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(["-c", "init.defaultBranch=master"])
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Set a file's modification time to 2020.
    fn age(path: &Path) {
        let then = SystemTime::UNIX_EPOCH + Duration::from_secs(1_577_836_800);
        if let Ok(f) = std::fs::File::options().write(true).open(path) {
            f.set_modified(then).expect("set the time");
        }
    }

    /// A repository with a linked worktree on `feature/I4346-x`, every git file of the
    /// worktree aged past any window, a build directory inside it, and empty places.
    struct Fixture {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        wt: PathBuf,
        target: PathBuf,
        places: Places,
    }

    impl Fixture {
        fn new() -> Fixture {
            let tmp = tempfile::tempdir().unwrap();
            let root = tmp.path().canonicalize().unwrap();
            let repo = root.join("repo");
            std::fs::create_dir_all(&repo).unwrap();
            git(&repo, &["init", "-q"]);
            std::fs::write(repo.join("a.txt"), "a\n").unwrap();
            git(&repo, &["add", "-A"]);
            git(&repo, &["commit", "-qm", "a"]);
            let wt = root.join("repo-wt/feature/I4346-x");
            git(
                &repo,
                &[
                    "worktree",
                    "add",
                    "-q",
                    "-b",
                    "feature/I4346-x",
                    wt.to_str().unwrap(),
                ],
            );
            let target = wt.join("apps/majordomus-cli/target");
            std::fs::create_dir_all(target.join("debug")).unwrap();
            let f = Fixture {
                _tmp: tmp,
                root: root.clone(),
                wt,
                target,
                places: Places {
                    offload_runs: root.join("offload-queue.runs"),
                    offload_lock: root.join("offload-derive.lock"),
                    derive_lock: None,
                },
            };
            f.age_git();
            f
        }

        fn gitdir(&self) -> PathBuf {
            super::super::state::git_dir_of(&self.wt).unwrap()
        }

        fn age_git(&self) {
            for f in ["HEAD", "index", "logs/HEAD"] {
                age(&self.gitdir().join(f));
            }
        }

        fn ask(&self, machine: &Machine, servers: &dyn Servers) -> PathLiveness {
            let mut r = judge(
                &[self.target.to_string_lossy().into_owned()],
                DEFAULT_IDLE_SECONDS,
                machine,
                servers,
                &self.places,
                SystemTime::now(),
            );
            r.paths.remove(0)
        }
    }

    /// A machine whose every reading succeeded, with the processes given.
    fn machine(processes: Vec<Process>) -> Machine {
        Machine {
            me: std::process::id(),
            processes: Ok(processes),
            directories: Ok(()),
            environments: Ok(()),
        }
    }

    fn signals(p: &PathLiveness) -> Vec<LivenessSignal> {
        p.reasons.iter().map(|r| r.signal).collect()
    }

    fn other(pid: u32) -> Process {
        Process {
            pid,
            ppid: 1,
            uid: 501,
            args: "zsh".into(),
            ..Process::default()
        }
    }

    #[test]
    fn a_path_no_signal_names_is_reclaimed() {
        let f = Fixture::new();
        let p = f.ask(&machine(vec![other(999_999)]), &Nobody);
        assert_eq!(p.verdict, LivenessVerdict::Reclaim, "{:?}", p.reasons);
        assert!(p.reasons.is_empty());
        assert_eq!(p.tree.as_deref(), Some(f.wt.to_str().unwrap()));
    }

    #[test]
    fn an_open_task_keeps_it() {
        let f = Fixture::new();
        let state = f.wt.join(".ai/local/state");
        std::fs::create_dir_all(&state).unwrap();
        std::fs::write(
            state.join("current.yaml"),
            "id: t-1\ntask: build\noutcome: active\n",
        )
        .unwrap();
        let p = f.ask(&machine(Vec::new()), &Nobody);
        assert_eq!(
            (p.verdict, signals(&p)),
            (LivenessVerdict::Keep, vec![LivenessSignal::Task])
        );
        assert!(p.reasons[0].detail.contains("t-1"), "{:?}", p.reasons);
        // a finished one does not, and one that is not a record is not knowing
        std::fs::write(state.join("current.yaml"), "id: t-1\noutcome: completed\n").unwrap();
        assert_eq!(
            f.ask(&machine(Vec::new()), &Nobody).verdict,
            LivenessVerdict::Reclaim
        );
        std::fs::write(state.join("current.yaml"), "outcome: active\n").unwrap();
        assert_eq!(
            signals(&f.ask(&machine(Vec::new()), &Nobody)),
            vec![LivenessSignal::Unreadable]
        );
    }

    #[test]
    fn a_head_written_ten_minutes_ago_keeps_it() {
        let f = Fixture::new();
        let head = f.gitdir().join("HEAD");
        let ten = SystemTime::now() - Duration::from_secs(600);
        std::fs::File::options()
            .write(true)
            .open(&head)
            .unwrap()
            .set_modified(ten)
            .unwrap();
        let p = f.ask(&machine(Vec::new()), &Nobody);
        assert_eq!(
            (p.verdict, signals(&p)),
            (LivenessVerdict::Keep, vec![LivenessSignal::GitActivity])
        );
        assert!(p.reasons[0].detail.contains("HEAD"), "{:?}", p.reasons);
        // outside a five-minute window it is idle
        let r = judge(
            &[f.target.to_string_lossy().into_owned()],
            300,
            &machine(Vec::new()),
            &Nobody,
            &f.places,
            SystemTime::now(),
        );
        assert_eq!(r.paths[0].verdict, LivenessVerdict::Reclaim);
    }

    #[test]
    fn a_claim_for_its_issue_keeps_it_and_a_declared_mesh_nobody_answers_for_keeps_everything() {
        let f = Fixture::new();
        let repo = f.root.join("repo");
        std::fs::create_dir_all(repo.join(".ai/repo/mesh")).unwrap();
        std::fs::write(
            repo.join(".ai/repo/mesh/m.yaml"),
            "kind: mesh\nenabled: true\n",
        )
        .unwrap();
        // no server holds a lease: the declared mesh cannot be read
        let r = judge(
            &[f.target.to_string_lossy().into_owned()],
            DEFAULT_IDLE_SECONDS,
            &machine(Vec::new()),
            &Nobody,
            &f.places,
            SystemTime::now(),
        );
        assert!(!r.complete);
        assert!(r.blind[0].contains("mesh"), "{:?}", r.blind);
        assert_eq!(signals(&r.paths[0]), vec![LivenessSignal::Unreadable]);

        // a live server holds the primary checkout's lease and answers with a claim for I4346
        let lease = crate::lease::lease_file(&repo, ".ai/local");
        std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
        std::fs::write(
            &lease,
            serde_json::json!({
                "schema": crate::lease::SCHEMA, "pid": std::process::id(), "token": "t",
                "root": repo, "url": "http://127.0.0.1:9",
            })
            .to_string(),
        )
        .unwrap();
        let mesh = serde_json::json!({"active": true, "state": {
            "claims": [
                {"key": "s/c1", "issue": "I4346", "scope": ["apps"], "state": {"state": "held"}},
                {"key": "s/c2", "issue": "I1", "scope": [f.wt], "state": {"state": "released"}},
            ],
            "sessions": [],
        }});
        let p = f.ask(&machine(Vec::new()), &Answering(mesh, Value::Null));
        assert_eq!(
            (p.verdict, signals(&p)),
            (LivenessVerdict::Keep, vec![LivenessSignal::MeshClaim])
        );
        assert!(p.reasons[0].detail.contains("s/c1"), "{:?}", p.reasons);
        assert!(
            !p.reasons[0].detail.contains("s/c2"),
            "a released claim is over"
        );

        // a session on the branch, and a claim over the tree's absolute path, name it too
        let mesh = serde_json::json!({"active": true, "state": {
            "claims": [{"key": "s/c3", "scope": [f.wt], "state": {"state": "held"}}],
            "sessions": [{"key": "s/x", "state": "active",
                          "info": {"branch": "feature/I4346-x"}}],
        }});
        let p = f.ask(&machine(Vec::new()), &Answering(mesh, Value::Null));
        assert!(p.reasons[0].detail.contains("s/c3"), "{:?}", p.reasons);
        assert!(
            p.reasons[0].detail.contains("session s/x"),
            "{:?}",
            p.reasons
        );

        // a server that says cooperation is off has not answered for the declared mesh
        let off = serde_json::json!({"active": false, "reason": "switched off"});
        let r = judge(
            &[f.target.to_string_lossy().into_owned()],
            DEFAULT_IDLE_SECONDS,
            &machine(Vec::new()),
            &Answering(off, Value::Null),
            &f.places,
            SystemTime::now(),
        );
        assert!(
            !r.complete && r.blind[0].contains("switched off"),
            "{:?}",
            r.blind
        );
    }

    #[test]
    fn a_peer_on_its_own_board_keeps_it() {
        let f = Fixture::new();
        let lease = crate::lease::lease_file(&f.wt, ".ai/local");
        std::fs::create_dir_all(lease.parent().unwrap()).unwrap();
        std::fs::write(
            &lease,
            serde_json::json!({
                "schema": crate::lease::SCHEMA, "pid": std::process::id(), "token": "t",
                "root": f.wt, "url": "http://127.0.0.1:9",
            })
            .to_string(),
        )
        .unwrap();
        let board = serde_json::json!({"peers": [
            {"id": "p1", "client": {"name": "claude-code"}, "attached": true, "claims": []},
        ]});
        let p = f.ask(&machine(Vec::new()), &Answering(Value::Null, board));
        assert_eq!(
            (p.verdict, signals(&p)),
            (LivenessVerdict::Keep, vec![LivenessSignal::BoardClaim])
        );
        // a live server that does not answer is not knowing
        let p = f.ask(&machine(Vec::new()), &Nobody);
        assert_eq!(signals(&p), vec![LivenessSignal::Unreadable]);
    }

    #[test]
    fn an_offload_pid_file_or_run_record_keeps_it() {
        let f = Fixture::new();
        // a run record of a live run in the worktree
        std::fs::create_dir_all(&f.places.offload_runs).unwrap();
        std::fs::write(
            f.places.offload_runs.join("abc.run"),
            format!(
                "{}\ntag abc\npid {}\nhost h\ncwd {}\n",
                crate::offload::owner::RUN_SCHEMA,
                std::process::id(),
                f.wt.display()
            ),
        )
        .unwrap();
        let p = f.ask(&machine(Vec::new()), &Nobody);
        assert_eq!(
            (p.verdict, signals(&p)),
            (LivenessVerdict::Keep, vec![LivenessSignal::QueuePidFile])
        );
        assert!(p.reasons[0].detail.contains("abc"), "{:?}", p.reasons);
        // a record that cannot be read is a blind source
        std::fs::write(f.places.offload_runs.join("abc.run"), "garbage\n").unwrap();
        let r = judge(
            &[f.target.to_string_lossy().into_owned()],
            DEFAULT_IDLE_SECONDS,
            &machine(Vec::new()),
            &Nobody,
            &f.places,
            SystemTime::now(),
        );
        assert!(
            !r.complete && r.blind[0].contains("offload"),
            "{:?}",
            r.blind
        );
        std::fs::remove_dir_all(&f.places.offload_runs).unwrap();

        // the offload lock names a live pid that works in the worktree
        std::fs::create_dir_all(&f.places.offload_lock).unwrap();
        let me = std::process::id();
        std::fs::write(f.places.offload_lock.join(me.to_string()), "").unwrap();
        std::fs::write(f.places.offload_lock.join("not-a-pid"), "").unwrap();
        let mut derive = other(me);
        derive.cwd = Some(f.wt.to_string_lossy().into_owned());
        let m = Machine {
            me: 1, // the derive is another process than the one asking
            ..machine(vec![derive])
        };
        let p = f.ask(&m, &Nobody);
        assert!(
            signals(&p).contains(&LivenessSignal::QueuePidFile),
            "{:?}",
            p.reasons
        );
    }

    #[test]
    fn a_running_runner_job_keeps_what_is_under_its_home() {
        let f = Fixture::new();
        let mut job = other(999_998);
        job.args = "/home/r/actions-runner/bin/Runner.Worker spawnclient 1 2".into();
        job.env = Some(vec![("HOME".into(), f.root.to_string_lossy().into_owned())]);
        let p = f.ask(&machine(vec![job.clone()]), &Nobody);
        assert_eq!(
            (p.verdict, signals(&p)),
            (LivenessVerdict::Keep, vec![LivenessSignal::RunnerJob])
        );
        // one whose home is somewhere else does not
        job.env = Some(vec![("HOME".into(), "/nowhere/else".into())]);
        assert_eq!(
            f.ask(&machine(vec![job.clone()]), &Nobody).verdict,
            LivenessVerdict::Reclaim
        );
        // and one whose home cannot be told keeps everything: its own uid's home is read from
        // the password database, and a uid nobody has has none
        job.env = None;
        job.uid = 4_000_000_000;
        let p = f.ask(&machine(vec![job]), &Nobody);
        assert_eq!(signals(&p), vec![LivenessSignal::Unreadable]);
    }

    #[test]
    fn a_process_that_builds_into_it_works_in_it_or_names_it_keeps_it() {
        let f = Fixture::new();
        let t = f.target.to_string_lossy().into_owned();
        let mut builder = other(999_997);
        builder.env = Some(vec![
            ("CARGO_TARGET_DIR".into(), t.clone()),
            ("HOME".into(), f.root.to_string_lossy().into_owned()),
        ]);
        let p = f.ask(&machine(vec![builder]), &Nobody);
        assert_eq!(
            (p.verdict, signals(&p)),
            (
                LivenessVerdict::Keep,
                vec![LivenessSignal::ProcessEnvironment]
            )
        );
        assert!(
            p.reasons[0].detail.contains("CARGO_TARGET_DIR"),
            "{:?}",
            p.reasons
        );

        // the executable a session runs, from inside the build directory
        let mut session = other(999_996);
        session.env = Some(vec![(
            MAJORDOMUS_BIN.into(),
            format!("{t}/debug//majordomus"),
        )]);
        assert_eq!(
            signals(&f.ask(&machine(vec![session]), &Nobody)),
            vec![LivenessSignal::ProcessEnvironment]
        );

        let mut inside_it = other(999_995);
        inside_it.cwd = Some(f.wt.to_string_lossy().into_owned());
        let p = f.ask(&machine(vec![inside_it.clone()]), &Nobody);
        // the worktree holds the target, so standing in the worktree is not standing in it
        assert_eq!(p.verdict, LivenessVerdict::Reclaim, "{:?}", p.reasons);
        inside_it.cwd = Some(format!("{t}/debug"));
        let p = f.ask(&machine(vec![inside_it]), &Nobody);
        assert_eq!(signals(&p), vec![LivenessSignal::ProcessDirectory]);
        assert!(
            p.reasons[0].detail.contains("working directory inside"),
            "{p:?}"
        );

        let mut naming = other(999_994);
        naming.args = format!("tail -f {t}/MARKER");
        assert_eq!(
            signals(&f.ask(&machine(vec![naming.clone()]), &Nobody)),
            vec![LivenessSignal::ProcessArguments]
        );
        // the process that asked, and its callers, are not counted for the paths they hand in
        naming.pid = std::process::id();
        assert_eq!(
            f.ask(&machine(vec![naming]), &Nobody).verdict,
            LivenessVerdict::Reclaim
        );
    }

    #[test]
    fn a_real_process_whose_cargo_target_dir_points_inside_is_read_and_keeps_it() {
        // The child is this test binary, run again as this one test: macOS withholds the
        // environment of Apple's platform binaries (`/bin/sleep`, `/bin/zsh`) from every
        // reader, and this one is not one of them.
        if std::env::var_os(HOLD).is_some() {
            std::thread::sleep(Duration::from_secs(30));
            return;
        }
        let f = Fixture::new();
        let mut child = std::process::Command::new(std::env::current_exe().expect("this binary"))
            .args([
                "--exact",
                "worktree::liveness::tests::a_real_process_whose_cargo_target_dir_points_inside_is_read_and_keeps_it",
                "--test-threads",
                "1",
            ])
            .env(HOLD, "1")
            .env("CARGO_TARGET_DIR", &f.target)
            .current_dir(&f.root)
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("start a child");
        let mut m = read_machine();
        let mut p = f.ask(&m, &Nobody);
        for _ in 0..50 {
            if signals(&p).contains(&LivenessSignal::ProcessEnvironment) {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
            m = read_machine();
            p = f.ask(&m, &Nobody);
        }
        let _ = child.kill();
        let _ = child.wait();
        if m.processes.is_err() || m.directories.is_err() {
            assert_eq!(
                p.verdict,
                LivenessVerdict::Keep,
                "a machine that cannot be read keeps it"
            );
            return;
        }
        assert!(
            signals(&p).contains(&LivenessSignal::ProcessEnvironment),
            "the child's CARGO_TARGET_DIR was not seen: {:?}",
            p.reasons
        );
        assert!(m.askers().contains(&std::process::id()));
    }

    #[test]
    fn a_machine_that_cannot_be_read_keeps_everything_and_says_so() {
        let f = Fixture::new();
        let blind = Machine {
            me: 1,
            processes: Err("ps listed no process".into()),
            directories: Err("the process table was not read".into()),
            environments: Ok(()),
        };
        let r = judge(
            &[f.target.to_string_lossy().into_owned()],
            DEFAULT_IDLE_SECONDS,
            &blind,
            &Nobody,
            &f.places,
            SystemTime::now(),
        );
        assert!(!r.complete);
        assert_eq!(r.blind.len(), 2, "{:?}", r.blind);
        assert_eq!(r.paths[0].verdict, LivenessVerdict::Keep);
        // a path that is not there is kept, and asking about it reads no machine
        let gone = answer(&[f.root.join("gone").to_string_lossy().into_owned()], 60);
        assert_eq!(gone.paths[0].verdict, LivenessVerdict::Keep);
        assert!(gone.complete);
        assert_eq!(gone.paths[0].reasons[0].signal, LivenessSignal::Unreadable);
    }

    #[test]
    fn the_loopback_servers_are_asked_over_http_and_a_silence_is_an_error() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}", server.server_addr().to_ip().unwrap());
        let serve = std::thread::spawn(move || {
            for (i, body) in ["{\"peers\":[]}", "not json", "gone"]
                .into_iter()
                .enumerate()
            {
                let rq = server.recv().unwrap();
                let code: u16 = if i == 2 { 404 } else { 200 };
                rq.respond(tiny_http::Response::from_string(body).with_status_code(code))
                    .unwrap();
            }
        });
        assert_eq!(
            Http.get(&url, "/x").unwrap()["peers"],
            serde_json::json!([])
        );
        assert!(Http.get(&url, "/x").unwrap_err().contains("no JSON"));
        assert!(Http.get(&url, "/x").unwrap_err().contains("404"));
        serve.join().unwrap();
        assert!(Http
            .get("http://127.0.0.1:1", "/x")
            .unwrap_err()
            .contains("did not answer"));
    }

    #[test]
    fn the_derive_lock_keeps_the_branch_its_live_owner_names() {
        let f = Fixture::new();
        let lock = f.root.join(".derive.lock");
        std::fs::create_dir_all(&lock).unwrap();
        // held with no owner written: not knowing
        assert_eq!(
            signals(&f.ask(&machine(Vec::new()), &Nobody)),
            vec![LivenessSignal::Unreadable]
        );
        std::fs::write(
            lock.join("owner"),
            format!("p2:feature/I4346-x {}\n", std::process::id()),
        )
        .unwrap();
        assert_eq!(
            signals(&f.ask(&machine(Vec::new()), &Nobody)),
            vec![LivenessSignal::DeriveLock]
        );
        // another branch's derive, or a dead owner's, does not keep it
        std::fs::write(lock.join("owner"), "p2:fix/other 1\n").unwrap();
        assert_eq!(
            f.ask(&machine(Vec::new()), &Nobody).verdict,
            LivenessVerdict::Reclaim
        );
        std::fs::write(lock.join("owner"), "p2:feature/I4346-x pid=4000000000\n").unwrap();
        assert_eq!(
            f.ask(&machine(Vec::new()), &Nobody).verdict,
            LivenessVerdict::Reclaim
        );
        // the environment moves the lock
        let mut places = f.places.clone();
        places.derive_lock = Some(f.root.join("elsewhere"));
        let r = judge(
            &[f.target.to_string_lossy().into_owned()],
            DEFAULT_IDLE_SECONDS,
            &machine(Vec::new()),
            &Nobody,
            &places,
            SystemTime::now(),
        );
        assert_eq!(r.paths[0].verdict, LivenessVerdict::Reclaim);
    }

    #[test]
    fn the_readings_are_parsed_as_the_tools_print_them() {
        let ps = parse_ps(
            "  12   1 501 /bin/zsh -l\n  13 12 0 cargo  build --target-dir //x//y/\nbad line\n",
        );
        assert_eq!(ps.len(), 2);
        assert_eq!((ps[1].pid, ps[1].ppid, ps[1].uid), (13, 12, 0));
        assert_eq!(ps[1].args, "cargo build --target-dir /x/y");
        let cwds = parse_lsof("p12\nfcwd\nn/a//b\np13\nfcwd\nn/c\n");
        assert_eq!(cwds.get(&12).map(String::as_str), Some("/a/b"));
        assert_eq!(cwds.len(), 2);
        assert_eq!(
            nul_strings(b"A=1\0\0CARGO_TARGET_DIR=/t\0"),
            vec!["A=1".to_string(), "CARGO_TARGET_DIR=/t".to_string()]
        );
        assert_eq!(
            variables(
                [
                    "PATH=/bin",
                    "CARGO_BUILD_BUILD_DIR=/b",
                    "HOME=/h",
                    "MAJORDOMUS_BIN=/m"
                ]
                .into_iter()
            ),
            vec![
                ("CARGO_BUILD_BUILD_DIR".to_string(), "/b".to_string()),
                ("HOME".to_string(), "/h".to_string()),
                ("MAJORDOMUS_BIN".to_string(), "/m".to_string()),
            ]
        );
        // KERN_PROCARGS2: argc, the executable, padding, the arguments, the environment
        let mut buf = 2i32.to_ne_bytes().to_vec();
        buf.extend_from_slice(b"/bin/sleep\0\0\0sleep\x0030\0CARGO_TARGET_DIR=/t\0HOME=/h\0\0junk");
        assert_eq!(
            procargs_environment(&buf),
            Some(vec![
                "CARGO_TARGET_DIR=/t".to_string(),
                "HOME=/h".to_string()
            ])
        );
        assert_eq!(procargs_environment(b"\x01"), None);
        assert_eq!(
            derive_owner("p2:feature/x 123"),
            (Some("feature/x".into()), Some(123))
        );
        assert_eq!(
            derive_owner("me:main pid=7 at 10:00"),
            (Some("main".into()), Some(7))
        );
        assert_eq!(derive_owner("by hand"), (None, None));
        assert_eq!(
            issue_of_branch("feature/I4346-every-x"),
            Some("I4346".into())
        );
        assert_eq!(issue_of_branch("I12"), Some("I12".into()));
        assert_eq!(issue_of_branch("feature/Ix-y"), None);
        assert_eq!(collapse("/a//b/"), "/a/b");
        assert_eq!(collapse("/"), "/");
        assert!(meets("/a/b", "/a") && meets("/a", "/a/b") && !meets("/a/bc", "/a/b"));
        assert_eq!(command_word("/usr/bin/env FOO=1"), "env");
        assert_eq!(
            named(&["a".into(), "b".into(), "c".into(), "d".into()]),
            "a, b, c and 1 more"
        );
        assert_eq!(idle_seconds(None), Ok(DEFAULT_IDLE_SECONDS));
        assert_eq!(idle_seconds(Some("20m")), Ok(1200));
        assert!(idle_seconds(Some("soon")).is_err());
        assert_eq!(
            absolute(&["x".into(), "/y".into()], Path::new("/b")),
            vec!["/b/x".to_string(), "/y".to_string()]
        );
        let tmp = spellings(Path::new("/tmp/nowhere-at-all"));
        assert!(
            tmp.contains(&"/private/tmp/nowhere-at-all".to_string()),
            "{tmp:?}"
        );
        let private = spellings(Path::new("/private/var/x-nowhere"));
        assert!(
            private.contains(&"/var/x-nowhere".to_string()),
            "{private:?}"
        );
        // this process's own user has a home
        // SAFETY: geteuid has no preconditions.
        assert!(home_of_uid(unsafe { libc::geteuid() }).is_some());
        assert_eq!(LivenessVerdict::Keep.as_str(), "keep");
        for s in [
            LivenessSignal::Task,
            LivenessSignal::MeshClaim,
            LivenessSignal::BoardClaim,
            LivenessSignal::DeriveLock,
            LivenessSignal::QueuePidFile,
            LivenessSignal::RunnerJob,
            LivenessSignal::ProcessEnvironment,
            LivenessSignal::ProcessDirectory,
            LivenessSignal::ProcessArguments,
            LivenessSignal::GitActivity,
            LivenessSignal::Unreadable,
        ] {
            assert_eq!(
                serde_json::to_value(s).unwrap(),
                Value::String(s.as_str().into())
            );
        }
    }
}
