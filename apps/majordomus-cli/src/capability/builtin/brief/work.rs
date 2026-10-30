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
/// What the section keeps of its budget for what it composes after the forge: the stages
/// share one deadline, so that a slow forge leaves the local items standing rather than
/// running the whole section past its budget (2026-10-30: three stages of 4 s each in a 5 s
/// budget replaced claims, branches, worktrees and tasks already read with "unread").
const AFTER_FORGE: Duration = Duration::from_millis(600);
/// How many open pull requests one read lists: past it, a branch with none among them is not
/// said to have none.
const PR_LIMIT: usize = 200;
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
    /// This session's entries on the boards the peers section read, when this process holds
    /// no mesh and the peers section answered inside its budget.
    pub(crate) board: Option<&'a OwnBoard>,
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
    // a process the mesh is not active in holds no journal: its claims are the board's
    // announcements, and when the board did not answer either, its zero claims were not counted
    let from_mesh = i.ctx.mesh.cooperation().is_some();
    let from_board = !from_mesh && i.board.is_some();
    let claims_read = from_mesh || from_board;
    if from_board {
        sources[0] = "the board's announcements (this process holds no mesh)".into();
    }
    if !claims_read {
        partial.push(
            "claims unread: the mesh is not active in this process and the board did not answer inside its budget"
                .into(),
        );
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

    // --- (d, e, f) the forge, through the monitor's shared cache, leaseholder only, every
    // stage inside one deadline
    let deadline = started + super::budget_of("work").saturating_sub(AFTER_FORGE);
    let left = || {
        deadline
            .saturating_duration_since(Instant::now())
            .min(FORGE_BUDGET)
    };
    let mut forge_age: Option<Duration> = None;
    let mut trains: BTreeMap<u64, String> = BTreeMap::new();
    let mut prs_read = false;
    if i.leaseholder {
        let budget = left();
        match (budget > Duration::ZERO)
            .then(|| github::open_prs(i.root, budget))
            .unwrap_or_else(|| Err(Unanswered::Failed("the section's budget ran out".into())))
        {
            Ok(a) => {
                forge_age = Some(a.age);
                let listed = a.value.as_array().map_or(0, Vec::len);
                // a full page may have cut an own pull request: none found is then no fact
                prs_read = listed < PR_LIMIT;
                if !prs_read {
                    partial.push(format!(
                        "pull requests: the first {PR_LIMIT} open ones were read, and a branch with none among them is {}",
                        super::report::NOT_READ
                    ));
                }
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
        let heads: BTreeMap<String, String> = items
            .iter()
            .filter_map(|it| Some((it.branch.clone(), it.head.clone()?)))
            .collect();
        let runs = latest_runs(i.root, &asked, &heads, left());
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
        let jobs = jobs_of(i.root, &wanted_jobs, left());
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
    // a branch with no open pull request whose head master already holds was merged (or its
    // work landed another way): it is never told to open a duplicate (2026-10-30)
    let master = common
        .as_deref()
        .and_then(|c| super::refs::resolve(c, "refs/remotes/origin/master"));
    for item in items.iter_mut() {
        item.word = word_of_read(item, prs_read).to_string();
        if item.word == "unmerged" {
            if let (Some(h), Some(m)) = (item.head.as_deref(), master.as_deref()) {
                match is_ancestor(i.root, h, m) {
                    Some(true) => item.word = "merged".into(),
                    Some(false) => {}
                    None => {
                        item.word = "unread".into();
                        partial.push(format!(
                            "whether master holds {} unread: git merge-base did not answer",
                            item.branch
                        ));
                    }
                }
            }
        }
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

    // --- (h) delivered since the previous report: an empty list is a fact only when it was
    // read (2026-10-30: "Delivered: none" for a session that had just pushed, because the
    // caller gave no previous instant)
    let (delivered, delivered_unread) = match i.since {
        None => (
            Vec::new(),
            Some("the caller named no previous report (--since)".to_string()),
        ),
        Some(_) if !claims_read => (
            Vec::new(),
            Some("this session's claims, and so its branches, were not read".to_string()),
        ),
        Some(since) => match delivered_since(i.root, since, &items) {
            Ok(d) => (d, None),
            Err(why) => (Vec::new(), Some(why)),
        },
    };
    if let Some(why) = &delivered_unread {
        if i.since.is_some() {
            partial.push(format!("delivered unread: {why}"));
        }
    }

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
    let claims_words = if from_mesh {
        format!("{held} claim(s) held, {conflicted} conflicted, {suspended} suspended; ")
    } else if from_board {
        format!(
            "{} claim(s) announced on the board (no mesh in this process); ",
            claims.len()
        )
    } else {
        String::new()
    };
    // pull requests that were not looked up are not counted as none
    let prs_words = if prs_read {
        format!("{prs} PR(s)")
    } else {
        "pull requests [unread]".to_string()
    };
    let mut summary = format!(
        "[{}] {claims_words}{} branch(es), {prs_words}",
        verdict.as_str(),
        items.len()
    );
    if let Some(a) = forge_age {
        summary.push_str(&format!("; forge cache {} s old", a.as_secs()));
    }
    if i.background.is_empty() {
        summary.push_str("; provider background [undeclared]");
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
        pull_requests_read: prs_read,
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
        delivered_unread,
        summary,
    }
}

/// This session's claims (held, conflicted, suspended), the pid of its client, and the
/// branches its mesh sessions say they are on.
fn own_claims(i: &Inputs<'_>) -> (Vec<BriefClaim>, Option<u32>, Vec<String>) {
    use crate::mesh::state::ClaimState;
    let Some(coop) = i.ctx.mesh.cooperation() else {
        return match i.board {
            Some(b) => (board_claims(b), board_pid(i).or(b.pid), Vec::new()),
            None => (Vec::new(), board_pid(i), Vec::new()),
        };
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

/// This session's own entries on the boards the peers section read: every attached peer whose
/// address carries the brief's provider session, with what it announced and, when its client
/// runs on this machine, the client's pid.
///
/// A process the mesh is not active in (a command line in a checkout whose server is absent)
/// holds no journal, and on 2026-10-30 the brief such a process composed said "0 branch(es),
/// 0 PR(s); claims unread" two lines above the board it had read, where this session's peer
/// announced its branch. The board is what that process can read, so the work section is
/// composed from it, and says so.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OwnBoard {
    /// Each own peer's id and one of its announcements.
    pub(crate) claims: Vec<(String, crate::peers::Announcement)>,
    /// The client's pid, when a peer of this session runs on this machine.
    pub(crate) pid: Option<u32>,
}

impl OwnBoard {
    /// The entries of `peers` that are this session's: attached, and carrying `psid`.
    pub(crate) fn of(peers: &[&crate::peers::Peer], psid: Option<&str>) -> Self {
        let Some(psid) = psid else {
            return Self::default();
        };
        let here = this_host();
        let mut own = Self::default();
        for p in peers.iter().filter(|p| p.attached) {
            let Some(a) = p.address.as_ref() else {
                continue;
            };
            if a.session.as_deref() != Some(psid) {
                continue;
            }
            if own.pid.is_none() && here.as_deref().is_none_or(|h| same_host(h, &a.host)) {
                own.pid = a.pid;
            }
            for c in &p.claims {
                own.claims.push((p.id.as_str().to_string(), c.clone()));
            }
        }
        own
    }
}

/// The claims of this session's board entries, as the work section lists them: `announced`,
/// because a board holds an announcement and never a lease.
fn board_claims(b: &OwnBoard) -> Vec<BriefClaim> {
    b.claims
        .iter()
        .map(|(peer, c)| BriefClaim {
            key: match &c.name {
                Some(n) => format!("board-{peer}/{n}"),
                None => format!("board-{peer}"),
            },
            issue: c
                .name
                .as_deref()
                .and_then(issue_in)
                .or_else(|| issue_in(&c.intent)),
            intent: Some(c.intent.clone()),
            scope: c.scope.clone(),
            state: "announced".into(),
        })
        .collect()
}

/// The first issue id a text names (`I4390`), standing as a word of its own.
pub(crate) fn issue_in(text: &str) -> Option<String> {
    let b = text.as_bytes();
    (0..b.len()).find_map(|at| {
        let starts = b[at] == b'I' && (at == 0 || !b[at - 1].is_ascii_alphanumeric());
        if !starts {
            return None;
        }
        let digits = b[at + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .count();
        let ends = b
            .get(at + 1 + digits)
            .is_none_or(|c| !c.is_ascii_alphanumeric());
        (digits >= 3 && ends).then(|| text[at..at + 1 + digits].to_string())
    })
}

/// This machine's host name, from `gethostname(3)`: no process is started for it.
fn this_host() -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: `buf` is a writable buffer of `buf.len()` bytes; the name is read only when the
    // call succeeds, and only up to its first NUL, which the slice bound keeps inside `buf`.
    let rc = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.len()) };
    if rc != 0 {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let name = String::from_utf8_lossy(&buf[..end]).trim().to_string();
    (!name.is_empty()).then_some(name)
}

/// Whether two host names are one machine: equal, or equal once a `.local` style domain is
/// taken off either.
fn same_host(a: &str, b: &str) -> bool {
    let short = |h: &str| h.split('.').next().unwrap_or(h).to_ascii_lowercase();
    a.eq_ignore_ascii_case(b) || short(a) == short(b)
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

/// The latest run of the required workflow on each branch's own head, asked in parallel;
/// `Ok(None)` for a branch with none yet. Another workflow's run (pages, labeler) is not the
/// branch's verdict, and a run of a head the branch has moved past is not its head's: on
/// 2026-10-30 a newer pages run hid a failing validate, and a superseded head's failure made
/// the current head `[failing]`. A train's head is not known here, so its runs are taken by
/// workflow alone.
fn latest_runs(
    root: &Path,
    branches: &[String],
    heads: &BTreeMap<String, String>,
    budget: Duration,
) -> BTreeMap<String, Result<Option<BriefRun>, String>> {
    if budget.is_zero() {
        return branches
            .iter()
            .map(|b| (b.clone(), Err("the section's budget ran out".to_string())))
            .collect();
    }
    std::thread::scope(|s| {
        let handles: Vec<_> = branches
            .iter()
            .map(|b| {
                let b = b.clone();
                let head = heads.get(&b).cloned();
                s.spawn(move || {
                    let r = github::runs_by_branch(root, &b, budget)
                        .map(|a| {
                            a.value
                                .as_array()
                                .and_then(|v| required_run(v, head.as_deref()))
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

/// The newest run of the required workflow, on `head` when it is known.
pub(crate) fn required_run<'v>(runs: &'v [Value], head: Option<&str>) -> Option<&'v Value> {
    runs.iter().find(|r| {
        r["workflowName"].as_str() == Some(super::REQUIRED_WORKFLOW)
            && head.is_none_or(|h| r["headSha"].as_str() == Some(h))
    })
}

/// Whether `ancestor` is reachable from `of`: `git merge-base --is-ancestor`, bounded. None
/// when git did not answer inside its bound.
fn is_ancestor(root: &Path, ancestor: &str, of: &str) -> Option<bool> {
    match git_status_bounded(root, &["merge-base", "--is-ancestor", ancestor, of])? {
        0 => Some(true),
        1 => Some(false),
        _ => None,
    }
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
fn jobs_of(root: &Path, ids: &[u64], budget: Duration) -> BTreeMap<u64, Result<Jobs, Unanswered>> {
    if budget.is_zero() {
        return BTreeMap::new();
    }
    std::thread::scope(|s| {
        let handles: Vec<_> = ids
            .iter()
            .map(|id| {
                let id = *id;
                s.spawn(move || {
                    (
                        id,
                        github::run_jobs(root, id, budget).map(|a| Jobs::of(&a.value)),
                    )
                })
            })
            .collect();
        handles.into_iter().filter_map(|h| h.join().ok()).collect()
    })
}

/// The status word of one item of work when its pull request may not have been looked up: a
/// branch with none found is `unmerged` only when the forge's pull requests were read, and
/// `unread` otherwise. On 2026-10-30 a composition that was not the leaseholder called a branch
/// "[unmerged] no PR" and raised `branch.no_pr` without having asked the forge.
pub(crate) fn word_of_read(item: &BriefWorkItem, prs_read: bool) -> &'static str {
    match word_of(item) {
        "unmerged" if !prs_read => "unread",
        w => w,
    }
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

/// Own pull requests merged into origin/master since `since`, own branch heads that reached
/// origin after it, and release tags created after it: one bounded `git log`, the
/// remote-tracking refs' reflogs (files), one bounded `git show` for the heads no reflog
/// entry dates, and one bounded `git for-each-ref`. Every instant is UTC. A git call that does
/// not answer leaves the list unread, with why, never empty.
fn delivered_since(
    root: &Path,
    since: &str,
    items: &[BriefWorkItem],
) -> Result<Vec<BriefDelivered>, String> {
    let then = crate::peers::epoch_seconds(since)
        .ok_or_else(|| format!("the previous report's instant {since} is not RFC 3339"))?;
    let mut out = Vec::new();
    let own: BTreeSet<&str> = items.iter().map(|i| i.branch.as_str()).collect();
    let text = git_bounded(
        root,
        &[
            "log",
            "--first-parent",
            &format!("--since={since}"),
            "--format=%H%x09%ct%x09%s",
            "refs/remotes/origin/master",
        ],
    )
    .ok_or("git log of origin/master did not answer inside its bound")?;
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
                at: at
                    .parse()
                    .map(super::utc)
                    .unwrap_or_else(|_| at.to_string()),
                at_source: "merge".into(),
            });
        }
    }
    // a head is delivered when origin took it after `since`: by its remote-tracking ref's
    // reflog, which a push from this clone or a fetch writes; the commit time only where no
    // entry dates it, and said as such
    let mut undated: Vec<&BriefWorkItem> = Vec::new();
    for item in items.iter().filter(|i| i.head.is_some()) {
        let head = item.head.as_deref().unwrap_or_default();
        match super::refs::took_head_at(root, &item.branch, head) {
            Some(t) => {
                let epoch = t
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                if epoch > then {
                    out.push(BriefDelivered {
                        kind: "pushed".into(),
                        pr: item.pr,
                        branch: item.branch.clone(),
                        sha: head.chars().take(10).collect(),
                        at: super::utc(epoch),
                        at_source: "push".into(),
                    });
                }
            }
            None => undated.push(item),
        }
    }
    if !undated.is_empty() {
        let mut args = vec!["show", "-s", "--format=%H%x09%ct"];
        args.extend(undated.iter().filter_map(|i| i.head.as_deref()));
        let text = git_bounded(root, &args)
            .ok_or("git show of the own branch heads did not answer inside its bound")?;
        for l in text.lines() {
            let Some((sha, at)) = l.split_once('\t') else {
                continue;
            };
            let Ok(epoch) = at.trim().parse::<i64>() else {
                continue;
            };
            if epoch > then {
                if let Some(item) = undated.iter().find(|i| i.head.as_deref() == Some(sha)) {
                    out.push(BriefDelivered {
                        kind: "pushed".into(),
                        pr: item.pr,
                        branch: item.branch.clone(),
                        sha: sha.chars().take(10).collect(),
                        at: super::utc(epoch),
                        at_source: "commit".into(),
                    });
                }
            }
        }
    }
    let tags = git_bounded(
        root,
        &[
            "for-each-ref",
            "--sort=-creatordate",
            "--count=20",
            "--format=%(refname:short)%09%(creatordate:unix)%09%(objectname)",
            "refs/tags/v*",
        ],
    )
    .ok_or("git for-each-ref of the release tags did not answer inside its bound")?;
    for l in tags.lines() {
        let mut f = l.splitn(3, '\t');
        let (Some(tag), Some(at), Some(sha)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        let Ok(epoch) = at.trim().parse::<i64>() else {
            continue;
        };
        if epoch > then {
            out.push(BriefDelivered {
                kind: "tagged".into(),
                pr: None,
                branch: tag.to_string(),
                sha: sha.trim().chars().take(10).collect(),
                at: super::utc(epoch),
                at_source: "tag".into(),
            });
        }
    }
    Ok(out)
}

/// `Merge pull request #2653 from korczis/chore/x` → `(2653, "chore/x")`.
pub(crate) fn merged_pr(subject: &str) -> Option<(u64, String)> {
    let rest = subject.strip_prefix("Merge pull request #")?;
    let (n, rest) = rest.split_once(" from ")?;
    let branch = rest.split_once('/').map(|(_, b)| b).unwrap_or(rest).trim();
    Some((n.trim().parse().ok()?, branch.to_string()))
}

/// One git call bounded by a second; its exit code, or nothing when it did not end in time.
fn git_status_bounded(root: &Path, args: &[&str]) -> Option<i32> {
    let mut child = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match child.try_wait() {
            Ok(Some(st)) => return st.code(),
            _ if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
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
    fn a_branchs_run_is_the_required_workflows_on_its_own_head() {
        // 2026-10-30: a newer pages run hid a failing validate, and a superseded head's
        // failure made the current head [failing]
        let runs = serde_json::json!([
            { "databaseId": 3, "workflowName": "pages", "headSha": "new", "status": "completed",
              "conclusion": "success" },
            { "databaseId": 2, "workflowName": "validate", "headSha": "old", "status": "completed",
              "conclusion": "failure" },
            { "databaseId": 1, "workflowName": "validate", "headSha": "new", "status": "in_progress" },
        ]);
        let runs = runs.as_array().unwrap();
        assert_eq!(
            required_run(runs, Some("new")).map(|r| r["databaseId"].clone()),
            Some(1.into())
        );
        assert_eq!(
            required_run(runs, None).map(|r| r["databaseId"].clone()),
            Some(2.into())
        );
        assert_eq!(required_run(runs, Some("none yet")), None);
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
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

    #[test]
    fn what_was_delivered_is_merged_pushed_and_tagged_in_utc() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let root = dir.path().join("clone");
        std::fs::create_dir(&root).unwrap();
        git(&root, &["init", "-q", "-b", "master"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", "base"]);
        let since = crate::peers::rfc3339(std::time::SystemTime::now() - Duration::from_secs(3600));
        git(&root, &["checkout", "-q", "-b", "feature/x"]);
        git(&root, &["commit", "-q", "--allow-empty", "-m", "work"]);
        let head = git(&root, &["rev-parse", "HEAD"]);
        // origin took the head now: the remote-tracking ref's reflog dates the push
        git(
            &root,
            &["update-ref", "refs/remotes/origin/feature/x", &head],
        );
        git(&root, &["checkout", "-q", "master"]);
        git(
            &root,
            &[
                "merge",
                "-q",
                "--no-ff",
                "-m",
                "Merge pull request #7 from o/feature/x",
                "feature/x",
            ],
        );
        let merged = git(&root, &["rev-parse", "HEAD"]);
        git(
            &root,
            &["update-ref", "refs/remotes/origin/master", &merged],
        );
        git(&root, &["tag", "-a", "v9.9.9", "-m", "a release"]);
        let items = vec![BriefWorkItem {
            branch: "feature/x".into(),
            head: Some(head.clone()),
            pushed: true,
            ..BriefWorkItem::default()
        }];
        let d = delivered_since(&root, &since, &items).expect("read");
        let kinds: Vec<(&str, &str)> = d
            .iter()
            .map(|x| (x.kind.as_str(), x.at_source.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [("merged", "merge"), ("pushed", "push"), ("tagged", "tag")],
            "{d:?}"
        );
        assert!(d.iter().all(|x| x.at.ends_with('Z')), "{d:?}");
        assert_eq!(d[0].pr, Some(7));
        assert_eq!(d[2].branch, "v9.9.9");
        // a head the reflog does not date is said with its commit time, as such
        std::fs::remove_dir_all(
            super::super::refs::git_dir(&root)
                .unwrap()
                .join("logs/refs/remotes/origin/feature"),
        )
        .expect("the reflog");
        let d = delivered_since(&root, &since, &items).expect("read");
        assert!(
            d.iter()
                .any(|x| x.kind == "pushed" && x.at_source == "commit"),
            "{d:?}"
        );
        // a previous instant that is no instant is unread, never "none"
        assert!(delivered_since(&root, "yesterday", &items).is_err());
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
            board: None,
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

    /// A board entry as a server answers it: `id` attached for provider session `session`.
    fn board_peer(
        id: &str,
        host: &str,
        pid: u32,
        session: &str,
        claims: Value,
    ) -> crate::peers::Peer {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "client": { "name": "claude-code", "version": "2.1.314" },
            "transport": "http",
            "connected_at": "2026-10-30T04:00:00Z",
            "last_seen_seconds_ago": 1,
            "attached": true,
            "claims": claims,
            "address": { "host": host, "pid": pid, "session": session },
        }))
        .expect("a board entry")
    }

    #[test]
    fn a_process_without_the_mesh_takes_its_own_work_from_the_board() {
        // measured 2026-10-30 in a linked worktree whose server was absent: the brief read the
        // board, where this session's peer announced its branch, and two lines above it said
        // "0 branch(es), 0 PR(s); claims unread: the mesh is not active in this process"
        let psid = "07a5ffde-74b3-4ca6-ab0d-1ad2c9e56d9e";
        let here = this_host().unwrap_or_else(|| "this-host".into());
        let mine = board_peer(
            "p2",
            &here,
            4041,
            psid,
            serde_json::json!([
                { "name": "I4390", "intent": "I4390 continuing the brief",
                  "scope": ["refs/heads/feature/the-brief-tells-the-truth", "share/brief/"],
                  "at": "2026-10-30T04:11:41Z" },
                { "name": "disk", "intent": "DISK on 641b", "scope": ["machine/641b/disk"],
                  "at": "2026-10-29T03:22:24Z" },
                { "name": "train", "intent": "the train I hold",
                  "scope": ["refs/heads/integration/train-12c-20261029"],
                  "at": "2026-10-29T22:50:00Z" },
            ]),
        );
        let other = board_peer(
            "p9",
            &here,
            11643,
            "61e1283f-0000-0000-0000-000000000000",
            serde_json::json!([{ "intent": "I4298 links", "scope": ["refs/heads/feature/links"],
                                 "at": "2026-10-30T01:00:00Z" }]),
        );
        let elsewhere = board_peer("p4", "lundra", 77, psid, serde_json::json!([]));
        let own = OwnBoard::of(&[&other, &elsewhere, &mine], Some(psid));
        assert_eq!(
            own.claims.len(),
            3,
            "this session's claims and nobody else's"
        );
        assert_eq!(
            own.pid,
            Some(4041),
            "the pid of the peer on this machine, never lundra's"
        );
        assert_eq!(OwnBoard::of(&[&mine], None), OwnBoard::default());

        let repo = crate::synthetic::SyntheticRepository::small().expect("a repository");
        let ctx = repo.context().expect("a context");
        assert!(ctx.mesh.cooperation().is_none(), "the fixture runs no mesh");
        let root = PathBuf::from(&ctx.index.repository.root);
        let procs: Result<Vec<Proc>, String> = Ok(vec![
            Proc {
                pid: 4041,
                ppid: 1,
                age_seconds: 9000,
                command: "claude".into(),
            },
            Proc {
                pid: 5000,
                ppid: 4041,
                age_seconds: 840,
                command: "/x/cargo".into(),
            },
        ]);
        let s = section(&Inputs {
            ctx: &ctx,
            root: &root,
            psid: Some(psid),
            board_ids: &[],
            leaseholder: false,
            since: None,
            background: &[],
            procs: &procs,
            board: Some(&own),
        });
        let branches: Vec<&str> = s.items.iter().map(|i| i.branch.as_str()).collect();
        assert_eq!(
            branches,
            ["feature/the-brief-tells-the-truth"],
            "{}",
            s.summary
        );
        assert_eq!(s.items[0].issue.as_deref(), Some("I4390"));
        // not the leaseholder: the forge was not asked, so no branch is said to have no PR
        assert!(!s.pull_requests_read);
        assert!(
            s.items.iter().all(|i| i.word != "unmerged"),
            "{:?}",
            s.items
        );
        assert!(
            s.summary.contains("pull requests [unread]"),
            "{}",
            s.summary
        );
        assert!(!s.summary.contains("0 PR(s)"), "{}", s.summary);
        assert!(
            s.claims.iter().all(|c| c.state == "announced"),
            "{:?}",
            s.claims
        );
        assert!(
            s.summary
                .contains("3 claim(s) announced on the board (no mesh in this process)"),
            "{}",
            s.summary
        );
        assert!(!s.summary.contains("claims unread"), "{}", s.summary);
        assert!(!s.summary.contains("no client pid"), "{}", s.summary);
        assert_eq!(
            s.background.iter().map(|b| b.pid).collect::<Vec<_>>(),
            [5000]
        );
        assert!(
            s.observed.source.starts_with("the board's announcements"),
            "{}",
            s.observed.source
        );
    }

    #[test]
    fn an_issue_is_a_word_of_its_own() {
        assert_eq!(issue_in("I4390").as_deref(), Some("I4390"));
        assert_eq!(issue_in("id:issue:I4382 taken").as_deref(), Some("I4382"));
        assert_eq!(issue_in("I3903-N8").as_deref(), Some("I3903"));
        assert_eq!(issue_in("TAKING now (I4337 fix)").as_deref(), Some("I4337"));
        assert_eq!(issue_in("DISK on 641b"), None);
        assert_eq!(issue_in("WI4390 or I12"), None);
        assert!(same_host(
            "Tomass-MacBook-Pro-2.local",
            "tomass-macbook-pro-2"
        ));
        assert!(!same_host(
            "Tomass-MacBook-Pro.local",
            "Tomass-MacBook-Pro-2.local"
        ));
    }

    #[test]
    fn each_item_has_its_word() {
        let mut it = BriefWorkItem {
            branch: "b".into(),
            pushed: true,
            ..BriefWorkItem::default()
        };
        assert_eq!(word_of(&it), "unmerged");
        assert_eq!(word_of_read(&it, true), "unmerged");
        assert_eq!(
            word_of_read(&it, false),
            "unread",
            "a pull request nobody looked up is not called absent"
        );
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
