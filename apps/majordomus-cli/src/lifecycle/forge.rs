//! Pull requests, from the forge when it answers and from git when it does not.
//!
//! Two things are deliberate here. The first is that the forge is optional and bounded: the
//! command is run with a deadline and a killed child, and its absence produces
//! [`super::model::PullRequestSource::Git`] with a stated limitation, never an empty list
//! that reads as "there are no open pull requests". The second is that the forge's own
//! `mergeable` field is recorded and never believed. This repository configures a
//! `merge=derived` driver per clone, which the forge cannot run, so it reports a conflict
//! in every generated file: six of eight open pull requests read `CONFLICTING` on the forge
//! on a day when `git merge-tree --write-tree` merged all six cleanly here. The verdict
//! comes from [`super::measure::conflicts_with`]; the forge's word is kept beside it as the
//! hint it is.

use std::path::Path;
use std::time::Duration;

use super::measure::{conflicts_with, parse_rfc3339_utc, run_bounded};
use super::model::{PullRequestSource, PullRequestState, PullRequestView};
use super::Clock;

/// How long the forge is given to answer before the child is killed.
const FORGE_BOUND: Duration = Duration::from_secs(20);

/// How many pull requests are asked for.
const FORGE_LIMIT: &str = "200";

/// Where pull request facts are wanted from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Source {
    /// Ask the forge; fall back to git when it does not answer. The default.
    #[default]
    Auto,
    /// Git only: no subprocess leaves this machine, and open pull requests are invisible.
    Git,
}

/// What a read of the forge produced.
#[derive(Debug, Clone, Default)]
pub struct Forge {
    /// The pull requests the forge reported, empty when it was not reached.
    pub pull_requests: Vec<PullRequestView>,
    /// Where these came from.
    pub source: PullRequestSource,
    /// What could not be seen, when something could not.
    pub limitations: Vec<String>,
}

impl Default for PullRequestSource {
    fn default() -> Self {
        PullRequestSource::Git
    }
}

/// Read the pull requests, resolving every open one's mergeability locally against `trunk`.
pub fn read(repo: &Path, trunk: Option<&str>, source: Source, clock: Clock) -> Forge {
    if source == Source::Git {
        return Forge {
            pull_requests: Vec::new(),
            source: PullRequestSource::Git,
            limitations: vec![
                "pull requests were read from git alone, by request: the trunk's own merge \
                 commits say exactly what landed, and open pull requests are not visible \
                 from here"
                    .into(),
            ],
        };
    }
    let raw = run_bounded(
        "gh",
        &[
            "pr",
            "list",
            "--state",
            "all",
            "--limit",
            FORGE_LIMIT,
            "--json",
            "number,title,headRefName,state,isDraft,mergeable,createdAt,mergeCommit,statusCheckRollup",
        ],
        repo,
        FORGE_BOUND,
    );
    let Some(raw) = raw else {
        return Forge {
            pull_requests: Vec::new(),
            source: PullRequestSource::Git,
            limitations: vec![
                "the forge was not reached (`gh` is absent, unauthenticated, or did not \
                 answer within the bound): open pull requests are not in this report, and \
                 their absence is not evidence that there are none"
                    .into(),
            ],
        };
    };
    let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else {
        return Forge {
            pull_requests: Vec::new(),
            source: PullRequestSource::Git,
            limitations: vec![
                "the forge answered with something this reader does not understand; pull \
                 requests were taken from git alone"
                    .into(),
            ],
        };
    };
    let mut views = Vec::new();
    for entry in parsed {
        if let Some(view) = view_of(&entry, repo, trunk, clock) {
            views.push(view);
        }
    }
    views.sort_by_key(|v| v.number);
    Forge {
        pull_requests: views,
        source: PullRequestSource::Forge,
        limitations: Vec::new(),
    }
}

fn view_of(
    entry: &serde_json::Value,
    repo: &Path,
    trunk: Option<&str>,
    clock: Clock,
) -> Option<PullRequestView> {
    let number = entry.get("number")?.as_u64()?;
    let branch = entry
        .get("headRefName")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let forge_state = entry.get("state").and_then(|v| v.as_str()).unwrap_or("");
    let draft = entry
        .get("isDraft")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let forge_mergeable = entry
        .get("mergeable")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let merge_commit = entry
        .get("mergeCommit")
        .and_then(|v| v.get("oid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let age_days = entry
        .get("createdAt")
        .and_then(|v| v.as_str())
        .and_then(parse_rfc3339_utc)
        .map(|t| clock.days_since(t));

    // The local verdict, which is the one that counts.
    let conflicts_locally = match (trunk, branch.as_deref()) {
        (Some(trunk), Some(branch)) if forge_state == "OPEN" => {
            conflicts_with(repo, trunk, branch)
        }
        _ => None,
    };
    // Did the content land anyway, by another route?
    let already_in_trunk = match (trunk, branch.as_deref()) {
        (Some(trunk), Some(branch)) if forge_state == "OPEN" => {
            super::measure::is_ancestor(repo, branch, trunk).unwrap_or(false)
        }
        _ => false,
    };
    let checks = check_rollup(entry);

    let (state, evidence) = match forge_state {
        "MERGED" => (
            PullRequestState::Landed,
            "the forge reports it merged".to_string(),
        ),
        "CLOSED" => (
            PullRequestState::Abandoned,
            "closed without landing".to_string(),
        ),
        _ if already_in_trunk => (
            PullRequestState::Superseded,
            "open, but its head is already reachable from the trunk: the content landed by \
             another route"
                .to_string(),
        ),
        _ if draft => (
            PullRequestState::Draft,
            "open and marked a draft by its author".to_string(),
        ),
        _ if conflicts_locally == Some(true) => (
            PullRequestState::Blocked,
            format!(
                "`git merge-tree --write-tree` conflicts against the trunk in this clone \
                 (the forge said {})",
                forge_mergeable.as_deref().unwrap_or("nothing")
            ),
        ),
        _ => match checks {
            Checks::Failing => (
                PullRequestState::Failing,
                "a check has failed".to_string(),
            ),
            Checks::Pending => (
                PullRequestState::AwaitingCi,
                "checks are running or have not reported".to_string(),
            ),
            Checks::Green => (
                PullRequestState::Mergeable,
                match (conflicts_locally, forge_mergeable.as_deref()) {
                    (Some(false), Some(forge)) if forge != "MERGEABLE" => format!(
                        "merges cleanly here and every check is green; the forge says \
                         {forge}, which it cannot know — it has no `merge=derived` driver"
                    ),
                    _ => "merges cleanly here and every check is green".to_string(),
                },
            ),
            Checks::Unknown => (
                PullRequestState::Unknown,
                "the forge reported no check status for this pull request".to_string(),
            ),
        },
    };

    Some(PullRequestView {
        number,
        title: entry
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        branch,
        state,
        source: PullRequestSource::Forge,
        merge_commit,
        age_days,
        conflicts_locally,
        forge_mergeable,
        evidence,
    })
}

/// What the checks of a pull request add up to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Checks {
    Green,
    Failing,
    Pending,
    Unknown,
}

fn check_rollup(entry: &serde_json::Value) -> Checks {
    let Some(items) = entry.get("statusCheckRollup").and_then(|v| v.as_array()) else {
        return Checks::Unknown;
    };
    if items.is_empty() {
        return Checks::Unknown;
    }
    let mut pending = false;
    for item in items {
        let conclusion = item
            .get("conclusion")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        let status = item
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        let state = item
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        if matches!(
            conclusion.as_str(),
            "FAILURE" | "TIMED_OUT" | "CANCELLED" | "STARTUP_FAILURE" | "ACTION_REQUIRED"
        ) || state == "FAILURE"
            || state == "ERROR"
        {
            return Checks::Failing;
        }
        let settled = conclusion == "SUCCESS"
            || conclusion == "SKIPPED"
            || conclusion == "NEUTRAL"
            || state == "SUCCESS";
        if !settled || (status != "COMPLETED" && !status.is_empty()) {
            pending = true;
        }
    }
    if pending {
        Checks::Pending
    } else {
        Checks::Green
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("the fixture is valid JSON")
    }

    #[test]
    fn a_failing_check_outranks_a_pending_one() {
        let v = entry(
            r#"{"statusCheckRollup":[
                {"status":"COMPLETED","conclusion":"SUCCESS"},
                {"status":"IN_PROGRESS","conclusion":null},
                {"status":"COMPLETED","conclusion":"FAILURE"}]}"#,
        );
        assert_eq!(check_rollup(&v), Checks::Failing);
    }

    #[test]
    fn no_checks_at_all_is_unknown_never_green() {
        assert_eq!(check_rollup(&entry(r#"{}"#)), Checks::Unknown);
        assert_eq!(
            check_rollup(&entry(r#"{"statusCheckRollup":[]}"#)),
            Checks::Unknown
        );
    }

    #[test]
    fn every_check_settled_is_green() {
        let v = entry(
            r#"{"statusCheckRollup":[
                {"status":"COMPLETED","conclusion":"SUCCESS"},
                {"status":"COMPLETED","conclusion":"SKIPPED"}]}"#,
        );
        assert_eq!(check_rollup(&v), Checks::Green);
    }

    #[test]
    fn asking_for_git_only_says_what_it_cannot_see() {
        let forge = read(
            Path::new("."),
            Some("master"),
            Source::Git,
            Clock::fixed(1_788_000_000),
        );
        assert_eq!(forge.source, PullRequestSource::Git);
        assert!(forge.pull_requests.is_empty());
        assert_eq!(
            forge.limitations.len(),
            1,
            "a source that cannot see open pull requests must say so"
        );
    }
}
