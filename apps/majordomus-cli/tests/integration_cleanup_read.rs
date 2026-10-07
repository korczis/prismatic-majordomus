//! The cleanup plan as a read, over MCP (WP23): what cleanup would close and what it leaves
//! for a person, decided offline from the recorded observation, beside the branch report
//! `prs cleanup` last recorded — and Obsolete, which only a person's label makes (owner
//! decision D3), listed for a person and never closed.
//!
//! The forge is a `gh` on the child's PATH answering from fixed text: one open pull request,
//! labelled `obsolete`, and one merged pull request whose branch origin still serves at the
//! head that merged. Origin is a bare repository beside the supervised fixture, so the
//! fetches and `git ls-remote` are real; nothing reaches the network.

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{json, Value};

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

/// The supervised fixture with an origin and a scripted forge on `bin`.
struct Forge {
    f: common::Fixture,
    bin: PathBuf,
    log: PathBuf,
}

fn forge() -> Forge {
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
    // #1 is open, its head one commit on master, labelled obsolete by a person
    git(&root, &["commit", "-q", "--allow-empty", "-m", "change 1"]);
    let head = git(&root, &["rev-parse", "HEAD"]);
    git(
        &root,
        &[
            "push",
            "-q",
            "origin",
            "HEAD:refs/heads/fix/one",
            "HEAD:refs/pull/1/head",
        ],
    );
    // #9 merged from fix/left, which origin still serves at the head that merged
    git(&root, &["commit", "-q", "--allow-empty", "-m", "change 9"]);
    let left = git(&root, &["rev-parse", "HEAD"]);
    git(&root, &["push", "-q", "origin", "HEAD:refs/heads/fix/left"]);
    git(&root, &["reset", "-q", "--hard", "HEAD~2"]);
    let open = json!([{
        "number": 1, "title": "change 1", "author": {"login": "someone"},
        "headRefName": "fix/one", "headRefOid": head, "baseRefName": "master",
        "isDraft": false, "labels": [{"name": "obsolete"}],
        "createdAt": "2026-10-01T00:00:00Z", "updatedAt": "2026-10-01T00:00:00Z",
        "body": "", "statusCheckRollup": [], "reviewDecision": "",
        "autoMergeRequest": null, "isCrossRepository": false
    }]);
    let merged = json!([{
        "number": 9, "state": "MERGED", "headRefName": "fix/left", "headRefOid": left,
        "isCrossRepository": false, "mergedAt": "2026-10-02T00:00:00Z"
    }]);
    std::fs::write(outside.join("open.json"), open.to_string()).unwrap();
    std::fs::write(outside.join("merged.json"), merged.to_string()).unwrap();
    let log = outside.join("gh.log");
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> "{log}"
case "$1 $2" in
  "repo view") echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}' ;;
  "api repos/o/r") echo '{{"allow_merge_commit":true,"delete_branch_on_merge":false}}' ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C "{origin}" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "api graphql") jq -c '{{data:{{repository:{{pullRequests:{{pageInfo:{{hasNextPage:false,endCursor:null}},nodes:[.[]|{{number,authorAssociation:"OWNER",isCrossRepository:false,timelineItems:{{pageInfo:{{hasNextPage:false,endCursor:null}},nodes:[]}}}}]}}}}}}}}' "{dir}/open.json" ;;
  "pr list")
    case " $* " in
      *" --state merged "*) cat "{dir}/merged.json" ;;
      *) cat "{dir}/open.json" ;;
    esac ;;
  *) echo UNEXPECTED >> "{log}"; exit 1 ;;
esac
"#,
        log = log.display(),
        origin = origin.display(),
        dir = outside.display(),
    );
    let gh = bin.join("gh");
    std::fs::write(&gh, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    Forge { f, bin, log }
}

fn path(fg: &Forge) -> String {
    format!(
        "{}:{}",
        fg.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn prs(fg: &Forge, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(common::BIN)
        .arg("prs")
        .arg("--repo")
        .arg(fg.f.root())
        .args(args)
        .env("PATH", path(fg))
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

/// One `tools/call` of `tool` against the fixture, with the scripted forge on the PATH so a
/// tool that reached for the network would show in its log.
fn mcp_call(fg: &Forge, tool: &str) -> Value {
    let mut child = Command::new(common::BIN)
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("PATH", path(fg))
        .current_dir(fg.f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": tool, "arguments": {} } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    frames[1]["result"]["structuredContent"].clone()
}

#[test]
fn the_cleanup_read_renders_the_plan_and_the_recorded_branches_offline() {
    let fg = forge();
    // nothing observed: the read says so, and recorded nothing
    let before = mcp_call(&fg, "majordomus_pull_requests_cleanup");
    assert_eq!(before["observed"], Value::Bool(false), "{before}");
    assert_eq!(before["branches"], Value::Null);

    let (code, out, err) = prs(&fg, &["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");
    let (code, out, err) = prs(&fg, &["cleanup"]);
    assert_eq!(code, 0, "cleanup: {out}{err}");
    assert!(out.contains("#1") && out.contains("obsolete"), "{out}");
    assert!(out.contains("fix/left"), "{out}");
    let asked = std::fs::read_to_string(&fg.log).unwrap();

    let r = mcp_call(&fg, "majordomus_pull_requests_cleanup");
    assert_eq!(r["observed"], Value::Bool(true), "{r}");
    let items = r["items"].as_array().expect("items");
    assert_eq!(items.len(), 1, "{r}");
    assert_eq!(items[0]["pr"], 1);
    assert_eq!(items[0]["disposition"], "obsolete");
    assert_eq!(
        items[0]["reasons"][0], "label_obsolete:obsolete",
        "the deciding reason first"
    );
    assert_eq!(items[0]["action"], "left_for_a_person");
    assert_eq!(r["branches"]["branches"][0]["branch"], "fix/left", "{r}");
    assert_eq!(r["branches"]["refreshed_by"], "majordomus prs cleanup");
    assert!(r["branches_age_seconds"].is_u64(), "{r}");
    // the read asked the forge nothing
    assert_eq!(
        std::fs::read_to_string(&fg.log).unwrap(),
        asked,
        "the cleanup read reached the forge"
    );
    // and cleanup --apply leaves the obsolete pull request open
    let (code, out, err) = prs(&fg, &["cleanup", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    let log = std::fs::read_to_string(&fg.log).unwrap();
    assert!(
        !log.contains("pr close"),
        "an obsolete pull request was closed:\n{log}"
    );
    assert!(!log.contains("UNEXPECTED"), "{log}");
}
