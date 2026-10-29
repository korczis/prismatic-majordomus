//! The work section: this session's own work, composed by the leaseholder from memory and
//! files first, and from the forge only through the monitor's shared, bounded cache.
//!
//! On the night of 2026-10-28 every blocker a session had to report was its own — a pull
//! request in conflict with master, a train whose run had timed out, a pushed branch with no
//! pull request, a build still running under the session — and the brief named none of it.
//! A session is found by what it holds: the mesh sessions keyed `.../claude-code-<psid>`
//! (its provider session) and this runtime's `.../board-<pN>` for each board peer whose
//! address carries that provider session. Their claims name the issues and, by
//! `refs/heads/<branch>` scopes, the branches; each branch's canonical worktree
//! (`<repository>-wt/<branch>`) holds its task; the forge holds its pull request and runs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::capability::handler::Context;
use crate::monitor::github::{self, Unanswered};

use super::machine::{base, Proc};
use super::{
    BriefBackground, BriefClaim, BriefDelivered, BriefObservation, BriefProcess, BriefRun,
    BriefSectionState, BriefVerdict, BriefWorkItem, BriefWorkSection,
};

/// How many runs the section asks for their jobs: the in-progress and failed ones first.
const JOB_READS: usize = 3;
/// What one read of the forge may take inside the section's budget.
const FORGE_BUDGET: Duration = github::BUDGET;
/// The commands a background process of this session is reported for.
const BACKGROUND: &[&str] = &["cargo", "rustc", "run.sh", "nextest", "cargo-nextest", "gh"];

/// What the section is composed from.
pub(crate) struct Inputs<'a> {
    pub(crate) ctx: &'a Context,
    pub(crate) root: &'a Path,
    /// The provider session the brief is about.
    pub(crate) psid: Option<&'a str>,
    /// The board ids of this checkout whose address carries that provider session.
    pub(crate) board_ids: &'a [String],
    /// Whether the composing process holds the lease: only then is the forge read.
    pub(crate) leaseholder: bool,
    /// The previous report's instant, when the caller gave one.
    pub(crate) since: Option<&'a str>,
    /// The provider's own background tasks, when its engine exposes them.
    pub(crate) background: &'a [BriefBackground],
    /// The one process table of this composition.
    pub(crate) procs: &'a Result<Vec<Proc>, String>,
}

/// The work section.
pub(crate) fn section(i: &Inputs<'_>) -> BriefWorkSection {
    let started = Instant::now();
    // a session is found by what it holds; with no provider session and no board peer there
    // is nobody to find, and the forge is not asked on nobody's behalf
    if i.psid.is_none() && i.board_ids.is_empty() {
        return BriefWorkSection {
            observed: BriefObservation {
                source: "the mesh journal (memory)".into(),
                remedy: Some("majordomus brief --session <provider session id>".into()),
                ..BriefObservation::default()
            },
            summary: "unread: no provider session and no board peer names this session".into(),
            ..BriefWorkSection::default()
        };
    }
    let mut sources: Vec<String> = vec!["mesh journal (memory)".into()];
    let mut partial: Vec<String> = Vec::new();

    // --- (a) claims and (g) the client process, from the mesh's memory
    let (claims, client_pid, session_branches) = own_claims(i);
    // a process the mesh is not active in holds no journal: its zero claims were not counted
    let claims_read = i.ctx.mesh.cooperation().is_some();
    if !claims_read {
        partial.push("claims unread: the mesh is not active in this process".into());
    }
    let mut branches: BTreeSet<String> = BTreeSet::new();
    for c in &claims {
        for s in &c.scope {
            if let Some(b) = s.strip_prefix("refs/heads/") {
                if !b.starts_with("integration/") {
                    branches.insert(b.to_string());
                }
            }
        }
    }
    let trunk = "master";
    for b in session_branches {
        if b != trunk {
            branches.insert(b);
        }
    }

    // --- (b, c) branches, their remote heads and canonical worktrees, from files
    let primary = primary_checkout(i.root);
    let common = super::refs::git_dir(i.root).map(|g| super::refs::common_dir(&g));
    let local = super::server::local_half(i.root);
    let mut items: Vec<BriefWorkItem> = branches
        .iter()
        .map(|b| {
            let remote = common
                .as_deref()
                .and_then(|c| super::refs::resolve(c, &format!("refs/remotes/origin/{b}")));
            let wt = primary.as_ref().map(|p| worktree_of(p, b));
            let wt = wt.filter(|w| w.is_dir());
            let task = wt.as_ref().and_then(|w| {
                super::super::continuity::read_task(&w.join(&local).join("state/current.yaml"))
            });
            let issue = claims
                .iter()
                .find(|c| c.scope.iter().any(|s| s == &format!("refs/heads/{b}")))
                .and_then(|c| c.issue.clone())
                .or_else(|| {
                    task.as_ref()
                        .map(|t| t.issue.clone())
                        .filter(|s| !s.is_empty())
                });
            BriefWorkItem {
                branch: b.clone(),
                issue,
                pushed: remote.is_some(),
                head: remote,
                worktree: wt.map(|w| w.display().to_string()),
                task: task
                    .as_ref()
                    .filter(|t| t.outcome == "active")
                    .map(|t| t.id.clone()),
                task_outcome: task.map(|t| t.outcome),
                ..BriefWorkItem::default()
            }
        })
        .collect();
    sources.push("refs and canonical worktrees (files)".into());

    // --- (d, e, f) the forge, through the monitor's shared cache, leaseholder only
    let mut forge_age: Option<Duration> = None;
    let mut trains: BTreeMap<u64, String> = BTreeMap::new();
    if i.leaseholder {
        match github::open_prs(i.root, FORGE_BUDGET) {
            Ok(a) => {
                forge_age = Some(a.age);
                for item in items.iter_mut() {
                    if let Some(pr) = a
                        .value
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|p| p["headRefName"].as_str() == Some(item.branch.as_str()))
                    {
                        item.pr = pr["number"].as_u64();
                        item.title = pr["title"].as_str().map(str::to_string);
                        item.merge_state = pr["mergeStateStatus"].as_str().map(str::to_string);
                        if let Some(h) = pr["headRefOid"].as_str() {
                            item.head = Some(h.to_string());
                        }
                    }
                }
                sources.push(format!("gh pr list (cache, {} s old)", a.age.as_secs()));
            }
            Err(e) => partial.push(format!("pull requests unread: {}", e.reason())),
        }
        trains = trains_of(i.ctx, &items);
        for item in items.iter_mut() {
            item.train = item.pr.and_then(|n| trains.get(&n).cloned());
        }
        // the latest run of each own branch and of each train carrying an own PR, in parallel
        let asked: Vec<String> = items
            .iter()
            .map(|it| it.branch.clone())
            .chain(trains.values().cloned())
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect();
        let runs = latest_runs(i.root, &asked);
        let mut wanted_jobs: BTreeSet<u64> = BTreeSet::new();
        for item in items.iter_mut() {
            let on = item.train.clone().unwrap_or_else(|| item.branch.clone());
            match runs.get(&on) {
                Some(Ok(Some(r))) => {
                    if r.wants_jobs() {
                        wanted_jobs.insert(r.id);
                    }
                    item.run = Some(r.clone());
                }
                Some(Ok(None)) => {}
                Some(Err(e)) => partial.push(format!("runs of {on} unread: {e}")),
                None => {}
            }
        }
        let wanted_jobs: Vec<u64> = wanted_jobs.into_iter().take(JOB_READS).collect();
        let jobs = jobs_of(i.root, &wanted_jobs);
        for item in items.iter_mut() {
            if let Some(r) = item.run.as_mut() {
                if let Some(Ok(j)) = jobs.get(&r.id) {
                    j.apply(r);
                }
            }
        }
        sources.push("gh run list / gh run view (cache)".into());
    } else {
        partial.push("forge not read: not the leaseholder".into());
    }
    for item in items.iter_mut() {
        item.word = word_of(item).to_string();
    }

    // --- (g) background processes under the client, from the one process table
    let background = match (client_pid, i.procs) {
        (Some(pid), Ok(table)) => descendants(table, pid)
            .into_iter()
            .filter(|p| is_background(p))
            .map(|p| BriefProcess {
                pid: p.pid,
                command: describe(p),
                age_seconds: p.age_seconds,
            })
            .collect(),
        (None, _) => {
            partial.push("background unread: no client pid on the session".into());
            Vec::new()
        }
        (_, Err(e)) => {
            partial.push(format!("background unread: {e}"));
            Vec::new()
        }
    };

    // --- (h) delivered since the previous report
    let delivered = match i.since {
        Some(since) => delivered_since(i.root, since, &items),
        None => Vec::new(),
    };

    let held = claims.iter().filter(|c| c.state == "held").count();
    let conflicted = claims.iter().filter(|c| c.state == "conflicted").count();
    let suspended = claims.iter().filter(|c| c.state == "suspended").count();
    let prs = items.iter().filter(|it| it.pr.is_some()).count();
    let state = if partial.is_empty() {
        BriefSectionState::Read
    } else {
        BriefSectionState::Partial
    };
    let verdict = match state {
        BriefSectionState::Read => BriefVerdict::Current,
        BriefSectionState::Partial => BriefVerdict::Partial,
        _ => BriefVerdict::Unread,
    };
    // unread claims are not counted: the partial note below says why, once
    let claims_words = if claims_read {
        format!("{held} claim(s) held, {conflicted} conflicted, {suspended} suspended; ")
    } else {
        String::new()
    };
    let mut summary = format!(
        "[{}] {claims_words}{} branch(es), {prs} PR(s)",
        verdict.as_str(),
        items.len()
    );
    if let Some(a) = forge_age {
        summary.push_str(&format!("; forge cache {} s old", a.as_secs()));
    }
    if !partial.is_empty() {
        summary.push_str(&format!("; {}", partial.join("; ")));
    }
    BriefWorkSection {
        state,
        observed: BriefObservation {
            source: sources.join(", "),
            observed_at: Some(crate::ledger::now()),
            age_seconds: forge_age.map(|a| a.as_secs()),
            bound_seconds: Some(github::served_bound(github::BRIEF_TTL).as_secs()),
            verdict,
            remedy: (verdict != BriefVerdict::Current).then(|| {
                if i.leaseholder {
                    "gh pr list --author @me ; majordomus mesh state".to_string()
                } else {
                    "majordomus serve ensure".to_string()
                }
            }),
            elapsed_ms: started.elapsed().as_millis() as u64,
            budget_ms: 0,
        },
        held,
        conflicted,
        suspended,
        claims,
        items,
        trains: trains
            .into_iter()
            .map(|(pr, train)| super::BriefTrain {
                pr,
                branch: train,
                source: "claim intent".into(),
            })
            .collect(),
        background,
        provider_background: i.background.to_vec(),
        provider_background_exposed: !i.background.is_empty(),
        delivered,
        summary,
    }
}

/// This session's claims (held, conflicted, suspended), the pid of its client, and the
/// branches its mesh sessions say they are on.
fn own_claims(i: &Inputs<'_>) -> (Vec<BriefClaim>, Option<u32>, Vec<String>) {
    use crate::mesh::state::ClaimState;
    let Some(coop) = i.ctx.mesh.cooperation() else {
        return (Vec::new(), board_pid(i), Vec::new());
    };
    let runtime = i.ctx.mesh.cooperation_status().runtime.unwrap_or_default();
    let provider_suffix = i.psid.map(|p| format!("/claude-code-{p}"));
    let board_suffixes: Vec<String> = i
        .board_ids
        .iter()
        .map(|id| format!("/board-{id}"))
        .collect();
    let state = coop.state();
    let mine = |key: &str, rt: &str| {
        provider_suffix.as_deref().is_some_and(|s| key.ends_with(s))
            || (rt == runtime && board_suffixes.iter().any(|s| key.ends_with(s.as_str())))
    };
    let mut client_pid = None;
    let mut branches = Vec::new();
    for s in &state.sessions {
        if mine(&s.key, &s.runtime) {
            if client_pid.is_none() {
                client_pid = s.info.client_pid;
            }
            if let Some(b) = &s.info.branch {
                branches.push(b.clone());
            }
        }
    }
    let claims = state
        .claims
        .iter()
        .filter(|c| mine(&c.session, &c.runtime))
        .filter_map(|c| {
            let word = match &c.state {
                ClaimState::Held => "held",
                ClaimState::Conflicted(_) => "conflicted",
                ClaimState::Suspended(_) => "suspended",
                _ => return None,
            };
            Some(BriefClaim {
                key: c.key.clone(),
                issue: c.issue.clone(),
                intent: c.intent.clone(),
                scope: c.scope.clone(),
                state: word.into(),
            })
        })
        .collect();
    (claims, client_pid.or_else(|| board_pid(i)), branches)
}

/// The pid a board peer of this session carries in its address.
fn board_pid(i: &Inputs<'_>) -> Option<u32> {
    i.ctx
        .peers
        .list()
        .into_iter()
        .filter(|p| i.board_ids.iter().any(|id| id == p.id.as_str()))
        .find_map(|p| p.address.and_then(|a| a.pid))
}

/// The primary checkout: the directory holding the common git directory.
pub(crate) fn primary_checkout(root: &Path) -> Option<PathBuf> {
    let common = super::refs::common_dir(&super::refs::git_dir(root)?);
    common.canonicalize().ok()?.parent().map(Path::to_path_buf)
}

/// The canonical worktree of a branch: `<primary>-wt/<branch>` (`project.worktree-topology`).
pub(crate) fn worktree_of(primary: &Path, branch: &str) -> PathBuf {
    let name = primary
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    primary.with_file_name(format!("{name}-wt")).join(branch)
}

/// The train carrying each own pull request: a held claim of any runtime on an
/// `integration/train-*` branch whose intent names the PR.
fn trains_of(ctx: &Context, items: &[BriefWorkItem]) -> BTreeMap<u64, String> {
    use crate::mesh::state::ClaimState;
    let mut out = BTreeMap::new();
    let Some(coop) = ctx.mesh.cooperation() else {
        return out;
    };
    let prs: Vec<u64> = items.iter().filter_map(|i| i.pr).collect();
    if prs.is_empty() {
        return out;
    }
    let state = coop.state();
    for c in state
        .claims
        .iter()
        .filter(|c| matches!(c.state, ClaimState::Held))
    {
        let Some(train) = c.scope.iter().find_map(|s| {
            let s = s.strip_prefix("refs/heads/").unwrap_or(s);
            s.starts_with("integration/train-").then(|| s.to_string())
        }) else {
            continue;
        };
        let intent = c.intent.as_deref().unwrap_or_default();
        for pr in &prs {
            if mentions(intent, *pr) {
                out.entry(*pr).or_insert_with(|| train.clone());
            }
        }
    }
    out
}

/// Whether `text` names `#<pr>` as a whole number.
pub(crate) fn mentions(text: &str, pr: u64) -> bool {
    let needle = format!("#{pr}");
    text.match_indices(&needle)
        .any(|(at, _)| !text[at + needle.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// The latest run of each branch, asked in parallel; `Ok(None)` for a branch with no run.
fn latest_runs(
    root: &Path,
    branches: &[String],
) -> BTreeMap<String, Result<Option<BriefRun>, String>> {
    std::thread::scope(|s| {
        let handles: Vec<_> = branches
            .iter()
            .map(|b| {
                let b = b.clone();
                s.spawn(move || {
                    let r = github::runs_by_branch(root, &b, FORGE_BUDGET)
                        .map(|a| {
                            a.value
                                .as_array()
                                .and_then(|v| v.first())
                                .map(BriefRun::from_gh)
                        })
                        .map_err(|e| e.reason().to_string());
                    (b, r)
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// What a run's jobs say.
pub(crate) struct Jobs {
    done: u32,
    total: u32,
    queued: u32,
    failed: Vec<String>,
}

impl Jobs {
    pub(crate) fn of(value: &Value) -> Jobs {
        let jobs = value["jobs"].as_array().cloned().unwrap_or_default();
        let st = |j: &Value, k: &str| j[k].as_str().unwrap_or_default().to_string();
        Jobs {
            total: jobs.len() as u32,
            done: jobs
                .iter()
                .filter(|j| st(j, "status") == "completed")
                .count() as u32,
            queued: jobs
                .iter()
                .filter(|j| matches!(st(j, "status").as_str(), "queued" | "waiting" | "pending"))
                .count() as u32,
            failed: jobs
                .iter()
                .filter(|j| {
                    matches!(
                        st(j, "conclusion").as_str(),
                        "failure" | "timed_out" | "startup_failure"
                    )
                })
                .map(|j| {
                    let c = st(j, "conclusion");
                    let n = st(j, "name");
                    if c == "failure" {
                        n
                    } else {
                        format!("{n} ({c})")
                    }
                })
                .collect(),
        }
    }

    pub(crate) fn apply(&self, r: &mut BriefRun) {
        r.jobs_done = Some(self.done);
        r.jobs_total = Some(self.total);
        r.jobs_queued = Some(self.queued);
        r.failed_jobs = self.failed.clone();
    }
}

/// The jobs of each run, asked in parallel.
fn jobs_of(root: &Path, ids: &[u64]) -> BTreeMap<u64, Result<Jobs, Unanswered>> {
    std::thread::scope(|s| {
        let handles: Vec<_> = ids
            .iter()
            .map(|id| {
                let id = *id;
                s.spawn(move || {
                    (
                        id,
                        github::run_jobs(root, id, FORGE_BUDGET).map(|a| Jobs::of(&a.value)),
                    )
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// The status word of one item of work, worst first.
pub(crate) fn word_of(item: &BriefWorkItem) -> &'static str {
    if item.run.as_ref().is_some_and(BriefRun::is_failing) {
        return "failing";
    }
    match (item.pr, item.merge_state.as_deref()) {
        (Some(_), Some("DIRTY")) => "dirty",
        (Some(_), Some("BLOCKED")) => "blocked",
        (Some(_), Some("BEHIND")) => "behind",
        (Some(_), Some("CLEAN")) | (Some(_), Some("HAS_HOOKS")) => "ready",
        (Some(_), Some("UNSTABLE")) => "warn",
        (Some(_), Some("DRAFT")) => "draft",
        (Some(_), _) => "unknown",
        (None, _) if item.pushed => "unmerged",
        (None, _) => "detached",
    }
}

/// Every process under `pid`, breadth first.
fn descendants(table: &[Proc], pid: u32) -> Vec<&Proc> {
    let mut out = Vec::new();
    let mut frontier = vec![pid];
    while let Some(p) = frontier.pop() {
        for child in table.iter().filter(|c| c.ppid == p && c.pid != p) {
            out.push(child);
            frontier.push(child.pid);
        }
    }
    out
}

fn is_background(p: &Proc) -> bool {
    let b = base(&p.command);
    BACKGROUND.contains(&b) || p.command.contains("scripts/") || p.command.contains("test/run.sh")
}

fn describe(p: &Proc) -> String {
    let b = base(&p.command);
    if b == "cargo" {
        "cargo".into()
    } else {
        b.to_string()
    }
}

/// Own pull requests merged into origin/master since `since`, and own branch heads whose
/// tip was committed after it: one bounded `git log` and one bounded `git show`.
fn delivered_since(root: &Path, since: &str, items: &[BriefWorkItem]) -> Vec<BriefDelivered> {
    let mut out = Vec::new();
    let own: BTreeSet<&str> = items.iter().map(|i| i.branch.as_str()).collect();
    if let Some(text) = git_bounded(
        root,
        &[
            "log",
            "--first-parent",
            &format!("--since={since}"),
            "--format=%H%x09%cI%x09%s",
            "refs/remotes/origin/master",
        ],
    ) {
        for l in text.lines() {
            let mut f = l.splitn(3, '\t');
            let (Some(sha), Some(at), Some(subject)) = (f.next(), f.next(), f.next()) else {
                continue;
            };
            let Some((pr, branch)) = merged_pr(subject) else {
                continue;
            };
            if own.contains(branch.as_str()) {
                out.push(BriefDelivered {
                    kind: "merged".into(),
                    pr: Some(pr),
                    branch,
                    sha: sha.chars().take(10).collect(),
                    at: at.to_string(),
                });
            }
        }
    }
    let heads: Vec<&BriefWorkItem> = items.iter().filter(|i| i.head.is_some()).collect();
    if !heads.is_empty() {
        let mut args = vec!["show", "-s", "--format=%H%x09%cI"];
        args.extend(heads.iter().filter_map(|i| i.head.as_deref()));
        if let Some(text) = git_bounded(root, &args) {
            let then = crate::peers::epoch_seconds(since).unwrap_or(i64::MAX);
            for l in text.lines() {
                let Some((sha, at)) = l.split_once('\t') else {
                    continue;
                };
                if crate::peers::epoch_seconds(at).is_some_and(|t| t > then) {
                    if let Some(item) = heads.iter().find(|i| i.head.as_deref() == Some(sha)) {
                        out.push(BriefDelivered {
                            kind: "pushed".into(),
                            pr: item.pr,
                            branch: item.branch.clone(),
                            sha: sha.chars().take(10).collect(),
                            at: at.to_string(),
                        });
                    }
                }
            }
        }
    }
    out
}

/// `Merge pull request #2653 from korczis/chore/x` → `(2653, "chore/x")`.
pub(crate) fn merged_pr(subject: &str) -> Option<(u64, String)> {
    let rest = subject.strip_prefix("Merge pull request #")?;
    let (n, rest) = rest.split_once(" from ")?;
    let branch = rest.split_once('/').map(|(_, b)| b).unwrap_or(rest).trim();
    Some((n.trim().parse().ok()?, branch.to_string()))
}

/// One git call bounded by a second; its stdout, or nothing.
pub(crate) fn git_bounded(root: &Path, args: &[&str]) -> Option<String> {
    use std::io::Read as _;
    let mut child = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match child.try_wait() {
            Ok(Some(st)) if st.success() => return reader.join().ok(),
            Ok(Some(_)) => return None,
            _ if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merge_subject_names_its_pull_request_and_branch() {
        assert_eq!(
            merged_pr("Merge pull request #2653 from korczis/chore/the-incidents"),
            Some((2653, "chore/the-incidents".into()))
        );
        assert_eq!(merged_pr("merge: master into x"), None);
        assert!(mentions("train 10a carries #2678 and #26", 2678));
        assert!(!mentions("#26789", 2678));
    }

    #[test]
    fn the_canonical_worktree_is_the_primarys_sibling() {
        assert_eq!(
            worktree_of(Path::new("/d/repo"), "feature/x"),
            PathBuf::from("/d/repo-wt/feature/x")
        );
    }

    #[test]
    fn jobs_are_counted_and_failures_named() {
        let v = serde_json::json!({ "jobs": [
            { "name": "build", "status": "completed", "conclusion": "success" },
            { "name": "structure", "status": "completed", "conclusion": "timed_out" },
            { "name": "lint", "status": "completed", "conclusion": "failure" },
            { "name": "suite", "status": "queued", "conclusion": "" },
        ]});
        let j = Jobs::of(&v);
        let mut r = BriefRun::default();
        j.apply(&mut r);
        assert_eq!(
            (r.jobs_done, r.jobs_total, r.jobs_queued),
            (Some(3), Some(4), Some(1))
        );
        assert_eq!(r.failed_jobs, ["structure (timed_out)", "lint"]);
    }

    #[test]
    fn the_background_is_the_clients_descendants_that_build_or_ask() {
        let p = |pid, ppid, c: &str| Proc {
            pid,
            ppid,
            age_seconds: 840,
            command: c.into(),
        };
        let table = vec![
            p(10, 1, "claude"),
            p(11, 10, "/bin/zsh"),
            p(12, 11, "/x/cargo"),
            p(13, 12, "/x/rustc"),
            p(20, 1, "/x/cargo"),
        ];
        let under: Vec<u32> = descendants(&table, 10)
            .into_iter()
            .filter(|p| is_background(p))
            .map(|p| p.pid)
            .collect();
        assert_eq!(
            under.len(),
            2,
            "cargo and its rustc, not another session's cargo"
        );
        assert!(under.contains(&12) && under.contains(&13));
    }

    #[test]
    fn claims_a_process_without_the_mesh_cannot_count_are_unread_not_zero() {
        // measured 2026-10-29: a command line composing the brief said "0 claim(s) held" for a
        // session holding nine, because the journal it would count lives in the leaseholder
        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        assert!(ctx.mesh.cooperation().is_none(), "the fixture runs no mesh");
        let root = PathBuf::from(&ctx.index.repository.root);
        let procs: Result<Vec<Proc>, String> = Ok(Vec::new());
        let s = section(&Inputs {
            ctx: &ctx,
            root: &root,
            psid: Some("07a5ffde-74b3-4ca6-ab0d-1ad2c9e56d9e"),
            board_ids: &[],
            leaseholder: false,
            since: None,
            background: &[],
            procs: &procs,
        });
        assert_eq!(s.state, BriefSectionState::Partial, "{}", s.summary);
        assert!(!s.summary.contains("claim(s) held"), "{}", s.summary);
        assert_eq!(
            s.summary.matches("claims unread").count(),
            1,
            "{}",
            s.summary
        );
        assert!(
            s.summary
                .contains("claims unread: the mesh is not active in this process"),
            "{}",
            s.summary
        );
    }

    #[test]
    fn each_item_has_its_word() {
        let mut it = BriefWorkItem {
            branch: "b".into(),
            pushed: true,
            ..BriefWorkItem::default()
        };
        assert_eq!(word_of(&it), "unmerged");
        it.pr = Some(1);
        it.merge_state = Some("DIRTY".into());
        assert_eq!(word_of(&it), "dirty");
        it.run = Some(BriefRun {
            status: "completed".into(),
            conclusion: Some("failure".into()),
            ..BriefRun::default()
        });
        assert_eq!(word_of(&it), "failing");
    }
}
