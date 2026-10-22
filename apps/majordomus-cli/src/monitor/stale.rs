//! Which workflow runs in flight nobody will read, and which must never be cancelled.
//!
//! `project.an-expedited-landing-is-the-owners-word` clause 7 says a run is stale when its
//! pull request is merged or closed, when its head is no longer the head of its branch, or
//! when another run of the same workflow already judges the same commit — and that four
//! kinds of run are never cancelled: the trunk's only run of a commit, a deployment in
//! flight, a release run, and a run on a head that is still current. Until this module the
//! judgement was a session reading `gh run list`, and one misreading cancels the only
//! verdict a commit will ever get.
//!
//! [`judge`] is that judgement as a function of what the forge printed: every run in flight
//! gets one [`RunVerdict`] with its reason, and only a stale one carries the command that
//! cancels it. Ignorance keeps a run: a branch whose head could not be read, or a run that
//! somebody dispatched by hand, is kept and says why. Nothing here cancels anything.

use std::collections::HashMap;
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{github, MonitorSourceHealth};

/// The trunk: its only run of a commit is the verdict releases are cut on.
pub(crate) const TRUNK: &str = "master";

/// Whether a run in flight may be cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunStanding {
    /// Nobody will read its verdict: it may be cancelled.
    Stale,
    /// Somebody will read it, or nothing here can show that nobody will: it stays.
    Kept,
}

/// Why a run is stale or kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunReason {
    /// Stale: the pull request of its branch is merged.
    PullRequestMerged,
    /// Stale: the pull request of its branch is closed without a merge.
    PullRequestClosed,
    /// Stale: its commit is no longer the head of its branch.
    HeadMoved,
    /// Stale: its branch no longer exists.
    BranchGone,
    /// Stale: an older run of the same workflow already judges the same commit.
    Duplicate,
    /// Kept: a release run.
    Release,
    /// Kept: a deployment in flight.
    Deployment,
    /// Kept: the trunk's only run of this commit by this workflow.
    TrunkOnlyRun,
    /// Kept: its commit is the head of its branch.
    CurrentHead,
    /// Kept: somebody dispatched it by hand, or another workflow asked for it.
    AskedFor,
    /// Kept: the head of its branch could not be read.
    HeadUnknown,
}

impl RunReason {
    /// The standing this reason gives a run.
    pub(crate) fn standing(self) -> RunStanding {
        match self {
            RunReason::PullRequestMerged
            | RunReason::PullRequestClosed
            | RunReason::HeadMoved
            | RunReason::BranchGone
            | RunReason::Duplicate => RunStanding::Stale,
            RunReason::Release
            | RunReason::Deployment
            | RunReason::TrunkOnlyRun
            | RunReason::CurrentHead
            | RunReason::AskedFor
            | RunReason::HeadUnknown => RunStanding::Kept,
        }
    }
}

/// One run in flight, judged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunVerdict {
    /// The run's id on the forge.
    pub id: u64,
    /// The workflow's name.
    pub workflow: String,
    /// The branch the run is of.
    pub branch: String,
    /// The commit the run judges.
    pub commit: String,
    /// What started it: `push`, `pull_request`, `schedule`, `workflow_dispatch`, ...
    pub event: String,
    /// `queued`, `in_progress`, `waiting`, ... as the forge says it.
    pub status: String,
    /// Whether it may be cancelled.
    pub standing: RunStanding,
    /// Why.
    pub reason: RunReason,
    /// The reason in one sentence, with what it was decided against.
    pub detail: String,
    /// The command that cancels it; present only for a stale run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<String>,
}

/// What the forge printed, as [`judge`] reads it.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
pub(crate) struct Listing {
    /// `gh run list --json databaseId,workflowName,status,event,headBranch,headSha,createdAt`.
    #[serde(default)]
    pub(crate) runs: Value,
    /// `gh pr list --state all --json number,state,headRefName,headRefOid`.
    #[serde(default)]
    pub(crate) pull_requests: Value,
    /// The head of each branch that no pull request decides: its commit, or `None` when the
    /// forge says the branch does not exist. A branch that is not a key here could not be
    /// read, which keeps every run that would have been decided by it.
    #[serde(default)]
    pub(crate) heads: HashMap<String, Option<String>>,
}

/// How many runs and pull requests one fetch asks for.
const RUNS: &str = "100";
const PULL_REQUESTS: &str = "200";

/// The most branches one fetch asks the head of; a branch beyond it stays unread, and its
/// runs are kept.
const HEADS: usize = 10;

/// What the forge says now about the repository at `root`: the runs, the pull requests, and
/// the head of every branch with a run in flight that no pull request decides.
pub(crate) fn fetch(root: &Path) -> Result<Listing, (MonitorSourceHealth, String)> {
    fetch_with(github::GH, root)
}

/// [`fetch`], asking `program` in place of `gh`: the seam a test puts its own program in.
pub(crate) fn fetch_with(
    program: &str,
    root: &Path,
) -> Result<Listing, (MonitorSourceHealth, String)> {
    let runs = github::run(
        program,
        root,
        &[
            "run",
            "list",
            "--limit",
            RUNS,
            "--json",
            "databaseId,workflowName,status,event,headBranch,headSha,createdAt",
        ],
    )?;
    let pull_requests = github::run(
        program,
        root,
        &[
            "pr",
            "list",
            "--state",
            "all",
            "--limit",
            PULL_REQUESTS,
            "--json",
            "number,state,headRefName,headRefOid",
        ],
    )?;
    let mut listing = Listing {
        runs,
        pull_requests,
        heads: HashMap::new(),
    };
    for branch in undecided(&listing).into_iter().take(HEADS) {
        let path = format!("repos/{{owner}}/{{repo}}/branches/{branch}");
        match github::run(program, root, &["api", &path]) {
            Ok(answer) => {
                if let Some(head) = answer["commit"]["sha"].as_str() {
                    listing.heads.insert(branch, Some(head.to_string()));
                }
            }
            // the forge answered, and the answer is that there is no such branch
            Err((_, reason)) if reason.contains("404") => {
                listing.heads.insert(branch, None);
            }
            // anything else is not knowing, and not knowing keeps the run
            Err(_) => {}
        }
    }
    Ok(listing)
}

/// The branches with a run in flight that no pull request decides, in the order of their
/// first run, each once: the ones whose head has to be asked for.
fn undecided(listing: &Listing) -> Vec<String> {
    let mut branches: Vec<String> = Vec::new();
    for run in listing.runs.as_array().into_iter().flatten() {
        let branch = text(run, "headBranch");
        if text(run, "status") == "completed" || branch == TRUNK || branches.contains(&branch) {
            continue;
        }
        let (open, merged, closed) = pull_requests_of(listing, &branch);
        if open.is_none() && merged.is_none() && closed.is_none() {
            branches.push(branch);
        }
    }
    branches
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

/// Whether a workflow publishes a release.
fn is_release(workflow: &str) -> bool {
    workflow.to_ascii_lowercase().contains("release")
}

/// Whether a workflow deploys: the site, or an image.
fn is_deployment(workflow: &str) -> bool {
    let lower = workflow.to_ascii_lowercase();
    lower.contains("pages") || lower.contains("deploy") || lower.contains("image")
}

/// The pull requests of a branch, as (open head, merged number, closed number): an open one
/// outranks a merged one, which outranks a closed one, because a branch reused after a merge
/// has a reader again.
fn pull_requests_of(listing: &Listing, branch: &str) -> (Option<String>, Option<u64>, Option<u64>) {
    let mut open = None;
    let mut merged = None;
    let mut closed = None;
    for pr in listing.pull_requests.as_array().into_iter().flatten() {
        if text(pr, "headRefName") != branch {
            continue;
        }
        let number = pr["number"].as_u64();
        match text(pr, "state").to_ascii_uppercase().as_str() {
            "OPEN" => open = Some(text(pr, "headRefOid")),
            "MERGED" => merged = merged.or(number),
            "CLOSED" => closed = closed.or(number),
            _ => {}
        }
    }
    (open, merged, closed)
}

/// Judge every run in flight of `listing`, in the order the forge listed them.
///
/// A completed run is not in the answer: it holds no runner and cannot be cancelled.
pub(crate) fn judge(listing: &Listing) -> Vec<RunVerdict> {
    let in_flight: Vec<&Value> = listing
        .runs
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| text(r, "status") != "completed")
        .collect();
    // the oldest run in flight of each (workflow, commit) is the one that judges it
    let mut first: HashMap<(String, String), (String, u64)> = HashMap::new();
    for run in &in_flight {
        let key = (text(run, "workflowName"), text(run, "headSha"));
        let at = (text(run, "createdAt"), run["databaseId"].as_u64().unwrap_or(0));
        match first.get(&key) {
            Some(known) if *known <= at => {}
            _ => {
                first.insert(key, at);
            }
        }
    }
    in_flight
        .into_iter()
        .map(|run| {
            let id = run["databaseId"].as_u64().unwrap_or(0);
            let workflow = text(run, "workflowName");
            let branch = text(run, "headBranch");
            let commit = text(run, "headSha");
            let event = text(run, "event");
            let (reason, detail) = reason_of(listing, &first, id, &workflow, &branch, &commit, &event);
            let standing = reason.standing();
            RunVerdict {
                id,
                workflow,
                branch,
                commit,
                event,
                status: text(run, "status"),
                standing,
                reason,
                detail,
                cancel: (standing == RunStanding::Stale).then(|| format!("gh run cancel {id}")),
            }
        })
        .collect()
}

fn short(commit: &str) -> &str {
    commit.get(..10).unwrap_or(commit)
}

/// The reason of one run. The order is the rule's: what is never cancelled is decided
/// before anything that could make a run stale.
fn reason_of(
    listing: &Listing,
    first: &HashMap<(String, String), (String, u64)>,
    id: u64,
    workflow: &str,
    branch: &str,
    commit: &str,
    event: &str,
) -> (RunReason, String) {
    if is_release(workflow) {
        return (
            RunReason::Release,
            format!("{workflow} is a release run, and a release run is never cancelled"),
        );
    }
    if is_deployment(workflow) {
        return (
            RunReason::Deployment,
            format!("{workflow} is a deployment in flight, and one is never cancelled"),
        );
    }
    if event == "workflow_dispatch" || event == "workflow_call" || event == "repository_dispatch" {
        return (
            RunReason::AskedFor,
            format!("started by {event}: somebody asked for this run"),
        );
    }
    let duplicate = first
        .get(&(workflow.to_string(), commit.to_string()))
        .filter(|(_, judging)| *judging != id)
        .map(|(_, judging)| *judging);
    if branch == TRUNK {
        return match duplicate {
            Some(judging) => (
                RunReason::Duplicate,
                format!(
                    "run {judging} of {workflow} already judges {} on the trunk",
                    short(commit)
                ),
            ),
            None => (
                RunReason::TrunkOnlyRun,
                format!(
                    "the trunk's only run of {} by {workflow}: the verdict a release is cut on",
                    short(commit)
                ),
            ),
        };
    }
    let (open, merged, closed) = pull_requests_of(listing, branch);
    let head = match open {
        Some(head) if !head.is_empty() => Some(head),
        _ => {
            if let Some(number) = merged {
                return (
                    RunReason::PullRequestMerged,
                    format!("#{number} of {branch} is merged: nobody reads this run"),
                );
            }
            if let Some(number) = closed {
                return (
                    RunReason::PullRequestClosed,
                    format!("#{number} of {branch} is closed: nobody reads this run"),
                );
            }
            match listing.heads.get(branch) {
                None => {
                    return (
                        RunReason::HeadUnknown,
                        format!("the head of {branch} could not be read, so the run is kept"),
                    )
                }
                Some(None) => {
                    return (
                        RunReason::BranchGone,
                        format!("{branch} no longer exists on the forge"),
                    )
                }
                Some(Some(head)) => Some(head.clone()),
            }
        }
    };
    let head = head.unwrap_or_default();
    if head != commit {
        return (
            RunReason::HeadMoved,
            format!(
                "{branch} is at {}, and this run judges {}",
                short(&head),
                short(commit)
            ),
        );
    }
    match duplicate {
        Some(judging) => (
            RunReason::Duplicate,
            format!(
                "run {judging} of {workflow} already judges {}",
                short(commit)
            ),
        ),
        None => (
            RunReason::CurrentHead,
            format!("{} is the head of {branch}", short(commit)),
        ),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run(id: u64, workflow: &str, branch: &str, sha: &str, event: &str) -> Value {
        json!({
            "databaseId": id, "workflowName": workflow, "headBranch": branch, "headSha": sha,
            "event": event, "status": "in_progress",
            "createdAt": format!("2026-10-13T10:{:02}:00Z", id % 60),
        })
    }

    fn pr(number: u64, state: &str, branch: &str, head: &str) -> Value {
        json!({ "number": number, "state": state, "headRefName": branch, "headRefOid": head })
    }

    fn listing(runs: Vec<Value>, prs: Vec<Value>, heads: &[(&str, Option<&str>)]) -> Listing {
        Listing {
            runs: Value::Array(runs),
            pull_requests: Value::Array(prs),
            heads: heads
                .iter()
                .map(|(b, s)| (b.to_string(), s.map(str::to_string)))
                .collect(),
        }
    }

    fn reasons(l: &Listing) -> Vec<(u64, RunReason)> {
        judge(l).into_iter().map(|v| (v.id, v.reason)).collect()
    }

    /// The four kinds the rule never cancels, each kept for its own reason, and none of
    /// them carries a command.
    #[test]
    fn what_is_never_cancelled_is_kept_and_carries_no_command() {
        let l = listing(
            vec![
                run(1, "validate", "master", "aaaa", "push"),
                run(2, "pages", "master", "aaaa", "push"),
                run(3, "release", "v1.0.0", "aaaa", "push"),
                run(4, "validate", "fix/x", "bbbb", "pull_request"),
                run(5, "docker-image", "master", "aaaa", "push"),
            ],
            vec![pr(7, "OPEN", "fix/x", "bbbb")],
            &[],
        );
        assert_eq!(
            reasons(&l),
            vec![
                (1, RunReason::TrunkOnlyRun),
                (2, RunReason::Deployment),
                (3, RunReason::Release),
                (4, RunReason::CurrentHead),
                (5, RunReason::Deployment),
            ]
        );
        assert!(judge(&l)
            .iter()
            .all(|v| v.standing == RunStanding::Kept && v.cancel.is_none()));
    }

    /// The trunk's run of an older commit is still that commit's only verdict: a newer
    /// commit on the trunk does not make it stale.
    #[test]
    fn an_older_trunk_commit_keeps_its_only_run() {
        let l = listing(
            vec![
                run(1, "validate", "master", "old0", "push"),
                run(2, "validate", "master", "new0", "push"),
            ],
            vec![],
            &[("master", Some("new0"))],
        );
        assert_eq!(
            reasons(&l),
            vec![(1, RunReason::TrunkOnlyRun), (2, RunReason::TrunkOnlyRun)]
        );
    }

    /// Merged, closed, moved, gone and duplicated: each stale for its reason, each with the
    /// command that cancels exactly that run.
    #[test]
    fn a_run_nobody_will_read_is_stale_and_names_its_command() {
        let l = listing(
            vec![
                run(10, "validate", "fix/merged", "m000", "pull_request"),
                run(11, "validate", "fix/closed", "c000", "pull_request"),
                run(12, "validate", "fix/moved", "old0", "pull_request"),
                run(13, "validate", "fix/gone", "g000", "push"),
                run(14, "validate", "master", "aaaa", "push"),
                run(15, "validate", "master", "aaaa", "schedule"),
                run(16, "validate", "fix/pushed", "old1", "push"),
            ],
            vec![
                pr(1, "MERGED", "fix/merged", "m000"),
                pr(2, "CLOSED", "fix/closed", "c000"),
                pr(3, "OPEN", "fix/moved", "new0"),
            ],
            &[("fix/pushed", Some("new1")), ("fix/gone", None)],
        );
        assert_eq!(
            reasons(&l),
            vec![
                (10, RunReason::PullRequestMerged),
                (11, RunReason::PullRequestClosed),
                (12, RunReason::HeadMoved),
                (13, RunReason::BranchGone),
                (14, RunReason::TrunkOnlyRun),
                (15, RunReason::Duplicate),
                (16, RunReason::HeadMoved),
            ]
        );
        let verdicts = judge(&l);
        assert_eq!(verdicts[0].cancel.as_deref(), Some("gh run cancel 10"));
        assert!(verdicts[0].detail.contains("#1 of fix/merged is merged"));
        assert!(verdicts[5].detail.contains("run 14 of validate already judges aaaa"));
        assert_eq!(verdicts[4].cancel, None);
    }

    /// Ignorance keeps a run: heads that could not be read, and a run somebody asked for.
    #[test]
    fn what_cannot_be_decided_is_kept() {
        let l = listing(
            vec![
                run(20, "validate", "fix/unknown", "u000", "push"),
                run(21, "validate", "release/record-v1", "r000", "workflow_dispatch"),
            ],
            vec![pr(9, "MERGED", "release/record-v1", "r000")],
            &[],
        );
        assert_eq!(
            reasons(&l),
            vec![(20, RunReason::HeadUnknown), (21, RunReason::AskedFor)]
        );
    }

    /// A branch reused after a merge has a reader again: the open pull request decides.
    #[test]
    fn an_open_pull_request_outranks_a_merged_one_of_the_same_branch() {
        let l = listing(
            vec![run(30, "validate", "fix/again", "h000", "pull_request")],
            vec![
                pr(1, "MERGED", "fix/again", "old0"),
                pr(2, "OPEN", "fix/again", "h000"),
            ],
            &[],
        );
        assert_eq!(reasons(&l), vec![(30, RunReason::CurrentHead)]);
    }

    /// Only a branch with a run in flight and no pull request has its head asked for.
    #[test]
    fn only_a_branch_no_pull_request_decides_has_its_head_asked_for() {
        let l = listing(
            vec![
                run(1, "validate", "master", "aaaa", "push"),
                run(2, "validate", "fix/with-pr", "bbbb", "pull_request"),
                run(3, "validate", "fix/bare", "cccc", "push"),
                run(4, "pages", "fix/bare", "cccc", "push"),
            ],
            vec![pr(1, "OPEN", "fix/with-pr", "bbbb")],
            &[],
        );
        assert_eq!(undecided(&l), vec!["fix/bare".to_string()]);
    }

    /// The fetch through a stand-in for `gh`: runs, pull requests, one head that exists and
    /// one the forge says is gone; and a program that cannot be started is the source's
    /// state, never an empty listing.
    #[cfg(unix)]
    #[test]
    fn a_fetch_reads_runs_pull_requests_and_the_heads_nothing_else_decides() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let gh = dir.path().join("gh");
        std::fs::write(
            &gh,
            r#"#!/bin/sh
case "$1 $2" in
  "run list") echo '[{"databaseId":3,"workflowName":"validate","status":"queued","event":"push","headBranch":"fix/bare","headSha":"cccc","createdAt":"2026-10-13T10:00:00Z"},{"databaseId":4,"workflowName":"validate","status":"queued","event":"push","headBranch":"fix/gone","headSha":"dddd","createdAt":"2026-10-13T10:01:00Z"}]' ;;
  "pr list") echo '[]' ;;
  "api repos/{owner}/{repo}/branches/fix/bare") echo '{"commit":{"sha":"cccc"}}' ;;
  *) echo "gh: Branch not found (HTTP 404)" >&2; exit 1 ;;
esac
"#,
        )
        .unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        let l = fetch_with(gh.to_str().unwrap(), dir.path()).unwrap();
        assert_eq!(l.heads.get("fix/bare"), Some(&Some("cccc".to_string())));
        assert_eq!(l.heads.get("fix/gone"), Some(&None));
        assert_eq!(
            reasons(&l),
            vec![(3, RunReason::CurrentHead), (4, RunReason::BranchGone)]
        );
        let missing = dir.path().join("no-such-program");
        let (state, reason) = fetch_with(missing.to_str().unwrap(), dir.path()).unwrap_err();
        assert_eq!(state, MonitorSourceHealth::Absent);
        assert!(reason.contains("could not be started"), "{reason}");
    }

    /// A completed run is not judged, and every reason has the standing its name says.
    #[test]
    fn a_completed_run_is_not_in_the_answer() {
        let mut done = run(40, "validate", "fix/x", "aaaa", "push");
        done["status"] = json!("completed");
        assert!(judge(&listing(vec![done], vec![], &[])).is_empty());
        assert_eq!(RunReason::Duplicate.standing(), RunStanding::Stale);
        assert_eq!(RunReason::AskedFor.standing(), RunStanding::Kept);
    }
}
