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
//! shared directory meanwhile, and writes derived files that say what it saw.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_majordomus");

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
exec env HOME="$h/home" PATH="$h/bin:$PATH" sh -c "$*"
"#;

const DF_SHIM: &str = r#"#!/bin/sh
echo "Filesystem 1024-blocks Used Available Capacity Mounted on"
echo "fake 1 1 $(cat "$(dirname "$0")/../free_kib") 1% /"
"#;

/// The derive a host runs. It builds into $CARGO_TARGET_DIR, optionally waits while the test
/// builds into the shared directory, then writes what it saw.
const FAKE_DERIVE: &str = r#"#!/bin/sh
set -e
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
        let mut y = format!(
            "schema: derive-offload/v1\nkind: derive-offload\nid: test\nmin_free_gib: 10\nqueue:\n  directory: {q}\n  lock: {l}\n  poll_seconds: 1\n  wait_max_seconds: 60\nremote:\n  spool: {s}\nhosts:\n",
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
        let mut cmd = Command::new(BIN);
        cmd.arg("offload").args(args).current_dir(self.session());
        self.env(&mut cmd);
        if let Some(s) = signal {
            cmd.env("OFFLOAD_SIGNAL", s);
        }
        cmd.output().unwrap()
    }
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
