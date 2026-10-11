//! GitHub, folded into signals: pull requests opened, merged and closed, and finished
//! workflow runs.
//!
//! GitHub keeps the history, so the read is stateless: each pull request and each run says
//! when it was opened, merged, closed or finished, and a signal is one of those moments
//! after the cursor's second. Reaching GitHub is the network, so the fetch is bounded in
//! time, shared by every reader for [`TTL`], and an unreachable or missing `gh` is a source
//! state with its reason — never a quiet repository.
//!
//! A run that succeeded is news only on the trunk (master's checks, a deploy, a release);
//! a pull request's green run is the expected and is counted as suppressed. A failed run is
//! always news.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::peers::parse_rfc3339;

use super::{
    MonitorSeverity, MonitorSignal, MonitorSource, MonitorSourceHealth, Reader, SourceRead,
};

/// How long one fetch is shared by every reader.
pub(crate) const TTL: Duration = Duration::from_secs(30);

/// How long one `gh` call may take before the source is reported as unreachable.
pub(crate) const BUDGET: Duration = Duration::from_secs(4);

/// How many pull requests and runs one fetch asks for.
const WINDOW: usize = 40;

/// The trunk: a run on it is news even when it succeeds.
const TRUNK: &str = "master";

/// What one fetch brought back: pull requests and runs as `gh` prints them.
#[derive(Debug, Clone)]
pub(crate) struct Fetched {
    pub(crate) prs: Value,
    pub(crate) runs: Value,
}

type Fetch = Result<Fetched, (MonitorSourceHealth, String)>;

static CACHE: Mutex<Option<(PathBuf, Instant, Fetch)>> = Mutex::new(None);

/// The pull requests and runs of the repository at `root`, from the cache when it is
/// younger than [`TTL`].
pub(crate) fn fetch(root: &Path) -> Fetch {
    if let Ok(cache) = CACHE.lock() {
        if let Some((at_root, at, fetched)) = cache.as_ref() {
            if at_root == root && at.elapsed() < TTL {
                return fetched.clone();
            }
        }
    }
    let fetched = fetch_now(root);
    if let Ok(mut cache) = CACHE.lock() {
        *cache = Some((root.to_path_buf(), Instant::now(), fetched.clone()));
    }
    fetched
}

fn fetch_now(root: &Path) -> Fetch {
    let prs = gh(
        root,
        &[
            "pr",
            "list",
            "--state",
            "all",
            "--limit",
            &WINDOW.to_string(),
            "--json",
            "number,title,state,createdAt,mergedAt,closedAt,headRefName,author,url",
        ],
    )?;
    let runs = gh(
        root,
        &[
            "run",
            "list",
            "--limit",
            &WINDOW.to_string(),
            "--json",
            "databaseId,workflowName,displayTitle,status,conclusion,event,headBranch,updatedAt,url",
        ],
    )?;
    Ok(Fetched { prs, runs })
}

/// One `gh` call, bounded by [`BUDGET`], its stdout parsed as JSON.
fn gh(root: &Path, args: &[&str]) -> Result<Value, (MonitorSourceHealth, String)> {
    run("gh", root, args)
}

/// One call of `program`, bounded by [`BUDGET`], its stdout parsed as JSON; every failure is
/// reported as `gh`'s, because `gh` is the only program asked in earnest.
fn run(program: &str, root: &Path, args: &[&str]) -> Result<Value, (MonitorSourceHealth, String)> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            (
                MonitorSourceHealth::Absent,
                format!("gh could not be started: {e}"),
            )
        })?;
    let deadline = Instant::now() + BUDGET;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err((
                    MonitorSourceHealth::Error,
                    format!("gh {} took longer than {}s", args[0], BUDGET.as_secs()),
                ));
            }
            Err(e) => return Err((MonitorSourceHealth::Error, format!("gh: {e}"))),
        }
    };
    let mut out = String::new();
    let mut err = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut out);
    }
    if let Some(mut s) = child.stderr.take() {
        let _ = s.read_to_string(&mut err);
    }
    if !status.success() {
        let reason = err
            .lines()
            .next()
            .unwrap_or("no reason given")
            .trim()
            .to_string();
        return Err((
            MonitorSourceHealth::Error,
            format!("gh {} failed: {reason}", args[0]),
        ));
    }
    serde_json::from_str(&out).map_err(|e| {
        (
            MonitorSourceHealth::Error,
            format!("gh {} printed no JSON: {e}", args[0]),
        )
    })
}

/// Fold what `gh` printed into signals after `after` (Unix seconds); without a position,
/// only moments at or after `since` are shown.
pub(crate) fn fold(
    fetched: &Fetched,
    after: Option<u64>,
    since: u64,
    reader: &Reader,
) -> SourceRead<u64> {
    let mut moments: Vec<(u64, Option<MonitorSignal>)> = Vec::new();
    for pr in fetched.prs.as_array().into_iter().flatten() {
        moments.extend(pull_request(pr, reader));
    }
    for run in fetched.runs.as_array().into_iter().flatten() {
        if let Some(moment) = workflow_run(run, reader) {
            moments.push(moment);
        }
    }
    moments.sort_by_key(|(at, _)| *at);
    let floor = after.unwrap_or_else(|| since.saturating_sub(1));
    let mut read = SourceRead::default();
    for (at, signal) in moments {
        read.high = Some(read.high.map_or(at, |h: u64| h.max(at)));
        if at <= floor {
            continue;
        }
        match signal {
            Some(signal) => read.signals.push((at, signal)),
            None => read.suppressed += 1,
        }
    }
    if let Some(after) = after {
        read.high = Some(read.high.map_or(after, |h| h.max(after)));
    }
    read
}

fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_string()
}

fn time(v: &Value, key: &str) -> Option<u64> {
    v[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .and_then(parse_rfc3339)
}

/// The moments of one pull request: opened, and merged or closed.
fn pull_request(pr: &Value, reader: &Reader) -> Vec<(u64, Option<MonitorSignal>)> {
    let number = pr["number"].as_u64();
    let title = text(pr, "title");
    let branch = text(pr, "headRefName");
    let author = pr["author"]["login"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let n = number.map(|n| format!("#{n}")).unwrap_or_default();
    let make = |at: u64, kind: &str, severity, what: String| {
        let mut s = MonitorSignal::new(at, MonitorSource::Github, kind, severity, &what);
        s.pr = number;
        s.branch = Some(branch.clone()).filter(|b| !b.is_empty());
        s.actor = Some(author.clone()).filter(|a| !a.is_empty());
        s.link = Some(text(pr, "url")).filter(|u| !u.is_empty());
        s.for_you = reader.named_in(&title) || reader.named_in(&branch);
        s
    };
    let mut moments = Vec::new();
    if let Some(at) = time(pr, "createdAt") {
        moments.push((
            at,
            Some(make(
                at,
                "pr_opened",
                MonitorSeverity::Info,
                format!("{n} opened: {title}"),
            )),
        ));
    }
    if let Some(at) = time(pr, "mergedAt") {
        moments.push((
            at,
            Some(make(
                at,
                "pr_merged",
                MonitorSeverity::Info,
                format!("{n} merged: {title}"),
            )),
        ));
    } else if text(pr, "state") == "CLOSED" {
        if let Some(at) = time(pr, "closedAt") {
            moments.push((
                at,
                Some(make(
                    at,
                    "pr_closed",
                    MonitorSeverity::Info,
                    format!("{n} closed: {title}"),
                )),
            ));
        }
    }
    moments
}

/// The moment a workflow run finished, and whether it is news.
fn workflow_run(run: &Value, reader: &Reader) -> Option<(u64, Option<MonitorSignal>)> {
    if text(run, "status") != "completed" {
        return None;
    }
    let at = time(run, "updatedAt")?;
    let conclusion = text(run, "conclusion");
    let workflow = text(run, "workflowName");
    let branch = text(run, "headBranch");
    let failed = !matches!(
        conclusion.as_str(),
        "success" | "skipped" | "neutral" | "cancelled"
    );
    if !failed && branch != TRUNK {
        return Some((at, None));
    }
    let lower = workflow.to_lowercase();
    let family = if lower.contains("release") {
        "release"
    } else if lower.contains("pages") || lower.contains("deploy") {
        "deploy"
    } else {
        "run"
    };
    let kind = format!("{family}_{}", if failed { "failed" } else { "succeeded" });
    let mut s = MonitorSignal::new(
        at,
        MonitorSource::Github,
        &kind,
        if failed {
            MonitorSeverity::Attention
        } else {
            MonitorSeverity::Info
        },
        &format!(
            "{workflow} {conclusion} on {branch}: {}",
            text(run, "displayTitle")
        ),
    );
    s.branch = Some(branch.clone()).filter(|b| !b.is_empty());
    s.link = Some(text(run, "url")).filter(|u| !u.is_empty());
    s.for_you = failed && reader.named_in(&branch);
    Some((at, Some(s)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fetched() -> Fetched {
        Fetched {
            prs: json!([
                {"number": 868, "title": "fix: the lab image survives", "state": "MERGED",
                 "createdAt": "2026-10-10T19:00:00Z", "mergedAt": "2026-10-10T20:00:00Z",
                 "closedAt": "2026-10-10T20:00:00Z", "headRefName": "fix/lab",
                 "author": {"login": "korczis"}, "url": "https://github.com/o/r/pull/868"},
                {"number": 870, "title": "a hello names its recipient", "state": "CLOSED",
                 "createdAt": "2026-10-10T19:30:00Z", "mergedAt": null,
                 "closedAt": "2026-10-10T20:30:00Z", "headRefName": "fix/1b-hello",
                 "author": {"login": "korczis"}, "url": ""},
                {"number": 871, "title": "open", "state": "OPEN",
                 "createdAt": "2026-10-10T20:45:00Z", "headRefName": "fix/y", "author": {}}
            ]),
            runs: json!([
                {"status": "completed", "conclusion": "success", "workflowName": "Deploy Pages",
                 "headBranch": "master", "updatedAt": "2026-10-10T20:10:00Z",
                 "displayTitle": "Merge #868", "url": "https://github.com/o/r/actions/runs/1"},
                {"status": "completed", "conclusion": "success", "workflowName": "ci",
                 "headBranch": "fix/y", "updatedAt": "2026-10-10T20:20:00Z", "displayTitle": "y"},
                {"status": "completed", "conclusion": "failure", "workflowName": "ci",
                 "headBranch": "fix/1b-hello", "updatedAt": "2026-10-10T20:25:00Z",
                 "displayTitle": "hello"},
                {"status": "in_progress", "conclusion": "", "workflowName": "Release",
                 "headBranch": "master", "updatedAt": "2026-10-10T20:50:00Z", "displayTitle": "v"}
            ]),
        }
    }

    #[test]
    fn every_moment_is_a_signal_and_a_green_branch_run_is_suppressed() {
        let read = fold(&fetched(), Some(0), 0, &Reader::named(Some("1b")));
        let kinds: Vec<&str> = read.signals.iter().map(|(_, s)| s.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec![
                "pr_opened",
                "pr_opened",
                "pr_merged",
                "deploy_succeeded",
                "run_failed",
                "pr_closed",
                "pr_opened"
            ]
        );
        assert_eq!(read.suppressed, 1, "the green run of a pull-request branch");
        let failed = &read.signals[4].1;
        assert_eq!(failed.severity, MonitorSeverity::Attention);
        assert!(failed.for_you, "it is the reader's branch");
        assert_eq!(
            read.signals[2].1.link.as_deref(),
            Some("https://github.com/o/r/pull/868")
        );
        assert_eq!(read.high, parse_rfc3339("2026-10-10T20:45:00Z"));
    }

    #[test]
    fn a_position_is_a_second_and_resumes_after_it() {
        let after = parse_rfc3339("2026-10-10T20:25:00Z");
        let read = fold(&fetched(), after, 0, &Reader::default());
        let kinds: Vec<&str> = read.signals.iter().map(|(_, s)| s.kind.as_str()).collect();
        assert_eq!(kinds, vec!["pr_closed", "pr_opened"]);
        let late = parse_rfc3339("2026-10-11T00:00:00Z");
        let nothing = fold(&fetched(), late, 0, &Reader::default());
        assert!(nothing.signals.is_empty());
        assert_eq!(nothing.high, late, "the position never moves back");
    }

    #[test]
    fn a_fresh_read_reaches_back_to_the_window() {
        let since = parse_rfc3339("2026-10-10T20:30:00Z").unwrap();
        let read = fold(&fetched(), None, since, &Reader::default());
        assert_eq!(read.signals.len(), 2, "closed at 20:30 and opened at 20:45");
    }

    #[test]
    fn a_missing_gh_is_absent_and_a_failing_one_is_an_error() {
        let dir = std::env::temp_dir();
        let (state, reason) =
            run("majordomus-no-such-gh-binary", &dir, &["pr"]).expect_err("no such binary");
        assert_eq!(state, MonitorSourceHealth::Absent);
        assert!(reason.contains("could not be started"), "{reason}");
        let (state, reason) = run("false", &dir, &["pr"]).expect_err("false fails");
        assert_eq!(state, MonitorSourceHealth::Error);
        assert!(reason.starts_with("gh pr failed"), "{reason}");
        let (state, reason) = run("echo", &dir, &["not json"]).expect_err("echo prints text");
        assert_eq!(state, MonitorSourceHealth::Error);
        assert!(reason.contains("printed no JSON"), "{reason}");
        assert_eq!(run("echo", &dir, &["[1]"]).unwrap(), json!([1]));
    }
}
