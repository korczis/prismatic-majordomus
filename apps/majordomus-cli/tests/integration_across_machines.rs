//! One integration executor per repository across machines (ADR 0101 §7), over real
//! processes: checkouts with their own node identity and their own `majordomus serve`,
//! linked through the repository's mesh on loopback — the path two machines take — and one
//! scripted forge (a bare origin and a `gh` on `PATH`) that both observe.
//!
//! What is proven, through the executable as a person runs it:
//!
//! - an executor that takes the lease where the mesh runs also holds an exclusive mesh claim
//!   on `integration/<repository>/<base>`, and its trail names that claim;
//! - while it holds it, `prs drain` on the other machine exits 12 naming the holding
//!   session, before it takes its own lease;
//! - an executor killed outright leaves its claim held — the other machine is still refused —
//!   and the next executor of the same checkout releases the leftover with its own;
//! - then the other machine's executor runs;
//! - a checkout whose server runs no mesh is guarded per clone, and `prs brief` says so.
//!
//! The decisions themselves are unit-tested beside the code (`integration::exclusive`);
//! this file proves they hold between processes.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{dist_share, Fixture, Served, BIN};
use serde_json::{json, Value};

use majordomus_cli::mesh::identity::NodeIdentity;

const SCOPE: &str = "integration/o/r/master";

/// The forge both machines observe: a bare origin, and a `gh` that answers for `o/r` from it
/// with no open pull request.
struct Forge {
    _dir: tempfile::TempDir,
    origin: PathBuf,
    bin: PathBuf,
}

impl Forge {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("a forge");
        let origin = dir.path().join("origin.git");
        git(
            dir.path(),
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "master",
                origin.to_str().unwrap(),
            ],
        );
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let script = format!(
            r#"#!/bin/sh
case "$1 $2" in
  "repo view") echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}' ;;
  "api repos/o/r") echo '{{"allow_merge_commit":true}}' ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C '{origin}' rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  *) echo '[]' ;;
esac
"#,
            origin = origin.display()
        );
        let gh = bin.join("gh");
        std::fs::write(&gh, script).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        Forge {
            _dir: dir,
            origin,
            bin,
        }
    }

    fn path(&self) -> String {
        format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .expect("git");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

struct Node {
    fixture: Fixture,
    state: PathBuf,
    key: String,
}

impl Node {
    fn new(forge: &Forge) -> Self {
        let fixture = Fixture::new();
        let state = fixture.parent().join("xdg-state");
        let key = NodeIdentity::load_or_create(&state.join("majordomus").join("node.json"))
            .expect("a node identity")
            .public
            .public_key;
        let root = fixture.root();
        git(
            &root,
            &["remote", "add", "origin", forge.origin.to_str().unwrap()],
        );
        git(&root, &["push", "-q", "-f", "origin", "HEAD:master"]);
        // the record that unlocks continuous mode (ADR 0101 §13): five merges earlier bounded
        // drains made and proved, in the words the executor writes to the clone's one trail
        let trail = root.join(".git/majordomus/integration");
        std::fs::create_dir_all(&trail).unwrap();
        let merged = r#"{"at":"2026-10-05T00:00:00Z","actor":"seed","action":"merge_succeeded","pr":null,"reasons":[],"detail":"an earlier bounded merge"}"#;
        std::fs::write(trail.join("events.jsonl"), format!("{merged}\n").repeat(5)).unwrap();
        Node {
            fixture,
            state,
            key,
        }
    }

    /// The checkout as git names it: what `prs` resolves its `--repo` to.
    fn root(&self) -> PathBuf {
        self.fixture.root().canonicalize().expect("the checkout")
    }

    fn declare(&self, allow: &[&str], seeds: &[&Served]) {
        let mut yaml = String::from(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: true\nmulticast:\n  enabled: false\ntrust:\n  policy: deny_unknown\n  allow:\n",
        );
        for key in allow {
            yaml.push_str(&format!("    - {key}\n"));
        }
        yaml.push_str(
            "cooperation:\n  heartbeat_seconds: 1\n  expiry_seconds: 4\n  repository: one-repository\n",
        );
        if !seeds.is_empty() {
            yaml.push_str("  seeds:\n");
            for s in seeds {
                yaml.push_str(&format!("    - http://{}\n", s.address));
            }
        }
        self.fixture.write(".ai/repo/mesh/majordomus.yaml", &yaml);
    }

    fn serve(&self) -> Served {
        Served::start_with_env(
            &self.fixture.root(),
            &["--discovery", "filesystem"],
            &[("XDG_STATE_HOME", self.state.to_str().unwrap())],
        )
    }

    fn prs(&self, forge: &Forge, args: &[&str]) -> Command {
        let mut c = Command::new(BIN);
        c.arg("prs")
            .arg("--repo")
            .arg(self.root())
            .args(args)
            .current_dir(self.root())
            .env("PATH", forge.path())
            .env("MAJORDOMUS_SHARE", dist_share())
            .env("XDG_STATE_HOME", &self.state);
        c
    }

    /// `prs <args>` to its end: (status, stdout, stderr).
    fn run(&self, forge: &Forge, args: &[&str]) -> (i32, String, String) {
        let out = self.prs(forge, args).output().expect("prs");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    /// A continuous drain left running: the executor holding the lease while it waits.
    fn hold(&self, forge: &Forge) -> Child {
        let child = self
            .prs(forge, &["drain", "--continuous", "--interval", "30"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("a continuous drain");
        let lock = self.lock();
        wait_until(
            "the continuous drain takes the lease",
            Duration::from_secs(60),
            || lock.exists(),
        );
        child
    }

    fn lock(&self) -> PathBuf {
        self.root()
            .join(".git/majordomus/locks/integration-master.lock")
    }
}

fn patience() -> u32 {
    std::env::var("MAJORDOMUS_TEST_PATIENCE")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|f| (1..=20).contains(f))
        .unwrap_or(1)
}

fn wait_until<F: FnMut() -> bool>(what: &str, timeout: Duration, mut check: F) {
    let timeout = timeout * patience();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check() {
            return;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    panic!("timed out after {timeout:?} waiting for: {what}");
}

fn connected(s: &Served) -> usize {
    s.get("/api/v1/mesh/cooperation").1["peers"]
        .as_array()
        .map(|p| {
            p.iter()
                .filter(|p| p["state"] == json!("connected"))
                .count()
        })
        .unwrap_or(0)
}

/// The held claims on [`SCOPE`] a runtime knows of.
fn held(s: &Served) -> Vec<Value> {
    s.get("/api/v1/mesh/state").1["state"]["claims"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| c["state"]["state"] == json!("held"))
        .filter(|c| c["scope"] == json!([SCOPE]))
        .collect()
}

fn terminate(mut child: Child) {
    // SAFETY: a plain signal to a child this test spawned and still owns.
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let _ = child.wait();
}

#[test]
fn one_executor_integrates_a_repository_across_machines() {
    let forge = Forge::new();
    let a = Node::new(&forge);
    let b = Node::new(&forge);
    a.declare(&[&a.key, &b.key], &[]);
    let sa = a.serve();
    b.declare(&[&a.key, &b.key], &[&sa]);
    let sb = b.serve();
    wait_until("the two runtimes link", Duration::from_secs(20), || {
        connected(&sa) == 1 && connected(&sb) == 1
    });
    for n in [&a, &b] {
        let (code, out, err) = n.run(&forge, &["refresh"]);
        assert_eq!(code, 0, "refresh:\n{out}\n{err}");
    }

    // ---------------------------------------------------------------- A holds it
    let a_drain = a.hold(&forge);
    wait_until("B learns of A's claim", Duration::from_secs(30), || {
        held(&sb).len() == 1
    });
    let claim = held(&sb).remove(0);
    let (_, brief, _) = a.run(&forge, &["brief"]);
    assert!(
        brief.contains(&format!(
            "across machines by mesh claim {}",
            claim["key"].as_str().unwrap()
        )),
        "{brief}"
    );

    // ---------------------------------------------------------------- B is refused
    let refused = |when: &str| {
        let (code, out, err) = b.run(&forge, &["drain", "--max", "1"]);
        assert_eq!(
            code, 12,
            "{when}: B's executor was not refused:\n{out}\n{err}"
        );
        assert!(
            err.contains("another integration executor holds integration/o/r/master on the mesh"),
            "{when}: {err}"
        );
        assert!(
            err.contains(&format!(
                "held by session {}",
                claim["session"].as_str().unwrap()
            )),
            "{when}: the refusal does not name A's session: {err}"
        );
        assert!(
            !b.lock().exists(),
            "{when}: B took its clone's lease although the repository's was held"
        );
    };
    refused("while A drains");
    // every executor of B is refused the same way: a continuous drain and an applied cleanup
    for args in [
        &["drain", "--continuous", "--interval", "30"][..],
        &["cleanup", "--apply"][..],
    ] {
        let (code, out, err) = b.run(&forge, args);
        assert_eq!(code, 12, "{args:?} on B was not refused:\n{out}\n{err}");
        assert!(err.contains("on the mesh"), "{args:?}: {err}");
        assert!(!b.lock().exists(), "{args:?} took B's lease");
    }

    // ---------------------------------------------------------------- A killed outright
    let mut a_drain = a_drain;
    a_drain.kill().expect("kill -9");
    let _ = a_drain.wait();
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(held(&sb).len(), 1, "a killed executor's claim stays held");
    refused("after A was killed");

    // the next executor of A's checkout releases what the killed one left
    let (code, out, err) = a.run(&forge, &["drain", "--max", "1"]);
    assert_eq!(code, 0, "A's next drain:\n{out}\n{err}");
    wait_until("nothing is held on B", Duration::from_secs(30), || {
        held(&sb).is_empty()
    });

    // ---------------------------------------------------------------- B's turn
    let (code, out, err) = b.run(&forge, &["drain", "--max", "1"]);
    assert_eq!(
        code, 0,
        "B's drain once the repository is free:\n{out}\n{err}"
    );
    let (_, events, _) = b.run(&forge, &["--format", "json", "events"]);
    assert!(
        events.contains("mesh_claim"),
        "B's lease was not recorded with its claim: {events}"
    );
    wait_until("B released its claim", Duration::from_secs(30), || {
        held(&sa).is_empty()
    });
}

#[test]
fn where_no_mesh_runs_the_lease_guards_the_clone_and_says_so() {
    let forge = Forge::new();
    // a server, but no mesh declared: the claim is not refused, it reaches nothing
    let c = Node::new(&forge);
    let _served = c.serve();
    let (code, out, err) = c.run(&forge, &["refresh"]);
    assert_eq!(code, 0, "refresh:\n{out}\n{err}");
    let drain = c.hold(&forge);
    let (code, brief, err) = c.run(&forge, &["brief"]);
    assert_eq!(code, 0, "{err}");
    assert!(brief.contains("per-clone guard only"), "{brief}");
    terminate(drain);
}
