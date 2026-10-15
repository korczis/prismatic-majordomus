//! The derive offload against hosts that are directories on this machine.
//!
//! The queue's own tests, with real holder processes on this machine, stand beside it in
//! `src/offload/queue.rs`. Here the command is driven end to end through the built
//! executable, with
//! `MAJORDOMUS_OFFLOAD_SSH` pointing at a stand-in that runs the remote command locally under
//! a fake host's home: every byte that would cross ssh crosses a process boundary here
//! instead, and the suite needs no network. A host's free space is what its stand-in `df`
//! reports, and a host that is "down" refuses the connection the way ssh does (exit 255).
//!
//! The host's derive is a stand-in `scripts/derive` committed in the fixture repository: it
//! builds an "executable" into `$CARGO_TARGET_DIR`, can wait for the test to build into the
//! shared directory meanwhile, and writes derived files that say what it saw (or nothing at
//! all, when the tree carries a `NOOP` file).
//!
//! Every step that can fail is made to fail, one at a time: the ssh stand-in fails the remote
//! command carrying `$OFFLOAD_SSH_FAIL`, or removes itself after the one carrying
//! `$OFFLOAD_SSH_VANISH`; a git stand-in put first on the session's `PATH` fails the
//! invocation carrying `$OFFLOAD_GIT_FAIL`. Each refusal is asserted by its exit code and its
//! words.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

// ------------------------------------------------------------------------- the command line

/// A cluster on disk: an origin, a session's clone, and hosts with homes, a stand-in `df`,
/// and a checkout of their own.
struct Cluster {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

const SSH_SHIM: &str = r#"#!/bin/sh
# a local stand-in for ssh: options, then user@host, then the command
while [ $# -gt 0 ]; do
  case $1 in
    -o|-i) shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
addr=$1; shift
h="$OFFLOAD_FAKE_HOSTS/$addr"
if [ ! -d "$h" ] || [ -e "$h/down" ]; then
  echo "ssh: connect to host $addr port 22: Connection refused" >&2
  exit 255
fi
cd "$h/home" || exit 255
# a test fails the one remote command carrying $OFFLOAD_SSH_FAIL, with its exit code
if [ -n "$OFFLOAD_SSH_FAIL" ]; then
  case "$*" in *"$OFFLOAD_SSH_FAIL"*)
    echo "ssh: injected failure" >&2; exit "${OFFLOAD_SSH_FAIL_CODE:-1}" ;;
  esac
fi
# or lets it run and then removes this stand-in: the next command cannot be started at all
if [ -n "$OFFLOAD_SSH_VANISH" ]; then
  case "$*" in *"$OFFLOAD_SSH_VANISH"*)
    env HOME="$h/home" PATH="$h/bin:$PATH" sh -c "$*"; s=$?; rm -f "$0"; exit $s ;;
  esac
fi
exec env HOME="$h/home" PATH="$h/bin:$PATH" sh -c "$*"
"#;

/// A stand-in for git on the session's side: it fails the one invocation whose arguments
/// carry `$OFFLOAD_GIT_FAIL` and hands every other one to the real git.
const GIT_SHIM: &str = r#"#!/bin/sh
if [ -n "$OFFLOAD_GIT_FAIL" ]; then
  case " $* " in *"$OFFLOAD_GIT_FAIL"*)
    echo "git: injected failure of: $*" >&2; exit 1 ;;
  esac
fi
exec "$OFFLOAD_REAL_GIT" "$@"
"#;

const DF_SHIM: &str = r#"#!/bin/sh
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "fake 1 1 $(cat "$(dirname "$0")/../free_kib") 1% /"
"#;

/// The derive a host runs. It builds into $CARGO_TARGET_DIR, optionally waits while the test
/// builds into the shared directory, then writes what it saw.
const FAKE_DERIVE: &str = r#"#!/bin/sh
set -e
if [ -f NOOP ]; then echo "==> derive: done"; exit 0; fi
mkdir -p "$CARGO_TARGET_DIR/debug"
printf 'built from %s\n' "$(git rev-parse HEAD)" > "$CARGO_TARGET_DIR/debug/majordomus"
if [ -n "$OFFLOAD_SIGNAL" ]; then
  : > "$OFFLOAD_SIGNAL/started"
  i=0
  while [ ! -e "$OFFLOAD_SIGNAL/go" ] && [ $i -lt 400 ]; do sleep 0.05; i=$((i+1)); done
fi
cp "$CARGO_TARGET_DIR/debug/majordomus" exe-seen.txt
printf 'derived from: %s\n' "$(cat a.txt)" > generated.txt
git rev-list --parents -n 1 HEAD | wc -w | tr -d ' ' > parents.txt
if [ -f DELETE ]; then rm -f a.txt; fi
if [ -f FAIL ]; then echo "error: the derive broke"; exit 1; fi
echo "==> derive: done"
"#;

fn exec(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Cluster {
    fn new(hosts: &[(&str, u64)]) -> Cluster {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        std::fs::write(root.join("gitconfig"), "[init]\n\tdefaultBranch = master\n").unwrap();
        let c = Cluster { _tmp: tmp, root };
        std::fs::create_dir_all(c.root.join("bin")).unwrap();
        exec(&c.root.join("bin/ssh"), SSH_SHIM);
        std::fs::create_dir_all(c.root.join("gitbin")).unwrap();
        exec(&c.root.join("gitbin/git"), GIT_SHIM);
        let seed = c.root.join("seed");
        std::fs::create_dir_all(seed.join("scripts")).unwrap();
        std::fs::create_dir_all(seed.join("apps/majordomus-cli/src")).unwrap();
        exec(&seed.join("scripts/derive"), FAKE_DERIVE);
        std::fs::write(seed.join("a.txt"), "one\n").unwrap();
        std::fs::write(seed.join("apps/majordomus-cli/src/lib.rs"), "").unwrap();
        c.git(&c.root, &["init", "-q", "--bare", "origin.git"]);
        c.git(&seed, &["init", "-q"]);
        c.git(&seed, &["add", "-A"]);
        c.git(&seed, &["commit", "-q", "-m", "seed"]);
        c.git(
            &seed,
            &[
                "push",
                "-q",
                &c.root.join("origin.git").to_string_lossy(),
                "master",
            ],
        );
        c.git(&c.root, &["clone", "-q", "origin.git", "session"]);
        for (addr, free_gib) in hosts {
            let h = c.root.join("hosts").join(addr);
            std::fs::create_dir_all(h.join("bin")).unwrap();
            std::fs::create_dir_all(h.join("home/dev")).unwrap();
            exec(&h.join("bin/df"), DF_SHIM);
            std::fs::write(h.join("free_kib"), (free_gib * 1_048_576).to_string()).unwrap();
            c.git(
                &h.join("home/dev"),
                &[
                    "clone",
                    "-q",
                    &c.root.join("origin.git").to_string_lossy(),
                    "prismatic-majordomus",
                ],
            );
        }
        c
    }

    fn env(&self, cmd: &mut Command) {
        cmd.env("GIT_CONFIG_GLOBAL", self.root.join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@invalid")
            .env("MAJORDOMUS_OFFLOAD_SSH", self.root.join("bin/ssh"))
            .env("OFFLOAD_FAKE_HOSTS", self.root.join("hosts"))
            .env_remove("OFFLOAD_SIGNAL");
    }

    fn git(&self, dir: &Path, args: &[&str]) -> String {
        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(dir).args(args);
        self.env(&mut cmd);
        let out = cmd.output().unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn session(&self) -> PathBuf {
        self.root.join("session")
    }

    fn home(&self, addr: &str) -> PathBuf {
        self.root.join("hosts").join(addr).join("home")
    }

    /// A declaration naming these hosts, every host-side path inside the cluster.
    fn declaration(&self, hosts: &[&str]) -> PathBuf {
        self.declaration_waiting(hosts, 60)
    }

    /// The same, with the queue's declared wait.
    fn declaration_waiting(&self, hosts: &[&str], wait_max_seconds: u64) -> PathBuf {
        let mut y = format!(
            "schema: derive-offload/v1\nkind: derive-offload\nid: test\nmin_free_gib: 10\nqueue:\n  directory: {q}\n  lock: {l}\n  poll_seconds: 1\n  wait_max_seconds: {wait_max_seconds}\nremote:\n  spool: {s}\nhosts:\n",
            q = self.root.join("queue").display(),
            l = self.root.join("lock").display(),
            s = self.root.join("spool").display(),
        );
        for (i, h) in hosts.iter().enumerate() {
            y.push_str(&format!("  - id: h{i}\n    address: {h}\n"));
        }
        let p = self.root.join("derive.yaml");
        std::fs::write(&p, y).unwrap();
        p
    }

    fn offload(&self, args: &[&str], signal: Option<&Path>) -> Output {
        let mut cmd = common::majordomus();
        cmd.arg("offload").args(args).current_dir(self.session());
        self.env(&mut cmd);
        if let Some(s) = signal {
            cmd.env("OFFLOAD_SIGNAL", s);
        }
        cmd.output().unwrap()
    }

    /// `offload derive .` with these extra arguments and this environment.
    fn derive(&self, decl: &Path, extra: &[&str], env: &[(&str, String)]) -> Output {
        let mut cmd = common::majordomus();
        cmd.args(["offload", "derive", ".", "--declaration"])
            .arg(decl)
            .args(extra)
            .current_dir(self.session());
        self.env(&mut cmd);
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    /// The environment that fails the session's git invocation carrying `what`.
    fn failing_git(&self, what: &str) -> Vec<(&'static str, String)> {
        let real = Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        vec![
            (
                "PATH",
                format!(
                    "{}:{}",
                    self.root.join("gitbin").display(),
                    std::env::var("PATH").unwrap()
                ),
            ),
            ("OFFLOAD_REAL_GIT", text(&real.stdout).trim().to_string()),
            ("OFFLOAD_GIT_FAIL", what.to_string()),
        ]
    }

    /// `failing_git` for a git invocation in the session's worktree: `-C <session> <what>`.
    fn failing_session_git(&self, what: &str) -> Vec<(&'static str, String)> {
        self.failing_git(&format!("-C {} {what}", self.session().display()))
    }
}

/// The exit code and the standard error of a refused offload, the code asserted.
fn refused(out: &Output, code: i32) -> String {
    let err = text(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(code),
        "stdout: {}\nstderr: {err}",
        text(&out.stdout)
    );
    err
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// Of four hosts, one refuses the connection and one is under the free-space minimum; the
/// two left are equally idle, and the one declared first is chosen.
#[test]
fn the_probe_skips_a_host_that_is_down_or_short_of_space() {
    let c = Cluster::new(&[
        ("u@down", 50),
        ("u@full", 5),
        ("u@roomy", 50),
        ("u@roomier", 80),
    ]);
    std::fs::write(c.root.join("hosts/u@down/down"), "").unwrap();
    let d = c.declaration(&["u@down", "u@full", "u@roomy", "u@roomier"]);
    let out = c.offload(
        &[
            "probe",
            "--format",
            "json",
            "--declaration",
            d.to_str().unwrap(),
        ],
        None,
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["chosen"], "h2");
    assert_eq!(v["hosts"][0]["standing"], "unreachable");
    assert_eq!(v["hosts"][1]["standing"], "under_minimum");
    assert_eq!(v["hosts"][1]["free_gib"], 5);
    assert_eq!(v["hosts"][2]["standing"], "eligible");
    assert_eq!(v["hosts"][3]["free_gib"], 80);

    std::fs::write(c.root.join("hosts/u@roomy/free_kib"), "0").unwrap();
    std::fs::write(c.root.join("hosts/u@roomier/free_kib"), "0").unwrap();
    let out = c.offload(&["probe", "--declaration", d.to_str().unwrap()], None);
    assert_eq!(
        out.status.code(),
        Some(12),
        "no eligible host is not a success"
    );
    assert!(text(&out.stdout).contains("no declared host can take a derive"));
}

/// The round trip: the uncommitted change travels, the host derives it, and the derived
/// files come back staged — nothing committed, nothing left on the host.
#[test]
fn a_derive_comes_back_staged_and_uncommitted() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    std::fs::write(s.join("a.txt"), "two\n").unwrap(); // authored, not committed
    let head = c.git(&s, &["rev-parse", "HEAD"]);

    let out = c.offload(
        &[
            "derive",
            ".",
            "--format",
            "json",
            "--declaration",
            d.to_str().unwrap(),
        ],
        None,
    );
    assert!(
        out.status.success(),
        "{}\n{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["host"], "h0");
    let staged = c.git(&s, &["diff", "--cached", "--name-only"]);
    for f in ["generated.txt", "exe-seen.txt", "parents.txt"] {
        assert!(
            staged.lines().any(|l| l == f),
            "{f} is not staged: {staged}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(s.join("generated.txt")).unwrap(),
        "derived from: two\n",
        "the derive did not see the uncommitted change"
    );
    assert_eq!(
        c.git(&s, &["rev-parse", "HEAD"]),
        head,
        "something was committed"
    );
    assert!(
        !staged.lines().any(|l| l == "a.txt"),
        "the authored change was staged for the author"
    );
    assert!(
        c.git(&s, &["for-each-ref", "refs/offload"]).is_empty(),
        "a scratch ref was left behind"
    );

    let home = c.home("u@lundra");
    assert!(
        !home
            .join("dev/prismatic-majordomus-wt")
            .join(format!("offload-{}", report["tag"].as_str().unwrap()))
            .exists(),
        "the host's worktree was kept"
    );
    let cache: Vec<_> = std::fs::read_dir(home.join(".cache"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(
        cache,
        ["majordomus-offload-target"],
        "the derive's private build directory was left on the host"
    );
    assert!(
        !c.root.join("lock").exists(),
        "the host's lock was left held"
    );
    assert_eq!(
        std::fs::read_dir(c.root.join("queue")).unwrap().count(),
        0,
        "a ticket was left in the queue"
    );
    let spool: Vec<_> = std::fs::read_dir(c.root.join("spool"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(
        spool.iter().all(|f| f.ends_with(".log")),
        "bundles were left in the spool: {spool:?}"
    );
}

/// While the derive runs, another build writes into the shared build directory — the 00:25Z
/// failure on the second MacBook. The derive's executable is untouched: it built into a
/// directory of its own. (Were CARGO_TARGET_DIR the shared directory, exe-seen.txt would
/// read "replaced".)
#[test]
fn a_concurrent_build_cannot_replace_the_derives_executable() {
    let c = Cluster::new(&[("u@macbook", 50)]);
    let d = c.declaration(&["u@macbook"]);
    let shared = c.home("u@macbook").join(".cache/majordomus-offload-target");
    std::fs::create_dir_all(shared.join("debug")).unwrap();
    std::fs::write(
        shared.join("debug/majordomus"),
        "warm from an earlier build\n",
    )
    .unwrap();
    let signal = c.root.join("signal");
    std::fs::create_dir_all(&signal).unwrap();

    let out = std::thread::scope(|s| {
        let run = s.spawn(|| {
            c.offload(
                &["derive", ".", "--declaration", d.to_str().unwrap()],
                Some(&signal),
            )
        });
        let started = Instant::now();
        while !signal.join("started").exists() {
            if run.is_finished() {
                let out = run.join().unwrap();
                panic!(
                    "the offload ended before its derive started:\n{}",
                    text(&out.stderr)
                );
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "the derive never started"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        // the build that never took the host's lock
        std::fs::write(shared.join("debug/majordomus"), "replaced\n").unwrap();
        std::fs::write(signal.join("go"), "").unwrap();
        run.join().unwrap()
    });
    assert!(out.status.success(), "{}", text(&out.stderr));
    let seen = std::fs::read_to_string(c.session().join("exe-seen.txt")).unwrap();
    assert!(
        seen.starts_with("built from "),
        "the derive's executable was replaced under it: {seen:?}"
    );
    assert_eq!(
        std::fs::read_to_string(shared.join("debug/majordomus")).unwrap(),
        "replaced\n",
        "the derive wrote into the shared directory"
    );
}

/// A merge in progress, resolved and staged, travels as a commit with both parents, and the
/// derived files come back staged into the merge, which is still in progress.
#[test]
fn a_merge_in_progress_travels_with_both_parents() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    c.git(&s, &["checkout", "-q", "-b", "topic"]);
    std::fs::write(s.join("a.txt"), "topic\n").unwrap();
    c.git(&s, &["commit", "-q", "-am", "topic"]);
    c.git(&s, &["checkout", "-q", "master"]);
    std::fs::write(s.join("a.txt"), "master\n").unwrap();
    c.git(&s, &["commit", "-q", "-am", "master side"]);
    c.git(&s, &["push", "-q", "origin", "master"]);
    let mut merge = Command::new("git");
    merge.arg("-C").arg(&s).args(["merge", "-q", "topic"]);
    c.env(&mut merge);
    assert!(
        !merge.output().unwrap().status.success(),
        "the merge was meant to conflict"
    );

    let unresolved = c.offload(&["derive", ".", "--declaration", d.to_str().unwrap()], None);
    assert_eq!(
        unresolved.status.code(),
        Some(2),
        "{}",
        text(&unresolved.stderr)
    );
    assert!(text(&unresolved.stderr).contains("unresolved paths"));

    std::fs::write(s.join("a.txt"), "both\n").unwrap();
    c.git(&s, &["add", "a.txt"]);
    let out = c.offload(&["derive", ".", "--declaration", d.to_str().unwrap()], None);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(
        std::fs::read_to_string(s.join("parents.txt"))
            .unwrap()
            .trim(),
        "3",
        "the input lost its second parent"
    );
    assert!(
        s.join(".git/MERGE_HEAD").exists(),
        "the merge was concluded for the author"
    );
}

/// What would travel wrong is refused before anything travels: an untracked file. What
/// comes back wrong is refused before anything is applied: a deleted tracked file. A derive
/// that fails says so with its own exit code and keeps the host's worktree.
#[test]
fn what_cannot_be_derived_faithfully_is_refused() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    let decl = d.to_str().unwrap();

    std::fs::write(s.join("new.txt"), "untracked\n").unwrap();
    let out = c.offload(&["derive", ".", "--declaration", decl], None);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("untracked files would not travel")
            && text(&out.stderr).contains("new.txt")
    );
    assert!(
        !c.root.join("spool").exists(),
        "something travelled before the refusal"
    );
    std::fs::remove_file(s.join("new.txt")).unwrap();

    std::fs::write(s.join("DELETE"), "").unwrap();
    c.git(&s, &["add", "DELETE"]);
    let out = c.offload(&["derive", ".", "--declaration", decl], None);
    assert_eq!(out.status.code(), Some(11), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("deleted tracked files") && text(&out.stderr).contains("a.txt")
    );
    assert!(
        !s.join("generated.txt").exists(),
        "a refused result was half-applied"
    );
    assert!(s.join("a.txt").exists());
    c.git(&s, &["rm", "-q", "--cached", "DELETE"]);
    std::fs::remove_file(s.join("DELETE")).unwrap();

    std::fs::write(s.join("FAIL"), "").unwrap();
    c.git(&s, &["add", "FAIL"]);
    let out = c.offload(&["derive", ".", "--declaration", decl], None);
    assert_eq!(out.status.code(), Some(10), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("the derive broke"),
        "{}",
        text(&out.stderr)
    );
    assert!(
        !c.root.join("lock").exists(),
        "a failed derive kept the host's lock"
    );

    let named = c.offload(
        &["derive", ".", "--host", "nowhere", "--declaration", decl],
        None,
    );
    assert_eq!(named.status.code(), Some(12));
    assert!(text(&named.stderr).contains("'nowhere' is not a declared host"));
}

fn env(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs.iter().map(|(k, v)| (*k, v.to_string())).collect()
}

/// Before anything travels, the worktree is read and the input commit written; a worktree
/// that is not there, is not a repository, or that git cannot read or write is refused, and
/// no scratch ref is left behind.
#[test]
fn a_worktree_git_cannot_read_is_refused_before_anything_travels() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    let decl = d.to_str().unwrap();

    let out = c.offload(&["derive", "no-such-dir", "--declaration", decl], None);
    assert!(refused(&out, 2).contains("no-such-dir: "));

    let plain = c.root.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let out = c.offload(
        &["derive", plain.to_str().unwrap(), "--declaration", decl],
        None,
    );
    assert!(refused(&out, 12).contains("git rev-parse --show-toplevel failed: "));

    let out = c.derive(
        &d,
        &[],
        &env(&[("PATH", c.root.join("bin").to_str().unwrap())]),
    );
    assert!(refused(&out, 12).contains("git could not be run: "));

    // a clean worktree travels as HEAD
    let out = c.derive(
        &d,
        &["--host", "h0"],
        &c.failing_session_git("rev-parse HEAD "),
    );
    assert!(refused(&out, 12).contains("git rev-parse HEAD failed: git: injected failure"));

    std::fs::write(s.join("a.txt"), "two\n").unwrap();
    for what in [
        "rev-parse --path-format=absolute --git-path MERGE_HEAD",
        "status --porcelain",
        "-c user.name=offload -c user.email=offload@invalid stash create",
        "update-ref refs/offload/in-",
        "bundle create",
    ] {
        let out = c.derive(&d, &["--host", "h0"], &c.failing_session_git(what));
        let err = refused(&out, 12);
        assert!(
            err.contains("failed: git: injected failure of: "),
            "{what}: {err}"
        );
        assert!(
            c.git(&s, &["for-each-ref", "refs/offload"]).is_empty(),
            "{what}: a scratch ref was left behind"
        );
    }

    let out = c.derive(
        &d,
        &["--host", "h0"],
        &env(&[("TMPDIR", c.root.join("no-tmp").to_str().unwrap())]),
    );
    assert!(refused(&out, 12).contains("no scratch directory could be made"));
    assert!(
        c.git(&s, &["for-each-ref", "refs/offload"]).is_empty(),
        "the input ref outlived a refusal"
    );
    assert!(
        !c.root.join("spool").exists(),
        "something travelled before the refusal"
    );
}

/// A merge in progress: each git step of writing its two-parent input can fail and says so,
/// and a resolved merge with unstaged changes is refused, because the staged tree travels.
#[test]
fn a_merge_input_that_cannot_be_written_is_refused() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    c.git(&s, &["checkout", "-q", "-b", "topic"]);
    std::fs::write(s.join("a.txt"), "topic\n").unwrap();
    c.git(&s, &["commit", "-q", "-am", "topic"]);
    c.git(&s, &["checkout", "-q", "master"]);
    std::fs::write(s.join("a.txt"), "master\n").unwrap();
    c.git(&s, &["commit", "-q", "-am", "master side"]);
    let mut merge = Command::new("git");
    merge.arg("-C").arg(&s).args(["merge", "-q", "topic"]);
    c.env(&mut merge);
    assert!(!merge.output().unwrap().status.success());

    let out = c.derive(&d, &[], &c.failing_git("--diff-filter=U"));
    assert!(refused(&out, 12).contains("git diff --name-only --diff-filter=U failed"));

    std::fs::write(s.join("a.txt"), "both\n").unwrap();
    c.git(&s, &["add", "a.txt"]);
    std::fs::write(s.join("a.txt"), "both, and more\n").unwrap();
    let out = c.derive(&d, &[], &[]);
    assert!(refused(&out, 2).contains("the merge has unstaged changes"));
    c.git(&s, &["add", "a.txt"]);

    for (what, step) in [
        ("write-tree", "git write-tree failed"),
        ("rev-parse MERGE_HEAD", "git rev-parse MERGE_HEAD failed"),
        ("commit-tree", "commit-tree"),
    ] {
        let out = c.derive(&d, &[], &c.failing_git(what));
        let err = refused(&out, 12);
        assert!(
            err.contains(step) && err.contains("injected failure"),
            "{what}: {err}"
        );
    }
    assert!(
        s.join(".git/MERGE_HEAD").exists(),
        "a refusal concluded the merge"
    );
}

/// Each remote step of the round trip — the probe, the upload, the preparation, the holder,
/// the ticket, the wait, the derive, the download — can fail, and each says which step and
/// on which host, with the exit code of its kind. The next derive is not blocked by what a
/// failed one left on the host.
#[test]
fn a_host_step_that_fails_is_named_with_its_host() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    let ssh = c.root.join("bin/ssh");
    let nowhere = c.root.join("bin/no-such-ssh");
    let nowhere = nowhere.to_str().unwrap();

    // clean: nothing to bundle, so the preparation is the first remote command
    let out = c.derive(
        &d,
        &["--host", "h0"],
        &env(&[("MAJORDOMUS_OFFLOAD_SSH", nowhere)]),
    );
    let err = refused(&out, 12);
    assert!(err.contains("offload derive: h0: "), "{err}");

    std::fs::write(s.join("a.txt"), "two\n").unwrap();
    let out = c.derive(&d, &[], &env(&[("MAJORDOMUS_OFFLOAD_SSH", nowhere)]));
    let err = refused(&out, 12);
    assert!(
        err.contains("derive-offload: h0 skipped: ")
            && err.contains("no declared host answers with its free-space minimum free"),
        "{err}"
    );
    let out = c.derive(
        &d,
        &["--host", "h0"],
        &env(&[("MAJORDOMUS_OFFLOAD_SSH", nowhere)]),
    );
    assert!(refused(&out, 12).contains("h0: the bundle could not be sent: "));

    for (marker, code, words) in [
        (
            "&& cat >",
            "1",
            "h0: the bundle could not be sent: ssh: injected failure",
        ),
        (
            "git fetch -q origin",
            "1",
            "h0: the host could not prepare the worktree: ssh: injected failure",
        ),
        (
            "exec cat",
            "1",
            "h0: no holder process could be started: the host's holder did not print its pid",
        ),
        (
            "&& : >",
            "1",
            "the ticket could not be written: ssh: injected failure",
        ),
        (
            "free ${f",
            "1",
            "h0 did not answer the observation (exit Some(1)): ssh: injected failure",
        ),
        (
            "is not held",
            "13",
            "h0: the derive step ended with Some(13): ssh: injected failure",
        ),
        (
            "-c 'cat ",
            "1",
            "h0: the derived bundle could not be fetched: ssh: injected failure",
        ),
    ] {
        let out = c.derive(
            &d,
            &["--host", "h0"],
            &env(&[
                ("OFFLOAD_SSH_FAIL", marker),
                ("OFFLOAD_SSH_FAIL_CODE", code),
            ]),
        );
        let err = refused(&out, 12);
        assert!(err.contains(words), "{marker}: {err}");
        assert!(
            !s.join("generated.txt").exists(),
            "{marker}: a failed round trip staged something"
        );
    }

    // a lock nobody can check is waited for until the declared wait runs out
    std::fs::create_dir(c.root.join("lock")).unwrap();
    let short = c.declaration_waiting(&["u@lundra"], 1);
    let out = c.derive(&short, &["--host", "h0"], &[]);
    assert!(refused(&out, 75).contains("gave up after "));
    std::fs::remove_dir(c.root.join("lock")).unwrap();

    // the host is gone once the lock is taken, then once the derive has run: each leaves a
    // lock whose holders are dead, which the next derive reclaims
    for marker in ["|| exit 3; :", "is not held"] {
        let out = c.derive(
            &d,
            &["--host", "h0"],
            &env(&[("OFFLOAD_SSH_VANISH", marker)]),
        );
        let err = refused(&out, 12);
        assert!(err.contains("offload derive: h0: "), "{marker}: {err}");
        assert!(!ssh.exists(), "{marker}: the stand-in did not vanish");
        exec(&ssh, SSH_SHIM);
    }

    // and after all of that the host still derives
    let out = c.derive(&d, &["--host", "h0"], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("reclaimed the lock"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(s.join("generated.txt")).unwrap(),
        "derived from: two\n"
    );
}

/// What comes back is checked and applied by git in the session; each of those steps can
/// fail, and a worktree that does not match the derived tree after applying is refused.
#[test]
fn a_derived_tree_that_cannot_be_applied_is_refused() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    std::fs::write(s.join("a.txt"), "two\n").unwrap();
    for (what, words) in [
        (
            "/back.bundle +refs/offload/",
            "failed: git: injected failure",
        ),
        ("diff --name-status", "git diff --name-status"),
        ("diff --name-only ", "git diff --name-only"),
        ("checkout ", "git checkout"),
        (
            "diff --quiet ",
            "the worktree does not match the derived tree after applying it",
        ),
    ] {
        let env = if what.starts_with('/') {
            c.failing_git(what)
        } else {
            c.failing_session_git(what)
        };
        let out = c.derive(&d, &["--host", "h0"], &env);
        let err = refused(&out, 12);
        assert!(err.contains(words), "{what}: {err}");
    }
}

/// A refused apply leaves no scratch ref behind, and the next offload of the same tree takes
/// its derived commit whatever an earlier process left: the tag is the input's, so it repeats,
/// and the derived commit does not (`stash create` stamps the time) — a plain fetch refused
/// that as a non-fast-forward, with nothing on stderr under -q.
#[test]
fn a_refused_apply_does_not_spoil_the_next_offload_of_the_same_tree() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let s = c.session();
    std::fs::write(s.join("a.txt"), "two\n").unwrap();
    let out = c.derive(&d, &["--host", "h0"], &c.failing_session_git("checkout "));
    let err = refused(&out, 12);
    assert!(err.contains("git checkout"), "{err}");
    assert_eq!(
        c.git(&s, &["for-each-ref", "refs/offload/"]),
        "",
        "a refused apply left a scratch ref"
    );
    // the next derive's commit must carry another second
    std::thread::sleep(Duration::from_millis(1100));
    let out = c.derive(&d, &["--host", "h0"], &[]);
    assert!(
        out.status.success(),
        "{}\n{}",
        text(&out.stdout),
        text(&out.stderr)
    );
    assert_eq!(c.git(&s, &["for-each-ref", "refs/offload/"]), "");
}

/// A derive that changes nothing stages nothing and says the worktree is current; the host
/// was chosen past one that is down, and the report carries the probe. A host named by its
/// address derives, and `--keep` leaves its worktree there.
#[test]
fn a_named_host_derives_and_a_current_tree_stages_nothing() {
    let c = Cluster::new(&[("u@down", 50), ("u@lundra", 50)]);
    std::fs::write(c.root.join("hosts/u@down/down"), "").unwrap();
    let d = c.declaration(&["u@down", "u@lundra"]);
    let s = c.session();

    std::fs::write(s.join("NOOP"), "").unwrap();
    c.git(&s, &["add", "NOOP"]);
    let out = c.derive(&d, &["--format", "json"], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(
        err.contains("derive-offload: h0 skipped: ")
            && err.contains("derive-offload: chose h1")
            && err.contains("the derive changed nothing"),
        "{err}"
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["host"], "h1");
    assert!(report.get("derived").is_none(), "{report}");
    assert_eq!(report["staged"], serde_json::json!([]));
    assert_eq!(report["probe"][0]["standing"], "unreachable");
    assert_eq!(c.git(&s, &["diff", "--cached", "--name-only"]), "NOOP\n");
    c.git(&s, &["rm", "-q", "--cached", "NOOP"]);
    std::fs::remove_file(s.join("NOOP")).unwrap();

    std::fs::write(s.join("a.txt"), "named\n").unwrap();
    let out = c.derive(&d, &["--host", "u@lundra", "--keep"], &[]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let summary = text(&out.stdout);
    assert!(
        summary.starts_with("derived ") && summary.contains(" on h1 in "),
        "{summary}"
    );
    assert!(summary.contains("\n  generated.txt\n"), "{summary}");
    let tag = summary.split_whitespace().nth(1).unwrap();
    assert!(
        c.home("u@lundra")
            .join("dev/prismatic-majordomus-wt")
            .join(format!("offload-{tag}"))
            .exists(),
        "--keep did not keep the host's worktree"
    );
}

/// The command's own refusals: a declaration that cannot be read, a directory that is no
/// repository, and output that cannot be written — each an error with its exit code, never a
/// listing cut short in silence.
#[test]
fn the_command_refuses_what_it_cannot_read_or_write() {
    let c = Cluster::new(&[("u@lundra", 50)]);
    let d = c.declaration(&["u@lundra"]);
    let decl = d.to_str().unwrap();

    let missing = c.root.join("missing.yaml");
    let out = c.offload(&["hosts", "--declaration", missing.to_str().unwrap()], None);
    assert!(refused(&out, 12).contains(&format!("offload: {}: ", missing.display())));

    let plain = c.root.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let out = c.offload(&["hosts", "--repo", plain.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(12), "{}", text(&out.stderr));

    let closed = |args: &[&str]| {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let mut cmd = common::majordomus();
        cmd.arg("offload").args(args).current_dir(c.session());
        c.env(&mut cmd);
        cmd.stdout(writer).output().unwrap()
    };
    for args in [
        &["hosts", "--declaration", decl][..],
        &["probe", "--declaration", decl][..],
        &["derive", ".", "--declaration", decl][..],
    ] {
        let out = closed(args);
        assert!(refused(&out, 13).contains("<stdout>: "), "{args:?}");
    }
}

/// A repository whose layer declares no host says so, and contacts nobody.
#[test]
fn a_repository_without_a_declaration_has_nothing_to_offload_to() {
    let f = common::Fixture::new();
    let (code, out, err) = common::run_in(&f.root(), &["offload", "hosts"], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.starts_with("no derive-offload declaration in this repository"),
        "{out}"
    );
    let (code, _, err) = common::run_in(&f.root(), &["offload", "probe"], "");
    assert_eq!(code, 12);
    assert!(err.contains("declares no derive-offload host"), "{err}");
}
