//! What merged pull requests leave on origin, through the real command line (owner decision
//! D4): the forge's `delete_branch_on_merge` decides deletion, so `prs cleanup` reports the
//! branches the forge left behind and deletes none of them.
//!
//! The read is cleanup's alone: `prs refresh`, which the executor runs before every decision,
//! never lists merged pull requests. The forge is a `gh` on the child's PATH that answers from
//! this test's files and logs every call; origin is a bare repository beside the clone, so
//! `git ls-remote` is real. Three merged pull requests came from three branches: one still
//! sits at the head that merged (left behind), one moved after its merge (newer work, never
//! reported), and one is checked out in a worktree of the clone (reported as kept, with its
//! path).

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
    base: PathBuf,
    origin: PathBuf,
    work: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

/// A commit on `branch` of the clone, pushed to origin; returns its sha.
fn commit_on(f: &Forge, branch: &str, file: &str) -> String {
    git(&f.work, &["checkout", "-q", "-B", branch, "master"]);
    std::fs::write(f.work.join(file), format!("{branch}\n")).unwrap();
    git(&f.work, &["add", file]);
    git(&f.work, &["commit", "-q", "-m", branch]);
    git(
        &f.work,
        &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
    );
    let sha = git(&f.work, &["rev-parse", "HEAD"]);
    git(&f.work, &["checkout", "-q", "master"]);
    sha
}

/// A bare origin with master only, a clone, and a scripted `gh` whose answers come from
/// files under `state`: `settings.json` for the repository settings, `merged.json` for the
/// merged pull requests (absent: that read is refused).
fn forge(delete_branch_on_merge: bool) -> Forge {
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
    git(&work, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
    std::fs::write(
        state.join("settings.json"),
        format!(
            r#"{{"allow_merge_commit":true,"delete_branch_on_merge":{delete_branch_on_merge}}}"#
        ),
    )
    .unwrap();
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> "{state}/log"
case "$1 $2" in
  "repo view") echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}' ;;
  "api repos/o/r") cat "{state}/settings.json" ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C "{origin}" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " $* " in
      *" --state merged "*) if [ -f "{state}/merged.json" ]; then cat "{state}/merged.json"; else echo 'refused' >&2; exit 1; fi ;;
      *) echo '[]' ;;
    esac ;;
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
        base,
        origin,
        work,
        bin,
        state,
    }
}

/// `majordomus prs --repo <dir> <args>`, with the scripted forge first on the PATH.
fn prs(f: &Forge, args: &[&str]) -> (i32, String, String) {
    let path = format!(
        "{}:{}",
        f.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new(common::BIN)
        .arg("prs")
        .arg("--repo")
        .arg(&f.work)
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

fn merged(number: u64, branch: &str, head: &str) -> String {
    format!(
        r#"{{"number":{number},"state":"MERGED","headRefName":"{branch}","headRefOid":"{head}","isCrossRepository":false,"mergedAt":"2026-10-0{number}T00:00:00Z"}}"#
    )
}

fn refreshed(f: &Forge) {
    let (code, out, err) = prs(f, &["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");
}

fn log(f: &Forge) -> String {
    std::fs::read_to_string(f.state.join("log")).unwrap_or_default()
}

/// What cleanup recorded for the surfaces that never reach the network.
fn record(f: &Forge) -> Value {
    let path = f
        .work
        .join(".ai/local/state/integration/left-branches.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("cleanup recorded its report"))
        .unwrap()
}

#[test]
fn cleanup_reports_the_branches_the_forge_left_and_deletes_none() {
    let f = forge(false);
    let left = commit_on(&f, "fix/left", "left.txt");
    let moved_at_merge = commit_on(&f, "fix/moved", "moved.txt");
    let here = commit_on(&f, "fix/here", "here.txt");
    // fix/moved gets newer work after its pull request merged
    git(&f.work, &["checkout", "-q", "fix/moved"]);
    std::fs::write(f.work.join("moved.txt"), "newer\n").unwrap();
    git(&f.work, &["commit", "-q", "-am", "newer"]);
    git(
        &f.work,
        &["push", "-q", "origin", "HEAD:refs/heads/fix/moved"],
    );
    git(&f.work, &["checkout", "-q", "master"]);
    // fix/here is checked out in a worktree of the clone
    let wt = f.base.join("here-wt");
    git(
        &f.work,
        &["worktree", "add", "-q", wt.to_str().unwrap(), "fix/here"],
    );
    std::fs::write(
        f.state.join("merged.json"),
        format!(
            "[{},{},{}]",
            merged(1, "fix/left", &left),
            merged(2, "fix/moved", &moved_at_merge),
            merged(3, "fix/here", &here)
        ),
    )
    .unwrap();
    let before = git(
        &f.origin,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );

    refreshed(&f);
    // refresh is the executor's hot path: it never asks what merged pull requests left
    assert!(
        !log(&f).contains("--state merged"),
        "refresh listed merged pull requests:\n{}",
        log(&f)
    );

    let (code, out, err) = prs(&f, &["cleanup"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("merged branches left on origin (2)"), "{out}");
    let line = |b: &str| {
        out.lines()
            .find(|l| l.trim_start().starts_with(b))
            .map(str::to_string)
    };
    let left_line = line("fix/left").expect("fix/left listed");
    assert!(
        left_line.contains("#1") && left_line.contains("left_for_a_person"),
        "{left_line}"
    );
    let here_line = line("fix/here").expect("fix/here listed");
    assert!(
        here_line.contains("kept: checked out at") && here_line.contains("here-wt"),
        "{here_line}"
    );
    assert!(line("fix/moved").is_none(), "{out}");
    assert!(out.contains("enable delete_branch_on_merge"), "{out}");

    let r = record(&f);
    assert_eq!(r["delete_branch_on_merge"], Value::Bool(false));
    assert_eq!(r["refreshed_by"], "majordomus prs cleanup");
    let names: Vec<&str> = r["branches"]
        .as_array()
        .expect("read")
        .iter()
        .map(|m| m["branch"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["fix/here", "fix/left"],
        "fix/moved moved after its merge"
    );

    // the JSON stays the list of pull requests to close: nothing is open here
    let (_, json, _) = prs(&f, &["cleanup", "--format", "json"]);
    assert_eq!(
        serde_json::from_str::<Value>(&json).unwrap(),
        Value::Array(vec![])
    );

    // and nothing was deleted, on origin or by any forge call
    let after = git(
        &f.origin,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    assert_eq!(before, after, "cleanup moved a ref on origin");
    let log = log(&f);
    assert!(
        !log.contains("UNEXPECTED"),
        "the forge was asked something unscripted:\n{log}"
    );
    assert!(!log.contains("DELETE") && !log.contains("delete"), "{log}");
}

#[test]
fn the_setting_names_what_would_clear_a_branch() {
    let f = forge(true);
    let left = commit_on(&f, "fix/left", "left.txt");
    std::fs::write(
        f.state.join("merged.json"),
        format!("[{}]", merged(1, "fix/left", &left)),
    )
    .unwrap();
    refreshed(&f);
    let (_, out, _) = prs(&f, &["cleanup"]);
    assert!(out.contains("outlived its merge"), "{out}");
    assert!(out.contains("git push origin --delete fix/left"), "{out}");
}

#[test]
fn only_master_on_origin_is_nothing_left_and_asks_nothing_more() {
    let f = forge(false);
    refreshed(&f);
    let (_, out, _) = prs(&f, &["cleanup"]);
    assert!(
        out.contains("merged branches: none left on origin"),
        "{out}"
    );
    assert!(
        !log(&f).contains("--state merged"),
        "a branchless origin still listed merged pull requests:\n{}",
        log(&f)
    );
}

#[test]
fn a_refused_read_is_unread_never_nothing_left() {
    let f = forge(false);
    commit_on(&f, "fix/left", "left.txt");
    // no merged.json: the forge refuses the merged list
    refreshed(&f);
    let (_, out, _) = prs(&f, &["cleanup"]);
    assert!(out.contains("merged branches: unread"), "{out}");
    assert_eq!(
        record(&f)["branches"],
        Value::Null,
        "unread is recorded as unread, never empty"
    );
}
