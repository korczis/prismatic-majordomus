//! The live runner: a task, run by a real coding harness, once without Majordomus and once
//! with it, from the same fixture, with the same model, judged by the same hidden tests.
//!
//! For each run the runner:
//!
//! 1. takes the provenance the record will carry — the repository's revision, the digest of
//!    the suite's freshness inputs, the harness's version — before anything else happens;
//! 2. copies the task's fixture into a fresh workspace, applies the task's setup patch, and
//!    commits it — the control's starting state. The workspace is `repo/` inside a new
//!    directory of the system temporary directory named by a digest: nothing in its path
//!    says which task, which arm or which benchmark it belongs to, and nothing beside it
//!    holds a hidden test, a transcript or another run's workspace;
//! 3. for the treatment only, installs Majordomus the way a user does (`init`, `update`,
//!    `capture install --provider claude-code`) and commits that too;
//! 4. runs every session of the task in the harness (Claude Code, headless, `stream-json`),
//!    with the user's own settings, MCP servers and skills shut out for both arms alike, each
//!    session under a wall-clock limit;
//! 5. runs the task's verify command, copies the hidden acceptance tests in, runs it again,
//!    and checks no test file the fixture started with was deleted;
//! 6. refuses the run if the freshness inputs or the harness changed while it was in flight,
//!    deletes the workspace, and writes one [`EconomicsRun`] with the provider's usage as
//!    reported, and nothing a session said. A record, once written, is never written over.
//!
//! The harness is a parameter: a test passes a replaying stand-in and exercises the whole
//! pipeline — workspace, sessions, gates, record — without a provider, a credential or a cent.
//! The transcripts stay in the work directory, `<work dir>/<suite>/<run id>/`, outside the
//! repository and outside every workspace.
//!
//! ```
//! use majordomus_cli::economics::runner::run_id;
//! assert_eq!(run_id("vat-rounding", "baseline", 2), "vat-rounding--baseline--r2");
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::model::*;
use super::{inputs_digest, usage, Declarations, DIR, RUN_SCHEMA};
use crate::metadata::yaml;
use crate::policy::{Policy, ProjectionMode};

/// How long a task's verify command may run. A test suite that hangs must end its run as a
/// failed check, not hold the whole suite.
const VERIFY_LIMIT: Duration = Duration::from_secs(600);

/// Why a recorded run is never replaced, said wherever replacing one is asked for.
const NEVER_OVERWRITTEN: &str = "a recorded run is evidence and is never overwritten: exclude \
     it in the methodology with a reason and record another repetition";

/// The id of a run: task, variant, repetition. One file per id; a recorded run is never
/// overwritten.
///
/// ```
/// use majordomus_cli::economics::runner::run_id;
/// let id = run_id("storage-v2", "majordomus", 1);
/// assert_eq!(id, "storage-v2--majordomus--r1");
/// // the two arms of one task and repetition differ in the variant alone
/// assert_ne!(run_id("storage-v2", "baseline", 1), id);
/// ```
pub fn run_id(task: &str, variant: &str, repetition: u32) -> String {
    format!("{task}--{variant}--r{repetition}")
}

/// Everything a live suite run is told. The work directory must lie outside `root`, and
/// [`run`] refuses the options otherwise, because the transcripts it collects there quote
/// prompts and code that never belong in the repository's history.
///
/// ```
/// # use majordomus_cli::economics::{model::*, Declarations};
/// # let methodology: EconomicsMethodology = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-methodology/v1", "version": 1, "title": "t", "question": "q",
/// #     "unit": "tokens", "primary_metric": "m", "classes": [], "variants": [], "success": [],
/// #     "pairing": { "key": [], "comparable": [], "valid": "v" },
/// #     "statistics": { "per_pair": "p", "location": "median", "interval": "i",
/// #         "confidence_bp": 9500, "resamples": 1, "seed": 1, "min_pairs_for_interval": 1 },
/// #     "publication": { "min_valid_pairs": 1, "min_categories": 1,
/// #         "min_pairs_per_category": 1, "min_repetitions": 1, "min_valid_pair_rate_bp": 1,
/// #         "max_interval_width_bp": 1, "require_current": true },
/// #     "outliers": { "rule": "r" } })).unwrap();
/// # let decl =
/// #     Declarations { methodology, suites: Default::default(), tasks: Default::default() };
/// use std::time::Duration;
/// use majordomus_cli::economics::runner::{run, RunOptions};
/// let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
/// let mut opts = RunOptions {
///     root: repo.path().into(), suite: "nightly".into(), tasks: vec![], repetitions: vec![1],
///     work_dir: work.path().into(), harness: "claude".into(), majordomus: "majordomus".into(),
///     parallel: 2, force: false, dry_run: true, session_timeout: Duration::from_secs(900),
/// };
/// // the suite is looked up in the declarations, never invented from the options
/// assert_eq!(run(&opts, &decl, &|_| {}).unwrap_err(), "no suite \"nightly\"");
/// // and a recorded run is never written over, whatever the options say
/// opts.force = true;
/// assert!(run(&opts, &decl, &|_| {}).unwrap_err().contains("never overwritten"));
/// ```
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// The repository whose declarations and share are used.
    pub root: PathBuf,
    /// The suite.
    pub suite: String,
    /// Only these tasks; all of the suite's when empty.
    pub tasks: Vec<String>,
    /// Only these repetitions; all of the suite's when empty.
    pub repetitions: Vec<u32>,
    /// Where transcripts go. Never inside the repository. The workspaces are not here: each
    /// is a fresh directory of the system temporary directory, removed when its run ends.
    pub work_dir: PathBuf,
    /// The harness executable (`claude`, or a stand-in).
    pub harness: PathBuf,
    /// The `majordomus` executable the treatment installs from.
    pub majordomus: PathBuf,
    /// Runs in flight at once.
    pub parallel: usize,
    /// Asks to record over an existing run, and is always refused by [`run`]: a recorded
    /// run is evidence. A run that should not count is excluded in the methodology, with its
    /// reason, and another repetition is recorded instead.
    pub force: bool,
    /// Prepare the workspaces and print the commands, run nothing.
    pub dry_run: bool,
    /// Per-session wall-clock limit.
    pub session_timeout: Duration,
}

/// One unit of work.
#[derive(Debug, Clone)]
struct Job {
    task: EconomicsTask,
    variant: String,
    repetition: u32,
}

/// A digest of a fixture tree and of further declaration files, each named by its path
/// relative to the repository, so that the same fixture digests the same in every checkout.
fn sha256_file_tree(repo: &Path, root: &Path, extra: &[&Path]) -> String {
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    let mut h = Sha256::new();
    for rel in &files {
        h.update(rel.as_bytes());
        h.update([0]);
        h.update(std::fs::read(root.join(rel)).unwrap_or_default());
        h.update([0]);
    }
    for p in extra {
        h.update(
            p.strip_prefix(repo)
                .unwrap_or(p)
                .to_string_lossy()
                .as_bytes(),
        );
        h.update([0]);
        h.update(std::fs::read(p).unwrap_or_default());
        h.update([0]);
    }
    format!("{:x}", h.finalize())
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if name == "__pycache__" || name == ".git" || name.ends_with(".pyc") {
            continue;
        }
        if p.is_dir() {
            walk(base, &p, out);
        } else if let Ok(rel) = p.strip_prefix(base) {
            out.push(rel.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn copy_fixture(from: &Path, to: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    walk(from, from, &mut files);
    for rel in files {
        let dst = to.join(&rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::copy(from.join(&rel), &dst).map_err(|e| format!("{rel}: {e}"))?;
    }
    Ok(())
}

/// The variables that tie a process to the session or the repository it was started from.
/// A child that inherited one would report into the operator's own session (`CLAUDECODE`,
/// the session id, the IDE's port), resolve the operator's project (`CLAUDE_PROJECT_DIR`),
/// find a build of the operator's (`CARGO_TARGET_DIR`), or run git against the operator's
/// repository instead of its workspace (`GIT_DIR` and its kin). Credentials and provider
/// routing (`ANTHROPIC_*`, `CLAUDE_CODE_USE_BEDROCK`, ...) are not among them: both arms
/// need them, and alike.
const TIED: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_PROJECT_DIR",
    "CARGO_TARGET_DIR",
    "CARGO_BUILD_TARGET_DIR",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_PREFIX",
];

/// Prefixes of the same: the operator's Majordomus state, read by the shell tool and the
/// executable alike.
const TIED_PREFIXES: &[&str] = &["MAJORDOMUS_", "MJ_"];

fn tied(name: &str) -> bool {
    TIED.contains(&name) || TIED_PREFIXES.iter().any(|p| name.starts_with(p))
}

/// The environment a harness and everything it spawns runs in: the caller's, minus every
/// variable that ties the child to the caller's session or repository. `bin`, when given,
/// is the `majordomus` executable whose directory goes first on `PATH`, which is how the
/// treatment's hooks find the tool they were installed from; the control is given none.
fn clean_env(cmd: &mut Command, bin: Option<&Path>) {
    for (k, _) in std::env::vars_os() {
        if k.to_str().is_some_and(tied) {
            cmd.env_remove(&k);
        }
    }
    if let Some(dir) = bin
        .and_then(Path::parent)
        .filter(|d| !d.as_os_str().is_empty())
    {
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{path}", dir.display()));
    }
    cmd.env("PYTHONDONTWRITEBYTECODE", "1");
    // The harness updates itself in the background unless told not to, and a version that
    // changes between two runs of one suite makes them incomparable.
    cmd.env("DISABLE_AUTOUPDATER", "1");
}

fn run_quiet(
    dir: &Path,
    program: &Path,
    args: &[&str],
    bin: Option<&Path>,
    share: Option<&Path>,
) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(dir).stdin(Stdio::null());
    clean_env(&mut cmd, bin);
    if let Some(s) = share {
        cmd.env("MAJORDOMUS_SHARE", s);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("{}: {e}", program.display()))?;
    if !out.status.success() {
        return Err(format!(
            "{} {} exited {}: {}",
            program.display(),
            args.join(" "),
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr)
                .lines()
                .last()
                .unwrap_or("")
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    run_quiet(dir, Path::new("git"), args, None, None)
}

/// The process groups the runner started and has not reaped yet, killed with the runner
/// when it dies of `SIGINT`, `SIGTERM` or `SIGHUP`. Every harness session and every
/// verification runs as the leader of a group of its own, so that a timeout can end it and
/// everything it started; the price is that a Ctrl-C at the terminal no longer reaches them,
/// and this is what pays it back. The handler does only what is safe inside a signal
/// handler: `killpg` each recorded group, restore the default disposition, and raise the
/// signal again so that the exit status still says which signal it was.
#[cfg(unix)]
mod groups {
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Once;

    const SLOTS: usize = 64;
    static LIVE: [AtomicI32; SLOTS] = [const { AtomicI32::new(0) }; SLOTS];
    static INSTALL: Once = Once::new();

    /// A recorded group; dropping it forgets the group.
    pub struct Registered(Option<usize>);

    /// Record `pgid` until the returned value is dropped. More groups at once than there
    /// are slots is not an error: the surplus is simply not killed on a signal.
    pub fn register(pgid: u32) -> Registered {
        INSTALL.call_once(|| {
            let handler = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
            for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
                // SAFETY: installing a handler that only calls async-signal-safe functions
                unsafe { libc::signal(signal, handler) };
            }
        });
        let Ok(pgid) = i32::try_from(pgid) else {
            return Registered(None);
        };
        for (i, slot) in LIVE.iter().enumerate() {
            if slot
                .compare_exchange(0, pgid, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                return Registered(Some(i));
            }
        }
        Registered(None)
    }

    impl Drop for Registered {
        fn drop(&mut self) {
            if let Some(i) = self.0 {
                LIVE[i].store(0, Ordering::SeqCst);
            }
        }
    }

    extern "C" fn on_signal(signal: libc::c_int) {
        for slot in &LIVE {
            let pgid = slot.load(Ordering::SeqCst);
            if pgid > 0 {
                // SAFETY: killpg is async-signal-safe; the group is one this process started
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
            }
        }
        // SAFETY: restoring the default disposition and re-raising are async-signal-safe
        unsafe {
            libc::signal(signal, libc::SIG_DFL);
            libc::raise(signal);
        }
    }
}

#[cfg(not(unix))]
mod groups {
    pub struct Registered;

    pub fn register(_: u32) -> Registered {
        Registered
    }
}

/// Start `cmd` as the leader of a process group of its own, recorded for [`groups`].
fn spawn_group(cmd: &mut Command) -> std::io::Result<(Child, groups::Registered)> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let child = cmd.spawn()?;
    let registered = groups::register(child.id());
    Ok((child, registered))
}

/// Wait for `child`, started by [`spawn_group`], at most `limit`. On expiry its whole
/// process group — the child and everything it started — is killed and the child reaped,
/// and `None` says it never finished.
fn wait_bounded(child: &mut Child, limit: Duration) -> Result<Option<ExitStatus>, String> {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(Some(status));
        }
        if started.elapsed() >= limit {
            #[cfg(unix)]
            // SAFETY: the child leads its own group (`spawn_group`) and is not reaped yet,
            // so the group id is still its own and names no other process's group.
            unsafe {
                libc::killpg(child.id() as libc::pid_t, libc::SIGKILL);
            }
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The directory one run works in: a fresh directory of the system temporary directory,
/// named `mj-bench-` and twelve hex digits of a digest of the run id, the process and the
/// time, so that its path says nothing about the task, the arm or the benchmark. It holds
/// the repository the agent works in (`repo/`) and the harness's empty MCP configuration,
/// and nothing else; it is removed with everything in it when the value is dropped.
struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn create(run: &str) -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let digest = crate::policy::sha256_hex(&format!("{run}\n{}\n{nanos}", std::process::id()));
        let dir = std::env::temp_dir().join(format!("mj-bench-{}", &digest[..12]));
        // `create_dir`, not `create_dir_all`: a directory that already exists is not fresh
        std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        Ok(Self { dir })
    }

    fn repo(&self) -> PathBuf {
        self.dir.join("repo")
    }

    fn mcp_config(&self) -> PathBuf {
        self.dir.join("mcp.json")
    }

    /// Remove it now, and say so if that failed; the drop that follows is then a no-op.
    fn remove(self) -> Result<(), String> {
        std::fs::remove_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The treatment's policy, changed in the two ways the benchmark needs, or why it could not
/// be:
///
/// - every projection whose target the fixture already has (`region`) is switched to region
///   mode, so that `update` adds the bootstrap to the team's own file as a generated region
///   instead of refusing to replace it;
/// - `session.ensure_server_on_start` is set to `false`. The methodology leaves the MCP
///   server out of the treatment, and with it on, what the start hook tells the agent would
///   depend on the operator's machine: it starts a server when a build of the executable is
///   at hand and tells the agent to build one when it is not.
///
/// The result is read back with the crate's own policy reader, and a text in which either
/// change did not take is refused: a treatment that silently kept the skeleton's defaults
/// would be measured as a different treatment.
fn benchmark_policy(text: &str, region: &[&str]) -> Result<String, String> {
    let mut text = text.to_string();
    for target in region {
        let line = format!("    target: {target}\n");
        if !text.contains(&line) {
            return Err(format!(
                "the policy declares no projection with target {target}"
            ));
        }
        text = text.replace(&line, &format!("{line}    mode: region\n"));
    }
    let setting = "ensure_server_on_start: false";
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let opens_session = |l: &String| {
        l.strip_prefix("session:").is_some_and(|rest| {
            let rest = rest.trim();
            rest.is_empty() || rest.starts_with('#')
        })
    };
    if lines
        .iter()
        .any(|l| l.starts_with("session:") && !opens_session(l))
    {
        return Err("the policy's session block is not a block mapping".into());
    }
    match lines.iter().position(opens_session) {
        None => {
            lines.push("session:".into());
            lines.push(format!("  {setting}"));
        }
        Some(start) => {
            // the block runs to the next key at the top level
            let body = start + 1;
            let end = lines[body..]
                .iter()
                .position(|l| {
                    !l.is_empty() && !l.starts_with(char::is_whitespace) && !l.starts_with('#')
                })
                .map_or(lines.len(), |i| body + i);
            let indent_of = |l: &String| l.len() - l.trim_start().len();
            let indent = lines[body..end]
                .iter()
                .find(|l| {
                    let t = l.trim_start();
                    !t.is_empty() && !t.starts_with('#')
                })
                .map_or(2, indent_of);
            let line = format!("{}{setting}", " ".repeat(indent));
            let key = lines[body..end].iter().position(|l| {
                indent_of(l) == indent && l.trim_start().starts_with("ensure_server_on_start:")
            });
            match key {
                Some(i) => lines[body + i] = line,
                None => lines.insert(body, line),
            }
        }
    }
    let mut out = lines.join("\n");
    out.push('\n');
    let policy: Policy = yaml::parse_into(&out)
        .map_err(|e| format!("the benchmark's policy does not read back: {e}"))?;
    if policy.session.ensure_server_on_start {
        return Err("session.ensure_server_on_start did not read back as false".into());
    }
    for target in region {
        let regional = policy
            .projections
            .iter()
            .any(|p| p.target == *target && p.mode == ProjectionMode::Region);
        if !regional {
            return Err(format!(
                "the projection of {target} did not read back in region mode"
            ));
        }
    }
    Ok(out)
}

/// The identity every workspace's commits carry. An agent reads `git log` as readily as the
/// tree, so the history says no more than the path does: nothing in it names a benchmark, a
/// task or an arm.
const WORKSPACE_AUTHOR: (&str, &str) = ("Developer", "developer@example.invalid");
/// The message of a workspace's first commit, which holds the fixture with the task's setup
/// patch folded in, so that the setup is not shown to the agent as a change of its own.
const WORKSPACE_FIRST_COMMIT: &str = "Initial commit";

/// Build a task's starting state in `ws`: fixture, setup patch, a commit; then, for the
/// treatment, Majordomus installed as a user installs it, and a second commit.
fn prepare(opts: &RunOptions, job: &Job, treatment: bool, ws: &Path) -> Result<String, String> {
    if ws.exists() {
        std::fs::remove_dir_all(ws).map_err(|e| format!("{}: {e}", ws.display()))?;
    }
    std::fs::create_dir_all(ws).map_err(|e| format!("{}: {e}", ws.display()))?;
    copy_fixture(&opts.root.join(&job.task.fixture), ws)?;
    git(ws, &["init", "-q", "-b", "main"])?;
    git(ws, &["config", "user.name", WORKSPACE_AUTHOR.0])?;
    git(ws, &["config", "user.email", WORKSPACE_AUTHOR.1])?;
    git(ws, &["config", "commit.gpgsign", "false"])?;
    if let Some(setup) = &job.task.setup {
        git(ws, &["apply", &opts.root.join(setup).to_string_lossy()])?;
    }
    git(ws, &["add", "-A"])?;
    git(ws, &["commit", "-qm", WORKSPACE_FIRST_COMMIT])?;
    if treatment {
        let share = opts.root.join("share");
        let bin = Some(opts.majordomus.as_path());
        run_quiet(ws, &opts.majordomus, &["init"], bin, Some(&share))?;
        let policy = ws.join(".ai/repo/policy.yaml");
        let read =
            |p: &Path| std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()));
        let kept: Vec<&str> = ["CLAUDE.md", "AGENTS.md", "GEMINI.md"]
            .into_iter()
            .filter(|t| ws.join(t).exists())
            .collect();
        let text = benchmark_policy(&read(&policy)?, &kept)?;
        std::fs::write(&policy, text).map_err(|e| format!("{}: {e}", policy.display()))?;
        for args in [
            &["update"][..],
            &["capture", "install", "--provider", "claude-code"][..],
        ] {
            run_quiet(ws, &opts.majordomus, args, bin, Some(&share))?;
        }
        // what the installation left is what the sessions will read
        let installed: Policy = yaml::parse_into(&read(&policy)?)?;
        if installed.session.ensure_server_on_start {
            return Err("the installation turned session.ensure_server_on_start back on".into());
        }
        git(ws, &["add", "-A"])?;
        git(ws, &["commit", "-qm", "Install Majordomus"])?;
    }
    Ok(git(ws, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// The harness invocation, prompt excepted: what the configuration digest covers.
fn harness_args(model: &str, budget: Option<u32>, empty_mcp: &Path) -> Vec<String> {
    let mut a: Vec<String> = [
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        model,
        "--setting-sources",
        "project,local",
        "--strict-mcp-config",
        "--mcp-config",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(empty_mcp.to_string_lossy().to_string());
    for s in [
        "--disable-slash-commands",
        "--no-session-persistence",
        "--permission-mode",
        "bypassPermissions",
    ] {
        a.push(s.into());
    }
    if let Some(b) = budget {
        a.push("--max-budget-usd".into());
        a.push(b.to_string());
    }
    a
}

/// The digest of a harness invocation, the path of the run's own empty MCP configuration
/// masked: two runs invoked alike digest alike, whichever workspace each ran in. The arm is
/// not part of it — the treatment differs in what its repository holds, never in how the
/// harness is called — so a control and a treatment of one pair must digest the same.
fn configuration_digest(args: &[String]) -> String {
    let masked: Vec<&str> = args
        .iter()
        .map(|a| {
            if a.ends_with("mcp.json") {
                "<empty-mcp>"
            } else {
                a.as_str()
            }
        })
        .collect();
    crate::policy::sha256_hex(&masked.join(" "))
}

fn harness_version(harness: &Path) -> String {
    let mut cmd = Command::new(harness);
    cmd.arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    clean_env(&mut cmd, None);
    cmd.output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .split_whitespace()
                .next()
                .unwrap_or("unknown")
                .to_string()
        })
        .unwrap_or_else(|| "unknown".into())
}

/// Run one session and read its transcript. A session the timeout ended is recorded as
/// having ended `timeout`, whatever its transcript got as far as saying.
fn session(
    opts: &RunOptions,
    ws: &Path,
    transcript: &Path,
    args: &[String],
    prompt: &str,
    index: u32,
    bin: Option<&Path>,
) -> Result<EconomicsSession, String> {
    let file =
        std::fs::File::create(transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
    let mut cmd = Command::new(&opts.harness);
    cmd.arg("-p")
        .arg(prompt)
        .args(args)
        .current_dir(ws)
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::null());
    clean_env(&mut cmd, bin);
    let started = Instant::now();
    let (mut child, _group) =
        spawn_group(&mut cmd).map_err(|e| format!("{}: {e}", opts.harness.display()))?;
    let timed_out = wait_bounded(&mut child, opts.session_timeout)?.is_none();
    let duration_ms = started.elapsed().as_millis() as u64;
    let text = std::fs::read_to_string(transcript).unwrap_or_default();
    let facts = usage::read_stream(&text).unwrap_or_default();
    let (ended, is_error) = if timed_out {
        ("timeout".to_string(), true)
    } else if facts.ended.is_empty() {
        ("no_result".to_string(), true)
    } else {
        (facts.ended, facts.is_error || facts.requests.is_empty())
    };
    Ok(EconomicsSession {
        index,
        prompt_sha256: crate::policy::sha256_hex(prompt),
        duration_ms,
        turns: facts.turns,
        ended,
        is_error,
        requests: facts.requests,
        models: facts.models,
        reported_cost_microusd: facts.reported_cost_microusd,
        tools: facts.tools,
        orientation: facts.orientation,
    })
}

/// The gate a run's sessions pass: every session ended with the harness's own success
/// result — a `result` event of subtype `success`, not flagged as an error, with usage
/// reported, and not ended by the timeout. A run whose last session was cut off can still
/// leave a tree that passes the tests; it is still not a completed run of the task.
fn sessions_check(sessions: &[EconomicsSession]) -> EconomicsCheck {
    let unfinished: Vec<String> = sessions
        .iter()
        .filter(|s| s.ended != "success" || s.is_error)
        .map(|s| {
            format!(
                "session {} ended {}{}",
                s.index,
                s.ended,
                if s.is_error { " (error)" } else { "" }
            )
        })
        .collect();
    let passed = !sessions.is_empty() && unfinished.is_empty();
    EconomicsCheck {
        id: "sessions_completed".into(),
        passed,
        detail: if sessions.is_empty() {
            "no session ran".into()
        } else if passed {
            format!(
                "all {} session(s) ended with the harness's success result",
                sessions.len()
            )
        } else {
            unfinished.join("; ")
        },
    }
}

/// Run a task's verify command in `ws` for at most `limit`. On expiry the command's whole
/// process group is killed and the check fails as having timed out. Nothing Majordomus
/// installed is on its `PATH`: both arms are verified alike.
fn verify(ws: &Path, command: &str, limit: Duration) -> (bool, String) {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command])
        .current_dir(ws)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    clean_env(&mut cmd, None);
    let (mut child, _group) = match spawn_group(&mut cmd) {
        Ok(c) => c,
        Err(e) => return (false, format!("could not run: {e}")),
    };
    // Drained on a thread of its own, so that a suite writing more than a pipe holds is not
    // blocked on it while it is being waited for.
    let mut stderr = child.stderr.take();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = Vec::new();
        if let Some(e) = stderr.as_mut() {
            let _ = e.read_to_end(&mut text);
        }
        let _ = tx.send(text);
    });
    match wait_bounded(&mut child, limit) {
        Err(e) => (false, format!("could not wait: {e}")),
        Ok(None) => (false, format!("timed out after {}s", limit.as_secs())),
        Ok(Some(status)) => {
            let text = rx.recv_timeout(Duration::from_secs(5)).unwrap_or_default();
            let tail = String::from_utf8_lossy(&text)
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .to_string();
            (
                status.success(),
                format!("exit {}: {tail}", status.code().unwrap_or(-1)),
            )
        }
    }
}

/// Files changed since the starting state `base`, committed or not. What the treatment's
/// own records write under `.ai/local/` is the tool's state, not the task's work.
fn changed_files(ws: &Path, base: &str) -> Vec<String> {
    let _ = git(ws, &["add", "-A"]);
    let out = git(ws, &["diff", "--cached", "--name-only", base]).unwrap_or_default();
    let mut files: Vec<String> = out
        .lines()
        .filter(|l| !l.starts_with(".ai/local/") && !l.contains("__pycache__"))
        .map(str::to_string)
        .collect();
    files.sort();
    files
}

/// Run one job end to end and return its record. Never panics on a failing session: a
/// session that errs is recorded as having erred, and the gates decide the outcome. The
/// workspace is gone when this returns, whatever it returns.
fn run_job(
    opts: &RunOptions,
    decl: &Declarations,
    suite: &EconomicsSuite,
    job: &Job,
    log: &dyn Fn(String),
) -> Result<EconomicsRun, String> {
    let id = run_id(&job.task.id, &job.variant, job.repetition);
    let treatment = Some(&job.variant) == suite.treatment.as_ref();
    let model = suite.model.clone().ok_or("the suite names no model")?;
    // The provenance, before anything is prepared: what the run is recorded against is what
    // it started from, and the end of the run is checked against it.
    let repository = super::revision(&opts.root)?;
    let inputs_at_start = inputs_digest(&opts.root, &suite.freshness_inputs)?;
    let harness_at_start = harness_version(&opts.harness);
    let task_dir = opts.root.join(DIR).join("tasks").join(&job.task.id);
    let mut extra: Vec<PathBuf> = vec![task_dir.join("task.yaml")];
    extra.extend(job.task.sessions.iter().map(|s| task_dir.join(&s.prompt)));
    if let Some(s) = &job.task.setup {
        extra.push(opts.root.join(s));
    }
    let extra_refs: Vec<&Path> = extra.iter().map(PathBuf::as_path).collect();
    let fixture_digest =
        sha256_file_tree(&opts.root, &opts.root.join(&job.task.fixture), &extra_refs);

    let workspace = Workspace::create(&id)?;
    let ws = workspace.repo();
    let base_commit = prepare(opts, job, treatment, &ws)?;
    let fixture_tests: BTreeSet<String> = {
        let mut f = Vec::new();
        walk(&ws.join("tests"), &ws.join("tests"), &mut f);
        f.into_iter().collect()
    };
    let empty_mcp = workspace.mcp_config();
    std::fs::write(&empty_mcp, "{\"mcpServers\":{}}\n").map_err(|e| e.to_string())?;
    let args = harness_args(&model, suite.max_budget_usd_per_session, &empty_mcp);
    let configuration_digest = configuration_digest(&args);
    if opts.dry_run {
        let short = &base_commit[..base_commit.len().min(12)];
        log(format!(
            "{id}: prepared a workspace at {short} ({} session(s)) and removed it again; \
             would run: {} -p <prompt> {}",
            job.task.sessions.len(),
            opts.harness.display(),
            args.join(" ")
        ));
        workspace.remove()?;
        return Err("dry run".into());
    }
    let transcripts = opts.work_dir.join(&suite.id).join(&id);
    std::fs::create_dir_all(&transcripts).map_err(|e| format!("{}: {e}", transcripts.display()))?;
    let bin = treatment.then_some(opts.majordomus.as_path());
    let started_at = crate::peers::rfc3339(SystemTime::now());
    let mut sessions = Vec::new();
    for (i, s) in job.task.sessions.iter().enumerate() {
        let prompt = std::fs::read_to_string(task_dir.join(&s.prompt))
            .map_err(|e| format!("{}: {e}", s.prompt))?;
        let transcript = transcripts.join(format!("session-{}.jsonl", i + 1));
        log(format!(
            "{id}: session {} of {} started",
            i + 1,
            job.task.sessions.len()
        ));
        let sess = session(
            opts,
            &ws,
            &transcript,
            &args,
            prompt.trim_end(),
            (i + 1) as u32,
            bin,
        )?;
        log(format!(
            "{id}: session {} ended {} after {}s, {} request(s)",
            i + 1,
            sess.ended,
            sess.duration_ms / 1000,
            sess.requests.len()
        ));
        sessions.push(sess);
    }
    // what the sessions changed, taken before the hidden tests are copied in
    let changed = changed_files(&ws, &base_commit);
    let (visible, visible_detail) = verify(&ws, &job.task.verify, VERIFY_LIMIT);
    let acceptance_dir = opts.root.join(&job.task.acceptance);
    let mut accepted = Vec::new();
    walk(&acceptance_dir, &acceptance_dir, &mut accepted);
    for f in &accepted {
        let dst = ws.join("tests").join(f);
        let _ = std::fs::create_dir_all(dst.parent().unwrap_or(&ws));
        std::fs::copy(acceptance_dir.join(f), &dst).map_err(|e| format!("{f}: {e}"))?;
    }
    let (acceptance, acceptance_detail) = verify(&ws, &job.task.verify, VERIFY_LIMIT);
    let missing: Vec<&String> = fixture_tests
        .iter()
        .filter(|f| !ws.join("tests").join(f).exists())
        .collect();
    let checks = vec![
        sessions_check(&sessions),
        EconomicsCheck {
            id: "visible_tests".into(),
            passed: visible,
            detail: visible_detail,
        },
        EconomicsCheck {
            id: "acceptance_tests".into(),
            passed: acceptance,
            detail: acceptance_detail,
        },
        EconomicsCheck {
            id: "tests_kept".into(),
            passed: missing.is_empty(),
            detail: if missing.is_empty() {
                format!("all {} fixture test file(s) present", fixture_tests.len())
            } else {
                format!(
                    "deleted: {}",
                    missing
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            },
        },
    ];
    let finished_at = crate::peers::rfc3339(SystemTime::now());
    // A run is recorded against what it started from only if that is still what there is.
    let inputs_at_end = inputs_digest(&opts.root, &suite.freshness_inputs)?;
    if inputs_at_end != inputs_at_start {
        return Err(format!(
            "the suite's freshness inputs changed while the run was in flight (digest {} at \
             the start, {} at the end): the checkout was edited, and nothing was recorded",
            &inputs_at_start[..12.min(inputs_at_start.len())],
            &inputs_at_end[..12.min(inputs_at_end.len())]
        ));
    }
    let harness_at_end = harness_version(&opts.harness);
    if harness_at_end != harness_at_start {
        return Err(format!(
            "the harness changed while the run was in flight ({harness_at_start} at the start, \
             {harness_at_end} at the end): nothing was recorded"
        ));
    }
    if let Err(e) = workspace.remove() {
        log(format!(
            "{id}: WARN the workspace could not be removed: {e}"
        ));
    }
    let models_reported: Vec<String> = sessions
        .iter()
        .flat_map(|s| {
            s.models
                .iter()
                .map(|m| m.model.clone())
                .chain(s.requests.iter().map(|r| r.model.clone()))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(EconomicsRun {
        schema: RUN_SCHEMA.into(),
        id,
        suite: suite.id.clone(),
        suite_version: suite.version,
        methodology: decl.methodology.version,
        task: job.task.id.clone(),
        variant: job.variant.clone(),
        repetition: job.repetition,
        provider: suite.provider.clone().unwrap_or_default(),
        harness: EconomicsHarness {
            name: suite.harness.clone().unwrap_or_default(),
            version: harness_at_start,
        },
        model_requested: model,
        models_reported,
        repository,
        majordomus_version: crate::VERSION.into(),
        fixture_digest,
        inputs_digest: inputs_at_start,
        configuration_digest,
        started_at,
        finished_at,
        outcome: EconomicsOutcome {
            completed: checks.iter().all(|c| c.passed),
            checks,
            changed_files: changed,
        },
        sessions,
    })
}

/// Where a run's record is written: one JSON file per run id under the suite's directory of
/// the evidence tree. [`run`] skips a job whose record already exists here, which is what
/// makes an interrupted suite resumable, and never writes over one.
///
/// ```
/// use std::path::Path;
/// use majordomus_cli::economics::{runner::{record_path, run_id}, DIR};
/// let id = run_id("vat-rounding", "baseline", 2);
/// let path = record_path(Path::new("/repo"), "pilot", &id);
/// let expected = Path::new("/repo").join(DIR).join("runs/pilot/vat-rounding--baseline--r2.json");
/// assert_eq!(path, expected);
/// ```
pub fn record_path(root: &Path, suite: &str, id: &str) -> PathBuf {
    root.join(DIR)
        .join("runs")
        .join(suite)
        .join(format!("{id}.json"))
}

/// Write a record that does not exist yet. The file is created exclusively (`O_EXCL`), so a
/// record that appeared in the meantime is refused rather than replaced; a write that fails
/// half way removes the file it created, which held nothing but this run's partial text.
fn write_record(path: &Path, run: &EconomicsRun) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    let mut text = serde_json::to_string_pretty(run).map_err(|e| e.to_string())?;
    text.push('\n');
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                format!(
                    "{} is already recorded; {NEVER_OVERWRITTEN}",
                    path.display()
                )
            } else {
                format!("{}: {e}", path.display())
            }
        })?;
    if let Err(e) = f.write_all(text.as_bytes()).and_then(|()| f.sync_all()) {
        drop(f);
        let _ = std::fs::remove_file(path);
        return Err(format!("{}: {e}", path.display()));
    }
    Ok(())
}

/// `dir` made absolute, created, and canonical; refused when it lies inside `root`, taken
/// canonical too, so that neither a symbolic link nor a relative spelling can put it there.
/// A directory refused is taken back: whatever this created for the check is removed again,
/// deepest first, as long as it is still empty.
fn outside(root: &Path, dir: &Path, what: &str) -> Result<PathBuf, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", root.display()))?;
    let dir = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .join(dir)
    };
    let created: Vec<PathBuf> = dir
        .ancestors()
        .take_while(|p| !p.exists())
        .map(Path::to_path_buf)
        .collect();
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let canonical = dir
        .canonicalize()
        .map_err(|e| format!("{}: {e}", dir.display()))?;
    if canonical.starts_with(&root) {
        for p in &created {
            if std::fs::remove_dir(p).is_err() {
                break;
            }
        }
        return Err(format!(
            "{what} must be outside the repository: transcripts never enter it"
        ));
    }
    Ok(canonical)
}

/// Run a live suite: every job not yet recorded, `parallel` at a time, control and
/// treatment of one task and repetition side by side so that both meet the provider under
/// the same conditions. Returns the records written and the jobs that failed to produce
/// one. Refuses, before any workspace exists, `force` (a recorded run is never written
/// over), an unknown suite, a suite that is not `live`, a task the suite names but nobody
/// declared, and a work directory — or a system temporary directory — inside the
/// repository.
///
/// ```
/// # use majordomus_cli::economics::{model::*, Declarations};
/// # let methodology: EconomicsMethodology = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-methodology/v1", "version": 1, "title": "t", "question": "q",
/// #     "unit": "tokens", "primary_metric": "m", "classes": [], "variants": [], "success": [],
/// #     "pairing": { "key": [], "comparable": [], "valid": "v" },
/// #     "statistics": { "per_pair": "p", "location": "median", "interval": "i",
/// #         "confidence_bp": 9500, "resamples": 1, "seed": 1, "min_pairs_for_interval": 1 },
/// #     "publication": { "min_valid_pairs": 1, "min_categories": 1,
/// #         "min_pairs_per_category": 1, "min_repetitions": 1, "min_valid_pair_rate_bp": 1,
/// #         "max_interval_width_bp": 1, "require_current": true },
/// #     "outliers": { "rule": "r" } })).unwrap();
/// # let pilot: EconomicsSuite = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-suite/v1", "id": "pilot", "version": 1, "kind": "live",
/// #     "title": "Pilot", "model": "claude-test", "control": "baseline",
/// #     "treatment": "majordomus", "tasks": [], "freshness_inputs": [] })).unwrap();
/// # let decl = Declarations {
/// #     methodology, suites: [("pilot".to_string(), pilot)].into(), tasks: Default::default() };
/// use std::{sync::Mutex, time::Duration};
/// use majordomus_cli::economics::runner::{run, RunOptions};
/// let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
/// let mut opts = RunOptions {
///     root: repo.path().into(), suite: "pilot".into(), tasks: vec![], repetitions: vec![],
///     work_dir: work.path().into(), harness: "claude".into(), majordomus: "majordomus".into(),
///     parallel: 1, force: false, dry_run: false, session_timeout: Duration::from_secs(600),
/// };
/// // `pilot` is a live suite that names no task yet: nothing runs, nothing is written
/// let log = Mutex::new(Vec::new());
/// let (written, failed) = run(&opts, &decl, &|line| log.lock().unwrap().push(line)).unwrap();
/// assert!(written.is_empty() && failed.is_empty());
/// assert_eq!(log.into_inner().unwrap(), ["pilot: 0 run(s) to do, 1 at a time"]);
/// // transcripts never enter the repository, and the refusal leaves nothing behind
/// opts.work_dir = repo.path().join("economics-work");
/// assert!(run(&opts, &decl, &|_| {}).unwrap_err().contains("outside the repository"));
/// assert!(!opts.work_dir.exists());
/// ```
pub fn run(
    opts: &RunOptions,
    decl: &Declarations,
    log: &(dyn Fn(String) + Sync),
) -> Result<(Vec<PathBuf>, Vec<String>), String> {
    if opts.force {
        return Err(NEVER_OVERWRITTEN.into());
    }
    let suite = decl
        .suites
        .get(&opts.suite)
        .ok_or_else(|| format!("no suite {:?}", opts.suite))?
        .clone();
    if suite.kind != "live" {
        return Err(format!(
            "suite {} is a {} suite; `economics measure` runs it",
            suite.id, suite.kind
        ));
    }
    let work_dir = outside(&opts.root, &opts.work_dir, "the work directory")?;
    // the workspaces go to the system temporary directory, which must not be the repository's
    outside(
        &opts.root,
        &std::env::temp_dir(),
        "the system temporary directory (TMPDIR)",
    )?;
    let opts = &RunOptions {
        work_dir,
        ..opts.clone()
    };
    let variants: Vec<String> = [suite.control.clone(), suite.treatment.clone()]
        .into_iter()
        .flatten()
        .collect();
    let mut jobs = Vec::new();
    for rep in 1..=suite.repetitions.unwrap_or(1) {
        if !opts.repetitions.is_empty() && !opts.repetitions.contains(&rep) {
            continue;
        }
        for t in &suite.tasks {
            if !opts.tasks.is_empty() && !opts.tasks.contains(t) {
                continue;
            }
            let task = decl
                .tasks
                .get(t)
                .ok_or_else(|| format!("no task {t}"))?
                .clone();
            for v in &variants {
                if !opts.dry_run && record_path(&opts.root, &suite.id, &run_id(t, v, rep)).exists()
                {
                    continue;
                }
                jobs.push(Job {
                    task: task.clone(),
                    variant: v.clone(),
                    repetition: rep,
                });
            }
        }
    }
    log(format!(
        "{}: {} run(s) to do, {} at a time",
        suite.id,
        jobs.len(),
        opts.parallel.max(1)
    ));
    let queue = Arc::new(Mutex::new(
        jobs.into_iter().collect::<std::collections::VecDeque<_>>(),
    ));
    let (tx, rx) = mpsc::channel::<Result<PathBuf, String>>();
    std::thread::scope(|scope| {
        for _ in 0..opts.parallel.max(1) {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            let suite = &suite;
            scope.spawn(move || loop {
                let job = queue.lock().ok().and_then(|mut q| q.pop_front());
                let Some(job) = job else { break };
                let id = run_id(&job.task.id, &job.variant, job.repetition);
                let result = run_job(opts, decl, suite, &job, log).and_then(|run| {
                    let path = record_path(&opts.root, &suite.id, &run.id);
                    write_record(&path, &run)?;
                    log(format!(
                        "{id}: recorded, completed={}",
                        run.outcome.completed
                    ));
                    Ok(path)
                });
                let _ = tx.send(result.map_err(|e| format!("{id}: {e}")));
            });
        }
    });
    drop(tx);
    let mut written = Vec::new();
    let mut failed = Vec::new();
    for r in rx {
        match r {
            Ok(p) => written.push(p),
            Err(e) => failed.push(e),
        }
    }
    written.sort();
    failed.sort();
    Ok((written, failed))
}

/// The reference solution of every task applied to its fixture: the hidden tests must fail
/// on the starting state and pass on the reference, or a failing run would say nothing about
/// the agent. Returns one line per task, and `Err` listing every task that breaks the rule.
/// Each task is prepared under `<work_dir>/references/<task>` with the control's starting
/// state and removed once checked; no harness and no provider is involved, so the check
/// costs nothing to repeat.
///
/// ```
/// # use majordomus_cli::economics::{model::*, Declarations};
/// # let methodology: EconomicsMethodology = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-methodology/v1", "version": 1, "title": "t", "question": "q",
/// #     "unit": "tokens", "primary_metric": "m", "classes": [], "variants": [], "success": [],
/// #     "pairing": { "key": [], "comparable": [], "valid": "v" },
/// #     "statistics": { "per_pair": "p", "location": "median", "interval": "i",
/// #         "confidence_bp": 9500, "resamples": 1, "seed": 1, "min_pairs_for_interval": 1 },
/// #     "publication": { "min_valid_pairs": 1, "min_categories": 1,
/// #         "min_pairs_per_category": 1, "min_repetitions": 1, "min_valid_pair_rate_bp": 1,
/// #         "max_interval_width_bp": 1, "require_current": true },
/// #     "outliers": { "rule": "r" } })).unwrap();
/// # let solve: EconomicsTask = serde_json::from_value(serde_json::json!({
/// #     "schema": "economics-task/v1", "id": "solve", "title": "Solve", "category": "c",
/// #     "complexity": "small", "source": "s", "fixture": "fx", "acceptance": "hidden",
/// #     "reference": "ref.patch", "verify": "test -f solved", "scope": [], "sessions": [] }))
/// #     .unwrap();
/// # let decl = Declarations {
/// #     methodology, suites: Default::default(), tasks: [("solve".to_string(), solve)].into() };
/// use majordomus_cli::economics::runner::check_references;
/// let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
/// // task `solve` starts from `fx/`, verifies with `test -f solved`, and its reference
/// // solution `ref.patch` creates that file
/// std::fs::create_dir_all(repo.path().join("fx")).unwrap();
/// std::fs::write(repo.path().join("fx/app.py"), "x = 1\n").unwrap();
/// let patch = "--- /dev/null\n+++ b/solved\n@@ -0,0 +1 @@\n+yes\n";
/// std::fs::write(repo.path().join("ref.patch"), patch).unwrap();
/// let lines = check_references(repo.path(), &decl, work.path()).unwrap();
/// assert_eq!(lines, ["solve: fails on the starting state, passes on the reference"]);
/// // and the workspace, hidden tests and all, is gone
/// assert!(!work.path().join("references/solve").exists());
/// ```
pub fn check_references(
    root: &Path,
    decl: &Declarations,
    work_dir: &Path,
) -> Result<Vec<String>, Vec<String>> {
    let mut ok = Vec::new();
    let mut bad = Vec::new();
    let mut by: BTreeMap<&str, &EconomicsTask> = BTreeMap::new();
    for t in decl.tasks.values() {
        by.insert(&t.id, t);
    }
    for (id, task) in by {
        let ws = work_dir.join("references").join(id);
        let opts = RunOptions {
            root: root.to_path_buf(),
            suite: String::new(),
            tasks: Vec::new(),
            repetitions: Vec::new(),
            work_dir: work_dir.to_path_buf(),
            harness: PathBuf::new(),
            majordomus: PathBuf::new(),
            parallel: 1,
            force: false,
            dry_run: false,
            session_timeout: Duration::from_secs(1),
        };
        let job = Job {
            task: task.clone(),
            variant: String::new(),
            repetition: 0,
        };
        if let Err(e) = prepare(&opts, &job, false, &ws) {
            bad.push(format!("{id}: {e}"));
            let _ = std::fs::remove_dir_all(&ws);
            continue;
        }
        let acceptance_dir = root.join(&task.acceptance);
        let mut files = Vec::new();
        walk(&acceptance_dir, &acceptance_dir, &mut files);
        for f in &files {
            let _ = std::fs::copy(acceptance_dir.join(f), ws.join("tests").join(f));
        }
        let (before, _) = verify(&ws, &task.verify, VERIFY_LIMIT);
        let applied = git(
            &ws,
            &["apply", &root.join(&task.reference).to_string_lossy()],
        );
        let (after, detail) = verify(&ws, &task.verify, VERIFY_LIMIT);
        let _ = std::fs::remove_dir_all(&ws);
        match (before, applied, after) {
            (false, Ok(_), true) => ok.push(format!(
                "{id}: fails on the starting state, passes on the reference"
            )),
            (true, _, _) => bad.push(format!(
                "{id}: the hidden tests already pass on the starting state"
            )),
            (_, Err(e), _) => bad.push(format!("{id}: the reference does not apply: {e}")),
            (_, _, false) => bad.push(format!("{id}: the reference does not pass: {detail}")),
        }
    }
    if bad.is_empty() {
        Ok(ok)
    } else {
        Err(bad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn methodology() -> EconomicsMethodology {
        serde_json::from_value(json!({
            "schema": "economics-methodology/v1", "version": 1, "title": "t", "question": "q",
            "unit": "tokens", "primary_metric": "m", "classes": [], "variants": [],
            "success": [], "pairing": { "key": [], "comparable": [], "valid": "v" },
            "statistics": { "per_pair": "p", "location": "median", "interval": "i",
                "confidence_bp": 9500, "resamples": 1, "seed": 1, "min_pairs_for_interval": 1 },
            "publication": { "min_valid_pairs": 1, "min_categories": 1,
                "min_pairs_per_category": 1, "min_repetitions": 1, "min_valid_pair_rate_bp": 1,
                "max_interval_width_bp": 1, "require_current": true },
            "outliers": { "rule": "r" }
        }))
        .unwrap()
    }

    fn suite(kind: &str, tasks: &[&str], treatment: Option<&str>) -> EconomicsSuite {
        serde_json::from_value(json!({
            "schema": "economics-suite/v1", "id": "pilot", "version": 1, "kind": kind,
            "title": "Pilot", "model": "claude-test", "control": "baseline",
            "treatment": treatment, "tasks": tasks, "freshness_inputs": []
        }))
        .unwrap()
    }

    fn task(id: &str, verify: &str) -> EconomicsTask {
        serde_json::from_value(json!({
            "schema": "economics-task/v1", "id": id, "title": id, "category": "c",
            "complexity": "small", "source": "s", "fixture": "fx", "acceptance": "hidden",
            "reference": "ref.patch", "verify": verify, "scope": [],
            "sessions": [{ "prompt": "prompt-1.md" }]
        }))
        .unwrap()
    }

    fn declarations(suite: EconomicsSuite, tasks: Vec<EconomicsTask>) -> Declarations {
        Declarations {
            methodology: methodology(),
            suites: [(suite.id.clone(), suite)].into(),
            tasks: tasks.into_iter().map(|t| (t.id.clone(), t)).collect(),
        }
    }

    fn options(root: &Path, work_dir: &Path) -> RunOptions {
        RunOptions {
            root: root.into(),
            suite: "pilot".into(),
            tasks: Vec::new(),
            repetitions: Vec::new(),
            work_dir: work_dir.into(),
            harness: PathBuf::from("/nonexistent/harness"),
            majordomus: PathBuf::from("/nonexistent/majordomus"),
            parallel: 2,
            force: false,
            dry_run: false,
            session_timeout: Duration::from_secs(1),
        }
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn files(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    /// A repository with a task `id` whose fixture has one test file, whose hidden test is
    /// `hidden/test_hidden.py`, and whose prompt is `prompt-1.md`, committed: a run records
    /// against a HEAD.
    fn repository(root: &Path, id: &str) {
        write(&root.join("fx/app.py"), "x = 1\n");
        write(&root.join("fx/tests/test_app.py"), "assert True\n");
        write(&root.join("hidden/test_hidden.py"), "assert solved\n");
        write(
            &root.join(DIR).join("tasks").join(id).join("prompt-1.md"),
            "Solve it.\n",
        );
        git(root, &["init", "-q"]).unwrap();
        git(root, &["add", "-A"]).unwrap();
        git(
            root,
            &[
                "-c",
                "user.email=t@example.com",
                "-c",
                "user.name=t",
                "commit",
                "-qm",
                "i",
            ],
        )
        .unwrap();
    }

    /// A stand-in harness: answers `--version`, runs `body` in the workspace, and prints a
    /// transcript that ends with `result`.
    fn harness(dir: &Path, body: &str, result: &str) -> PathBuf {
        let path = dir.join("harness");
        let request = r#"{"type":"assistant","message":{"id":"m1","model":"claude-test","#
            .to_string()
            + r#""content":[],"usage":{"input_tokens":10,"output_tokens":5}}}"#;
        let script = format!(
            "#!/bin/sh\n\
             if [ \"$1\" = \"--version\" ]; then echo '1.2.3 (Stand-in)'; exit 0; fi\n\
             {body}\necho '{request}'\necho '{result}'\n"
        );
        write(&path, &script);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    /// Whether the process `pid` is gone within a few seconds. A zombie counts as gone: it
    /// has been killed, and only its reaping is left to whoever inherited it.
    fn ended(pid: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let out = Command::new("ps")
                .args(["-o", "stat=", "-p", pid])
                .output()
                .unwrap();
            let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if stat.is_empty() || stat.starts_with('Z') {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    const SUCCESS: &str = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":1}"#;

    const CREATE_SOLVED: &str = "--- /dev/null\n+++ b/solved\n@@ -0,0 +1 @@\n+yes\n";

    fn record(root: &Path, id: &str) -> EconomicsRun {
        let text = std::fs::read_to_string(record_path(root, "pilot", id)).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn walk_skips_bytecode_caches_and_git_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        write(&d.join("app.py"), "x = 1\n");
        write(&d.join("pkg/mod.py"), "y = 2\n");
        write(&d.join("pkg/mod.pyc"), "\0");
        write(&d.join("__pycache__/app.cpython-312.pyc"), "\0");
        write(&d.join("pkg/__pycache__/notes.txt"), "cached");
        write(&d.join(".git/HEAD"), "ref: refs/heads/main\n");
        write(&d.join("tests/test_app.py"), "def test_app(): pass\n");
        assert_eq!(files(d), ["app.py", "pkg/mod.py", "tests/test_app.py"]);
        assert!(
            files(&d.join("absent")).is_empty(),
            "a missing directory holds nothing"
        );
    }

    #[test]
    fn a_fixture_digests_the_same_in_every_checkout_and_changes_with_one_byte() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        for repo in [a.path(), b.path()] {
            write(&repo.join("fx/app.py"), "x = 1\n");
            write(&repo.join("fx/tests/test_app.py"), "assert True\n");
            write(&repo.join("tasks/t/task.yaml"), "id: t\n");
        }
        let digest = |repo: &Path| {
            sha256_file_tree(repo, &repo.join("fx"), &[&repo.join("tasks/t/task.yaml")])
        };
        let first = digest(a.path());
        assert_eq!(first.len(), 64, "a hex SHA-256");
        assert_eq!(
            first,
            digest(b.path()),
            "the checkout's location is not part of the digest"
        );

        write(&b.path().join("fx/__pycache__/app.cpython-312.pyc"), "\0");
        assert_eq!(
            first,
            digest(b.path()),
            "bytecode a run leaves behind is not the fixture"
        );

        write(&b.path().join("fx/app.py"), "x = 2\n");
        assert_ne!(first, digest(b.path()), "one changed byte in the fixture");

        write(&b.path().join("fx/app.py"), "x = 1\n");
        write(&b.path().join("tasks/t/task.yaml"), "id: u\n");
        assert_ne!(
            first,
            digest(b.path()),
            "one changed byte in a declaration file"
        );
    }

    #[test]
    fn the_harness_is_shut_off_from_user_configuration_and_given_the_budget() {
        let mcp = Path::new("/tmp/mj-bench-0123456789ab/mcp.json");
        let args = harness_args("claude-test", Some(5), mcp);
        for flag in [
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--no-session-persistence",
        ] {
            assert!(
                args.iter().any(|a| a == flag),
                "{flag} missing from {args:?}"
            );
        }
        let after = |flag: &str| {
            let i = args.iter().position(|a| a == flag)?;
            args.get(i + 1).map(String::as_str)
        };
        assert_eq!(after("--setting-sources"), Some("project,local"));
        assert_eq!(
            after("--mcp-config"),
            Some("/tmp/mj-bench-0123456789ab/mcp.json")
        );
        assert_eq!(after("--model"), Some("claude-test"));
        assert_eq!(after("--output-format"), Some("stream-json"));
        assert_eq!(after("--max-budget-usd"), Some("5"));

        let unbounded = harness_args("claude-test", None, mcp);
        assert!(!unbounded.iter().any(|a| a == "--max-budget-usd"));
        assert_eq!(
            unbounded.len() + 2,
            args.len(),
            "the budget is the only difference"
        );

        // the two arms of a pair are invoked alike from different workspaces
        let other = harness_args(
            "claude-test",
            Some(5),
            Path::new("/tmp/mj-bench-ba9876543210/mcp.json"),
        );
        assert_eq!(configuration_digest(&args), configuration_digest(&other));
        assert_ne!(
            configuration_digest(&args),
            configuration_digest(&unbounded)
        );
        assert_ne!(
            configuration_digest(&args),
            configuration_digest(&harness_args("claude-other", Some(5), mcp))
        );
    }

    #[test]
    fn only_what_ties_a_child_to_the_callers_session_or_repository_is_withheld() {
        for name in [
            "CLAUDECODE",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_SSE_PORT",
            "CLAUDE_PROJECT_DIR",
            "MAJORDOMUS_SHARE",
            "MAJORDOMUS_ROOT",
            "MJ_TASK",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_TARGET_DIR",
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_INDEX_FILE",
        ] {
            assert!(tied(name), "{name} ties the child to the caller");
        }
        // credentials and provider routing: both arms need them, and alike
        for name in [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_BASE_URL",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_USE_VERTEX",
            "AWS_REGION",
            "HOME",
            "PATH",
            "CARGO_HOME",
        ] {
            assert!(
                !tied(name),
                "{name} is not the caller's session or repository"
            );
        }
    }

    #[test]
    fn a_child_never_inherits_the_callers_session_or_repository() {
        let removed = ["MAJORDOMUS_RUNNER_TEST_PROBE", "MJ_RUNNER_TEST_PROBE"];
        for name in removed {
            std::env::set_var(name, "1");
        }
        std::env::set_var("CLAUDE_CODE_RUNNER_TEST_PROBE", "1");

        let mut cmd = Command::new("true");
        clean_env(&mut cmd, Some(Path::new("/opt/majordomus/bin/majordomus")));
        let envs: BTreeMap<String, Option<String>> = cmd
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect();
        let mut bare = Command::new("true");
        clean_env(&mut bare, None);
        let bare_path = bare.get_envs().any(|(k, _)| k == "PATH");
        let mut relative = Command::new("true");
        clean_env(&mut relative, Some(Path::new("majordomus")));
        let relative_path = relative.get_envs().any(|(k, _)| k == "PATH");

        // the probes go before anything is asserted, so a failure leaves no residue behind
        for name in removed {
            std::env::remove_var(name);
        }
        std::env::remove_var("CLAUDE_CODE_RUNNER_TEST_PROBE");

        for name in removed {
            assert_eq!(
                envs.get(name),
                Some(&None),
                "{name} must be removed from the child"
            );
        }
        assert!(
            !envs.contains_key("CLAUDE_CODE_RUNNER_TEST_PROBE"),
            "a CLAUDE_CODE_ variable that is not the session's is inherited as is"
        );
        let path = envs["PATH"].as_deref().unwrap();
        assert!(
            path.starts_with("/opt/majordomus/bin:"),
            "the installed executable first: {path}"
        );
        assert_eq!(envs["PYTHONDONTWRITEBYTECODE"].as_deref(), Some("1"));
        assert_eq!(
            envs["DISABLE_AUTOUPDATER"].as_deref(),
            Some("1"),
            "the harness stays put"
        );
        assert!(
            !bare_path,
            "the control is given no executable: PATH is left alone"
        );
        assert!(
            !relative_path,
            "an executable with no directory never puts `` on PATH"
        );
    }

    #[test]
    fn run_refuses_a_work_directory_inside_the_repository() {
        let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let decl = declarations(suite("live", &[], Some("majordomus")), Vec::new());

        let inside = options(repo.path(), &repo.path().join("tmp/economics"));
        let err = run(&inside, &decl, &|_| {}).unwrap_err();
        assert!(err.contains("outside the repository"), "{err}");
        assert!(
            !repo.path().join("tmp").exists(),
            "what the check created is taken back"
        );

        let outside = options(repo.path(), work.path());
        assert_eq!(
            run(&outside, &decl, &|_| {}).unwrap(),
            (Vec::new(), Vec::new())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_cannot_put_the_work_directory_inside_the_repository() {
        let (repo, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let decl = declarations(suite("live", &[], Some("majordomus")), Vec::new());
        // a link outside that leads in
        let into = other.path().join("into-repo");
        std::os::unix::fs::symlink(repo.path(), &into).unwrap();
        let err = run(&options(repo.path(), &into.join("work")), &decl, &|_| {}).unwrap_err();
        assert!(err.contains("outside the repository"), "{err}");
        assert!(
            !repo.path().join("work").exists(),
            "taken back through the link as well"
        );
        // and a repository named through a link, with the work directory spelled directly
        let root_link = other.path().join("repo-link");
        std::os::unix::fs::symlink(repo.path(), &root_link).unwrap();
        let err = run(&options(&root_link, &repo.path().join("w")), &decl, &|_| {}).unwrap_err();
        assert!(err.contains("outside the repository"), "{err}");
        // a directory that already existed is left where it was
        std::fs::create_dir(repo.path().join("kept")).unwrap();
        assert!(run(
            &options(repo.path(), &repo.path().join("kept")),
            &decl,
            &|_| {}
        )
        .is_err());
        assert!(repo.path().join("kept").is_dir());
    }

    #[test]
    fn run_refuses_an_unknown_suite_a_context_suite_and_an_undeclared_task() {
        let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut opts = options(repo.path(), work.path());

        let live = declarations(suite("live", &[], None), Vec::new());
        opts.suite = "nightly".into();
        assert_eq!(
            run(&opts, &live, &|_| {}).unwrap_err(),
            "no suite \"nightly\""
        );

        opts.suite = "pilot".into();
        let context = declarations(suite("context", &[], None), Vec::new());
        let err = run(&opts, &context, &|_| {}).unwrap_err();
        assert!(
            err.contains("is a context suite") && err.contains("economics measure"),
            "{err}"
        );

        let ghost = declarations(suite("live", &["ghost"], None), Vec::new());
        assert_eq!(run(&opts, &ghost, &|_| {}).unwrap_err(), "no task ghost");
    }

    #[test]
    fn force_is_refused_before_anything_happens() {
        let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let decl = declarations(suite("live", &["t"], None), vec![task("t", "true")]);
        let mut opts = options(repo.path(), &work.path().join("w"));
        opts.force = true;
        let err = run(&opts, &decl, &|_| {}).unwrap_err();
        assert!(
            err.contains("never overwritten") && err.contains("methodology"),
            "{err}"
        );
        assert!(
            !work.path().join("w").exists(),
            "refused before the work directory exists"
        );
    }

    #[test]
    fn a_record_is_created_once_and_never_written_over() {
        let repo = tempfile::tempdir().unwrap();
        let path = record_path(repo.path(), "pilot", "t--baseline--r1");
        let run: EconomicsRun = serde_json::from_value(json!({
            "schema": "economics-run/v1", "id": "t--baseline--r1", "suite": "pilot",
            "suite_version": 1, "methodology": 1, "task": "t", "variant": "baseline",
            "repetition": 1, "provider": "anthropic",
            "harness": {"name": "claude-code", "version": "0"}, "model_requested": "m",
            "models_reported": ["m"], "repository": {"commit": "0", "dirty": false},
            "majordomus_version": "0", "fixture_digest": "", "inputs_digest": "",
            "configuration_digest": "", "started_at": "", "finished_at": "", "sessions": [],
            "outcome": {"completed": true, "checks": [], "changed_files": []}
        }))
        .unwrap();
        write_record(&path, &run).unwrap();
        let first = std::fs::read_to_string(&path).unwrap();
        let mut other = run.clone();
        other.outcome.completed = false;
        let err = write_record(&path, &other).unwrap_err();
        assert!(
            err.contains("already recorded") && err.contains("never overwritten"),
            "{err}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            first,
            "the record is untouched"
        );
    }

    #[test]
    fn a_recorded_run_is_not_run_again() {
        let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let decl = declarations(
            suite("live", &["t"], Some("majordomus")),
            vec![task("t", "true")],
        );
        for variant in ["baseline", "majordomus"] {
            write(
                &record_path(repo.path(), "pilot", &run_id("t", variant, 1)),
                "{}\n",
            );
        }
        let opts = options(repo.path(), work.path());
        let log = Mutex::new(Vec::new());
        let (written, failed) = run(&opts, &decl, &|l| log.lock().unwrap().push(l)).unwrap();
        assert!(written.is_empty() && failed.is_empty());
        assert_eq!(
            log.into_inner().unwrap(),
            ["pilot: 0 run(s) to do, 2 at a time"]
        );
        assert!(
            !work.path().join("pilot").exists(),
            "no transcript directory was made"
        );
    }

    #[test]
    fn the_control_starts_from_the_committed_fixture() {
        let (repo, ws) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write(&repo.path().join("fx/app.py"), "x = 1\n");
        write(&repo.path().join("fx/tests/test_app.py"), "assert True\n");
        write(
            &repo.path().join("fx/__pycache__/app.cpython-312.pyc"),
            "\0",
        );
        let job = Job {
            task: task("t", "true"),
            variant: "baseline".into(),
            repetition: 1,
        };
        let at = ws.path().join("repo");
        let head = prepare(&options(repo.path(), ws.path()), &job, false, &at).unwrap();
        assert_eq!(head.len(), 40, "a commit: {head}");
        assert_eq!(files(&at), ["app.py", "tests/test_app.py"]);
        let status = git(&at, &["status", "--porcelain"]).unwrap();
        assert_eq!(
            status, "",
            "the fixture is committed: the control starts from a clean tree"
        );
        // the history names no benchmark, task or arm: one commit, by a plain identity
        let history = git(&at, &["log", "--format=%an <%ae> %cn <%ce> %s"]).unwrap();
        assert_eq!(
            history.trim(),
            "Developer <developer@example.invalid> Developer <developer@example.invalid> \
             Initial commit"
        );
    }

    #[test]
    fn a_dry_run_prepares_a_workspace_runs_nothing_and_leaves_nothing() {
        let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        repository(repo.path(), "t");
        let decl = declarations(suite("live", &["t"], None), vec![task("t", "true")]);
        let mut opts = options(repo.path(), work.path());
        opts.dry_run = true;
        let log = Mutex::new(Vec::new());
        let (written, failed) = run(&opts, &decl, &|l| log.lock().unwrap().push(l)).unwrap();
        assert!(written.is_empty());
        assert_eq!(failed, ["t--baseline--r1: dry run"]);
        let log = log.into_inner().unwrap();
        let plan = log
            .iter()
            .find(|l| l.contains("would run:"))
            .expect("the command is printed");
        assert!(plan.contains("--strict-mcp-config") && plan.contains("/nonexistent/harness"));
        assert!(plan.contains("prepared a workspace at ") && plan.contains("removed it again"));
        let mcp = plan
            .split_whitespace()
            .find(|w| w.ends_with("mcp.json"))
            .unwrap();
        assert!(
            !Path::new(mcp).parent().unwrap().exists(),
            "the workspace is gone: {mcp}"
        );
        assert!(
            !work.path().join("pilot").exists(),
            "no transcript directory for a dry run"
        );
    }

    #[test]
    fn a_run_records_in_a_neutral_workspace_and_leaves_no_workspace_behind() {
        let (repo, work, probe) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        let id = "distinctive-task";
        repository(repo.path(), id);
        let where_ = probe.path().join("where");
        let listing = probe.path().join("listing");
        let body = format!(
            "pwd > '{}'\nls -a .. > '{}'\necho yes > solved",
            where_.display(),
            listing.display()
        );
        let mut opts = options(repo.path(), work.path());
        opts.harness = harness(probe.path(), &body, SUCCESS);
        opts.session_timeout = Duration::from_secs(60);
        let decl = declarations(
            suite("live", &[id], None),
            vec![task(id, "test -f solved")],
        );
        let (written, failed) = run(&opts, &decl, &|_| {}).unwrap();
        assert!(failed.is_empty(), "{failed:?}");
        assert_eq!(
            written,
            [record_path(
                repo.path(),
                "pilot",
                "distinctive-task--baseline--r1"
            )]
        );

        let r = record(repo.path(), "distinctive-task--baseline--r1");
        let ids: Vec<&str> = r.outcome.checks.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "sessions_completed",
                "visible_tests",
                "acceptance_tests",
                "tests_kept"
            ]
        );
        assert!(r.outcome.completed, "{:?}", r.outcome.checks);
        assert_eq!(r.sessions[0].ended, "success");
        assert_eq!(r.harness.version, "1.2.3");
        assert_eq!(
            r.repository.commit,
            git(repo.path(), &["rev-parse", "HEAD"]).unwrap().trim()
        );
        assert_eq!(r.inputs_digest, inputs_digest(repo.path(), &[]).unwrap());
        assert!(
            r.outcome.changed_files == ["solved"],
            "{:?}",
            r.outcome.changed_files
        );

        // the workspace named nothing, sat alone, and is gone
        let at = std::fs::read_to_string(&where_).unwrap();
        let at = Path::new(at.trim());
        assert_eq!(at.file_name().unwrap(), "repo");
        let parent = at
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            parent.starts_with("mj-bench-") && parent.len() == "mj-bench-".len() + 12,
            "{parent}"
        );
        assert!(
            parent["mj-bench-".len()..]
                .chars()
                .all(|c| c.is_ascii_hexdigit()),
            "{parent}"
        );
        for word in [id, "baseline", "pilot", "economics"] {
            assert!(
                !at.to_string_lossy().contains(word),
                "{word} in {}",
                at.display()
            );
        }
        let beside = std::fs::read_to_string(&listing).unwrap();
        let beside: BTreeSet<&str> = beside.lines().collect();
        assert_eq!(
            beside,
            BTreeSet::from([".", "..", "mcp.json", "repo"]),
            "nothing else beside it"
        );
        assert!(
            !at.exists() && !at.parent().unwrap().exists(),
            "the workspace was removed"
        );

        // the transcript is in the work directory, not in the workspace
        let transcript = work
            .path()
            .join("pilot/distinctive-task--baseline--r1/session-1.jsonl");
        assert!(std::fs::read_to_string(transcript)
            .unwrap()
            .contains("\"result\""));
    }

    #[test]
    fn a_session_that_failed_or_timed_out_is_not_a_completed_run() {
        let (repo, work, probe) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        repository(repo.path(), "t");
        let decl = declarations(
            suite("live", &["t"], None),
            vec![task("t", "test -f solved")],
        );

        // the tree passes every test, but the harness reported an error
        let failed =
            r#"{"type":"result","subtype":"error_max_budget_usd","is_error":true,"num_turns":3}"#;
        let mut opts = options(repo.path(), work.path());
        opts.session_timeout = Duration::from_secs(60);
        opts.harness = harness(probe.path(), "echo yes > solved", failed);
        run(&opts, &decl, &|_| {}).unwrap();
        let r = record(repo.path(), "t--baseline--r1");
        assert!(!r.outcome.completed);
        let gate = &r.outcome.checks[0];
        assert_eq!(
            (gate.id.as_str(), gate.passed),
            ("sessions_completed", false)
        );
        assert_eq!(gate.detail, "session 1 ended error_max_budget_usd (error)");
        assert!(
            r.outcome.checks[1..].iter().all(|c| c.passed),
            "only the session gate failed"
        );

        // cut off by the timeout after it solved the task and said so: still not completed,
        // and whatever it started is killed with it
        let pid = probe.path().join("pid");
        let body = format!(
            "echo yes > solved\nsleep 60 &\necho $! > '{}'\nsleep 60",
            pid.display()
        );
        opts.harness = harness(probe.path(), &body, SUCCESS);
        opts.session_timeout = Duration::from_secs(1);
        let decl = declarations(
            serde_json::from_value(json!({
                "schema": "economics-suite/v1", "id": "pilot", "version": 1, "kind": "live",
                "title": "Pilot", "model": "claude-test", "control": "baseline",
                "repetitions": 2, "tasks": ["t"], "freshness_inputs": []
            }))
            .unwrap(),
            vec![task("t", "test -f solved")],
        );
        opts.repetitions = vec![2];
        let started = Instant::now();
        let (written, failed) = run(&opts, &decl, &|_| {}).unwrap();
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "the timeout ended the session"
        );
        assert!(failed.is_empty() && written.len() == 1, "{failed:?}");
        let r = record(repo.path(), "t--baseline--r2");
        assert_eq!(
            (r.sessions[0].ended.as_str(), r.sessions[0].is_error),
            ("timeout", true)
        );
        assert!(!r.outcome.completed);
        assert_eq!(
            r.outcome.checks[0].detail,
            "session 1 ended timeout (error)"
        );
        let pid = std::fs::read_to_string(&pid).unwrap();
        assert!(
            ended(pid.trim()),
            "a process the session started outlived its timeout"
        );
    }

    #[test]
    fn a_run_whose_checkout_changed_in_flight_is_not_recorded() {
        let (repo, work, probe) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        repository(repo.path(), "t");
        let decl = declarations(suite("live", &["t"], None), vec![task("t", "true")]);
        let mut opts = options(repo.path(), work.path());
        opts.session_timeout = Duration::from_secs(60);
        // the session edits a tracked file of the operator's checkout
        let edit = format!(
            "echo 'x = 2' > '{}'",
            repo.path().join("fx/app.py").display()
        );
        opts.harness = harness(probe.path(), &edit, SUCCESS);
        let (written, failed) = run(&opts, &decl, &|_| {}).unwrap();
        assert!(written.is_empty());
        assert_eq!(failed.len(), 1);
        assert!(
            failed[0].contains("changed while the run was in flight"),
            "{}",
            failed[0]
        );
        assert!(!record_path(repo.path(), "pilot", "t--baseline--r1").exists());
    }

    #[test]
    fn verify_is_bounded_and_ends_everything_it_started() {
        let ws = tempfile::tempdir().unwrap();
        let started = Instant::now();
        let (passed, detail) = verify(
            ws.path(),
            "sleep 60 & echo $! > pid; sleep 60",
            Duration::from_secs(1),
        );
        assert!(started.elapsed() < Duration::from_secs(20));
        assert!(!passed);
        assert_eq!(detail, "timed out after 1s");
        let pid = std::fs::read_to_string(ws.path().join("pid")).unwrap();
        assert!(
            ended(pid.trim()),
            "the background process survived the timeout"
        );

        let (passed, detail) = verify(
            ws.path(),
            "echo first >&2; echo last >&2; exit 3",
            VERIFY_LIMIT,
        );
        assert!(!passed);
        assert_eq!(detail, "exit 3: last");
        assert_eq!(
            verify(ws.path(), "true", VERIFY_LIMIT),
            (true, "exit 0: ".to_string())
        );
    }

    #[test]
    fn the_session_gate_names_every_session_that_did_not_succeed() {
        let s = |index: u32, ended: &str, is_error: bool| EconomicsSession {
            index,
            prompt_sha256: String::new(),
            duration_ms: 0,
            turns: None,
            ended: ended.into(),
            is_error,
            requests: vec![],
            models: vec![],
            reported_cost_microusd: None,
            tools: BTreeMap::new(),
            orientation: EconomicsOrientation::default(),
        };
        let ok = sessions_check(&[s(1, "success", false), s(2, "success", false)]);
        assert!(ok.passed);
        assert_eq!(
            ok.detail,
            "all 2 session(s) ended with the harness's success result"
        );
        let bad = sessions_check(&[
            s(1, "success", false),
            s(2, "no_result", true),
            s(3, "success", true),
        ]);
        assert!(!bad.passed);
        assert_eq!(
            bad.detail,
            "session 2 ended no_result (error); session 3 ended success (error)"
        );
        let none = sessions_check(&[]);
        assert!(!none.passed, "a run of no session completed nothing");
    }

    #[test]
    fn the_benchmark_policy_turns_the_server_off_and_keeps_the_teams_files() {
        let skeleton = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../share/skeleton/policy.yaml"
        ))
        .unwrap();
        let before: Policy = yaml::parse_into(&skeleton).unwrap();
        assert!(
            before.session.ensure_server_on_start,
            "the skeleton converges on a server"
        );

        let text = benchmark_policy(&skeleton, &["CLAUDE.md"]).unwrap();
        let after: Policy = yaml::parse_into(&text).unwrap();
        assert!(!after.session.ensure_server_on_start);
        assert_eq!(
            after.session.freshness, before.session.freshness,
            "the rest of the block kept"
        );
        let modes: Vec<(&str, ProjectionMode)> = after
            .projections
            .iter()
            .map(|p| (p.target.as_str(), p.mode))
            .collect();
        assert_eq!(
            modes,
            [
                ("AGENTS.md", ProjectionMode::File),
                ("CLAUDE.md", ProjectionMode::Region),
                ("GEMINI.md", ProjectionMode::File)
            ]
        );
        assert_eq!(
            text.matches("ensure_server_on_start").count(),
            1,
            "replaced, not duplicated"
        );
        // one line replaced, one added (`mode: region`), nothing else touched
        let gone: Vec<&str> = skeleton
            .lines()
            .filter(|l| !text.lines().any(|t| t == *l))
            .collect();
        assert_eq!(gone.len(), 1, "{gone:?}");
        assert!(
            gone[0]
                .trim_start()
                .starts_with("ensure_server_on_start: true"),
            "{gone:?}"
        );
        assert_eq!(text.lines().count(), skeleton.lines().count() + 1);

        // a policy without the key gets it, one without the block gets both
        let absent = "version: 1\nsession:\n  briefing_on_start: true\nprojections: []\n";
        let text = benchmark_policy(absent, &[]).unwrap();
        assert!(
            text.contains("session:\n  ensure_server_on_start: false\n  briefing_on_start: true\n"),
            "{text}"
        );
        let text = benchmark_policy("version: 1\nprojections: []\n", &[]).unwrap();
        assert!(
            !yaml::parse_into::<Policy>(&text)
                .unwrap()
                .session
                .ensure_server_on_start
        );

        // a nested key of the same name is not the setting
        let nested =
            "session:\n  briefing_on_start: true\n  other:\n    ensure_server_on_start: true\n";
        let text = benchmark_policy(nested, &[]).unwrap();
        assert!(
            text.contains("\n    ensure_server_on_start: true\n"),
            "the nested key is left alone"
        );

        // a change that cannot be made is refused, never run as a different treatment
        let err = benchmark_policy("version: 1\nprojections: []\n", &["CLAUDE.md"]).unwrap_err();
        assert!(err.contains("CLAUDE.md"), "{err}");
        assert!(benchmark_policy("session: { ensure_server_on_start: true }\n", &[]).is_err());
    }

    #[test]
    fn a_reference_must_fail_before_and_pass_after() {
        let (repo, work) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write(&repo.path().join("fx/app.py"), "x = 1\n");
        write(&repo.path().join("fx/tests/test_app.py"), "assert True\n");
        write(
            &repo.path().join("hidden/test_hidden.py"),
            "assert solved\n",
        );
        write(&repo.path().join("ref.patch"), CREATE_SOLVED);
        // the hidden test is copied in before either verification
        let solves = task("solves", "test -f tests/test_hidden.py && test -f solved");
        let trivial = task("trivial", "true");
        let mut unapplied = task("unapplied", "test -f solved");
        unapplied.reference = "missing.patch".into();
        let decl = declarations(suite("live", &[], None), vec![solves, trivial, unapplied]);

        let bad = check_references(repo.path(), &decl, work.path()).unwrap_err();
        assert_eq!(bad.len(), 2, "{bad:?}");
        assert_eq!(
            bad[0],
            "trivial: the hidden tests already pass on the starting state"
        );
        assert!(
            bad[1].starts_with("unapplied: the reference does not apply"),
            "{}",
            bad[1]
        );

        let only = declarations(
            suite("live", &[], None),
            vec![task("solves", "test -f solved")],
        );
        let ok = check_references(repo.path(), &only, work.path()).unwrap();
        assert_eq!(
            ok,
            ["solves: fails on the starting state, passes on the reference"]
        );
        assert!(
            files(&work.path().join("references")).is_empty(),
            "no hidden test left behind"
        );
    }
}
