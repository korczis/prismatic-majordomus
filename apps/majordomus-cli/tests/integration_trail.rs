//! The integration audit trail through the real command line (ADR 0101): one file under the
//! common git directory, written by every worktree of a repository and read by each.
//!
//! The forge is a `gh` on the child's PATH that answers the questions the adapter asks from
//! this test's files, logs every call, and closes a pull request when asked. One pull
//! request is open and its head is master's own commit, so its work is on master already:
//! `prs cleanup --apply` closes it, and the trail must hold the lease, the observation, the
//! attempt before the closure and the closure after it — readable from a second worktree
//! that never ran anything but a read.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

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

struct Forge {
    _tmp: tempfile::TempDir,
    work: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// A bare origin, a clone of it, and a scripted `gh` that serves #2 — whose head is master.
fn forge() -> Forge {
    let tmp = tempfile::tempdir().expect("tempdir");
    let base = tmp.path().canonicalize().expect("canonical tempdir");
    let origin = base.join("origin.git");
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
    git(
        &work,
        &[
            "push",
            "-q",
            "origin",
            "HEAD:refs/heads/master",
            "HEAD:refs/heads/feature/2",
            "HEAD:refs/pull/2/head",
        ],
    );
    let head = git(&work, &["rev-parse", "HEAD"]);
    let pr = format!(
        r#"[{{"number":2,"title":"change 2","author":{{"login":"someone"}},"headRefName":"feature/2","headRefOid":"{head}","baseRefName":"master","isDraft":false,"labels":[],"createdAt":"2026-09-02T00:00:00Z","updatedAt":"2026-09-02T00:00:00Z","body":"","statusCheckRollup":[{{"__typename":"CheckRun","name":"ci","status":"COMPLETED","conclusion":"SUCCESS"}}],"reviewDecision":"","autoMergeRequest":null,"isCrossRepository":false}}]"#
    );
    std::fs::write(state.join("open.json"), pr).unwrap();
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> "{state}/log"
case "$1 $2" in
  "repo view") echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}' ;;
  "api repos/o/r") echo '{{"allow_merge_commit":true}}' ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C "{origin}" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "pr list") if [ -f "{state}/closed-2" ]; then echo '[]'; else cat "{state}/open.json"; fi ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr view") if [ -f "{state}/closed-$3" ]; then echo "CLOSED {head}"; else echo "OPEN {head}"; fi ;;
  "pr close") touch "{state}/closed-$3" ;;
  *) echo UNEXPECTED >> "{state}/log"; exit 1 ;;
esac
"#,
        state = state.display(),
        origin = origin.display(),
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    Forge {
        _tmp: tmp,
        work,
        bin,
        state,
    }
}

/// `majordomus prs --repo <dir> <args>`, with the scripted forge first on the PATH.
fn prs(f: &Forge, dir: &Path, args: &[&str]) -> (i32, String, String) {
    let path = format!(
        "{}:{}",
        f.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(common::BIN)
        .arg("prs")
        .arg("--repo")
        .arg(dir)
        .args(args)
        .env("PATH", path)
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("majordomus runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn actions(events: &Value) -> Vec<String> {
    events
        .as_array()
        .expect("a list of events")
        .iter()
        .map(|e| e["action"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn every_act_is_on_the_repositorys_trail_before_it_is_taken() {
    let f = forge();
    let (code, _, err) = prs(&f, &f.work, &["refresh"]);
    assert_eq!(code, 0, "refresh: {err}");
    let trail = f.work.join(".git/majordomus/integration/events.jsonl");
    let text = std::fs::read_to_string(&trail).expect("the repository's trail");
    assert!(text.contains(r#""action":"observed""#), "{text}");
    assert!(
        !f.work
            .join(".ai/local/state/integration/events.jsonl")
            .exists(),
        "a checkout keeps a trail of its own"
    );

    let (code, out, err) = prs(&f, &f.work, &["cleanup", "--apply"]);
    assert_eq!(code, 0, "cleanup: {out}{err}");
    assert!(out.contains("closed"), "{out}");
    let log = std::fs::read_to_string(f.state.join("log")).unwrap();
    assert!(log.contains("pr close 2 --comment"), "{log}");

    // a second worktree of the same clone reads the same trail
    let second = f.work.parent().unwrap().join("second");
    git(
        &f.work,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            second.to_str().unwrap(),
            "master",
        ],
    );
    let (code, out, err) = prs(&f, &second, &["events", "--format", "json"]);
    assert_eq!(code, 0, "events: {err}");
    let events: Value = serde_json::from_str(&out).expect("events as JSON");
    let acts: Vec<String> = actions(&events)
        .into_iter()
        .filter(|a| a != "observed")
        .collect();
    assert_eq!(
        acts,
        [
            "lease_acquired",
            "close_attempted",
            "closed_redundant",
            "lease_released"
        ],
        "{out}"
    );
    let closed = events
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["action"] == "closed_redundant")
        .unwrap();
    assert_eq!(closed["pr"], 2);
    assert!(
        closed["evidence"].as_array().is_some_and(|e| !e.is_empty()),
        "the closure names the evidence it was decided on: {closed}"
    );
}

#[test]
fn an_observation_the_trail_cannot_record_is_refused() {
    let f = forge();
    let trail = f.work.join(".git/majordomus/integration/events.jsonl");
    // a directory where the trail must be: nothing can be appended to it
    std::fs::create_dir_all(&trail).unwrap();
    let (code, out, err) = prs(&f, &f.work, &["refresh"]);
    assert_eq!(code, 12, "refresh went on without its trail: {out}{err}");
    assert!(err.contains("events.jsonl"), "{err}");
    // a dry run still decides, records nothing it acts on, and exits as a dry run does
    std::fs::remove_dir(&trail).unwrap();
    let (code, out, err) = prs(&f, &f.work, &["drain", "--dry-run"]);
    assert_eq!(code, 0, "dry run: {out}{err}");
    // it says why it stopped, once: the idle step is the reason and nothing repeats it
    assert_eq!(out.matches("nothing is ready").count(), 1, "{out}");
    assert!(!out.contains("stopped:"), "{out}");
    // the report as JSON keeps both: the step's reason and why the drain stopped
    let (code, out, err) = prs(&f, &f.work, &["drain", "--dry-run", "--format", "json"]);
    assert_eq!(code, 0, "dry run as JSON: {out}{err}");
    let report: Value = serde_json::from_str(&out).expect("the report as JSON");
    assert_eq!(report["dry_run"], true);
    assert!(
        report["stopped"]
            .as_str()
            .is_some_and(|s| s.contains("nothing is ready")),
        "{out}"
    );
}
