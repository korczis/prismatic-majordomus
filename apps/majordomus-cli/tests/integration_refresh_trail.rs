//! A refresh records what it observed on the repository's trail, and a trail that cannot be
//! written refuses the refresh: an observation nobody can find on the trail is not reported
//! as taken. Through the real command line, against a scripted forge and a local origin.

mod common;

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
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
}

#[test]
fn a_trail_that_cannot_be_written_refuses_the_refresh() {
    let f = common::Fixture::new();
    let root = f.root();
    let outside = f.container();
    let origin = outside.join("origin.git");
    let bin = outside.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    git(
        &outside,
        &["init", "-q", "--bare", "-b", "master", "origin.git"],
    );
    git(
        &root,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&root, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
    let script = format!(
        r#"#!/bin/sh
case "$1 $2" in
  "repo view") echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}' ;;
  "api repos/o/r") echo '{{"allow_merge_commit":true}}' ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C "{origin}" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list") echo '[]' ;;
  *) exit 1 ;;
esac
"#,
        origin = origin.display(),
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // the repository's trail is in the way: a directory where the file belongs
    std::fs::create_dir_all(root.join(".git/majordomus/integration/events.jsonl/in-the-way"))
        .unwrap();
    let out = common::majordomus()
        .args(["prs", "--repo"])
        .arg(&root)
        .arg("refresh")
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("majordomus runs");
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(12), "{err}");
    assert!(
        err.contains("events.jsonl"),
        "the refusal names the trail: {err}"
    );
}

/// `prs refresh` against a scripted forge whose every read is answered except `gh pr list`,
/// which runs `list` (a shell command): whether it succeeded, and what it printed on stderr.
fn refresh_answering_list(list: &str) -> (bool, String) {
    let f = common::Fixture::new();
    let root = f.root();
    let outside = f.container();
    let origin = outside.join("origin.git");
    let bin = outside.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    git(
        &outside,
        &["init", "-q", "--bare", "-b", "master", "origin.git"],
    );
    git(
        &root,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&root, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
    let script = format!(
        r#"#!/bin/sh
case "$1 $2" in
  "repo view") echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}' ;;
  "api repos/o/r") echo '{{"allow_merge_commit":true}}' ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C "{origin}" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list") {list} ;;
  *) exit 1 ;;
esac
"#,
        origin = origin.display(),
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let out = common::majordomus()
        .args(["prs", "--repo"])
        .arg(&root)
        .arg("refresh")
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .expect("majordomus runs");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// The open list a scripted forge answers with something that is not JSON is a refused
/// refresh naming `gh pr list`, never an observation of no pull requests.
#[test]
fn an_open_list_that_is_not_json_refuses_the_refresh() {
    let (ok, err) = refresh_answering_list("echo 'this is not json'");
    assert!(!ok, "a list that is not JSON is no observation: {err}");
    assert!(err.contains("pr list"), "the refusal names the read: {err}");
}

/// The open list the forge never answers — an outage on every attempt, asked again until the
/// attempts are spent — is a refused refresh naming `gh pr list`, never an observation of no
/// pull requests.
#[test]
fn an_open_list_the_forge_never_answers_refuses_the_refresh() {
    let (ok, err) = refresh_answering_list("echo 'HTTP 502: Bad Gateway' >&2; exit 1");
    assert!(!ok, "an unanswered list is no observation: {err}");
    assert!(err.contains("pr list"), "the refusal names the read: {err}");
}
