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

/// The scripted forge. `@STATE@`, `@ORIGIN@` and `@WORK@` are replaced with the test's paths.
/// A file under the state directory changes an answer: `down` refuses every call,
/// `protection.json` holding `FAIL` or `TRANSIENT` refuses the protection read, `closed-fails`
/// the list of closed pull requests, `view-fails` every `pr view`, `view-transient` the view
/// of a successor; `view-<n>.json` is the successor `n` as the forge shows it; `forget` and
/// `unfetchable` lose the observation or the origin as a merge is looked at.
const GH: &str = r#"#!/bin/sh
S="@STATE@"; O="@ORIGIN@"; W="@WORK@"
echo "$*" >> "$S/log"
if [ -f "$S/down" ]; then echo 'HTTP 401: Bad credentials' >&2; exit 1; fi
case "$1 $2" in
  "repo view") echo '{"nameWithOwner":"o/r","defaultBranchRef":{"name":"master"}}' ;;
  "api repos/o/r") echo '{"allow_merge_commit":true,"allow_squash_merge":true,"allow_rebase_merge":false}' ;;
  "api repos/o/r/commits/master") printf '{"sha":"%s"}\n' "$(git -C "$O" rev-parse master)" ;;
  "api repos/o/r/branches/master/protection")
    if grep -q TRANSIENT "$S/protection.json" 2>/dev/null; then echo 'gh: HTTP 503: Service Unavailable' >&2; exit 1
    elif grep -q FAIL "$S/protection.json" 2>/dev/null; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1
    elif [ -f "$S/protection.json" ]; then cat "$S/protection.json"
    else echo 'gh: Branch not protected (HTTP 404)' >&2; exit 1; fi ;;
  "api repos/o/r/rules/branches/master")
    if grep -q FAIL "$S/rules.json" 2>/dev/null; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
    cat "$S/rules.json" 2>/dev/null || echo '[]' ;;
  "pr list")
    case " $* " in
      *" --state closed "*)
        if [ -f "$S/closed-fails" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
        echo '[]' ;;
      *) jq -c --slurpfile gone "$S/gone.json" '[.[] | select(.number as $n | ($gone[0] | index($n)) | not)]' "$S/prs.json" ;;
    esac ;;
  "pr merge")
    n="$3"; mode="$(cat "$S/merge-mode" 2>/dev/null || echo ok)"
    if [ "$mode" = silent ]; then exit 1; fi
    if [ "$mode" = noop ]; then exit 0; fi
    if [ "$mode" = foreign ]; then
      t="$(mktemp -d)"; git clone -q "$O" "$t/f" 2>/dev/null
      (cd "$t/f" && echo "foreign before $n" >> foreign.txt && git -c user.email=f@example.com -c user.name=f add foreign.txt \
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
    if [ -f "$S/view-fails" ]; then echo 'HTTP 403: Resource not accessible by integration' >&2; exit 1; fi
    case " $* " in
      *" --json number,state,headRefOid,body "*)
        if [ -f "$S/view-transient" ]; then echo 'HTTP 502: Bad Gateway' >&2; exit 1
        elif [ -f "$S/view-$3.json" ]; then cat "$S/view-$3.json"
        else echo "no pull requests found for #$3" >&2; exit 1; fi ;;
      *" --json state,headRefOid "*)
        if [ -f "$S/closed-$3" ]; then echo "CLOSED x"
        elif [ -f "$S/moved-$3" ]; then echo "OPEN $(cat "$S/moved-$3")"
        else jq -r --argjson n "$3" '.[] | select(.number == $n) | "OPEN " + .headRefOid' "$S/prs.json"; fi ;;
      *) if [ -f "$S/forget" ]; then rm -f "$W/.ai/local/state/integration/observation.json"; fi
         if [ -f "$S/unfetchable" ] && [ -d "$O" ]; then mv "$O" "$O.away"; fi
         if [ -f "$S/merged-$3" ]; then echo MERGED; else echo OPEN; fi ;;
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
            .replace("@ORIGIN@", &origin.display().to_string())
            .replace("@WORK@", &work.display().to_string());
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

    /// Pull request `n`'s body, as the forge lists it.
    fn body(&self, n: u64, text: &str) {
        let path = self.state.join("prs.json");
        let mut list: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for p in list.iter_mut().filter(|p| p["number"] == n) {
            p["body"] = Value::from(text);
        }
        std::fs::write(&path, serde_json::to_string(&list).unwrap()).unwrap();
    }

    /// Master moves on: a commit of somebody else's lands on origin.
    fn advance(&self) {
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        std::fs::write(self.work.join("later.txt"), "later\n").unwrap();
        git(&self.work, &["add", "later.txt"]);
        git(&self.work, &["commit", "-q", "-m", "later"]);
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
    }

    /// One merge, asked of a forge that answers `as`; the trail's last word on it.
    fn merge_once(&self, how: &str) -> (i32, Value) {
        let head = self.branch(1);
        self.open(&[(1, &head)]);
        self.set(how, "");
        let (code, out, err) = self.prs(&["drain", "--max", "1"]);
        let failed = self
            .last("verification_failed")
            .unwrap_or_else(|| panic!("{how}: nothing failed: {out}{err}"));
        (code, failed)
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
    // a person reading the plan is told why it is not a full answer
    let (code, _, err) = f.prs(&["plan"]);
    assert_eq!(code, 10);
    assert!(err.contains("! "), "{err}");

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

#[test]
fn a_forge_that_cannot_be_read_leaves_every_executor_command_unusable() {
    let f = Forge::new();
    f.set("down", "");
    for args in [
        &["refresh"][..],
        &["drain", "--max", "1"],
        &["drain", "--continuous"],
        &["cleanup", "--apply"],
        &["prove-dry-run"],
    ] {
        let (code, out, err) = f.prs(args);
        assert_eq!(code, 12, "{args:?}: {out}{err}");
        assert!(err.contains("Bad credentials"), "{args:?}: {err}");
    }
    // observed once: a drain that cannot observe again is unusable too
    std::fs::remove_file(f.state.join("down")).unwrap();
    let (code, _, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{err}");
    f.set("down", "");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 12, "{out}{err}");
    // and an observation that cannot be read names no base to take the lease of
    std::fs::remove_file(f.state.join("down")).unwrap();
    std::fs::write(
        f.work.join(".ai/local/state/integration/observation.json"),
        "{",
    )
    .unwrap();
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(err.contains("observation.json"), "{err}");
}

#[test]
fn a_second_executor_is_refused_the_lease() {
    let f = Forge::new();
    let (code, _, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{err}");
    let dir = f.work.join(".git/majordomus/locks");
    std::fs::create_dir_all(&dir).unwrap();
    let held = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join("integration-master.lock"))
        .unwrap();
    held.try_lock().expect("the test holds the lease");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(err.contains("another integration executor"), "{err}");
    drop(held);
}

#[test]
fn a_continuous_drain_stops_on_a_merge_it_cannot_prove_and_resumes_when_told() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    f.set("merge-mode", "foreign");
    let (code, out, err) = f.prs(&["drain", "--continuous"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("cycle 1:"), "{out}");
    assert!(out.contains("could not be verified"), "{out}");
    // resumed: the failure is acknowledged under the lease, and the next one is merged —
    // onto a master that moves again, so this run ends the same way
    let head = f.branch(2);
    f.open(&[(2, &head)]);
    let (code, out, err) = f.prs(&["drain", "--continuous", "--resume-after-failure"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(f.last("failure_acknowledged").is_some());
    assert!(f.log().contains("pr merge 2 "), "{}", f.log());
}

#[test]
fn a_merge_refused_without_a_word_is_still_named() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    f.set("merge-mode", "silent");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "{out}{err}");
    let failed = f.last("merge_failed").expect("merge_failed on the trail");
    assert!(
        failed["detail"].as_str().unwrap().contains("said nothing"),
        "{failed}"
    );
}

#[test]
fn a_merge_that_cannot_be_looked_at_is_not_proved() {
    for (how, said) in [
        ("view-fails", "the forge could not be asked"),
        ("forget", "no observation to verify against"),
        ("unfetchable", "git fetch of the base failed"),
    ] {
        let f = Forge::new();
        let (code, failed) = f.merge_once(how);
        assert_eq!(code, 10, "{how}");
        assert!(
            failed["detail"].as_str().unwrap().contains(said),
            "{how}: {failed}"
        );
    }
}

#[test]
fn a_merge_the_forge_never_shows_is_not_merged() {
    let f = Forge::new();
    f.set("merge-mode", "noop");
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 10, "{out}{err}");
    let failed = f.last("verification_failed").expect("verification_failed");
    assert!(
        failed["detail"]
            .as_str()
            .unwrap()
            .contains("still says OPEN"),
        "{failed}"
    );
}

#[test]
fn a_closure_the_forge_cannot_confirm_is_not_made() {
    for (how, said) in [
        ("closed-3", "is CLOSED on the forge, not open"),
        ("view-fails", "HTTP 403"),
    ] {
        let f = Forge::new();
        let master = git(&f.origin, &["rev-parse", "master"]);
        git(&f.work, &["push", "-q", "origin", "HEAD:refs/pull/3/head"]);
        f.open(&[(3, &master)]);
        f.set(how, "");
        let (code, out, err) = f.prs(&["cleanup", "--apply"]);
        assert_eq!(code, 0, "{how}: {out}{err}");
        let failed = f.last("close_failed").expect("close_failed on the trail");
        assert!(
            failed["detail"].as_str().unwrap().contains(said),
            "{how}: {failed}"
        );
        assert!(!f.log().contains("pr close 3"), "{how}: closed anyway");
    }
}

#[test]
fn a_successor_an_open_body_names_is_read_and_one_the_forge_cannot_show_is_unread() {
    let f = Forge::new();
    let head = f.branch(1);
    let successor = f.branch(7);
    f.open(&[(1, &head)]);
    f.body(1, "Superseded by #7");
    f.set(
        "view-7.json",
        &serde_json::json!({"number": 7, "state": "CLOSED", "headRefOid": successor, "body": ""})
            .to_string(),
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        git(&f.work, &["rev-parse", "refs/majordomus/prs/7"]),
        successor,
        "the successor's head was not fetched"
    );
    // a successor the forge says nothing of is unread, and the observation stands
    f.body(1, "Superseded by #8");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let (_, out, _) = f.prs(&["status", "--format", "json"]);
    assert!(out.contains("successor_unread:#8"), "{out}");
    // an outage that outlasts the retries fails the observation
    f.set("view-transient", "");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
}

#[test]
fn a_base_whose_requirements_cannot_be_read_is_unread_or_unobserved() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    // a protection the forge refuses to show leaves the requirement unread
    f.set("protection.json", "FAIL");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let (_, out, _) = f.prs(&["status", "--format", "json"]);
    let q: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(q["policy"]["required_checks"], Value::Null, "{out}");
    // the closed pull requests that declare a supersession cannot be listed
    f.set(
        "protection.json",
        r#"{"required_status_checks":{"contexts":["ci"]}}"#,
    );
    f.set("closed-fails", "");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
    std::fs::remove_file(f.state.join("closed-fails")).unwrap();
    // a protection read the forge keeps failing fails the observation
    f.set("protection.json", "TRANSIENT");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
}

#[test]
fn bringing_master_into_a_branch_that_cannot_derive_is_a_refresh_failure() {
    let f = Forge::new();
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["drain", "--max", "1", "--refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let failed = f
        .last("refresh_failed")
        .expect("refresh_failed on the trail");
    assert!(
        failed["detail"]
            .as_str()
            .unwrap()
            .contains("scripts/derive"),
        "{failed}"
    );
    assert!(
        !f.log().contains("pr merge"),
        "a branch behind master merged"
    );
}

#[test]
fn the_dry_run_proof_says_what_it_compared() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["prove-dry-run"]);
    assert!(code == 0 || code == 10, "{out}{err}");
    assert!(
        out.contains("observed classification (base master):"),
        "{out}"
    );
    let (_, out, _) = f.prs(&["prove-dry-run", "--format", "json"]);
    let proof: Value = serde_json::from_str(&out).expect("the proof as JSON");
    assert_eq!(proof["base"], "master");
}
