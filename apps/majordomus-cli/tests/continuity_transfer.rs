//! `majordomus continuity` through the built executable, on two clones of one remote, each
//! with its own home and state directory — so its own device key — and nothing shared but
//! the remote.
//!
//! The domain's decisions are proved beside it, in `continuity`'s own tests. What only the
//! executable can prove is proved here: that every subcommand reaches its capability
//! through the one executor, that the device key lives in the state directory the process
//! was given and nowhere else, that a read never creates one, and that a refusal is an
//! exit code a script can branch on, with the reason on stderr.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{dist_share, Fixture, BIN};
use serde_json::Value;

/// One machine: a checkout, and the home and state directory it runs with.
struct Machine {
    root: PathBuf,
    home: PathBuf,
}

impl Machine {
    fn new(root: PathBuf, home: PathBuf) -> Machine {
        std::fs::create_dir_all(&home).unwrap();
        Machine { root, home }
    }

    fn state(&self) -> PathBuf {
        self.home.join("state")
    }

    /// Run `majordomus continuity <args>` here: the exit code, stdout and stderr.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = Command::new(BIN)
            .arg("continuity")
            .args(args)
            .current_dir(&self.root)
            .env("HOME", &self.home)
            .env("XDG_STATE_HOME", self.state())
            .env("MAJORDOMUS_LOG", "error")
            .env("MAJORDOMUS_SHARE", dist_share())
            .output()
            .expect("spawn majordomus");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8(out.stdout).unwrap(),
            String::from_utf8(out.stderr).unwrap(),
        )
    }

    /// The JSON answer of a subcommand that succeeds.
    fn json(&self, args: &[&str]) -> Value {
        let mut all = args.to_vec();
        all.extend(["--format", "json"]);
        let (code, out, err) = self.run(&all);
        assert_eq!(code, 0, "continuity {args:?}: {out}{err}");
        serde_json::from_str(&out).unwrap()
    }

    fn git(&self, args: &[&str]) -> String {
        git(&self.root, args)
    }
}

fn git(root: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A handover record in `root`, as `majordomus handover` writes one.
fn handover(root: &Path, stamp: &str, next: &str) {
    let dir = root.join(".ai/local/state/handovers");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{stamp}--feature-x--0000000--00.md")),
        format!(
            "---\nschema_version: 1\ncreated_at: 2026-10-03T12:00:00Z\ntask_id: none\n\
             branch: feature/x\nworking_tree: clean\n---\n\n# Objective\nShip x\n\n\
             # Current State\nhalf\n\n# Next Action\n{next}\n"
        ),
    )
    .unwrap();
}

#[test]
fn a_handover_published_on_one_machine_is_resumed_on_another() {
    let f = Fixture::new();
    let remote = f.parent().join("remote.git");
    git(
        &f.parent(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    let a = Machine::new(f.root(), f.parent().join("home-a"));
    a.git(&["checkout", "-qb", "feature/x"]);
    a.git(&["remote", "add", "origin", remote.to_str().unwrap()]);
    a.git(&["push", "-q", "-u", "origin", "feature/x"]);

    // the device key is made in the state directory this process was given, labelled
    let device = a.json(&["device", "--label", "macbook-pro"]);
    assert_eq!(device["device"]["label"], "macbook-pro");
    assert!(a.state().join("majordomus/node.json").is_file());
    let (code, text, _) = a.run(&["device"]);
    assert_eq!(code, 0);
    assert!(text.starts_with("device      macbook-pro\n"), "{text}");
    assert!(text.contains(&format!(
        "public key  {}",
        device["public_key"].as_str().unwrap()
    )));

    // nothing to publish is a refusal a script can branch on, before any write
    let (code, _, err) = a.run(&["publish"]);
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("no handover record"), "{err}");

    handover(&a.root, "20261003T120000Z", "write the test");
    let (code, text, err) = a.run(&["publish", "--issue", "#184"]);
    assert_eq!(code, 0, "{err}");
    assert!(text.starts_with("Published   "), "{text}");
    let status = a.json(&["status"]);
    assert_eq!(status["store"]["sync"], "never_synced");
    assert_eq!(status["device"]["label"], "macbook-pro");
    let (code, text, _) = a.run(&["sync"]);
    assert_eq!(code, 0);
    assert!(
        text.starts_with("Sync        published with origin"),
        "{text}"
    );
    let records = a.json(&["records"]);
    assert_eq!(records["records"].as_array().unwrap().len(), 1);
    let record = records["records"][0]["id"].as_str().unwrap().to_string();

    // machine B: a clone, a home of its own, and no key yet
    let b_root = f.parent().join("b");
    git(
        &f.parent(),
        &[
            "clone",
            "-q",
            remote.to_str().unwrap(),
            b_root.to_str().unwrap(),
        ],
    );
    let b = Machine::new(b_root, f.parent().join("home-b"));
    b.git(&["checkout", "-q", "feature/x"]);
    let status = b.json(&["status"]);
    assert!(
        status["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "continuity.no_device_identity"),
        "{status}"
    );
    assert!(
        !b.state().join("majordomus/node.json").exists(),
        "a read created a key"
    );
    let (code, text, _) = b.run(&["status"]);
    assert_eq!(code, 0);
    assert!(text.contains("resumable   none"), "{text}");

    let synced = b.json(&["sync"]);
    assert_eq!(synced["fetched"], 1);
    let (code, text, _) = b.run(&["records"]);
    assert_eq!(code, 0);
    assert!(text.starts_with("1 record(s)"), "{text}");
    let plan = b.json(&["plan"]);
    assert_eq!(plan["status"], "ready_with_warnings");
    assert_eq!(plan["record"]["id"], record.as_str());
    let (code, text, _) = b.run(&["plan", "--record", &record[..8]]);
    assert_eq!(code, 0);
    assert!(
        text.starts_with("Resume      ready_with_warnings"),
        "{text}"
    );

    // an unknown record is nothing to resume, and the plan names why
    let unknown = b.json(&["plan", "--record", "ffffffff"]);
    assert_eq!(unknown["status"], "nothing_to_resume");
    assert_eq!(unknown["blockers"][0]["code"], "record_unknown");

    let resumed = b.json(&["resume"]);
    assert_eq!(resumed["resumed"], true);
    assert_eq!(resumed["plan"]["next_action"], "write the test");
    let (code, text, _) = b.run(&["resume", "--record", &record]);
    assert_eq!(code, 0);
    assert!(text.starts_with("Session resumed"), "{text}");

    // what is not a remote name is refused before git is asked
    let (code, _, err) = b.run(&["sync", "--remote", "a:b"]);
    assert_eq!(code, 10);
    assert!(err.contains("is not a remote name"), "{err}");
}

/// An answer that cannot be written is a failure of the transport and never a success: the
/// lines are collected and written once, so a report is printed whole or not at all, in
/// either format.
#[test]
fn an_answer_that_cannot_be_written_is_a_transport_failure() {
    use std::process::Stdio;
    let f = Fixture::new();
    let a = Machine::new(f.root(), f.parent().join("home-a"));
    for format in ["text", "json"] {
        // The writing end of a pipe whose only reader has exited: the stdin std made for a
        // process that is gone. `std::io::pipe` is newer than the crate's rust-version.
        let mut gone = Command::new(BIN)
            .arg("--version")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let writer = gone.stdin.take().unwrap();
        assert!(gone.wait().unwrap().success());
        let out = Command::new(BIN)
            .args(["continuity", "status", "--format", format])
            .current_dir(&a.root)
            .env("HOME", &a.home)
            .env("XDG_STATE_HOME", a.state())
            .env("MAJORDOMUS_LOG", "error")
            .env("MAJORDOMUS_SHARE", dist_share())
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer))
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap();
        assert_eq!(
            out.status.code(),
            Some(13),
            "--format {format}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// A repository that is not there is refused before any capability is asked, with the path
/// on stderr and nothing on stdout.
#[test]
fn a_repository_that_does_not_exist_is_refused_and_nothing_is_answered() {
    let f = Fixture::new();
    let a = Machine::new(f.root(), f.parent().join("home-a"));
    let missing = f.path("does-not-exist");
    let (code, out, err) = a.run(&["status", "--repo", missing.to_str().unwrap()]);
    assert_eq!(code, 13, "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(err.contains("does-not-exist"), "{err}");
}

/// A device with nowhere to keep its key is told so: with no home and no state directory
/// there is no identity to read and none is invented somewhere else.
#[test]
fn a_process_with_no_home_and_no_state_directory_is_refused() {
    let f = Fixture::new();
    for args in [&["status"][..], &["device"][..]] {
        let out = Command::new(BIN)
            .arg("continuity")
            .args(args)
            .current_dir(f.root())
            .env_remove("HOME")
            .env_remove("XDG_STATE_HOME")
            .env("MAJORDOMUS_LOG", "error")
            .env("MAJORDOMUS_SHARE", dist_share())
            .output()
            .expect("spawn majordomus");
        assert_ne!(out.status.code(), Some(0), "continuity {args:?}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.contains("no HOME and no XDG_STATE_HOME"),
            "{args:?}: {err}"
        );
        assert!(out.stdout.is_empty(), "{args:?} answered on stdout");
    }
}

/// A device key that cannot be read stops every subcommand before it answers anything: a
/// record signed by a key made up on the spot would be a forgery of this device's own.
#[test]
fn a_device_key_that_cannot_be_read_stops_every_subcommand() {
    let f = Fixture::new();
    let a = Machine::new(f.root(), f.parent().join("home-a"));
    a.git(&["checkout", "-qb", "feature/x"]);
    // the repository declares a mesh, so the machine is opened with one
    let device = a.json(&["device", "--label", "macbook-pro"]);
    f.write(
        ".ai/repo/mesh/majordomus.yaml",
        &format!(
            "schema: mesh/v1\nkind: mesh-declaration\nid: majordomus\nenabled: false\n             trust:\n  policy: deny_unknown\n  allow:\n    - {}\n",
            device["public_key"].as_str().unwrap()
        ),
    );
    a.git(&["add", "-A"]);
    a.git(&["commit", "-qm", "the mesh declaration"]);
    let (code, _, err) = a.run(&["status"]);
    assert_eq!(code, 0, "{err}");
    handover(&a.root, "20261003T120000Z", "write the test");

    std::fs::write(a.state().join("majordomus/node.json"), "{ not a key").unwrap();
    for args in [
        &["status"][..],
        &["records"][..],
        &["plan"][..],
        &["resume"][..],
        &["sync"][..],
        &["publish"][..],
        &["device"][..],
    ] {
        let (code, out, err) = a.run(args);
        assert_ne!(
            code, 0,
            "continuity {args:?} answered with an unreadable key: {out}"
        );
        assert!(out.is_empty(), "continuity {args:?}: {out}");
        assert!(!err.is_empty(), "continuity {args:?} said nothing");
    }
}

/// A repository whose git knows no committer still publishes: the store's commit carries
/// no authored work, so it is made under a neutral identity instead of being refused.
#[test]
fn a_repository_with_no_git_identity_still_publishes() {
    let f = Fixture::new();
    let a = Machine::new(f.root(), f.parent().join("home-a"));
    a.git(&["checkout", "-qb", "feature/x"]);
    handover(&a.root, "20261003T120000Z", "write the test");
    // no identity in the repository, and none from the machine's own configuration
    let _ = Command::new("git")
        .arg("-C")
        .arg(&a.root)
        .args(["config", "--unset-all", "user.email"])
        .status();
    let _ = Command::new("git")
        .arg("-C")
        .arg(&a.root)
        .args(["config", "--unset-all", "user.name"])
        .status();
    let out = Command::new(BIN)
        .args(["continuity", "publish", "--format", "json"])
        .current_dir(&a.root)
        .env("HOME", &a.home)
        .env("XDG_STATE_HOME", a.state())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_AUTHOR_NAME")
        .env_remove("GIT_AUTHOR_EMAIL")
        .env_remove("GIT_COMMITTER_NAME")
        .env_remove("GIT_COMMITTER_EMAIL")
        .env_remove("EMAIL")
        .env("MAJORDOMUS_LOG", "error")
        .env("MAJORDOMUS_SHARE", dist_share())
        .output()
        .expect("spawn majordomus");
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let published: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(published["written"], true);
    let author = git(
        &a.root,
        &[
            "log",
            "-1",
            "--format=%an <%ae>",
            "refs/majordomus/continuity",
        ],
    );
    assert_eq!(author, "majordomus <majordomus@localhost>");
}
