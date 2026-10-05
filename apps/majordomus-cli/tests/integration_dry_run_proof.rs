//! The dry-run proof through the real command line and over MCP (WP29, `majordomus prs
//! prove-dry-run`): the executor's non-mutating cycle — refresh, plan, drain --dry-run,
//! cleanup without --apply — runs between two snapshots of everything it could move, and the
//! proof says whether anything did. Case 852's scenario, as a Rust integration test, so the
//! instrumented executable is measured.
//!
//! The forge is a `gh` on the child's PATH that answers reads from this test's files and
//! refuses every write; origin is a bare repository beside the supervised fixture, so the
//! fetches and `git ls-remote` are real and nothing reaches the network. `MODE` makes the
//! forge move under the proof: a new branch while the opening snapshot lists the pull requests
//! (`intrude`, so the closing snapshot finds it), or, while the closing one lists them, a pull
//! request's head moved (`move`) or gone (`gone`), which the mirror check reads after it.
//! `act` appends an act to the repository's trail while the closing snapshot lists, which the
//! proof must see. `REFUSE` makes the forge refuse everything, `REFUSE_REFRESH_AT=N` only the
//! Nth refresh's listing, and `GARBLE_AT=N` answers the Nth snapshot's list with text that is
//! not a list.

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

struct Forge {
    f: common::Fixture,
    origin: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    h1: String,
    h2: String,
}

/// Case 852's repository: #2 already landed (redundant), #1 branches from master (ready).
fn forge() -> Forge {
    let f = common::Fixture::new();
    let root = f.root();
    let outside = f.container();
    let origin = outside.join("origin.git");
    let bin = outside.join("bin");
    let state = outside.join("state");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&state).unwrap();
    git(
        &outside,
        &["init", "-q", "--bare", "-b", "master", "origin.git"],
    );
    git(
        &root,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&root, &["branch", "-M", "master"]);
    git(&root, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
    git(&root, &["checkout", "-q", "-b", "feature/2"]);
    std::fs::write(root.join("two.txt"), "two\n").unwrap();
    git(&root, &["add", "two.txt"]);
    git(&root, &["commit", "-q", "-m", "two"]);
    git(
        &root,
        &[
            "push",
            "-q",
            "origin",
            "HEAD:refs/heads/feature/2",
            "HEAD:refs/pull/2/head",
        ],
    );
    git(&root, &["checkout", "-q", "master"]);
    git(
        &root,
        &["merge", "-q", "--no-ff", "feature/2", "-m", "two, landed"],
    );
    git(&root, &["push", "-q", "origin", "HEAD:refs/heads/master"]);
    git(&root, &["checkout", "-q", "-b", "feature/1"]);
    std::fs::write(root.join("one.txt"), "one\n").unwrap();
    git(&root, &["add", "one.txt"]);
    git(&root, &["commit", "-q", "-m", "one"]);
    git(
        &root,
        &[
            "push",
            "-q",
            "origin",
            "HEAD:refs/heads/feature/1",
            "HEAD:refs/pull/1/head",
        ],
    );
    let h1 = git(&root, &["rev-parse", "feature/1"]);
    let h2 = git(&root, &["rev-parse", "feature/2"]);
    git(&root, &["checkout", "-q", "master"]);
    let pr = |n: u64, head: &str, branch: &str, at: &str| {
        json!({
            "number": n, "title": format!("change {n}"), "author": {"login": "someone"},
            "headRefName": branch, "headRefOid": head, "baseRefName": "master",
            "isDraft": false, "labels": [], "createdAt": at, "updatedAt": at, "body": "",
            "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci",
                                   "status": "COMPLETED", "conclusion": "SUCCESS"}],
            "reviewDecision": "", "autoMergeRequest": null, "isCrossRepository": false
        })
    };
    std::fs::write(
        state.join("prs.json"),
        json!([
            pr(1, &h1, "feature/1", "2026-09-01T00:00:00Z"),
            pr(2, &h2, "feature/2", "2026-09-02T00:00:00Z")
        ])
        .to_string(),
    )
    .unwrap();
    let script = format!(
        r#"#!/bin/sh
echo "$*" >> "{state}/log"
[ -n "$REFUSE" ] && {{ echo "refused" >&2; exit 1; }}
case "$1 $2" in
  "repo view") if [ -n "$GARBLE_VIEW" ]; then echo 'not a view'; else echo '{{"nameWithOwner":"o/r","defaultBranchRef":{{"name":"master"}}}}'; fi ;;
  "api repos/o/r") echo '{{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}}' ;;
  "api repos/o/r/commits/master") printf '{{"sha":"%s"}}\n' "$(git -C "{origin}" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection") echo '{{"required_status_checks":{{"contexts":["ci"]}}}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "pr list")
    case " $* " in
      *" --state closed "*|*" --state merged "*) echo '[]' ;;
      *"number,headRefOid,state,labels"*)
        # the proof's own snapshot: the second one closes the cycle
        n=$(cat "{state}/snapshots" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > "{state}/snapshots"
        # the opening snapshot reads origin before it lists: a ref made now is in the closing one
        if [ "$n" = 1 ] && [ "$MODE" = intrude ]; then
          git -C "{origin}" update-ref refs/heads/intruder "$(git -C "{origin}" rev-parse master)"
        fi
        if [ "$n" = 2 ]; then
          case "$MODE" in
            act) printf '{{"action":"merge_attempted"}}\n' >> "$TRAIL" ;;
            move) git -C "{origin}" update-ref refs/pull/1/head "$(git -C "{origin}" rev-parse master)" ;;
            gone) git -C "{origin}" update-ref -d refs/pull/1/head ;;
            unreach) mv "{origin}" "{origin}.away" ;;
          esac
        fi
        if [ "${{REFUSE_SNAPSHOT_AT:-0}}" = "$n" ]; then echo "refused" >&2; exit 1; fi
        if [ "${{GARBLE_AT:-0}}" = "$n" ]; then echo 'not a list'; else cat "{state}/prs.json"; fi ;;
      *)
        # a refresh's listing: REFUSE_REFRESH_AT=N refuses the Nth
        r=$(cat "{state}/refreshes" 2>/dev/null || echo 0); r=$((r + 1)); echo $r > "{state}/refreshes"
        if [ "${{REFUSE_REFRESH_AT:-0}}" = "$r" ]; then echo "refused" >&2; exit 1; fi
        cat "{state}/prs.json" ;;
    esac ;;
  *) echo "WRITE-OR-UNEXPECTED" >> "{state}/log"; exit 1 ;;
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
        f,
        origin,
        bin,
        state,
        h1,
        h2,
    }
}

fn path(fg: &Forge) -> String {
    format!(
        "{}:{}",
        fg.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

/// `majordomus prs prove-dry-run <args>` with `env` set for the scripted forge; the snapshot
/// counter starts again, so each proof has its own second snapshot.
fn proof(fg: &Forge, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let _ = std::fs::remove_file(fg.state.join("snapshots"));
    let _ = std::fs::remove_file(fg.state.join("refreshes"));
    let mut c = Command::new(common::BIN);
    c.arg("prs")
        .arg("prove-dry-run")
        .arg("--repo")
        .arg(fg.f.root())
        .args(args)
        .env("PATH", path(fg))
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE");
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().expect("majordomus runs");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn log(fg: &Forge) -> String {
    std::fs::read_to_string(fg.state.join("log")).unwrap_or_default()
}

fn origin_refs(fg: &Forge) -> String {
    git(
        &fg.origin,
        &["for-each-ref", "--format=%(objectname) %(refname)"],
    )
}

#[test]
fn a_quiet_forge_proves_that_nothing_moved() {
    let fg = forge();
    let before = origin_refs(&fg);
    // first proof: nothing observed yet, so the base is the forge's default branch
    let (code, out, err) = proof(&fg, &[], &[]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("nothing moved"), "{out}");
    let h1 = &fg.h1[..12];
    let h2 = &fg.h2[..12];
    assert!(
        out.lines()
            .any(|l| l.starts_with(&format!("  #1  {h1}  ready"))),
        "{out}"
    );
    assert!(
        out.lines()
            .any(|l| l.starts_with(&format!("  #2  {h2}  redundant"))),
        "{out}"
    );
    // second proof, as a document: the base now comes from the observation
    let (code, out, err) = proof(&fg, &["--format", "json"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let p: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(p["ok"], true);
    assert_eq!(p["base"], "master");
    let sections: Vec<&str> = p["before"]["sections"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(sections, ["remote", "forge", "trail", "lease", "local"]);
    assert_eq!(p["before"], p["after"]);
    assert_eq!(p["moved"], json!([]));
    assert_eq!(p["mirrors"], json!([]));
    let step = |name: &str| {
        p["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["step"] == name)
            .map(|s| s["summary"].as_str().unwrap().to_string())
            .unwrap_or_default()
    };
    assert!(step("drain --dry-run").contains("would merge #1"), "{p}");
    assert!(step("cleanup").contains("#2 would_close"), "{p}");
    assert!(step("plan").contains("the next merge is #1"), "{p}");
    // nothing reached the forge but reads, and origin is as it was
    let log = log(&fg);
    assert!(!log.contains("WRITE-OR-UNEXPECTED"), "{log}");
    assert!(
        !log.lines()
            .any(|l| l.starts_with("pr merge") || l.starts_with("pr close")),
        "{log}"
    );
    assert_eq!(origin_refs(&fg), before);
    assert!(!fg
        .f
        .root()
        .join(".ai/local/state/integration/events.jsonl")
        .exists());
}

#[test]
fn a_forge_that_moves_during_the_cycle_is_caught_and_named() {
    let fg = forge();
    let (code, out, _) = proof(&fg, &[], &[("MODE", "intrude")]);
    assert_eq!(code, 10, "{out}");
    assert!(
        out.lines()
            .any(|l| l.trim_start().starts_with("+ remote") && l.ends_with("refs/heads/intruder")),
        "{out}"
    );
}

#[test]
fn a_mirror_that_is_not_what_origin_serves_fails_the_proof() {
    for mode in ["move", "gone"] {
        let fg = forge();
        let (code, out, err) = proof(&fg, &["--format", "json"], &[("MODE", mode)]);
        assert_eq!(code, 10, "{mode}: {out}{err}");
        let p: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(p["ok"], false);
        let mirrors = p["mirrors"].to_string();
        assert!(
            mirrors.contains("refs/majordomus/prs/1"),
            "{mode}: {mirrors}"
        );
        let want = if mode == "gone" {
            "origin serves nothing there"
        } else {
            "and origin serves"
        };
        assert!(mirrors.contains(want), "{mode}: {mirrors}");
    }
}

#[test]
fn a_forge_that_cannot_answer_fails_the_proof_rather_than_passing_it() {
    let fg = forge();
    let (code, out, err) = proof(&fg, &[], &[("REFUSE", "1")]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(err.contains("gh"), "{err}");
    let (code, out, err) = proof(&fg, &[], &[("GARBLE_AT", "1")]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(
        err.contains("did not answer a list of pull requests"),
        "{err}"
    );
}

#[test]
fn the_proof_answers_over_mcp_too() {
    let fg = forge();
    let mut child = Command::new(common::BIN)
        .arg("mcp")
        .env("MAJORDOMUS_SHARE", common::dist_share())
        .env("PATH", path(&fg))
        .current_dir(fg.f.root())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "0" } } })).unwrap();
        writeln!(stdin, "{}", json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "majordomus_pull_requests_prove_dry_run", "arguments": {} } })).unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let frames: Vec<Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let p = &frames[1]["result"]["structuredContent"];
    assert_eq!(p["ok"], true, "{}", frames[1]);
    assert_eq!(p["moved"], json!([]));
}

#[test]
fn a_garbled_closing_snapshot_fails_the_proof_too() {
    let fg = forge();
    let (code, out, err) = proof(&fg, &[], &[("GARBLE_AT", "2")]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(
        err.contains("did not answer a list of pull requests"),
        "{err}"
    );
}

#[test]
fn every_refresh_the_cycle_makes_is_a_read_whose_refusal_fails_the_proof() {
    let fg = forge();
    let mut n = 1;
    loop {
        let (code, out, err) = proof(&fg, &[], &[("REFUSE_REFRESH_AT", &n.to_string())]);
        if code == 0 {
            break;
        }
        assert_eq!(code, 12, "refusing refresh {n}: {out}{err}");
        n += 1;
        assert!(n < 20, "the cycle never stopped refreshing");
    }
    assert!(
        n > 2,
        "the cycle refreshes at least twice: observe, then drain and cleanup observe"
    );
}

/// The trail is the repository's, under the common git directory: an act appearing in it
/// while the cycle runs is caught, and observations the cycle records are not acts.
#[test]
fn an_act_on_the_repositorys_trail_during_the_cycle_is_caught() {
    let fg = forge();
    let trail = fg.f.root().join(".git/majordomus/integration/events.jsonl");
    let trail = trail.to_str().unwrap().to_string();
    // a trail with an act already in it, from a cleanup that took and gave back the lease
    let (code, out, err) = prs_cmd(&fg, &["cleanup", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        std::path::Path::new(&trail).exists(),
        "cleanup --apply left no trail at {trail}: {out}{err}\n{}",
        git(&fg.f.root(), &["rev-parse", "--git-common-dir"])
    );
    // and a lease record a crashed executor left behind: the dry cycle never takes the lease,
    // so the proof must find it unchanged
    let locks = fg.f.root().join(".git/majordomus/locks");
    std::fs::create_dir_all(&locks).unwrap();
    std::fs::write(locks.join("integration-master.lock"), "pid 1 host gone\n").unwrap();
    let (code, out, err) = proof(&fg, &["--format", "json"], &[("TRAIL", &trail)]);
    assert_eq!(
        code, 0,
        "observations alone must not fail the proof: {out}{err}"
    );
    let p: Value = serde_json::from_str(&out).unwrap();
    let section = |name: &str| {
        p["before"]["sections"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == name)
            .unwrap()["lines"]
            .to_string()
    };
    assert!(section("trail").contains("acts "), "{}", section("trail"));
    assert!(
        section("lease").contains("integration-"),
        "{}",
        section("lease")
    );
    let (code, out, err) = proof(&fg, &[], &[("MODE", "act"), ("TRAIL", &trail)]);
    assert_eq!(code, 10, "an act during the cycle passed: {out}{err}");
    assert!(out.contains("trail"), "{out}");
}

#[test]
fn an_empty_forge_has_nothing_to_merge_or_close() {
    let fg = forge();
    std::fs::write(fg.state.join("prs.json"), "[]").unwrap();
    let (code, out, err) = proof(&fg, &["--format", "json"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let p: Value = serde_json::from_str(&out).unwrap();
    let steps = p["steps"].to_string();
    assert!(steps.contains("nothing is ready to merge"), "{steps}");
    assert!(steps.contains("idle"), "{steps}");
    assert!(steps.contains("nothing to close"), "{steps}");
}

fn prs_cmd(fg: &Forge, args: &[&str]) -> (i32, String, String) {
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

#[test]
fn every_read_the_proof_cannot_take_fails_it() {
    // the default branch unnamed, a snapshot's own listing refused
    for env in [
        ("GARBLE_VIEW", "1"),
        ("REFUSE_SNAPSHOT_AT", "1"),
        ("REFUSE_SNAPSHOT_AT", "2"),
    ] {
        let fg = forge();
        let (code, out, err) = proof(&fg, &[], &[env]);
        assert_eq!(code, 12, "{env:?}: {out}{err}");
    }
    // origin unreachable before the cycle, and gone by the time the mirrors are judged
    let fg = forge();
    git(
        &fg.f.root(),
        &["remote", "set-url", "origin", "/nonexistent/origin.git"],
    );
    let (code, out, err) = proof(&fg, &[], &[]);
    assert_eq!(code, 12, "{out}{err}");
    let fg = forge();
    let (code, out, err) = proof(&fg, &[], &[("MODE", "unreach")]);
    assert_eq!(code, 12, "{out}{err}");
    // a trail that cannot be read is no proof of anything
    let fg = forge();
    std::fs::create_dir_all(fg.f.root().join(".git/majordomus/integration/events.jsonl")).unwrap();
    let (code, out, err) = proof(&fg, &[], &[]);
    assert_eq!(code, 12, "{out}{err}");
}

#[test]
fn a_lease_entry_that_cannot_be_read_is_named_unreadable_and_stays_so() {
    let fg = forge();
    let locks = fg.f.root().join(".git/majordomus/locks");
    std::fs::create_dir_all(locks.join("integration-master.lock")).unwrap();
    let (code, out, err) = proof(&fg, &["--format", "json"], &[]);
    assert_eq!(code, 0, "{out}{err}");
    let p: Value = serde_json::from_str(&out).unwrap();
    assert!(
        p["before"]
            .to_string()
            .contains("integration-master.lock unreadable"),
        "{}",
        p["before"]
    );
}
