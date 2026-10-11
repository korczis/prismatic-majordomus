//! The record pull request the executor reopens (I2296), through the real command line.
//!
//! A release proposes its record as `release/record-<tag>`, opened with the workflow's own
//! token. A forge starts no workflow from an event that token caused, so the pull request has
//! no check of its own and can never merge until a person closes and reopens it. `prs drain`
//! does that itself, once per head, with `reopen_attempted` on the trail first.
//!
//! The forge is a `gh` on the child's PATH that answers from this test's files and logs every
//! call. A file under its state directory changes an answer: `closed-<n>` is a closed pull
//! request, `moved-<n>` holds the head the forge shows instead, `view-fails` refuses the
//! detailed view, `close-fails` and `reopen-fails` refuse those acts. The executable is the
//! instrumented one, so the forge-facing code is measured here and not only in a shell case.

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
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
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
  "api repos/o/r/branches/master/protection") echo '{"required_status_checks":{"strict":true,"contexts":["ci"]}}' ;;
  "api repos/o/r/rules/branches/master") echo '[]' ;;
  "api graphql")
    jq -c '{data: {repository: {pullRequests: {
      pageInfo: {hasNextPage: false, endCursor: null},
      nodes: [.[] | {number, authorAssociation: "OWNER", isCrossRepository: false, timelineItems: {
        pageInfo: {hasNextPage: false, endCursor: null}, nodes: []}}]}}}}' "$S/prs.json" ;;
  "pr list")
    if [ -f "$S/merged-1" ] || [ -f "$S/closed-1" ]; then echo '[]'; else cat "$S/prs.json"; fi ;;
  "pr view")
    n="$3"
    state=OPEN; [ -f "$S/closed-$n" ] && state=CLOSED; [ -f "$S/merged-$n" ] && state=MERGED
    case " $* " in
      *" --json state,headRefOid,author,createdAt,statusCheckRollup "*)
        if [ -f "$S/view-fails" ]; then echo 'HTTP 403: Resource not accessible by integration' >&2; exit 1; fi
        if [ -f "$S/view-garbled" ]; then echo 'not json'; exit 0; fi
        jq -c --argjson n "$n" --arg state "$state" --arg moved "$(cat "$S/moved-$n" 2>/dev/null)" \
          '.[] | select(.number == $n) | {state: $state,
            headRefOid: (if $moved == "" then .headRefOid else $moved end),
            author, createdAt, statusCheckRollup}' "$S/prs.json" ;;
      *" --json state,headRefOid "*)
        jq -r --argjson n "$n" --arg state "$state" '.[] | select(.number == $n) | $state + " " + .headRefOid' "$S/prs.json" ;;
      *) echo "$state" ;;
    esac ;;
  "pr close")
    if [ -f "$S/close-fails" ]; then echo 'HTTP 403: Resource not accessible by integration' >&2; exit 1; fi
    touch "$S/closed-$3" ;;
  "pr reopen")
    if [ -f "$S/reopen-fails" ]; then echo 'HTTP 403: Resource not accessible by integration' >&2; exit 1; fi
    rm -f "$S/closed-$3"; touch "$S/reopened-$3" ;;
  "pr merge")
    n="$3"; t="$(mktemp -d)"
    git clone -q "$O" "$t/c" 2>/dev/null && cd "$t/c" && git fetch -q origin "refs/pull/$n/head" \
      && git -c user.email=f@example.com -c user.name=forge merge -q --no-ff FETCH_HEAD -m "Merge pull request #$n" \
      && git push -q origin HEAD:master || exit 1
    touch "$S/merged-$n" ;;
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
    /// A bare origin with a base commit, a clone of it, and pull request 1 open from
    /// `release/record-v9.9.9`: opened by `author` at `created`, with `rollup` as its checks.
    fn with_record(author: &str, created: &str, rollup: Value) -> (Forge, String) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path().canonicalize().expect("canonical tempdir");
        let (origin, work) = (base.join("origin.git"), base.join("work"));
        let (bin, state) = (base.join("bin"), base.join("state"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&state).unwrap();
        git(
            &base,
            &["init", "-q", "--bare", "-b", "master", "origin.git"],
        );
        git(&base, &["clone", "-q", "origin.git", "work"]);
        git(&work, &["config", "user.email", "t@example.com"]);
        git(&work, &["config", "user.name", "t"]);
        std::fs::write(work.join("a.txt"), "base\n").unwrap();
        git(&work, &["add", "a.txt"]);
        git(&work, &["commit", "-q", "-m", "base"]);
        git(&work, &["push", "-q", "origin", "HEAD:master"]);
        // the record: one commit on master, pushed as the branch and as the pull request's head
        git(
            &work,
            &[
                "checkout",
                "-q",
                "-B",
                "release/record-v9.9.9",
                "origin/master",
            ],
        );
        std::fs::write(work.join("record.txt"), "v9.9.9\n").unwrap();
        git(&work, &["add", "record.txt"]);
        git(&work, &["commit", "-q", "-m", "record v9.9.9"]);
        git(
            &work,
            &[
                "push",
                "-q",
                "origin",
                "HEAD:refs/heads/release/record-v9.9.9",
                "HEAD:refs/pull/1/head",
            ],
        );
        let head = git(&work, &["rev-parse", "HEAD"]);
        git(&work, &["checkout", "-q", "--detach", "origin/master"]);
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
        let f = Forge {
            _tmp: tmp,
            origin,
            work,
            bin,
            state,
        };
        f.list(author, created, &head, rollup);
        (f, head)
    }

    /// A record the workflow's token opened long ago, with no check of its own.
    fn stranded() -> (Forge, String) {
        Forge::with_record(
            "app/github-actions",
            "2026-10-10T01:56:10Z",
            serde_json::json!([]),
        )
    }

    fn list(&self, author: &str, created: &str, head: &str, rollup: Value) {
        let list = serde_json::json!([{
            "number": 1, "title": "chore(release): record v9.9.9",
            "author": {"is_bot": author.starts_with("app/"), "login": author},
            "headRefName": "release/record-v9.9.9", "headRefOid": head, "baseRefName": "master",
            "isDraft": false, "labels": [], "createdAt": created, "updatedAt": created, "body": "",
            "statusCheckRollup": rollup, "reviewDecision": "", "latestReviews": [],
            "reviewRequests": [], "autoMergeRequest": null, "isCrossRepository": false
        }]);
        std::fs::write(
            self.state.join("prs.json"),
            serde_json::to_string(&list).unwrap(),
        )
        .unwrap();
    }

    fn set(&self, file: &str, text: &str) {
        std::fs::write(self.state.join(file), text).unwrap();
    }

    fn has(&self, file: &str) -> bool {
        self.state.join(file).exists()
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.state.join("log")).unwrap_or_default()
    }

    /// How many calls the forge logged that start with `prefix`.
    fn calls(&self, prefix: &str) -> usize {
        self.log().lines().filter(|l| l.starts_with(prefix)).count()
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
            .env_remove("GIT_INDEX_FILE")
            .output()
            .expect("majordomus runs");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn actions(&self) -> Vec<String> {
        let (code, out, err) = self.prs(&["events", "--format", "json"]);
        assert_eq!(code, 0, "events: {err}");
        serde_json::from_str::<Value>(&out)
            .expect("events as JSON")
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|e| e["action"].as_str().unwrap_or("").to_string())
            .collect()
    }
}

/// The current time as the forge writes one.
fn now_rfc3339() -> String {
    let out = Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .expect("date runs");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn a_stranded_record_is_reopened_once_and_merged_when_its_check_passes() {
    let (f, head) = Forge::stranded();

    // a dry run names the act and asks the forge for nothing but views
    let (code, out, err) = f.prs(&["drain", "--dry-run"]);
    assert_eq!(code, 0, "dry run: {out}{err}");
    assert!(out.contains("would close and reopen #1"), "{out}");
    assert_eq!(f.calls("pr close") + f.calls("pr reopen"), 0, "{}", f.log());

    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    assert!(out.contains("closed and reopened #1"), "{out}");
    assert!(f.has("reopened-1") && !f.has("closed-1"), "{}", f.log());
    let log = f.log();
    let (closed, reopened) = (log.find("pr close 1 --comment"), log.find("pr reopen 1"));
    assert!(closed.is_some() && closed < reopened, "{log}");
    assert!(
        log.contains("opened with the workflow's own token"),
        "{log}"
    );
    let actions = f.actions();
    let at = |a: &str| actions.iter().position(|x| x == a);
    assert!(at("lease_acquired") < at("reopen_attempted"), "{actions:?}");
    assert!(at("reopen_attempted") < at("reopened"), "{actions:?}");
    assert!(at("reopened") < at("lease_released"), "{actions:?}");

    // the same head is never reopened twice, however long its run takes to report
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(f.calls("pr reopen"), 1, "{}", f.log());

    // its own run reports: the record merges like any other pull request
    f.list(
        "app/github-actions",
        "2026-10-10T01:56:10Z",
        &head,
        serde_json::json!([{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
            "conclusion": "SUCCESS", "completedAt": "2026-10-10T03:30:00Z"}]),
    );
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    assert!(out.contains("merged #1"), "{out}");
    assert_eq!(
        git(&f.origin, &["rev-parse", "master^2"]),
        head,
        "the record's head is what landed"
    );
}

#[test]
fn a_record_in_any_other_state_is_left_alone() {
    // a person opened it: a run of its own exists or will
    let (f, _) = Forge::with_record("korczis", "2026-10-10T01:56:10Z", serde_json::json!([]));
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "{out}");
    assert!(f.calls("pr view 1 --json state,headRefOid,author") >= 1);
    assert_eq!(f.calls("pr close"), 0, "{}", f.log());

    // it was opened a moment ago: an empty rollup says nothing yet
    let (f, _) = Forge::with_record("app/github-actions", &now_rfc3339(), serde_json::json!([]));
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(f.calls("pr close"), 0, "{}", f.log());

    // a run was started for this head: it has checks of its own, the required one among the
    // ones still to come
    let (f, _) = Forge::with_record(
        "app/github-actions",
        "2026-10-10T01:56:10Z",
        serde_json::json!([{"__typename": "CheckRun", "name": "plan", "status": "IN_PROGRESS",
            "conclusion": ""}]),
    );
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(f.calls("pr close"), 0, "{}", f.log());

    // the forge cannot show it, shows something that is not a view, or shows another head
    for (file, text) in [
        ("view-fails", ""),
        ("view-garbled", ""),
        ("moved-1", "another"),
    ] {
        let (f, _) = Forge::stranded();
        f.set(file, text);
        let (code, out, _) = f.prs(&["drain", "--max", "1"]);
        assert_eq!(code, 0, "{file}: {out}");
        assert_eq!(f.calls("pr close"), 0, "{file}: {}", f.log());
        assert!(
            f.actions().iter().all(|a| !a.starts_with("reopen")),
            "{file}"
        );
    }
}

#[test]
fn a_reopen_the_forge_refuses_is_a_finding_and_says_what_was_left() {
    // the closure is refused: nothing changed, and the drain says so
    let (f, _) = Forge::stranded();
    f.set("close-fails", "");
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 10, "{out}");
    assert!(
        out.contains("#1: reopening failed (policy_violation)"),
        "{out}"
    );
    assert!(!f.has("closed-1"));
    assert_eq!(f.calls("pr reopen"), 0, "{}", f.log());

    // the reopen is refused after the closure: the record is closed, and the words name the
    // remedy a person runs
    let (f, _) = Forge::stranded();
    f.set("reopen-fails", "");
    let (code, out, _) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 10, "{out}");
    assert!(
        out.contains("was closed and could not be reopened") && out.contains("gh pr reopen 1"),
        "{out}"
    );
    assert!(f.has("closed-1"));
    let actions = f.actions();
    let at = |a: &str| actions.iter().position(|x| x == a);
    assert!(at("reopen_attempted") < at("reopen_failed"), "{actions:?}");
    assert!(!actions.iter().any(|a| a == "reopened"), "{actions:?}");
}
