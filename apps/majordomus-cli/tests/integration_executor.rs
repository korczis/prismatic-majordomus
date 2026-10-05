//! The executor through the real command line (ADR 0101): observe, decide, merge, prove what
//! landed, and refuse what it cannot prove — against a bare origin and a scripted `gh`.
//!
//! The forge answers from files under its state directory, logs every call, and merges for
//! real: a merge is a merge commit pushed to the bare origin, so what the executor proves after
//! it is proved from git, as it is against GitHub. What the forge does on a merge is a file
//! too (`merge-mode`): merge and answer, merge and lose the answer, or let somebody else's
//! change land first. The cases under test/cases drive the same paths from a shell; this file
//! drives them from the test harness, so the executable that runs them is the instrumented
//! one and the paths it takes are measured.

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

/// The scripted forge. `@STATE@` and `@ORIGIN@` are replaced with the test's paths.
const GH: &str = r#"#!/bin/sh
S="@STATE@"; O="@ORIGIN@"
echo "$*" >> "$S/log"
case "$1 $2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "$(git -C "$O" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection")
    if [ -f "$S/protection.json" ]; then cat "$S/protection.json"
    else echo 'gh: Branch not protected (HTTP 404)' >&2; exit 1; fi ;;
  "api repos/o/r/rules/branches/master")
    if grep -q FAIL "$S/rules.json" 2>/dev/null; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
    cat "$S/rules.json" 2>/dev/null || echo '[]' ;;
  "pr list")
    case " $* " in
      *" --state closed "*) echo '[]' ;;
      *) jq -c --slurpfile gone "$S/gone.json" '[.[] | select(.number as $n | ($gone[0] | index($n)) | not)]' "$S/prs.json" ;;
    esac ;;
  "pr merge")
    n="$3"; mode="$(cat "$S/merge-mode" 2>/dev/null || echo ok)"
    if [ "$mode" = foreign ]; then
      t="$(mktemp -d)"; git clone -q "$O" "$t/f" 2>/dev/null
      (cd "$t/f" && echo foreign > foreign.txt && git -c user.email=f@example.com -c user.name=f add foreign.txt \
        && git -c user.email=f@example.com -c user.name=f commit -qm foreign && git push -q origin HEAD:master)
    fi
    t="$(mktemp -d)"
    git clone -q "$O" "$t/c" 2>/dev/null && cd "$t/c" && git fetch -q origin "refs/pull/$n/head" \
      && git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #$n" \
      && git push -q origin HEAD:master || exit 1
    jq -c --argjson n "$n" '. + [$n]' "$S/gone.json" > "$S/gone.tmp" && mv "$S/gone.tmp" "$S/gone.json"
    touch "$S/merged-$n"
    if [ "$mode" = lost ]; then echo 'Post "https://api.github.com/graphql": net/http: request canceled (Client.Timeout exceeded) timed out' >&2; exit 1; fi ;;
  "pr view")
    case " $* " in
      *" --json state,headRefOid "*)
        if [ -f "$S/closed-$3" ]; then echo "CLOSED x"
        elif [ -f "$S/moved-$3" ]; then echo "OPEN $(cat "$S/moved-$3")"
        else jq -r --argjson n "$3" '.[] | select(.number == $n) | "OPEN " + .headRefOid' "$S/prs.json"; fi ;;
      *) if [ -f "$S/merged-$3" ]; then echo MERGED; else echo OPEN; fi ;;
    esac ;;
  "pr close") touch "$S/closed-$3"
    jq -c --argjson n "$3" '. + [$n]' "$S/gone.json" > "$S/gone.tmp" && mv "$S/gone.tmp" "$S/gone.json" ;;
  *) echo UNEXPECTED >> "$S/log"; exit 1 ;;
esac
"#;

struct Forge {
    _tmp: tempfile::TempDir,
    origin: PathBuf,
    work: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

impl Forge {
    /// A bare origin with a base commit, a clone of it, and a forge with no pull request open.
    fn new() -> Forge {
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
        git(&work, &["push", "-q", "origin", "HEAD:master"]);
        let script = GH
            .replace("@STATE@", &state.display().to_string())
            .replace("@ORIGIN@", &origin.display().to_string());
        let gh = bin.join("gh");
        std::fs::write(&gh, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        std::fs::write(state.join("gone.json"), "[]").unwrap();
        std::fs::write(
            state.join("protection.json"),
            r#"{"required_status_checks":{"strict":true,"contexts":["ci"]}}"#,
        )
        .unwrap();
        let f = Forge {
            _tmp: tmp,
            origin,
            work,
            bin,
            state,
        };
        f.open(&[]);
        f
    }

    /// A branch from the current master with one commit, pushed as pull request `n`'s head.
    fn branch(&self, n: u64) -> String {
        git(&self.work, &["fetch", "-q", "origin"]);
        git(
            &self.work,
            &[
                "checkout",
                "-q",
                "-B",
                &format!("feature/{n}"),
                "origin/master",
            ],
        );
        std::fs::write(self.work.join(format!("{n}.txt")), format!("{n}\n")).unwrap();
        git(&self.work, &["add", &format!("{n}.txt")]);
        git(&self.work, &["commit", "-q", "-m", &format!("change {n}")]);
        git(
            &self.work,
            &[
                "push",
                "-q",
                "origin",
                &format!("HEAD:refs/heads/feature/{n}"),
                &format!("HEAD:refs/pull/{n}/head"),
            ],
        );
        let head = git(&self.work, &["rev-parse", "HEAD"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        head
    }

    /// The open pull requests: (number, head), each with a passed `ci`.
    fn open(&self, prs: &[(u64, &str)]) {
        let list: Vec<Value> = prs
            .iter()
            .map(|(n, head)| {
                serde_json::json!({
                    "number": n, "title": format!("change {n}"), "author": {"login": "someone"},
                    "headRefName": format!("feature/{n}"), "headRefOid": head, "baseRefName": "master",
                    "isDraft": false, "labels": [], "createdAt": format!("2026-09-0{n}T00:00:00Z"),
                    "updatedAt": format!("2026-09-0{n}T00:00:00Z"), "body": "",
                    "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
                        "conclusion": "SUCCESS", "completedAt": "2026-09-01T00:05:00Z"}],
                    "reviewDecision": "", "latestReviews": [], "reviewRequests": [],
                    "autoMergeRequest": null, "isCrossRepository": false
                })
            })
            .collect();
        std::fs::write(
            self.state.join("prs.json"),
            serde_json::to_string(&list).unwrap(),
        )
        .unwrap();
    }

    fn set(&self, file: &str, text: &str) {
        std::fs::write(self.state.join(file), text).unwrap();
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.state.join("log")).unwrap_or_default()
    }

    /// `majordomus prs --repo <work> <args>`, with the scripted forge first on the PATH.
    fn prs(&self, args: &[&str]) -> (i32, String, String) {
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let out = Command::new(common::BIN)
            .arg("prs")
            .arg("--repo")
            .arg(&self.work)
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

    fn events(&self) -> Vec<Value> {
        let (code, out, err) = self.prs(&["events", "--format", "json"]);
        assert_eq!(code, 0, "events: {err}");
        serde_json::from_str::<Value>(&out)
            .expect("events as JSON")
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    fn last(&self, action: &str) -> Option<Value> {
        self.events()
            .into_iter()
            .rev()
            .find(|e| e["action"] == action)
    }
}

#[test]
fn a_ready_pull_request_is_merged_proved_and_recorded() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    let (code, _, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {err}");

    let (code, out, err) = f.prs(&["status", "--format", "json"]);
    assert_eq!(code, 0, "status: {out}{err}");
    let q: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(q["next_merge"], 1, "{out}");
    assert_eq!(q["policy"]["up_to_date_required"], true, "{out}");
    let (code, out, _) = f.prs(&["status"]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = f.prs(&["plan"]);
    assert_eq!(code, 0, "{out}");
    let (code, out, _) = f.prs(&["explain", "1"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("rank factors:"), "{out}");
    let (code, _, _) = f.prs(&["explain", "9"]);
    assert_ne!(code, 0, "an unknown pull request is not explained");

    // a dry run decides and changes nothing
    let (code, out, err) = f.prs(&["drain", "--dry-run"]);
    assert_eq!(code, 0, "dry run: {out}{err}");
    assert!(!f.log().contains("pr merge"), "a dry run merged");

    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    assert!(out.contains("merged #1"), "{out}");
    let merged = f
        .last("merge_succeeded")
        .expect("merge_succeeded on the trail");
    let commit = merged["merge_commit"].as_str().expect("merge_commit");
    assert_eq!(git(&f.origin, &["rev-parse", &format!("{commit}^2")]), head);
    assert_eq!(git(&f.origin, &["rev-parse", "master"]), commit);

    let (code, out, _) = f.prs(&["brief"]);
    assert_eq!(code, 0);
    assert!(out.contains("#1"), "the brief names the last merge: {out}");
}

#[test]
fn a_merge_whose_answer_was_lost_is_found_landed() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    f.set("merge-mode", "lost");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    let merged = f
        .last("merge_succeeded")
        .expect("merge_succeeded on the trail");
    assert!(
        merged["detail"]
            .as_str()
            .unwrap()
            .contains("answer was lost"),
        "{merged}"
    );
    assert_eq!(
        f.log().matches("pr merge 1 ").count(),
        1,
        "the merge was asked twice"
    );
}

#[test]
fn a_merge_onto_a_master_that_moved_holds_the_next_drain_until_acknowledged() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    f.set("merge-mode", "foreign");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 10, "an unproved merge is a finding: {out}{err}");
    let failed = f.last("verification_failed").expect("verification_failed");
    assert!(
        failed["detail"]
            .as_str()
            .unwrap()
            .contains("unexpected_master"),
        "{failed}"
    );

    // the next drain merges nothing and says how to go on
    let head2 = f.branch(2);
    f.open(&[(2, &head2)]);
    f.set("merge-mode", "ok");
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("--resume-after-failure"), "{out}");
    assert!(!f.log().contains("pr merge 2 "), "a held drain merged");

    let (code, out, err) = f.prs(&["drain", "--max", "1", "--resume-after-failure"]);
    assert_eq!(code, 0, "resumed: {out}{err}");
    assert!(f.last("failure_acknowledged").is_some());
    assert!(out.contains("merged #2"), "{out}");
}

#[test]
fn a_closure_of_a_head_that_moved_is_refused_by_the_forge() {
    let f = Forge::new();
    // #3's head is master's own commit: its work is on master, so cleanup would close it
    let master = git(&f.origin, &["rev-parse", "master"]);
    git(&f.work, &["push", "-q", "origin", "HEAD:refs/pull/3/head"]);
    f.open(&[(3, &master)]);
    let (code, out, err) = f.prs(&["cleanup", "--dry-run", "--format", "json"]);
    assert_eq!(code, 0, "listing: {out}{err}");
    assert!(out.contains("would_close"), "{out}");
    // a person pushes between the decision and the closure: the forge's head is another one
    f.set("moved-3", "0123456789012345678901234567890123456789");
    let (code, out, err) = f.prs(&["cleanup", "--apply"]);
    assert_eq!(code, 0, "cleanup: {out}{err}");
    assert!(out.contains("close_failed"), "{out}");
    assert!(!f.log().contains("pr close 3"), "a moved head was closed");
    let failed = f.last("close_failed").expect("close_failed on the trail");
    assert!(
        failed["detail"].as_str().unwrap().contains("head moved"),
        "{failed}"
    );
}

#[test]
fn what_the_base_requires_is_read_from_protection_and_rulesets() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);

    // a ruleset binds a second check to an app; the run reports it, so it passes
    f.set(
        "rules.json",
        r#"[{"type":"required_status_checks","parameters":{"strict_required_status_checks_policy":true,"required_status_checks":[{"context":"ci","integration_id":15368}]}}]"#,
    );
    f.prs(&["refresh"]);
    let (_, out, _) = f.prs(&["status", "--format", "json"]);
    let q: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(q["policy"]["up_to_date_required"], true, "{out}");

    // an unprotected base requires nothing, and so nothing is ready (D5)
    std::fs::remove_file(f.state.join("protection.json")).unwrap();
    f.set("rules.json", "[]");
    f.prs(&["refresh"]);
    let (code, out, _) = f.prs(&["status", "--format", "json"]);
    assert_eq!(code, 10, "{out}");
    let q: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(q["next_merge"], Value::Null);
    assert!(out.contains("no_required_checks"), "{out}");

    // a ruleset read the forge refuses leaves the requirement unread
    f.set(
        "protection.json",
        r#"{"required_status_checks":{"contexts":["ci"]}}"#,
    );
    f.set("rules.json", "FAIL");
    f.prs(&["refresh"]);
    let (code, out, _) = f.prs(&["status", "--format", "json"]);
    assert_eq!(code, 10, "{out}");
    assert!(out.contains("unread"), "{out}");
    assert!(!f.log().contains("UNEXPECTED"), "{}", f.log());
}
