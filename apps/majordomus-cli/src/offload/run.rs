//! One offloaded derive, end to end: the worktree goes to a host as a git bundle, the host
//! derives it in its turn, and the derived files come back staged.
//!
//! What it does, and what it never does:
//!
//! - The tree that travels is the worktree as it stands: HEAD when it is clean, otherwise a
//!   `git stash create` snapshot of its tracked and staged changes. That writes a commit
//!   object and touches neither the index, the worktree nor a hook — which matters, because
//!   the commit hook refuses a tree that is not derived yet, so the authored change cannot
//!   be committed before its derive. Untracked files would not travel and are refused.
//! - A merge in progress travels as a real merge commit with both parents, written by
//!   `git commit-tree` from the staged tree once every conflict is resolved: a stash would
//!   drop the second parent, and the changelog — composed from the log — would be derived
//!   without the merged commits (PR #854, 2026-10-10). No ref of the branch moves.
//! - Nothing is pushed to origin, so no CI run starts, and no hook of this repository runs.
//! - On the host, the derive runs in a detached worktree `offload-<tag>`, under the queue,
//!   with a build directory of its own: a copy-on-write clone of the shared one where the
//!   filesystem has clones, removed when the derive ends. A build into the shared directory
//!   by a derive that never took the lock cannot replace this derive's executable (it did,
//!   on the second MacBook, 2026-10-11 00:25Z).
//! - Back here the files the derive changed are checked out of the derived snapshot and
//!   staged. Nothing is committed. A derive that deletes a tracked file is refused rather
//!   than half-applied.
//! - The machine's own derive lock is not taken: it guards this machine, and this machine
//!   is not deriving.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::declaration::{OffloadDeclaration, OffloadHost};
use super::host::{probe, remote_of, HostReport};
use super::queue::{Queue, QueueError};
use super::remote::Holder;

/// A precondition of the worktree is not met: an untracked file, an unresolved merge.
pub const EXIT_PRECONDITION: u8 = 2;
/// The derive failed on the host.
pub const EXIT_DERIVE_FAILED: u8 = 10;
/// The derive deleted a tracked file; nothing was applied.
pub const EXIT_DELETES: u8 = 11;
/// No host could take the derive, or the host could not be used, or the result did not apply.
pub const EXIT_UNAVAILABLE: u8 = 12;
/// The queue's declared wait ran out.
pub const EXIT_TIMED_OUT: u8 = 75;

/// Why an offload stopped, with the exit code the command answers.
#[derive(Debug)]
pub struct Refusal {
    /// The exit code.
    pub code: u8,
    /// What happened and what to do.
    pub reason: String,
}

fn refuse(code: u8, reason: impl Into<String>) -> Refusal {
    Refusal {
        code,
        reason: reason.into(),
    }
}

/// What an offload did.
#[derive(Debug, Clone, Serialize)]
pub struct OffloadReport {
    /// The host it ran on.
    pub host: String,
    /// The first ten characters of the commit that travelled.
    pub tag: String,
    /// The commit that travelled.
    pub input: String,
    /// The derived snapshot, when the derive changed anything.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived: Option<String>,
    /// The files staged here, sorted.
    pub staged: Vec<String>,
    /// Seconds spent waiting in the host's queue.
    pub waited_seconds: u64,
    /// Seconds in all.
    pub seconds: u64,
    /// Every host's measurement, when the host was chosen rather than named.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub probe: Vec<HostReport>,
}

/// How a derive is asked for.
#[derive(Debug, Clone, Default)]
pub struct OffloadRequest {
    /// The worktree to derive.
    pub worktree: PathBuf,
    /// A declared host id or address; absent means choose.
    pub host: Option<String>,
    /// Keep the host's worktree after the derive.
    pub keep: bool,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, Refusal> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| refuse(EXIT_UNAVAILABLE, format!("git could not be run: {e}")))?;
    if !out.status.success() {
        return Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn git_ok(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The identity of the scratch commit objects this module writes (the input snapshot, the
/// merge input): no ref names them and nobody's identity belongs on them, and a machine
/// without a configured identity can still offload.
const SCRATCH_NAME: &str = "user.name=offload";
const SCRATCH_EMAIL: &str = "user.email=offload@invalid";

/// The commit that travels, after the worktree's preconditions are checked.
pub fn input_commit(worktree: &Path) -> Result<String, Refusal> {
    let merge_head = git(
        worktree,
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "MERGE_HEAD",
        ],
    )?;
    let in_merge = Path::new(merge_head.trim()).exists();
    if in_merge {
        if !git(worktree, &["diff", "--name-only", "--diff-filter=U"])?
            .trim()
            .is_empty()
        {
            return Err(refuse(
                EXIT_PRECONDITION,
                "the merge still has unresolved paths — resolve and git add them first",
            ));
        }
        if !git_ok(worktree, &["diff", "--quiet"]) {
            return Err(refuse(EXIT_PRECONDITION, "the merge has unstaged changes — git add them first, the staged tree is what travels"));
        }
    }
    let status = git(
        worktree,
        &["status", "--porcelain", "--untracked-files=all"],
    )?;
    let untracked: Vec<&str> = status
        .lines()
        .filter_map(|l| l.strip_prefix("?? "))
        .collect();
    if !untracked.is_empty() {
        return Err(refuse(
            EXIT_PRECONDITION,
            format!(
                "untracked files would not travel — git add them first (a derive sees what the commit will): {}",
                untracked.join(", ")
            ),
        ));
    }
    if in_merge {
        let tree = git(worktree, &["write-tree"])?;
        let other = git(worktree, &["rev-parse", "MERGE_HEAD"])?;
        let head = git(
            worktree,
            &[
                "-c",
                SCRATCH_NAME,
                "-c",
                SCRATCH_EMAIL,
                "commit-tree",
                tree.trim(),
                "-p",
                "HEAD",
                "-p",
                other.trim(),
                "-m",
                "derive-offload input: a merge in progress",
            ],
        )?;
        return Ok(head.trim().to_string());
    }
    let stash = git(
        worktree,
        &[
            "-c",
            SCRATCH_NAME,
            "-c",
            SCRATCH_EMAIL,
            "stash",
            "create",
            "derive-offload input",
        ],
    )?;
    if !stash.trim().is_empty() {
        return Ok(stash.trim().to_string());
    }
    Ok(git(worktree, &["rev-parse", "HEAD"])?.trim().to_string())
}

/// Prepare the host's worktree from the uploaded bundle, or — when the input is already in
/// the base, so there was nothing to bundle — from the commit itself, which the host has
/// once it has fetched origin. Arguments: checkout, worktrees directory, spool, tag, input.
const PREPARE: &str = r#"set -e
c=$1; w=$2; spool=$3; tag=$4; input=$5
case $c in /*) ;; *) c=$HOME/$c ;; esac
case $w in /*) ;; *) w=$HOME/$w ;; esac
cd "$c"
git fetch -q origin
if [ -f "$spool/offload-$tag.bundle" ]; then
  git fetch -q "$spool/offload-$tag.bundle" "refs/offload/in-$tag:refs/offload/$tag"
else
  git update-ref "refs/offload/$tag" "$input"
fi
d=$w/offload-$tag
[ -d "$d" ] || git worktree add -q --detach "$d" "refs/offload/$tag"
cd "$d"
git checkout -q -f --detach "refs/offload/$tag"
git clean -fdq"#;

/// Derive on the host, under a lock the caller holds. Its own pid joins the lock's holders,
/// so the lock stays held exactly as long as this derive runs. Arguments: lock, worktrees
/// directory, tag, shared build directory, derive command, spool, PATH prefix
/// (colon-separated), then the files to touch. Prints `target <dir>`, then `nothing` or
/// `derived <sha>`; exit 10 when the derive fails, 13 when the lock is not there.
const DERIVE: &str = r#"l=$1; w=$2; tag=$3; shared=$4; derive=$5; spool=$6; prefix=$7; shift 7
[ -d "$l" ] || { echo "the lock $l is not held" >&2; exit 13; }
echo "pid $$"
: > "$l/$$"
case $w in /*) ;; *) w=$HOME/$w ;; esac
case $shared in /*) ;; *) shared=$HOME/$shared ;; esac
p=""; IFS=:; for e in $prefix; do case $e in /*) ;; *) e=$HOME/$e ;; esac; p="$p$e:"; done; unset IFS
export PATH="$p$PATH"
cd "$w/offload-$tag" || exit 13
private="$shared-$tag"; log="$spool/offload-$tag.log"
mkdir -p "$shared" "$spool"; rm -rf "$private"
if [ "$(uname)" = Darwin ]; then cp -cR "$shared" "$private" 2>/dev/null || { rm -rf "$private"; mkdir -p "$private"; }
else cp -a --reflink=always "$shared" "$private" 2>/dev/null || { rm -rf "$private"; mkdir -p "$private"; }; fi
trap 'rm -rf "$private"' EXIT
trap 'rm -rf "$private"; exit 129' HUP
trap 'rm -rf "$private"; exit 130' INT
trap 'rm -rf "$private"; exit 143' TERM
export CARGO_TARGET_DIR="$private"
echo "target $private"
for f in "$@"; do [ -e "$f" ] && touch "$f"; done
if ! env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN "$derive" > "$log" 2>&1; then
  echo "derive failed on $(hostname); the last lines of $log:" >&2
  grep -vE ' INFO |^ +(Compiling|Checking)' "$log" | tail -8 >&2
  exit 10
fi
grep -E '^==> derive: done' "$log" >&2 || true
git add -A
sha=$(git -c user.name=offload -c user.email=offload@invalid stash create derived || true)
if [ -z "$sha" ]; then echo nothing; exit 0; fi
git update-ref "refs/offload/$tag-derived" "$sha"
git bundle create "$spool/offload-$tag-back.bundle" "refs/offload/$tag..refs/offload/$tag-derived" >/dev/null 2>&1
echo "derived $sha""#;

/// Upload: standard input to a spool file. Arguments: spool, file name.
const UPLOAD: &str = r#"mkdir -p "$1" && cat > "$1/$2""#;

/// Download: a spool file to standard output. Arguments: spool, file name.
const DOWNLOAD: &str = r#"cat "$1/$2""#;

/// Clean up the host: the bundles, and the worktree unless kept. Arguments: checkout,
/// worktrees directory, spool, tag, keep (1 or 0).
const CLEANUP: &str = r#"c=$1; w=$2; spool=$3; tag=$4; keep=$5
case $c in /*) ;; *) c=$HOME/$c ;; esac
case $w in /*) ;; *) w=$HOME/$w ;; esac
rm -f "$spool/offload-$tag.bundle" "$spool/offload-$tag-back.bundle"
[ "$keep" = 1 ] || git -C "$c" worktree remove --force "$w/offload-$tag" 2>/dev/null || true
exit 0"#;

/// A ref this process wrote and removes however it ends.
struct ScratchRef<'a> {
    dir: &'a Path,
    name: String,
}

impl Drop for ScratchRef<'_> {
    fn drop(&mut self) {
        let _ = git(self.dir, &["update-ref", "-d", &self.name]);
    }
}

/// The lock held on a host, released however this process ends short of a crash; after a
/// crash the holders are dead and the next waiter reclaims it.
struct HeldLock<'a, 'q> {
    queue: &'q Queue<'a>,
    pids: Vec<u32>,
}

impl Drop for HeldLock<'_, '_> {
    fn drop(&mut self) {
        self.queue.release(&self.pids);
    }
}

fn pick_host(
    d: &OffloadDeclaration,
    named: Option<&str>,
    log: &mut dyn Write,
) -> Result<(OffloadHost, Vec<HostReport>), Refusal> {
    if let Some(name) = named {
        let host = d.host(name).cloned().ok_or_else(|| {
            refuse(
                EXIT_UNAVAILABLE,
                format!(
                    "'{name}' is not a declared host; declared: {}",
                    d.hosts
                        .iter()
                        .map(|h| h.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )
        })?;
        return Ok((host, Vec::new()));
    }
    let reports = probe(d);
    for r in &reports {
        if let Some(why) = &r.reason {
            let _ = writeln!(log, "derive-offload: {} skipped: {why}", r.id);
        }
    }
    let Some(chosen) = reports.iter().find(|r| r.chosen) else {
        return Err(refuse(
            EXIT_UNAVAILABLE,
            "no declared host answers with its free-space minimum free",
        ));
    };
    let _ = writeln!(
        log,
        "derive-offload: chose {} ({} running or waiting there, {} GiB free)",
        chosen.id,
        chosen.load.unwrap_or(0),
        chosen.free_gib.unwrap_or(0)
    );
    let host = d
        .host(&chosen.id)
        .cloned()
        .expect("a chosen host is declared");
    Ok((host, reports))
}

/// Offload one derive. Progress goes to `log`; the answer is the report or a refusal.
pub fn offload(
    d: &OffloadDeclaration,
    request: &OffloadRequest,
    log: &mut dyn Write,
) -> Result<OffloadReport, Refusal> {
    let started = Instant::now();
    let worktree = std::fs::canonicalize(&request.worktree).map_err(|e| {
        refuse(
            EXIT_PRECONDITION,
            format!("{}: {e}", request.worktree.display()),
        )
    })?;
    let top = git(&worktree, &["rev-parse", "--show-toplevel"])?;
    let worktree = PathBuf::from(top.trim());

    let input = input_commit(&worktree)?;
    let (host, probe_reports) = pick_host(d, request.host.as_deref(), log)?;
    let remote = remote_of(&host);
    // The base must be current here so the bundle carries only what the host lacks.
    let _ = git(&worktree, &["fetch", "-q", "origin"]);

    let tag = input[..10].to_string();
    let in_ref = format!("refs/offload/in-{tag}");
    git(&worktree, &["update-ref", &in_ref, &input])?;
    let _in_guard = ScratchRef {
        dir: &worktree,
        name: in_ref.clone(),
    };
    let scratch = tempfile_dir()?;
    let spool = d.remote.spool.as_str();
    // An input the base already reaches has nothing to bundle (git refuses an empty bundle);
    // the host has it once it fetches origin.
    if !git_ok(&worktree, &["merge-base", "--is-ancestor", &input, &d.base]) {
        let bundle = scratch.path.join("in.bundle");
        let range = format!("{}..{in_ref}", d.base);
        git(
            &worktree,
            &["bundle", "create", &bundle.to_string_lossy(), &range],
        )?;
        let upload = remote
            .run(
                UPLOAD,
                &[spool, &format!("offload-{tag}.bundle")],
                Some(&bundle),
            )
            .map_err(|e| {
                refuse(
                    EXIT_UNAVAILABLE,
                    format!("{}: the bundle could not be sent: {e}", host.id),
                )
            })?;
        if !upload.ok() {
            return Err(refuse(
                EXIT_UNAVAILABLE,
                format!(
                    "{}: the bundle could not be sent: {}",
                    host.id,
                    upload.stderr.trim()
                ),
            ));
        }
    }
    let _ = writeln!(
        log,
        "derive-offload: {tag} on {} — deriving (log: {}:{spool}/offload-{tag}.log)",
        host.id, host.address
    );
    let prep = remote
        .run(
            PREPARE,
            &[&d.remote.checkout, &d.remote.worktrees, spool, &tag, &input],
            None,
        )
        .map_err(|e| refuse(EXIT_UNAVAILABLE, format!("{}: {e}", host.id)))?;
    if !prep.ok() {
        return Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "{}: the host could not prepare the worktree: {}",
                host.id,
                prep.stderr.trim()
            ),
        ));
    }

    let queue = Queue {
        remote: &remote,
        directory: &d.queue.directory,
        lock: &d.queue.lock,
        poll: Duration::from_secs(d.queue.poll_seconds),
        wait_max: Duration::from_secs(d.queue.wait_max_seconds),
    };
    let holder = Holder::start(&remote).map_err(|e| {
        refuse(
            EXIT_UNAVAILABLE,
            format!("{}: no holder process could be started: {e}", host.id),
        )
    })?;
    let ticket = queue
        .enqueue(holder.pid, &tag)
        .map_err(|e| refuse(EXIT_UNAVAILABLE, e.to_string()))?;
    let wait_started = Instant::now();
    queue.wait(&ticket, holder.pid, log).map_err(|e| match e {
        QueueError::TimedOut(_) => refuse(EXIT_TIMED_OUT, e.to_string()),
        _ => refuse(EXIT_UNAVAILABLE, e.to_string()),
    })?;
    let waited = wait_started.elapsed().as_secs();
    let mut lock = HeldLock {
        queue: &queue,
        pids: vec![holder.pid],
    };

    let prefix = d.remote.path.join(":");
    let mut args: Vec<&str> = vec![
        &d.queue.lock,
        &d.remote.worktrees,
        &tag,
        &d.remote.target,
        &d.remote.derive,
        spool,
        &prefix,
    ];
    args.extend(d.remote.touch.iter().map(String::as_str));
    let out = remote
        .run(DERIVE, &args, None)
        .map_err(|e| refuse(EXIT_UNAVAILABLE, format!("{}: {e}", host.id)))?;
    let _ = log.write_all(out.stderr.as_bytes());
    // the derive's own pid file is dead now; release it with the holder's
    lock.pids.extend(out.stdout.lines().filter_map(|l| {
        l.strip_prefix("pid ")
            .and_then(|p| p.trim().parse::<u32>().ok())
    }));
    // released now rather than at the end of the scope, so the next waiter starts while
    // this one applies its result
    queue.release(&lock.pids);
    lock.pids.clear();
    drop(holder);
    let cleanup = |keep: bool| {
        let _ = remote.run(
            CLEANUP,
            &[
                &d.remote.checkout,
                &d.remote.worktrees,
                spool,
                &tag,
                if keep { "1" } else { "0" },
            ],
            None,
        );
    };
    match out.code {
        Some(0) => {}
        Some(10) => {
            cleanup(true);
            return Err(refuse(
                EXIT_DERIVE_FAILED,
                format!(
                    "the derive failed on {}; its worktree is kept there",
                    host.id
                ),
            ));
        }
        other => {
            cleanup(true);
            return Err(refuse(
                EXIT_UNAVAILABLE,
                format!(
                    "{}: the derive step ended with {other:?}: {}",
                    host.id,
                    out.stderr.trim()
                ),
            ));
        }
    }
    let derived = out
        .stdout
        .lines()
        .find_map(|l| l.strip_prefix("derived "))
        .map(|s| s.trim().to_string());
    let mut report = OffloadReport {
        host: host.id.clone(),
        tag: tag.clone(),
        input: input.clone(),
        derived: derived.clone(),
        staged: Vec::new(),
        waited_seconds: waited,
        seconds: 0,
        probe: probe_reports,
    };
    let Some(sha) = derived else {
        cleanup(request.keep);
        let _ = writeln!(
            log,
            "derive-offload: the derive changed nothing; {} is already current",
            worktree.display()
        );
        report.seconds = started.elapsed().as_secs();
        return Ok(report);
    };

    let back = scratch.path.join("back.bundle");
    let dl = remote
        .run_to_file(
            DOWNLOAD,
            &[spool, &format!("offload-{tag}-back.bundle")],
            &back,
        )
        .map_err(|e| refuse(EXIT_UNAVAILABLE, format!("{}: {e}", host.id)))?;
    if !dl.ok() {
        return Err(refuse(
            EXIT_UNAVAILABLE,
            format!(
                "{}: the derived bundle could not be fetched: {}",
                host.id,
                dl.stderr.trim()
            ),
        ));
    }
    cleanup(request.keep);
    let derived_ref = format!("refs/offload/{tag}-derived");
    git(
        &worktree,
        &[
            "fetch",
            "-q",
            &back.to_string_lossy(),
            &format!("{derived_ref}:{derived_ref}"),
        ],
    )?;
    apply(&worktree, &input, &sha, &derived_ref, &mut report)?;
    let _ = git(&worktree, &["update-ref", "-d", &derived_ref]);
    let _ = writeln!(
        log,
        "derive-offload: applied {} derived file(s), staged, not committed",
        report.staged.len()
    );
    report.seconds = started.elapsed().as_secs();
    Ok(report)
}

/// Check the derived snapshot against the input and stage what it changed.
fn apply(
    worktree: &Path,
    input: &str,
    sha: &str,
    derived_ref: &str,
    report: &mut OffloadReport,
) -> Result<(), Refusal> {
    let status = git(worktree, &["diff", "--name-status", input, sha])?;
    let deleted: Vec<&str> = status.lines().filter(|l| l.starts_with('D')).collect();
    if !deleted.is_empty() {
        return Err(refuse(
            EXIT_DELETES,
            format!(
                "the derive deleted tracked files; nothing was applied — apply by hand from {derived_ref}:\n{}",
                deleted.join("\n")
            ),
        ));
    }
    let names = git(worktree, &["diff", "--name-only", input, sha])?;
    let files: Vec<String> = names
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    for chunk in files.chunks(200) {
        let mut args = vec!["checkout", sha, "--"];
        args.extend(chunk.iter().map(String::as_str));
        git(worktree, &args)?;
    }
    if !git_ok(worktree, &["diff", "--quiet", sha, "--", "."]) {
        return Err(refuse(
            EXIT_UNAVAILABLE,
            "the worktree does not match the derived tree after applying it",
        ));
    }
    report.staged = files;
    report.staged.sort();
    Ok(())
}

/// A scratch directory removed when dropped.
struct Scratch {
    path: PathBuf,
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn tempfile_dir() -> Result<Scratch, Refusal> {
    let base = std::env::temp_dir();
    for n in 0..100u32 {
        let path = base.join(format!("derive-offload.{}.{n}", std::process::id()));
        if std::fs::create_dir(&path).is_ok() {
            return Ok(Scratch { path });
        }
    }
    Err(refuse(
        EXIT_UNAVAILABLE,
        "no scratch directory could be made",
    ))
}
