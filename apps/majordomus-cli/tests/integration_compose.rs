//! `majordomus prs compose` through the real command line (ADR 0114): several pull requests
//! merged, in order, into one new batch branch, against a bare origin and a scripted `gh`.
//!
//! The harness is the repair tests' (tests/integration_repair.rs), copied rather than shared as
//! that file copied the executor's, with one more answer: `pr create` records the head, title
//! and body it was asked to open and answers with a pull request's address. What the scripted
//! world of the unit tests cannot show is shown here: the merges git makes, the shape of the
//! branch that is pushed, the manifest that is committed, and that nothing else moves.

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
/// a merge is looked at. The declarations read (`api graphql`) answers that every listed pull
/// request is its owner's and that nothing mentions it.
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
  "api graphql") jq -c --slurpfile gone "$S/gone.json" '{data:{repository:{pullRequests:{pageInfo:{hasNextPage:false,endCursor:null},nodes:[.[]|select(.number as $n|($gone[0]|index($n))|not)|{number,authorAssociation:"OWNER",isCrossRepository:false,timelineItems:{pageInfo:{hasNextPage:false,endCursor:null},nodes:[]}}]}}}}' "$S/prs.json" ;;
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
  "pr create")
    if [ -f "$S/create-fails" ]; then echo 'HTTP 422: Validation Failed' >&2; exit 1; fi
    while [ $# -gt 0 ]; do
      case "$1" in
        --body) printf '%s\n' "$2" > "$S/created-body" ;;
        --head) printf '%s\n' "$2" > "$S/created-head" ;;
        --title) printf '%s\n' "$2" > "$S/created-title" ;;
      esac
      shift
    done
    echo 'https://github.com/o/r/pull/77' ;;
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
        // the clone's layer, as little of one as `prs` reads: a manifest naming the policy
        // and the decisions, a policy that declares no batch size, and ADR 0114 accepted —
        // which a person did, and which the tests that need otherwise undo
        std::fs::create_dir_all(work.join(".ai/repo/adrs")).unwrap();
        std::fs::write(
            work.join(".ai/manifest.yaml"),
            "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  adrs: repo/adrs\n",
        )
        .unwrap();
        std::fs::write(work.join(".ai/repo/policy.yaml"), "version: 1\n").unwrap();
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
        f.decision("accepted");
        f
    }

    /// The status the clone's layer gives ADR 0114.
    fn decision(&self, status: &str) {
        std::fs::write(
            self.work.join(".ai/repo/adrs/0114-a-batch-is-composed.md"),
            format!("---\nschema: adr/v1\nid: adr-0114\nkind: adr\ntitle: A batch is composed\nstatus: {status}\ndate: 2026-10-08\n---\n\n# 114. A batch is composed\n"),
        )
        .unwrap();
    }

    /// The policy's `integration.batch.max_members`.
    fn cap(&self, max: usize) {
        std::fs::write(
            self.work.join(".ai/repo/policy.yaml"),
            format!("version: 1\nintegration:\n  batch:\n    max_members: {max}\n"),
        )
        .unwrap();
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

    /// One open pull request as the forge lists it, with a passed `ci`.
    fn listed(n: u64, head: &str) -> Value {
        serde_json::json!({
            "number": n, "title": format!("change {n}"), "author": {"login": "someone"},
            "headRefName": format!("feature/{n}"), "headRefOid": head, "baseRefName": "master",
            "isDraft": false, "labels": [], "createdAt": format!("2026-09-0{}T00:00:00Z", n % 10),
            "updatedAt": format!("2026-09-0{}T00:00:00Z", n % 10), "body": "",
            "statusCheckRollup": [{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED",
                "conclusion": "SUCCESS", "completedAt": "2026-09-01T00:05:00Z"}],
            "reviewDecision": "", "latestReviews": [], "reviewRequests": [],
            "autoMergeRequest": null, "isCrossRepository": false
        })
    }

    /// The open pull requests: (number, head), each with a passed `ci`.
    fn open(&self, prs: &[(u64, &str)]) {
        let list: Vec<Value> = prs.iter().map(|(n, head)| Self::listed(*n, head)).collect();
        self.open_as(&list);
    }

    /// The open pull requests, exactly as given.
    fn open_as(&self, list: &[Value]) {
        std::fs::write(
            self.state.join("prs.json"),
            serde_json::to_string(list).unwrap(),
        )
        .unwrap();
    }

    fn set(&self, file: &str, text: &str) {
        std::fs::write(self.state.join(file), text).unwrap();
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

impl Forge {
    /// A no-op `scripts/derive` on master: a fixture with no derived files has nothing to
    /// regenerate, and the composition runs the derive of the tree it merged.
    fn derives(&self) {
        let dir = self.work.join("scripts");
        std::fs::create_dir_all(&dir).unwrap();
        let derive = dir.join("derive");
        std::fs::write(&derive, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&derive, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        git(&self.work, &["add", "scripts/derive"]);
        git(&self.work, &["commit", "-q", "-m", "derive"]);
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
    }

    /// Pull request `n` writes the authored `shared.txt` its own way: clean against master,
    /// and in conflict with any other that does the same.
    fn shares(&self, n: u64) -> String {
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
        std::fs::write(self.work.join("shared.txt"), format!("as #{n} has it\n")).unwrap();
        git(&self.work, &["add", "shared.txt"]);
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

    /// The trail holds one verified merge: the record that unlocks composition.
    fn unlocked(&self) {
        let common = git(
            &self.work,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        );
        let dir = Path::new(&common).join("majordomus/integration");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("events.jsonl"),
            "{\"at\":\"2026-10-05T00:00:00Z\",\"actor\":\"seed\",\"action\":\"merge_succeeded\",\"pr\":90,\"reasons\":[],\"detail\":\"seeded\"}\n",
        )
        .unwrap();
    }

    fn origin_ref(&self, name: &str) -> String {
        git(&self.origin, &["rev-parse", name])
    }

    /// The batch branches origin holds.
    fn batches(&self) -> Vec<String> {
        git(
            &self.origin,
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/int",
            ],
        )
        .lines()
        .map(str::to_string)
        .collect()
    }

    fn actions(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|e| e["action"].as_str().map(str::to_string))
            .collect()
    }

    fn state(&self, file: &str) -> String {
        std::fs::read_to_string(self.state.join(file)).unwrap_or_default()
    }
}

/// #1 and #2 each add a file; #3 and #4 each write `shared.txt` their own way. Master moved
/// after all four were opened, so every one of them is behind it.
fn four() -> (Forge, Vec<String>) {
    let f = Forge::new();
    f.derives();
    let heads = vec![f.branch(1), f.branch(2), f.shares(3), f.shares(4)];
    f.advance();
    let open: Vec<(u64, &str)> = heads
        .iter()
        .enumerate()
        .map(|(i, h)| (i as u64 + 1, h.as_str()))
        .collect();
    f.open(&open);
    (f, heads)
}

#[test]
fn a_dry_run_names_the_members_and_who_is_left_out_and_moves_nothing() {
    let (f, heads) = four();
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");
    let before = f.log();
    let (code, out, err) = f.prs(&["compose", "--max", "3"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("3 eligible, 1 left out"), "{out}");
    assert!(out.contains("dry run: would merge the members"), "{out}");
    assert!(out.contains("left out:"), "{out}");
    let (code, out, _) = f.prs(&["compose", "--max", "3", "--dry-run", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    let report: Value = serde_json::from_str(&out).expect("the report as JSON");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["outcome"]["outcome"], "would_compose");
    let members: Vec<u64> = report["plan"]["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["number"].as_u64().unwrap())
        .collect();
    assert_eq!(members, [1, 2, 3], "the queue's rank order");
    assert_eq!(
        report["plan"]["members"][0]["head"],
        Value::from(heads[0].clone())
    );
    assert_eq!(report["plan"]["left_out"][0]["number"], 4);
    assert_eq!(report["plan"]["left_out"][0]["reason"]["kind"], "over_max");
    // a dry run asks the forge nothing, records nothing and pushes nothing
    assert_eq!(f.log(), before, "a dry run asked the forge");
    assert!(f.batches().is_empty());
    assert!(!f.actions().iter().any(|a| a.starts_with("compose")));
    // a size that is no batch is refused by the command line itself
    let (code, _, err) = f.prs(&["compose", "--max", "1"]);
    assert_eq!(code, 2, "{err}");
}

#[test]
fn the_size_of_a_batch_is_the_policys_and_max_only_lowers_it() {
    let (f, _) = four();
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");
    let members = |args: &[&str]| -> (i32, Vec<u64>, String) {
        let (code, out, err) = f.prs(args);
        let numbers = serde_json::from_str::<Value>(&out)
            .ok()
            .and_then(|r| r["plan"]["members"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|m| m["number"].as_u64())
            .collect();
        (code, numbers, err)
    };
    // no key and no flag: nothing decides, and the refusal names the key
    let (code, none, err) = members(&["compose", "--format", "json"]);
    assert_eq!(code, 10, "{err}");
    assert!(none.is_empty());
    assert!(err.contains("integration.batch.max_members"), "{err}");
    assert!(err.contains("--max"), "{err}");
    // the policy's value is the size
    f.cap(3);
    let (code, three, err) = members(&["compose", "--format", "json"]);
    assert_eq!((code, three), (0, vec![1, 2, 3]), "{err}");
    // --max lowers it
    let (code, two, err) = members(&["compose", "--max", "2", "--format", "json"]);
    assert_eq!((code, two), (0, vec![1, 2]), "{err}");
    // and never raises it: refused, not clamped
    let (code, none, err) = members(&["compose", "--max", "4", "--format", "json"]);
    assert_eq!(code, 10, "{err}");
    assert!(none.is_empty());
    assert!(
        err.contains("--max 4 is above the 3 the policy allows"),
        "{err}"
    );
    // a policy that cannot be read is not "no key"
    std::fs::write(f.work.join(".ai/repo/policy.yaml"), "version: [\n").unwrap();
    let (code, _, err) = members(&["compose", "--max", "2", "--format", "json"]);
    assert_eq!(code, 12, "{err}");
    assert!(err.contains("policy could not be read"), "{err}");
}

#[test]
fn applying_is_refused_while_the_decision_is_not_accepted_and_the_dry_run_is_not() {
    let (f, _) = four();
    f.unlocked();
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");
    let asked = f.log();
    for (status, said) in [
        ("proposed", "its status is `proposed`"),
        ("rejected", "its status is `rejected`"),
    ] {
        f.decision(status);
        let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
        assert_eq!(code, 10, "{status}: {out}{err}");
        assert!(err.contains("ADR 0114") && err.contains(said), "{err}");
        assert!(err.contains("a person's act"), "{err}");
        // the dry run is unaffected
        let (code, out, err) = f.prs(&["compose", "--max", "4"]);
        assert_eq!(code, 0, "{status}: {out}{err}");
        assert!(out.contains("dry run: would merge the members"), "{out}");
    }
    // a layer that does not hold the decision at all has not accepted it
    std::fs::remove_file(f.work.join(".ai/repo/adrs/0114-a-batch-is-composed.md")).unwrap();
    let (code, _, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 10, "{err}");
    assert!(err.contains("holds no such decision"), "{err}");
    // asked before the forge, the lease and the trail: nothing was asked, pushed or recorded
    assert_eq!(f.log(), asked, "a refused act asked the forge");
    assert!(f.batches().is_empty());
    assert!(!f.actions().iter().any(|a| a.starts_with("compose")));
    // accepted, it composes
    f.decision("accepted");
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(f.batches().len(), 1);
}

#[test]
fn applying_is_refused_before_the_forge_until_the_trail_holds_a_verified_merge() {
    let (f, _) = four();
    let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(
        err.contains("ADR 0114 D7") && err.contains("holds 0"),
        "{err}"
    );
    assert_eq!(f.log(), "", "the refusal asked the forge");
    assert!(f.batches().is_empty());
    assert!(f.actions().is_empty());
}

#[test]
fn applying_composes_one_merge_per_member_a_manifest_and_one_commit_on_a_new_branch() {
    let (f, heads) = four();
    f.unlocked();
    let master = f.origin_ref("refs/heads/master");
    let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    // #4 writes the file #3 wrote: it is dropped, named with the path, and the rest compose
    let id = format!("{}-1-2-3", &master[..10]);
    let branch = format!("int/batch-{id}");
    assert_eq!(f.batches(), std::slice::from_ref(&branch));
    assert!(out.contains(&format!("composed {branch}")), "{out}");
    assert!(out.contains("pull request #77"), "{out}");
    assert!(
        out.contains("dropped during the composition:") && out.contains("shared.txt"),
        "{out}"
    );
    let tip = f.origin_ref(&format!("refs/heads/{branch}"));
    // attribution is structural: from master, on the first-parent line, one merge per member
    // whose second parent is that member's head, then the one composition commit
    let line: Vec<String> = git(
        &f.origin,
        &[
            "rev-list",
            "--first-parent",
            "--reverse",
            &format!("{master}..{tip}"),
        ],
    )
    .lines()
    .map(str::to_string)
    .collect();
    assert_eq!(line.len(), 4, "{line:?}");
    for (merge, head) in line.iter().zip(&heads[..3]) {
        assert_eq!(&git(&f.origin, &["rev-parse", &format!("{merge}^2")]), head);
    }
    assert_eq!(
        git(&f.origin, &["rev-parse", &format!("{}^1", line[0])]),
        master
    );
    let subject = git(&f.origin, &["log", "-1", "--format=%s", &line[0]]);
    assert_eq!(subject, "Merge pull request #1 into a batch on master");
    // the composition commit is no merge, and changes the manifest alone
    let path = format!(".ai/repo/integration/batches/{id}.yaml");
    assert_eq!(
        git(&f.origin, &["rev-list", "--parents", "-1", &tip])
            .split_whitespace()
            .count(),
        2
    );
    assert_eq!(
        git(
            &f.origin,
            &["diff", "--name-only", &format!("{tip}^"), &tip]
        ),
        path
    );
    let manifest = git(&f.origin, &["show", &format!("{tip}:{path}")]);
    assert!(
        manifest.starts_with(&format!(
            "schema: integration-batch/v1\nid: '{id}'\nbase: master\nbase_master: '{master}'\n"
        )),
        "{manifest}"
    );
    for (n, (merge, head)) in line.iter().zip(&heads[..3]).enumerate() {
        assert!(
            manifest.contains(&format!("number: {}", n + 1)),
            "{manifest}"
        );
        assert!(
            manifest.contains(head) && manifest.contains(merge),
            "{manifest}"
        );
    }
    assert!(
        !manifest.contains(&heads[3]),
        "the dropped member is in the manifest"
    );
    // the pull request that was asked for supersedes each member, and only them
    assert_eq!(f.state("created-head").trim(), branch);
    let body = f.state("created-body");
    for n in 1..=3 {
        assert!(
            body.lines().any(|l| l == format!("Supersedes #{n}")),
            "{body}"
        );
    }
    assert!(!body.contains("Supersedes #4"), "{body}");
    assert!(f
        .log()
        .contains("pr create --base master --head int/batch-"));
    // the trail named the act before it was taken, and its outcome after
    let actions = f.actions();
    let at = |a: &str| {
        actions
            .iter()
            .position(|x| x == a)
            .unwrap_or_else(|| panic!("{a} not on the trail: {actions:?}"))
    };
    assert!(at("compose_selected") < at("compose_attempted"));
    assert!(at("compose_attempted") < at("composed"));
    let composed = f.last("composed").unwrap();
    assert_eq!(composed["head_after"], Value::from(tip.clone()));
    assert_eq!(composed["pr"], 77);
    assert_eq!(composed["master_before"], Value::from(master.clone()));
    // nothing else moved: master, every member's branch, and no scratch worktree is left
    assert_eq!(f.origin_ref("refs/heads/master"), master);
    for (n, head) in heads.iter().enumerate() {
        assert_eq!(
            &f.origin_ref(&format!("refs/heads/feature/{}", n + 1)),
            head
        );
    }
    assert_eq!(git(&f.work, &["worktree", "list"]).lines().count(), 1);

    // the same members on the same master are the same batch: composing again pushes nothing
    let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("already exists on origin"), "{out}");
    assert_eq!(f.origin_ref(&format!("refs/heads/{branch}")), tip);
    let refused = f
        .last("compose_refused")
        .expect("compose_refused on the trail");
    assert!(
        refused["detail"]
            .as_str()
            .unwrap()
            .contains("nothing was pushed"),
        "{refused}"
    );
}

#[test]
fn a_composition_left_with_one_member_pushes_nothing() {
    let f = Forge::new();
    f.derives();
    let heads = [f.shares(3), f.shares(4)];
    f.advance();
    f.open(&[(3, &heads[0]), (4, &heads[1])]);
    f.unlocked();
    let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("REFUSE: nothing was pushed"), "{out}");
    assert!(out.contains("shared.txt"), "{out}");
    assert!(f.batches().is_empty());
    assert!(!f.log().contains("pr create"), "{}", f.log());
    let refused = f
        .last("compose_refused")
        .expect("compose_refused on the trail");
    assert_eq!(refused["class"], "conflict");
    assert_eq!(git(&f.work, &["worktree", "list"]).lines().count(), 1);
}

#[test]
fn a_derive_that_fails_and_a_pull_request_that_cannot_be_opened_are_refused_acts() {
    // no scripts/derive on master: the composed tree cannot be derived, so nothing is pushed
    let f = Forge::new();
    let heads = [f.branch(1), f.branch(2)];
    f.advance();
    f.open(&[(1, &heads[0]), (2, &heads[1])]);
    f.unlocked();
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("scripts/derive could not run"), "{out}");
    assert!(f.batches().is_empty());

    // the forge refuses to open the pull request: the branch is there, and the refusal says so
    let (f, _) = four();
    f.unlocked();
    f.set("create-fails", "");
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert_eq!(f.batches().len(), 1);
    assert!(
        out.contains("was pushed at") && out.contains("could not be opened"),
        "{out}"
    );
    assert!(f.last("composed").is_none());
    assert!(f.last("compose_refused").is_some());
}

// ---------------------------------------------------------------- the gate (ADR 0114 D5)

impl Forge {
    /// A branch from the current master that merges `heads` in order, each with `--no-ff`,
    /// as a person builds a batch by hand. Its tip, left checked out detached.
    fn merges(&self, heads: &[&str]) -> String {
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        for head in heads {
            git(
                &self.work,
                &[
                    "merge",
                    "-q",
                    "--no-ff",
                    "-m",
                    &format!("merge {head}"),
                    head,
                ],
            );
        }
        git(&self.work, &["rev-parse", "HEAD"])
    }

    /// One more commit on what is checked out, writing `path`. Its id.
    fn commits(&self, path: &str, text: &str) -> String {
        let file = self.work.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
        git(&self.work, &["add", "--", path]);
        git(
            &self.work,
            &["commit", "-q", "-m", &format!("write {path}")],
        );
        git(&self.work, &["rev-parse", "HEAD"])
    }

    /// `prs batch-check --base origin/master --head <head>`, as JSON.
    fn check(&self, head: &str) -> (i32, Value, String) {
        let (code, out, err) = self.prs(&[
            "batch-check",
            "--base",
            "origin/master",
            "--head",
            head,
            "--format",
            "json",
        ]);
        (code, serde_json::from_str(&out).unwrap_or(Value::Null), err)
    }

    /// The first-parent merges of `tip` from master, oldest first.
    fn line(&self, tip: &str) -> Vec<String> {
        git(
            &self.work,
            &[
                "rev-list",
                "--first-parent",
                "--reverse",
                "--merges",
                &format!("origin/master..{tip}"),
            ],
        )
        .lines()
        .map(str::to_string)
        .collect()
    }
}

/// A manifest as `prs compose` writes one, for the members `(number, head, merge commit)`.
fn manifest_text(master: &str, members: &[(u64, &str, &str)]) -> (String, String) {
    let id = members
        .iter()
        .fold(master[..10].to_string(), |id, (n, _, _)| {
            format!("{id}-{n}")
        });
    let mut text = format!(
        "schema: integration-batch/v1\nid: '{id}'\nbase: master\nbase_master: '{master}'\ncomposed_at: '2026-10-08T10:00:00Z'\nmembers:\n"
    );
    for (n, head, merge) in members {
        text.push_str(&format!(
            "  - number: {n}\n    head: '{head}'\n    title: 'change {n}'\n    merge_commit: '{merge}'\n"
        ));
    }
    (format!(".ai/repo/integration/batches/{id}.yaml"), text)
}

fn kinds(report: &Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|f| f["kind"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn a_plain_branch_and_a_stack_are_not_batches_and_git_alone_says_so() {
    let (f, heads) = four();
    // a branch of ordinary commits
    let (code, report, err) = f.check(&heads[0]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(report["verdict"], "not_a_batch");
    assert_eq!(report["forge_read"], false);
    // a branch that merges one other pull request's head is a stack
    let stack = f.merges(&[&heads[0]]);
    f.commits("stacked.txt", "on top\n");
    let stack_tip = git(&f.work, &["rev-parse", "HEAD"]);
    assert_ne!(stack, stack_tip);
    let (code, report, err) = f.check(&stack_tip);
    assert_eq!(code, 0, "{err}");
    assert_eq!(report["verdict"], "not_a_batch");
    // a branch that merged the base twice merged nobody
    git(&f.work, &["checkout", "-q", "--detach", &heads[1]]);
    f.advance_keeping();
    git(
        &f.work,
        &["merge", "-q", "--no-ff", "-m", "refresh", "origin/master"],
    );
    f.advance_keeping();
    git(
        &f.work,
        &[
            "merge",
            "-q",
            "--no-ff",
            "-m",
            "refresh again",
            "origin/master",
        ],
    );
    let refreshed = git(&f.work, &["rev-parse", "HEAD"]);
    let (code, report, err) = f.check(&refreshed);
    assert_eq!(code, 0, "{err}");
    assert_eq!(report["verdict"], "not_a_batch");
    // none of it asked the forge: git alone decided, so an unreadable forge changes nothing
    assert!(!f.log().contains("pr list"), "{}", f.log());
    f.set("down", "");
    let (code, _, err) = f.check(&stack_tip);
    assert_eq!(code, 0, "{err}");
    let (code, out, _) = f.prs(&[
        "batch-check",
        "--base",
        "origin/master",
        "--head",
        &stack_tip,
    ]);
    assert_eq!(code, 0);
    assert!(out.starts_with("batch-check: not a batch"), "{out}");
    assert_eq!(out.lines().count(), 1, "one line: {out}");
}

impl Forge {
    /// Master moves on, and what is checked out stays checked out.
    fn advance_keeping(&self) {
        let here = git(&self.work, &["rev-parse", "HEAD"]);
        let n = git(&self.origin, &["rev-list", "--count", "master"]);
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        std::fs::write(self.work.join(format!("later-{n}.txt")), "later\n").unwrap();
        git(&self.work, &["add", &format!("later-{n}.txt")]);
        git(&self.work, &["commit", "-q", "-m", "later"]);
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", &here]);
    }
}

#[test]
fn a_hand_built_batch_with_no_manifest_is_refused_naming_both_members() {
    let (f, heads) = four();
    let tip = f.merges(&[&heads[0], &heads[1]]);
    let (code, report, err) = f.check(&tip);
    assert_eq!(code, 10, "{err}");
    assert_eq!(report["verdict"], "refused");
    assert_eq!(report["forge_read"], true);
    assert_eq!(kinds(&report), ["manifest_missing"]);
    let members: Vec<u64> = report["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["number"].as_u64().unwrap())
        .collect();
    assert_eq!(members, [1, 2]);
    let (code, out, _) = f.prs(&["batch-check", "--base", "origin/master", "--head", &tip]);
    assert_eq!(code, 10);
    assert!(out.contains("REFUSED") && out.contains("#1, #2"), "{out}");
    assert!(
        out.contains("no manifest under .ai/repo/integration/batches/"),
        "{out}"
    );
    assert!(
        out.contains(&heads[0][..10]) && out.contains(&heads[1][..10]),
        "{out}"
    );
    // the forge's test merge of that branch is looked through to the branch itself
    git(&f.work, &["checkout", "-q", "--detach", "origin/master"]);
    git(
        &f.work,
        &["merge", "-q", "--no-ff", "-m", "test merge", &tip],
    );
    let test_merge = git(&f.work, &["rev-parse", "HEAD"]);
    let (code, report, err) = f.check(&test_merge);
    assert_eq!(code, 10, "{err}");
    assert_eq!(report["head"], Value::from(tip.clone()));
    assert_eq!(report["looked_through"], Value::from(test_merge));
    // a forge that cannot be read is "cannot run", never clean
    f.set("down", "");
    let (code, out, err) = f.prs(&["batch-check", "--base", "origin/master", "--head", &tip]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(out.is_empty(), "{out}");
    assert!(
        err.contains("batch-check cannot run") && err.contains("could not be listed"),
        "{err}"
    );
}

#[test]
fn a_composed_batch_passes_and_one_authored_commit_on_it_is_refused_naming_the_path() {
    let (f, heads) = four();
    f.unlocked();
    let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    let branch = f.batches().remove(0);
    git(
        &f.work,
        &["fetch", "-q", "origin", &format!("refs/heads/{branch}")],
    );
    let tip = f.origin_ref(&format!("refs/heads/{branch}"));
    let (code, report, err) = f.check(&tip);
    assert_eq!(code, 0, "{err}{report}");
    assert_eq!(report["verdict"], "batch");
    assert_eq!(report["members"].as_array().unwrap().len(), 3);
    assert!(report["manifest"]
        .as_str()
        .unwrap()
        .ends_with("-1-2-3.yaml"));
    let (code, out, _) = f.prs(&["batch-check", "--base", "origin/master", "--head", &tip]);
    assert_eq!(code, 0);
    assert!(
        out.starts_with("batch-check: ok:") && out.contains("#1, #2, #3"),
        "{out}"
    );

    // master moves and is merged into the batch: that merge is nobody's member, and what it
    // brings is not the batch's change
    git(&f.work, &["checkout", "-q", "--detach", &tip]);
    f.advance_keeping();
    git(
        &f.work,
        &["merge", "-q", "--no-ff", "-m", "refresh", "origin/master"],
    );
    let refreshed = git(&f.work, &["rev-parse", "HEAD"]);
    let (code, report, err) = f.check(&refreshed);
    assert_eq!(code, 0, "{err}{report}");
    assert_eq!(report["verdict"], "batch");
    assert_eq!(report["members"].as_array().unwrap().len(), 3);

    // a fix written on the batch branch belongs to no member
    git(&f.work, &["checkout", "-q", "--detach", &tip]);
    let fix = f.commits("fix.txt", "a fix nobody's pull request saw\n");
    let (code, report, err) = f.check(&fix);
    assert_eq!(code, 10, "{err}");
    assert_eq!(kinds(&report), ["foreign_change"]);
    assert_eq!(report["findings"][0]["path"], "fix.txt");
    assert_eq!(report["findings"][0]["commit"], Value::from(fix.clone()));
    let (_, out, _) = f.prs(&["batch-check", "--base", "origin/master", "--head", &fix]);
    assert!(out.contains("changes fix.txt on the batch branch"), "{out}");
    assert!(out.contains(&fix[..10]), "{out}");
    // a member that moved since is no longer what the batch carries
    git(&f.work, &["checkout", "-q", "--detach", &heads[1]]);
    let moved = f.commits("2b.txt", "more\n");
    f.open(&[(1, &heads[0]), (2, &moved), (3, &heads[2]), (4, &heads[3])]);
    let (code, report, err) = f.check(&tip);
    assert_eq!(code, 10, "{err}");
    let found = kinds(&report);
    assert!(
        found.contains(&"member_not_merged".to_string()),
        "{found:?}"
    );
    assert!(found.contains(&"foreign_change".to_string()), "{found:?}");
}

#[test]
fn a_manifest_that_disagrees_with_the_merges_is_refused_naming_the_member_and_the_line() {
    let (f, heads) = four();
    let master = f.origin_ref("refs/heads/master");
    let tip = f.merges(&[&heads[0], &heads[1]]);
    let line = f.line(&tip);
    assert_eq!(line.len(), 2);
    let with = |members: &[(u64, &str, &str)]| -> (Value, String, String) {
        git(&f.work, &["checkout", "-q", "--detach", &tip]);
        let (path, text) = manifest_text(&master, members);
        let head = f.commits(&path, &text);
        let (code, report, err) = f.check(&head);
        assert!(code == 0 || code == 10, "{code}: {err}");
        let (_, said, _) = f.prs(&["batch-check", "--base", "origin/master", "--head", &head]);
        (report, said, path)
    };
    // the manifest git agrees with
    let (report, said, _) = with(&[(1, &heads[0], &line[0]), (2, &heads[1], &line[1])]);
    assert_eq!(report["verdict"], "batch", "{said}");
    // the same members, the other way round
    let (report, said, path) = with(&[(2, &heads[1], &line[1]), (1, &heads[0], &line[0])]);
    assert_eq!(report["verdict"], "refused");
    assert_eq!(
        kinds(&report),
        ["member_out_of_order", "member_out_of_order"]
    );
    assert!(
        said.contains(&format!("{path}:7 lists #2 as member 1")),
        "{said}"
    );
    // a head that is not the merge's second parent
    let (report, said, path) = with(&[(1, &heads[0], &line[0]), (2, &heads[2], &line[1])]);
    assert_eq!(kinds(&report), ["wrong_head"]);
    assert_eq!(report["findings"][0]["number"], 2);
    assert!(
        said.contains(&format!("{path}:11 gives #2 the head")),
        "{said}"
    );
    // a merge commit that is not the member's
    let (report, _, _) = with(&[(1, &heads[0], &line[0]), (2, &heads[1], &line[0])]);
    assert_eq!(kinds(&report), ["wrong_merge_commit"]);
    // a member that was merged and is not named
    let (report, said, path) = with(&[(1, &heads[0], &line[0])]);
    assert_eq!(kinds(&report), ["member_not_in_manifest"]);
    assert_eq!(report["findings"][0]["number"], 2);
    assert_eq!(
        report["findings"][0]["merge_commit"],
        Value::from(line[1].clone())
    );
    assert!(
        said.contains(&format!("{path} names no member #2")),
        "{said}"
    );
    // a member that is named and was not merged
    let (report, said, _) = with(&[
        (1, &heads[0], &line[0]),
        (2, &heads[1], &line[1]),
        (3, &heads[2], &line[1]),
    ]);
    assert_eq!(kinds(&report), ["member_not_merged"]);
    assert!(said.contains(":15 names member #3"), "{said}");
    // two manifests are no identity, and a file that is no manifest is none
    git(&f.work, &["checkout", "-q", "--detach", &tip]);
    f.commits(".ai/repo/integration/batches/a.yaml", "schema: other/v1\n");
    let one = git(&f.work, &["rev-parse", "HEAD"]);
    let (code, report, _) = f.check(&one);
    assert_eq!(code, 10);
    assert_eq!(kinds(&report), ["manifest_unreadable"]);
    let two = f.commits(".ai/repo/integration/batches/b.yaml", "schema: other/v1\n");
    let (_, report, _) = f.check(&two);
    // neither is the batch's manifest, so each is also a change no composition may make
    assert_eq!(
        kinds(&report),
        ["manifest_ambiguous", "foreign_change", "foreign_change"]
    );
}

/// A manifest the branch adds or changes makes it a batch to be judged, whatever it merges.
/// Counted by its merges alone, a batch whose members moved since it was composed merged
/// "fewer than two other open pull requests" and left the gate as not a batch, exit 0, with a
/// manifest that named members nothing carried.
#[test]
fn a_manifest_makes_a_branch_a_batch_to_be_judged_whatever_it_merges() {
    let (f, heads) = four();
    let master = f.origin_ref("refs/heads/master");
    let tip = f.merges(&[&heads[0], &heads[1]]);
    let line = f.line(&tip);
    let (path, text) = manifest_text(
        &master,
        &[(1, &heads[0], &line[0]), (2, &heads[1], &line[1])],
    );
    let batch = f.commits(&path, &text);
    let (code, report, err) = f.check(&batch);
    assert_eq!(code, 0, "{err}{report}");
    assert_eq!(report["verdict"], "batch");

    // #2's owner pushed: one member merge is left, and the manifest still names two
    git(&f.work, &["checkout", "-q", "--detach", &heads[1]]);
    let moved = f.commits("2b.txt", "more\n");
    f.open(&[(1, &heads[0]), (2, &moved), (3, &heads[2]), (4, &heads[3])]);
    let (code, report, err) = f.check(&batch);
    assert_eq!(code, 10, "a one-member remnant is judged: {err}{report}");
    assert_eq!(report["verdict"], "refused");
    assert_eq!(report["forge_read"], true);
    assert_eq!(report["manifest"], Value::from(path.clone()));
    assert_eq!(report["members"].as_array().unwrap().len(), 1);
    let found = kinds(&report);
    assert_eq!(found[0], "member_not_merged", "{found:?}");
    assert_eq!(report["findings"][0]["number"], 2);
    // what the stale merge brought belongs to no member any more
    assert!(found.contains(&"foreign_change".to_string()), "{found:?}");
    let (code, said, _) = f.prs(&["batch-check", "--base", "origin/master", "--head", &batch]);
    assert_eq!(code, 10);
    assert!(
        said.contains(&format!(
            "adds or changes {path}, merges #1 and is not the batch"
        )),
        "{said}"
    );
    assert!(
        said.contains(&format!("{path}:11 names member #2")),
        "{said}"
    );

    // both moved: no member merge at all, and it is still judged, each member named
    git(&f.work, &["checkout", "-q", "--detach", &heads[0]]);
    let moved_too = f.commits("1b.txt", "more\n");
    f.open(&[(1, &moved_too), (2, &moved), (3, &heads[2]), (4, &heads[3])]);
    let (code, report, err) = f.check(&batch);
    assert_eq!(code, 10, "{err}{report}");
    let found = kinds(&report);
    assert_eq!(
        found.iter().filter(|k| *k == "member_not_merged").count(),
        2,
        "{found:?}"
    );
    let (_, said, _) = f.prs(&["batch-check", "--base", "origin/master", "--head", &batch]);
    assert!(said.contains("merges no other open pull request"), "{said}");
    f.open(&[
        (1, &heads[0]),
        (2, &heads[1]),
        (3, &heads[2]),
        (4, &heads[3]),
    ]);

    // a stack that carries a manifest naming exactly its one merge is a batch of one
    let stack = f.merges(&[&heads[0]]);
    let (one, text) = manifest_text(&master, &[(1, &heads[0], &stack)]);
    let remnant = f.commits(&one, &text);
    let (code, report, err) = f.check(&remnant);
    assert_eq!(code, 10, "{err}{report}");
    assert_eq!(kinds(&report), ["too_few_members"]);
    assert_eq!(report["findings"][0]["found"], 1);

    // a branch of ordinary commits that changes a manifest is judged too, and what it needs
    // from the forge it cannot do without: unreadable is "cannot run", never clean
    git(&f.work, &["checkout", "-q", "--detach", &heads[2]]);
    let plain = f.commits(&path, &text);
    let (code, report, err) = f.check(&plain);
    assert_eq!(code, 10, "{err}{report}");
    assert!(
        kinds(&report).contains(&"member_not_merged".to_string()),
        "{report}"
    );
    f.set("down", "");
    let (code, out, err) = f.prs(&["batch-check", "--base", "origin/master", "--head", &plain]);
    assert_eq!(code, 12, "{out}{err}");
    assert!(out.is_empty(), "{out}");
    assert!(
        err.contains("batch-check cannot run") && err.contains("batch manifest(s)"),
        "{err}"
    );
}

// ---------------------------------------------------------------- the one bump (D6)

impl Forge {
    /// Master declares a version, and carries a launcher whose `release bump` raises it to
    /// what the state file `bump-to` holds, writes nothing when there is none, and fails
    /// when `bump-fails` exists: the composed tree's own executable, as far as the
    /// composition can tell.
    fn versioned(&self) {
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        let crate_dir = self.work.join("apps/majordomus-cli");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"majordomus-cli\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let bin = self.work.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let launcher = bin.join("majordomus-cli");
        std::fs::write(
            &launcher,
            format!(
                "#!/bin/sh\nS=\"{}\"\necho \"$*\" >> \"$S/launcher-log\"\n[ \"$1 $2\" = \"release bump\" ] || exit 2\nif [ -f \"$S/bump-fails\" ]; then echo 'release: the contract is not measurable here' ; exit 12; fi\nif [ -f \"$S/bump-to\" ]; then\n  to=\"$(cat \"$S/bump-to\")\"\n  sed \"s/^version = .*/version = \\\"$to\\\"/\" apps/majordomus-cli/Cargo.toml > Cargo.tmp && mv Cargo.tmp apps/majordomus-cli/Cargo.toml\nfi\nexit 0\n",
                self.state.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        git(
            &self.work,
            &[
                "add",
                "apps/majordomus-cli/Cargo.toml",
                "bin/majordomus-cli",
            ],
        );
        git(
            &self.work,
            &["commit", "-q", "-m", "a version and a launcher"],
        );
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
    }

    /// Two members behind a master that declares 0.1.0, the trail unlocked.
    fn two_versioned() -> (Forge, Vec<String>) {
        let f = Forge::new();
        f.derives();
        f.versioned();
        let heads = vec![f.branch(1), f.branch(2)];
        f.advance();
        f.open(&[(1, &heads[0]), (2, &heads[1])]);
        f.unlocked();
        (f, heads)
    }

    /// What the composition commit of the one batch on origin changed, and its manifest.
    fn composition(&self) -> (String, Vec<String>, String) {
        let branch = self.batches().remove(0);
        let tip = self.origin_ref(&format!("refs/heads/{branch}"));
        let changed = git(
            &self.origin,
            &["diff", "--name-only", &format!("{tip}^"), &tip],
        )
        .lines()
        .map(str::to_string)
        .collect();
        let path = format!(
            ".ai/repo/integration/batches/{}.yaml",
            branch.trim_start_matches("int/batch-")
        );
        let manifest = git(&self.origin, &["show", &format!("{tip}:{path}")]);
        (tip, changed, manifest)
    }
}

#[test]
fn the_composition_takes_one_bump_to_what_the_composed_tree_requires() {
    let (f, _) = Forge::two_versioned();
    f.set("bump-to", "0.2.0");
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("version: raised 0.1.0 -> 0.2.0"), "{out}");
    // asked once, of the composed tree's own launcher
    assert_eq!(f.state("launcher-log").trim(), "release bump");
    let (tip, changed, manifest) = f.composition();
    assert!(
        manifest.contains("version_before: '0.1.0'\nversion_after: '0.2.0'\nmembers:"),
        "{manifest}"
    );
    // one commit: the manifest and the version file, and nothing else
    assert_eq!(changed.len(), 2, "{changed:?}");
    assert!(changed.contains(&"apps/majordomus-cli/Cargo.toml".to_string()));
    assert!(git(
        &f.origin,
        &["show", &format!("{tip}:apps/majordomus-cli/Cargo.toml")]
    )
    .contains("version = \"0.2.0\""));
    assert!(git(&f.origin, &["log", "-1", "--format=%B", &tip]).contains("raised 0.1.0 -> 0.2.0"));
    // and the gate lets the version file through: it is what `release bump` writes
    git(&f.work, &["fetch", "-q", "origin", &tip]);
    let (code, report, err) = f.check(&tip);
    assert_eq!(code, 0, "{err}{report}");
    assert_eq!(report["verdict"], "batch");
}

#[test]
fn a_contract_that_requires_nothing_writes_no_bump_and_the_manifest_says_so() {
    let (f, _) = Forge::two_versioned();
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("version: stays 0.1.0"), "{out}");
    let (tip, changed, manifest) = f.composition();
    assert!(
        manifest.contains("version_before: '0.1.0'\nversion_after: '0.1.0'\n"),
        "{manifest}"
    );
    assert_eq!(changed.len(), 1, "only the manifest: {changed:?}");
    assert!(git(&f.origin, &["log", "-1", "--format=%B", &tip]).contains("version stays 0.1.0"));

    // a version nobody could decide composes nothing
    let (f, _) = Forge::two_versioned();
    f.set("bump-fails", "");
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(
        out.contains("the version the composed tree must declare could not be decided")
            && out.contains("not measurable here"),
        "{out}"
    );
    assert!(f.batches().is_empty());
    assert!(f.last("compose_refused").is_some());
    assert_eq!(git(&f.work, &["worktree", "list"]).lines().count(), 1);
}

// ---------------------------------------------------------------- explain

#[test]
fn explain_names_a_batchs_members_in_order_and_a_members_batch() {
    let (f, heads) = four();
    f.unlocked();
    let (code, out, err) = f.prs(&["compose", "--max", "4", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    let branch = f.batches().remove(0);
    let tip = f.origin_ref(&format!("refs/heads/{branch}"));
    // the forge lists the batch the composition opened, its body as it was created
    git(&f.origin, &["update-ref", "refs/pull/77/head", &tip]);
    let mut batch = Forge::listed(77, &tip);
    batch["headRefName"] = Value::from(branch.clone());
    batch["title"] = Value::from(f.state("created-title").trim());
    batch["body"] = Value::from(f.state("created-body"));
    batch["createdAt"] = Value::from("2026-09-09T00:00:00Z");
    let mut list: Vec<Value> = heads
        .iter()
        .enumerate()
        .map(|(i, h)| Forge::listed(i as u64 + 1, h))
        .collect();
    list.push(batch);
    f.open_as(&list);
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");

    let (_, out, err) = f.prs(&["explain", "77"]);
    let id = branch.trim_start_matches("int/batch-");
    assert!(
        out.contains(&format!("batch:        {id} on master")),
        "{out}{err}"
    );
    assert!(out.contains("3 member(s) in composition order"), "{out}");
    let at = |needle: &str| {
        out.find(needle)
            .unwrap_or_else(|| panic!("{needle}: {out}"))
    };
    let line = f.line(&tip);
    for (i, head) in heads[..3].iter().enumerate() {
        let said = format!(
            "{}. #{:<5} head {}  merged as {}",
            i + 1,
            i + 1,
            &head[..10],
            &line[i][..10]
        );
        at(&said);
    }
    assert!(at("1. #1") < at("2. #2") && at("2. #2") < at("3. #3"));
    assert!(!out.contains("carried by"), "{out}");
    // a member names the open batch that carries it; the one left out names none
    let (_, out, _) = f.prs(&["explain", "2"]);
    assert!(out.contains("carried by:   batch #77, open"), "{out}");
    let (_, out, _) = f.prs(&["explain", "4"]);
    assert!(
        !out.contains("carried by") && !out.contains("batch:"),
        "{out}"
    );
    // the same facts, machine-readable, on the one assessment every surface renders
    let (_, out, _) = f.prs(&["explain", "77", "--format", "json"]);
    let view: Value = serde_json::from_str(&out).expect("explain as JSON");
    let members: Vec<u64> = view["assessment"]["batch"]["members"]
        .as_array()
        .expect("the batch's members")
        .iter()
        .map(|m| m["number"].as_u64().unwrap())
        .collect();
    assert_eq!(members, [1, 2, 3]);
    assert_eq!(
        view["assessment"]["batch"]["members"][0]["head"],
        Value::from(heads[0].clone())
    );
    let (_, out, _) = f.prs(&["explain", "1", "--format", "json"]);
    let view: Value = serde_json::from_str(&out).expect("explain as JSON");
    assert_eq!(view["assessment"]["carried_by"], 77);
    assert!(view["assessment"]["batch"].is_null());
    // a batch is one candidate, never a member, and a member a batch carries is not offered
    let (_, out, _) = f.prs(&["compose", "--max", "4", "--format", "json"]);
    let report: Value = serde_json::from_str(&out).expect("the plan");
    let why = |n: u64| {
        report["plan"]["left_out"]
            .as_array()
            .unwrap()
            .iter()
            .find(|l| l["number"] == n)
            .map(|l| l["reason"]["kind"].clone())
    };
    assert_eq!(why(77), Some(Value::from("is_batch")));
    assert_eq!(why(1), Some(Value::from("named_by_open")));
}

// ---------------------------------------------------------------- the act's rarer endings

#[test]
fn a_conflict_on_derived_paths_alone_is_a_refused_act_and_no_member_is_blamed() {
    let f = Forge::new();
    f.derives();
    // master marks gen.txt as derived; this clone declares no merge.derived driver
    git(&f.work, &["fetch", "-q", "origin"]);
    git(&f.work, &["checkout", "-q", "--detach", "origin/master"]);
    f.commits(".gitattributes", "gen.txt merge=derived\n");
    git(&f.work, &["push", "-q", "origin", "HEAD:master"]);
    let member = |n: u64| {
        git(&f.work, &["fetch", "-q", "origin"]);
        git(&f.work, &["checkout", "-q", "--detach", "origin/master"]);
        f.commits(&format!("{n}.txt"), "authored\n");
        let head = f.commits("gen.txt", &format!("generated for #{n}\n"));
        git(
            &f.work,
            &[
                "push",
                "-q",
                "origin",
                &format!("HEAD:refs/heads/feature/{n}"),
                &format!("HEAD:refs/pull/{n}/head"),
            ],
        );
        head
    };
    let heads = [member(1), member(2)];
    f.advance();
    f.open(&[(1, &heads[0]), (2, &heads[1])]);
    f.unlocked();
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(
        out.contains("merging #2 conflicts only on 1 derived path(s) (gen.txt)")
            && out.contains("merge.derived driver"),
        "{out}"
    );
    assert!(!out.contains("dropped during the composition"), "{out}");
    assert!(f.batches().is_empty());
    assert_eq!(git(&f.work, &["worktree", "list"]).lines().count(), 1);
}

#[test]
fn a_title_the_manifest_cannot_carry_and_an_origin_that_cannot_be_listed_push_nothing() {
    // a title with a line break: the manifest would not read back as what was composed
    let (f, heads) = four();
    f.unlocked();
    let mut list: Vec<Value> = heads[..2]
        .iter()
        .enumerate()
        .map(|(i, h)| Forge::listed(i as u64 + 1, h))
        .collect();
    list[1]["title"] = Value::from("change 2\nand a second line");
    f.open_as(&list);
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(
        out.contains("does not read back as it was written; nothing was pushed"),
        "{out}"
    );
    assert!(f.batches().is_empty());
    assert!(!f.log().contains("pr create"));

    // origin goes away after the members were fetched and before the branch is looked for:
    // a listing that fails is its own refusal, never "the branch is not there"
    let (f, _) = four();
    f.unlocked();
    let hooks = std::path::PathBuf::from(git(
        &f.work,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    ))
    .join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("post-merge");
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\nif [ -d \"{0}\" ]; then mv \"{0}\" \"{0}.away\"; fi\n",
            f.origin.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let (code, out, err) = f.prs(&["compose", "--max", "2", "--apply"]);
    std::fs::remove_file(&hook).unwrap();
    let away = std::path::PathBuf::from(format!("{}.away", f.origin.display()));
    assert!(away.is_dir(), "the hook never ran: {out}{err}");
    std::fs::rename(&away, &f.origin).unwrap();
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("the branch could not be listed"), "{out}");
    assert!(f.batches().is_empty());
    assert!(f.last("composed").is_none());
    assert_eq!(git(&f.work, &["worktree", "list"]).lines().count(), 1);
}

#[test]
fn a_head_the_composition_already_holds_is_dropped_and_named() {
    // #2's head is #1's too: two pull requests opened from one commit. The second merge is
    // no merge commit, so #2 would carry nothing and is dropped; #3 still rides.
    let f = Forge::new();
    f.derives();
    let one = f.branch(1);
    git(&f.origin, &["update-ref", "refs/pull/2/head", &one]);
    let three = f.branch(3);
    f.advance();
    f.open(&[(1, &one), (2, &one), (3, &three)]);
    f.unlocked();
    let (code, out, err) = f.prs(&["compose", "--max", "3", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("dropped during the composition:")
            && out.contains("its head is already contained in the members before it"),
        "{out}"
    );
    let (_, _, manifest) = f.composition();
    assert!(
        manifest.contains("number: 1") && manifest.contains("number: 3"),
        "{manifest}"
    );
    assert!(!manifest.contains("number: 2"), "{manifest}");
    let id = f.batches().remove(0);
    assert!(id.ends_with("-3-1") || id.ends_with("-1-3"), "{id}");
}

// ---------------------------------------------------------------- the manifest is an object

/// A manifest under `.ai/repo/integration/batches/` is an object of the layer: indexed under
/// its id, served, and held to the shipped schema — by the distribution's own kind and the
/// class the skeleton's sources declare, with nothing the repository adds.
#[test]
fn a_manifest_in_the_layer_is_indexed_and_validated_as_its_kind() {
    let f = common::Fixture::new();
    let class = std::fs::read_to_string(
        common::dist_share().join("skeleton/ai/repo/knowledge/sources.yaml"),
    )
    .expect("the skeleton's sources");
    let at = class
        .find("  - id: integration-batch\n")
        .expect("the skeleton declares the class");
    let block: String = class[at..]
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .map(|l| format!("{l}\n"))
        .collect();
    f.write(
        ".ai/repo/knowledge/sources.yaml",
        &format!("{}\n{block}", common::SOURCES),
    );
    f.write(
        ".ai/repo/integration/README.md",
        &common::context_doc("ai.repo.integration", "Integration"),
    );
    let master = "4090b2b8ae17c0ffee4090b2b8ae17c0ffee4090";
    let head = |n: u64| format!("{n:040x}");
    let (path, text) = manifest_text(
        master,
        &[(806, &head(1), &head(2)), (815, &head(3), &head(4))],
    );
    f.write(&path, &text);
    f.commit("a batch landed");
    let (code, v, err) = common::inspect(&f.root(), &[]);
    assert_eq!(code, 0, "{err}");
    let uri = "majordomus://integration-batch/4090b2b8ae-806-815";
    assert!(
        common::resource_uris(&v).iter().any(|u| u == uri),
        "{:?}",
        common::resource_uris(&v)
    );
    let about = |v: &Value| -> Vec<Value> {
        common::diagnostics(v)
            .into_iter()
            .filter(|d| d.to_string().contains("integration/batches"))
            .collect()
    };
    assert!(about(&v).is_empty(), "{:#?}", about(&v));
    // a key the contract does not know is a diagnostic naming the file, not a silent pass
    f.write(&path, &format!("{text}authored_fix: true\n"));
    f.commit("a manifest somebody edited");
    let (_, v, _) = common::inspect(&f.root(), &[]);
    let found = about(&v);
    assert!(
        found.iter().any(|d| d.to_string().contains("authored_fix")),
        "{found:#?}"
    );
}
