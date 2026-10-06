//! `majordomus prs repair` through the real command line (WP19, ADR 0101): bringing master into
//! one named pull request whose only conflict with it is over derived files, against a bare
//! origin and a scripted `gh`.
//!
//! The harness is the executor tests' (tests/integration_executor.rs on the executor branch),
//! copied rather than shared while that file is still being extended: a forge that answers
//! from files under its state directory and logs every call. The cases under test/cases drive
//! the same command from a shell; this file drives it from the test harness, so the executable
//! that runs is the instrumented one and the paths it takes are measured.

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
    /// regenerate, and the repair runs the derive of the tree it merged.
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
        git(&self.work, &["config", "user.email", "t@example.com"]);
        git(&self.work, &["config", "user.name", "t"]);
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        git(&self.work, &["add", "scripts/derive"]);
        git(&self.work, &["commit", "-q", "-m", "derive"]);
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
    }

    /// Pull request `n` changes the authored a.txt, and master changes it too afterwards.
    fn authored_conflict(&self, n: u64) -> String {
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
        std::fs::write(self.work.join("a.txt"), "the branch's\n").unwrap();
        git(
            &self.work,
            &["commit", "-q", "-am", "the branch rewrites a.txt"],
        );
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
        std::fs::write(self.work.join("a.txt"), "master's\n").unwrap();
        git(
            &self.work,
            &["commit", "-q", "-am", "master rewrites a.txt"],
        );
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
        git(&self.work, &["fetch", "-q", "origin"]);
        head
    }

    fn origin_ref(&self, name: &str) -> String {
        git(&self.origin, &["rev-parse", name])
    }

    fn actions(&self) -> Vec<String> {
        self.events()
            .into_iter()
            .filter_map(|e| e["action"].as_str().map(str::to_string))
            .collect()
    }
}

#[test]
fn a_dry_run_says_what_it_would_do_and_moves_nothing() {
    let f = Forge::new();
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["refresh"]);
    assert_eq!(code, 0, "refresh: {out}{err}");
    let before = f.log();
    let (code, out, err) = f.prs(&["repair", "1"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("dry run: would merge master"), "{out}");
    // named by its head branch, the same pull request and the same answer
    let (code, out, _) = f.prs(&["repair", "feature/1", "--dry-run"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("#1 (feature/1)"), "{out}");
    let (code, out, _) = f.prs(&["repair", "1", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    let report: Value = serde_json::from_str(&out).expect("the report as JSON");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["pr"], 1);
    assert_eq!(report["diagnostics"], serde_json::json!([]));
    // a dry run asks the forge nothing, records nothing and pushes nothing
    assert_eq!(f.log(), before, "a dry run asked the forge");
    assert_eq!(f.origin_ref("refs/heads/feature/1"), head);
    assert!(!f.actions().iter().any(|a| a.starts_with("repair")));
}

#[test]
fn a_dry_run_over_an_observation_the_clone_has_moved_past_exits_10() {
    let f = Forge::new();
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    assert_eq!(f.prs(&["refresh"]).0, 0);
    // master moves on and the clone fetches it, after the observation was taken
    std::fs::write(f.work.join("later2.txt"), "later still\n").unwrap();
    git(&f.work, &["add", "later2.txt"]);
    git(&f.work, &["commit", "-q", "-m", "later still"]);
    git(&f.work, &["push", "-q", "origin", "HEAD:master"]);
    git(&f.work, &["fetch", "-q", "origin"]);
    let (code, out, err) = f.prs(&["repair", "1"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(err.contains("stale observation"), "{err}");
    let (code, out, _) = f.prs(&["repair", "1", "--format", "json"]);
    assert_eq!(code, 10, "{out}");
    let report: Value = serde_json::from_str(&out).expect("the report as JSON");
    assert!(
        !report["diagnostics"].as_array().unwrap().is_empty(),
        "{out}"
    );
}

#[test]
fn applying_brings_master_in_after_the_trail_names_the_act() {
    let f = Forge::new();
    f.derives();
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("repaired #1 (feature/1)"), "{out}");
    let pushed = f.origin_ref("refs/heads/feature/1");
    assert_ne!(pushed, head, "nothing was pushed");
    // a fast-forward of the observed head, carrying master; master itself untouched
    git(&f.origin, &["merge-base", "--is-ancestor", &head, &pushed]);
    git(
        &f.origin,
        &["merge-base", "--is-ancestor", "master", &pushed],
    );
    let actions = f.actions();
    let at = |a: &str| {
        actions
            .iter()
            .position(|x| x == a)
            .unwrap_or_else(|| panic!("{a} not on the trail: {actions:?}"))
    };
    assert!(at("repair_selected") < at("repair_attempted"));
    assert!(at("repair_attempted") < at("repaired"));
    let repaired = f.last("repaired").unwrap();
    assert_eq!(repaired["head_after"], Value::from(pushed));
}

#[test]
fn an_authored_conflict_is_refused_naming_the_file_and_nothing_is_pushed() {
    let f = Forge::new();
    f.derives();
    let head = f.authored_conflict(1);
    f.open(&[(1, &head)]);
    assert_eq!(f.prs(&["refresh"]).0, 0);
    let (code, out, err) = f.prs(&["repair", "1"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("a.txt"), "{out}");
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("a.txt"), "{out}");
    assert_eq!(
        f.origin_ref("refs/heads/feature/1"),
        head,
        "a refused repair pushed"
    );
    assert!(
        f.last("repair_refused").is_some(),
        "the refusal is not on the trail"
    );
    assert!(
        f.last("repair_attempted").is_none(),
        "a refused repair was attempted"
    );
}

#[test]
fn a_derive_that_cannot_run_is_a_refused_act_and_the_branch_is_untouched() {
    let f = Forge::new();
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    let refused = f
        .last("repair_refused")
        .expect("repair_refused on the trail");
    assert!(
        refused["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("derive"),
        "{refused}"
    );
    assert_eq!(f.origin_ref("refs/heads/feature/1"), head);
}

#[test]
fn a_forge_that_cannot_be_read_leaves_the_act_unusable() {
    let f = Forge::new();
    f.derives();
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    f.set("down", "");
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 12, "{out}{err}");
    assert_eq!(f.origin_ref("refs/heads/feature/1"), head);
}

#[test]
fn a_target_that_is_not_an_open_pull_request_is_refused() {
    let f = Forge::new();
    let head = f.branch(1);
    f.open(&[(1, &head)]);
    assert_eq!(f.prs(&["refresh"]).0, 0);
    let (code, out, err) = f.prs(&["repair", "9"]);
    assert_eq!(code, 10, "{out}{err}");
    assert!(out.contains("not an open pull request"), "{out}");
    let (code, out, err) = f.prs(&["repair", "no/such-branch"]);
    assert_eq!(code, 10, "{out}{err}");
}

impl Forge {
    /// `scripts/derive` on master is `body`, a shell script run in the merge's worktree.
    fn derive_script(&self, body: &str) {
        let derive = self.work.join("scripts/derive");
        std::fs::write(&derive, format!("#!/bin/sh\n{body}\n")).unwrap();
        git(&self.work, &["add", "scripts/derive"]);
        git(
            &self.work,
            &["commit", "-q", "-m", "derive does something else"],
        );
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
    }

    /// gen.txt is a derived artifact (`merge=derived`), changed by pull request `n` and then by
    /// master, while the branch also changes its own authored n.txt.
    fn derived_conflict(&self, n: u64) -> String {
        git(&self.work, &["fetch", "-q", "origin"]);
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        std::fs::write(self.work.join(".gitattributes"), "gen.txt merge=derived\n").unwrap();
        std::fs::write(self.work.join("gen.txt"), "generated 0\n").unwrap();
        git(&self.work, &["add", ".gitattributes", "gen.txt"]);
        git(&self.work, &["commit", "-q", "-m", "a derived artifact"]);
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
        let head = {
            git(
                &self.work,
                &["checkout", "-q", "-B", &format!("feature/{n}")],
            );
            std::fs::write(self.work.join(format!("{n}.txt")), format!("{n}\n")).unwrap();
            std::fs::write(self.work.join("gen.txt"), format!("generated by {n}\n")).unwrap();
            git(&self.work, &["add", "-A"]);
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
            git(&self.work, &["rev-parse", "HEAD"])
        };
        git(&self.work, &["checkout", "-q", "--detach", "origin/master"]);
        std::fs::write(self.work.join("gen.txt"), "generated 1\n").unwrap();
        git(&self.work, &["commit", "-q", "-am", "master regenerates"]);
        git(&self.work, &["push", "-q", "origin", "HEAD:master"]);
        git(&self.work, &["fetch", "-q", "origin"]);
        head
    }
}

#[test]
fn a_derived_conflict_this_clone_has_no_driver_for_is_named_and_nothing_is_pushed() {
    let f = Forge::new();
    f.derives();
    let head = f.derived_conflict(1);
    f.open(&[(1, &head)]);
    assert_eq!(f.prs(&["refresh"]).0, 0);
    // git's answer, with the derived attribute, is that master is all it lacks
    let (code, out, err) = f.prs(&["repair", "1"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("would merge"), "{out}");
    // the act merges in a clone that declares no merge.derived driver: git's text merge
    // stops on gen.txt, and the refusal says which kind of path stopped it
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    let refused = f
        .last("repair_refused")
        .expect("repair_refused on the trail");
    let detail = refused["detail"].as_str().unwrap_or_default();
    assert!(detail.contains("gen.txt (derived)"), "{detail}");
    assert!(detail.contains("just derive-merge-driver"), "{detail}");
    assert_eq!(
        f.origin_ref("refs/heads/feature/1"),
        head,
        "a refused repair pushed"
    );
}

#[test]
fn a_derive_that_moves_the_merge_elsewhere_pushes_nothing() {
    let f = Forge::new();
    f.derives();
    // a derive that throws the merge away and commits on master instead: what would be
    // pushed no longer descends from the observed head
    f.derive_script(
        "git merge --abort && git checkout -q --detach origin/master && echo moved > moved.txt",
    );
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    let refused = f
        .last("repair_refused")
        .expect("repair_refused on the trail");
    let detail = refused["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("does not descend from the observed head"),
        "{detail}"
    );
    assert_eq!(
        f.origin_ref("refs/heads/feature/1"),
        head,
        "a refused repair pushed"
    );
}

/// `repair --apply` on #1 while its author moves the branch on origin to the commit in the
/// state's `move-to` file: the derive is the act's window between observing and pushing, so
/// the derive does the moving, with `push` (a fixture act on the scratch origin).
fn moved_during_the_act(push: &str, to: impl Fn(&Forge, &str) -> String) -> (Forge, String) {
    let f = Forge::new();
    f.derives();
    f.derive_script(&format!(
        "git -C '{}' {push} origin \"$(cat '{}')\":refs/heads/feature/1",
        f.work.display(),
        f.state.join("move-to").display()
    ));
    let head = f.branch(1);
    f.advance();
    f.open(&[(1, &head)]);
    let moved = to(&f, &head);
    std::fs::write(f.state.join("move-to"), &moved).unwrap();
    let (code, out, err) = f.prs(&["repair", "1", "--apply"]);
    assert_eq!(code, 10, "{out}{err}");
    let refused = f
        .last("repair_refused")
        .expect("repair_refused on the trail");
    let detail = refused["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains(&format!(
            "the branch moved since it was observed: {moved} is not {head}"
        )),
        "{detail}"
    );
    assert_eq!(
        f.origin_ref("refs/heads/feature/1"),
        moved,
        "the repair pushed over the author's move"
    );
    (f, head)
}

/// The author pushed on top of the observed head meanwhile: refused, nothing pushed. Git would
/// refuse this push by itself too, as no fast-forward of what origin now serves.
#[test]
fn a_branch_moved_forward_during_the_act_is_refused_and_nothing_is_pushed() {
    moved_during_the_act("push -q", |f, head| {
        git(
            &f.work,
            &[
                "commit-tree",
                &format!("{head}^{{tree}}"),
                "-p",
                head,
                "-m",
                "the author's next commit",
            ],
        )
    });
}

/// The author rewound the branch meanwhile: only the check against origin refuses this, since
/// a plain push of a descendant of the observed head would be a fast-forward of the rewound
/// branch and restore what the author removed.
#[test]
fn a_branch_rewound_during_the_act_is_refused_and_nothing_is_pushed() {
    moved_during_the_act("push -q -f", |f, head| {
        git(&f.work, &["rev-parse", &format!("{head}^")])
    });
}
