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
/// The run was stopped with `majordomus offload stop` (I3956).
pub const EXIT_STOPPED: u8 = 14;

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
    /// Who the run belonged to, as it was exported into it and recorded on the host (I3956).
    pub owner: super::owner::RunOwner,
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
/// so the lock stays held exactly as long as this derive runs.
///
/// Everything the derive compiles stays in its own run directory, `<shared>-<tag>`, which
/// the traps remove however the derive ends. Pinning the target directory alone is not
/// that: the checkout declares a shared `build-dir`, so cargo kept every intermediate in the
/// host's `build-shared/majordomus`, outside what the traps remove, and on 2026-10-18 that
/// directory held 87 GB on lundra and the volume reached 0 bytes. The build directory is
/// pinned to the run directory too. Arguments: lock, worktrees
/// directory, tag, shared build directory, derive command, spool, PATH prefix
/// (colon-separated), the commit the derived files may observe (exported to the derive as
/// `MJ_DERIVE_OBSERVE`; empty leaves it unset), whether the caller holds standard input open
/// (`1`: its end means the caller is gone, and the run is stopped), the run-record directory,
/// the owner's session, peer, issue, host and pid ([`super::owner::RunOwner::args`]), then the
/// files to touch; [`derive_command`] builds them. Prints `pid <shell>`, `target <dir>`,
/// `group <pgid|none>`, then `nothing` or `derived <sha>`; exit 10 when the derive fails, 13
/// when the lock is not there, 14 when `offload stop` stopped it, 129 when the caller went
/// away.
///
/// Who owns the run (I3956): the owner is exported into every process the derive starts
/// (`MAJORDOMUS_OWNER_*`, and the run's own tag as `MAJORDOMUS_OWNER_RUN`) and written, with
/// the run's process group, into `<runs>/<tag>.run` — atomically, through a temporary file
/// and a rename — for as long as the run lives; every ending this shell sees removes it, so
/// a record that outlives its run is the evidence of a crash. The derive runs in a process
/// group of its own: `setsid`, or Python's `os.setsid` where a host has no `setsid`. A child
/// started with `&` by a non-interactive shell is never a group leader, so `setsid` makes
/// the new group in the derive's own process and its pid is the group's id. A run that is
/// stopped — its caller gone, a signal — is stopped as a group, `TERM` and then `KILL`, so no
/// `rustc` under it is left running with init as its parent.
const DERIVE: &str = r#"l=$1; w=$2; tag=$3; shared=$4; derive=$5; spool=$6; prefix=$7; observe=$8; held=$9
runs=${10}; os=${11}; op=${12}; oi=${13}; oh=${14}; opid=${15}; shift 15
[ -d "$l" ] || { echo "the lock $l is not held" >&2; exit 13; }
echo "pid $$"
: > "$l/$$"
case $w in /*) ;; *) w=$HOME/$w ;; esac
case $shared in /*) ;; *) shared=$HOME/$shared ;; esac
case $runs in /*) ;; *) runs=$HOME/$runs ;; esac
p=""; IFS=:; for e in $prefix; do case $e in /*) ;; *) e=$HOME/$e ;; esac; p="$p$e:"; done; unset IFS
export PATH="$p$PATH"
cd "$w/offload-$tag" || exit 13
private="$shared-$tag"; log="$spool/offload-$tag.log"; record="$runs/$tag.run"
mkdir -p "$shared" "$spool" "$runs"; rm -rf "$private"
if [ "$(uname)" = Darwin ]; then cp -cR "$shared" "$private" 2>/dev/null || { rm -rf "$private"; mkdir -p "$private"; }
else cp -a --reflink=always "$shared" "$private" 2>/dev/null || { rm -rf "$private"; mkdir -p "$private"; }; fi
child=""; pg=""; watch=""
ended() {
  [ -n "$watch" ] && kill "$watch" 2>/dev/null
  rm -f "$record" "$record.tmp" "$runs/$tag.stopped"; rm -rf "$private"
}
stopped() {
  if [ -n "$pg" ]; then
    kill -TERM -"$pg" 2>/dev/null; i=0
    while kill -0 -"$pg" 2>/dev/null && [ "$i" -lt 10 ]; do sleep 0.2; i=$((i+1)); done
    kill -KILL -"$pg" 2>/dev/null
  fi
  [ -n "$child" ] && kill -TERM "$child" 2>/dev/null
  ended
}
trap 'ended' EXIT
trap 'stopped; exit 129' HUP
trap 'stopped; exit 130' INT
trap 'stopped; exit 143' TERM
if [ "$held" = 1 ]; then
  main=$$
  exec 3<&0
  ( while IFS= read -r _; do :; done <&3; kill -HUP "$main" 2>/dev/null ) &
  watch=$!
  exec 3<&- 0</dev/null
fi
MAJORDOMUS_OWNER_RUN=$tag; export MAJORDOMUS_OWNER_RUN
for v in "MAJORDOMUS_OWNER_SESSION=$os" "MAJORDOMUS_OWNER_PEER=$op" "MAJORDOMUS_OWNER_ISSUE=$oi" \
  "MAJORDOMUS_OWNER_HOST=$oh" "MAJORDOMUS_OWNER_PID=$opid"; do
  case $v in *=) ;; *) export "$v" ;; esac
done
export CARGO_TARGET_DIR="$private"
export CARGO_BUILD_BUILD_DIR="$private"
[ -n "$observe" ] && { MJ_DERIVE_OBSERVE=$observe; export MJ_DERIVE_OBSERVE; }
echo "target $private"
for f in "$@"; do [ -e "$f" ] && touch "$f"; done
group='import os,sys; os.setsid(); os.execvp(sys.argv[1], sys.argv[1:])'
if command -v setsid >/dev/null 2>&1; then how=setsid
elif python3 -c 'import os' >/dev/null 2>&1; then how=python3
else how=; fi
case $how in
  setsid) env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN setsid "$derive" > "$log" 2>&1 < /dev/null & ;;
  python3) env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN python3 -c "$group" "$derive" > "$log" 2>&1 < /dev/null & ;;
  *) env -u MAJORDOMUS_SHARE -u MAJORDOMUS_BIN "$derive" > "$log" 2>&1 < /dev/null & ;;
esac
child=$!
[ -n "$how" ] && pg=$child
echo "group ${pg:-none}"
{ echo "majordomus-offload-run/v1"; echo "tag $tag"; echo "pid $$"; echo "pgid $pg"
  echo "started_at $(date +%s)"; echo "host $(hostname)"; echo "cwd $PWD"
  echo "owner_session $os"; echo "owner_peer $op"; echo "owner_issue $oi"
  echo "owner_host $oh"; echo "owner_pid $opid"; } > "$record.tmp" && mv -f "$record.tmp" "$record"
rc=0; wait "$child" || rc=$?
child=""
if [ "$rc" != 0 ] && [ -f "$runs/$tag.stopped" ]; then
  echo "the derive was stopped on $(hostname) by session $(cat "$runs/$tag.stopped")" >&2
  exit 14
fi
if [ "$rc" != 0 ]; then
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

/// What [`derive_command`] builds the host's derive from: every value the [`DERIVE`] script
/// takes, by name rather than by position.
#[derive(Debug, Clone, Copy)]
pub(crate) struct DeriveArgs<'a> {
    /// The queue's lock directory, held by the caller.
    pub lock: &'a str,
    /// The host's worktrees directory.
    pub worktrees: &'a str,
    /// The run's tag.
    pub tag: &'a str,
    /// The shared build directory the run's own is cloned from.
    pub shared: &'a str,
    /// The derive command.
    pub derive: &'a str,
    /// The spool directory.
    pub spool: &'a str,
    /// The PATH prefix, colon-separated.
    pub prefix: &'a str,
    /// The commit the derived files may observe; empty leaves it unset.
    pub observe: &'a str,
    /// Whether the caller holds the script's standard input open for as long as it runs.
    pub held: bool,
    /// The host's run-record directory ([`super::owner::runs_directory`]).
    pub runs: &'a str,
    /// The owner, as [`super::owner::RunOwner::args`] gives it.
    pub owner: &'a [String; 5],
    /// The files to touch before the derive.
    pub touch: &'a [String],
}

/// The host's derive as a script and its positional arguments, in the order [`DERIVE`]
/// reads them: the one place that order is written.
pub(crate) fn derive_command(a: &DeriveArgs) -> (&'static str, Vec<String>) {
    let mut args: Vec<String> = [
        a.lock,
        a.worktrees,
        a.tag,
        a.shared,
        a.derive,
        a.spool,
        a.prefix,
        a.observe,
        if a.held { "1" } else { "0" },
        a.runs,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    args.extend(a.owner.iter().cloned());
    args.extend(a.touch.iter().cloned());
    (DERIVE, args)
}

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

    // The commit the derived files may observe: the worktree's HEAD, never the input. The
    // input of a dirty tree is a `git stash create` snapshot and that of a merge in progress
    // a `commit-tree` commit; neither reaches origin, so a derived file that named one
    // (delivery.json's observed.commit) could not be checked anywhere else, and CI's
    // `generate-site-data --check` called it stale (#1759, I3836). Read before the input, so
    // a branch with no commit yet is refused here, by what it lacks, before anything is
    // snapshot.
    let observe = git(&worktree, &["rev-parse", "--verify", "HEAD"])
        .map_err(|_| {
            refuse(
                EXIT_PRECONDITION,
                "the worktree's branch has no commit yet: a derive observes HEAD, so commit first",
            )
        })?
        .trim()
        .to_string();
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
    let owner = super::owner::RunOwner::of_this_process(&worktree);
    let runs = super::owner::runs_directory(&d.queue.directory);
    let _ = writeln!(
        log,
        "derive-offload: {tag} runs for session {}, peer {}, issue {} (record {}:{runs}/{tag}.run; \
         stop it with `majordomus offload stop {tag} --host {}`)",
        owner.session.as_deref().unwrap_or("(unnamed)"),
        owner.peer.as_deref().unwrap_or("(none)"),
        owner.issue.as_deref().unwrap_or("(none)"),
        host.id,
        host.id
    );
    let (script, args) = derive_command(&DeriveArgs {
        lock: &d.queue.lock,
        worktrees: &d.remote.worktrees,
        tag: &tag,
        shared: &d.remote.target,
        derive: &d.remote.derive,
        spool,
        prefix: &prefix,
        observe: &observe,
        held: true,
        runs: &runs,
        owner: &owner.args(),
        touch: &d.remote.touch,
    });
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = remote
        .run_held(script, &args)
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
        Some(14) => {
            cleanup(true);
            return Err(refuse(
                EXIT_STOPPED,
                format!(
                    "the derive was stopped on {} with `offload stop`; its worktree is kept there",
                    host.id
                ),
            ));
        }
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
        owner,
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
    // The ref is this process's scratch and goes however the apply ends: a refused apply
    // used to leave it behind, and the next offload of the same tree (same tag) then fetched
    // a different derived commit (`stash create` stamps the time) onto it, which git refuses
    // as a non-fast-forward with nothing on stderr under -q. `+` overwrites whatever an
    // earlier process left, and the guard removes it on every path.
    let _derived_guard = ScratchRef {
        dir: &worktree,
        name: derived_ref.clone(),
    };
    git(
        &worktree,
        &[
            "fetch",
            "-q",
            &back.to_string_lossy(),
            &format!("+{derived_ref}:{derived_ref}"),
        ],
    )?;
    apply(&worktree, &input, &sha, &derived_ref, &mut report)?;
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
    apply_with(worktree, input, sha, derived_ref, report, git, git_ok)
}

/// [`apply`], running `git` and `git_ok` as given: the real ones in earnest, a test's own
/// failures in a test.
fn apply_with(
    worktree: &Path,
    input: &str,
    sha: &str,
    derived_ref: &str,
    report: &mut OffloadReport,
    git: impl Fn(&Path, &[&str]) -> Result<String, Refusal>,
    git_ok: impl Fn(&Path, &[&str]) -> bool,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> OffloadReport {
        OffloadReport {
            host: "h".into(),
            tag: "t".into(),
            input: "i".into(),
            derived: Some("d".into()),
            staged: Vec::new(),
            waited_seconds: 0,
            seconds: 0,
            probe: Vec::new(),
            owner: super::super::owner::RunOwner::default(),
        }
    }

    /// A git that answers `name-only` with `files` and fails the first argument named in
    /// `failing`.
    fn fake(
        files: &'static str,
        failing: &'static str,
    ) -> impl Fn(&Path, &[&str]) -> Result<String, Refusal> {
        move |_, args| {
            if args.contains(&failing) {
                return Err(refuse(EXIT_UNAVAILABLE, format!("git {failing} failed")));
            }
            Ok(if args.contains(&"--name-only") {
                files.to_string()
            } else {
                String::new()
            })
        }
    }

    fn applied(
        git: impl Fn(&Path, &[&str]) -> Result<String, Refusal>,
        matches: bool,
    ) -> (Result<(), Refusal>, OffloadReport) {
        let mut r = report();
        let out = apply_with(Path::new("."), "i", "s", "refs/x", &mut r, git, |_, _| {
            matches
        });
        (out, r)
    }

    #[test]
    fn a_derived_tree_is_staged_sorted_when_the_worktree_matches_it() {
        let (out, r) = applied(fake("b\na\n\n", "never"), true);
        assert!(out.is_ok());
        assert_eq!(r.staged, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn every_git_failure_and_a_mismatch_after_applying_is_a_refusal() {
        for failing in ["--name-only", "checkout"] {
            let (out, r) = applied(fake("a\n", failing), true);
            let e = out.expect_err(failing);
            assert_eq!(e.code, EXIT_UNAVAILABLE);
            assert!(e.reason.contains(failing), "{}", e.reason);
            assert!(r.staged.is_empty(), "nothing is staged on a failure");
        }
        let (out, r) = applied(fake("a\n", "never"), false);
        let e = out.expect_err("a worktree that does not match");
        assert_eq!(e.code, EXIT_UNAVAILABLE);
        assert!(
            e.reason.contains("does not match the derived tree"),
            "{}",
            e.reason
        );
        assert!(r.staged.is_empty());
    }

    /// A held derive whose caller goes away is stopped as a group (I3956): while it runs, the
    /// host's run record names its owner and its process group, the owner is in the derive's
    /// environment, and when the caller's end of standard input closes, every process of the
    /// group — a grandchild that would have outlived its parent included — is stopped and the
    /// record removed. Measured on lundra 2026-10-22: a `rustc` survived its cargo and shell.
    #[test]
    fn a_held_derive_whose_caller_goes_is_stopped_as_a_group_and_its_record_removed() {
        use std::io::Read;
        let root = tempfile::tempdir().expect("a temporary directory");
        let at = |p: &str| root.path().join(p);
        let tag = "held1";
        for d in ["lock", "wt/offload-held1", "target"] {
            std::fs::create_dir_all(at(d)).unwrap();
        }
        // the derive: a grandchild that sleeps, its pid, its group and the owner written
        // down, then waits
        let derive = at("derive");
        std::fs::write(
            &derive,
            format!(
                "#!/bin/sh\nsleep 300 &\necho $! > {gc}\n\
                 echo \"$$ $(ps -o pgid= -p $$ | tr -d ' ')\" > {group}\n\
                 printf '%s|%s|%s|%s\\n' \"$MAJORDOMUS_OWNER_SESSION\" \"$MAJORDOMUS_OWNER_PEER\" \"$MAJORDOMUS_OWNER_ISSUE\" \"$MAJORDOMUS_OWNER_RUN\" > {env}\n\
                 wait\n",
                gc = at("grandchild").display(),
                group = at("group").display(),
                env = at("owner-env").display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&derive, std::fs::Permissions::from_mode(0o755)).unwrap();
        let runs = at("runs");
        let owner = ["claude-code-s9", "p2", "I3956", "mac", "24025"].map(String::from);
        let (script, args) = derive_command(&DeriveArgs {
            lock: at("lock").to_str().unwrap(),
            worktrees: at("wt").to_str().unwrap(),
            tag,
            shared: at("target").to_str().unwrap(),
            derive: derive.to_str().unwrap(),
            spool: at("spool").to_str().unwrap(),
            prefix: "",
            observe: "",
            held: true,
            runs: runs.to_str().unwrap(),
            owner: &owner,
            touch: &[],
        });
        let mut child = std::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .arg("sh")
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("sh runs");
        let record = runs.join(format!("{tag}.run"));
        let deadline = Instant::now() + Duration::from_secs(30);
        while !(record.exists()
            && at("grandchild").exists()
            && at("owner-env").exists()
            && at("group").exists())
        {
            assert!(Instant::now() < deadline, "the run never recorded itself");
            std::thread::sleep(Duration::from_millis(50));
        }
        let text = std::fs::read_to_string(&record).unwrap();
        let r = crate::offload::owner::RunRecord::parse(&text).expect(&text);
        assert_eq!(r.tag, tag);
        assert_eq!(r.owner.session.as_deref(), Some("claude-code-s9"));
        assert_eq!(
            (r.owner.peer.as_deref(), r.owner.pid),
            (Some("p2"), Some(24025))
        );
        assert!(r.cwd.ends_with("offload-held1"), "{text}");
        assert!(r.started_at > 0 && !r.host.is_empty(), "{text}");
        assert_eq!(r.owner.issue.as_deref(), Some("I3956"));
        let seen = std::fs::read_to_string(at("owner-env")).unwrap();
        assert_eq!(
            seen, "claude-code-s9|p2|I3956|held1\n",
            "the owner is in the derive's environment"
        );
        // the derive leads a process group of its own, and the record names that group
        let group = std::fs::read_to_string(at("group")).unwrap();
        let (pid, pgid) = group.trim().split_once(' ').expect(&group);
        if let Some(recorded) = r.pgid {
            assert_eq!(pid, pgid, "the derive is not its group's leader: {group}");
            assert_eq!(pgid, recorded.to_string(), "{text}");
        }
        let grandchild: i32 = std::fs::read_to_string(at("grandchild"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        // the caller goes away: its end of standard input closes
        drop(child.stdin.take());
        let status = child.wait().expect("the run ends");
        let mut out = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut out)
            .unwrap();
        assert_eq!(
            status.code(),
            Some(129),
            "a run whose caller went is stopped: {out}"
        );
        assert!(!record.exists(), "the record ends with the run");
        assert!(!at("target-held1").exists(), "the run directory is removed");
        if let Some(pgid) = r.pgid {
            let deadline = Instant::now() + Duration::from_secs(10);
            // SAFETY: kill with signal 0 only asks whether the pid exists
            while unsafe { libc::kill(grandchild, 0) } == 0 {
                assert!(
                    Instant::now() < deadline,
                    "the grandchild outlived its group: {out}"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            assert!(out.contains(&format!("group {pgid}")), "{out}");
        } else {
            // a host with neither setsid nor Python has no group to stop; say so and clean up
            // SAFETY: the pid was written by this test's own derive a moment ago
            unsafe { libc::kill(grandchild, libc::SIGKILL) };
            assert!(out.contains("group none"), "{out}");
        }
    }

    /// The remote derive, run here exactly as a host runs it: whatever build directory the
    /// caller's environment names, the derive compiles inside its own run directory, and
    /// that directory is gone afterwards, whether the derive succeeds or fails. The shared
    /// directory it was seeded from is left as it was.
    #[test]
    fn a_remote_derive_builds_only_inside_its_run_directory_and_removes_it() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let at = |p: &str| root.path().join(p);
        let decoy = at("build-shared");
        std::fs::create_dir_all(&decoy).unwrap();
        for (tag, outcome) in [("ok1", 0), ("bad1", 1)] {
            let lock = at(&format!("lock-{tag}"));
            std::fs::create_dir_all(&lock).unwrap();
            let tree = at(&format!("wt/offload-{tag}"));
            std::fs::create_dir_all(&tree).unwrap();
            let git = |args: &[&str]| {
                let out = std::process::Command::new("git")
                    .current_dir(&tree)
                    .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                    .args(args)
                    .output()
                    .expect("git runs");
                assert!(out.status.success(), "git {args:?}: {out:?}");
            };
            git(&["init", "-q"]);
            std::fs::write(tree.join("a"), "a").unwrap();
            git(&["add", "-A"]);
            git(&["commit", "-q", "-m", "a"]);
            git(&["update-ref", &format!("refs/offload/{tag}"), "HEAD"]);
            // the derive: says where it was told to build, compiles something there, and
            // succeeds or fails as the case asks
            let derive = at(&format!("derive-{tag}"));
            std::fs::write(
                &derive,
                format!(
                    "#!/bin/sh\nprintf '%s\\n%s\\n' \"$CARGO_BUILD_BUILD_DIR\" \"$CARGO_TARGET_DIR\" > {seen}\n\
                     printf '%s %s\\n' \"$MAJORDOMUS_OWNER_SESSION\" \"$MAJORDOMUS_OWNER_ISSUE\" > {seen}-owner\n\
                     mkdir -p \"$CARGO_BUILD_BUILD_DIR/debug/deps\" && : > \"$CARGO_BUILD_BUILD_DIR/debug/deps/x\"\n\
                     exit {outcome}\n",
                    seen = at(&format!("seen-{tag}")).display()
                ),
            )
            .unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&derive, std::fs::Permissions::from_mode(0o755)).unwrap();
            let shared = at("target");
            let (worktrees, spool, runs) = (at("wt"), at("spool"), at("runs"));
            let owner = ["claude-code-s1", "", "I3956", "mac", ""].map(String::from);
            let (script, args) = derive_command(&DeriveArgs {
                lock: lock.to_str().unwrap(),
                worktrees: worktrees.to_str().unwrap(),
                tag,
                shared: shared.to_str().unwrap(),
                derive: derive.to_str().unwrap(),
                spool: spool.to_str().unwrap(),
                prefix: "",
                observe: "",
                held: false,
                runs: runs.to_str().unwrap(),
                owner: &owner,
                touch: &[],
            });
            let out = std::process::Command::new("sh")
                .arg("-c")
                .arg(script)
                .arg("sh")
                .args(&args)
                .env("CARGO_BUILD_BUILD_DIR", &decoy)
                .output()
                .expect("sh runs");
            let private = format!("{}-{tag}", shared.display());
            let seen = std::fs::read_to_string(at(&format!("seen-{tag}"))).expect("the derive ran");
            assert_eq!(
                seen,
                format!("{private}\n{private}\n"),
                "{tag}: the build and target directories are the run's own"
            );
            assert!(
                !std::path::Path::new(&private).exists(),
                "{tag}: the run directory is removed: {out:?}"
            );
            assert_eq!(
                std::fs::read_dir(&decoy).unwrap().count(),
                0,
                "{tag}: nothing lands in the caller's build directory"
            );
            assert!(shared.is_dir(), "{tag}: the shared seed stays");
            let owner = std::fs::read_to_string(at(&format!("seen-{tag}-owner"))).unwrap();
            assert_eq!(
                owner, "claude-code-s1 I3956\n",
                "{tag}: the derive sees its owner"
            );
            assert!(
                !runs.join(format!("{tag}.run")).exists(),
                "{tag}: the run record ends with the run"
            );
            let code = out.status.code();
            if outcome == 0 {
                assert_eq!(code, Some(0), "{out:?}");
            } else {
                assert_eq!(code, Some(i32::from(EXIT_DERIVE_FAILED)), "{out:?}");
            }
        }
    }
}
