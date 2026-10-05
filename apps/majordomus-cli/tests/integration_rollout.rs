//! The rollout gate through the real command line (ADR 0101 §13): `prs drain --continuous`
//! is the last stage of the rollout, and the trail's record of verified merges is what
//! unlocks it.
//!
//! No forge is needed to prove the refusal, because it is asked before the lease, the base
//! or the network. The record is seeded into the repository's one trail, under the common
//! git directory, with the words the executor writes; past the gate the drain goes on to
//! read the base, and a `gh` that fails every call makes that visible as a different exit.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

struct Repo {
    _tmp: tempfile::TempDir,
    work: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// A clone with one commit, and a `gh` that logs and refuses every call.
fn repo() -> Repo {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().canonicalize().expect("canonical tempdir");
    let work = base.join("work");
    let bin = base.join("bin");
    let state = base.join("state");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    git(
        &base,
        &["init", "-q", "--bare", "-b", "master", "origin.git"],
    );
    git(&base, &["clone", "-q", "origin.git", "work"]);
    std::fs::write(work.join("a.txt"), "base\n").unwrap();
    git(&work, &["add", "a.txt"]);
    git(&work, &["commit", "-q", "-m", "base"]);
    git(&work, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
    let gh = bin.join("gh");
    std::fs::write(
        &gh,
        format!(
            "#!/bin/sh\necho \"$*\" >> \"{}/log\"\nexit 1\n",
            state.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    Repo {
        _tmp: tmp,
        work,
        bin,
        state,
    }
}

/// Append events to the repository's trail, as the executor writes them.
fn seed(r: &Repo, actions: &[&str]) {
    let common = git(
        &r.work,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    let dir = Path::new(&common).join("majordomus/integration");
    std::fs::create_dir_all(&dir).unwrap();
    let mut trail = std::fs::read_to_string(dir.join("events.jsonl")).unwrap_or_default();
    for a in actions {
        trail.push_str(&format!(
            r#"{{"at":"2026-10-05T00:00:00Z","actor":"seed","action":"{a}","pr":1,"reasons":[],"detail":"seeded"}}"#
        ));
        trail.push('\n');
    }
    std::fs::write(dir.join("events.jsonl"), trail).unwrap();
}

/// `majordomus prs --repo <work> drain --continuous --interval 30`.
fn continuous(r: &Repo) -> (i32, String) {
    let path = format!(
        "{}:{}",
        r.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(common::BIN)
        .args(["prs", "--repo"])
        .arg(&r.work)
        .args(["drain", "--continuous", "--interval", "30"])
        .env("PATH", path)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("majordomus runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn forge_calls(r: &Repo) -> String {
    std::fs::read_to_string(r.state.join("log")).unwrap_or_default()
}

#[test]
fn a_continuous_drain_without_a_record_is_refused_before_anything_happens() {
    let r = repo();
    let (code, err) = continuous(&r);
    assert_eq!(code, 10, "{err}");
    assert!(
        err.contains("holds 0") && err.contains("ADR 0101 §13"),
        "{err}"
    );
    assert_eq!(forge_calls(&r), "", "the refusal asked the forge");
    let common = git(
        &r.work,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    assert!(
        !Path::new(&common)
            .join("majordomus/locks/integration-master.lock")
            .exists(),
        "the refusal took the lease"
    );
}

#[test]
fn the_record_unlocks_it_and_an_unverified_merge_locks_it_again() {
    let r = repo();
    // four verified merges and the attempts around them: one short
    seed(
        &r,
        &[
            "merge_attempted",
            "merge_succeeded",
            "merge_succeeded",
            "merge_failed",
            "merge_succeeded",
            "merge_succeeded",
        ],
    );
    let (code, err) = continuous(&r);
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("holds 4"), "{err}");

    // the fifth: past the gate, the drain reads the base, which this forge refuses
    seed(&r, &["merge_succeeded"]);
    let (code, err) = continuous(&r);
    assert_ne!(code, 10, "the record did not unlock it: {err}");
    assert!(!err.contains("ADR 0101 §13"), "{err}");
    assert!(
        !forge_calls(&r).is_empty(),
        "past the gate, the forge is asked"
    );

    // a merge that could not be verified ends the record, acknowledged or not
    seed(
        &r,
        &[
            "verification_failed",
            "failure_acknowledged",
            "merge_succeeded",
        ],
    );
    let (code, err) = continuous(&r);
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("holds 1"), "{err}");
}
