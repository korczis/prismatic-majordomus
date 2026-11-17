//! The executor through the real command line (ADR 0101): observe, decide, merge, prove what
//! landed, and refuse what it cannot prove — against a bare origin and a scripted `gh`.
//!
//! The forge answers from files under its state directory, logs every call, and lands for
//! real: a fast-forward moves the bare origin's master to the head, refused unless master is
//! its ancestor, and a merge is a merge commit pushed there, so what the executor proves after
//! it is proved from git, as it is against GitHub. What the forge does on a landing is a file
//! too (`merge-mode`): land and answer, land and lose the answer, let somebody else's change
//! land right after (`foreign`), or, for a fast-forward, just before (`moved`). The cases under test/cases drive the same paths from a shell; this file
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
/// `protection.json` holding `FAIL` or `TRANSIENT` refuses the protection read, `view-fails`
/// every `pr view`, `view-transient` the view of a successor; `view-<n>.json` is the successor
/// `n` as the forge shows it; `forget` and `unfetchable` lose the observation or the origin as
/// a merge is looked at; `act` pushes a branch to origin, as somebody else would, while the
/// declarations are read.
///
/// Who may declare a successor, and what was declared, is an `api graphql` read too (the one
/// that asks for `timelineItems`), asked whenever a pull request is open: `declarations.json`
/// is its answer, and without one every listed pull request is its owner's and nothing
/// mentions it. `declarations-<n>.json` is the answer about pull request `n` alone, asked when
/// its cross-references did not fit a page or no page listed it; `declarations-fails`
/// refuses every such read, `declarations-page-fails` only the one of every open pull request,
/// `declarations-one-fails` only the one of a pull request alone, and
/// `declarations-transient` fails each as an outage would. Nothing lists closed pull
/// requests.
///
/// Who wrote each check is the `api graphql` read, asked only when the base binds a context
/// to an app: `writers.json` is its answer, and without one every listed pull request is
/// answered at its listed head with each check run written by app 15368. `writers-<n>.json`
/// is the answer about pull request `n` alone, asked when its contexts did not fit a page;
/// `writers-fails` refuses the read and `writers-transient` fails it as an outage would.
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
    if grep -q TRANSIENT "$S/rules.json" 2>/dev/null; then echo 'gh: HTTP 503: Service Unavailable' >&2; exit 1; fi
    if grep -q FAIL "$S/rules.json" 2>/dev/null; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
    cat "$S/rules.json" 2>/dev/null || echo '[]' ;;
  "api graphql")
    # the forge's deployments and releases, which `served forge` observes (since 3e8225a4329
    # not `prs refresh`; I4315)
    # (deploy::observed): none here, and no test of this file is about them
    case "$*" in *"deployments(last"*)
      echo '{"data":{"repository":{"url":"https://example.invalid/o/r","deployments":{"nodes":[]},"releases":{"nodes":[]}}}}'
      exit 0 ;;
    esac
    case "$*" in *timelineItems*)
      if [ -f "$S/declarations-transient" ]; then echo 'gh: HTTP 503: Service Unavailable' >&2; exit 1; fi
      if [ -f "$S/declarations-fails" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
      if [ -f "$S/act" ]; then git -C "$O" update-ref refs/heads/intruder refs/heads/master; fi
      one=""
      for a in "$@"; do case "$a" in number=*) one="${a#number=}" ;; esac; done
      if [ -z "$one" ] && [ -f "$S/declarations-page-fails" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
      if [ -n "$one" ] && [ -f "$S/declarations-one-fails" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
      if [ -n "$one" ]; then cat "$S/declarations-$one.json"
      elif [ -f "$S/declarations.json" ]; then cat "$S/declarations.json"
      else jq -c --slurpfile gone "$S/gone.json" '{data: {repository: {pullRequests: {
          pageInfo: {hasNextPage: false, endCursor: null},
          nodes: [.[] | select(.number as $n | ($gone[0] | index($n)) | not) | {
            number, authorAssociation: "OWNER", isCrossRepository: false, timelineItems: {
              pageInfo: {hasNextPage: false, endCursor: null}, nodes: []}}]}}}}' "$S/prs.json"
      fi
      exit $? ;;
    esac
    if [ -f "$S/writers-transient" ]; then echo 'gh: HTTP 503: Service Unavailable' >&2; exit 1; fi
    if [ -f "$S/writers-fails" ]; then echo 'gh: Resource not accessible by integration (HTTP 403)' >&2; exit 1; fi
    one=""
    for a in "$@"; do case "$a" in number=*) one="${a#number=}" ;; esac; done
    if [ -n "$one" ]; then cat "$S/writers-$one.json"
    elif [ -f "$S/writers.json" ]; then cat "$S/writers.json"
    else jq -c --slurpfile gone "$S/gone.json" '{data: {repository: {pullRequests: {
        pageInfo: {hasNextPage: false, endCursor: null},
        nodes: [.[] | select(.number as $n | ($gone[0] | index($n)) | not) | {
          number, headRefOid, statusCheckRollup: {contexts: {
            pageInfo: {hasNextPage: false, endCursor: null},
            nodes: [(.statusCheckRollup // [])[] | if .__typename == "CheckRun"
              then . + {checkSuite: {app: {databaseId: 15368}}} else . end]}}}]}}}}' "$S/prs.json"
    fi ;;
  "pr list")
    case " $* " in
      *" --state closed "*) echo UNEXPECTED >> "$S/log"; exit 1 ;;
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
  "api -X")
    # the fast-forward: master moved to the head, never forced, and refused as the forge does
    # when master is not that head's ancestor; the pull request at that head shows as merged
    [ "$3 $4" = "PATCH repos/o/r/git/refs/heads/master" ] || { echo UNEXPECTED >> "$S/log"; exit 1; }
    sha=""; for a in "$@"; do case "$a" in sha=*) sha="${a#sha=}" ;; esac; done
    mode="$(cat "$S/merge-mode" 2>/dev/null || echo ok)"
    if [ "$mode" = silent ]; then exit 1; fi
    if [ "$mode" = noop ]; then exit 0; fi
    foreign() {
      t="$(mktemp -d)"; git clone -q "$O" "$t/f" 2>/dev/null
      (cd "$t/f" && echo "foreign $sha" >> foreign.txt && git -c user.email=f@example.com -c user.name=f add foreign.txt \
        && git -c user.email=f@example.com -c user.name=f commit -qm foreign && git push -q origin HEAD:master)
    }
    if [ "$mode" = moved ]; then foreign; fi
    old="$(git -C "$O" rev-parse master)"
    git -C "$O" merge-base --is-ancestor "$old" "$sha" || { echo 'gh: Update is not a fast forward (HTTP 422)' >&2; exit 1; }
    git -C "$O" update-ref refs/heads/master "$sha" "$old" || exit 1
    for n in $(jq -r --arg s "$sha" '.[] | select(.headRefOid == $s) | .number' "$S/prs.json"); do
      jq -c --argjson n "$n" '. + [$n]' "$S/gone.json" > "$S/gone.tmp" && mv "$S/gone.tmp" "$S/gone.json"
      touch "$S/merged-$n"
    done
    if [ "$mode" = foreign ]; then foreign; fi
    if [ "$mode" = lost ]; then echo 'Patch "https://api.github.com/repos/o/r/git/refs/heads/master": net/http: request canceled (Client.Timeout exceeded) timed out' >&2; exit 1; fi
    echo "{\"object\":{\"sha\":\"$sha\"}}" ;;
  "pr view")
    if [ -f "$S/view-fails" ]; then echo 'HTTP 403: Resource not accessible by integration' >&2; exit 1; fi
    case " $* " in
      *" --json number,state,headRefOid,body,mergeCommit,baseRefName,author,isCrossRepository,changedFiles "*)
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
        // the person's identity, in the clone: the executor's own merges and commits run here,
        // and a machine with none configured (a CI runner) must not decide the outcome
        git(&work, &["config", "user.email", "t@example.com"]);
        git(&work, &["config", "user.name", "t"]);
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

    /// Pull request `n`'s `key`, as the forge lists it, is `value`.
    fn edit(&self, n: u64, key: &str, value: Value) {
        let path = self.state.join("prs.json");
        let mut list: Vec<Value> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for p in list.iter_mut().filter(|p| p["number"] == n) {
            p[key] = value.clone();
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

    /// The record that unlocks continuous mode (ADR 0101 §13): five merges earlier bounded
    /// drains made and proved, in the words the executor writes to the clone's one trail.
    fn seed_verified_merges(&self) {
        let trail = self.work.join(".git/majordomus/integration");
        std::fs::create_dir_all(&trail).unwrap();
        let merged = r#"{"at":"2026-10-05T00:00:00Z","actor":"seed","action":"merge_succeeded","pr":null,"reasons":[],"detail":"an earlier bounded merge"}"#;
        std::fs::write(trail.join("events.jsonl"), format!("{merged}\n").repeat(5)).unwrap();
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
        let out = common::majordomus()
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
    assert!(!f.log().contains("api -X"), "a dry run moved master");

    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    assert!(out.contains("merged #1"), "{out}");
    let merged = f
        .last("merge_succeeded")
        .expect("merge_succeeded on the trail");
    // a head that contains master is fast-forwarded to: master is that head, no merge commit
    let commit = merged["merge_commit"].as_str().expect("merge_commit");
    assert_eq!(commit, head);
    assert_eq!(git(&f.origin, &["rev-parse", "master"]), head);

    let (code, out, _) = f.prs(&["brief"]);
    assert_eq!(code, 0);
    assert!(out.contains("#1"), "the brief names the last merge: {out}");
}

/// A combined top whose head merges two members and contains master is ready while its
/// members are held by the label `hold`; a drain fast-forwards master to the top's head,
/// proves master is that head, and leaves the members redundant and open (I3910).
#[test]
fn a_green_train_is_fast_forwarded_and_verified() {
    let f = Forge::new();
    let one = f.branch(1);
    let two = f.branch(2);
    git(
        &f.work,
        &[
            "checkout",
            "-q",
            "-B",
            "integration/train-1",
            "origin/master",
        ],
    );
    git(
        &f.work,
        &["merge", "-q", "--no-ff", "-m", "train: #1", "feature/1"],
    );
    git(
        &f.work,
        &["merge", "-q", "--no-ff", "-m", "train: #2", "feature/2"],
    );
    git(
        &f.work,
        &[
            "push",
            "-q",
            "origin",
            "HEAD:refs/heads/integration/train-1",
            "HEAD:refs/pull/3/head",
        ],
    );
    let top = git(&f.work, &["rev-parse", "HEAD"]);
    git(&f.work, &["checkout", "-q", "--detach", "origin/master"]);
    let before = git(&f.origin, &["rev-parse", "master"]);
    f.open(&[(1, &one), (2, &two), (3, &top)]);
    f.edit(3, "headRefName", Value::from("integration/train-1"));
    for member in [1, 2] {
        f.edit(member, "labels", serde_json::json!([{"name": "hold"}]));
    }

    let (code, out, err) = f.prs(&["drain", "--max", "3"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    assert!(out.contains("merged #3"), "{out}");
    assert_eq!(git(&f.origin, &["rev-parse", "master"]), top);
    let log = f.log();
    assert!(
        log.contains(&format!(
            "api -X PATCH repos/o/r/git/refs/heads/master -f sha={top} -F force=false"
        )),
        "{log}"
    );
    assert!(
        !log.contains("pr merge"),
        "a merge commit was asked for: {log}"
    );
    assert!(!log.contains("--admin"), "{log}");
    let attempted = f.last("merge_attempted").expect("merge_attempted");
    assert_eq!(attempted["detail"], "--fast-forward");
    let merged = f.last("merge_succeeded").expect("merge_succeeded");
    assert_eq!(merged["pr"], 3);
    assert_eq!(merged["master_before"], Value::from(before));
    assert_eq!(merged["master_after"], Value::from(top.clone()));
    assert_eq!(merged["merge_commit"], Value::from(top));

    // the members the train carried are on master now: redundant, and nothing closed them
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {err}");
    let (code, out2, _) = f.prs(&["status", "--format", "json"]);
    assert_eq!(code, 0, "{out}{out2}");
    let q: Value = serde_json::from_str(&out2).unwrap();
    for member in [1, 2] {
        let a = q["assessments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["number"] == member)
            .unwrap_or_else(|| panic!("#{member} is open: {out2}"));
        assert_eq!(a["disposition"], "redundant", "{a}");
    }
    assert!(!f.log().contains("pr close"), "the drain closed something");
}

/// A head that carries master without containing it (its lower link landed by a merge commit)
/// is the one head still landed by a merge commit, pinned to that head, never a fast-forward.
#[test]
fn a_head_that_carries_master_is_landed_by_a_merge_commit() {
    let f = Forge::new();
    let one = f.branch(1);
    git(&f.work, &["checkout", "-q", "-B", "feature/2", &one]);
    std::fs::write(f.work.join("2.txt"), "2\n").unwrap();
    git(&f.work, &["add", "2.txt"]);
    git(&f.work, &["commit", "-q", "-m", "change 2"]);
    git(
        &f.work,
        &[
            "push",
            "-q",
            "origin",
            "HEAD:refs/heads/feature/2",
            "HEAD:refs/pull/2/head",
        ],
    );
    let two = git(&f.work, &["rev-parse", "HEAD"]);
    // #1 landed as a merge commit, as every landing did before the executor fast-forwarded
    git(&f.work, &["checkout", "-q", "--detach", "origin/master"]);
    git(&f.work, &["merge", "-q", "--no-ff", "-m", "Merge #1", &one]);
    git(&f.work, &["push", "-q", "origin", "HEAD:master"]);
    f.open(&[(2, &two)]);

    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    assert!(out.contains("merged #2"), "{out}");
    let log = f.log();
    assert!(
        log.contains(&format!("pr merge 2 --merge --match-head-commit {two}")),
        "{log}"
    );
    assert!(
        !log.contains("api -X"),
        "a carrier was fast-forwarded to: {log}"
    );
    let attempted = f.last("merge_attempted").expect("merge_attempted");
    assert_eq!(attempted["detail"], "--merge");
    assert_eq!(git(&f.origin, &["rev-parse", "master^2"]), two);
}

/// A fast-forward the forge refuses because master moved after the decision is no landing to
/// verify: it is a stale refusal on the trail, the drain decides again from the new master,
/// and the next drain is not held for a person.
#[test]
fn a_fast_forward_onto_a_master_that_moved_is_refused_and_decided_again() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    f.set("merge-mode", "moved");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "drain: {out}{err}");
    let failed = f.last("merge_failed").expect("merge_failed on the trail");
    assert_eq!(failed["class"], "stale", "{failed}");
    assert!(
        failed["detail"]
            .as_str()
            .unwrap()
            .contains("not a fast forward"),
        "{failed}"
    );
    assert!(f.last("verification_failed").is_none());
    assert_ne!(git(&f.origin, &["rev-parse", "master"]), head);
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(!out.contains("--resume-after-failure"), "{out}");
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
        f.log().matches(&format!("sha={head} ")).count(),
        1,
        "the fast-forward was asked twice"
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
    assert!(
        !f.log().contains(&format!("sha={head2} ")),
        "a held drain landed"
    );

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

    // a ruleset binds the check to an app: the refresh reads who wrote each check run, and
    // that app's run reports it, so it passes
    f.set(
        "rules.json",
        r#"[{"type":"required_status_checks","parameters":{"strict_required_status_checks_policy":true,"required_status_checks":[{"context":"ci","integration_id":15368}]}}]"#,
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let (code, out, _) = f.prs(&["status", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    let q: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(q["policy"]["up_to_date_required"], true, "{out}");
    assert_eq!(
        q["policy"]["required_checks"],
        serde_json::json!([{"context": "ci", "app_id": 15368}]),
        "{out}"
    );
    assert_eq!(q["assessments"][0]["disposition"], "ready", "{out}");
    assert_eq!(q["next_merge"], 1, "{out}");

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
    // and a program reading it finds them in the JSON, not on standard error
    let (code, out, err) = f.prs(&["plan", "--format", "json"]);
    assert_eq!(code, 10);
    assert!(!err.contains("! "), "{err}");
    assert!(out.contains("diagnostics"), "{out}");

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

/// A check the base binds to an app is read with its writer, through the command line: the
/// bound app's run passes it, another app's does not, a writer nobody read is unknown, and a
/// writer read that fails is a failed observation that leaves the recorded one as it was.
#[test]
fn a_bound_check_is_read_with_its_writer() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    let observation = f.work.join(".ai/local/state/integration/observation.json");
    let status = |f: &Forge| -> (i32, Value) {
        let (code, out, err) = f.prs(&["refresh"]);
        assert_eq!(code, 0, "refresh: {out}{err}");
        let (code, out, _) = f.prs(&["status", "--format", "json"]);
        (code, serde_json::from_str(&out).expect("status is JSON"))
    };
    let asked = |f: &Forge| {
        f.log()
            .lines()
            .filter(|l| l.starts_with("api graphql ") && l.contains("statusCheckRollup"))
            .count()
    };
    let writers = |head: &str, more: bool, app: Value| {
        serde_json::json!({"data": {"repository": {"pullRequests": {
            "pageInfo": {"hasNextPage": false, "endCursor": null},
            "nodes": [{"number": 1, "headRefOid": head, "statusCheckRollup": {"contexts": {
                "pageInfo": {"hasNextPage": more, "endCursor": "x1"},
                "nodes": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
                    "conclusion": "SUCCESS", "completedAt": "2026-09-01T00:05:00Z",
                    "checkSuite": {"app": app}}]}}}]}}}})
        .to_string()
    };

    // nothing bound: no writer is asked for, and the check run of the name is the check
    let (code, q) = status(&f);
    assert_eq!(
        (code, &q["assessments"][0]["disposition"]),
        (0, &Value::from("ready"))
    );
    assert_eq!(asked(&f), 0, "{}", f.log());

    // bound: one read, with the names as strings and only the page size typed
    f.set(
        "protection.json",
        r#"{"required_status_checks":{"strict":true,"contexts":["ci"],"checks":[{"context":"ci","app_id":15368}]}}"#,
    );
    let (code, q) = status(&f);
    assert_eq!(code, 0, "{q}");
    assert_eq!(q["assessments"][0]["disposition"], "ready", "{q}");
    assert_eq!(asked(&f), 1, "{}", f.log());
    let log = f.log();
    let call = log
        .lines()
        .find(|l| l.starts_with("api graphql ") && l.contains("statusCheckRollup"))
        .unwrap();
    assert!(call.contains(" -f owner=o -f name=r -F n=50"), "{call}");
    assert!(
        !call.contains("after="),
        "the first page has no cursor: {call}"
    );
    let recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(&observation).unwrap()).unwrap();
    assert_eq!(recorded["pull_requests"][0]["checks"][0]["app_id"], 15368);

    // another app's run of the name is not the check
    f.set(
        "writers.json",
        &writers(&head, false, serde_json::json!({"databaseId": 1})),
    );
    let (_, q) = status(&f);
    assert_eq!(
        q["assessments"][0]["disposition"], "waiting_for_checks",
        "{q}"
    );
    assert_eq!(q["assessments"][0]["required_checks"], "missing", "{q}");

    // the head moved between the list and the writer read: nobody read who wrote this head's
    // run, so it is unknown, the queue says so, and nothing is next
    let moved = "0123456789012345678901234567890123456789";
    f.set(
        "writers.json",
        &writers(moved, false, serde_json::json!({"databaseId": 15368})),
    );
    let (code, q) = status(&f);
    assert_eq!(code, 10, "{q}");
    assert_eq!(q["assessments"][0]["disposition"], "unknown", "{q}");
    assert_eq!(
        q["assessments"][0]["reasons"][0], "required_checks:unknown",
        "{q}"
    );
    assert_eq!(q["next_merge"], Value::Null, "{q}");
    let said = q["diagnostics"].to_string();
    assert!(said.contains("whose app was not read (#1)"), "{said}");
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert!(!f.log().contains("pr merge"), "{code}: {out}{err}");

    // contexts that did not fit a page are read again for that pull request alone, whole
    f.set(
        "writers.json",
        &writers(&head, true, serde_json::json!({"databaseId": 15368})),
    );
    f.set(
        "writers-1.json",
        &serde_json::json!({"data": {"repository": {"pullRequest": {
            "number": 1, "headRefOid": head, "statusCheckRollup": {"contexts": {
                "pageInfo": {"hasNextPage": false, "endCursor": null},
                "nodes": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
                    "conclusion": "SUCCESS", "completedAt": "2026-09-01T00:05:00Z",
                    "checkSuite": {"app": {"databaseId": 15368}}}]}}}}}})
        .to_string(),
    );
    let (code, q) = status(&f);
    assert_eq!(code, 0, "{q}");
    assert_eq!(q["assessments"][0]["disposition"], "ready", "{q}");
    assert!(
        f.log().contains(" -f owner=o -f name=r -F number=1"),
        "{}",
        f.log()
    );

    // a writer read the forge refuses fails the refresh and leaves the observation as it was
    let before = std::fs::read(&observation).unwrap();
    f.set("writers-fails", "");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(err.contains("Resource not accessible"), "{err}");
    assert_eq!(std::fs::read(&observation).unwrap(), before);
    std::fs::remove_file(f.state.join("writers-fails")).unwrap();
    // and so does one that keeps failing as an outage does
    f.set("writers-transient", "");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
    assert_eq!(std::fs::read(&observation).unwrap(), before);
    assert!(!f.log().contains("UNEXPECTED"), "{}", f.log());
}

#[test]
fn a_forge_that_cannot_be_read_leaves_every_executor_command_unusable() {
    let f = Forge::new();
    f.set("down", "");
    // continuous mode's record, so that it too reaches the forge it cannot read
    f.seed_verified_merges();
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
    f.seed_verified_merges();
    let (code, out, err) = f.prs(&["drain", "--continuous"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("cycle 1:"), "{out}");
    assert!(out.contains("could not be verified"), "{out}");
    // the merge it could not prove ended the record that unlocked continuous mode (ADR 0101
    // §13): resuming continuously is refused before the lease, acknowledging nothing
    let head = f.branch(2);
    f.open(&[(2, &head)]);
    let (code, out, err) = f.prs(&["drain", "--continuous", "--resume-after-failure"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(err.contains("verified merges"), "{err}");
    assert!(f.last("failure_acknowledged").is_none());
    assert!(!f.log().contains(&format!("sha={head} ")), "{}", f.log());
    // resumed by a bounded drain: the failure is acknowledged under the lease, and the next
    // one is merged — onto a master that moves again, so this run ends the same way
    let (code, out, err) = f.prs(&["drain", "--max", "1", "--resume-after-failure"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(f.last("failure_acknowledged").is_some());
    assert!(f.log().contains(&format!("sha={head} ")), "{}", f.log());
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
    // who may declare a successor, and what was declared, cannot be read
    f.set(
        "protection.json",
        r#"{"required_status_checks":{"contexts":["ci"]}}"#,
    );
    // fails the observation, as a failed list does
    f.set("declarations-fails", "");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(err.contains("Resource not accessible"), "{err}");
    std::fs::remove_file(f.state.join("declarations-fails")).unwrap();
    // a protection or a ruleset read the forge keeps failing fails the observation
    f.set("protection.json", "TRANSIENT");
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 12, "{out}{err}");
    f.set(
        "protection.json",
        r#"{"required_status_checks":{"contexts":["ci"]}}"#,
    );
    f.set("rules.json", "TRANSIENT");
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
        !f.log().contains("pr merge") && !f.log().contains("api -X"),
        "a branch behind master landed"
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
    // somebody pushes during the cycle: the proof names it and fails
    f.set("act", "");
    let (code, out, err) = f.prs(&["prove-dry-run"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("moved something"), "{out}");
}

/// One open pull request node of a declarations answer: its author's association and the
/// cross-references to it, with whether more of them follow.
fn declared(number: u64, association: &str, more: bool, events: Vec<Value>) -> Value {
    serde_json::json!({"number": number, "authorAssociation": association,
        "isCrossRepository": false, "timelineItems": {
        "pageInfo": {"hasNextPage": more, "endCursor": if more { Value::from("t1") } else { Value::Null }},
        "nodes": events}})
}

/// A declarations answer listing `nodes`, with no page of pull requests after it.
fn declarations_page(nodes: Vec<Value>) -> String {
    serde_json::json!({"data": {"repository": {"pullRequests": {
        "pageInfo": {"hasNextPage": false, "endCursor": null}, "nodes": nodes}}}})
    .to_string()
}

/// The answer about one pull request's cross-references alone.
fn references_page(node: Value) -> String {
    serde_json::json!({"data": {"repository": {"pullRequest": node}}}).to_string()
}

/// How many declarations reads the log holds, of every open one or of one alone.
fn declarations_asked(f: &Forge, of_one: bool) -> usize {
    f.log()
        .lines()
        .filter(|l| l.starts_with("api graphql ") && l.contains("timelineItems"))
        .filter(|l| l.contains(" -F number=") == of_one)
        .count()
}

fn status_of(f: &Forge) -> Value {
    let (_, out, err) = f.prs(&["status", "--format", "json"]);
    serde_json::from_str(&out).unwrap_or_else(|e| panic!("status is JSON ({e}): {out}{err}"))
}

#[test]
fn the_declarations_are_read_with_every_open_pull_request() {
    let f = Forge::new();
    // nothing open: nothing is asked about declarations
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        declarations_asked(&f, false) + declarations_asked(&f, true),
        0,
        "{}",
        f.log()
    );

    // #7 said it supersedes #1, and was merged with a merge commit; #1 is still open
    let seven = f.branch(7);
    git(&f.work, &["checkout", "-q", "--detach", "origin/master"]);
    git(
        &f.work,
        &[
            "merge",
            "-q",
            "--no-ff",
            &seven,
            "-m",
            "Merge pull request #7",
        ],
    );
    git(&f.work, &["push", "-q", "origin", "HEAD:master"]);
    let merge_commit = git(&f.work, &["rev-parse", "HEAD"]);
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(declarations_asked(&f, false), 1, "{}", f.log());
    assert_eq!(declarations_asked(&f, true), 0, "{}", f.log());
    let log = f.log();
    let call = log.lines().find(|l| l.contains("timelineItems")).unwrap();
    assert!(call.contains(" -f owner=o -f name=r -F n=50"), "{call}");
    assert!(
        !call.contains("after="),
        "the first page has no cursor: {call}"
    );
    assert!(
        !log.contains("pr list --state closed"),
        "no search of closed pull requests decides: {log}"
    );
    let q = status_of(&f);
    assert_eq!(q["assessments"][0]["disposition"], "ready", "{q}");
    let observation = f.work.join(".ai/local/state/integration/observation.json");
    let recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(&observation).unwrap()).unwrap();
    assert_eq!(recorded["schema"], 4);
    assert_eq!(recorded["pull_requests"][0]["author_association"], "OWNER");
    assert_eq!(recorded["pull_requests"][0]["cross_references"], "whole");

    // its cross-references name #7, an owner's pull request of this repository: it counts
    let source = |association: &str| {
        serde_json::json!({"isCrossRepository": false, "source": {
            "__typename": "PullRequest", "number": 7, "state": "MERGED", "body": "Supersedes #1",
            "headRefOid": seven, "mergeCommit": {"oid": merge_commit}, "isCrossRepository": false,
            "authorAssociation": association, "author": {"login": "ana"}, "baseRefName": "master",
            "changedFiles": 1}})
    };
    f.set(
        "declarations.json",
        &declarations_page(vec![declared(1, "OWNER", false, vec![source("OWNER")])]),
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let q = status_of(&f);
    assert_eq!(q["assessments"][0]["disposition"], "superseded", "{q}");
    assert_eq!(q["assessments"][0]["superseded_by"], 7, "{q}");
    assert_eq!(
        git(&f.work, &["rev-parse", "refs/majordomus/prs/7"]),
        seven,
        "the declarer's head was fetched, for git to say whether it landed"
    );
    // the same declaration by somebody who may not make one decides nothing
    f.set(
        "declarations.json",
        &declarations_page(vec![declared(
            1,
            "OWNER",
            false,
            vec![source("CONTRIBUTOR")],
        )]),
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let q = status_of(&f);
    assert_eq!(q["assessments"][0]["disposition"], "ready", "{q}");
    assert!(
        q["assessments"][0]["evidence"]
            .to_string()
            .contains("#7 by ana (CONTRIBUTOR) says it supersedes #1"),
        "{q}"
    );
    let (code, out, err) = f.prs(&["cleanup", "--apply"]);
    assert!(!f.log().contains("pr close"), "{code}: {out}{err}");
    assert!(!f.log().contains("UNEXPECTED"), "{}", f.log());
}

#[test]
fn a_timeline_that_does_not_fit_is_read_again_and_a_truncated_one_holds() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    // more pull requests mention #1 than its page holds
    f.set(
        "declarations.json",
        &declarations_page(vec![declared(1, "OWNER", true, vec![])]),
    );
    f.set(
        "declarations-1.json",
        &references_page(declared(1, "OWNER", false, vec![])),
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(declarations_asked(&f, true), 1, "{}", f.log());
    assert!(
        f.log().contains(" -f owner=o -f name=r -F number=1"),
        "{}",
        f.log()
    );
    let q = status_of(&f);
    assert_eq!(q["assessments"][0]["disposition"], "ready", "{q}");

    // and more than are read at all: it is held, and the queue says so
    f.set(
        "declarations-1.json",
        &references_page(declared(1, "OWNER", true, vec![])),
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(
        declarations_asked(&f, true),
        1 + 50,
        "fifty pages of its references, and no more: {}",
        f.log()
    );
    assert!(f.log().contains(" -F number=1 -f after=t1"), "{}", f.log());
    let q = status_of(&f);
    assert_eq!(q["assessments"][0]["disposition"], "unknown", "{q}");
    assert_eq!(
        q["assessments"][0]["reasons"][0], "declarations_unread",
        "{q}"
    );
    assert_eq!(q["next_merge"], Value::Null, "{q}");
    assert!(
        q["diagnostics"]
            .to_string()
            .contains("the pull requests that mention #1 were not all read (truncated: #1)"),
        "{q}"
    );
    let recorded: Value = serde_json::from_str(
        &std::fs::read_to_string(f.work.join(".ai/local/state/integration/observation.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        recorded["pull_requests"][0]["cross_references"],
        "truncated"
    );
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert!(!f.log().contains("pr merge"), "{code}: {out}{err}");
    let (code, out, err) = f.prs(&["cleanup", "--apply"]);
    assert!(!f.log().contains("pr close"), "{code}: {out}{err}");

    // a pull request no page lists, and the forge no longer shows alone, was not read either
    f.set("declarations.json", &declarations_page(vec![]));
    f.set(
        "declarations-1.json",
        r#"{"data": {"repository": {"pullRequest": null}}}"#,
    );
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let q = status_of(&f);
    assert_eq!(
        q["assessments"][0]["reasons"][0], "declarations_unread",
        "{q}"
    );
    assert!(
        q["assessments"][0]["evidence"]
            .to_string()
            .contains("references_unread"),
        "{q}"
    );
    assert!(!f.log().contains("UNEXPECTED"), "{}", f.log());
}

#[test]
fn a_declarations_read_that_fails_fails_the_refresh_and_replaces_no_observation() {
    let f = Forge::new();
    let (one, two) = (f.branch(1), f.branch(2));
    f.open(&[(1, &one), (2, &two)]);
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let observation = f.work.join(".ai/local/state/integration/observation.json");
    let before = std::fs::read(&observation).unwrap();
    // a refresh that fails: exit 12, saying what the forge said, and the observation recorded
    // before it is byte for byte what it was — never one with a part missing
    let fails = |f: &Forge, said: &str, when: &str| {
        let (code, out, err) = f.prs(&["refresh"]);
        assert_eq!(code, 12, "{when}: {out}{err}");
        assert!(err.contains(said), "{when}: {err}");
        assert_eq!(std::fs::read(&observation).unwrap(), before, "{when}");
    };

    // the page of every open one is refused: asked once, and no pull request is asked for
    // alone to stand in for it
    let (pages, alone) = (declarations_asked(&f, false), declarations_asked(&f, true));
    f.set("declarations-page-fails", "");
    fails(&f, "Resource not accessible", "the page refused");
    assert_eq!(declarations_asked(&f, false), pages + 1, "{}", f.log());
    assert_eq!(declarations_asked(&f, true), alone, "{}", f.log());
    std::fs::remove_file(f.state.join("declarations-page-fails")).unwrap();

    // a page said more mention #2 than it holds, and the forge refuses the rest: #1 was read
    // whole, and the refresh fails all the same
    f.set(
        "declarations.json",
        &declarations_page(vec![
            declared(1, "OWNER", false, vec![]),
            declared(2, "OWNER", true, vec![]),
        ]),
    );
    f.set("declarations-one-fails", "");
    fails(&f, "Resource not accessible", "#2's own read refused");
    assert_eq!(declarations_asked(&f, true), alone + 1, "{}", f.log());
    std::fs::remove_file(f.state.join("declarations-one-fails")).unwrap();
    std::fs::remove_file(f.state.join("declarations.json")).unwrap();

    // an outage that outlasts the retries fails it too
    f.set("declarations-transient", "");
    fails(&f, "503", "an outage");
    // and nothing is decided or done by an executor that cannot observe
    let (code, out, err) = f.prs(&["drain", "--max", "1"]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(!f.log().contains("pr merge"), "{code}: {out}{err}");
    let (code, out, err) = f.prs(&["cleanup", "--apply"]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(!f.log().contains("pr close"), "{code}: {out}{err}");
    assert_eq!(std::fs::read(&observation).unwrap(), before);
    std::fs::remove_file(f.state.join("declarations-transient")).unwrap();

    // answered again, the refresh is made
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "{out}{err}");
    let q = status_of(&f);
    assert_eq!(q["next_merge"], 1, "{q}");
    assert!(!f.log().contains("UNEXPECTED"), "{}", f.log());
}
